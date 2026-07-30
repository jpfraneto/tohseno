use crate::text::{invalid, validate_lower_hex};
use crate::Result;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest as _, Sha256};
use std::fmt;

#[derive(Clone, Copy, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Bytes32([u8; 32]);

impl Bytes32 {
    pub const ZERO: Self = Self([0; 32]);

    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn into_bytes(self) -> [u8; 32] {
        self.0
    }

    pub fn from_hex(field: &'static str, value: &str) -> Result<Self> {
        validate_lower_hex(field, value, 32)?;
        let mut bytes = [0_u8; 32];
        decode_lower_hex(&value[2..], &mut bytes)
            .ok_or_else(|| invalid(field, "contains invalid hexadecimal"))?;
        Ok(Self(bytes))
    }

    pub fn to_hex(self) -> String {
        encode_prefixed(&self.0)
    }
}

impl fmt::Debug for Bytes32 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

impl fmt::Display for Bytes32 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

impl Serialize for Bytes32 {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Bytes32 {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::from_hex("bytes32", &value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ShotId(Bytes32);

impl ShotId {
    pub fn random() -> Self {
        let mut bytes = [0_u8; 32];
        OsRng.fill_bytes(&mut bytes);
        Self(Bytes32::new(bytes))
    }

    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(Bytes32::new(bytes))
    }

    pub const fn bytes(self) -> Bytes32 {
        self.0
    }

    pub fn is_zero(self) -> bool {
        self.0 == Bytes32::ZERO
    }
}

impl fmt::Debug for ShotId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, formatter)
    }
}

impl fmt::Display for ShotId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}

/// Stable identity of one concrete manifestation of a Shot.
///
/// Expression IDs are random rather than derived from a name, repository,
/// bundle identifier, or platform so those replaceable facts may change
/// without changing expression identity.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ExpressionId(Bytes32);

impl ExpressionId {
    pub fn random() -> Self {
        let mut bytes = [0_u8; 32];
        OsRng.fill_bytes(&mut bytes);
        Self(Bytes32::new(bytes))
    }

    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(Bytes32::new(bytes))
    }

    pub const fn bytes(self) -> Bytes32 {
        self.0
    }

    pub fn is_zero(self) -> bool {
        self.0 == Bytes32::ZERO
    }

    /// Deterministic projection used only when adapting a frozen v1 Apple
    /// lineage, which did not carry an ExpressionID.
    pub fn for_legacy_v1(shot_id: ShotId, bundle_id: &str) -> Self {
        let mut preimage =
            Vec::with_capacity(b"TOHSENO-LEGACY-V1-EXPRESSION\0".len() + 32 + bundle_id.len());
        preimage.extend_from_slice(b"TOHSENO-LEGACY-V1-EXPRESSION\0");
        preimage.extend_from_slice(shot_id.bytes().as_bytes());
        preimage.extend_from_slice(bundle_id.as_bytes());
        Self(sha256(&preimage))
    }
}

impl fmt::Debug for ExpressionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, formatter)
    }
}

impl fmt::Display for ExpressionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}

/// Content-bound identity of one accepted immutable expression state.
///
/// The derivation intentionally excludes folders, remotes, display names, and
/// token addresses. It binds the Shot, expression, expression-local ordinal,
/// accepted genome, and concrete source state.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VersionId(Bytes32);

impl VersionId {
    pub fn derive(
        shot_id: ShotId,
        expression_id: ExpressionId,
        ordinal: u64,
        genome_digest: Bytes32,
        source_digest: Bytes32,
    ) -> Self {
        let mut preimage =
            Vec::with_capacity(b"TOHSENO-VERSION-ID-V2\0".len() + 32 + 32 + 8 + 32 + 32);
        preimage.extend_from_slice(b"TOHSENO-VERSION-ID-V2\0");
        preimage.extend_from_slice(shot_id.bytes().as_bytes());
        preimage.extend_from_slice(expression_id.bytes().as_bytes());
        preimage.extend_from_slice(&ordinal.to_be_bytes());
        preimage.extend_from_slice(genome_digest.as_bytes());
        preimage.extend_from_slice(source_digest.as_bytes());
        Self(sha256(&preimage))
    }

    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(Bytes32::new(bytes))
    }

    /// Deterministic projection used only for an already signed v1 Apple
    /// record. It binds the untouched v1 record commitment rather than
    /// inventing a historical neutral genome.
    pub fn for_legacy_v1(
        shot_id: ShotId,
        expression_id: ExpressionId,
        sequence: u32,
        record_commitment: Bytes32,
    ) -> Self {
        let mut preimage =
            Vec::with_capacity(b"TOHSENO-LEGACY-V1-VERSION\0".len() + 32 + 32 + 4 + 32);
        preimage.extend_from_slice(b"TOHSENO-LEGACY-V1-VERSION\0");
        preimage.extend_from_slice(shot_id.bytes().as_bytes());
        preimage.extend_from_slice(expression_id.bytes().as_bytes());
        preimage.extend_from_slice(&sequence.to_be_bytes());
        preimage.extend_from_slice(record_commitment.as_bytes());
        Self(sha256(&preimage))
    }

    pub const fn bytes(self) -> Bytes32 {
        self.0
    }

    pub fn is_zero(self) -> bool {
        self.0 == Bytes32::ZERO
    }
}

impl fmt::Debug for VersionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, formatter)
    }
}

impl fmt::Display for VersionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Address20(HexAddress);

impl Address20 {
    pub const fn from_bytes(bytes: [u8; 20]) -> Self {
        Self(HexAddress(bytes))
    }

    pub const fn as_bytes(&self) -> &[u8; 20] {
        &(self.0).0
    }
}

impl fmt::Debug for Address20 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, formatter)
    }
}

impl fmt::Display for Address20 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct HexAddress([u8; 20]);

impl Serialize for HexAddress {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&encode_prefixed(&self.0))
    }
}

impl<'de> Deserialize<'de> for HexAddress {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        validate_lower_hex("address", &value, 20).map_err(serde::de::Error::custom)?;
        let mut bytes = [0_u8; 20];
        decode_lower_hex(&value[2..], &mut bytes)
            .ok_or_else(|| serde::de::Error::custom("address contains invalid hexadecimal"))?;
        Ok(Self(bytes))
    }
}

impl fmt::Debug for HexAddress {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&encode_prefixed(&self.0))
    }
}

impl fmt::Display for HexAddress {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&encode_prefixed(&self.0))
    }
}

pub fn sha256(bytes: &[u8]) -> Bytes32 {
    Bytes32::new(Sha256::digest(bytes).into())
}

fn encode_prefixed(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(2 + bytes.len() * 2);
    output.push_str("0x");
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn decode_lower_hex(input: &str, output: &mut [u8]) -> Option<()> {
    if input.len() != output.len() * 2 {
        return None;
    }
    for (index, target) in output.iter_mut().enumerate() {
        let high = decode_nibble(input.as_bytes()[index * 2])?;
        let low = decode_nibble(input.as_bytes()[index * 2 + 1])?;
        *target = (high << 4) | low;
    }
    Some(())
}

fn decode_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_width_hex_is_lowercase_and_strict() {
        let value = Bytes32::new([0xab; 32]);
        assert_eq!(value.to_hex(), format!("0x{}", "ab".repeat(32)));
        assert_eq!(Bytes32::from_hex("digest", &value.to_hex()).unwrap(), value);
        assert!(Bytes32::from_hex("digest", &value.to_hex().to_uppercase()).is_err());
        assert!(Bytes32::from_hex("digest", "0x00").is_err());
    }

    #[test]
    fn serde_rejects_uppercase_and_wrong_width_addresses() {
        assert!(serde_json::from_str::<Address20>(
            "\"0x0000000000000000000000000000000000000001\""
        )
        .is_ok());
        assert!(serde_json::from_str::<Address20>(
            "\"0X0000000000000000000000000000000000000001\""
        )
        .is_err());
    }
}
