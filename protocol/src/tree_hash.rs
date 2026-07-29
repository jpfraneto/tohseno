use crate::digest::{sha256, Bytes32};
use crate::{ProtocolError, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, Metadata};
use std::io::Read;
use std::path::{Component, Path};
use unicode_normalization::UnicodeNormalization;

pub const SOURCE_TREE_DOMAIN: &[u8] = b"TOHSENO-SOURCE-TREE-V1\0";

/// These protocol-generated sidecars contain or depend on the tree
/// commitment. Excluding them prevents self-reference. `fascia.json` and all
/// actual Fascia source remain included.
pub const SELF_REFERENTIAL_EXCLUSIONS: &[&str] = &[
    "TOHSENO/shot.json",
    "TOHSENO/signature.json",
    "TOHSENO/conformance.json",
    "TOHSENO/embedded-provenance.json",
];

const EXCLUDED_DIRECTORY_COMPONENTS: &[&str] = &[
    ".git",
    ".tohseno",
    ".build",
    ".swiftpm",
    "DerivedData",
    "build",
    "xcuserdata",
];

const PRIVATE_FILE_NAMES: &[&str] = &[
    ".DS_Store",
    ".env",
    "build.log",
    "harness.log",
    "prompt.md",
    "TASK.md",
];

const PRIVATE_EXTENSIONS: &[&str] = &[
    "log",
    "mobileprovision",
    "p8",
    "p12",
    "pem",
    "pfx",
    "xcuserstate",
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceTreeEntry {
    pub path: String,
    pub content_sha256: Bytes32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceTreeCommitment {
    pub digest: Bytes32,
    pub entries: Vec<SourceTreeEntry>,
}

/// Hashes one explicit source root without consulting Git, ignore files, the
/// current directory, environment variables, or any global filesystem state.
pub fn hash_source_tree(root: &Path) -> Result<SourceTreeCommitment> {
    let root_metadata = metadata(root)?;
    if root_metadata.file_type().is_symlink() {
        return Err(ProtocolError::TreeSymlink(root.display().to_string()));
    }
    if !root_metadata.is_dir() {
        return Err(ProtocolError::TreeEntryType(root.display().to_string()));
    }
    let mut entries = Vec::new();
    walk(root, root, &mut entries)?;
    entries.sort_by(|left, right| left.path.as_bytes().cmp(right.path.as_bytes()));

    let mut normalized = BTreeSet::new();
    let mut case_folded = BTreeMap::new();
    for entry in &entries {
        if !normalized.insert(entry.path.clone()) {
            return Err(ProtocolError::DuplicateTreePath(entry.path.clone()));
        }
        let folded = entry.path.to_ascii_lowercase();
        if let Some(previous) = case_folded.insert(folded, entry.path.clone()) {
            if previous != entry.path {
                return Err(ProtocolError::DuplicateTreePath(format!(
                    "{previous} collides with {} on an Apple filesystem",
                    entry.path
                )));
            }
        }
    }
    let digest = hash_entries(&entries)?;
    Ok(SourceTreeCommitment { digest, entries })
}

/// Exact tree stream:
///
/// `domain || u64be(file_count) || Σ(u32be(path_len) || path_utf8 || sha256(file))`
pub fn hash_entries(entries: &[SourceTreeEntry]) -> Result<Bytes32> {
    let mut sorted = entries.to_vec();
    sorted.sort_by(|left, right| left.path.as_bytes().cmp(right.path.as_bytes()));
    let mut case_folded = BTreeMap::new();
    for entry in &sorted {
        let folded = entry.path.to_ascii_lowercase();
        if let Some(previous) = case_folded.insert(folded, entry.path.clone()) {
            if previous != entry.path {
                return Err(ProtocolError::DuplicateTreePath(format!(
                    "{previous} collides with {} on an Apple filesystem",
                    entry.path
                )));
            }
        }
    }
    let count = u64::try_from(sorted.len())
        .map_err(|_| ProtocolError::InvalidTreePath("too many files".into()))?;
    let mut stream = Vec::new();
    stream.extend_from_slice(SOURCE_TREE_DOMAIN);
    stream.extend_from_slice(&count.to_be_bytes());
    let mut previous: Option<&str> = None;
    for entry in &sorted {
        validate_normalized_path(&entry.path)?;
        if previous == Some(&entry.path) {
            return Err(ProtocolError::DuplicateTreePath(entry.path.clone()));
        }
        previous = Some(&entry.path);
        let path = entry.path.as_bytes();
        let length = u32::try_from(path.len())
            .map_err(|_| ProtocolError::InvalidTreePath(entry.path.clone()))?;
        stream.extend_from_slice(&length.to_be_bytes());
        stream.extend_from_slice(path);
        stream.extend_from_slice(entry.content_sha256.as_bytes());
    }
    Ok(sha256(&stream))
}

pub fn exclusion_reason(normalized_path: &str) -> Option<&'static str> {
    if SELF_REFERENTIAL_EXCLUSIONS.contains(&normalized_path) {
        return Some("protocol sidecar excluded to prevent self-reference");
    }
    None
}

/// Paths that must not be silently omitted from the signed source world.
///
/// Generated build state and private signing material are rejected outright:
/// excluding them would let an Xcode target consume unsigned bytes, while
/// hashing them could disclose secrets through a public source commitment.
pub fn forbidden_source_reason(normalized_path: &str) -> Option<&'static str> {
    let components = normalized_path.split('/').collect::<Vec<_>>();
    if components
        .iter()
        .any(|component| EXCLUDED_DIRECTORY_COMPONENTS.contains(component))
    {
        return Some("build, VCS, or user-local directory is forbidden inside source");
    }
    let file_name = components.last().copied().unwrap_or_default();
    if PRIVATE_FILE_NAMES.contains(&file_name)
        || (file_name.starts_with(".env.") && file_name.len() > 5)
    {
        return Some("private, temporary, or log file is forbidden inside source");
    }
    if Path::new(file_name)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| PRIVATE_EXTENSIONS.contains(&extension))
    {
        return Some("signing, log, or user-local artifact is forbidden inside source");
    }
    None
}

/// Hashes a living working tree with the same rules as [`hash_source_tree`],
/// except entries that a sealed world *forbids* (VCS state, `.tohseno`,
/// Finder droppings, logs, user-local Xcode state) are silently skipped
/// instead of rejected. A clean sealed snapshot and the working tree it was
/// taken from therefore produce the same digest, which is what makes
/// "unsealed changes" a checkable fact. Sealed records must never use this;
/// they keep the strict [`hash_source_tree`].
pub fn hash_working_tree(root: &Path) -> Result<SourceTreeCommitment> {
    let root_metadata = metadata(root)?;
    if root_metadata.file_type().is_symlink() {
        return Err(ProtocolError::TreeSymlink(root.display().to_string()));
    }
    if !root_metadata.is_dir() {
        return Err(ProtocolError::TreeEntryType(root.display().to_string()));
    }
    let mut entries = Vec::new();
    walk_lenient(root, root, &mut entries)?;
    entries.sort_by(|left, right| left.path.as_bytes().cmp(right.path.as_bytes()));
    let digest = hash_entries(&entries)?;
    Ok(SourceTreeCommitment { digest, entries })
}

fn walk_lenient(root: &Path, directory: &Path, entries: &mut Vec<SourceTreeEntry>) -> Result<()> {
    let iterator = fs::read_dir(directory).map_err(|source| ProtocolError::TreeIo {
        path: directory.display().to_string(),
        source,
    })?;
    for item in iterator {
        let item = item.map_err(|source| ProtocolError::TreeIo {
            path: directory.display().to_string(),
            source,
        })?;
        let path = item.path();
        let relative = path
            .strip_prefix(root)
            .map_err(|_| ProtocolError::InvalidTreePath(path.display().to_string()))?;
        // Living folders carry names strict hashing cannot express
        // (Finder's "Icon\r", non-UTF-8); they are skipped, not fatal.
        let Ok(normalized) = normalize_relative_path(relative) else {
            continue;
        };
        // Skipped entries are skipped even when they are symlinks, so a
        // symlinked `.git` (worktrees) cannot block a working-tree hash.
        if exclusion_reason(&normalized).is_some() || forbidden_source_reason(&normalized).is_some()
        {
            continue;
        }
        let metadata = metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(ProtocolError::TreeSymlink(normalized));
        }
        if metadata.is_dir() {
            walk_lenient(root, &path, entries)?;
        } else if metadata.is_file() {
            entries.push(SourceTreeEntry {
                path: normalized,
                content_sha256: hash_regular_file(&path, &metadata)?,
            });
        }
        // Sockets, FIFOs, and other specials are skipped: the snapshot
        // never copies them, so they can never enter a sealed world.
    }
    Ok(())
}

fn walk(root: &Path, directory: &Path, entries: &mut Vec<SourceTreeEntry>) -> Result<()> {
    let iterator = fs::read_dir(directory).map_err(|source| ProtocolError::TreeIo {
        path: directory.display().to_string(),
        source,
    })?;
    for item in iterator {
        let item = item.map_err(|source| ProtocolError::TreeIo {
            path: directory.display().to_string(),
            source,
        })?;
        let path = item.path();
        let relative = path
            .strip_prefix(root)
            .map_err(|_| ProtocolError::InvalidTreePath(path.display().to_string()))?;
        let normalized = normalize_relative_path(relative)?;
        let metadata = metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(ProtocolError::TreeSymlink(normalized));
        }
        if exclusion_reason(&normalized).is_some() {
            continue;
        }
        if forbidden_source_reason(&normalized).is_some() {
            return Err(ProtocolError::TreeForbidden(normalized));
        }
        if metadata.is_dir() {
            walk(root, &path, entries)?;
        } else if metadata.is_file() {
            entries.push(SourceTreeEntry {
                path: normalized,
                content_sha256: hash_regular_file(&path, &metadata)?,
            });
        } else {
            return Err(ProtocolError::TreeEntryType(normalized));
        }
    }
    Ok(())
}

fn normalize_relative_path(relative: &Path) -> Result<String> {
    let mut components = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(value) => {
                let value = value.to_str().ok_or_else(|| {
                    ProtocolError::InvalidTreePath(relative.display().to_string())
                })?;
                let normalized = value.nfc().collect::<String>();
                if normalized.is_empty() || normalized == "." || normalized == ".." {
                    return Err(ProtocolError::InvalidTreePath(
                        relative.display().to_string(),
                    ));
                }
                components.push(normalized);
            }
            _ => {
                return Err(ProtocolError::InvalidTreePath(
                    relative.display().to_string(),
                ))
            }
        }
    }
    let path = components.join("/");
    validate_normalized_path(&path)?;
    Ok(path)
}

fn validate_normalized_path(path: &str) -> Result<()> {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path.chars().any(char::is_control)
        || path.split('/').any(|component| {
            component.is_empty()
                || component == "."
                || component == ".."
                || component.nfc().collect::<String>() != component
        })
    {
        return Err(ProtocolError::InvalidTreePath(path.into()));
    }
    Ok(())
}

fn hash_regular_file(path: &Path, initial: &Metadata) -> Result<Bytes32> {
    let mut file = File::open(path).map_err(|source| ProtocolError::TreeIo {
        path: path.display().to_string(),
        source,
    })?;
    let opened = file.metadata().map_err(|source| ProtocolError::TreeIo {
        path: path.display().to_string(),
        source,
    })?;
    if !opened.is_file() || !same_file(initial, &opened) {
        return Err(ProtocolError::TreeChanged(path.display().to_string()));
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|source| ProtocolError::TreeIo {
            path: path.display().to_string(),
            source,
        })?;
    let final_metadata = metadata(path)?;
    if final_metadata.file_type().is_symlink()
        || !same_file(initial, &final_metadata)
        || opened.len() != final_metadata.len()
        || bytes.len() as u64 != opened.len()
    {
        return Err(ProtocolError::TreeChanged(path.display().to_string()));
    }
    Ok(sha256(&bytes))
}

fn metadata(path: &Path) -> Result<Metadata> {
    fs::symlink_metadata(path).map_err(|source| ProtocolError::TreeIo {
        path: path.display().to_string(),
        source,
    })
}

#[cfg(unix)]
fn same_file(left: &Metadata, right: &Metadata) -> bool {
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
fn same_file(left: &Metadata, right: &Metadata) -> bool {
    left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
        && left.file_type() == right.file_type()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn sorting_is_input_order_independent_and_length_prefixed() {
        let a = SourceTreeEntry {
            path: "a".into(),
            content_sha256: sha256(b"a"),
        };
        let bc = SourceTreeEntry {
            path: "bc".into(),
            content_sha256: sha256(b"bc"),
        };
        assert_eq!(
            hash_entries(&[a.clone(), bc.clone()]).unwrap(),
            hash_entries(&[bc, a]).unwrap()
        );
    }

    #[test]
    fn protocol_sidecars_are_excluded_but_fascia_is_committed() {
        assert!(exclusion_reason("TOHSENO/shot.json").is_some());
        assert!(exclusion_reason("TOHSENO/signature.json").is_some());
        assert!(exclusion_reason("TOHSENO/conformance.json").is_some());
        assert!(exclusion_reason("TOHSENO/embedded-provenance.json").is_some());
        assert!(exclusion_reason("TOHSENO/fascia.json").is_none());
        assert!(exclusion_reason("TohsenoFascia/Provenance.swift").is_none());
        assert!(exclusion_reason("build/Evil.swift").is_none());
        assert!(forbidden_source_reason("build/Evil.swift").is_some());
        assert!(forbidden_source_reason("Secrets.p8").is_some());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_instead_of_following_them() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::NamedTempFile::new().unwrap();
        symlink(outside.path(), root.path().join("escape")).unwrap();
        assert!(matches!(
            hash_source_tree(root.path()),
            Err(ProtocolError::TreeSymlink(_))
        ));
    }

    #[test]
    fn hashes_raw_bytes_and_ignores_excluded_sidecars() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("TOHSENO")).unwrap();
        fs::write(root.path().join("App.swift"), b"\xffraw").unwrap();
        fs::write(root.path().join("TOHSENO/fascia.json"), b"{}").unwrap();
        fs::write(root.path().join("TOHSENO/signature.json"), b"one").unwrap();
        let first = hash_source_tree(root.path()).unwrap();
        fs::write(root.path().join("TOHSENO/signature.json"), b"two").unwrap();
        let second = hash_source_tree(root.path()).unwrap();
        assert_eq!(first.digest, second.digest);
        assert_eq!(first.entries.len(), 2);
    }

    #[test]
    fn rejects_generated_or_private_paths_instead_of_omitting_them() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("build")).unwrap();
        fs::write(root.path().join("build/Evil.swift"), b"target member").unwrap();
        assert!(matches!(
            hash_source_tree(root.path()),
            Err(ProtocolError::TreeForbidden(path)) if path == "build"
        ));
    }

    #[test]
    fn working_tree_hash_matches_its_clean_snapshot() {
        let working = tempfile::tempdir().unwrap();
        fs::write(working.path().join("App.swift"), b"struct App {}\n").unwrap();
        fs::create_dir(working.path().join("Assets")).unwrap();
        fs::write(working.path().join("Assets/a.json"), b"{}\n").unwrap();
        // Junk a living folder accumulates, forbidden inside a sealed world.
        fs::write(working.path().join(".DS_Store"), b"finder\n").unwrap();
        fs::create_dir(working.path().join(".git")).unwrap();
        fs::write(working.path().join(".git/HEAD"), b"ref\n").unwrap();
        fs::create_dir(working.path().join(".tohseno")).unwrap();
        fs::write(working.path().join(".tohseno/app.toml"), b"name\n").unwrap();

        let clean = tempfile::tempdir().unwrap();
        fs::write(clean.path().join("App.swift"), b"struct App {}\n").unwrap();
        fs::create_dir(clean.path().join("Assets")).unwrap();
        fs::write(clean.path().join("Assets/a.json"), b"{}\n").unwrap();

        assert!(hash_source_tree(working.path()).is_err());
        assert_eq!(
            hash_working_tree(working.path()).unwrap().digest,
            hash_source_tree(clean.path()).unwrap().digest
        );
    }
}
