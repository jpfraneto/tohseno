//! Shared bounded readers for local files that can be replaced by another
//! process while the engine is validating them.

use std::fs::{self, Metadata, OpenOptions};
use std::io::{self, Read};
use std::path::Path;

/// Reads one caller-bounded regular file without following its final symlink.
///
/// The path is inspected before opening, the descriptor is compared with that
/// observation, and both the descriptor and path are inspected again after
/// reading. This makes user-controlled factory inputs fail closed when they
/// are replaced or modified during admission.
pub fn read_bounded_regular_file(path: &Path, maximum: u64) -> io::Result<Vec<u8>> {
    read_bounded_regular_file_impl(path, maximum, || {})
}

/// Applies [`read_bounded_regular_file`] and additionally requires UTF-8.
pub fn read_bounded_utf8(path: &Path, maximum: u64) -> io::Result<String> {
    String::from_utf8(read_bounded_regular_file(path, maximum)?).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "bounded regular file is not valid UTF-8",
        )
    })
}

fn read_bounded_regular_file_impl(
    path: &Path,
    maximum: u64,
    after_open: impl FnOnce(),
) -> io::Result<Vec<u8>> {
    let initial = fs::symlink_metadata(path)?;
    require_bounded_regular(&initial, maximum)?;

    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let mut file = options.open(path)?;
    let opened = file.metadata()?;
    if !opened.is_file() || opened.len() > maximum || !same_file_version(&initial, &opened) {
        return Err(changed_file_error());
    }

    after_open();

    let capacity = usize::try_from(opened.len()).unwrap_or(0);
    let mut bytes = Vec::with_capacity(capacity);
    file.by_ref()
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)?;
    let descriptor_after_read = file.metadata()?;
    let path_after_read = fs::symlink_metadata(path)?;
    if bytes.len() as u64 > maximum
        || bytes.len() as u64 != opened.len()
        || !descriptor_after_read.is_file()
        || path_after_read.file_type().is_symlink()
        || !path_after_read.is_file()
        || !same_file_version(&opened, &descriptor_after_read)
        || !same_file_version(&opened, &path_after_read)
    {
        return Err(changed_file_error());
    }
    Ok(bytes)
}

fn require_bounded_regular(metadata: &Metadata, maximum: u64) -> io::Result<()> {
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > maximum {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "path is not a bounded regular file",
        ));
    }
    Ok(())
}

fn changed_file_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "bounded regular file changed while it was read",
    )
}

#[cfg(unix)]
fn same_file_version(left: &Metadata, right: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(not(unix))]
fn same_file_version(left: &Metadata, right: &Metadata) -> bool {
    left.file_type() == right.file_type()
        && left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
        && left.created().ok() == right.created().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_oversized_files_before_reading() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("bounded.bin");
        fs::write(&path, b"12345").unwrap();

        let error = read_bounded_regular_file(&path, 4).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symbolic_links_without_following_them() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("target");
        let link = directory.path().join("link");
        fs::write(&target, b"private").unwrap();
        symlink(&target, &link).unwrap();

        let error = read_bounded_regular_file(&link, 64).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn rejects_a_path_replaced_after_its_descriptor_is_open() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("input");
        let displaced = directory.path().join("displaced");
        fs::write(&path, b"first").unwrap();

        let error = read_bounded_regular_file_impl(&path, 64, || {
            fs::rename(&path, &displaced).unwrap();
            fs::write(&path, b"other").unwrap();
        })
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }
}
