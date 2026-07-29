use crate::digest::{sha256, Bytes32};
use crate::text::invalid;
use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use unicode_normalization::UnicodeNormalization;

pub const GENESIS_INPUT_DOMAIN: &[u8] = b"TOHSENO-GENESIS-INPUT-V1\0";
pub const MAX_GENESIS_IMAGES: usize = 8;

/// One image entry in the Genesis-input commitment. The digest is SHA-256 of
/// the image's exact raw bytes; no decoding, metadata stripping, or image
/// normalization occurs.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenesisImage {
    pub filename: String,
    pub content_sha256: Bytes32,
}

pub fn genesis_image(filename: impl Into<String>, raw_bytes: &[u8]) -> Result<GenesisImage> {
    let image = GenesisImage {
        filename: filename.into(),
        content_sha256: sha256(raw_bytes),
    };
    validate_filename(&image.filename)?;
    Ok(image)
}

/// Hashes exact prompt bytes and pre-hashed image entries.
///
/// The stream is:
///
/// `domain || u64be(prompt_len) || prompt || u64be(image_count) ||
///  Σ(u32be(filename_len) || filename_utf8 || sha256(raw_image_bytes))`
///
/// Images are sorted by unsigned UTF-8 filename bytes. Filenames must already
/// be NFC-normalized simple names and duplicates are rejected.
pub fn genesis_input_sha256(prompt: &[u8], images: &[GenesisImage]) -> Result<Bytes32> {
    let prompt_len = u64::try_from(prompt.len())
        .map_err(|_| invalid("genesis.prompt", "is too large to encode"))?;
    if images.len() > MAX_GENESIS_IMAGES {
        return Err(invalid(
            "genesis.images",
            format!("must contain at most {MAX_GENESIS_IMAGES} images"),
        ));
    }
    let image_count = u64::try_from(images.len())
        .map_err(|_| invalid("genesis.images", "contains too many entries"))?;
    let mut sorted = images.to_vec();
    sorted.sort_by(|left, right| left.filename.as_bytes().cmp(right.filename.as_bytes()));

    let mut seen = BTreeSet::new();
    let mut stream = Vec::new();
    stream.extend_from_slice(GENESIS_INPUT_DOMAIN);
    stream.extend_from_slice(&prompt_len.to_be_bytes());
    stream.extend_from_slice(prompt);
    stream.extend_from_slice(&image_count.to_be_bytes());
    for image in &sorted {
        validate_filename(&image.filename)?;
        if !seen.insert(&image.filename) {
            return Err(invalid(
                "genesis.images",
                "contains a duplicate normalized filename",
            ));
        }
        let filename = image.filename.as_bytes();
        let filename_len = u32::try_from(filename.len())
            .map_err(|_| invalid("genesis.image.filename", "is too large to encode"))?;
        stream.extend_from_slice(&filename_len.to_be_bytes());
        stream.extend_from_slice(filename);
        stream.extend_from_slice(image.content_sha256.as_bytes());
    }
    Ok(sha256(&stream))
}

/// Convenience entry point for callers holding raw image bytes.
pub fn genesis_input_sha256_from_bytes(prompt: &[u8], images: &[(&str, &[u8])]) -> Result<Bytes32> {
    let images = images
        .iter()
        .map(|(filename, bytes)| genesis_image(*filename, bytes))
        .collect::<Result<Vec<_>>>()?;
    genesis_input_sha256(prompt, &images)
}

fn validate_filename(filename: &str) -> Result<()> {
    if filename.is_empty()
        || filename == "."
        || filename == ".."
        || filename.contains('/')
        || filename.contains('\\')
        || filename.chars().any(char::is_control)
        || filename.nfc().collect::<String>() != filename
    {
        return Err(invalid(
            "genesis.image.filename",
            "must be a nonempty NFC-normalized filename without paths or control characters",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector_commits_raw_prompt_image_names_and_raw_image_bytes() {
        let prompt = b"Build a quiet field notebook.\n";
        let a = genesis_image("cover.png", b"\x89PNG\r\nraw-a").unwrap();
        let b = genesis_image("reference.jpg", b"\xff\xd8raw-b\xff\xd9").unwrap();
        let digest = genesis_input_sha256(prompt, &[b.clone(), a.clone()]).unwrap();
        assert_eq!(
            digest,
            Bytes32::from_hex(
                "genesis_input_sha256",
                "0xace2418f77d2ed4184a2f141e902ce349c438949c94ac3544495f31993ed1848",
            )
            .unwrap()
        );
        assert_eq!(
            digest,
            genesis_input_sha256_from_bytes(
                prompt,
                &[
                    ("reference.jpg", b"\xff\xd8raw-b\xff\xd9"),
                    ("cover.png", b"\x89PNG\r\nraw-a"),
                ],
            )
            .unwrap()
        );
    }

    #[test]
    fn rejects_duplicates_paths_and_non_nfc_names() {
        let image = genesis_image("same.png", b"a").unwrap();
        assert!(genesis_input_sha256(b"x", &[image.clone(), image]).is_err());
        assert!(genesis_image("../escape.png", b"x").is_err());
        assert!(genesis_image("e\u{301}.png", b"x").is_err());
    }
}
