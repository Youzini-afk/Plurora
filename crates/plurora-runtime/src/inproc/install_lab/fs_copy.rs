use std::ffi::OsStr;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};

use anyhow::Result;

const EXCLUDED_NAMES: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    ".DS_Store",
    "node_modules",
    "target",
    ".venv",
    "venv",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
];

pub(super) const MAX_FILES: u64 = 25_000;
pub(super) const MAX_DIRECTORIES: u64 = 25_000;
pub(super) const MAX_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Default)]
struct CopyStats {
    files: u64,
    directories: u64,
    bytes: u64,
}

impl CopyStats {
    fn add_directory(&mut self) -> Result<()> {
        self.directories = self.directories.saturating_add(1);
        anyhow::ensure!(
            self.directories <= MAX_DIRECTORIES,
            "external workspace directory count limit exceeded"
        );
        Ok(())
    }

    fn add_file(&mut self, bytes: u64) -> Result<()> {
        self.files = self.files.saturating_add(1);
        self.bytes = self.bytes.saturating_add(bytes);
        anyhow::ensure!(
            self.files <= MAX_FILES,
            "external workspace file count limit exceeded"
        );
        anyhow::ensure!(
            self.bytes <= MAX_BYTES,
            "external workspace byte limit exceeded"
        );
        Ok(())
    }
}

pub(super) fn ensure_non_overlapping_roots(source: &Path, destination: &Path) -> Result<()> {
    if destination.starts_with(source) || source.starts_with(destination) {
        anyhow::bail!("source and managed workspace roots must not overlap");
    }
    Ok(())
}

pub(super) fn copy_external_tree_bounded(source: &Path, destination: &Path) -> Result<()> {
    let destination = ManagedDirectory::create(destination)?;
    copy_external_tree_bounded_into_impl(source, &destination, &mut |_| {})
}

fn copy_external_tree_bounded_impl<F>(
    source: &Path,
    destination: &Path,
    before_open: &mut F,
) -> Result<()>
where
    F: FnMut(&Path),
{
    let destination = ManagedDirectory::create(destination)?;
    copy_external_tree_bounded_into_impl(source, &destination, before_open)
}

pub(super) fn copy_external_tree_bounded_into(
    source: &Path,
    destination: &ManagedDirectory,
) -> Result<()> {
    copy_external_tree_bounded_into_impl(source, destination, &mut |_| {})
}

fn copy_external_tree_bounded_into_impl<F>(
    source: &Path,
    destination: &ManagedDirectory,
    before_open: &mut F,
) -> Result<()>
where
    F: FnMut(&Path),
{
    let source_root = fs::canonicalize(source)
        .map_err(|_| anyhow::anyhow!("external source root could not be canonicalized"))?;
    ensure_non_overlapping_roots(&source_root, destination.path())?;
    let mut stats = CopyStats::default();
    platform::copy_root(&source_root, destination.inner(), &mut stats, before_open)
}

pub(super) struct ManagedDirectory {
    inner: platform::ManagedDirectory,
}

impl ManagedDirectory {
    pub(super) fn open(path: &Path) -> Result<Self> {
        Ok(Self {
            inner: platform::ManagedDirectory::open(path)?,
        })
    }

    pub(super) fn create(path: &Path) -> Result<Self> {
        Ok(Self {
            inner: platform::ManagedDirectory::create(path)?,
        })
    }

    pub(super) fn create_child(&self, name: &OsStr) -> Result<Self> {
        validate_child_name(name)?;
        Ok(Self {
            inner: self.inner.create_child(name)?,
        })
    }

    pub(super) fn path(&self) -> &Path {
        self.inner.path()
    }

    pub(super) fn stable_access_path(&self) -> Result<PathBuf> {
        self.inner.stable_access_path()
    }

    pub(super) fn ensure_path_identity(&self) -> Result<()> {
        self.inner.ensure_path_identity()
    }

    pub(super) fn promote_child(&self, child: &ManagedDirectory, name: &OsStr) -> Result<()> {
        validate_child_name(name)?;
        self.inner.promote_child(&child.inner, name)
    }

    pub(super) fn atomic_write(&self, name: &OsStr, bytes: &[u8]) -> Result<()> {
        validate_child_name(name)?;
        self.inner.atomic_write(name, bytes)
    }

    pub(super) fn remove(self) -> Result<()> {
        self.inner.remove()
    }

    fn inner(&self) -> &platform::ManagedDirectory {
        &self.inner
    }
}

fn validate_child_name(name: &OsStr) -> Result<()> {
    let mut components = Path::new(name).components();
    anyhow::ensure!(
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none(),
        "managed workspace child name is invalid"
    );
    Ok(())
}

fn is_excluded(name: &std::ffi::OsStr) -> bool {
    name.to_str()
        .is_some_and(|name| EXCLUDED_NAMES.contains(&name))
}

fn copy_open_file(
    source: &fs::File,
    mut destination: fs::File,
    stats: &mut CopyStats,
) -> Result<()> {
    stats.add_file(0)?;
    let remaining = MAX_BYTES.saturating_sub(stats.bytes);
    let mut source = source.take(remaining.saturating_add(1));
    let copied = io::copy(&mut source, &mut destination)?;
    anyhow::ensure!(
        copied <= remaining,
        "external workspace byte limit exceeded"
    );
    stats.bytes = stats.bytes.saturating_add(copied);
    destination.flush()?;
    Ok(())
}

fn validate_relative_symlink(
    source_root: &Path,
    source_parent: &Path,
    relative_parent: &Path,
    target: &Path,
) -> Result<PathBuf> {
    anyhow::ensure!(
        !target.is_absolute(),
        "external workspace contains an absolute symlink"
    );

    let mut relative = relative_parent.to_path_buf();
    for component in target.components() {
        match component {
            Component::Normal(part) => relative.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                anyhow::ensure!(
                    relative.pop(),
                    "external workspace symlink escapes its root"
                );
            }
            Component::Prefix(_) | Component::RootDir => {
                anyhow::bail!("external workspace contains an absolute symlink");
            }
        }
    }

    let resolved = fs::canonicalize(source_parent.join(target))
        .map_err(|_| anyhow::anyhow!("external workspace contains a dangling symlink"))?;
    anyhow::ensure!(
        resolved.starts_with(source_root),
        "external workspace symlink escapes its root"
    );
    Ok(resolved)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
mod platform {
    use std::ffi::{c_char, CString, OsStr, OsString};
    use std::fs::{self, File, OpenOptions};
    use std::io::Write;
    use std::os::fd::{AsRawFd, FromRawFd, RawFd};
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    use std::path::{Path, PathBuf};

    use anyhow::{Context, Result};

    use super::{copy_open_file, is_excluded, validate_relative_symlink, CopyStats};

    const AT_FDCWD: i32 = -100;
    const AT_REMOVEDIR: i32 = 0x200;
    const O_CLOEXEC: i32 = 0o2000000;
    const O_CREAT: i32 = 0o100;
    const O_DIRECTORY: i32 = 0o200000;
    const O_EXCL: i32 = 0o200;
    const O_NOFOLLOW: i32 = 0o400000;
    const O_PATH: i32 = 0o10000000;
    const O_WRONLY: i32 = 0o1;
    const RENAME_NOREPLACE: u32 = 1;

    unsafe extern "C" {
        fn openat(dirfd: i32, pathname: *const c_char, flags: i32, ...) -> i32;
        fn mkdirat(dirfd: i32, pathname: *const c_char, mode: u32) -> i32;
        fn readlinkat(
            dirfd: i32,
            pathname: *const c_char,
            buffer: *mut c_char,
            buffer_size: usize,
        ) -> isize;
        fn renameat2(
            olddirfd: i32,
            oldpath: *const c_char,
            newdirfd: i32,
            newpath: *const c_char,
            flags: u32,
        ) -> i32;
        fn symlinkat(target: *const c_char, newdirfd: i32, linkpath: *const c_char) -> i32;
        fn unlinkat(dirfd: i32, pathname: *const c_char, flags: i32) -> i32;
    }

    pub(super) struct ManagedDirectory {
        file: File,
        path: PathBuf,
        parent: Option<File>,
        name: Option<OsString>,
        device: u64,
        inode: u64,
    }

    impl ManagedDirectory {
        pub(super) fn open(path: &Path) -> Result<Self> {
            let expected = fs::canonicalize(path)
                .map_err(|_| anyhow::anyhow!("managed destination could not be canonicalized"))?;
            let metadata = fs::symlink_metadata(path)?;
            anyhow::ensure!(
                metadata.is_dir() && !metadata.file_type().is_symlink(),
                "managed destination must be a real directory"
            );
            let file = open_path_handle(path)?;
            let metadata = file.metadata()?;
            anyhow::ensure!(metadata.is_dir(), "managed destination must be a directory");
            let actual = fs::canonicalize(proc_fd_path(file.as_raw_fd()))?;
            anyhow::ensure!(
                actual == expected,
                "managed destination changed while it was being opened"
            );
            Ok(Self {
                file,
                path: expected,
                parent: None,
                name: None,
                device: metadata.dev(),
                inode: metadata.ino(),
            })
        }

        pub(super) fn create(path: &Path) -> Result<Self> {
            let parent_path = path
                .parent()
                .ok_or_else(|| anyhow::anyhow!("managed destination has no parent"))?;
            let name = path
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("managed destination has no name"))?;
            super::validate_child_name(name)?;
            Self::open(parent_path)?.create_child(name)
        }

        pub(super) fn create_child(&self, name: &OsStr) -> Result<Self> {
            self.ensure_path_identity()?;
            let name_c = c_string(name, "managed workspace name contains a null byte")?;
            let result = unsafe { mkdirat(self.file.as_raw_fd(), name_c.as_ptr(), 0o700) };
            if result != 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            let file = match open_directory_relative(self.file.as_raw_fd(), &name_c) {
                Ok(file) => file,
                Err(error) => {
                    let _ =
                        unsafe { unlinkat(self.file.as_raw_fd(), name_c.as_ptr(), AT_REMOVEDIR) };
                    return Err(error);
                }
            };
            let metadata = file.metadata()?;
            let child = Self {
                file,
                path: self.path.join(name),
                parent: Some(self.file.try_clone()?),
                name: Some(name.to_os_string()),
                device: metadata.dev(),
                inode: metadata.ino(),
            };
            if let Err(error) = child.ensure_path_identity() {
                let _ = unsafe { unlinkat(self.file.as_raw_fd(), name_c.as_ptr(), AT_REMOVEDIR) };
                return Err(error);
            }
            Ok(child)
        }

        pub(super) fn path(&self) -> &Path {
            &self.path
        }

        pub(super) fn stable_access_path(&self) -> Result<PathBuf> {
            self.ensure_path_identity()?;
            Ok(proc_fd_path(self.file.as_raw_fd()))
        }

        pub(super) fn ensure_path_identity(&self) -> Result<()> {
            let actual = fs::canonicalize(proc_fd_path(self.file.as_raw_fd()))
                .map_err(|_| anyhow::anyhow!("managed destination has no stable path"))?;
            let current = fs::canonicalize(&self.path)
                .map_err(|_| anyhow::anyhow!("managed destination path changed"))?;
            let current_metadata = fs::symlink_metadata(&self.path)?;
            anyhow::ensure!(
                actual == current
                    && !current_metadata.file_type().is_symlink()
                    && current_metadata.dev() == self.device
                    && current_metadata.ino() == self.inode,
                "managed destination identity changed"
            );
            Ok(())
        }

        pub(super) fn promote_child(&self, child: &Self, name: &OsStr) -> Result<()> {
            self.ensure_path_identity()?;
            child.ensure_path_identity()?;
            let parent = child
                .parent
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("managed staging directory has no owner"))?;
            let parent_metadata = parent.metadata()?;
            anyhow::ensure!(
                parent_metadata.dev() == self.device && parent_metadata.ino() == self.inode,
                "managed staging directory has a different owner"
            );
            let old_name = c_string(
                child.name.as_deref().unwrap_or_default(),
                "managed staging name contains a null byte",
            )?;
            let new_name = c_string(name, "managed source name contains a null byte")?;
            let result = unsafe {
                renameat2(
                    self.file.as_raw_fd(),
                    old_name.as_ptr(),
                    self.file.as_raw_fd(),
                    new_name.as_ptr(),
                    RENAME_NOREPLACE,
                )
            };
            if result != 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            Ok(())
        }

        pub(super) fn atomic_write(&self, name: &OsStr, bytes: &[u8]) -> Result<()> {
            self.ensure_path_identity()?;
            let temporary = OsString::from(format!(".tmp-{}", uuid::Uuid::new_v4()));
            let mut file = self.create_file(&temporary)?;
            if let Err(error) = file.write_all(bytes).and_then(|_| file.flush()) {
                drop(file);
                let temporary =
                    c_string(&temporary, "managed temporary name contains a null byte")?;
                let _ = unsafe { unlinkat(self.file.as_raw_fd(), temporary.as_ptr(), 0) };
                return Err(error.into());
            }
            drop(file);
            self.ensure_path_identity()?;
            let temporary = c_string(&temporary, "managed temporary name contains a null byte")?;
            let name = c_string(name, "managed record name contains a null byte")?;
            let result = unsafe {
                renameat2(
                    self.file.as_raw_fd(),
                    temporary.as_ptr(),
                    self.file.as_raw_fd(),
                    name.as_ptr(),
                    RENAME_NOREPLACE,
                )
            };
            if result != 0 {
                let error = std::io::Error::last_os_error();
                let _ = unsafe { unlinkat(self.file.as_raw_fd(), temporary.as_ptr(), 0) };
                return Err(error.into());
            }
            Ok(())
        }

        pub(super) fn remove(self) -> Result<()> {
            self.remove_contents()?;
            let Some(parent) = self.parent.as_ref() else {
                anyhow::bail!("managed destination owner is unavailable");
            };
            let name = self.name.as_deref().unwrap_or_default();
            let name_c = c_string(name, "managed workspace name contains a null byte")?;
            let current = open_directory_relative(parent.as_raw_fd(), &name_c)?;
            let metadata = current.metadata()?;
            anyhow::ensure!(
                metadata.dev() == self.device && metadata.ino() == self.inode,
                "managed destination identity changed before removal"
            );
            drop(current);
            let result = unsafe { unlinkat(parent.as_raw_fd(), name_c.as_ptr(), AT_REMOVEDIR) };
            if result != 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            Ok(())
        }

        fn create_file(&self, name: &OsStr) -> Result<File> {
            self.ensure_path_identity()?;
            let name = c_string(name, "managed workspace name contains a null byte")?;
            let fd = unsafe {
                openat(
                    self.file.as_raw_fd(),
                    name.as_ptr(),
                    O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC,
                    0o600u32,
                )
            };
            if fd < 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            Ok(unsafe { File::from_raw_fd(fd) })
        }

        fn create_symlink(&self, name: &OsStr, target: &Path) -> Result<()> {
            self.ensure_path_identity()?;
            let name = c_string(name, "managed workspace name contains a null byte")?;
            let target = CString::new(target.as_os_str().as_bytes())
                .map_err(|_| anyhow::anyhow!("managed symlink target contains a null byte"))?;
            let result =
                unsafe { symlinkat(target.as_ptr(), self.file.as_raw_fd(), name.as_ptr()) };
            if result != 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            Ok(())
        }

        fn remove_contents(&self) -> Result<()> {
            for entry in fs::read_dir(proc_fd_path(self.file.as_raw_fd()))? {
                let name = entry?.file_name();
                let name_c = c_string(&name, "managed workspace name contains a null byte")?;
                let opened = open_relative_handle(self.file.as_raw_fd(), &name)?;
                let metadata = opened.metadata()?;
                if metadata.is_dir() {
                    let child = Self {
                        file: opened,
                        path: self.path.join(&name),
                        parent: Some(self.file.try_clone()?),
                        name: Some(name),
                        device: metadata.dev(),
                        inode: metadata.ino(),
                    };
                    child.remove()?;
                } else {
                    drop(opened);
                    let result = unsafe { unlinkat(self.file.as_raw_fd(), name_c.as_ptr(), 0) };
                    if result != 0 {
                        return Err(std::io::Error::last_os_error().into());
                    }
                }
            }
            Ok(())
        }
    }

    pub(super) fn copy_root<F>(
        source_root: &Path,
        destination: &ManagedDirectory,
        stats: &mut CopyStats,
        before_open: &mut F,
    ) -> Result<()>
    where
        F: FnMut(&Path),
    {
        let root = open_path_handle(source_root)?;
        let metadata = root.metadata()?;
        anyhow::ensure!(metadata.is_dir(), "external source must be a directory");
        validate_handle_containment(&root, source_root)?;
        copy_entry(
            source_root,
            &root,
            source_root,
            Path::new(""),
            destination,
            stats,
            before_open,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn copy_entry<F>(
        source_root: &Path,
        source_directory: &File,
        source_display: &Path,
        relative_directory: &Path,
        destination: &ManagedDirectory,
        stats: &mut CopyStats,
        before_open: &mut F,
    ) -> Result<()>
    where
        F: FnMut(&Path),
    {
        destination.ensure_path_identity()?;
        // Enumerate through the already-open directory, not its mutable pathname. The
        // pathname remains useful only for diagnostics and the destination-relative hook.
        for entry in fs::read_dir(proc_fd_path(source_directory.as_raw_fd()))? {
            let entry = entry?;
            let name = entry.file_name();
            if is_excluded(&name) {
                continue;
            }

            let from = source_display.join(&name);
            before_open(&from);

            let opened = open_relative_handle(source_directory.as_raw_fd(), &name)
                .with_context(|| "external workspace entry changed while it was being opened")?;
            validate_handle_containment(&opened, source_root)?;
            let metadata = opened.metadata()?;
            if metadata.is_dir() {
                stats.add_directory()?;
                let to = destination.create_child(&name)?;
                copy_entry(
                    source_root,
                    &opened,
                    &from,
                    &relative_directory.join(&name),
                    &to,
                    stats,
                    before_open,
                )?;
            } else if metadata.is_file() {
                let data = open_handle_for_read(&opened)?;
                let to = destination.create_file(&name)?;
                copy_open_file(&data, to, stats)?;
            } else if metadata.file_type().is_symlink() {
                let target = read_opened_symlink(&opened)?;
                validate_relative_symlink(
                    source_root,
                    source_display,
                    relative_directory,
                    &target,
                )?;
                stats.add_file(target.as_os_str().len() as u64)?;
                destination.create_symlink(&name, &target)?;
            } else {
                anyhow::bail!("external workspace contains an unsupported filesystem entry");
            }
        }
        Ok(())
    }

    fn open_path_handle(path: &Path) -> Result<File> {
        let path = CString::new(path.as_os_str().as_bytes())
            .map_err(|_| anyhow::anyhow!("external source path contains a null byte"))?;
        open_handle(AT_FDCWD, &path)
    }

    fn open_relative_handle(parent: RawFd, name: &OsStr) -> Result<File> {
        let name = c_string(name, "external workspace name contains a null byte")?;
        open_handle(parent, &name)
    }

    fn open_directory_relative(parent: RawFd, name: &CString) -> Result<File> {
        let fd = unsafe {
            openat(
                parent,
                name.as_ptr(),
                O_PATH | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(unsafe { File::from_raw_fd(fd) })
    }

    fn c_string(name: &OsStr, message: &'static str) -> Result<CString> {
        CString::new(name.as_bytes()).map_err(|_| anyhow::anyhow!(message))
    }

    fn open_handle(parent: RawFd, name: &CString) -> Result<File> {
        // O_PATH inspects the directory entry without activating devices or blocking on FIFOs.
        // O_NOFOLLOW pins the entry itself, so a concurrent symlink swap cannot redirect reads.
        let fd = unsafe { openat(parent, name.as_ptr(), O_PATH | O_NOFOLLOW | O_CLOEXEC) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(unsafe { File::from_raw_fd(fd) })
    }

    fn open_handle_for_read(handle: &File) -> Result<File> {
        let metadata = handle.metadata()?;
        let path = proc_fd_path(handle.as_raw_fd());
        let data = OpenOptions::new()
            .read(true)
            .custom_flags(O_CLOEXEC)
            .open(path)?;
        let data_metadata = data.metadata()?;
        anyhow::ensure!(
            metadata.dev() == data_metadata.dev() && metadata.ino() == data_metadata.ino(),
            "external workspace entry identity changed while it was being opened"
        );
        Ok(data)
    }

    fn validate_handle_containment(handle: &File, source_root: &Path) -> Result<()> {
        let actual = fs::canonicalize(proc_fd_path(handle.as_raw_fd()))
            .map_err(|_| anyhow::anyhow!("external workspace entry has no stable path"))?;
        anyhow::ensure!(
            actual.starts_with(source_root),
            "external workspace entry escapes its root"
        );
        Ok(())
    }

    fn read_opened_symlink(handle: &File) -> Result<PathBuf> {
        let empty = b"\0";
        let mut capacity = 256usize;
        loop {
            let mut buffer = vec![0u8; capacity];
            let length = unsafe {
                readlinkat(
                    handle.as_raw_fd(),
                    empty.as_ptr().cast(),
                    buffer.as_mut_ptr().cast(),
                    buffer.len(),
                )
            };
            if length < 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            let length = length as usize;
            if length < buffer.len() {
                buffer.truncate(length);
                return Ok(PathBuf::from(OsString::from_vec(buffer)));
            }
            capacity = capacity.saturating_mul(2);
        }
    }

    fn proc_fd_path(fd: RawFd) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/{fd}"))
    }
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
mod platform {
    use std::ffi::{OsStr, OsString};
    use std::fs::{self, File};
    use std::io::Write;
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::MetadataExt;
    use std::path::{Path, PathBuf};

    use anyhow::Result;

    use super::{copy_open_file, is_excluded, validate_relative_symlink, CopyStats};

    pub(super) struct ManagedDirectory {
        file: File,
        path: PathBuf,
        parent: Option<File>,
        name: Option<OsString>,
        device: u64,
        inode: u64,
    }

    impl ManagedDirectory {
        pub(super) fn open(path: &Path) -> Result<Self> {
            let metadata = fs::symlink_metadata(path)?;
            anyhow::ensure!(
                metadata.is_dir() && !metadata.file_type().is_symlink(),
                "managed destination must be a real directory"
            );
            let expected = fs::canonicalize(path)?;
            let file = File::open(path)?;
            let metadata = file.metadata()?;
            let directory = Self {
                file,
                path: expected,
                parent: None,
                name: None,
                device: metadata.dev(),
                inode: metadata.ino(),
            };
            directory.ensure_path_identity()?;
            Ok(directory)
        }

        pub(super) fn create(path: &Path) -> Result<Self> {
            let parent = path
                .parent()
                .ok_or_else(|| anyhow::anyhow!("managed destination has no parent"))?;
            let name = path
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("managed destination has no name"))?;
            super::validate_child_name(name)?;
            Self::open(parent)?.create_child(name)
        }

        pub(super) fn create_child(&self, name: &OsStr) -> Result<Self> {
            self.ensure_path_identity()?;
            let child_path = fd_directory_path(self.file.as_raw_fd()).join(name);
            fs::create_dir(&child_path)?;
            let file = File::open(&child_path)?;
            let metadata = file.metadata()?;
            let child = Self {
                file,
                path: self.path.join(name),
                parent: Some(self.file.try_clone()?),
                name: Some(name.to_os_string()),
                device: metadata.dev(),
                inode: metadata.ino(),
            };
            child.ensure_path_identity()?;
            Ok(child)
        }

        pub(super) fn path(&self) -> &Path {
            &self.path
        }

        pub(super) fn stable_access_path(&self) -> Result<PathBuf> {
            self.ensure_path_identity()?;
            Ok(fd_directory_path(self.file.as_raw_fd()))
        }

        pub(super) fn ensure_path_identity(&self) -> Result<()> {
            let current = fs::canonicalize(&self.path)
                .map_err(|_| anyhow::anyhow!("managed destination path changed"))?;
            let handle_path = fs::canonicalize(fd_directory_path(self.file.as_raw_fd()))?;
            let metadata = fs::symlink_metadata(&self.path)?;
            anyhow::ensure!(
                current == handle_path
                    && !metadata.file_type().is_symlink()
                    && metadata.dev() == self.device
                    && metadata.ino() == self.inode,
                "managed destination identity changed"
            );
            Ok(())
        }

        pub(super) fn promote_child(&self, child: &Self, name: &OsStr) -> Result<()> {
            self.ensure_path_identity()?;
            child.ensure_path_identity()?;
            let parent = child
                .parent
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("managed staging directory has no owner"))?;
            let metadata = parent.metadata()?;
            anyhow::ensure!(
                metadata.dev() == self.device && metadata.ino() == self.inode,
                "managed staging directory has a different owner"
            );
            let root = fd_directory_path(self.file.as_raw_fd());
            let destination = root.join(name);
            anyhow::ensure!(
                fs::symlink_metadata(&destination)
                    .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound),
                "managed workspace source destination already exists"
            );
            fs::rename(
                root.join(child.name.as_deref().unwrap_or_default()),
                destination,
            )?;
            Ok(())
        }

        pub(super) fn atomic_write(&self, name: &OsStr, bytes: &[u8]) -> Result<()> {
            self.ensure_path_identity()?;
            let root = fd_directory_path(self.file.as_raw_fd());
            let temporary = root.join(format!(".tmp-{}", uuid::Uuid::new_v4()));
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            if let Err(error) = file.write_all(bytes).and_then(|_| file.flush()) {
                drop(file);
                let _ = fs::remove_file(&temporary);
                return Err(error.into());
            }
            drop(file);
            self.ensure_path_identity()?;
            let destination = root.join(name);
            if let Err(error) = fs::rename(&temporary, &destination) {
                let _ = fs::remove_file(&temporary);
                return Err(error.into());
            }
            Ok(())
        }

        pub(super) fn remove(self) -> Result<()> {
            let root = fd_directory_path(self.file.as_raw_fd());
            for entry in fs::read_dir(&root)? {
                let path = entry?.path();
                let metadata = fs::symlink_metadata(&path)?;
                if metadata.is_dir() && !metadata.file_type().is_symlink() {
                    fs::remove_dir_all(path)?;
                } else {
                    fs::remove_file(path)?;
                }
            }
            let parent = self
                .parent
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("managed destination owner is unavailable"))?;
            let name = self.name.as_deref().unwrap_or_default();
            let path = fd_directory_path(parent.as_raw_fd()).join(name);
            let current = File::open(&path)?;
            let metadata = current.metadata()?;
            anyhow::ensure!(
                metadata.dev() == self.device && metadata.ino() == self.inode,
                "managed destination identity changed before removal"
            );
            drop(current);
            drop(self.file);
            fs::remove_dir(path)?;
            Ok(())
        }

        fn create_file(&self, name: &OsStr) -> Result<File> {
            self.ensure_path_identity()?;
            Ok(fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(fd_directory_path(self.file.as_raw_fd()).join(name))?)
        }

        fn create_symlink(&self, name: &OsStr, target: &Path) -> Result<()> {
            self.ensure_path_identity()?;
            std::os::unix::fs::symlink(
                target,
                fd_directory_path(self.file.as_raw_fd()).join(name),
            )?;
            Ok(())
        }
    }

    pub(super) fn copy_root<F>(
        source_root: &Path,
        destination: &ManagedDirectory,
        stats: &mut CopyStats,
        before_open: &mut F,
    ) -> Result<()>
    where
        F: FnMut(&Path),
    {
        let root = open_validated(source_root, source_root)?;
        anyhow::ensure!(
            root.metadata()?.is_dir(),
            "external source must be a directory"
        );
        copy_entry(
            source_root,
            &root,
            source_root,
            Path::new(""),
            destination,
            stats,
            before_open,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn copy_entry<F>(
        source_root: &Path,
        source_directory: &File,
        source_display: &Path,
        relative_directory: &Path,
        destination: &ManagedDirectory,
        stats: &mut CopyStats,
        before_open: &mut F,
    ) -> Result<()>
    where
        F: FnMut(&Path),
    {
        destination.ensure_path_identity()?;
        // `/dev/fd` is the handle-backed directory view on the supported non-Linux Unix
        // targets. If it is unavailable, fail closed instead of enumerating a swapped path.
        for entry in fs::read_dir(fd_directory_path(source_directory.as_raw_fd()))? {
            let entry = entry?;
            let name = entry.file_name();
            if is_excluded(&name) {
                continue;
            }
            let from = source_display.join(&name);
            before_open(&from);
            let metadata = fs::symlink_metadata(&from)?;
            if metadata.file_type().is_symlink() {
                let target = fs::read_link(&from)?;
                validate_relative_symlink(
                    source_root,
                    source_display,
                    relative_directory,
                    &target,
                )?;
                stats.add_file(target.as_os_str().len() as u64)?;
                destination.create_symlink(&name, &target)?;
                continue;
            }

            let opened = open_validated(&from, source_root)?;
            let metadata = opened.metadata()?;
            if metadata.is_dir() {
                stats.add_directory()?;
                let to = destination.create_child(&name)?;
                copy_entry(
                    source_root,
                    &opened,
                    &from,
                    &relative_directory.join(&name),
                    &to,
                    stats,
                    before_open,
                )?;
            } else if metadata.is_file() {
                let to = destination.create_file(&name)?;
                copy_open_file(&opened, to, stats)?;
            } else {
                anyhow::bail!("external workspace contains an unsupported filesystem entry");
            }
        }
        Ok(())
    }

    fn open_validated(path: &Path, source_root: &Path) -> Result<File> {
        // Non-Linux Unix targets validate the inode reached by open against the canonical,
        // contained path before any bytes are read. A swap either changes the inode or path.
        let opened = File::open(path)?;
        let opened_metadata = opened.metadata()?;
        let canonical = fs::canonicalize(path)?;
        anyhow::ensure!(
            canonical.starts_with(source_root),
            "external workspace entry escapes its root"
        );
        let path_metadata = fs::metadata(canonical)?;
        anyhow::ensure!(
            opened_metadata.dev() == path_metadata.dev()
                && opened_metadata.ino() == path_metadata.ino(),
            "external workspace entry identity changed while it was being opened"
        );
        Ok(opened)
    }

    fn fd_directory_path(fd: i32) -> PathBuf {
        PathBuf::from(format!("/dev/fd/{fd}"))
    }
}

#[cfg(windows)]
mod platform {
    use std::ffi::{c_void, OsStr, OsString};
    use std::fs::{self, File, OpenOptions};
    use std::io::Write;
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::{AsRawHandle, FromRawHandle};
    use std::os::windows::prelude::{OsStrExt, OsStringExt};
    use std::path::{Path, PathBuf};

    use anyhow::Result;

    use super::{copy_open_file, is_excluded, validate_relative_symlink, CopyStats};

    const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
    const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    const FILE_SHARE_READ: u32 = 0x1;
    const FILE_SHARE_WRITE: u32 = 0x2;
    const FILE_SHARE_DELETE: u32 = 0x4;
    const DELETE_ACCESS: u32 = 0x0001_0000;
    const SYNCHRONIZE: u32 = 0x0010_0000;
    const GENERIC_READ: u32 = 0x8000_0000;
    const GENERIC_WRITE: u32 = 0x4000_0000;
    const FILE_LIST_DIRECTORY: u32 = 0x0001;
    const FILE_ADD_FILE: u32 = 0x0002;
    const FILE_ADD_SUBDIRECTORY: u32 = 0x0004;
    const FILE_TRAVERSE: u32 = 0x0020;
    const FILE_READ_ATTRIBUTES: u32 = 0x0080;
    const FILE_WRITE_ATTRIBUTES: u32 = 0x0100;
    const FILE_OPEN: u32 = 1;
    const FILE_CREATE: u32 = 2;
    const FILE_DIRECTORY_FILE: u32 = 0x0000_0001;
    const FILE_NON_DIRECTORY_FILE: u32 = 0x0000_0040;
    const FILE_SYNCHRONOUS_IO_NONALERT: u32 = 0x0000_0020;
    const FILE_OPEN_FOR_BACKUP_INTENT: u32 = 0x0000_4000;
    const FILE_OPEN_REPARSE_POINT_OPTION: u32 = 0x0020_0000;
    const OBJ_CASE_INSENSITIVE: u32 = 0x40;
    const NT_FILE_RENAME_INFORMATION_CLASS: u32 = 10;
    const NT_FILE_DISPOSITION_INFORMATION_CLASS: u32 = 13;
    const FILE_ATTRIBUTE_TAG_INFO_CLASS: i32 = 9;
    const FILE_ID_BOTH_DIRECTORY_INFO_CLASS: i32 = 10;
    const FILE_ID_BOTH_DIRECTORY_RESTART_INFO_CLASS: i32 = 11;
    const IO_REPARSE_TAG_SYMLINK: u32 = 0xA000_000C;
    const IO_REPARSE_TAG_NAME_SURROGATE: u32 = 0x2000_0000;
    const ERROR_NO_MORE_FILES: i32 = 18;

    #[repr(C)]
    #[derive(Default)]
    struct FileTime {
        low_date_time: u32,
        high_date_time: u32,
    }

    #[repr(C)]
    #[derive(Default)]
    struct ByHandleFileInformation {
        file_attributes: u32,
        creation_time: FileTime,
        last_access_time: FileTime,
        last_write_time: FileTime,
        volume_serial_number: u32,
        file_size_high: u32,
        file_size_low: u32,
        number_of_links: u32,
        file_index_high: u32,
        file_index_low: u32,
    }

    #[repr(C)]
    #[derive(Default)]
    struct FileAttributeTagInformation {
        file_attributes: u32,
        reparse_tag: u32,
    }

    #[repr(C)]
    struct FileIdBothDirectoryInformation {
        next_entry_offset: u32,
        file_index: u32,
        creation_time: i64,
        last_access_time: i64,
        last_write_time: i64,
        change_time: i64,
        end_of_file: i64,
        allocation_size: i64,
        file_attributes: u32,
        file_name_length: u32,
        ea_size: u32,
        short_name_length: u8,
        short_name: [u16; 12],
        file_id: i64,
        file_name: [u16; 1],
    }

    #[repr(C)]
    struct UnicodeString {
        length: u16,
        maximum_length: u16,
        buffer: *mut u16,
    }

    #[repr(C)]
    struct ObjectAttributes {
        length: u32,
        root_directory: *mut c_void,
        object_name: *mut UnicodeString,
        attributes: u32,
        security_descriptor: *mut c_void,
        security_quality_of_service: *mut c_void,
    }

    #[repr(C)]
    struct IoStatusBlock {
        status_or_pointer: *mut c_void,
        information: usize,
    }

    #[repr(C)]
    struct FileRenameInformation {
        replace_if_exists: u8,
        root_directory: *mut c_void,
        file_name_length: u32,
        file_name: [u16; 1],
    }

    #[repr(C)]
    struct FileDispositionInformation {
        delete_file: u8,
    }

    unsafe extern "system" {
        fn GetFileInformationByHandle(
            file: *mut c_void,
            information: *mut ByHandleFileInformation,
        ) -> i32;
        fn GetFileInformationByHandleEx(
            file: *mut c_void,
            information_class: i32,
            information: *mut c_void,
            information_size: u32,
        ) -> i32;
        fn GetFinalPathNameByHandleW(
            file: *mut c_void,
            path: *mut u16,
            path_length: u32,
            flags: u32,
        ) -> u32;
    }

    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn RtlNtStatusToDosError(status: u32) -> u32;
        fn NtCreateFile(
            file_handle: *mut *mut c_void,
            desired_access: u32,
            object_attributes: *mut ObjectAttributes,
            io_status_block: *mut IoStatusBlock,
            allocation_size: *mut i64,
            file_attributes: u32,
            share_access: u32,
            create_disposition: u32,
            create_options: u32,
            ea_buffer: *mut c_void,
            ea_length: u32,
        ) -> i32;
        fn NtSetInformationFile(
            file_handle: *mut c_void,
            io_status_block: *mut IoStatusBlock,
            file_information: *mut c_void,
            length: u32,
            file_information_class: u32,
        ) -> i32;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct FileIdentity {
        volume: u32,
        index: u64,
    }

    struct OpenedObject {
        file: File,
        final_path: PathBuf,
        identity: FileIdentity,
        attributes: u32,
        reparse_tag: u32,
    }

    pub(super) struct ManagedDirectory {
        file: File,
        path: PathBuf,
        parent: Option<File>,
        identity: FileIdentity,
    }

    impl ManagedDirectory {
        pub(super) fn open(path: &Path) -> Result<Self> {
            let metadata = fs::symlink_metadata(path)?;
            anyhow::ensure!(
                metadata.is_dir() && !metadata.file_type().is_symlink(),
                "managed destination must be a real directory"
            );
            let file = open_destination_path(path)?;
            let tag = file_attribute_tag(&file)?;
            anyhow::ensure!(
                tag.file_attributes & FILE_ATTRIBUTE_DIRECTORY != 0
                    && tag.file_attributes & FILE_ATTRIBUTE_REPARSE_POINT == 0,
                "managed destination must be a real directory"
            );
            let identity = file_identity(&file)?;
            let final_path = final_path(&file)?;
            let directory = Self {
                file,
                path: final_path,
                parent: None,
                identity,
            };
            directory.ensure_path_identity()?;
            Ok(directory)
        }

        pub(super) fn create(path: &Path) -> Result<Self> {
            let parent = path
                .parent()
                .ok_or_else(|| anyhow::anyhow!("managed destination has no parent"))?;
            let name = path
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("managed destination has no name"))?;
            super::validate_child_name(name)?;
            Self::open(parent)?.create_child(name)
        }

        pub(super) fn create_child(&self, name: &OsStr) -> Result<Self> {
            self.ensure_path_identity()?;
            let file = nt_create_relative(&self.file, name, FILE_CREATE, Some(true))?;
            let tag = file_attribute_tag(&file)?;
            anyhow::ensure!(
                tag.file_attributes & FILE_ATTRIBUTE_DIRECTORY != 0
                    && tag.file_attributes & FILE_ATTRIBUTE_REPARSE_POINT == 0,
                "managed destination child must be a real directory"
            );
            let child = Self {
                identity: file_identity(&file)?,
                file,
                path: self.path.join(name),
                parent: Some(self.file.try_clone()?),
            };
            child.ensure_path_identity()?;
            Ok(child)
        }

        pub(super) fn path(&self) -> &Path {
            &self.path
        }

        pub(super) fn stable_access_path(&self) -> Result<PathBuf> {
            self.ensure_path_identity()?;
            Ok(final_path(&self.file)?)
        }

        pub(super) fn ensure_path_identity(&self) -> Result<()> {
            let opened = open_object(&self.path, false)
                .map_err(|_| anyhow::anyhow!("managed destination path changed"))?;
            let handle_path = final_path(&self.file)?;
            anyhow::ensure!(
                opened.identity == self.identity
                    && opened.final_path == handle_path
                    && opened.attributes & FILE_ATTRIBUTE_DIRECTORY != 0
                    && opened.attributes & FILE_ATTRIBUTE_REPARSE_POINT == 0,
                "managed destination identity changed"
            );
            Ok(())
        }

        pub(super) fn promote_child(&self, child: &Self, name: &OsStr) -> Result<()> {
            self.ensure_path_identity()?;
            child.ensure_path_identity()?;
            let parent = child
                .parent
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("managed staging directory has no owner"))?;
            anyhow::ensure!(
                file_identity(parent)? == self.identity,
                "managed staging directory has a different owner"
            );
            rename_handle_relative(&child.file, &self.file, name)
        }

        pub(super) fn atomic_write(&self, name: &OsStr, bytes: &[u8]) -> Result<()> {
            self.ensure_path_identity()?;
            let temporary = OsString::from(format!(".tmp-{}", uuid::Uuid::new_v4()));
            let mut file = self.create_file(&temporary)?;
            if let Err(error) = file.write_all(bytes).and_then(|_| file.flush()) {
                let _ = mark_delete(&file);
                return Err(error.into());
            }
            if let Err(error) = rename_handle_relative(&file, &self.file, name) {
                let _ = mark_delete(&file);
                return Err(error);
            }
            Ok(())
        }

        pub(super) fn remove(self) -> Result<()> {
            visit_directory_names(&self.file, &mut |name| {
                let file = nt_create_relative(&self.file, &name, FILE_OPEN, None)?;
                let tag = file_attribute_tag(&file)?;
                if tag.file_attributes & FILE_ATTRIBUTE_DIRECTORY != 0
                    && tag.file_attributes & FILE_ATTRIBUTE_REPARSE_POINT == 0
                {
                    let child = Self {
                        identity: file_identity(&file)?,
                        path: final_path(&file)?,
                        file,
                        parent: Some(self.file.try_clone()?),
                    };
                    child.remove()?;
                } else {
                    mark_delete(&file)?;
                }
                Ok(())
            })?;
            mark_delete(&self.file)
        }

        fn create_file(&self, name: &OsStr) -> Result<File> {
            self.ensure_path_identity()?;
            nt_create_relative(&self.file, name, FILE_CREATE, Some(false))
        }

        fn create_symlink(&self, name: &OsStr, target: &Path, directory: bool) -> Result<()> {
            self.ensure_path_identity()?;
            let destination = final_path(&self.file)?.join(name);
            if directory {
                std::os::windows::fs::symlink_dir(target, destination)?;
            } else {
                std::os::windows::fs::symlink_file(target, destination)?;
            }
            Ok(())
        }
    }

    fn open_destination_path(path: &Path) -> Result<File> {
        let mut options = OpenOptions::new();
        options
            .access_mode(
                FILE_LIST_DIRECTORY
                    | FILE_ADD_FILE
                    | FILE_ADD_SUBDIRECTORY
                    | FILE_TRAVERSE
                    | FILE_READ_ATTRIBUTES
                    | FILE_WRITE_ATTRIBUTES
                    | DELETE_ACCESS
                    | SYNCHRONIZE,
            )
            // Delete sharing stays closed so the pinned directory cannot be renamed or
            // replaced while normal fetch/hash code remains able to open it for I/O.
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT);
        Ok(options.open(path)?)
    }

    fn nt_create_relative(
        parent: &File,
        name: &OsStr,
        disposition: u32,
        directory: Option<bool>,
    ) -> Result<File> {
        super::validate_child_name(name)?;
        let mut wide = name.encode_wide().collect::<Vec<_>>();
        anyhow::ensure!(
            !wide.contains(&0) && wide.len().saturating_mul(2) <= u16::MAX as usize,
            "managed workspace child name is invalid"
        );
        let byte_length = (wide.len() * 2) as u16;
        let mut unicode = UnicodeString {
            length: byte_length,
            maximum_length: byte_length,
            buffer: wide.as_mut_ptr(),
        };
        let mut attributes = ObjectAttributes {
            length: std::mem::size_of::<ObjectAttributes>() as u32,
            root_directory: parent.as_raw_handle(),
            object_name: &mut unicode,
            attributes: OBJ_CASE_INSENSITIVE,
            security_descriptor: std::ptr::null_mut(),
            security_quality_of_service: std::ptr::null_mut(),
        };
        let mut status_block = IoStatusBlock {
            status_or_pointer: std::ptr::null_mut(),
            information: 0,
        };
        let mut handle = std::ptr::null_mut();
        let desired_access = if directory == Some(false) {
            GENERIC_READ | GENERIC_WRITE | DELETE_ACCESS | SYNCHRONIZE
        } else {
            FILE_LIST_DIRECTORY
                | FILE_ADD_FILE
                | FILE_ADD_SUBDIRECTORY
                | FILE_TRAVERSE
                | FILE_READ_ATTRIBUTES
                | FILE_WRITE_ATTRIBUTES
                | DELETE_ACCESS
                | SYNCHRONIZE
        };
        let type_option = match directory {
            Some(true) => FILE_DIRECTORY_FILE,
            Some(false) => FILE_NON_DIRECTORY_FILE,
            None => 0,
        };
        let status = unsafe {
            NtCreateFile(
                &mut handle,
                desired_access,
                &mut attributes,
                &mut status_block,
                std::ptr::null_mut(),
                FILE_ATTRIBUTE_NORMAL,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                disposition,
                type_option
                    | FILE_SYNCHRONOUS_IO_NONALERT
                    | FILE_OPEN_REPARSE_POINT_OPTION
                    | FILE_OPEN_FOR_BACKUP_INTENT,
                std::ptr::null_mut(),
                0,
            )
        };
        if status < 0 {
            return Err(nt_error(status).into());
        }
        Ok(unsafe { File::from_raw_handle(handle) })
    }

    fn rename_handle_relative(file: &File, parent: &File, name: &OsStr) -> Result<()> {
        super::validate_child_name(name)?;
        let wide = name.encode_wide().collect::<Vec<_>>();
        anyhow::ensure!(
            !wide.contains(&0) && wide.len().saturating_mul(2) <= u32::MAX as usize,
            "managed workspace child name is invalid"
        );
        let header = std::mem::offset_of!(FileRenameInformation, file_name);
        let size = header + wide.len() * std::mem::size_of::<u16>();
        let words = size.div_ceil(std::mem::size_of::<usize>());
        let mut storage = vec![0usize; words];
        let information = storage.as_mut_ptr().cast::<FileRenameInformation>();
        unsafe {
            std::ptr::addr_of_mut!((*information).replace_if_exists).write(0);
            std::ptr::addr_of_mut!((*information).root_directory).write(parent.as_raw_handle());
            std::ptr::addr_of_mut!((*information).file_name_length).write((wide.len() * 2) as u32);
            std::ptr::copy_nonoverlapping(
                wide.as_ptr(),
                std::ptr::addr_of_mut!((*information).file_name).cast::<u16>(),
                wide.len(),
            );
        }
        let mut status_block = IoStatusBlock {
            status_or_pointer: std::ptr::null_mut(),
            information: 0,
        };
        let status = unsafe {
            NtSetInformationFile(
                file.as_raw_handle(),
                &mut status_block,
                information.cast(),
                size as u32,
                NT_FILE_RENAME_INFORMATION_CLASS,
            )
        };
        if status < 0 {
            return Err(nt_error(status).into());
        }
        Ok(())
    }

    fn mark_delete(file: &File) -> Result<()> {
        let mut information = FileDispositionInformation { delete_file: 1 };
        let mut status_block = IoStatusBlock {
            status_or_pointer: std::ptr::null_mut(),
            information: 0,
        };
        let status = unsafe {
            NtSetInformationFile(
                file.as_raw_handle(),
                &mut status_block,
                (&mut information as *mut FileDispositionInformation).cast(),
                std::mem::size_of::<FileDispositionInformation>() as u32,
                NT_FILE_DISPOSITION_INFORMATION_CLASS,
            )
        };
        if status < 0 {
            return Err(nt_error(status).into());
        }
        Ok(())
    }

    fn nt_error(status: i32) -> std::io::Error {
        let code = unsafe { RtlNtStatusToDosError(status as u32) };
        std::io::Error::from_raw_os_error(code as i32)
    }

    impl OpenedObject {
        fn is_directory(&self) -> bool {
            self.attributes & FILE_ATTRIBUTE_DIRECTORY != 0
        }

        fn is_reparse_point(&self) -> bool {
            self.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        }
    }

    pub(super) fn copy_root<F>(
        source_root: &Path,
        destination: &ManagedDirectory,
        stats: &mut CopyStats,
        before_open: &mut F,
    ) -> Result<()>
    where
        F: FnMut(&Path),
    {
        let inspected_root = open_verified(source_root, None)?;
        anyhow::ensure!(
            inspected_root.is_directory(),
            "external source must be a directory"
        );
        let root = if inspected_root.is_reparse_point() {
            anyhow::ensure!(
                inspected_root.reparse_tag & IO_REPARSE_TAG_NAME_SURROGATE == 0,
                "external source root must not be a redirecting reparse point"
            );
            open_verified_with(source_root, None, true)?
        } else {
            inspected_root
        };
        anyhow::ensure!(root.is_directory(), "external source must be a directory");
        copy_entry(
            &root.final_path,
            &root,
            Path::new(""),
            destination,
            stats,
            before_open,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn copy_entry<F>(
        root_final_path: &Path,
        source_directory: &OpenedObject,
        relative_directory: &Path,
        destination: &ManagedDirectory,
        stats: &mut CopyStats,
        before_open: &mut F,
    ) -> Result<()>
    where
        F: FnMut(&Path),
    {
        destination.ensure_path_identity()?;
        visit_directory_names(&source_directory.file, &mut |name| {
            if is_excluded(&name) {
                return Ok(());
            }
            let from = source_directory.final_path.join(&name);
            before_open(&from);

            let opened = open_verified(&from, Some(root_final_path))?;
            confirm_path_identity(source_directory)?;
            anyhow::ensure!(
                opened.final_path.parent() == Some(source_directory.final_path.as_path()),
                "external workspace entry changed parents while it was being opened"
            );
            if opened.is_reparse_point() {
                if opened.reparse_tag == IO_REPARSE_TAG_SYMLINK {
                    let target = fs::read_link(&from)?;
                    let reopened = open_verified(&from, Some(root_final_path))?;
                    anyhow::ensure!(
                        reopened.identity == opened.identity
                            && reopened.reparse_tag == opened.reparse_tag,
                        "external workspace symlink changed while it was being read"
                    );
                    let resolved = validate_relative_symlink(
                        root_final_path,
                        &source_directory.final_path,
                        relative_directory,
                        &target,
                    )?;
                    stats.add_file(target.as_os_str().len() as u64)?;
                    destination.create_symlink(&name, &target, resolved.is_dir())?;
                } else {
                    // Name-surrogate tags (junctions and other redirecting reparse points)
                    // can change path ancestry and are rejected. Non-redirecting tags such
                    // as cloud placeholders remain usable through a followed, revalidated
                    // handle instead of being blanket-disabled.
                    anyhow::ensure!(
                        opened.reparse_tag & IO_REPARSE_TAG_NAME_SURROGATE == 0,
                        "external workspace contains a redirecting reparse point"
                    );
                    let followed = open_verified_following(&from, root_final_path)?;
                    confirm_path_identity(source_directory)?;
                    anyhow::ensure!(
                        followed.final_path.parent() == Some(source_directory.final_path.as_path()),
                        "external workspace entry changed parents while it was being opened"
                    );
                    if followed.is_directory() {
                        stats.add_directory()?;
                        let to = destination.create_child(&name)?;
                        copy_entry(
                            root_final_path,
                            &followed,
                            &relative_directory.join(&name),
                            &to,
                            stats,
                            before_open,
                        )?;
                    } else if followed.file.metadata()?.is_file() {
                        let to = destination.create_file(&name)?;
                        copy_open_file(&followed.file, to, stats)?;
                    } else {
                        anyhow::bail!(
                            "external workspace contains an unsupported filesystem entry"
                        );
                    }
                }
            } else if opened.is_directory() {
                stats.add_directory()?;
                let to = destination.create_child(&name)?;
                copy_entry(
                    root_final_path,
                    &opened,
                    &relative_directory.join(&name),
                    &to,
                    stats,
                    before_open,
                )?;
            } else if opened.file.metadata()?.is_file() {
                let to = destination.create_file(&name)?;
                copy_open_file(&opened.file, to, stats)?;
            } else {
                anyhow::bail!("external workspace contains an unsupported filesystem entry");
            }
            Ok(())
        })?;
        Ok(())
    }

    fn open_verified(path: &Path, root_final_path: Option<&Path>) -> Result<OpenedObject> {
        open_verified_with(path, root_final_path, false)
    }

    fn open_verified_following(path: &Path, root_final_path: &Path) -> Result<OpenedObject> {
        open_verified_with(path, Some(root_final_path), true)
    }

    fn open_verified_with(
        path: &Path,
        root_final_path: Option<&Path>,
        follow_reparse: bool,
    ) -> Result<OpenedObject> {
        let opened = open_object(path, follow_reparse)?;
        if let Some(root) = root_final_path {
            anyhow::ensure!(
                opened.final_path.starts_with(root),
                "external workspace entry escapes its root"
            );
        }

        // Reopen the path reported by the handle and compare stable volume/file IDs. This
        // catches a rename or reparse swap between CreateFile and final-path validation.
        let confirmed = open_object(&opened.final_path, follow_reparse)?;
        anyhow::ensure!(
            confirmed.identity == opened.identity,
            "external workspace entry identity changed while it was being opened"
        );
        Ok(opened)
    }

    fn confirm_path_identity(opened: &OpenedObject) -> Result<()> {
        let confirmed = open_object(&opened.final_path, false)?;
        anyhow::ensure!(
            confirmed.identity == opened.identity,
            "external workspace directory identity changed during enumeration"
        );
        Ok(())
    }

    fn visit_directory_names<F>(directory: &File, visit: &mut F) -> Result<()>
    where
        F: FnMut(OsString) -> Result<()>,
    {
        // This is a reusable API buffer, not a source-tree limit. Windows filenames are
        // bounded independently, while repeated calls enumerate directories of any size.
        const BUFFER_BYTES: usize = 64 * 1024;
        let mut storage = vec![0u64; BUFFER_BYTES / std::mem::size_of::<u64>()];
        let buffer = unsafe {
            std::slice::from_raw_parts_mut(storage.as_mut_ptr().cast::<u8>(), BUFFER_BYTES)
        };
        let mut restart = true;
        loop {
            let information_class = if restart {
                FILE_ID_BOTH_DIRECTORY_RESTART_INFO_CLASS
            } else {
                FILE_ID_BOTH_DIRECTORY_INFO_CLASS
            };
            let result = unsafe {
                GetFileInformationByHandleEx(
                    directory.as_raw_handle(),
                    information_class,
                    buffer.as_mut_ptr().cast(),
                    buffer.len() as u32,
                )
            };
            if result == 0 {
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() == Some(ERROR_NO_MORE_FILES) {
                    break;
                }
                return Err(error.into());
            }
            restart = false;

            let mut offset = 0usize;
            loop {
                let file_name_offset =
                    std::mem::offset_of!(FileIdBothDirectoryInformation, file_name);
                anyhow::ensure!(
                    offset.saturating_add(file_name_offset) <= buffer.len(),
                    "Windows directory enumeration returned an invalid record"
                );
                let record = unsafe {
                    buffer
                        .as_ptr()
                        .add(offset)
                        .cast::<FileIdBothDirectoryInformation>()
                };
                let next_entry_offset = unsafe {
                    std::ptr::read_unaligned(std::ptr::addr_of!((*record).next_entry_offset))
                } as usize;
                let file_name_length = unsafe {
                    std::ptr::read_unaligned(std::ptr::addr_of!((*record).file_name_length))
                } as usize;
                anyhow::ensure!(
                    file_name_length % std::mem::size_of::<u16>() == 0
                        && offset
                            .saturating_add(file_name_offset)
                            .saturating_add(file_name_length)
                            <= buffer.len(),
                    "Windows directory enumeration returned an invalid filename"
                );
                let file_name = unsafe {
                    std::slice::from_raw_parts(
                        buffer.as_ptr().add(offset + file_name_offset).cast::<u16>(),
                        file_name_length / std::mem::size_of::<u16>(),
                    )
                };
                let name = OsString::from_wide(file_name);
                if name != "." && name != ".." {
                    visit(name)?;
                }

                if next_entry_offset == 0 {
                    break;
                }
                anyhow::ensure!(
                    next_entry_offset >= file_name_offset
                        && offset.saturating_add(next_entry_offset) < buffer.len(),
                    "Windows directory enumeration returned an invalid next offset"
                );
                offset += next_entry_offset;
            }
        }
        Ok(())
    }

    fn open_object(path: &Path, follow_reparse: bool) -> Result<OpenedObject> {
        let mut options = OpenOptions::new();
        options
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE);
        let flags = if follow_reparse {
            FILE_FLAG_BACKUP_SEMANTICS
        } else {
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT
        };
        let file = options.custom_flags(flags).open(path)?;
        let identity = file_identity(&file)?;
        let tag = file_attribute_tag(&file)?;
        let final_path = final_path(&file)?;
        Ok(OpenedObject {
            file,
            final_path,
            identity,
            attributes: tag.file_attributes,
            reparse_tag: tag.reparse_tag,
        })
    }

    fn file_identity(file: &File) -> Result<FileIdentity> {
        let mut information = ByHandleFileInformation::default();
        let result = unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut information) };
        if result == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(FileIdentity {
            volume: information.volume_serial_number,
            index: u64::from(information.file_index_high) << 32
                | u64::from(information.file_index_low),
        })
    }

    fn file_attribute_tag(file: &File) -> Result<FileAttributeTagInformation> {
        let mut information = FileAttributeTagInformation::default();
        let result = unsafe {
            GetFileInformationByHandleEx(
                file.as_raw_handle(),
                FILE_ATTRIBUTE_TAG_INFO_CLASS,
                (&mut information as *mut FileAttributeTagInformation).cast(),
                std::mem::size_of::<FileAttributeTagInformation>() as u32,
            )
        };
        if result == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(information)
    }

    fn final_path(file: &File) -> Result<PathBuf> {
        let needed =
            unsafe { GetFinalPathNameByHandleW(file.as_raw_handle(), std::ptr::null_mut(), 0, 0) };
        if needed == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let mut buffer = vec![0u16; needed as usize];
        let written = unsafe {
            GetFinalPathNameByHandleW(
                file.as_raw_handle(),
                buffer.as_mut_ptr(),
                buffer.len() as u32,
                0,
            )
        };
        if written == 0 || written as usize >= buffer.len() {
            return Err(std::io::Error::last_os_error().into());
        }
        buffer.truncate(written as usize);
        Ok(PathBuf::from(String::from_utf16(&buffer)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn managed_copy_preserves_source_files_and_skips_build_caches() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let source = temporary.path().join("source");
        let destination = temporary.path().join("copy");
        fs::create_dir_all(source.join("node_modules"))?;
        fs::create_dir_all(source.join("target"))?;
        fs::write(source.join("app.ts"), "export const app = true;\n")?;
        fs::write(source.join(".gitignore"), "dist/\n")?;
        fs::write(source.join("node_modules/dependency.js"), "ignored\n")?;
        fs::write(source.join("target/artifact"), "ignored\n")?;

        copy_external_tree_bounded(&source, &destination)?;
        assert_eq!(
            fs::read_to_string(destination.join("app.ts"))?,
            "export const app = true;\n"
        );
        assert!(destination.join(".gitignore").is_file());
        assert!(!destination.join("node_modules").exists());
        assert!(!destination.join("target").exists());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn managed_copy_preserves_contained_relative_symlink() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let source = temporary.path().join("source");
        let destination = temporary.path().join("copy");
        fs::create_dir_all(&source)?;
        fs::write(source.join("actual.txt"), "contained")?;
        std::os::unix::fs::symlink("actual.txt", source.join("alias.txt"))?;

        copy_external_tree_bounded(&source, &destination)?;
        assert_eq!(
            fs::read_link(destination.join("alias.txt"))?,
            Path::new("actual.txt")
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn managed_copy_rejects_static_symlink_escape() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let source = temporary.path().join("source");
        let outside = temporary.path().join("outside");
        fs::create_dir_all(&source)?;
        fs::create_dir_all(&outside)?;
        fs::write(outside.join("private"), "private")?;
        std::os::unix::fs::symlink("../outside/private", source.join("escape"))?;

        assert!(copy_external_tree_bounded(&source, &temporary.path().join("copy")).is_err());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn managed_copy_rejects_intermediate_directory_symlink() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let source = temporary.path().join("source");
        let outside = temporary.path().join("outside");
        fs::create_dir_all(&source)?;
        fs::create_dir_all(&outside)?;
        fs::write(outside.join("private"), "private")?;
        std::os::unix::fs::symlink("../outside", source.join("intermediate"))?;

        let destination = temporary.path().join("copy");
        assert!(copy_external_tree_bounded(&source, &destination).is_err());
        assert!(!destination.join("intermediate/private").exists());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn managed_copy_rejects_controlled_file_to_symlink_swap() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let source = temporary.path().join("source");
        let outside = temporary.path().join("outside");
        fs::create_dir_all(&source)?;
        fs::create_dir_all(&outside)?;
        fs::write(source.join("victim.txt"), "public")?;
        fs::write(outside.join("private.txt"), "private")?;

        let victim = source.join("victim.txt");
        let parked = source.join("victim.parked");
        let (start_swap, wait_for_swap) = std::sync::mpsc::sync_channel(0);
        let (swap_complete, wait_for_completion) = std::sync::mpsc::sync_channel(0);
        let swap_victim = victim.clone();
        let swapper = std::thread::spawn(move || -> Result<()> {
            wait_for_swap.recv()?;
            fs::rename(&swap_victim, &parked)?;
            std::os::unix::fs::symlink("../outside/private.txt", &swap_victim)?;
            swap_complete.send(())?;
            Ok(())
        });
        let mut hook_called = false;
        let result = copy_external_tree_bounded_impl(
            &source,
            &temporary.path().join("copy"),
            &mut |about_to_open| {
                if !hook_called && about_to_open == victim {
                    start_swap.send(()).expect("start controlled source swap");
                    wait_for_completion
                        .recv()
                        .expect("wait for controlled source swap");
                    hook_called = true;
                }
            },
        );
        if !hook_called {
            start_swap.send(())?;
            wait_for_completion.recv()?;
        }
        swapper.join().expect("controlled swap thread panicked")?;

        assert!(hook_called, "the controlled race hook did not execute");
        assert!(result.is_err());
        assert_ne!(
            fs::read_to_string(temporary.path().join("copy/victim.txt")).ok(),
            Some("private".to_owned())
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn managed_copy_rejects_controlled_directory_to_symlink_swap() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let source = temporary.path().join("source");
        let outside = temporary.path().join("outside");
        fs::create_dir_all(source.join("victim"))?;
        fs::create_dir_all(&outside)?;
        fs::write(source.join("victim/public.txt"), "public")?;
        fs::write(outside.join("private.txt"), "private")?;

        let victim = source.join("victim");
        let parked = source.join("victim.parked");
        let (start_swap, wait_for_swap) = std::sync::mpsc::sync_channel(0);
        let (swap_complete, wait_for_completion) = std::sync::mpsc::sync_channel(0);
        let swap_victim = victim.clone();
        let swapper = std::thread::spawn(move || -> Result<()> {
            wait_for_swap.recv()?;
            fs::rename(&swap_victim, &parked)?;
            std::os::unix::fs::symlink("../outside", &swap_victim)?;
            swap_complete.send(())?;
            Ok(())
        });
        let mut hook_called = false;
        let destination = temporary.path().join("copy");
        let result = copy_external_tree_bounded_impl(&source, &destination, &mut |path| {
            if !hook_called && path == victim {
                start_swap.send(()).expect("start controlled source swap");
                wait_for_completion
                    .recv()
                    .expect("wait for controlled source swap");
                hook_called = true;
            }
        });
        if !hook_called {
            start_swap.send(())?;
            wait_for_completion.recv()?;
        }
        swapper.join().expect("controlled swap thread panicked")?;

        assert!(hook_called, "the controlled race hook did not execute");
        assert!(result.is_err());
        assert!(!destination.join("victim/private.txt").exists());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn managed_copy_rejects_controlled_destination_ancestor_to_symlink_swap() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let source = temporary.path().join("source");
        let owner_path = temporary.path().join("workspace");
        let parked = temporary.path().join("workspace.parked");
        let outside = temporary.path().join("outside");
        fs::create_dir_all(&source)?;
        fs::create_dir_all(&outside)?;
        fs::write(source.join("public.txt"), "public")?;

        let owner = ManagedDirectory::create(&owner_path)?;
        let staging = owner.create_child(".source-staging".as_ref())?;
        let (start_swap, wait_for_swap) = std::sync::mpsc::sync_channel(0);
        let (swap_complete, wait_for_completion) = std::sync::mpsc::sync_channel(0);
        let swap_owner = owner_path.clone();
        let swap_outside = outside.clone();
        let swapper = std::thread::spawn(move || -> Result<()> {
            wait_for_swap.recv()?;
            fs::rename(&swap_owner, &parked)?;
            std::os::unix::fs::symlink(&swap_outside, &swap_owner)?;
            swap_complete.send(())?;
            Ok(())
        });

        let mut hook_called = false;
        let result = copy_external_tree_bounded_into_impl(&source, &staging, &mut |_| {
            if !hook_called {
                start_swap
                    .send(())
                    .expect("start controlled destination swap");
                wait_for_completion
                    .recv()
                    .expect("wait for controlled destination swap");
                hook_called = true;
            }
        });
        swapper.join().expect("controlled swap thread panicked")?;

        assert!(hook_called, "the controlled race hook did not execute");
        assert!(result.is_err());
        assert!(!outside.join("public.txt").exists());
        Ok(())
    }

    #[cfg(windows)]
    fn create_junction(target: &Path, junction: &Path) -> Result<()> {
        // Directory junctions exercise Windows reparse handling without requiring the
        // SeCreateSymbolicLinkPrivilege. Setup failure is explicit rather than a skipped pass.
        let output = std::process::Command::new("cmd")
            .args(["/d", "/c", "mklink", "/J"])
            .arg(junction)
            .arg(target)
            .output()?;
        anyhow::ensure!(
            output.status.success(),
            "Windows reparse test setup failed (junction support is required): {}",
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn pinned_windows_directory_accepts_fetch_writes_without_reopening_its_ancestors() -> Result<()>
    {
        let temporary = tempfile::tempdir()?;
        let owner = ManagedDirectory::create(&temporary.path().join("workspace"))?;
        let staging = owner.create_child(".source-staging".as_ref())?;
        let stable_path = staging.stable_access_path()?;

        fs::write(stable_path.join("fetched.txt"), "fetched")?;

        staging.ensure_path_identity()?;
        assert_eq!(
            fs::read_to_string(stable_path.join("fetched.txt"))?,
            "fetched"
        );
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn managed_copy_rejects_windows_intermediate_reparse_point() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let source = temporary.path().join("source");
        let outside = temporary.path().join("outside");
        fs::create_dir_all(&source)?;
        fs::create_dir_all(&outside)?;
        fs::write(outside.join("private.txt"), "private")?;
        create_junction(&outside, &source.join("intermediate"))?;

        let destination = temporary.path().join("copy");
        assert!(copy_external_tree_bounded(&source, &destination).is_err());
        assert!(!destination.join("intermediate/private.txt").exists());
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn managed_copy_rejects_controlled_directory_to_junction_swap() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let source = temporary.path().join("source");
        let outside = temporary.path().join("outside");
        fs::create_dir_all(source.join("victim"))?;
        fs::create_dir_all(&outside)?;
        fs::write(source.join("victim/public.txt"), "public")?;
        fs::write(outside.join("private.txt"), "private")?;
        let replacement = temporary.path().join("replacement-junction");
        create_junction(&outside, &replacement)?;

        let victim = source.join("victim");
        let parked = source.join("victim.parked");
        let (start_swap, wait_for_swap) = std::sync::mpsc::sync_channel(0);
        let (swap_complete, wait_for_completion) = std::sync::mpsc::sync_channel(0);
        let swap_victim = victim.clone();
        let swapper = std::thread::spawn(move || -> Result<()> {
            wait_for_swap.recv()?;
            fs::rename(&swap_victim, &parked)?;
            fs::rename(&replacement, &swap_victim)?;
            swap_complete.send(())?;
            Ok(())
        });
        let mut hook_called = false;
        let destination = temporary.path().join("copy");
        let result = copy_external_tree_bounded_impl(&source, &destination, &mut |path| {
            if !hook_called && path.file_name() == victim.file_name() {
                start_swap.send(()).expect("start controlled source swap");
                wait_for_completion
                    .recv()
                    .expect("wait for controlled source swap");
                hook_called = true;
            }
        });
        if !hook_called {
            start_swap.send(())?;
            wait_for_completion.recv()?;
        }
        swapper.join().expect("controlled swap thread panicked")?;

        assert!(hook_called, "the controlled race hook did not execute");
        assert!(result.is_err());
        assert!(!destination.join("victim/private.txt").exists());
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn managed_copy_blocks_controlled_destination_ancestor_to_junction_swap() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let source = temporary.path().join("source");
        let owner_path = temporary.path().join("workspace");
        let parked = temporary.path().join("workspace.parked");
        let outside = temporary.path().join("outside");
        let replacement = temporary.path().join("replacement-junction");
        fs::create_dir_all(&source)?;
        fs::create_dir_all(&outside)?;
        fs::write(source.join("public.txt"), "public")?;
        create_junction(&outside, &replacement)?;

        let owner = ManagedDirectory::create(&owner_path)?;
        let staging = owner.create_child(".source-staging".as_ref())?;
        let (start_swap, wait_for_swap) = std::sync::mpsc::sync_channel(0);
        let (swap_complete, wait_for_completion) = std::sync::mpsc::sync_channel(0);
        let swap_owner = owner_path.clone();
        let swapper = std::thread::spawn(move || -> Result<()> {
            wait_for_swap.recv()?;
            let renamed = fs::rename(&swap_owner, &parked).is_ok();
            if renamed {
                fs::rename(&replacement, &swap_owner)?;
            }
            swap_complete.send(renamed)?;
            Ok(())
        });

        let mut swap_succeeded = false;
        let result = copy_external_tree_bounded_into_impl(&source, &staging, &mut |_| {
            if !swap_succeeded {
                start_swap
                    .send(())
                    .expect("start controlled destination swap");
                swap_succeeded = wait_for_completion
                    .recv()
                    .expect("wait for controlled destination swap");
            }
        });
        swapper.join().expect("controlled swap thread panicked")?;

        assert!(
            !swap_succeeded,
            "the pinned Windows destination ancestor was renamed"
        );
        result?;
        assert!(!outside.join("public.txt").exists());
        Ok(())
    }
}
