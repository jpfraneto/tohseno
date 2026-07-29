use crate::digest::{Bytes32, ShotId};
use crate::identity::BuilderId;
use crate::record::ShotRecord;
use crate::signature::{P256PublicKey, SignatureSidecar};
use crate::{ProtocolError, Result};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedEvolution {
    pub sequence: u32,
    pub commitment: Bytes32,
    pub signer: P256PublicKey,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedLineage {
    pub shot_id: ShotId,
    pub builder_id: BuilderId,
    pub root_sequence: u32,
    pub legacy_latest_shot: Option<u32>,
    pub evolutions: Vec<VerifiedEvolution>,
}

impl VerifiedLineage {
    pub fn head(&self) -> Option<&VerifiedEvolution> {
        self.evolutions.last()
    }
}

/// Verifies a normal sequence-1 root or a declared legacy-adoption N+1 root,
/// then record shape, signature, contiguous sequence, prior commitments,
/// stable Shot identity, stable BuilderID, bundle identity, Fascia version,
/// and monotonic timestamps. Adoption origin is forbidden after the root.
///
/// Device authorization is deliberately a separate judgment. The caller must
/// validate each returned signer against local signed device actions or the
/// deployed BuilderAccount at the relevant sequence.
pub fn verify_lineage(entries: &[(&ShotRecord, &SignatureSidecar)]) -> Result<VerifiedLineage> {
    let Some((genesis, _)) = entries.first() else {
        return Err(ProtocolError::Lineage {
            sequence: 0,
            reason: "lineage is empty".into(),
        });
    };
    let shot_id = genesis.shot_id;
    let builder_id = genesis.builder_id;
    let bundle_id = genesis.bundle_id.clone();
    let fascia = genesis.fascia.clone();
    genesis.validate().map_err(|error| ProtocolError::Lineage {
        sequence: genesis.sequence,
        reason: format!("lineage root is invalid: {error}"),
    })?;
    let legacy_latest_shot = genesis.legacy_latest_shot();
    let root_sequence = match legacy_latest_shot {
        Some(_) => genesis.sequence,
        None if genesis.sequence == 1 && genesis.previous.is_none() => 1,
        None => {
            return Err(ProtocolError::Lineage {
                sequence: genesis.sequence,
                reason: "lineage must begin at sequence 1 or at a declared legacy-adoption root"
                    .into(),
            })
        }
    };
    let mut evolutions = Vec::with_capacity(entries.len());
    let mut previous_commitment = None;
    let mut previous_time = None;

    for (index, (record, signature)) in entries.iter().enumerate() {
        let offset = u32::try_from(index).map_err(|_| ProtocolError::Lineage {
            sequence: u32::MAX,
            reason: "lineage is too long".into(),
        })?;
        let expected_sequence =
            root_sequence
                .checked_add(offset)
                .ok_or_else(|| ProtocolError::Lineage {
                    sequence: u32::MAX,
                    reason: "lineage sequence overflowed".into(),
                })?;
        let fail = |reason: &str| ProtocolError::Lineage {
            sequence: record.sequence,
            reason: reason.into(),
        };
        if record.sequence != expected_sequence {
            return Err(fail("sequence is not contiguous from the lineage root"));
        }
        if index > 0 && record.origin.is_some() {
            return Err(fail(
                "legacy-adoption origin is permitted only on the lineage root",
            ));
        }
        if record.shot_id != shot_id {
            return Err(fail("ShotID changed"));
        }
        if record.builder_id != builder_id {
            return Err(fail("BuilderID changed without an ownership-proof model"));
        }
        if record.bundle_id != bundle_id {
            return Err(fail("bundle identifier changed"));
        }
        if record.fascia != fascia {
            return Err(fail("Fascia identifier changed"));
        }
        if record.previous != previous_commitment {
            return Err(fail("previous commitment does not match"));
        }
        if previous_time.is_some_and(|time| record.created_at.unix_timestamp() < time) {
            return Err(fail("created_at moved backwards"));
        }
        record
            .verify_signature(signature)
            .map_err(|error| fail(&format!("record signature failed: {error}")))?;
        let commitment = record
            .commitment()
            .map_err(|error| fail(&format!("commitment failed: {error}")))?;
        evolutions.push(VerifiedEvolution {
            sequence: record.sequence,
            commitment,
            signer: signature.public_key.clone(),
        });
        previous_commitment = Some(commitment);
        previous_time = Some(record.created_at.unix_timestamp());
    }

    Ok(VerifiedLineage {
        shot_id,
        builder_id,
        root_sequence,
        legacy_latest_shot,
        evolutions,
    })
}
