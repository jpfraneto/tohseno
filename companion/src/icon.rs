//! Private icon blobs carried only inside recipient-encrypted companion envelopes.

use crate::crypto::{base64url, decode_base64url, sha256};
use crate::snapshot::{IconDescriptor, MAX_ICON_BYTES, MAX_ICON_DIMENSION};
use crate::{require, validate_identifier, Result};
use serde::{Deserialize, Serialize};

pub const ICON_BLOB_SCHEMA: &str = "tohseno.companion-icon-blob/1";

/// Exact bounded image bytes for an `IconDescriptor`.
///
/// This object is private payload, never relay metadata. The relay sees only
/// the authenticated envelope that contains its canonical JSON encoding.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IconBlob {
    pub schema: String,
    pub blob_id: String,
    pub revision: u64,
    pub media_type: String,
    pub byte_length: u64,
    pub width: u32,
    pub height: u32,
    pub placeholder: bool,
    pub sha256: String,
    pub bytes: String,
}

impl IconBlob {
    pub fn new(
        blob_id: impl Into<String>,
        revision: u64,
        media_type: impl Into<String>,
        placeholder: bool,
        bytes: &[u8],
    ) -> Result<Self> {
        let media_type = media_type.into();
        let (width, height) = image_dimensions(&media_type, bytes)?;
        let value = Self {
            schema: ICON_BLOB_SCHEMA.into(),
            blob_id: blob_id.into(),
            revision,
            media_type,
            byte_length: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            width,
            height,
            placeholder,
            sha256: base64url(&sha256(bytes)),
            bytes: base64url(bytes),
        };
        value.validate()?;
        Ok(value)
    }

    pub fn from_descriptor(descriptor: &IconDescriptor, bytes: &[u8]) -> Result<Self> {
        descriptor.validate()?;
        let blob = Self::new(
            descriptor.blob_id.clone(),
            descriptor.revision,
            descriptor.media_type.clone(),
            descriptor.placeholder,
            bytes,
        )?;
        blob.matches_descriptor(descriptor)?;
        Ok(blob)
    }

    pub fn descriptor(&self) -> Result<IconDescriptor> {
        self.validate()?;
        Ok(IconDescriptor {
            blob_id: self.blob_id.clone(),
            revision: self.revision,
            media_type: self.media_type.clone(),
            byte_length: self.byte_length,
            width: self.width,
            height: self.height,
            placeholder: self.placeholder,
        })
    }

    pub fn validate(&self) -> Result<()> {
        require(
            self.schema == ICON_BLOB_SCHEMA,
            "unsupported companion icon blob schema",
        )?;
        validate_identifier("icon blob ID", &self.blob_id)?;
        require(self.revision > 0, "icon blob revision must be positive")?;
        require(
            matches!(self.media_type.as_str(), "image/png" | "image/jpeg"),
            "icon blob media type must be image/png or image/jpeg",
        )?;
        require(
            (1..=MAX_ICON_BYTES).contains(&self.byte_length),
            "icon blob byte length is invalid",
        )?;
        require(
            (1..=MAX_ICON_DIMENSION).contains(&self.width)
                && (1..=MAX_ICON_DIMENSION).contains(&self.height),
            "icon blob dimensions are invalid",
        )?;
        let bytes = self.decoded_bytes()?;
        require(
            u64::try_from(bytes.len()).ok() == Some(self.byte_length),
            "icon blob byte length does not match its bytes",
        )?;
        let digest = base64url(&sha256(&bytes));
        require(self.sha256 == digest, "icon blob SHA-256 does not match")?;
        let (width, height) = image_dimensions(&self.media_type, &bytes)?;
        require(
            (width, height) == (self.width, self.height),
            "icon blob dimensions do not match its image header",
        )
    }

    pub fn decoded_bytes(&self) -> Result<Vec<u8>> {
        decode_base64url("icon blob bytes", &self.bytes, MAX_ICON_BYTES as usize)
    }

    pub fn matches_descriptor(&self, descriptor: &IconDescriptor) -> Result<()> {
        self.validate()?;
        descriptor.validate()?;
        require(
            self.blob_id == descriptor.blob_id
                && self.revision == descriptor.revision
                && self.media_type == descriptor.media_type
                && self.byte_length == descriptor.byte_length
                && self.width == descriptor.width
                && self.height == descriptor.height
                && self.placeholder == descriptor.placeholder,
            "icon blob does not match its descriptor",
        )
    }
}

fn image_dimensions(media_type: &str, bytes: &[u8]) -> Result<(u32, u32)> {
    let dimensions = match media_type {
        "image/png" => png_dimensions(bytes),
        "image/jpeg" => jpeg_dimensions(bytes),
        _ => None,
    }
    .ok_or_else(|| {
        crate::CompanionError::Invalid("icon bytes are not the declared image type".into())
    })?;
    require(
        (1..=MAX_ICON_DIMENSION).contains(&dimensions.0)
            && (1..=MAX_ICON_DIMENSION).contains(&dimensions.1),
        "icon image dimensions exceed the companion bound",
    )?;
    Ok(dimensions)
}

fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 24
        || &bytes[..8] != SIGNATURE
        || u32::from_be_bytes(bytes[8..12].try_into().ok()?) != 13
        || &bytes[12..16] != b"IHDR"
    {
        return None;
    }
    Some((
        u32::from_be_bytes(bytes[16..20].try_into().ok()?),
        u32::from_be_bytes(bytes[20..24].try_into().ok()?),
    ))
}

fn jpeg_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 4 || bytes[..2] != [0xff, 0xd8] {
        return None;
    }
    let mut offset = 2_usize;
    while offset < bytes.len() {
        if bytes[offset] != 0xff {
            return None;
        }
        while offset < bytes.len() && bytes[offset] == 0xff {
            offset += 1;
        }
        let marker = *bytes.get(offset)?;
        offset += 1;
        if marker == 0x00 {
            return None;
        }
        if marker == 0xd9 || marker == 0xda {
            return None;
        }
        if marker == 0x01 || (0xd0..=0xd8).contains(&marker) {
            continue;
        }
        let segment_length = usize::from(u16::from_be_bytes([
            *bytes.get(offset)?,
            *bytes.get(offset + 1)?,
        ]));
        if segment_length < 2 {
            return None;
        }
        let segment_start = offset.checked_add(2)?;
        let segment_end = offset.checked_add(segment_length)?;
        if segment_end > bytes.len() {
            return None;
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
            if segment_length < 7 {
                return None;
            }
            let height = u32::from(u16::from_be_bytes([
                *bytes.get(segment_start + 1)?,
                *bytes.get(segment_start + 2)?,
            ]));
            let width = u32::from(u16::from_be_bytes([
                *bytes.get(segment_start + 3)?,
                *bytes.get(segment_start + 4)?,
            ]));
            return Some((width, height));
        }
        offset = segment_end;
    }
    None
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
    fn validates_exact_bounded_png_and_descriptor() {
        let blob = IconBlob::new("icon_fixture", 7, "image/png", false, PNG).unwrap();
        assert_eq!((blob.width, blob.height), (1, 1));
        let descriptor = blob.descriptor().unwrap();
        blob.matches_descriptor(&descriptor).unwrap();
        assert_eq!(IconBlob::from_descriptor(&descriptor, PNG).unwrap(), blob);
    }

    #[test]
    fn rejects_tamper_mime_dimensions_and_oversize() {
        let mut blob = IconBlob::new("icon_fixture", 1, "image/png", false, PNG).unwrap();
        blob.bytes.replace_range(0..1, "A");
        assert!(blob.validate().is_err());

        let mut dimensions = PNG.to_vec();
        dimensions[16..20].copy_from_slice(&2049_u32.to_be_bytes());
        assert!(IconBlob::new("icon_fixture", 1, "image/png", false, &dimensions).is_err());
        assert!(IconBlob::new("icon_fixture", 1, "image/jpeg", false, PNG).is_err());
        assert!(IconBlob::new(
            "icon_fixture",
            1,
            "image/png",
            false,
            &vec![0; MAX_ICON_BYTES as usize + 1],
        )
        .is_err());
    }
}
