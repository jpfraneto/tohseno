use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::BTreeSet;
use tohseno_protocol::actions::{
    Eip712Domain, RegistryActionV2, SHOT_REGISTRY_DOMAIN, SHOT_REGISTRY_V2_EIP712_VERSION,
};
use tohseno_protocol::canonical;
use tohseno_protocol::digest::{Address20, Bytes32, ShotId};
use tohseno_protocol::identity::ROBINHOOD_CHAIN_ID;
use tohseno_protocol::public_checkpoint::{
    verify_public_checkpoint_segment, PublicCheckpoint, PublicCheckpointAnchor,
    PublicCheckpointWitness, PUBLIC_CHECKPOINT_CONTRACT_GENERATION,
};
use tohseno_protocol::record::CanonicalTimestamp;

fn vectors() -> Value {
    serde_json::from_str(include_str!("../test-vectors/public-checkpoint.json")).unwrap()
}

fn decode<T: DeserializeOwned>(value: &Value) -> T {
    serde_json::from_value(value.clone()).unwrap()
}

fn bytes(value: &Value) -> Bytes32 {
    decode(value)
}

#[test]
fn frozen_public_checkpoint_bytes_form_a_complete_public_only_chain() {
    let vectors = vectors();
    assert_eq!(
        vectors["schema"],
        "tohseno.public-checkpoint-test-vectors/1"
    );
    let checkpoints = vectors["checkpoints"]
        .as_array()
        .unwrap()
        .iter()
        .map(|vector| {
            let checkpoint: PublicCheckpoint = decode(&vector["value"]);
            checkpoint.validate().unwrap();
            assert_eq!(
                canonical::to_string(&checkpoint).unwrap(),
                vector["rfc8785"].as_str().unwrap()
            );
            assert_eq!(checkpoint.commitment().unwrap(), bytes(&vector["sha256"]));
            checkpoint
        })
        .collect::<Vec<_>>();

    let verified = verify_public_checkpoint_segment(&checkpoints, None).unwrap();
    assert!(verified.complete_from_registration);
    assert_eq!(verified.first_checkpoint_sequence, 1);
    assert_eq!(verified.last_checkpoint_sequence, 2);
    assert_eq!(
        verified.head,
        checkpoints.last().unwrap().commitment().unwrap()
    );

    let partial = verify_public_checkpoint_segment(&checkpoints[1..], None).unwrap();
    assert!(!partial.complete_from_registration);
    let anchor = PublicCheckpointAnchor {
        witness: checkpoints[0].witness.clone(),
        shot_id: checkpoints[0].shot_id,
        checkpoint_sequence: 1,
        head: checkpoints[0].commitment().unwrap(),
        issued_at: checkpoints[0].issued_at.clone(),
    };
    let anchored = verify_public_checkpoint_segment(&checkpoints[1..], Some(&anchor)).unwrap();
    assert!(!anchored.complete_from_registration);
    assert_eq!(anchored.head, verified.head);
}

#[test]
fn public_checkpoint_binds_exact_registry_actions_without_local_lineage() {
    let vectors = vectors();
    let first: PublicCheckpoint = decode(&vectors["checkpoints"][0]["value"]);
    let second: PublicCheckpoint = decode(&vectors["checkpoints"][1]["value"]);
    let domain = Eip712Domain {
        name: SHOT_REGISTRY_DOMAIN.into(),
        version: SHOT_REGISTRY_V2_EIP712_VERSION.into(),
        chain_id: ROBINHOOD_CHAIN_ID,
        verifying_contract: first.witness.registry,
    };
    let register = RegistryActionV2::RegisterShot {
        shot_id: first.shot_id,
        controller: Address20::from_bytes([0x88; 20]),
        head: first.commitment().unwrap(),
        salt: Bytes32::new([0x33; 32]),
        nonce: 0,
        deadline: 2_000_000_000,
    };
    first.validate_registry_action(&register, &domain).unwrap();

    let append = RegistryActionV2::AppendCheckpoint {
        shot_id: second.shot_id,
        previous_head: first.commitment().unwrap(),
        new_head: second.commitment().unwrap(),
        checkpoint_sequence: 2,
        nonce: 1,
        deadline: 2_000_000_100,
    };
    second.validate_registry_action(&append, &domain).unwrap();

    let mut wrong_head = append.clone();
    if let RegistryActionV2::AppendCheckpoint { new_head, .. } = &mut wrong_head {
        *new_head = Bytes32::new([0xaa; 32]);
    }
    assert!(second
        .validate_registry_action(&wrong_head, &domain)
        .is_err());

    let mut wrong_domain = domain.clone();
    wrong_domain.verifying_contract = Address20::from_bytes([0x67; 20]);
    assert!(second
        .validate_registry_action(&append, &wrong_domain)
        .is_err());

    let transfer = RegistryActionV2::TransferShot {
        shot_id: second.shot_id,
        current_controller: Address20::from_bytes([0x88; 20]),
        new_controller: Address20::from_bytes([0x99; 20]),
        current_head: second.commitment().unwrap(),
        checkpoint_sequence: 2,
        nonce: 2,
        deadline: 2_000_000_200,
    };
    assert!(second.validate_registry_action(&transfer, &domain).is_err());
}

#[test]
fn public_checkpoint_rejects_forks_gaps_replay_and_witness_changes() {
    let vectors = vectors();
    let first: PublicCheckpoint = decode(&vectors["checkpoints"][0]["value"]);
    let second: PublicCheckpoint = decode(&vectors["checkpoints"][1]["value"]);

    let mut gap = second.clone();
    gap.checkpoint_sequence = 3;
    assert!(verify_public_checkpoint_segment(&[first.clone(), gap], None).is_err());

    let mut wrong_previous = second.clone();
    wrong_previous.previous_checkpoint = Some(Bytes32::new([0xaa; 32]));
    assert!(verify_public_checkpoint_segment(&[first.clone(), wrong_previous], None).is_err());

    let mut wrong_shot = second.clone();
    wrong_shot.shot_id = ShotId::from_bytes([0x12; 32]);
    assert!(verify_public_checkpoint_segment(&[first.clone(), wrong_shot], None).is_err());

    let mut wrong_witness = second.clone();
    wrong_witness.witness.registry = Address20::from_bytes([0x67; 20]);
    assert!(verify_public_checkpoint_segment(&[first.clone(), wrong_witness], None).is_err());

    let mut backwards = second;
    backwards.issued_at = CanonicalTimestamp::parse("2026-07-30T11:59:59Z").unwrap();
    assert!(verify_public_checkpoint_segment(&[first, backwards], None).is_err());
}

#[test]
fn public_checkpoint_shape_cannot_carry_private_or_runtime_commitments() {
    let vectors = vectors();
    let value = &vectors["checkpoints"][0]["value"];
    let keys = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        keys,
        BTreeSet::from([
            "checkpoint_sequence",
            "issued_at",
            "previous_checkpoint",
            "protocol",
            "protocol_version",
            "schema",
            "scope",
            "shot_id",
            "witness",
        ])
    );
    let witness_keys = value["witness"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        witness_keys,
        BTreeSet::from(["chain_id", "contract_generation", "registry"])
    );

    // This guard is intentionally architectural: the public checkpoint module
    // must never grow a dependency on private lineage or app-runtime records.
    let source = include_str!("../src/public_checkpoint.rs");
    for forbidden in [
        "ContinuityStatement",
        "Feedback",
        "IntentionRecord",
        "LineageAction",
        "VersionId",
        "ExpressionId",
        "payload_digest",
        "content_commitment",
    ] {
        assert!(
            !source.contains(forbidden),
            "public checkpoint source depends on forbidden private/runtime concept {forbidden}"
        );
    }

    let mut unknown = value.clone();
    unknown
        .as_object_mut()
        .unwrap()
        .insert("content_commitment".into(), Value::String("0x00".into()));
    assert!(
        canonical::from_slice::<PublicCheckpoint>(&serde_json::to_vec(&unknown).unwrap()).is_err()
    );
    let duplicate = br#"{"protocol":"tohseno","protocol":"tohseno"}"#;
    assert!(canonical::from_slice::<PublicCheckpoint>(duplicate).is_err());
}

#[test]
fn public_checkpoint_constructor_accepts_only_the_frozen_witness_generation() {
    let witness = PublicCheckpointWitness {
        contract_generation: PUBLIC_CHECKPOINT_CONTRACT_GENERATION.into(),
        chain_id: ROBINHOOD_CHAIN_ID,
        registry: Address20::from_bytes([0x66; 20]),
    };
    PublicCheckpoint::new(
        witness.clone(),
        ShotId::from_bytes([1; 32]),
        1,
        None,
        CanonicalTimestamp::parse("2026-07-30T12:00:00Z").unwrap(),
    )
    .unwrap();

    let mut unknown = witness;
    unknown.contract_generation = "next".into();
    assert!(PublicCheckpoint::new(
        unknown,
        ShotId::from_bytes([1; 32]),
        1,
        None,
        CanonicalTimestamp::parse("2026-07-30T12:00:00Z").unwrap(),
    )
    .is_err());
}
