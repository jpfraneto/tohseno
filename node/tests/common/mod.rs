use p256::ecdsa::signature::hazmat::PrehashSigner;
use p256::ecdsa::{Signature, SigningKey};
use tohseno_node::predict_candidate_builder_id;
use tohseno_protocol::canonical;
use tohseno_protocol::digest::{sha256, Bytes32, ShotId};
use tohseno_protocol::identity::BuilderId;
use tohseno_protocol::lineage::{LineageAction, LineagePayload, SignedLineageAction};
use tohseno_protocol::ontology::{
    ArtifactAvailability, ArtifactAvailabilityRecord, ArtifactDescriptor, AvailabilityStatus,
    Ownership, ShotCommitment, ARTIFACT_AVAILABILITY_SCHEMA, OWNERSHIP_SCHEMA,
};
use tohseno_protocol::record::CanonicalTimestamp;
use tohseno_protocol::signature::{P256PublicKey, P256Signature};

pub struct TestKey {
    signing: SigningKey,
    pub public: P256PublicKey,
    pub builder: BuilderId,
}

impl TestKey {
    pub fn new(byte: u8) -> Self {
        let signing = SigningKey::from_bytes((&[byte; 32]).into()).unwrap();
        let point = signing.verifying_key().to_encoded_point(false);
        let public = P256PublicKey {
            x: Bytes32::new(point.x().unwrap().to_vec().try_into().unwrap()),
            y: Bytes32::new(point.y().unwrap().to_vec().try_into().unwrap()),
        };
        Self {
            signing,
            builder: predict_candidate_builder_id(&public).unwrap(),
            public,
        }
    }

    pub fn sign(&self, action: LineageAction) -> SignedLineageAction {
        let digest = action.signing_digest().unwrap();
        let signature: Signature = self.signing.sign_prehash(digest.as_bytes()).unwrap();
        let signature = signature.normalize_s().unwrap_or(signature);
        action
            .attach_signature(
                self.public.clone(),
                P256Signature {
                    r: Bytes32::new(signature.r().to_bytes().into()),
                    s: Bytes32::new(signature.s().to_bytes().into()),
                },
            )
            .unwrap()
    }
}

pub fn timestamp(second: u64) -> CanonicalTimestamp {
    CanonicalTimestamp::parse(format!("2026-07-29T00:00:{second:02}Z")).unwrap()
}

pub fn root_action(
    key: &TestKey,
    shot_id: ShotId,
    handling: AvailabilityStatus,
) -> SignedLineageAction {
    let commitment = ShotCommitment::new(
        Bytes32::new([0x91; 32]),
        key.builder,
        key.public.clone(),
        timestamp(1),
    );
    key.sign(
        LineageAction::new(
            1,
            None,
            shot_id,
            key.builder,
            timestamp(1),
            handling,
            LineagePayload::Commitment(commitment),
        )
        .unwrap(),
    )
}

pub fn availability_action(
    key: &TestKey,
    root: &SignedLineageAction,
    artifact_bytes: &[u8],
    status: AvailabilityStatus,
    role: &str,
) -> SignedLineageAction {
    availability_after(key, root, artifact_bytes, status, role)
}

pub fn ownership_action(
    current: &TestKey,
    next: &TestKey,
    previous: &SignedLineageAction,
) -> SignedLineageAction {
    let sequence = previous.action.sequence + 1;
    current.sign(
        LineageAction::new(
            sequence,
            Some(previous.commitment().unwrap()),
            previous.action.shot_id,
            current.builder,
            timestamp(sequence),
            AvailabilityStatus::PubliclyAvailable,
            LineagePayload::Ownership(Ownership {
                schema: OWNERSHIP_SCHEMA.into(),
                previous_controller: current.builder,
                new_controller: next.builder,
                new_controller_key: next.public.clone(),
                reason: "Owner-authorized transfer for node policy testing.".into(),
                effective_at: timestamp(sequence),
            }),
        )
        .unwrap(),
    )
}

pub fn availability_after(
    key: &TestKey,
    previous: &SignedLineageAction,
    artifact_bytes: &[u8],
    status: AvailabilityStatus,
    role: &str,
) -> SignedLineageAction {
    availability_after_with_handling(
        key,
        previous,
        artifact_bytes,
        status,
        role,
        AvailabilityStatus::PubliclyAvailable,
    )
}

pub fn availability_after_with_handling(
    key: &TestKey,
    previous: &SignedLineageAction,
    artifact_bytes: &[u8],
    status: AvailabilityStatus,
    role: &str,
    handling: AvailabilityStatus,
) -> SignedLineageAction {
    let availability = ArtifactAvailability {
        schema: ARTIFACT_AVAILABILITY_SCHEMA.into(),
        artifact: ArtifactDescriptor {
            digest: sha256(artifact_bytes),
            media_type: "application/octet-stream".into(),
            byte_length: artifact_bytes.len().try_into().unwrap(),
            name: Some("artifact.bin".into()),
        },
        status,
        locations: Vec::new(),
    };
    key.sign(
        LineageAction::new(
            previous.action.sequence + 1,
            Some(previous.commitment().unwrap()),
            previous.action.shot_id,
            key.builder,
            timestamp(previous.action.sequence + 1),
            handling,
            LineagePayload::ArtifactAvailability(ArtifactAvailabilityRecord {
                target_role: role.into(),
                availability,
                observed_at: timestamp(previous.action.sequence + 1),
            }),
        )
        .unwrap(),
    )
}

pub fn bytes(action: &SignedLineageAction) -> Vec<u8> {
    canonical::to_vec(action).unwrap()
}
