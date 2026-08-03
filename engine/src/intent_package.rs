//! Private, noncanonical browser-to-local intention transport package.
//!
//! This framing is deliberately outside the TOHSENO protocol. Parsing ends in
//! the engine's existing private-reference validation and never creates a Shot.

use crate::shot_layout::validate_private_reference_bytes;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use tohseno_protocol::digest::sha256;

pub const INTENT_PACKAGE_SCHEMA: &str = "tohseno.intent-package/1";
pub const INTENT_PACKAGE_MAGIC: &[u8; 8] = b"TOHSINT1";
pub const MAX_WEB_PROMPT_BYTES: usize = 1024 * 1024;
pub const MAX_WEB_REFERENCE_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_WEB_REFERENCE_TOTAL_BYTES: usize = 48 * 1024 * 1024;
pub const MAX_INTENT_PACKAGE_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_INTENT_REFERENCES: usize = 8;
const HEADER_BYTES: usize = 12;
const MAX_MANIFEST_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntentPackage {
    pub created_at: String,
    pub prompt: String,
    pub references: Vec<IntentPackageReference>,
    pub package_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntentPackageReference {
    pub ordinal: usize,
    pub display_filename: String,
    pub media_type: String,
    pub sha256: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: String,
    created_at: String,
    prompt: PromptManifest,
    references: Vec<ReferenceManifest>,
    limits: LimitsManifest,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PromptManifest {
    encoding: String,
    byte_length: u64,
    sha256: String,
    text: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReferenceManifest {
    ordinal: u64,
    display_filename: String,
    media_type: String,
    byte_length: u64,
    sha256: String,
    payload_offset: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LimitsManifest {
    version: u64,
    max_prompt_bytes: u64,
    max_references: u64,
    max_reference_bytes: u64,
    max_total_reference_bytes: u64,
    max_package_bytes: u64,
}

#[derive(Debug)]
pub enum IntentPackageError {
    Invalid(String),
    Json(serde_json::Error),
}

impl std::fmt::Display for IntentPackageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(reason) => formatter.write_str(reason),
            Self::Json(error) => write!(formatter, "intent package manifest is invalid: {error}"),
        }
    }
}

impl std::error::Error for IntentPackageError {}

impl From<serde_json::Error> for IntentPackageError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

fn invalid(reason: impl Into<String>) -> IntentPackageError {
    IntentPackageError::Invalid(reason.into())
}

fn digest_hex(bytes: &[u8]) -> String {
    sha256(bytes).to_hex().trim_start_matches("0x").to_owned()
}

fn validate_digest(value: &str) -> Result<(), IntentPackageError> {
    if value.len() != 64
        || value
            .as_bytes()
            .iter()
            .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(byte))
    {
        return Err(invalid(
            "intent package contains a malformed SHA-256 digest",
        ));
    }
    Ok(())
}

fn expected_limits() -> LimitsManifest {
    LimitsManifest {
        version: 1,
        max_prompt_bytes: MAX_WEB_PROMPT_BYTES as u64,
        max_references: MAX_INTENT_REFERENCES as u64,
        max_reference_bytes: MAX_WEB_REFERENCE_BYTES as u64,
        max_total_reference_bytes: MAX_WEB_REFERENCE_TOTAL_BYTES as u64,
        max_package_bytes: MAX_INTENT_PACKAGE_BYTES as u64,
    }
}

pub fn build_intent_package(
    created_at: &str,
    prompt: &str,
    references: &[(String, String, Vec<u8>)],
) -> Result<Vec<u8>, IntentPackageError> {
    validate_input(prompt, references)?;
    let mut payload_offset = 0_u64;
    let mut manifest_references = Vec::with_capacity(references.len());
    for (ordinal, (display_filename, media_type, bytes)) in references.iter().enumerate() {
        manifest_references.push(ReferenceManifest {
            ordinal: ordinal as u64,
            display_filename: display_filename.clone(),
            media_type: media_type.clone(),
            byte_length: bytes.len() as u64,
            sha256: digest_hex(bytes),
            payload_offset,
        });
        payload_offset = payload_offset
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| invalid("intent package payload length overflowed"))?;
    }
    let prompt_bytes = prompt.as_bytes();
    let manifest = Manifest {
        schema: INTENT_PACKAGE_SCHEMA.into(),
        created_at: created_at.into(),
        prompt: PromptManifest {
            encoding: "utf-8".into(),
            byte_length: prompt_bytes.len() as u64,
            sha256: digest_hex(prompt_bytes),
            text: prompt.into(),
        },
        references: manifest_references,
        limits: expected_limits(),
    };
    let manifest_bytes = serde_json::to_vec(&manifest)?;
    if manifest_bytes.len() > MAX_MANIFEST_BYTES || manifest_bytes.len() > u32::MAX as usize {
        return Err(invalid("intent package manifest is too large"));
    }
    let total = HEADER_BYTES
        .checked_add(manifest_bytes.len())
        .and_then(|value| value.checked_add(payload_offset as usize))
        .ok_or_else(|| invalid("intent package length overflowed"))?;
    if total > MAX_INTENT_PACKAGE_BYTES {
        return Err(invalid("intent package exceeds 64 MiB"));
    }
    let mut package = Vec::with_capacity(total);
    package.extend_from_slice(INTENT_PACKAGE_MAGIC);
    package.extend_from_slice(&(manifest_bytes.len() as u32).to_be_bytes());
    package.extend_from_slice(&manifest_bytes);
    for (_, _, bytes) in references {
        package.extend_from_slice(bytes);
    }
    Ok(package)
}

pub fn parse_intent_package(bytes: &[u8]) -> Result<IntentPackage, IntentPackageError> {
    if bytes.len() > MAX_INTENT_PACKAGE_BYTES {
        return Err(invalid("intent package exceeds 64 MiB"));
    }
    if bytes.len() < HEADER_BYTES || &bytes[..8] != INTENT_PACKAGE_MAGIC {
        return Err(invalid("intent package magic or version is unsupported"));
    }
    let manifest_length = u32::from_be_bytes(
        bytes[8..12]
            .try_into()
            .expect("fixed-width manifest length"),
    ) as usize;
    if manifest_length == 0 || manifest_length > MAX_MANIFEST_BYTES {
        return Err(invalid("intent package manifest length is invalid"));
    }
    let payload_start = HEADER_BYTES
        .checked_add(manifest_length)
        .ok_or_else(|| invalid("intent package manifest length overflowed"))?;
    if payload_start > bytes.len() {
        return Err(invalid("intent package is truncated before its payload"));
    }
    let manifest: Manifest = serde_json::from_slice(&bytes[HEADER_BYTES..payload_start])?;
    if manifest.schema != INTENT_PACKAGE_SCHEMA || manifest.limits != expected_limits() {
        return Err(invalid(
            "intent package schema, version, or limits are unsupported",
        ));
    }
    let prompt_bytes = manifest.prompt.text.as_bytes();
    if manifest.prompt.encoding != "utf-8"
        || prompt_bytes.len() as u64 != manifest.prompt.byte_length
        || manifest.prompt.byte_length as usize > MAX_WEB_PROMPT_BYTES
        || manifest.prompt.text.trim().is_empty()
    {
        return Err(invalid("intent package prompt is empty or malformed"));
    }
    validate_digest(&manifest.prompt.sha256)?;
    if digest_hex(prompt_bytes) != manifest.prompt.sha256 {
        return Err(invalid("intent package prompt digest does not match"));
    }
    if manifest.references.len() > MAX_INTENT_REFERENCES {
        return Err(invalid(
            "intent package contains more than eight references",
        ));
    }
    let payload = &bytes[payload_start..];
    let mut expected_offset = 0_usize;
    let mut total_reference_bytes = 0_usize;
    let mut digests = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut references = Vec::with_capacity(manifest.references.len());
    for (index, reference) in manifest.references.into_iter().enumerate() {
        if reference.ordinal != index as u64 || reference.payload_offset as usize != expected_offset
        {
            return Err(invalid(
                "intent package reference offsets contain a gap, overlap, or invalid order",
            ));
        }
        let length = usize::try_from(reference.byte_length)
            .map_err(|_| invalid("intent package reference length overflowed"))?;
        if length > MAX_WEB_REFERENCE_BYTES {
            return Err(invalid("intent package reference exceeds 16 MiB"));
        }
        let end = expected_offset
            .checked_add(length)
            .ok_or_else(|| invalid("intent package reference length overflowed"))?;
        if end > payload.len() {
            return Err(invalid("intent package reference payload is truncated"));
        }
        total_reference_bytes = total_reference_bytes
            .checked_add(length)
            .ok_or_else(|| invalid("intent package reference total overflowed"))?;
        if total_reference_bytes > MAX_WEB_REFERENCE_TOTAL_BYTES {
            return Err(invalid("intent package references exceed 48 MiB"));
        }
        let reference_bytes = &payload[expected_offset..end];
        validate_digest(&reference.sha256)?;
        if digest_hex(reference_bytes) != reference.sha256 {
            return Err(invalid("intent package reference digest does not match"));
        }
        if !digests.insert(reference.sha256.clone()) {
            return Err(invalid("intent package references must not repeat content"));
        }
        if !names.insert(reference.display_filename.to_ascii_lowercase()) {
            return Err(invalid(
                "intent package reference names collide on Apple filesystems",
            ));
        }
        validate_private_reference_bytes(
            &reference.display_filename,
            &reference.media_type,
            reference_bytes,
        )
        .map_err(|error| invalid(error.to_string()))?;
        references.push(IntentPackageReference {
            ordinal: index,
            display_filename: reference.display_filename,
            media_type: reference.media_type,
            sha256: reference.sha256,
            bytes: reference_bytes.to_vec(),
        });
        expected_offset = end;
    }
    if expected_offset != payload.len() {
        return Err(invalid(
            "intent package contains trailing unaccounted bytes",
        ));
    }
    Ok(IntentPackage {
        created_at: manifest.created_at,
        prompt: manifest.prompt.text,
        references,
        package_sha256: digest_hex(bytes),
    })
}

fn validate_input(
    prompt: &str,
    references: &[(String, String, Vec<u8>)],
) -> Result<(), IntentPackageError> {
    if prompt.trim().is_empty() || prompt.len() > MAX_WEB_PROMPT_BYTES {
        return Err(invalid(
            "intent package prompt must be nonempty and no larger than 1 MiB",
        ));
    }
    if references.len() > MAX_INTENT_REFERENCES {
        return Err(invalid(
            "intent package contains more than eight references",
        ));
    }
    let mut total = 0_usize;
    let mut digests = BTreeSet::new();
    let mut names = BTreeSet::new();
    for (name, media_type, bytes) in references {
        if bytes.len() > MAX_WEB_REFERENCE_BYTES {
            return Err(invalid("intent package reference exceeds 16 MiB"));
        }
        total = total
            .checked_add(bytes.len())
            .ok_or_else(|| invalid("intent package reference total overflowed"))?;
        if total > MAX_WEB_REFERENCE_TOTAL_BYTES {
            return Err(invalid("intent package references exceed 48 MiB"));
        }
        validate_private_reference_bytes(name, media_type, bytes)
            .map_err(|error| invalid(error.to_string()))?;
        let digest = digest_hex(bytes);
        if !digests.insert(digest) {
            return Err(invalid("intent package references must not repeat content"));
        }
        if !names.insert(name.to_ascii_lowercase()) {
            return Err(invalid(
                "intent package reference names collide on Apple filesystems",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64ct::{Base64, Encoding};

    fn rewrite_manifest(package: &[u8], change: impl FnOnce(&mut serde_json::Value)) -> Vec<u8> {
        let manifest_length = u32::from_be_bytes(package[8..12].try_into().unwrap()) as usize;
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&package[12..12 + manifest_length]).unwrap();
        change(&mut manifest);
        let encoded = serde_json::to_vec(&manifest).unwrap();
        let mut rewritten = INTENT_PACKAGE_MAGIC.to_vec();
        rewritten.extend_from_slice(&(encoded.len() as u32).to_be_bytes());
        rewritten.extend_from_slice(&encoded);
        rewritten.extend_from_slice(&package[12 + manifest_length..]);
        rewritten
    }

    fn png(label: u8) -> Vec<u8> {
        [b"\x89PNG\r\n\x1a\n".as_slice(), &[label]].concat()
    }

    #[test]
    fn round_trip_preserves_utf8_prompt_and_ordered_references() {
        let references = vec![
            ("one.png".into(), "image/png".into(), png(1)),
            (
                "two.jpeg".into(),
                "image/jpeg".into(),
                b"\xff\xd8\xfftwo".to_vec(),
            ),
        ];
        let bytes = build_intent_package("2026-08-03T00:00:00Z", "A tree 🌲", &references).unwrap();
        let parsed = parse_intent_package(&bytes).unwrap();
        assert_eq!(parsed.prompt, "A tree 🌲");
        assert_eq!(parsed.references[0].display_filename, "one.png");
        assert_eq!(parsed.references[1].bytes, b"\xff\xd8\xfftwo");
    }

    #[test]
    fn rejects_truncation_trailing_bytes_and_digest_changes() {
        let package = build_intent_package(
            "2026-08-03T00:00:00Z",
            "Make it.",
            &[("one.png".into(), "image/png".into(), png(1))],
        )
        .unwrap();
        assert!(parse_intent_package(&package[..package.len() - 1]).is_err());
        let mut trailing = package.clone();
        trailing.push(0);
        assert!(parse_intent_package(&trailing).is_err());
        let mut corrupted = package;
        *corrupted.last_mut().unwrap() ^= 1;
        assert!(parse_intent_package(&corrupted).is_err());
    }

    #[test]
    fn rejects_invalid_magic_empty_prompt_duplicates_and_unsafe_names() {
        let mut package = build_intent_package("now", "Make it.", &[]).unwrap();
        package[0] ^= 1;
        assert!(parse_intent_package(&package).is_err());
        assert!(build_intent_package("now", "  ", &[]).is_err());
        let bytes = png(1);
        assert!(build_intent_package(
            "now",
            "Make it.",
            &[
                ("one.png".into(), "image/png".into(), bytes.clone()),
                ("two.png".into(), "image/png".into(), bytes),
            ],
        )
        .is_err());
        assert!(build_intent_package(
            "now",
            "Make it.",
            &[("../one.png".into(), "image/png".into(), png(1))],
        )
        .is_err());
    }

    #[test]
    fn rejects_unknown_versions_offsets_signatures_and_extension_disagreement() {
        let package = build_intent_package(
            "2026-08-03T00:00:00Z",
            "Make it.",
            &[("one.png".into(), "image/png".into(), png(1))],
        )
        .unwrap();
        let version = rewrite_manifest(&package, |manifest| {
            manifest["schema"] = "tohseno.intent-package/2".into();
        });
        assert!(parse_intent_package(&version).is_err());
        let gap = rewrite_manifest(&package, |manifest| {
            manifest["references"][0]["payload_offset"] = 1.into();
        });
        assert!(parse_intent_package(&gap).is_err());
        assert!(build_intent_package(
            "now",
            "Make it.",
            &[("one.png".into(), "image/png".into(), b"not-png".to_vec())],
        )
        .is_err());
        assert!(build_intent_package(
            "now",
            "Make it.",
            &[("one.jpeg".into(), "image/png".into(), png(1))],
        )
        .is_err());
        assert!(build_intent_package(
            "now",
            "Make it.",
            &(0..9)
                .map(|index| (format!("{index}.png"), "image/png".into(), png(index)))
                .collect::<Vec<_>>(),
        )
        .is_err());
    }

    #[test]
    fn imports_the_checked_in_browser_compatibility_vector() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../fixtures/intent-package-v1.json")).unwrap();
        let bytes = Base64::decode_vec(fixture["package_base64"].as_str().unwrap()).unwrap();
        let parsed = parse_intent_package(&bytes).unwrap();
        assert_eq!(parsed.prompt, fixture["prompt"].as_str().unwrap());
        assert_eq!(
            parsed.package_sha256,
            fixture["package_sha256"].as_str().unwrap()
        );
        assert_eq!(parsed.references.len(), 2);
        assert_eq!(parsed.references[0].display_filename, "tree.png");
        assert_eq!(parsed.references[1].display_filename, "detail.jpeg");
    }
}
