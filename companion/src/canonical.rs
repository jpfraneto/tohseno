//! RFC 8785 JSON Canonicalization Scheme helpers shared by every signature.

use crate::{CompanionError, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

const MAX_SAFE_JSON_INTEGER: u64 = (1_u64 << 53) - 1;

pub fn to_vec<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let value = serde_json::to_value(value)?;
    validate_safe_numbers(&value)?;
    serde_json_canonicalizer::to_vec(&value).map_err(CompanionError::Canonical)
}

pub fn to_string<T: Serialize>(value: &T) -> Result<String> {
    let value = serde_json::to_value(value)?;
    validate_safe_numbers(&value)?;
    serde_json_canonicalizer::to_string(&value).map_err(CompanionError::Canonical)
}

fn validate_safe_numbers(value: &Value) -> Result<()> {
    match value {
        Value::Number(number) => {
            let safe = number
                .as_u64()
                .is_some_and(|value| value <= MAX_SAFE_JSON_INTEGER)
                || number.as_i64().is_some_and(|value| {
                    value >= -(MAX_SAFE_JSON_INTEGER as i64)
                        && value <= MAX_SAFE_JSON_INTEGER as i64
                });
            if !safe {
                return Err(CompanionError::Invalid(
                    "JSON number is not a cross-language safe integer".into(),
                ));
            }
        }
        Value::Array(values) => {
            for value in values {
                validate_safe_numbers(value)?;
            }
        }
        Value::Object(values) => {
            for value in values.values() {
                validate_safe_numbers(value)?;
            }
        }
        Value::Null | Value::Bool(_) | Value::String(_) => {}
    }
    Ok(())
}

/// Parse a JSON object and require that the supplied bytes already are its
/// unique RFC 8785 representation. This rejects duplicate/alternate wire
/// encodings at boundaries that sign or commit JSON bytes.
pub fn from_slice<T>(bytes: &[u8]) -> Result<T>
where
    T: DeserializeOwned + Serialize,
{
    let value: T = serde_json::from_slice(bytes)?;
    let encoded = to_vec(&value)?;
    if encoded != bytes {
        return Err(CompanionError::Invalid(
            "JSON bytes are not their RFC 8785 canonical representation".into(),
        ));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Deserialize, Serialize)]
    struct Fixture {
        z: u64,
        a: String,
    }

    #[test]
    fn canonical_bytes_sort_object_keys_and_are_unique() {
        let fixture = Fixture {
            z: 2,
            a: "one".into(),
        };
        let encoded = to_vec(&fixture).unwrap();
        assert_eq!(encoded, br#"{"a":"one","z":2}"#);
        assert!(from_slice::<Fixture>(&encoded).is_ok());
        assert!(from_slice::<Fixture>(br#"{"z":2,"a":"one"}"#).is_err());
    }

    #[test]
    fn canonical_bytes_reject_cross_language_unsafe_integers() {
        let fixture = Fixture {
            z: MAX_SAFE_JSON_INTEGER + 1,
            a: "unsafe".into(),
        };
        assert!(to_vec(&fixture).is_err());
        assert!(to_string(&fixture).is_err());
    }
}
