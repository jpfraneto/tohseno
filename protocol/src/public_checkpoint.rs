//! Privacy-safe public projection for ShotRegistry heads.
//!
//! The canonical coherent-intention lineage may contain private ancestors.
//! Its ordinary action commitments are therefore never registry heads. This
//! deliberately narrow chain commits only public witness coordinates and can
//! be rebuilt without touching intention, feedback, runtime, or artifact
//! records.

use crate::actions::{
    Eip712Domain, RegistryActionV2, SHOT_REGISTRY_DOMAIN, SHOT_REGISTRY_V2_EIP712_VERSION,
};
use crate::builder::MAX_SAFE_JSON_INTEGER;
use crate::canonical;
use crate::digest::{Address20, Bytes32, ShotId};
use crate::identity::ROBINHOOD_CHAIN_ID;
use crate::record::CanonicalTimestamp;
use crate::text::invalid;
use crate::Result;
use serde::{Deserialize, Serialize};

pub const PUBLIC_CHECKPOINT_PROTOCOL: &str = "tohseno";
pub const PUBLIC_CHECKPOINT_PROTOCOL_VERSION: &str = "2";
pub const PUBLIC_CHECKPOINT_SCHEMA: &str = "tohseno.public-checkpoint/1";
pub const PUBLIC_CHECKPOINT_CONTRACT_GENERATION: &str = "0.8.0";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicCheckpointScope {
    ShotIdentityContinuity,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicCheckpointWitness {
    pub contract_generation: String,
    pub chain_id: u64,
    pub registry: Address20,
}

impl PublicCheckpointWitness {
    pub fn validate(&self) -> Result<()> {
        if self.contract_generation != PUBLIC_CHECKPOINT_CONTRACT_GENERATION {
            return Err(invalid(
                "public_checkpoint.witness.contract_generation",
                format!("must be {PUBLIC_CHECKPOINT_CONTRACT_GENERATION}"),
            ));
        }
        if self.chain_id != ROBINHOOD_CHAIN_ID {
            return Err(invalid(
                "public_checkpoint.witness.chain_id",
                format!("must be {ROBINHOOD_CHAIN_ID}"),
            ));
        }
        if self.registry.as_bytes().iter().all(|byte| *byte == 0) {
            return Err(invalid(
                "public_checkpoint.witness.registry",
                "must not be the zero address",
            ));
        }
        Ok(())
    }
}

/// One canonical public-only checkpoint.
///
/// It intentionally has no arbitrary digest, lineage-action reference,
/// expression/version reference, controller, content field, or free text.
/// Controller authorization is supplied by the live ShotRegistry action whose
/// `head` equals this record's commitment.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicCheckpoint {
    pub protocol: String,
    pub protocol_version: String,
    pub schema: String,
    pub scope: PublicCheckpointScope,
    pub witness: PublicCheckpointWitness,
    pub shot_id: ShotId,
    pub checkpoint_sequence: u64,
    pub previous_checkpoint: Option<Bytes32>,
    pub issued_at: CanonicalTimestamp,
}

impl PublicCheckpoint {
    pub fn new(
        witness: PublicCheckpointWitness,
        shot_id: ShotId,
        checkpoint_sequence: u64,
        previous_checkpoint: Option<Bytes32>,
        issued_at: CanonicalTimestamp,
    ) -> Result<Self> {
        let checkpoint = Self {
            protocol: PUBLIC_CHECKPOINT_PROTOCOL.into(),
            protocol_version: PUBLIC_CHECKPOINT_PROTOCOL_VERSION.into(),
            schema: PUBLIC_CHECKPOINT_SCHEMA.into(),
            scope: PublicCheckpointScope::ShotIdentityContinuity,
            witness,
            shot_id,
            checkpoint_sequence,
            previous_checkpoint,
            issued_at,
        };
        checkpoint.validate()?;
        Ok(checkpoint)
    }

    pub fn validate(&self) -> Result<()> {
        if self.protocol != PUBLIC_CHECKPOINT_PROTOCOL {
            return Err(invalid(
                "public_checkpoint.protocol",
                format!("must be {PUBLIC_CHECKPOINT_PROTOCOL}"),
            ));
        }
        if self.protocol_version != PUBLIC_CHECKPOINT_PROTOCOL_VERSION {
            return Err(invalid(
                "public_checkpoint.protocol_version",
                format!("must be {PUBLIC_CHECKPOINT_PROTOCOL_VERSION}"),
            ));
        }
        if self.schema != PUBLIC_CHECKPOINT_SCHEMA {
            return Err(invalid(
                "public_checkpoint.schema",
                format!("must be {PUBLIC_CHECKPOINT_SCHEMA}"),
            ));
        }
        self.witness.validate()?;
        if self.shot_id.is_zero() {
            return Err(invalid("public_checkpoint.shot_id", "must not be zero"));
        }
        if self.checkpoint_sequence == 0 || self.checkpoint_sequence > MAX_SAFE_JSON_INTEGER {
            return Err(invalid(
                "public_checkpoint.checkpoint_sequence",
                "must be a positive JavaScript-safe integer",
            ));
        }
        match (self.checkpoint_sequence, self.previous_checkpoint) {
            (1, None) => {}
            (1, Some(_)) => {
                return Err(invalid(
                    "public_checkpoint.previous_checkpoint",
                    "checkpoint 1 must not claim a predecessor",
                ))
            }
            (_, None) => {
                return Err(invalid(
                    "public_checkpoint.previous_checkpoint",
                    "a continuation must name the prior public checkpoint",
                ))
            }
            (_, Some(value)) if value == Bytes32::ZERO => {
                return Err(invalid(
                    "public_checkpoint.previous_checkpoint",
                    "must not be zero",
                ))
            }
            _ => {}
        }
        Ok(())
    }

    /// SHA-256 of the RFC 8785 checkpoint bytes.
    pub fn commitment(&self) -> Result<Bytes32> {
        self.validate()?;
        canonical::sha256_commitment(self)
    }

    /// Binds this public-only body to the exact registry action that will
    /// authorize it. Signature and live ERC-1271 checks remain separate.
    pub fn validate_registry_action(
        &self,
        action: &RegistryActionV2,
        domain: &Eip712Domain,
    ) -> Result<()> {
        self.validate()?;
        action.validate()?;
        domain.validate_for_version(SHOT_REGISTRY_DOMAIN, SHOT_REGISTRY_V2_EIP712_VERSION)?;
        if domain.chain_id != self.witness.chain_id
            || domain.verifying_contract != self.witness.registry
        {
            return Err(invalid(
                "public_checkpoint.witness",
                "must equal the registry action domain",
            ));
        }
        let head = self.commitment()?;
        match action {
            RegistryActionV2::RegisterShot {
                shot_id,
                head: action_head,
                ..
            } if *shot_id == self.shot_id
                && *action_head == head
                && self.checkpoint_sequence == 1
                && self.previous_checkpoint.is_none() =>
            {
                Ok(())
            }
            RegistryActionV2::AppendCheckpoint {
                shot_id,
                previous_head,
                new_head,
                checkpoint_sequence,
                ..
            } if *shot_id == self.shot_id
                && self.previous_checkpoint == Some(*previous_head)
                && *new_head == head
                && *checkpoint_sequence == self.checkpoint_sequence =>
            {
                Ok(())
            }
            RegistryActionV2::RegisterShot { .. } | RegistryActionV2::AppendCheckpoint { .. } => {
                Err(invalid(
                    "public_checkpoint.registry_action",
                    "does not bind this checkpoint exactly",
                ))
            }
            RegistryActionV2::TransferShot { .. } => Err(invalid(
                "public_checkpoint.registry_action",
                "a transfer preserves the current public checkpoint",
            )),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicCheckpointAnchor {
    pub witness: PublicCheckpointWitness,
    pub shot_id: ShotId,
    pub checkpoint_sequence: u64,
    pub head: Bytes32,
    pub issued_at: CanonicalTimestamp,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedPublicCheckpointSegment {
    pub witness: PublicCheckpointWitness,
    pub shot_id: ShotId,
    pub first_checkpoint_sequence: u64,
    pub last_checkpoint_sequence: u64,
    pub head: Bytes32,
    pub complete_from_registration: bool,
}

/// Verifies public-only adjacency and chronology.
///
/// This proves canonical bytes and continuity of the projection. It does not
/// prove controller authority or on-chain acceptance; those require the
/// matching ShotRegistry state/events.
pub fn verify_public_checkpoint_segment(
    checkpoints: &[PublicCheckpoint],
    anchor: Option<&PublicCheckpointAnchor>,
) -> Result<VerifiedPublicCheckpointSegment> {
    let Some(first) = checkpoints.first() else {
        return Err(invalid("public_checkpoint.segment", "must not be empty"));
    };
    let witness = first.witness.clone();
    let shot_id = first.shot_id;
    let mut expected_sequence = match anchor {
        Some(value) => value
            .checkpoint_sequence
            .checked_add(1)
            .ok_or_else(|| invalid("public_checkpoint.segment", "anchor sequence overflowed"))?,
        None => first.checkpoint_sequence,
    };
    let mut expected_previous = anchor.map(|value| value.head).or(first.previous_checkpoint);
    let mut prior_time = anchor.map(|value| value.issued_at.unix_timestamp());

    if anchor.is_some_and(|value| value.shot_id != shot_id) {
        return Err(invalid(
            "public_checkpoint.segment",
            "anchor ShotID differs",
        ));
    }
    if anchor.is_some_and(|value| value.witness != witness) {
        return Err(invalid(
            "public_checkpoint.segment",
            "anchor witness differs",
        ));
    }

    for checkpoint in checkpoints {
        checkpoint.validate()?;
        if checkpoint.witness != witness {
            return Err(invalid(
                "public_checkpoint.segment",
                "witness coordinates changed",
            ));
        }
        if checkpoint.shot_id != shot_id {
            return Err(invalid("public_checkpoint.segment", "ShotID changed"));
        }
        if checkpoint.checkpoint_sequence != expected_sequence {
            return Err(invalid(
                "public_checkpoint.segment",
                "sequence is not contiguous",
            ));
        }
        if checkpoint.previous_checkpoint != expected_previous {
            return Err(invalid(
                "public_checkpoint.segment",
                "predecessor does not match",
            ));
        }
        if prior_time.is_some_and(|value| checkpoint.issued_at.unix_timestamp() < value) {
            return Err(invalid(
                "public_checkpoint.segment",
                "timestamp moved backwards",
            ));
        }
        expected_previous = Some(checkpoint.commitment()?);
        prior_time = Some(checkpoint.issued_at.unix_timestamp());
        expected_sequence = expected_sequence
            .checked_add(1)
            .ok_or_else(|| invalid("public_checkpoint.segment", "sequence overflowed"))?;
    }

    let complete_from_registration =
        anchor.is_none() && first.checkpoint_sequence == 1 && first.previous_checkpoint.is_none();
    Ok(VerifiedPublicCheckpointSegment {
        witness,
        shot_id,
        first_checkpoint_sequence: first.checkpoint_sequence,
        last_checkpoint_sequence: checkpoints
            .last()
            .expect("nonempty checkpoint segment")
            .checkpoint_sequence,
        head: expected_previous.expect("verified checkpoint supplied a commitment"),
        complete_from_registration,
    })
}
