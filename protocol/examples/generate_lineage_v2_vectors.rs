use p256::ecdsa::signature::hazmat::PrehashSigner;
use p256::ecdsa::{Signature, SigningKey};
use serde_json::json;
use tohseno_protocol::digest::{sha256, Address20, Bytes32, ShotId};
use tohseno_protocol::identity::BuilderId;
use tohseno_protocol::lineage::{LineageAction, LineagePayload};
use tohseno_protocol::ontology::{
    ArtifactAvailability, ArtifactAvailabilityRecord, ArtifactDescriptor, ArtifactLocation,
    ArtifactLocationKind, AvailabilityStatus, ShotCommitment, ARTIFACT_AVAILABILITY_SCHEMA,
};
use tohseno_protocol::record::CanonicalTimestamp;
use tohseno_protocol::signature::{P256PublicKey, P256Signature};

fn main() {
    let signing = SigningKey::from_bytes((&[1_u8; 32]).into()).unwrap();
    let point = signing.verifying_key().to_encoded_point(false);
    let x: [u8; 32] = point.x().unwrap().to_vec().try_into().unwrap();
    let y: [u8; 32] = point.y().unwrap().to_vec().try_into().unwrap();
    let public = P256PublicKey {
        x: Bytes32::new(x),
        y: Bytes32::new(y),
    };
    let builder = BuilderId::new(Address20::from_bytes([0x11; 20]));
    let shot_id = ShotId::from_bytes([0x22; 32]);
    let first_time = CanonicalTimestamp::parse("2026-07-29T00:00:01Z").unwrap();
    let commitment = ShotCommitment::new(
        sha256(b"{\"preserved_intention\":\"Build a quiet notebook.\"}"),
        builder,
        public.clone(),
        first_time.clone(),
    );
    let first = sign(
        &signing,
        LineageAction::new(
            1,
            None,
            shot_id,
            builder,
            first_time,
            AvailabilityStatus::PubliclyAvailable,
            LineagePayload::Commitment(commitment),
        )
        .unwrap(),
        &public,
    );
    let first_commitment = first.commitment().unwrap();
    let artifact_bytes = b"public expression blueprint";
    let second = sign(
        &signing,
        LineageAction::new(
            2,
            Some(first_commitment),
            shot_id,
            builder,
            CanonicalTimestamp::parse("2026-07-29T00:00:02Z").unwrap(),
            AvailabilityStatus::PubliclyAvailable,
            LineagePayload::ArtifactAvailability(ArtifactAvailabilityRecord {
                target_role: "expression_definition".into(),
                availability: ArtifactAvailability {
                    schema: ARTIFACT_AVAILABILITY_SCHEMA.into(),
                    artifact: ArtifactDescriptor {
                        digest: sha256(artifact_bytes),
                        media_type: "application/json".into(),
                        byte_length: artifact_bytes.len().try_into().unwrap(),
                        name: Some("expression.json".into()),
                    },
                    status: AvailabilityStatus::PubliclyAvailable,
                    locations: vec![ArtifactLocation {
                        kind: ArtifactLocationKind::ContentAddress,
                        value: format!("sha256:{}", sha256(artifact_bytes)),
                    }],
                },
                observed_at: CanonicalTimestamp::parse("2026-07-29T00:00:02Z").unwrap(),
            }),
        )
        .unwrap(),
        &public,
    );
    let second_commitment = second.commitment().unwrap();
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "schema": "tohseno.lineage-test-vectors/2",
            "actions": [first, second],
            "commitments": [first_commitment, second_commitment]
        }))
        .unwrap()
    );
}

fn sign(
    signing: &SigningKey,
    action: LineageAction,
    public: &P256PublicKey,
) -> tohseno_protocol::lineage::SignedLineageAction {
    let digest = action.signing_digest().unwrap();
    let signature: Signature = signing.sign_prehash(digest.as_bytes()).unwrap();
    let signature = signature.normalize_s().unwrap_or(signature);
    action
        .attach_signature(
            public.clone(),
            P256Signature {
                r: Bytes32::new(signature.r().to_bytes().into()),
                s: Bytes32::new(signature.s().to_bytes().into()),
            },
        )
        .unwrap()
}
