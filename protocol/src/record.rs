use crate::canonical;
use crate::digest::{Bytes32, ShotId};
use crate::identity::BuilderId;
use crate::signature::SignatureSidecar;
use crate::text::{invalid, validate_bundle_id, validate_slug, validate_token};
use crate::Result;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

pub const PROTOCOL_NAME: &str = "tohseno";
pub const SHOT_SCHEMA: &str = "tohseno.shot/1";
pub const APPLE_FASCIA_ID: &str = "tohseno.apple/1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalTimestamp(String);

impl CanonicalTimestamp {
    pub fn parse(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        let parsed = OffsetDateTime::parse(&value, &Rfc3339)
            .map_err(|_| invalid("created_at", "must be a valid RFC3339 timestamp"))?;
        if parsed.offset() != time::UtcOffset::UTC
            || parsed.nanosecond() != 0
            || value.len() != 20
            || !value.ends_with('Z')
        {
            return Err(invalid(
                "created_at",
                "must use exact UTC second form YYYY-MM-DDTHH:MM:SSZ",
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn unix_timestamp(&self) -> i64 {
        OffsetDateTime::parse(&self.0, &Rfc3339)
            .expect("validated canonical timestamp")
            .unix_timestamp()
    }
}

impl Serialize for CanonicalTimestamp {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for CanonicalTimestamp {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactoryDescriptor {
    pub implementation: String,
    pub version: String,
    pub source_commit: String,
}

impl FactoryDescriptor {
    pub fn validate(&self) -> Result<()> {
        validate_token("factory.implementation", &self.implementation, 1, 200)?;
        validate_token("factory.version", &self.version, 1, 64)?;
        if self.source_commit.len() != 40
            || !self
                .source_commit
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(invalid(
                "factory.source_commit",
                "must be a 40-character lowercase Git object ID",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ShotOrigin {
    LegacyAdoption {
        legacy_latest_shot: u32,
        legacy_source_sha256: Bytes32,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShotRecord {
    pub protocol: String,
    pub schema: String,
    pub shot_id: ShotId,
    pub slug: String,
    pub builder_id: BuilderId,
    pub sequence: u32,
    pub previous: Option<Bytes32>,
    pub fascia: String,
    pub bundle_id: String,
    pub bundle_version: u32,
    pub genesis_input_sha256: Bytes32,
    pub source_tree_sha256: Bytes32,
    pub fascia_sha256: Bytes32,
    pub factory: FactoryDescriptor,
    pub created_at: CanonicalTimestamp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<ShotOrigin>,
}

impl ShotRecord {
    pub fn validate(&self) -> Result<()> {
        if self.protocol != PROTOCOL_NAME {
            return Err(invalid("protocol", format!("must be {PROTOCOL_NAME}")));
        }
        if self.schema != SHOT_SCHEMA {
            return Err(invalid("schema", format!("must be {SHOT_SCHEMA}")));
        }
        if self.fascia != APPLE_FASCIA_ID {
            return Err(invalid("fascia", format!("must be {APPLE_FASCIA_ID}")));
        }
        if self.shot_id.is_zero() {
            return Err(invalid("shot_id", "must not be zero"));
        }
        self.builder_id.validate()?;
        validate_slug("slug", &self.slug)?;
        validate_bundle_id("bundle_id", &self.bundle_id)?;
        if self.sequence == 0 {
            return Err(invalid("sequence", "must be at least 1"));
        }
        if self.bundle_version != self.sequence {
            return Err(invalid(
                "bundle_version",
                "must equal the Evolution sequence",
            ));
        }
        match &self.origin {
            Some(ShotOrigin::LegacyAdoption {
                legacy_latest_shot,
                legacy_source_sha256,
            }) => {
                if *legacy_latest_shot == 0 {
                    return Err(invalid("origin.legacy_latest_shot", "must be at least 1"));
                }
                let expected = legacy_latest_shot.checked_add(1).ok_or_else(|| {
                    invalid(
                        "origin.legacy_latest_shot",
                        "must leave room for the adopted root Evolution",
                    )
                })?;
                if self.sequence != expected {
                    return Err(invalid(
                        "sequence",
                        "an adopted root must equal legacy_latest_shot + 1",
                    ));
                }
                if self.previous.is_some() {
                    return Err(invalid(
                        "previous",
                        "an adopted root has no protocol commitment to reference and must be null",
                    ));
                }
                if *legacy_source_sha256 == Bytes32::ZERO {
                    return Err(invalid(
                        "origin.legacy_source_sha256",
                        "must commit the adopted legacy source",
                    ));
                }
            }
            None => match (self.sequence, self.previous) {
                (1, None) => {}
                (1, Some(_)) => return Err(invalid("previous", "must be null for sequence 1")),
                (_, Some(_)) => {}
                (_, None) => {
                    return Err(invalid(
                        "previous",
                        "must contain the preceding protocol Evolution commitment",
                    ))
                }
            },
        }
        self.factory.validate()?;
        Ok(())
    }

    pub fn legacy_latest_shot(&self) -> Option<u32> {
        self.origin.as_ref().map(
            |ShotOrigin::LegacyAdoption {
                 legacy_latest_shot, ..
             }| *legacy_latest_shot,
        )
    }

    /// SHA-256 of RFC 8785 canonical record bytes. The signature sidecar is a
    /// separate object and is therefore never part of this commitment.
    pub fn commitment(&self) -> Result<Bytes32> {
        self.validate()?;
        canonical::sha256_commitment(self)
    }

    pub fn verify_signature(&self, signature: &SignatureSidecar) -> Result<()> {
        self.validate()?;
        signature.verify(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::digest::Address20;

    fn record(sequence: u32, previous: Option<Bytes32>) -> ShotRecord {
        ShotRecord {
            protocol: PROTOCOL_NAME.into(),
            schema: SHOT_SCHEMA.into(),
            shot_id: ShotId::from_bytes([1; 32]),
            slug: "paper-press".into(),
            builder_id: BuilderId::new(Address20::from_bytes([2; 20])),
            sequence,
            previous,
            fascia: APPLE_FASCIA_ID.into(),
            bundle_id: "com.example.paper-press".into(),
            bundle_version: sequence,
            genesis_input_sha256: Bytes32::new([3; 32]),
            source_tree_sha256: Bytes32::new([4; 32]),
            fascia_sha256: Bytes32::new([5; 32]),
            factory: FactoryDescriptor {
                implementation: "example/factory".into(),
                version: "1.0.0-rc.1".into(),
                source_commit: "a".repeat(40),
            },
            created_at: CanonicalTimestamp::parse("2026-07-28T00:00:00Z").unwrap(),
            origin: None,
        }
    }

    #[test]
    fn sequence_one_is_genesis_and_bundle_version_is_identical() {
        assert!(record(1, None).validate().is_ok());
        assert!(record(1, Some(Bytes32::new([9; 32]))).validate().is_err());
        assert!(record(2, None).validate().is_err());
        let mut wrong = record(2, Some(Bytes32::new([9; 32])));
        wrong.bundle_version = 3;
        assert!(wrong.validate().is_err());
    }

    #[test]
    fn legacy_adoption_is_a_null_previous_root_at_n_plus_one() {
        let mut adopted = record(8, None);
        adopted.origin = Some(ShotOrigin::LegacyAdoption {
            legacy_latest_shot: 7,
            legacy_source_sha256: Bytes32::new([0xaa; 32]),
        });
        assert!(adopted.validate().is_ok());

        let mut wrong_sequence = adopted.clone();
        wrong_sequence.sequence = 9;
        wrong_sequence.bundle_version = 9;
        assert!(wrong_sequence.validate().is_err());

        let mut fabricated_previous = adopted.clone();
        fabricated_previous.previous = Some(Bytes32::new([0xbb; 32]));
        assert!(fabricated_previous.validate().is_err());

        let mut empty_source = adopted;
        empty_source.origin = Some(ShotOrigin::LegacyAdoption {
            legacy_latest_shot: 7,
            legacy_source_sha256: Bytes32::ZERO,
        });
        assert!(empty_source.validate().is_err());
    }

    #[test]
    fn timestamp_has_one_cross_language_form() {
        assert!(CanonicalTimestamp::parse("2026-07-28T00:00:00Z").is_ok());
        assert!(CanonicalTimestamp::parse("2026-07-28T00:00:00.000Z").is_err());
        assert!(CanonicalTimestamp::parse("2026-07-28T00:00:00-00:00").is_err());
    }
}
