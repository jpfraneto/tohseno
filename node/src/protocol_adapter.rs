//! The intentionally small boundary between transport/storage and protocol
//! law. No node-local wire action exists.

use crate::model::MissingArtifact;
use crate::{NodeError, Result, MAX_ACTION_BYTES};
use tohseno_protocol::canonical;
use tohseno_protocol::digest::{Bytes32, ShotId};
use tohseno_protocol::lineage::{LineagePayload, SignedLineageAction};
use tohseno_protocol::ontology::AvailabilityStatus;

#[derive(Clone, Debug)]
pub(crate) struct ValidatedAction {
    pub signed: SignedLineageAction,
    pub digest: Bytes32,
    pub shot_id: ShotId,
    pub sequence: u64,
    pub previous: Option<Bytes32>,
    pub canonical_bytes: Vec<u8>,
    pub missing_artifacts: Vec<MissingArtifact>,
}

pub(crate) fn parse_public(bytes: &[u8]) -> Result<ValidatedAction> {
    if bytes.len() > MAX_ACTION_BYTES {
        return Err(NodeError::ActionTooLarge {
            limit: MAX_ACTION_BYTES,
        });
    }
    let signed: SignedLineageAction = canonical::from_slice(bytes)?;
    signed.verify()?;
    if signed.action.availability != AvailabilityStatus::PubliclyAvailable {
        return Err(NodeError::NotPublic);
    }
    let digest = signed.commitment()?;
    let canonical_bytes = canonical::to_vec(&signed)?;
    if canonical_bytes.len() > MAX_ACTION_BYTES {
        return Err(NodeError::ActionTooLarge {
            limit: MAX_ACTION_BYTES,
        });
    }
    let missing_artifacts = availability_observations(&signed, digest);
    Ok(ValidatedAction {
        shot_id: signed.action.shot_id,
        sequence: signed.action.sequence,
        previous: signed.action.previous,
        signed,
        digest,
        canonical_bytes,
        missing_artifacts,
    })
}

fn availability_observations(
    action: &SignedLineageAction,
    action_digest: Bytes32,
) -> Vec<MissingArtifact> {
    let LineagePayload::ArtifactAvailability(record) = &action.action.payload else {
        return Vec::new();
    };
    let status = record.availability.status;
    let reason = match status {
        AvailabilityStatus::Absent => "publisher reports that the artifact is absent",
        AvailabilityStatus::Unknown => "artifact availability is unknown",
        AvailabilityStatus::IntentionallyPrivate => "artifact is intentionally private",
        AvailabilityStatus::LocallyAvailable => {
            "artifact is local to another holder and is not stored by this node"
        }
        AvailabilityStatus::PubliclyAvailable => {
            "artifact is publicly advertised but is not stored by this node"
        }
        AvailabilityStatus::Replicated => {
            "replication was observed elsewhere; this node stores only the action"
        }
        AvailabilityStatus::CryptographicallyVerified => {
            "artifact was verified elsewhere; this node stores only the action"
        }
        AvailabilityStatus::OnChainAnchored => {
            "an anchor does not imply that artifact bytes are held by this node"
        }
    };
    vec![MissingArtifact {
        digest: record.availability.artifact.digest,
        declared_status: availability_name(status).into(),
        reason: format!("{}: {reason}", record.target_role),
        observed_in_action: action_digest,
        candidate_authority: crate::model::AuthorityStatus::Unresolved,
    }]
}

fn availability_name(status: AvailabilityStatus) -> &'static str {
    match status {
        AvailabilityStatus::Absent => "absent",
        AvailabilityStatus::Unknown => "unknown",
        AvailabilityStatus::IntentionallyPrivate => "intentionally_private",
        AvailabilityStatus::LocallyAvailable => "locally_available",
        AvailabilityStatus::PubliclyAvailable => "publicly_available",
        AvailabilityStatus::Replicated => "replicated",
        AvailabilityStatus::CryptographicallyVerified => "cryptographically_verified",
        AvailabilityStatus::OnChainAnchored => "on_chain_anchored",
    }
}
