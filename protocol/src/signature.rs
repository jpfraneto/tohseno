use crate::canonical;
use crate::digest::Bytes32;
use crate::text::invalid;
use crate::{ProtocolError, Result};
use p256::ecdsa::signature::hazmat::PrehashVerifier;
use p256::ecdsa::{Signature, VerifyingKey};
use p256::{EncodedPoint, FieldBytes};
use serde::{Deserialize, Serialize};

pub const COMPACT_SIGNATURE_VERSION: u8 = 1;
pub const COMPACT_SIGNATURE_LENGTH: usize = 129;

const P256_HALF_ORDER: [u8; 32] = [
    0x7f, 0xff, 0xff, 0xff, 0x80, 0x00, 0x00, 0x00, 0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xde, 0x73, 0x7d, 0x56, 0xd3, 0x8b, 0xcf, 0x42, 0x79, 0xdc, 0xe5, 0x61, 0x7e, 0x31, 0x92, 0xa8,
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct P256PublicKey {
    pub x: Bytes32,
    pub y: Bytes32,
}

impl P256PublicKey {
    pub fn validate(&self) -> Result<()> {
        self.verifying_key().map(|_| ())
    }

    pub fn verifying_key(&self) -> Result<VerifyingKey> {
        let x: FieldBytes = self.x.into_bytes().into();
        let y: FieldBytes = self.y.into_bytes().into();
        let point = EncodedPoint::from_affine_coordinates(&x, &y, false);
        VerifyingKey::from_encoded_point(&point).map_err(|_| ProtocolError::InvalidPublicKey)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct P256Signature {
    pub r: Bytes32,
    pub s: Bytes32,
}

impl P256Signature {
    pub fn validate_scalars(&self) -> Result<Signature> {
        Signature::from_scalars(*self.r.as_bytes(), *self.s.as_bytes())
            .map_err(|_| ProtocolError::InvalidSignature)
    }

    pub fn is_low_s(&self) -> bool {
        self.s.as_bytes() <= &P256_HALF_ORDER
    }

    pub fn validate_low_s(&self) -> Result<Signature> {
        let signature = self.validate_scalars()?;
        if !self.is_low_s() {
            return Err(ProtocolError::HighSignatureS);
        }
        Ok(signature)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SignatureAlgorithm {
    #[serde(rename = "p256")]
    P256,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetachedP256Signature {
    pub algorithm: SignatureAlgorithm,
    pub digest: Bytes32,
    pub signature: P256Signature,
    pub low_s: bool,
}

impl DetachedP256Signature {
    pub fn validate(&self) -> Result<()> {
        if self.algorithm != SignatureAlgorithm::P256 {
            return Err(invalid("signature.algorithm", "must be p256"));
        }
        if !self.low_s {
            return Err(invalid("signature.low_s", "must be true"));
        }
        self.signature.validate_low_s().map(|_| ())
    }

    pub fn verify(&self, public_key: &P256PublicKey) -> Result<()> {
        self.validate()?;
        verify_digest(public_key, self.digest, &self.signature)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignatureSidecar {
    pub schema: String,
    pub algorithm: SignatureAlgorithm,
    pub digest: Bytes32,
    pub public_key: P256PublicKey,
    pub signature: P256Signature,
    pub low_s: bool,
}

impl SignatureSidecar {
    pub const SCHEMA: &'static str = "tohseno.signature/1";

    pub fn validate(&self) -> Result<()> {
        if self.schema != Self::SCHEMA {
            return Err(invalid(
                "signature.schema",
                format!("must be {}", Self::SCHEMA),
            ));
        }
        if self.algorithm != SignatureAlgorithm::P256 {
            return Err(invalid("signature.algorithm", "must be p256"));
        }
        if !self.low_s {
            return Err(invalid("signature.low_s", "must be true"));
        }
        self.public_key.validate()?;
        self.signature.validate_low_s().map(|_| ())
    }

    pub fn verify<T: Serialize>(&self, signed_object: &T) -> Result<()> {
        self.validate()?;
        let observed = canonical::sha256_commitment(signed_object)?;
        if self.digest != observed {
            return Err(ProtocolError::DigestMismatch);
        }
        verify_digest(&self.public_key, observed, &self.signature)
    }
}

pub fn verify_digest(
    public_key: &P256PublicKey,
    digest: Bytes32,
    signature: &P256Signature,
) -> Result<()> {
    let verifying_key = public_key.verifying_key()?;
    let signature = signature.validate_low_s()?;
    verifying_key
        .verify_prehash(digest.as_bytes(), &signature)
        .map_err(|_| ProtocolError::InvalidSignature)
}

pub fn encode_compact(
    public_key: &P256PublicKey,
    signature: &P256Signature,
) -> Result<[u8; COMPACT_SIGNATURE_LENGTH]> {
    public_key.validate()?;
    signature.validate_low_s()?;
    let mut encoded = [0_u8; COMPACT_SIGNATURE_LENGTH];
    encoded[0] = COMPACT_SIGNATURE_VERSION;
    encoded[1..33].copy_from_slice(public_key.x.as_bytes());
    encoded[33..65].copy_from_slice(public_key.y.as_bytes());
    encoded[65..97].copy_from_slice(signature.r.as_bytes());
    encoded[97..129].copy_from_slice(signature.s.as_bytes());
    Ok(encoded)
}

pub fn decode_compact(encoded: &[u8]) -> Result<(P256PublicKey, P256Signature)> {
    if encoded.len() != COMPACT_SIGNATURE_LENGTH || encoded[0] != COMPACT_SIGNATURE_VERSION {
        return Err(invalid(
            "compact_signature",
            "must be version 0x01 followed by x, y, r, and s",
        ));
    }
    let component = |range: std::ops::Range<usize>| {
        let mut bytes = [0_u8; 32];
        bytes.copy_from_slice(&encoded[range]);
        Bytes32::new(bytes)
    };
    let public_key = P256PublicKey {
        x: component(1..33),
        y: component(33..65),
    };
    let signature = P256Signature {
        r: component(65..97),
        s: component(97..129),
    };
    public_key.validate()?;
    signature.validate_low_s()?;
    Ok((public_key, signature))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_zero_scalars_and_high_s_before_verification() {
        let zero = P256Signature {
            r: Bytes32::ZERO,
            s: Bytes32::ZERO,
        };
        assert!(matches!(
            zero.validate_scalars(),
            Err(ProtocolError::InvalidSignature)
        ));
        let high = P256Signature {
            r: Bytes32::new([1; 32]),
            s: Bytes32::new([0xff; 32]),
        };
        assert!(!high.is_low_s());
    }
}
