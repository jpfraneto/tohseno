use crate::catalog::DependencyLock;
use crate::{NetworkError, Result};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Component, Path, PathBuf};
use tar::{Archive, Builder, EntryType, Header};
use tohseno_protocol::digest::Bytes32;
use unicode_normalization::UnicodeNormalization;

pub const MAX_FILE_COUNT: u64 = 100_000;
pub const MAX_FILE_BYTES: u64 = 128 * 1024 * 1024;
pub const MAX_TREE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub const MAX_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotReport {
    pub artifact_sha256: Bytes32,
    pub artifact_byte_length: u64,
    pub source_tree_sha256: Bytes32,
    pub file_count: u64,
    pub source_byte_length: u64,
    pub paths: Vec<String>,
    pub dependency_locks: Vec<DependencyLock>,
}

#[derive(Clone, Debug)]
struct FileEntry {
    absolute: PathBuf,
    relative: String,
    byte_length: u64,
    executable: bool,
}

pub fn create_deterministic_snapshot(
    source_root: &Path,
    destination: &Path,
) -> Result<SnapshotReport> {
    let root = source_root.canonicalize()?;
    let root_metadata = fs::symlink_metadata(&root)?;
    if !root_metadata.is_dir() || root_metadata.file_type().is_symlink() {
        return Err(NetworkError::UnsafePath(source_root.to_path_buf()));
    }
    if destination.exists() {
        return Err(NetworkError::UnsafePath(destination.to_path_buf()));
    }
    let entries = collect_files(&root)?;
    if entries.is_empty() {
        return Err(NetworkError::Invalid(
            "publication snapshot is empty".into(),
        ));
    }
    if destination.starts_with(&root) {
        return Err(NetworkError::UnsafePath(destination.to_path_buf()));
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination)?;
    let mut hashing = HashingWriter::new(BufWriter::new(file));
    {
        let mut archive = Builder::new(&mut hashing);
        archive.mode(tar::HeaderMode::Deterministic);
        for entry in &entries {
            let before = fs::symlink_metadata(&entry.absolute)?;
            if !before.file_type().is_file()
                || before.file_type().is_symlink()
                || has_multiple_links(&before)
            {
                return Err(NetworkError::UnsafePath(entry.absolute.clone()));
            }
            let mut input = File::open(&entry.absolute)?;
            let after_open = input.metadata()?;
            if !same_file_observation(&before, &after_open) {
                return Err(NetworkError::ConcurrentMutation(entry.absolute.clone()));
            }
            let mut header = Header::new_gnu();
            header.set_entry_type(EntryType::Regular);
            header.set_size(entry.byte_length);
            header.set_mode(if entry.executable { 0o755 } else { 0o644 });
            header.set_uid(0);
            header.set_gid(0);
            header.set_mtime(0);
            header.set_username("")?;
            header.set_groupname("")?;
            header.set_cksum();
            archive.append_data(&mut header, &entry.relative, &mut input)?;
            let after = fs::symlink_metadata(&entry.absolute)?;
            if !same_file_observation(&before, &after) {
                return Err(NetworkError::ConcurrentMutation(entry.absolute.clone()));
            }
        }
        archive.finish()?;
    }
    hashing.flush()?;
    let (artifact_sha256, artifact_byte_length) = hashing.finish();
    if artifact_byte_length > MAX_ARCHIVE_BYTES {
        return Err(NetworkError::Oversized);
    }

    let verification_root = sibling_verification_directory(destination)?;
    fs::create_dir(&verification_root)?;
    let extraction =
        extract_verified_snapshot(destination, &verification_root, Some(artifact_sha256));
    let report = match extraction {
        Ok(value) => value,
        Err(error) => {
            let _ = fs::remove_dir_all(&verification_root);
            return Err(error);
        }
    };
    let verified_metadata = (|| -> Result<_> {
        Ok((
            tohseno_protocol::tree_hash::hash_source_tree(&verification_root)?.digest,
            crate::build_profile::collect_dependency_locks(&verification_root)?,
        ))
    })();
    let (source_tree_sha256, dependency_locks) = match verified_metadata {
        Ok(value) => value,
        Err(error) => {
            let _ = fs::remove_dir_all(&verification_root);
            return Err(error);
        }
    };
    fs::remove_dir_all(&verification_root)?;
    Ok(SnapshotReport {
        artifact_sha256,
        artifact_byte_length,
        source_tree_sha256,
        file_count: report.file_count,
        source_byte_length: report.source_byte_length,
        paths: report.paths,
        dependency_locks,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtractionReport {
    pub file_count: u64,
    pub source_byte_length: u64,
    pub paths: Vec<String>,
}

pub fn extract_verified_snapshot(
    artifact: &Path,
    destination: &Path,
    expected_sha256: Option<Bytes32>,
) -> Result<ExtractionReport> {
    let destination_meta = fs::symlink_metadata(destination)?;
    if !destination_meta.is_dir() || destination_meta.file_type().is_symlink() {
        return Err(NetworkError::UnsafePath(destination.to_path_buf()));
    }
    if fs::read_dir(destination)?.next().is_some() {
        return Err(NetworkError::UnsafePath(destination.to_path_buf()));
    }
    let metadata = fs::symlink_metadata(artifact)?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_ARCHIVE_BYTES {
        return Err(NetworkError::Oversized);
    }
    if let Some(expected) = expected_sha256 {
        let observed = hash_file(artifact)?;
        if observed != expected {
            return Err(NetworkError::Invalid(
                "source artifact digest mismatch".into(),
            ));
        }
    }
    let file = File::open(artifact)?;
    let mut archive = Archive::new(BufReader::new(file));
    let mut seen = BTreeSet::new();
    let mut casefolded = BTreeMap::new();
    let mut count = 0_u64;
    let mut bytes = 0_u64;
    for item in archive.entries()? {
        let mut item = item?;
        if item.header().entry_type() != EntryType::Regular {
            return Err(NetworkError::Invalid(
                "source archive contains a non-regular entry".into(),
            ));
        }
        let path = item.path()?.into_owned();
        let normalized = safe_relative_path(&path)?;
        let folded = normalized.to_lowercase();
        if !seen.insert(normalized.clone())
            || casefolded.insert(folded, normalized.clone()).is_some()
        {
            return Err(NetworkError::Invalid(
                "source archive contains a path collision".into(),
            ));
        }
        let length = item.header().size()?;
        if length > MAX_FILE_BYTES {
            return Err(NetworkError::Oversized);
        }
        count = count.checked_add(1).ok_or(NetworkError::Oversized)?;
        bytes = bytes.checked_add(length).ok_or(NetworkError::Oversized)?;
        if count > MAX_FILE_COUNT || bytes > MAX_TREE_BYTES {
            return Err(NetworkError::Oversized);
        }
        let output = destination.join(&normalized);
        if let Some(parent) = output.parent() {
            create_safe_directories(destination, parent)?;
        }
        let mut target = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&output)?;
        let mode = item.header().mode()?;
        std::io::copy(&mut item, &mut target)?;
        target.flush()?;
        if mode & 0o111 != 0 {
            set_executable(&output)?;
        }
    }
    Ok(ExtractionReport {
        file_count: count,
        source_byte_length: bytes,
        paths: seen.into_iter().collect(),
    })
}

fn collect_files(root: &Path) -> Result<Vec<FileEntry>> {
    let mut files = Vec::new();
    collect_directory(root, root, 0, &mut files)?;
    files.sort_by(|left, right| left.relative.as_bytes().cmp(right.relative.as_bytes()));
    let mut seen = BTreeSet::new();
    let mut folded = BTreeSet::new();
    let mut total = 0_u64;
    for file in &files {
        if !seen.insert(file.relative.clone()) || !folded.insert(file.relative.to_lowercase()) {
            return Err(NetworkError::Invalid(
                "publication snapshot contains a path collision".into(),
            ));
        }
        total = total
            .checked_add(file.byte_length)
            .ok_or(NetworkError::Oversized)?;
    }
    if files.len() as u64 > MAX_FILE_COUNT || total > MAX_TREE_BYTES {
        return Err(NetworkError::Oversized);
    }
    Ok(files)
}

fn collect_directory(
    root: &Path,
    directory: &Path,
    depth: usize,
    files: &mut Vec<FileEntry>,
) -> Result<()> {
    if depth > 64 {
        return Err(NetworkError::UnsafePath(directory.to_path_buf()));
    }
    let mut children = fs::read_dir(directory)?.collect::<std::io::Result<Vec<_>>>()?;
    children.sort_by_key(|entry| entry.file_name());
    for entry in children {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        let relative_path = path
            .strip_prefix(root)
            .map_err(|_| NetworkError::UnsafePath(path.clone()))?;
        let relative = safe_relative_path(relative_path)?;
        if metadata.file_type().is_symlink()
            || (!metadata.is_dir() && !metadata.is_file())
            || (metadata.is_file() && has_multiple_links(&metadata))
        {
            return Err(NetworkError::UnsafePath(relative_path.to_path_buf()));
        }
        if metadata.is_dir() {
            if excluded_directory(&relative) {
                continue;
            }
            collect_directory(root, &path, depth + 1, files)?;
            continue;
        }
        if secret_path(&relative) {
            return Err(NetworkError::SecretPath(PathBuf::from(relative)));
        }
        if excluded_file(&relative) {
            continue;
        }
        if metadata.len() > MAX_FILE_BYTES {
            return Err(NetworkError::Oversized);
        }
        if likely_secret_content(&path, metadata.len())? {
            return Err(NetworkError::SecretContent(PathBuf::from(relative)));
        }
        files.push(FileEntry {
            absolute: path,
            relative,
            byte_length: metadata.len(),
            executable: executable(&metadata),
        });
    }
    Ok(())
}

fn excluded_directory(path: &str) -> bool {
    path.split('/').any(|part| {
        matches!(
            part,
            ".git"
                | "DerivedData"
                | ".build"
                | "build"
                | ".swiftpm"
                | "xcuserdata"
                | ".tohseno"
                | ".idea"
                | ".vscode"
        )
    })
}

fn excluded_file(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    name == ".DS_Store"
        || name.ends_with(".xcuserstate")
        || name.ends_with(".log")
        || name.ends_with('~')
}

fn secret_path(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();
    name == ".env"
        || name.starts_with(".env.")
        || [
            "p8",
            "p12",
            "pem",
            "pfx",
            "key",
            "mobileprovision",
            "cer",
            "crt",
        ]
        .iter()
        .any(|extension| name.ends_with(&format!(".{extension}")))
}

fn likely_secret_content(path: &Path, length: u64) -> Result<bool> {
    if length > 8 * 1024 * 1024 {
        return Ok(false);
    }
    let mut input = File::open(path)?;
    let mut bytes = Vec::with_capacity(length as usize);
    input.read_to_end(&mut bytes)?;
    const NEEDLES: [&[u8]; 10] = [
        b"-----BEGIN PRIVATE KEY-----",
        b"-----BEGIN EC PRIVATE KEY-----",
        b"-----BEGIN RSA PRIVATE KEY-----",
        b"sk_live_",
        b"rk_live_",
        b"ghp_",
        b"github_pat_",
        b"AKIA",
        b"BANKR_API_KEY=",
        b"STRIPE_SECRET_KEY=",
    ];
    Ok(NEEDLES
        .iter()
        .any(|needle| bytes.windows(needle.len()).any(|window| window == *needle)))
}

fn safe_relative_path(path: &Path) -> Result<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => {
                let value = value
                    .to_str()
                    .ok_or_else(|| NetworkError::UnsafePath(path.to_path_buf()))?;
                if value.is_empty()
                    || value
                        .chars()
                        .any(|character| character == '\0' || character.is_control())
                {
                    return Err(NetworkError::UnsafePath(path.to_path_buf()));
                }
                let normalized = value.nfc().collect::<String>();
                if normalized != value {
                    return Err(NetworkError::UnsafePath(path.to_path_buf()));
                }
                parts.push(normalized);
            }
            _ => return Err(NetworkError::UnsafePath(path.to_path_buf())),
        }
    }
    if parts.is_empty() {
        return Err(NetworkError::UnsafePath(path.to_path_buf()));
    }
    Ok(parts.join("/"))
}

fn create_safe_directories(root: &Path, target: &Path) -> Result<()> {
    let relative = target
        .strip_prefix(root)
        .map_err(|_| NetworkError::UnsafePath(target.to_path_buf()))?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(value) = component else {
            return Err(NetworkError::UnsafePath(target.to_path_buf()));
        };
        current.push(value);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
            Ok(_) => return Err(NetworkError::UnsafePath(current)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir(&current)?,
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn hash_file(path: &Path) -> Result<Bytes32> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(Bytes32::new(digest.finalize().into()))
}

fn sibling_verification_directory(destination: &Path) -> Result<PathBuf> {
    let parent = destination
        .parent()
        .ok_or_else(|| NetworkError::UnsafePath(destination.to_path_buf()))?;
    let name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| NetworkError::UnsafePath(destination.to_path_buf()))?;
    for attempt in 0..1_000_u32 {
        let candidate = parent.join(format!(".{name}.verify-{attempt}"));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(NetworkError::Invalid(
        "could not reserve verification directory".into(),
    ))
}

#[cfg(unix)]
fn executable(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn executable(_metadata: &fs::Metadata) -> bool {
    false
}

#[cfg(unix)]
fn has_multiple_links(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    metadata.nlink() != 1
}

#[cfg(not(unix))]
fn has_multiple_links(_metadata: &fs::Metadata) -> bool {
    false
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<()> {
    Ok(())
}

fn same_file_observation(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    same_file_identity(before, after)
        && before.len() == after.len()
        && before.modified().ok() == after.modified().ok()
}

#[cfg(unix)]
fn same_file_identity(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    before.dev() == after.dev()
        && before.ino() == after.ino()
        && before.ctime() == after.ctime()
        && before.ctime_nsec() == after.ctime_nsec()
        && before.nlink() == 1
        && after.nlink() == 1
}

#[cfg(not(unix))]
fn same_file_identity(_before: &fs::Metadata, _after: &fs::Metadata) -> bool {
    true
}

struct HashingWriter<W> {
    inner: W,
    digest: Sha256,
    bytes: u64,
}

impl<W> HashingWriter<W> {
    fn new(inner: W) -> Self {
        Self {
            inner,
            digest: Sha256::new(),
            bytes: 0,
        }
    }

    fn finish(self) -> (Bytes32, u64) {
        (Bytes32::new(self.digest.finalize().into()), self.bytes)
    }
}

impl<W: Write> Write for HashingWriter<W> {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let written = self.inner.write(buffer)?;
        self.digest.update(&buffer[..written]);
        self.bytes = self.bytes.saturating_add(written as u64);
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn fixture() -> (TempDir, PathBuf) {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("App.swift"), b"import SwiftUI\n").unwrap();
        fs::create_dir(source.join("Fixture.xcodeproj")).unwrap();
        fs::write(
            source.join("Fixture.xcodeproj/project.pbxproj"),
            b"fixture\n",
        )
        .unwrap();
        (temp, source)
    }

    #[test]
    fn snapshot_is_deterministic_and_round_trips() {
        let (temp, source) = fixture();
        let first = temp.path().join("first.tar");
        let second = temp.path().join("second.tar");
        let a = create_deterministic_snapshot(&source, &first).unwrap();
        let b = create_deterministic_snapshot(&source, &second).unwrap();
        assert_eq!(a.artifact_sha256, b.artifact_sha256);
        assert_eq!(fs::read(first).unwrap(), fs::read(second).unwrap());
        assert_eq!(a.file_count, 2);
        assert_eq!(a.source_tree_sha256, b.source_tree_sha256);
    }

    #[test]
    fn secrets_and_symlinks_fail_closed() {
        let (temp, source) = fixture();
        fs::write(source.join(".env"), b"TOKEN=secret").unwrap();
        assert!(matches!(
            create_deterministic_snapshot(&source, &temp.path().join("secret.tar")),
            Err(NetworkError::SecretPath(_))
        ));
        fs::remove_file(source.join(".env")).unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(source.join("App.swift"), source.join("Alias.swift"))
                .unwrap();
            assert!(matches!(
                create_deterministic_snapshot(&source, &temp.path().join("link.tar")),
                Err(NetworkError::UnsafePath(_))
            ));
            fs::remove_file(source.join("Alias.swift")).unwrap();
            fs::hard_link(source.join("App.swift"), source.join("HardLink.swift")).unwrap();
            assert!(matches!(
                create_deterministic_snapshot(&source, &temp.path().join("hard-link.tar")),
                Err(NetworkError::UnsafePath(_))
            ));
        }
    }

    #[test]
    fn extraction_rejects_parent_traversal() {
        let temp = TempDir::new().unwrap();
        let tar_path = temp.path().join("bad.tar");
        let mut header = Header::new_gnu();
        header.set_size(1);
        header.set_mode(0o644);
        header.set_cksum();
        // tar::Builder itself rejects parent traversal. Hand-author the name
        // field to prove our reader also refuses it.
        let mut raw = header.as_bytes().to_owned();
        raw[..11].copy_from_slice(b"../x.swift\0");
        raw[148..156].fill(b' ');
        let checksum: u32 = raw.iter().map(|byte| u32::from(*byte)).sum();
        let encoded = format!("{checksum:06o}\0 ");
        raw[148..156].copy_from_slice(encoded.as_bytes());
        let mut output = File::create(&tar_path).unwrap();
        output.write_all(&raw).unwrap();
        output.write_all(b"x").unwrap();
        output.write_all(&vec![0; 511 + 1024]).unwrap();
        let destination = temp.path().join("out");
        fs::create_dir(&destination).unwrap();
        assert!(extract_verified_snapshot(&tar_path, &destination, None).is_err());
    }
}
