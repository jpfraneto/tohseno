use serde::de::DeserializeOwned;
use serde_json::Value;
use tohseno_protocol::actions::{
    type_hash, Eip712Domain, PublicAction, RegistryActionV2, ShotRegistrationCommitmentV2,
    SignedPublicAction, SignedRegistryActionV2, APPEND_CHECKPOINT_V2_TYPE, REGISTER_SHOT_V2_TYPE,
    SHOT_REGISTRATION_COMMITMENT_V2_TYPE, TRANSFER_SHOT_V2_TYPE,
};
use tohseno_protocol::canonical;
use tohseno_protocol::digest::{Address20, Bytes32, ShotId};
use tohseno_protocol::signature::{decode_compact, verify_digest, P256Signature};

fn vectors() -> Value {
    serde_json::from_str(include_str!("../test-vectors/registry-v2.json")).unwrap()
}

fn decode<T: DeserializeOwned>(value: &Value) -> T {
    serde_json::from_value(value.clone()).unwrap()
}

fn bytes(value: &Value) -> Bytes32 {
    decode(value)
}

#[test]
fn frozen_registry_v2_vectors_match_the_exact_contract_words() {
    let vectors = vectors();
    assert_eq!(vectors["schema"], "tohseno.registry-v2-test-vectors/1");
    assert_eq!(vectors["contract_generation"], "0.8.0");
    assert_eq!(vectors["commit_window"]["minimum_age_seconds"], 60);
    assert_eq!(vectors["commit_window"]["maximum_age_seconds"], 86_400);
    assert_eq!(vectors["commit_window"]["inclusive"], true);

    for (type_string, expected) in vectors["type_hashes"].as_object().unwrap() {
        assert_eq!(type_hash(type_string), bytes(expected));
    }
    assert_eq!(
        type_hash(SHOT_REGISTRATION_COMMITMENT_V2_TYPE),
        bytes(&vectors["type_hashes"][SHOT_REGISTRATION_COMMITMENT_V2_TYPE])
    );
    assert_eq!(
        type_hash(REGISTER_SHOT_V2_TYPE),
        bytes(&vectors["type_hashes"][REGISTER_SHOT_V2_TYPE])
    );
    assert_eq!(
        type_hash(APPEND_CHECKPOINT_V2_TYPE),
        bytes(&vectors["type_hashes"][APPEND_CHECKPOINT_V2_TYPE])
    );
    assert_eq!(
        type_hash(TRANSFER_SHOT_V2_TYPE),
        bytes(&vectors["type_hashes"][TRANSFER_SHOT_V2_TYPE])
    );

    let domain: Eip712Domain = decode(&vectors["domain"]["value"]);
    assert_eq!(domain.separator(), bytes(&vectors["domain"]["separator"]));
    let commitment: ShotRegistrationCommitmentV2 =
        decode(&vectors["registration_commitment"]["value"]);
    assert_eq!(
        commitment.commitment().unwrap(),
        bytes(&vectors["registration_commitment"]["hash"])
    );

    for name in ["register_shot", "append_checkpoint", "transfer_shot"] {
        let vector = &vectors["actions"][name];
        let action: RegistryActionV2 = decode(&vector["value"]);
        assert_eq!(
            action.type_string(),
            vector["type_string"].as_str().unwrap()
        );
        assert_eq!(action.struct_hash().unwrap(), bytes(&vector["struct_hash"]));
        let digest = bytes(&vector["digest"]);
        assert_eq!(action.digest(&domain).unwrap(), digest);
        let signature: P256Signature = decode(&vector["signature"]);
        let signed: SignedRegistryActionV2 = decode(&vector["signed"]);
        signed.verify().unwrap();
        verify_digest(&signed.signer, digest, &signature).unwrap();

        let compact =
            decode_hex(vector["compact_signature_hex"].as_str().unwrap()).expect("compact hex");
        assert_eq!(
            decode_compact(&compact).unwrap(),
            (signed.signer.clone(), signature)
        );
    }

    let register: RegistryActionV2 = decode(&vectors["actions"]["register_shot"]["value"]);
    assert_eq!(
        register
            .registration_commitment(&domain)
            .unwrap()
            .commitment()
            .unwrap(),
        commitment.commitment().unwrap()
    );
}

#[test]
fn registry_v2_commitment_binds_every_supported_coordinate_and_rejects_wrong_chain() {
    let vectors = vectors();
    let original: ShotRegistrationCommitmentV2 =
        decode(&vectors["registration_commitment"]["value"]);
    let expected = original.commitment().unwrap();

    let mut changed = original.clone();
    changed.controller = Address20::from_bytes([0x89; 20]);
    assert_ne!(changed.commitment().unwrap(), expected);
    changed = original.clone();
    changed.shot_id = ShotId::from_bytes([0x12; 32]);
    assert_ne!(changed.commitment().unwrap(), expected);
    changed = original.clone();
    changed.salt = Bytes32::new([0x34; 32]);
    assert_ne!(changed.commitment().unwrap(), expected);
    changed = original.clone();
    changed.registry = Address20::from_bytes([0x67; 20]);
    assert_ne!(changed.commitment().unwrap(), expected);
    changed = original.clone();
    changed.deadline += 1;
    assert_ne!(changed.commitment().unwrap(), expected);

    changed = original;
    changed.chain_id += 1;
    assert!(changed.commitment().is_err());
}

#[test]
fn registry_v2_tampering_downgrade_and_generation_confusion_fail() {
    let vectors = vectors();
    let signed_value = &vectors["actions"]["register_shot"]["signed"];
    let signed: SignedRegistryActionV2 = decode(signed_value);
    signed.verify().unwrap();

    let mut tampered = signed.clone();
    if let RegistryActionV2::RegisterShot { head, .. } = &mut tampered.action {
        *head = Bytes32::new([0x55; 32]);
    }
    assert!(tampered.verify().is_err());

    let mut wrong_digest = signed.clone();
    wrong_digest.authorization.digest = Bytes32::new([0xaa; 32]);
    assert!(wrong_digest.verify().is_err());

    let mut wrong_version = signed.clone();
    wrong_version.domain.version = "1".into();
    assert!(wrong_version.verify().is_err());

    let mut wrong_chain = signed.clone();
    wrong_chain.domain.chain_id += 1;
    assert!(wrong_chain.verify().is_err());

    let mut wrong_schema = signed.clone();
    wrong_schema.schema = "tohseno.registry-action/1".into();
    assert!(wrong_schema.verify().is_err());

    let encoded = serde_json::to_vec(signed_value).unwrap();
    assert!(
        canonical::from_slice::<SignedPublicAction>(&encoded).is_err(),
        "a successor action was accepted as a frozen v0.7 public action"
    );
    let frozen_v1: Value =
        serde_json::from_str(include_str!("../test-vectors/protocol-v1.json")).unwrap();
    let v1_create = &frozen_v1["eip712"]["create_shot"]["value"];
    let v1_bytes = serde_json::to_vec(v1_create).unwrap();
    assert!(
        canonical::from_slice::<RegistryActionV2>(&v1_bytes).is_err(),
        "a frozen v0.7 action was accepted as a successor action"
    );
    let _: PublicAction = decode(v1_create);

    let duplicate = br#"{"schema":"tohseno.registry-action/2","schema":"tohseno.registry-action/2","domain":{},"action":{},"signer":{},"authorization":{}}"#;
    assert!(canonical::from_slice::<SignedRegistryActionV2>(duplicate).is_err());
    let unknown = serde_json::to_vec(&serde_json::json!({
        "schema": signed.schema,
        "domain": signed.domain,
        "action": signed.action,
        "signer": signed.signer,
        "authorization": signed.authorization,
        "unexpected": true
    }))
    .unwrap();
    assert!(canonical::from_slice::<SignedRegistryActionV2>(&unknown).is_err());
}

#[test]
fn registry_v2_rejects_structurally_impossible_actions() {
    let shot_id = ShotId::from_bytes([1; 32]);
    let address = Address20::from_bytes([2; 20]);
    let head = Bytes32::new([3; 32]);
    let salt = Bytes32::new([4; 32]);

    let register = |deadline| RegistryActionV2::RegisterShot {
        shot_id,
        controller: address,
        head,
        salt,
        nonce: 0,
        deadline,
    };
    assert!(register(1).validate().is_ok());
    assert!(register(0).validate().is_err());

    let same_head = RegistryActionV2::AppendCheckpoint {
        shot_id,
        previous_head: head,
        new_head: head,
        checkpoint_sequence: 2,
        nonce: 1,
        deadline: 1,
    };
    assert!(same_head.validate().is_err());

    let first_checkpoint_append = RegistryActionV2::AppendCheckpoint {
        shot_id,
        previous_head: head,
        new_head: Bytes32::new([5; 32]),
        checkpoint_sequence: 1,
        nonce: 1,
        deadline: 1,
    };
    assert!(first_checkpoint_append.validate().is_err());

    let same_controller = RegistryActionV2::TransferShot {
        shot_id,
        current_controller: address,
        new_controller: address,
        current_head: head,
        checkpoint_sequence: 1,
        nonce: 1,
        deadline: 1,
    };
    assert!(same_controller.validate().is_err());
}

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    let value = value.strip_prefix("0x")?;
    if value.len() % 2 != 0 {
        return None;
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = nibble(pair[0])?;
            let low = nibble(pair[1])?;
            Some((high << 4) | low)
        })
        .collect()
}

fn nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}
