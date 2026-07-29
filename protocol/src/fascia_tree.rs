use crate::digest::{sha256, Bytes32};
use crate::{ProtocolError, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, Metadata};
use std::io::Read;
use std::path::{Component, Path};
use unicode_normalization::UnicodeNormalization;

pub const DEFAULT_FASCIA_EXCLUSIONS: &[&str] = &[".build", ".swiftpm", "Package.resolved"];
/// Exact reusable Apple law accepted by the GENESIS 1.0.0-rc.1 candidate.
///
/// This is intentionally a source constant, not an environment-selected
/// value. Update it only alongside an explicit candidate Fascia revision.
pub const PINNED_APPLE_FASCIA_SHA256: &str =
    "0x75d80349643ec24537283f215cf3c793d3949a5cb7c6d13bfbefffcd8ea38e7f";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FasciaInputEntry {
    pub path: String,
    pub content: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FasciaTreeEntry {
    pub path: String,
    pub content_length: u64,
    pub content_sha256: Bytes32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FasciaTreeCommitment {
    pub digest: Bytes32,
    pub entries: Vec<FasciaTreeEntry>,
}

/// Hashes the reusable Apple Fascia reference tree with its frozen exclusions.
pub fn hash_fascia_tree(root: &Path) -> Result<FasciaTreeCommitment> {
    hash_fascia_tree_with_exclusions(root, DEFAULT_FASCIA_EXCLUSIONS)
}

/// Hashes a Fascia tree using anchored root-relative subtree exclusions.
///
/// Exclusion `E` matches only `P == E` or a path beginning with `E + "/"`.
/// A symlink at an encountered path is rejected before exclusion matching.
pub fn hash_fascia_tree_with_exclusions(
    root: &Path,
    exclusions: &[&str],
) -> Result<FasciaTreeCommitment> {
    validate_exclusions(exclusions)?;
    let root_metadata = metadata(root)?;
    if root_metadata.file_type().is_symlink() {
        return Err(ProtocolError::FasciaTreeSymlink(root.display().to_string()));
    }
    if !root_metadata.is_dir() {
        return Err(ProtocolError::FasciaTreeEntryType(
            root.display().to_string(),
        ));
    }
    let mut loaded = Vec::new();
    walk(root, root, exclusions, &mut loaded)?;
    let digest = hash_fascia_entries(&loaded)?;
    loaded.sort_by(|left, right| left.path.as_bytes().cmp(right.path.as_bytes()));
    let entries = loaded
        .into_iter()
        .map(|entry| FasciaTreeEntry {
            path: entry.path,
            content_length: entry.content.len() as u64,
            content_sha256: sha256(&entry.content),
        })
        .collect();
    Ok(FasciaTreeCommitment { digest, entries })
}

/// Exact Fascia stream:
///
/// `Σ(u64be(path_len) || path_utf8 || u64be(content_len) || raw_content)`
///
/// There is deliberately no domain prefix and no file count. The empty tree
/// commitment is SHA-256 of the empty byte string.
pub fn hash_fascia_entries(entries: &[FasciaInputEntry]) -> Result<Bytes32> {
    let mut sorted = entries.to_vec();
    sorted.sort_by(|left, right| left.path.as_bytes().cmp(right.path.as_bytes()));
    validate_unique_paths(&sorted)?;
    let mut stream = Vec::new();
    for entry in &sorted {
        validate_path(&entry.path)?;
        let path_len = u64::try_from(entry.path.len())
            .map_err(|_| ProtocolError::InvalidFasciaTreePath(entry.path.clone()))?;
        let content_len = u64::try_from(entry.content.len())
            .map_err(|_| ProtocolError::InvalidFasciaTreePath(entry.path.clone()))?;
        stream.extend_from_slice(&path_len.to_be_bytes());
        stream.extend_from_slice(entry.path.as_bytes());
        stream.extend_from_slice(&content_len.to_be_bytes());
        stream.extend_from_slice(&entry.content);
    }
    Ok(sha256(&stream))
}

fn walk(
    root: &Path,
    directory: &Path,
    exclusions: &[&str],
    entries: &mut Vec<FasciaInputEntry>,
) -> Result<()> {
    let iterator = fs::read_dir(directory).map_err(|source| ProtocolError::FasciaTreeIo {
        path: directory.display().to_string(),
        source,
    })?;
    for item in iterator {
        let item = item.map_err(|source| ProtocolError::FasciaTreeIo {
            path: directory.display().to_string(),
            source,
        })?;
        let path = item.path();
        let relative = path
            .strip_prefix(root)
            .map_err(|_| ProtocolError::InvalidFasciaTreePath(path.display().to_string()))?;
        let normalized = normalize_path(relative)?;
        let metadata = metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(ProtocolError::FasciaTreeSymlink(normalized));
        }
        if is_excluded(&normalized, exclusions) {
            continue;
        }
        if metadata.is_dir() {
            walk(root, &path, exclusions, entries)?;
        } else if metadata.is_file() {
            entries.push(FasciaInputEntry {
                path: normalized,
                content: read_regular_file(&path, &metadata)?,
            });
        } else {
            return Err(ProtocolError::FasciaTreeEntryType(normalized));
        }
    }
    Ok(())
}

fn is_excluded(path: &str, exclusions: &[&str]) -> bool {
    exclusions.iter().any(|excluded| {
        path == *excluded
            || path
                .strip_prefix(excluded)
                .is_some_and(|rest| rest.starts_with('/'))
    })
}

fn validate_exclusions(exclusions: &[&str]) -> Result<()> {
    let mut unique = BTreeSet::new();
    for exclusion in exclusions {
        validate_path(exclusion)?;
        if !unique.insert(*exclusion) {
            return Err(ProtocolError::InvalidFasciaTreePath(format!(
                "duplicate exclusion {exclusion}"
            )));
        }
    }
    Ok(())
}

fn validate_unique_paths(entries: &[FasciaInputEntry]) -> Result<()> {
    let mut exact = BTreeSet::new();
    let mut casefold = BTreeMap::new();
    for entry in entries {
        validate_path(&entry.path)?;
        if !exact.insert(&entry.path) {
            return Err(ProtocolError::DuplicateFasciaTreePath(entry.path.clone()));
        }
        let folded = entry.path.to_ascii_lowercase();
        if let Some(previous) = casefold.insert(folded, entry.path.clone()) {
            if previous != entry.path {
                return Err(ProtocolError::DuplicateFasciaTreePath(format!(
                    "{previous} collides with {} on an Apple filesystem",
                    entry.path
                )));
            }
        }
    }
    Ok(())
}

fn normalize_path(relative: &Path) -> Result<String> {
    let mut components = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(value) => {
                let value = value.to_str().ok_or_else(|| {
                    ProtocolError::InvalidFasciaTreePath(relative.display().to_string())
                })?;
                let normalized = value.nfc().collect::<String>();
                if normalized.is_empty() || normalized == "." || normalized == ".." {
                    return Err(ProtocolError::InvalidFasciaTreePath(
                        relative.display().to_string(),
                    ));
                }
                components.push(normalized);
            }
            _ => {
                return Err(ProtocolError::InvalidFasciaTreePath(
                    relative.display().to_string(),
                ))
            }
        }
    }
    let path = components.join("/");
    validate_path(&path)?;
    Ok(path)
}

fn validate_path(path: &str) -> Result<()> {
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
        return Err(ProtocolError::InvalidFasciaTreePath(path.into()));
    }
    Ok(())
}

fn read_regular_file(path: &Path, initial: &Metadata) -> Result<Vec<u8>> {
    let mut file = File::open(path).map_err(|source| ProtocolError::FasciaTreeIo {
        path: path.display().to_string(),
        source,
    })?;
    let opened = file
        .metadata()
        .map_err(|source| ProtocolError::FasciaTreeIo {
            path: path.display().to_string(),
            source,
        })?;
    if !opened.is_file() || !same_file(initial, &opened) {
        return Err(ProtocolError::FasciaTreeChanged(path.display().to_string()));
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|source| ProtocolError::FasciaTreeIo {
            path: path.display().to_string(),
            source,
        })?;
    let final_metadata = metadata(path)?;
    if final_metadata.file_type().is_symlink()
        || !same_file(initial, &final_metadata)
        || opened.len() != final_metadata.len()
        || bytes.len() as u64 != opened.len()
    {
        return Err(ProtocolError::FasciaTreeChanged(path.display().to_string()));
    }
    Ok(bytes)
}

fn metadata(path: &Path) -> Result<Metadata> {
    fs::symlink_metadata(path).map_err(|source| ProtocolError::FasciaTreeIo {
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

    #[test]
    fn empty_tree_is_sha256_empty_and_input_order_does_not_matter() {
        assert_eq!(hash_fascia_entries(&[]).unwrap(), sha256(b""));
        let a = FasciaInputEntry {
            path: "a".into(),
            content: b"one".to_vec(),
        };
        let b = FasciaInputEntry {
            path: "b".into(),
            content: b"two".to_vec(),
        };
        assert_eq!(
            hash_fascia_entries(&[a.clone(), b.clone()]).unwrap(),
            hash_fascia_entries(&[b, a]).unwrap()
        );
    }

    #[test]
    fn exclusions_are_anchored_subtrees() {
        assert!(is_excluded(".build", DEFAULT_FASCIA_EXCLUSIONS));
        assert!(is_excluded(".build/cache/value", DEFAULT_FASCIA_EXCLUSIONS));
        assert!(!is_excluded("nested/.buildish", DEFAULT_FASCIA_EXCLUSIONS));
        assert!(!is_excluded(
            "Package.resolved.backup",
            DEFAULT_FASCIA_EXCLUSIONS
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_an_encountered_symlink_before_exclusion() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        symlink("/tmp", root.path().join(".build")).unwrap();
        assert!(matches!(
            hash_fascia_tree(root.path()),
            Err(ProtocolError::FasciaTreeSymlink(_))
        ));
    }
}
