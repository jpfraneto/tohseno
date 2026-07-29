use crate::digest::{sha256, Bytes32};
use crate::{ProtocolError, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;

/// Produces RFC 8785 JSON Canonicalization Scheme bytes.
///
/// All normative TOHSENO schemas use integers only. Implementations must
/// reject duplicate object members before canonicalization; Serde's derived
/// closed structs do this, and every protocol structure denies unknown fields.
pub fn to_vec<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    serde_json_canonicalizer::to_vec(value)
        .map_err(|error| ProtocolError::CanonicalJson(error.to_string()))
}

pub fn to_string<T: Serialize>(value: &T) -> Result<String> {
    String::from_utf8(to_vec(value)?)
        .map_err(|error| ProtocolError::CanonicalJson(error.to_string()))
}

pub fn sha256_commitment<T: Serialize>(value: &T) -> Result<Bytes32> {
    Ok(sha256(&to_vec(value)?))
}

/// Parses exactly one closed JSON object and rejects trailing data.
///
/// The target type is responsible for `#[serde(deny_unknown_fields)]`.
pub fn from_slice<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = T::deserialize(&mut deserializer)?;
    deserializer.end()?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct Closed {
        a: u32,
        z: String,
    }

    #[test]
    fn follows_rfc_8785_key_order_and_escaping() {
        let value = Closed {
            a: 1,
            z: "\u{20ac}\n".into(),
        };
        assert_eq!(to_string(&value).unwrap(), "{\"a\":1,\"z\":\"€\\n\"}");
    }

    #[test]
    fn closed_parse_rejects_unknown_duplicate_and_trailing_members() {
        assert!(from_slice::<Closed>(br#"{"a":1,"z":"x","other":2}"#).is_err());
        assert!(from_slice::<Closed>(br#"{"a":1,"a":2,"z":"x"}"#).is_err());
        assert!(from_slice::<Closed>(br#"{"a":1,"z":"x"} null"#).is_err());
    }
}
