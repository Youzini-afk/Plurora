use std::collections::HashMap;
use std::io::SeekFrom;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll};

use async_trait::async_trait;
use bytes::Bytes;
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::RwLock;

pub const SHA256_DIGEST_PREFIX: &str = "sha256:";

pub type ObjectStream = Pin<Box<dyn AsyncRead + Send + Unpin + 'static>>;

struct IntegrityCheckingReader<R> {
    inner: R,
    expected: String,
    hasher: Option<Sha256>,
    verified: bool,
    failure: Option<String>,
}

impl<R> IntegrityCheckingReader<R> {
    fn new(inner: R, expected: impl Into<String>) -> Self {
        Self {
            inner,
            expected: expected.into(),
            hasher: Some(Sha256::new()),
            verified: false,
            failure: None,
        }
    }
}

impl<R> AsyncRead for IntegrityCheckingReader<R>
where
    R: AsyncRead + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        if let Some(message) = &self.failure {
            return Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                message.clone(),
            )));
        }
        if self.verified || buffer.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }

        let before = buffer.filled().len();
        match Pin::new(&mut self.inner).poll_read(cx, buffer) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(error)) => Poll::Ready(Err(error)),
            Poll::Ready(Ok(())) => {
                let after = buffer.filled().len();
                if after > before {
                    if let Some(hasher) = self.hasher.as_mut() {
                        hasher.update(&buffer.filled()[before..after]);
                    }
                    return Poll::Ready(Ok(()));
                }

                let hasher = self
                    .hasher
                    .take()
                    .expect("unverified reader always retains its hasher");
                let actual = format!("{SHA256_DIGEST_PREFIX}{:x}", hasher.finalize());
                if actual != self.expected {
                    let message = ObjectStoreError::Integrity {
                        expected: self.expected.clone(),
                        actual,
                    }
                    .to_string();
                    self.failure = Some(message.clone());
                    return Poll::Ready(Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        message,
                    )));
                }
                self.verified = true;
                Poll::Ready(Ok(()))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectInfo {
    pub digest: String,
    pub size_bytes: u64,
}

#[derive(Debug, Error)]
pub enum ObjectStoreError {
    #[error("invalid object digest '{digest}': {reason}")]
    InvalidDigest { digest: String, reason: String },
    #[error("unsupported object digest algorithm '{algorithm}'")]
    UnsupportedDigestAlgorithm { algorithm: String },
    #[error("object '{digest}' was not found")]
    NotFound { digest: String },
    #[error("object integrity check failed: expected '{expected}', computed '{actual}'")]
    Integrity { expected: String, actual: String },
    #[error(
        "artifact descriptor size mismatch for '{digest}': expected {expected_size} bytes, found {actual_size} bytes"
    )]
    DescriptorSizeMismatch {
        digest: String,
        expected_size: u64,
        actual_size: u64,
    },
    #[error("object store I/O failed at '{}': {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

impl ObjectStoreError {
    fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}

#[async_trait]
pub trait ObjectStore: Send + Sync {
    async fn put(&self, bytes: Bytes) -> Result<ObjectInfo, ObjectStoreError>;
    async fn get(&self, digest: &str) -> Result<Bytes, ObjectStoreError>;
    async fn has(&self, digest: &str) -> Result<bool, ObjectStoreError>;
    async fn verify(&self, digest: &str) -> Result<ObjectInfo, ObjectStoreError>;
    async fn stream(&self, digest: &str) -> Result<ObjectStream, ObjectStoreError>;
}

pub fn sha256_digest(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("{SHA256_DIGEST_PREFIX}{digest:x}")
}

fn sha256_hex(digest: &str) -> Result<&str, ObjectStoreError> {
    let Some((algorithm, value)) = digest.split_once(':') else {
        return Err(ObjectStoreError::InvalidDigest {
            digest: digest.to_string(),
            reason: "missing algorithm prefix".to_string(),
        });
    };
    if algorithm != "sha256" {
        return Err(ObjectStoreError::UnsupportedDigestAlgorithm {
            algorithm: algorithm.to_string(),
        });
    }
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ObjectStoreError::InvalidDigest {
            digest: digest.to_string(),
            reason: "sha256 value must be exactly 64 lowercase hexadecimal characters".to_string(),
        });
    }
    Ok(value)
}

fn verify_bytes(expected: &str, bytes: &[u8]) -> Result<ObjectInfo, ObjectStoreError> {
    sha256_hex(expected)?;
    let actual = sha256_digest(bytes);
    if actual != expected {
        return Err(ObjectStoreError::Integrity {
            expected: expected.to_string(),
            actual,
        });
    }
    Ok(ObjectInfo {
        digest: expected.to_string(),
        size_bytes: bytes.len() as u64,
    })
}

#[derive(Debug, Default)]
pub struct InMemoryObjectStore {
    objects: RwLock<HashMap<String, Bytes>>,
}

impl InMemoryObjectStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl ObjectStore for InMemoryObjectStore {
    async fn put(&self, bytes: Bytes) -> Result<ObjectInfo, ObjectStoreError> {
        let digest = sha256_digest(&bytes);
        let info = ObjectInfo {
            digest: digest.clone(),
            size_bytes: bytes.len() as u64,
        };
        self.objects.write().await.entry(digest).or_insert(bytes);
        Ok(info)
    }

    async fn get(&self, digest: &str) -> Result<Bytes, ObjectStoreError> {
        sha256_hex(digest)?;
        let bytes = self
            .objects
            .read()
            .await
            .get(digest)
            .cloned()
            .ok_or_else(|| ObjectStoreError::NotFound {
                digest: digest.to_string(),
            })?;
        verify_bytes(digest, &bytes)?;
        Ok(bytes)
    }

    async fn has(&self, digest: &str) -> Result<bool, ObjectStoreError> {
        sha256_hex(digest)?;
        Ok(self.objects.read().await.contains_key(digest))
    }

    async fn verify(&self, digest: &str) -> Result<ObjectInfo, ObjectStoreError> {
        let bytes = self.get(digest).await?;
        Ok(ObjectInfo {
            digest: digest.to_string(),
            size_bytes: bytes.len() as u64,
        })
    }

    async fn stream(&self, digest: &str) -> Result<ObjectStream, ObjectStoreError> {
        let bytes = self.get(digest).await?;
        Ok(Box::pin(IntegrityCheckingReader::new(
            std::io::Cursor::new(bytes),
            digest,
        )))
    }
}

#[derive(Debug, Clone)]
pub struct FilesystemObjectStore {
    root: PathBuf,
}

impl FilesystemObjectStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn object_path(&self, digest: &str) -> Result<PathBuf, ObjectStoreError> {
        let value = sha256_hex(digest)?;
        Ok(self.root.join("sha256").join(value))
    }

    async fn verify_path(&self, digest: &str, path: &Path) -> Result<ObjectInfo, ObjectStoreError> {
        let mut file = tokio::fs::File::open(path).await.map_err(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                ObjectStoreError::NotFound {
                    digest: digest.to_string(),
                }
            } else {
                ObjectStoreError::io(path, source)
            }
        })?;
        self.verify_file(digest, path, &mut file).await
    }

    async fn verify_file(
        &self,
        digest: &str,
        path: &Path,
        file: &mut tokio::fs::File,
    ) -> Result<ObjectInfo, ObjectStoreError> {
        sha256_hex(digest)?;
        let mut hasher = Sha256::new();
        let mut size_bytes = 0_u64;
        let mut buffer = vec![0_u8; 64 * 1024];
        loop {
            let read = file
                .read(&mut buffer)
                .await
                .map_err(|source| ObjectStoreError::io(path, source))?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            size_bytes += read as u64;
        }
        let actual = format!("{SHA256_DIGEST_PREFIX}{:x}", hasher.finalize());
        if actual != digest {
            return Err(ObjectStoreError::Integrity {
                expected: digest.to_string(),
                actual,
            });
        }
        Ok(ObjectInfo {
            digest: digest.to_string(),
            size_bytes,
        })
    }

    async fn prepare_object_directory(&self, directory: &Path) -> Result<(), ObjectStoreError> {
        let root_existed = tokio::fs::try_exists(&self.root)
            .await
            .map_err(|source| ObjectStoreError::io(&self.root, source))?;
        tokio::fs::create_dir_all(&self.root)
            .await
            .map_err(|source| ObjectStoreError::io(&self.root, source))?;
        if !root_existed {
            if let Some(parent) = self.root.parent() {
                sync_directory(parent).await?;
            }
        }

        let directory_existed = tokio::fs::try_exists(directory)
            .await
            .map_err(|source| ObjectStoreError::io(directory, source))?;
        tokio::fs::create_dir_all(directory)
            .await
            .map_err(|source| ObjectStoreError::io(directory, source))?;
        if !directory_existed {
            sync_directory(&self.root).await?;
        }
        Ok(())
    }
}

#[cfg(unix)]
async fn sync_directory(path: &Path) -> Result<(), ObjectStoreError> {
    let directory = tokio::fs::File::open(path)
        .await
        .map_err(|source| ObjectStoreError::io(path, source))?;
    directory
        .sync_all()
        .await
        .map_err(|source| ObjectStoreError::io(path, source))
}

#[cfg(not(unix))]
async fn sync_directory(_path: &Path) -> Result<(), ObjectStoreError> {
    Ok(())
}

#[async_trait]
impl ObjectStore for FilesystemObjectStore {
    async fn put(&self, bytes: Bytes) -> Result<ObjectInfo, ObjectStoreError> {
        let digest = sha256_digest(&bytes);
        let info = ObjectInfo {
            digest: digest.clone(),
            size_bytes: bytes.len() as u64,
        };
        let path = self.object_path(&digest)?;
        if tokio::fs::try_exists(&path)
            .await
            .map_err(|source| ObjectStoreError::io(&path, source))?
        {
            self.verify_path(&digest, &path).await?;
            return Ok(info);
        }

        let parent = path.parent().expect("object path always has a parent");
        self.prepare_object_directory(parent).await?;
        let temp_path = parent.join(format!(".tmp-{}", uuid::Uuid::new_v4()));
        let write_result = async {
            let mut file = tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp_path)
                .await
                .map_err(|source| ObjectStoreError::io(&temp_path, source))?;
            file.write_all(&bytes)
                .await
                .map_err(|source| ObjectStoreError::io(&temp_path, source))?;
            file.flush()
                .await
                .map_err(|source| ObjectStoreError::io(&temp_path, source))?;
            file.sync_all()
                .await
                .map_err(|source| ObjectStoreError::io(&temp_path, source))?;
            drop(file);
            match tokio::fs::rename(&temp_path, &path).await {
                Ok(()) => sync_directory(parent).await,
                Err(rename_error) => match tokio::fs::try_exists(&path).await {
                    Ok(true) => {
                        let _ = tokio::fs::remove_file(&temp_path).await;
                        self.verify_path(&digest, &path).await.map(|_| ())
                    }
                    Ok(false) => Err(ObjectStoreError::io(&path, rename_error)),
                    Err(source) => Err(ObjectStoreError::io(&path, source)),
                },
            }
        }
        .await;
        if write_result.is_err() {
            let _ = tokio::fs::remove_file(&temp_path).await;
        }
        write_result?;
        Ok(info)
    }

    async fn get(&self, digest: &str) -> Result<Bytes, ObjectStoreError> {
        let path = self.object_path(digest)?;
        let bytes = tokio::fs::read(&path).await.map_err(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                ObjectStoreError::NotFound {
                    digest: digest.to_string(),
                }
            } else {
                ObjectStoreError::io(&path, source)
            }
        })?;
        verify_bytes(digest, &bytes)?;
        Ok(Bytes::from(bytes))
    }

    async fn has(&self, digest: &str) -> Result<bool, ObjectStoreError> {
        let path = self.object_path(digest)?;
        tokio::fs::try_exists(&path)
            .await
            .map_err(|source| ObjectStoreError::io(path, source))
    }

    async fn verify(&self, digest: &str) -> Result<ObjectInfo, ObjectStoreError> {
        let path = self.object_path(digest)?;
        self.verify_path(digest, &path).await
    }

    async fn stream(&self, digest: &str) -> Result<ObjectStream, ObjectStoreError> {
        let path = self.object_path(digest)?;
        let mut file = tokio::fs::File::open(&path).await.map_err(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                ObjectStoreError::NotFound {
                    digest: digest.to_string(),
                }
            } else {
                ObjectStoreError::io(&path, source)
            }
        })?;
        self.verify_file(digest, &path, &mut file).await?;
        file.seek(SeekFrom::Start(0))
            .await
            .map_err(|source| ObjectStoreError::io(&path, source))?;
        Ok(Box::pin(IntegrityCheckingReader::new(file, digest)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn identical_bytes_have_identical_digests_across_stores() {
        let memory = InMemoryObjectStore::new();
        let directory = tempfile::tempdir().unwrap();
        let filesystem = FilesystemObjectStore::new(directory.path());
        let bytes = Bytes::from_static(b"portable object");

        let memory_info = memory.put(bytes.clone()).await.unwrap();
        let filesystem_info = filesystem.put(bytes).await.unwrap();

        assert_eq!(memory_info, filesystem_info);
        assert!(memory_info.digest.starts_with(SHA256_DIGEST_PREFIX));
    }

    #[tokio::test]
    async fn filesystem_store_rejects_tampered_content() {
        let directory = tempfile::tempdir().unwrap();
        let store = FilesystemObjectStore::new(directory.path());
        let info = store.put(Bytes::from_static(b"original")).await.unwrap();
        let path = store.object_path(&info.digest).unwrap();
        tokio::fs::write(path, b"tampered").await.unwrap();

        assert!(matches!(
            store.get(&info.digest).await,
            Err(ObjectStoreError::Integrity { .. })
        ));
        assert!(matches!(
            store.stream(&info.digest).await,
            Err(ObjectStoreError::Integrity { .. })
        ));
    }

    #[tokio::test]
    async fn stream_returns_verified_object_bytes() {
        let store = InMemoryObjectStore::new();
        let info = store.put(Bytes::from_static(b"stream me")).await.unwrap();
        let mut stream = store.stream(&info.digest).await.unwrap();
        let mut output = Vec::new();
        stream.read_to_end(&mut output).await.unwrap();
        assert_eq!(output, b"stream me");
    }

    #[tokio::test]
    async fn stream_detects_mutation_after_open() {
        let directory = tempfile::tempdir().unwrap();
        let store = FilesystemObjectStore::new(directory.path());
        let info = store.put(Bytes::from_static(b"original")).await.unwrap();
        let mut stream = store.stream(&info.digest).await.unwrap();
        let path = store.object_path(&info.digest).unwrap();
        tokio::fs::write(path, b"tampered").await.unwrap();
        let mut output = Vec::new();
        assert!(stream.read_to_end(&mut output).await.is_err());
    }
}
