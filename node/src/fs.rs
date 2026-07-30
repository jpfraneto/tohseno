use crate::{NodeError, Result};
use rand_core::{OsRng, RngCore};
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

pub(crate) fn absolute(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

/// Creates a directory one component at a time and rejects every symlink in
/// the resolved storage path. This intentionally does not canonicalize through
/// a link supplied by an operator.
pub(crate) fn ensure_real_directory(path: &Path) -> Result<PathBuf> {
    let path = absolute(path)?;
    if path
        .components()
        .any(|component| component == Component::ParentDir)
    {
        return Err(NodeError::UnsafeStorage(path));
    }

    // Resolve the nearest existing real directory once, then retain that
    // canonical path for all node operations. This accepts macOS's system
    // `/var -> /private/var` alias without allowing the requested storage
    // directory itself (or the first missing ancestor) to be a symlink.
    let mut existing = path.clone();
    let mut missing = Vec::<OsString>::new();
    loop {
        match fs::symlink_metadata(&existing) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(NodeError::UnsafeStorage(existing));
            }
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let name = existing
                    .file_name()
                    .ok_or_else(|| NodeError::UnsafeStorage(path.clone()))?
                    .to_os_string();
                missing.push(name);
                if !existing.pop() {
                    return Err(NodeError::UnsafeStorage(path));
                }
            }
            Err(error) => return Err(error.into()),
        }
    }
    let mut current = fs::canonicalize(existing)?;
    for part in missing.into_iter().rev() {
        current.push(part);
        match fs::create_dir(&current) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
        let metadata = fs::symlink_metadata(&current)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(NodeError::UnsafeStorage(current));
        }
    }
    Ok(current)
}

pub(crate) fn read_regular_limited(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(NodeError::UnsafeStorage(path.to_path_buf()));
    }
    if metadata.len() > limit as u64 {
        return Err(NodeError::ActionTooLarge { limit });
    }
    let mut options = OpenOptions::new();
    options.read(true);
    set_no_follow(&mut options);
    let file = options.open(path)?;
    let opened = file.metadata()?;
    if !opened.is_file() || opened.len() > limit as u64 {
        return Err(NodeError::UnsafeStorage(path.to_path_buf()));
    }
    let mut bytes = Vec::with_capacity(opened.len() as usize);
    file.take((limit + 1) as u64).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(NodeError::ActionTooLarge { limit });
    }
    Ok(bytes)
}

/// Atomically creates `target` without ever replacing an existing entry.
///
/// A private temporary regular file is fsynced, then linked to the final name.
/// `hard_link` gives create-new semantics on the same filesystem. The caller
/// may treat `Ok(false)` as an idempotent pre-existing target and compare its
/// bytes before trusting it.
pub(crate) fn create_new_atomic(target: &Path, bytes: &[u8]) -> Result<bool> {
    let parent = target
        .parent()
        .ok_or_else(|| NodeError::UnsafeStorage(target.to_path_buf()))?;
    ensure_real_directory(parent)?;
    match fs::symlink_metadata(target) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(NodeError::UnsafeStorage(target.to_path_buf()));
        }
        Ok(_) => return Ok(false),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    for _ in 0..128 {
        let temporary = parent.join(format!(".node-write-{}", random_suffix()));
        let mut options = OpenOptions::new();
        options.create_new(true).write(true);
        set_private_create(&mut options);
        match options.open(&temporary) {
            Ok(mut file) => {
                let result = (|| {
                    file.write_all(bytes)?;
                    file.sync_all()?;
                    drop(file);
                    match fs::hard_link(&temporary, target) {
                        Ok(()) => {
                            sync_directory(parent)?;
                            Ok(true)
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                            Ok(false)
                        }
                        Err(error) => Err(error.into()),
                    }
                })();
                let _ = fs::remove_file(&temporary);
                return result;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err(NodeError::Io(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not allocate a private temporary storage file",
    )))
}

fn random_suffix() -> String {
    let mut bytes = [0_u8; 16];
    OsRng.fill_bytes(&mut bytes);
    let mut output = String::with_capacity(32);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn sync_directory(path: &Path) -> Result<()> {
    File::open(path)?.sync_all()?;
    Ok(())
}

#[cfg(unix)]
fn set_no_follow(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.custom_flags(libc::O_NOFOLLOW);
}

#[cfg(not(unix))]
fn set_no_follow(_options: &mut OpenOptions) {}

#[cfg(unix)]
fn set_private_create(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.custom_flags(libc::O_NOFOLLOW).mode(0o600);
}

#[cfg(not(unix))]
fn set_private_create(_options: &mut OpenOptions) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_new_never_overwrites() {
        let temporary = tempfile::tempdir().unwrap();
        let target = temporary.path().join("record");
        assert!(create_new_atomic(&target, b"first").unwrap());
        assert!(!create_new_atomic(&target, b"second").unwrap());
        assert_eq!(read_regular_limited(&target, 16).unwrap(), b"first");
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_storage_component() {
        use std::os::unix::fs::symlink;
        let temporary = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        symlink(outside.path(), temporary.path().join("linked")).unwrap();
        assert!(matches!(
            ensure_real_directory(&temporary.path().join("linked/actions")),
            Err(NodeError::UnsafeStorage(_))
        ));
    }
}
