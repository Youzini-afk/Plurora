use std::collections::BTreeMap;

use bytes::Bytes;
use plurora_core::ArtifactDescriptor;
use serde_json::Value;

use super::Runtime;
use crate::{EventStore, ObjectInfo, ObjectStoreError, ObjectStream};

pub const GENERIC_BLOB_ARTIFACT_TYPE_URI: &str = "urn:plurora:artifact:blob:v1";

#[derive(Debug, Clone)]
pub struct ArtifactCommitRequest {
    pub artifact_type_uri: String,
    pub media_type: String,
    pub bytes: Bytes,
    pub references: Vec<String>,
    pub annotations: BTreeMap<String, Value>,
}

impl ArtifactCommitRequest {
    pub fn blob(media_type: impl Into<String>, bytes: impl Into<Bytes>) -> Self {
        Self {
            artifact_type_uri: GENERIC_BLOB_ARTIFACT_TYPE_URI.to_string(),
            media_type: media_type.into(),
            bytes: bytes.into(),
            references: Vec::new(),
            annotations: BTreeMap::new(),
        }
    }
}

impl<S> Runtime<S>
where
    S: EventStore,
{
    pub async fn commit_artifact(
        &self,
        request: ArtifactCommitRequest,
    ) -> Result<ArtifactDescriptor, ObjectStoreError> {
        let info = self.config.object_store.put(request.bytes).await?;
        Ok(ArtifactDescriptor {
            artifact_type_uri: request.artifact_type_uri,
            media_type: request.media_type,
            digest: info.digest,
            size_bytes: info.size_bytes,
            references: request.references,
            annotations: request.annotations,
        })
    }

    pub async fn read_artifact(
        &self,
        descriptor: &ArtifactDescriptor,
    ) -> Result<Bytes, ObjectStoreError> {
        let bytes = self.config.object_store.get(&descriptor.digest).await?;
        ensure_descriptor_size(descriptor, bytes.len() as u64)?;
        Ok(bytes)
    }

    pub async fn verify_artifact(
        &self,
        descriptor: &ArtifactDescriptor,
    ) -> Result<ObjectInfo, ObjectStoreError> {
        let info = self.config.object_store.verify(&descriptor.digest).await?;
        ensure_descriptor_size(descriptor, info.size_bytes)?;
        Ok(info)
    }

    pub async fn stream_artifact(
        &self,
        descriptor: &ArtifactDescriptor,
    ) -> Result<ObjectStream, ObjectStoreError> {
        self.verify_artifact(descriptor).await?;
        self.config.object_store.stream(&descriptor.digest).await
    }
}

fn ensure_descriptor_size(
    descriptor: &ArtifactDescriptor,
    actual_size: u64,
) -> Result<(), ObjectStoreError> {
    if actual_size != descriptor.size_bytes {
        return Err(ObjectStoreError::DescriptorSizeMismatch {
            digest: descriptor.digest.clone(),
            expected_size: descriptor.size_bytes,
            actual_size,
        });
    }
    Ok(())
}
