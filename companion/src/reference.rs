//! Exact phone-to-Mac reference bytes and their bounded transport chunks.
//!
//! A logical reference blob preserves the owner's exact image bytes. Transport
//! always carries deterministic chunks so the content-blind relay can retain
//! large engine-valid references without weakening the 16 MiB envelope bound.
//! Every chunk is authenticated by its companion envelope; the whole-object
//! commitment is verified again after reassembly on the Mac.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::canonical;
use crate::command::{CompanionCommand, ReferenceDescriptor, MAX_REFERENCES};
use crate::crypto::{base64url, decode_base64url, sha256};
use crate::{require, validate_identifier, validate_text, CompanionError, Result};

pub const REFERENCE_BLOB_SCHEMA: &str = "tohseno.companion-reference-blob/1";
pub const REFERENCE_BLOB_CHUNK_SCHEMA: &str = "tohseno.companion-reference-blob-chunk/1";
pub const MAX_REFERENCE_BLOB_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_REFERENCE_CHUNK_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_REFERENCE_CHUNKS: usize =
    MAX_REFERENCES * MAX_REFERENCE_BLOB_BYTES.div_ceil(MAX_REFERENCE_CHUNK_BYTES);

/// A strict logical reference object. This exact shape is also useful at SDK
/// boundaries and in shared vectors; relay transport uses `ReferenceBlobChunk`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceBlob {
    pub schema: String,
    pub blob_id: String,
    pub origin_name: String,
    pub media_type: String,
    pub byte_length: u64,
    pub sha256: String,
    pub bytes: String,
}

impl ReferenceBlob {
    pub fn new(
        blob_id: impl Into<String>,
        origin_name: impl Into<String>,
        media_type: impl Into<String>,
        bytes: &[u8],
    ) -> Result<Self> {
        let value = Self {
            schema: REFERENCE_BLOB_SCHEMA.into(),
            blob_id: blob_id.into(),
            origin_name: origin_name.into(),
            media_type: media_type.into(),
            byte_length: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            sha256: base64url(&sha256(bytes)),
            bytes: base64url(bytes),
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<()> {
        require(
            self.schema == REFERENCE_BLOB_SCHEMA,
            "unsupported companion reference blob schema",
        )?;
        validate_reference_metadata(
            &self.blob_id,
            &self.origin_name,
            &self.media_type,
            self.byte_length,
            &self.sha256,
        )?;
        let bytes = self.decoded_bytes()?;
        require(
            u64::try_from(bytes.len()).ok() == Some(self.byte_length),
            "reference blob byte length does not match its bytes",
        )?;
        require(
            self.sha256 == base64url(&sha256(&bytes)),
            "reference blob SHA-256 does not match its bytes",
        )?;
        validate_image(&self.media_type, &bytes)
    }

    pub fn decoded_bytes(&self) -> Result<Vec<u8>> {
        decode_base64url(
            "reference blob bytes",
            &self.bytes,
            MAX_REFERENCE_BLOB_BYTES,
        )
    }

    pub fn descriptor(&self) -> Result<ReferenceDescriptor> {
        self.validate()?;
        Ok(ReferenceDescriptor {
            blob_id: self.blob_id.clone(),
            origin_name: self.origin_name.clone(),
            media_type: self.media_type.clone(),
            byte_length: self.byte_length,
            sha256: self.sha256.clone(),
        })
    }

    pub fn matches_descriptor(&self, descriptor: &ReferenceDescriptor) -> Result<()> {
        self.validate()?;
        require(
            self.descriptor()? == *descriptor,
            "reference blob does not match its command descriptor",
        )
    }

    pub fn chunks(&self) -> Result<Vec<ReferenceBlobChunk>> {
        self.validate()?;
        let bytes = self.decoded_bytes()?;
        let chunk_count = bytes.len().div_ceil(MAX_REFERENCE_CHUNK_BYTES);
        require(
            (1..=MAX_REFERENCE_BLOB_BYTES.div_ceil(MAX_REFERENCE_CHUNK_BYTES))
                .contains(&chunk_count),
            "reference blob chunk count is invalid",
        )?;
        bytes
            .chunks(MAX_REFERENCE_CHUNK_BYTES)
            .enumerate()
            .map(|(index, chunk)| {
                ReferenceBlobChunk::new(self, index as u64, chunk_count as u64, chunk)
            })
            .collect()
    }
}

/// One deterministic transport unit for a logical `ReferenceBlob`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceBlobChunk {
    pub schema: String,
    pub blob_id: String,
    pub origin_name: String,
    pub media_type: String,
    pub byte_length: u64,
    pub sha256: String,
    pub chunk_index: u64,
    pub chunk_count: u64,
    pub chunk_byte_length: u64,
    pub chunk_sha256: String,
    pub bytes: String,
}

impl ReferenceBlobChunk {
    fn new(blob: &ReferenceBlob, chunk_index: u64, chunk_count: u64, bytes: &[u8]) -> Result<Self> {
        let value = Self {
            schema: REFERENCE_BLOB_CHUNK_SCHEMA.into(),
            blob_id: blob.blob_id.clone(),
            origin_name: blob.origin_name.clone(),
            media_type: blob.media_type.clone(),
            byte_length: blob.byte_length,
            sha256: blob.sha256.clone(),
            chunk_index,
            chunk_count,
            chunk_byte_length: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            chunk_sha256: base64url(&sha256(bytes)),
            bytes: base64url(bytes),
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<()> {
        require(
            self.schema == REFERENCE_BLOB_CHUNK_SCHEMA,
            "unsupported companion reference blob chunk schema",
        )?;
        validate_reference_metadata(
            &self.blob_id,
            &self.origin_name,
            &self.media_type,
            self.byte_length,
            &self.sha256,
        )?;
        let expected_count = usize::try_from(self.byte_length)
            .unwrap_or(usize::MAX)
            .div_ceil(MAX_REFERENCE_CHUNK_BYTES);
        require(
            self.chunk_count == expected_count as u64
                && (1..=MAX_REFERENCE_BLOB_BYTES.div_ceil(MAX_REFERENCE_CHUNK_BYTES) as u64)
                    .contains(&self.chunk_count)
                && self.chunk_index < self.chunk_count,
            "reference chunk position is invalid",
        )?;
        let expected_length = if self.chunk_index + 1 == self.chunk_count {
            let remainder =
                usize::try_from(self.byte_length).unwrap_or(usize::MAX) % MAX_REFERENCE_CHUNK_BYTES;
            if remainder == 0 {
                MAX_REFERENCE_CHUNK_BYTES
            } else {
                remainder
            }
        } else {
            MAX_REFERENCE_CHUNK_BYTES
        };
        require(
            self.chunk_byte_length == expected_length as u64,
            "reference chunk length is not canonical",
        )?;
        let bytes = self.decoded_bytes()?;
        require(
            bytes.len() == expected_length,
            "reference chunk byte length does not match its bytes",
        )?;
        require(
            self.chunk_sha256 == base64url(&sha256(&bytes)),
            "reference chunk SHA-256 does not match its bytes",
        )
    }

    pub fn decoded_bytes(&self) -> Result<Vec<u8>> {
        decode_base64url(
            "reference chunk bytes",
            &self.bytes,
            MAX_REFERENCE_CHUNK_BYTES,
        )
    }

    pub fn descriptor(&self) -> Result<ReferenceDescriptor> {
        self.validate()?;
        Ok(ReferenceDescriptor {
            blob_id: self.blob_id.clone(),
            origin_name: self.origin_name.clone(),
            media_type: self.media_type.clone(),
            byte_length: self.byte_length,
            sha256: self.sha256.clone(),
        })
    }
}

/// The two accepted plaintext shapes for the phone-to-Mac mailbox. Serde's
/// untagged representation deliberately leaves existing command bytes exactly
/// unchanged; the closed top-level `schema` selects the branch.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PhoneToMacPayload {
    Command(CompanionCommand),
    ReferenceBlobChunk(ReferenceBlobChunk),
}

impl PhoneToMacPayload {
    pub fn from_canonical_slice(bytes: &[u8]) -> Result<Self> {
        let value: Self = canonical::from_slice(bytes)?;
        match &value {
            Self::Command(command) => {
                require(
                    command.body.schema == crate::command::COMPANION_COMMAND_SCHEMA,
                    "unsupported phone-to-Mac command schema",
                )?;
            }
            Self::ReferenceBlobChunk(chunk) => chunk.validate()?,
        }
        Ok(value)
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        canonical::to_vec(self)
    }
}

/// In-memory conformance assembler. It is serializable so the Local Workspace
/// Service can durably checkpoint the same deterministic state while storing
/// chunk bytes as separately bounded private files.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceBlobAssembler {
    chunks: BTreeMap<u64, ReferenceBlobChunk>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChunkAdmission {
    Stored,
    Duplicate,
    Complete(ReferenceBlob),
}

impl ReferenceBlobAssembler {
    pub fn admit(&mut self, chunk: ReferenceBlobChunk) -> Result<ChunkAdmission> {
        chunk.validate()?;
        if let Some(first) = self.chunks.values().next() {
            require(
                chunk.blob_id == first.blob_id
                    && chunk.origin_name == first.origin_name
                    && chunk.media_type == first.media_type
                    && chunk.byte_length == first.byte_length
                    && chunk.sha256 == first.sha256
                    && chunk.chunk_count == first.chunk_count,
                "reference chunks do not describe one exact blob",
            )?;
        }
        if let Some(existing) = self.chunks.get(&chunk.chunk_index) {
            return if existing == &chunk {
                Ok(ChunkAdmission::Duplicate)
            } else {
                Err(CompanionError::Invalid(
                    "reference chunk position was reused with different bytes".into(),
                ))
            };
        }
        let chunk_count = chunk.chunk_count;
        self.chunks.insert(chunk.chunk_index, chunk);
        if self.chunks.len() != chunk_count as usize {
            return Ok(ChunkAdmission::Stored);
        }
        let first = self
            .chunks
            .values()
            .next()
            .ok_or_else(|| CompanionError::Invalid("reference assembler is empty".into()))?;
        let mut bytes = Vec::with_capacity(first.byte_length as usize);
        for index in 0..chunk_count {
            let value = self.chunks.get(&index).ok_or_else(|| {
                CompanionError::Invalid("reference assembler has a missing chunk".into())
            })?;
            bytes.extend_from_slice(&value.decoded_bytes()?);
        }
        let blob = ReferenceBlob::new(
            first.blob_id.clone(),
            first.origin_name.clone(),
            first.media_type.clone(),
            &bytes,
        )?;
        require(
            blob.sha256 == first.sha256 && blob.byte_length == first.byte_length,
            "reassembled reference does not match its whole-object commitment",
        )?;
        Ok(ChunkAdmission::Complete(blob))
    }
}

fn validate_reference_metadata(
    blob_id: &str,
    origin_name: &str,
    media_type: &str,
    byte_length: u64,
    digest: &str,
) -> Result<()> {
    validate_identifier("reference blob ID", blob_id)?;
    validate_text("reference origin name", origin_name, 512)?;
    require(
        !origin_name.contains(['/', '\\', '\0']) && origin_name != "." && origin_name != "..",
        "reference origin name must be a plain filename",
    )?;
    require(
        matches!(media_type, "image/png" | "image/jpeg"),
        "reference media type must be image/png or image/jpeg",
    )?;
    require(
        (1..=MAX_REFERENCE_BLOB_BYTES as u64).contains(&byte_length),
        "reference blob byte length is invalid",
    )?;
    crate::crypto::decode_array::<32>("reference SHA-256", digest)?;
    Ok(())
}

fn validate_image(media_type: &str, bytes: &[u8]) -> Result<()> {
    let valid = match media_type {
        "image/png" => valid_png_header(bytes),
        "image/jpeg" => valid_jpeg_header(bytes),
        _ => false,
    };
    require(valid, "reference bytes are not the declared image type")
}

fn valid_png_header(bytes: &[u8]) -> bool {
    const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    bytes.len() >= 24
        && &bytes[..8] == SIGNATURE
        && u32::from_be_bytes(bytes[8..12].try_into().unwrap_or_default()) == 13
        && &bytes[12..16] == b"IHDR"
        && u32::from_be_bytes(bytes[16..20].try_into().unwrap_or_default()) > 0
        && u32::from_be_bytes(bytes[20..24].try_into().unwrap_or_default()) > 0
}

fn valid_jpeg_header(bytes: &[u8]) -> bool {
    if bytes.len() < 4 || bytes[..2] != [0xff, 0xd8] {
        return false;
    }
    let mut offset = 2_usize;
    while offset < bytes.len() {
        if bytes[offset] != 0xff {
            return false;
        }
        while offset < bytes.len() && bytes[offset] == 0xff {
            offset += 1;
        }
        let Some(&marker) = bytes.get(offset) else {
            return false;
        };
        offset += 1;
        if marker == 0 || marker == 0xd9 || marker == 0xda {
            return false;
        }
        if marker == 0x01 || (0xd0..=0xd8).contains(&marker) {
            continue;
        }
        let (Some(&high), Some(&low)) = (bytes.get(offset), bytes.get(offset + 1)) else {
            return false;
        };
        let length = usize::from(u16::from_be_bytes([high, low]));
        if length < 2 || offset.saturating_add(length) > bytes.len() {
            return false;
        }
        if matches!(
            marker,
            0xc0 | 0xc1
                | 0xc2
                | 0xc3
                | 0xc5
                | 0xc6
                | 0xc7
                | 0xc9
                | 0xca
                | 0xcb
                | 0xcd
                | 0xce
                | 0xcf
        ) {
            let start = offset + 2;
            if length < 7 || start + 4 >= bytes.len() {
                return false;
            }
            let height = u16::from_be_bytes([bytes[start + 1], bytes[start + 2]]);
            let width = u16::from_be_bytes([bytes[start + 3], bytes[start + 4]]);
            return width > 0 && height > 0;
        }
        offset += length;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0x0d, 0x49, 0x48, 0x44, 0x52, 0,
        0, 0, 1, 0, 0, 0, 1, 8, 4, 0, 0, 0, 0xb5, 0x1c, 0x0c, 2, 0, 0, 0, 0x0b, 0x49, 0x44, 0x41,
        0x54, 0x78, 0xda, 0x63, 0x64, 0xf8, 0x0f, 0, 1, 5, 1, 1, 0x27, 0x18, 0xe3, 0x66, 0, 0, 0,
        0, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];

    #[test]
    fn strict_blob_descriptor_and_single_chunk_round_trip() {
        let blob =
            ReferenceBlob::new("reference_fixture", "reference.png", "image/png", PNG).unwrap();
        assert_eq!(blob.descriptor().unwrap().byte_length, PNG.len() as u64);
        let chunks = blob.chunks().unwrap();
        assert_eq!(chunks.len(), 1);
        let mut assembler = ReferenceBlobAssembler::default();
        assert_eq!(
            assembler.admit(chunks[0].clone()).unwrap(),
            ChunkAdmission::Complete(blob.clone())
        );
        assert_eq!(
            assembler.admit(chunks[0].clone()).unwrap(),
            ChunkAdmission::Duplicate
        );
    }

    #[test]
    fn chunking_is_canonical_and_out_of_order_reassembles() {
        let mut bytes = vec![0_u8; MAX_REFERENCE_CHUNK_BYTES + PNG.len()];
        bytes[..PNG.len()].copy_from_slice(PNG);
        let blob = ReferenceBlob::new("reference_large", "large.png", "image/png", &bytes).unwrap();
        let chunks = blob.chunks().unwrap();
        assert_eq!(chunks.len(), 2);
        assert_eq!(
            chunks[0].chunk_byte_length,
            MAX_REFERENCE_CHUNK_BYTES as u64
        );
        let mut assembler = ReferenceBlobAssembler::default();
        assert_eq!(
            assembler.admit(chunks[1].clone()).unwrap(),
            ChunkAdmission::Stored
        );
        let checkpoint = canonical::to_vec(&assembler).unwrap();
        let mut assembler: ReferenceBlobAssembler = canonical::from_slice(&checkpoint).unwrap();
        assert_eq!(
            assembler.admit(chunks[0].clone()).unwrap(),
            ChunkAdmission::Complete(blob)
        );
    }

    #[test]
    fn rejects_type_digest_length_position_and_path_tampering() {
        let blob =
            ReferenceBlob::new("reference_fixture", "reference.png", "image/png", PNG).unwrap();
        let mut invalid = blob.clone();
        invalid.media_type = "image/jpeg".into();
        assert!(invalid.validate().is_err());
        let mut invalid = blob.clone();
        invalid.origin_name = "../reference.png".into();
        assert!(invalid.validate().is_err());
        let mut chunk = blob.chunks().unwrap().remove(0);
        chunk.chunk_sha256 = base64url(&[9_u8; 32]);
        assert!(chunk.validate().is_err());
        let mut chunk = blob.chunks().unwrap().remove(0);
        chunk.chunk_index = 1;
        assert!(chunk.validate().is_err());
    }

    #[test]
    fn jpeg_requires_a_real_frame_header_and_nonzero_dimensions() {
        let jpeg = [
            0xff, 0xd8, 0xff, 0xc0, 0x00, 0x11, 0x08, 0x00, 0x01, 0x00, 0x01, 0x03, 0x01, 0x11,
            0x00, 0x02, 0x11, 0x00, 0x03, 0x11, 0x00, 0xff, 0xd9,
        ];
        ReferenceBlob::new("reference_jpeg", "reference.jpg", "image/jpeg", &jpeg).unwrap();
        assert!(
            ReferenceBlob::new("reference_wrong_type", "reference.jpg", "image/jpeg", PNG,)
                .is_err()
        );
        assert!(ReferenceBlob::new(
            "reference_truncated",
            "reference.jpg",
            "image/jpeg",
            &[0xff, 0xd8, 0xff, 0xd9],
        )
        .is_err());
    }
}
