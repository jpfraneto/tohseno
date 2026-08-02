use p256::ecdsa::signature::hazmat::PrehashSigner;
use p256::ecdsa::{Signature, SigningKey};
use sha2::{Digest as _, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use tohseno_protocol::canonical;
use tohseno_protocol::contract_activation::{
    release_authority_key_id, ActivatedContract, ChainBlock, ContractActivation,
    DeploymentObservation, ReleaseAuthority, ReleaseAuthorityApproval, ReleaseAuthorityPolicy,
    ReleaseAuthorityPurpose, SignedContractActivation, CONTRACT_ACTIVATION_PROTOCOL,
    CONTRACT_ACTIVATION_SCHEMA, CONTRACT_ACTIVATION_SIGNING_DOMAIN,
    RELEASE_AUTHORITY_POLICY_SCHEMA, SIGNED_CONTRACT_ACTIVATION_SCHEMA,
};
use tohseno_protocol::contract_generation::ContractGeneration;
use tohseno_protocol::digest::Bytes32;
use tohseno_protocol::identity::device_key_id;
use tohseno_protocol::record::CanonicalTimestamp;
use tohseno_protocol::signature::{
    DetachedP256Signature, P256PublicKey, P256Signature, SignatureAlgorithm,
};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn generation() -> ContractGeneration {
    canonical::from_slice(
        &fs::read(repository_root().join("contracts/generations/0.8.0/generation.json")).unwrap(),
    )
    .unwrap()
}

fn policy_and_keys() -> (ReleaseAuthorityPolicy, Vec<(Bytes32, SigningKey)>) {
    let mut pairs = [1_u8, 2, 3]
        .into_iter()
        .map(|scalar| {
            let key = SigningKey::from_bytes((&[scalar; 32]).into()).unwrap();
            let public_key = public_key(&key);
            let authority = ReleaseAuthority::from_public_key(public_key).unwrap();
            (authority, key)
        })
        .collect::<Vec<_>>();
    pairs.sort_by_key(|(authority, _)| authority.key_id);
    let keys = pairs
        .iter()
        .map(|(authority, key)| (authority.key_id, key.clone()))
        .collect();
    let policy = ReleaseAuthorityPolicy {
        schema: RELEASE_AUTHORITY_POLICY_SCHEMA.into(),
        protocol: CONTRACT_ACTIVATION_PROTOCOL.into(),
        protocol_major: 2,
        purpose: ReleaseAuthorityPurpose::ContractGenerationActivation,
        threshold: 2,
        authorities: pairs.into_iter().map(|(authority, _)| authority).collect(),
        issued_at: CanonicalTimestamp::parse("2026-07-30T20:00:00Z").unwrap(),
    };
    policy.validate().unwrap();
    (policy, keys)
}

fn activation(
    generation: &ContractGeneration,
    policy: &ReleaseAuthorityPolicy,
) -> ContractActivation {
    ContractActivation {
        schema: CONTRACT_ACTIVATION_SCHEMA.into(),
        protocol: CONTRACT_ACTIVATION_PROTOCOL.into(),
        protocol_major: 2,
        generation: generation.generation.clone(),
        activation_sequence: 1,
        previous_activation: None,
        generation_definition_sha256: generation.digest().unwrap(),
        authority_policy_sha256: policy.digest().unwrap(),
        chain_id: generation.chain.chain_id,
        builder_account_runtime_keccak256: generation
            .contracts
            .builder_account
            .runtime_code_keccak256,
        factory: ActivatedContract {
            address: generation.create2.builder_account_factory.predicted_address,
            runtime_code_keccak256: generation
                .contracts
                .builder_account_factory
                .runtime_code_keccak256,
            deployment: DeploymentObservation {
                transaction_hash: Bytes32::new([0x11; 32]),
                block_number: 100,
                block_hash: Bytes32::new([0x12; 32]),
            },
        },
        registry: ActivatedContract {
            address: generation.create2.shot_registry.predicted_address,
            runtime_code_keccak256: generation.contracts.shot_registry.runtime_code_keccak256,
            deployment: DeploymentObservation {
                transaction_hash: Bytes32::new([0x21; 32]),
                block_number: 101,
                block_hash: Bytes32::new([0x22; 32]),
            },
        },
        activation_block: ChainBlock {
            block_number: 102,
            block_hash: Bytes32::new([0x31; 32]),
        },
        p256_probe_sha256: Bytes32::new([0x41; 32]),
        issued_at: CanonicalTimestamp::parse("2026-07-30T21:00:00Z").unwrap(),
    }
}

fn signed(
    activation: ContractActivation,
    keys: &[(Bytes32, SigningKey)],
) -> SignedContractActivation {
    let digest = activation.signing_digest().unwrap();
    let approvals = keys
        .iter()
        .take(2)
        .map(|(key_id, key)| ReleaseAuthorityApproval {
            key_id: *key_id,
            authorization: DetachedP256Signature {
                algorithm: SignatureAlgorithm::P256,
                digest,
                signature: sign(key, digest),
                low_s: true,
            },
        })
        .collect();
    SignedContractActivation {
        schema: SIGNED_CONTRACT_ACTIVATION_SCHEMA.into(),
        activation,
        approvals,
    }
}

#[test]
fn threshold_activation_binds_generation_policy_and_exact_chain_evidence() {
    let generation = generation();
    let (policy, keys) = policy_and_keys();
    let activation = activation(&generation, &policy);
    activation.validate().unwrap();
    activation.validate_against_generation(&generation).unwrap();
    let signed = signed(activation, &keys);
    signed.verify_for_generation(&policy, &generation).unwrap();

    let canonical = canonical::to_vec(&signed.activation).unwrap();
    let mut preimage = Vec::from(CONTRACT_ACTIVATION_SIGNING_DOMAIN);
    preimage.extend_from_slice(&canonical);
    assert_eq!(
        signed.activation.signing_digest().unwrap(),
        Bytes32::new(Sha256::digest(preimage).into())
    );
}

#[test]
fn release_authority_is_cryptographically_distinct_from_builder_devices() {
    let (policy, _) = policy_and_keys();
    for authority in &policy.authorities {
        assert_eq!(
            authority.key_id,
            release_authority_key_id(&authority.public_key)
        );
        assert_ne!(authority.key_id, device_key_id(&authority.public_key));
    }
    assert_eq!(
        policy.digest().unwrap(),
        canonical::sha256_commitment(&policy).unwrap()
    );
}

#[test]
fn threshold_unknown_key_replay_and_tampering_fail() {
    let generation = generation();
    let (policy, keys) = policy_and_keys();
    let original = signed(activation(&generation, &policy), &keys);
    original.verify(&policy).unwrap();

    let mut insufficient = original.clone();
    insufficient.approvals.pop();
    assert!(insufficient.verify(&policy).is_err());

    let mut duplicate = original.clone();
    duplicate.approvals[1] = duplicate.approvals[0].clone();
    assert!(duplicate.verify(&policy).is_err());

    let mut reordered = original.clone();
    reordered.approvals.swap(0, 1);
    assert!(reordered.verify(&policy).is_err());

    let mut unknown = original.clone();
    unknown.approvals[0].key_id = Bytes32::new([0xaa; 32]);
    assert!(unknown.verify(&policy).is_err());

    let mut tampered = original.clone();
    tampered.activation.activation_block.block_number += 1;
    assert!(tampered.verify(&policy).is_err());

    let mut wrong_policy = policy.clone();
    wrong_policy.threshold = 1;
    assert!(original.verify(&wrong_policy).is_err());
}

#[test]
fn generation_and_successor_mismatches_fail_closed() {
    let generation = generation();
    let (policy, keys) = policy_and_keys();
    let first = activation(&generation, &policy);

    let mut instantiated_runtime = first.clone();
    instantiated_runtime.builder_account_runtime_keccak256 = Bytes32::new([0xaa; 32]);
    instantiated_runtime.registry.runtime_code_keccak256 = Bytes32::new([0xbb; 32]);
    instantiated_runtime
        .validate_against_generation(&generation)
        .unwrap();

    let mut wrong_runtime = first.clone();
    wrong_runtime.factory.runtime_code_keccak256 = Bytes32::new([0xbb; 32]);
    assert!(wrong_runtime
        .validate_against_generation(&generation)
        .is_err());
    let wrong_runtime_signed = signed(wrong_runtime, &keys);
    assert!(wrong_runtime_signed
        .verify_for_generation(&policy, &generation)
        .is_err());

    let mut wrong_definition = first.clone();
    wrong_definition.generation_definition_sha256 = Bytes32::new([0xcc; 32]);
    assert!(wrong_definition
        .validate_against_generation(&generation)
        .is_err());

    let mut second = first.clone();
    second.activation_sequence = 2;
    second.previous_activation = Some(first.signing_digest().unwrap());
    second.activation_block.block_number += 1;
    second.activation_block.block_hash = Bytes32::new([0x32; 32]);
    second.issued_at = CanonicalTimestamp::parse("2026-07-30T22:00:00Z").unwrap();
    second.validate_successor_of(&first).unwrap();

    second.previous_activation = Some(Bytes32::new([0xdd; 32]));
    assert!(second.validate_successor_of(&first).is_err());
}

#[test]
fn the_committed_production_activation_verifies_under_the_committed_trust_root() {
    // The 2026-08-02 owner ceremony committed the production instances; this
    // test permanently proves their internal consistency from raw bytes.
    let directory = repository_root().join("release/contract-activations");
    let mut entries = fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    entries.sort();
    assert_eq!(
        entries,
        vec![
            "OWNER_CANARY_WAIVER.md",
            "OWNER_POLICY_APPROVAL.md",
            "README.md",
            "independent-verification-1.json",
            "p256-probe-20260802T013802Z.json",
            "release-authority-policy.json",
            "signed-contract-activation-1.json",
        ]
    );

    let committed_policy: ReleaseAuthorityPolicy =
        canonical::from_slice(&fs::read(directory.join("release-authority-policy.json")).unwrap())
            .unwrap();
    assert_eq!(
        committed_policy.digest().unwrap().to_string(),
        "0xf14410692ebe34f6855b8dbec5cb08733aa737f1cd86f385694e4fb575df943c"
    );
    let committed_signed: SignedContractActivation = canonical::from_slice(
        &fs::read(directory.join("signed-contract-activation-1.json")).unwrap(),
    )
    .unwrap();
    committed_signed
        .verify_for_generation(&committed_policy, &generation())
        .unwrap();
    assert_eq!(committed_signed.activation.activation_sequence, 1);
    let probe_bytes = fs::read(directory.join("p256-probe-20260802T013802Z.json")).unwrap();
    let probe_digest: [u8; 32] = Sha256::digest(&probe_bytes).into();
    assert_eq!(
        committed_signed.activation.p256_probe_sha256.as_bytes(),
        &probe_digest
    );

    let committed_value = serde_json::to_value(&committed_signed.activation).unwrap();
    for field in [
        "private_key",
        "mnemonic",
        "builder_id",
        "shot_id",
        "installation_id",
    ] {
        assert!(!contains_key(&committed_value, field));
    }

    let activation = serde_json::to_value(activation(&generation(), &policy_and_keys().0)).unwrap();
    let forbidden = [
        "private_key",
        "mnemonic",
        "builder_id",
        "shot_id",
        "installation_id",
    ];
    for field in forbidden {
        assert!(!contains_key(&activation, field));
    }

    let mut unknown = activation;
    unknown
        .as_object_mut()
        .unwrap()
        .insert("trusted".into(), serde_json::Value::Bool(true));
    assert!(
        canonical::from_slice::<ContractActivation>(&serde_json::to_vec(&unknown).unwrap())
            .is_err()
    );
}

fn public_key(signing_key: &SigningKey) -> P256PublicKey {
    let point = signing_key.verifying_key().to_encoded_point(false);
    let copy = |bytes: &[u8]| {
        let mut value = [0_u8; 32];
        value.copy_from_slice(bytes);
        Bytes32::new(value)
    };
    P256PublicKey {
        x: copy(point.x().unwrap()),
        y: copy(point.y().unwrap()),
    }
}

fn sign(signing_key: &SigningKey, digest: Bytes32) -> P256Signature {
    let signature: Signature = signing_key.sign_prehash(digest.as_bytes()).unwrap();
    let signature = signature.normalize_s().unwrap_or(signature);
    P256Signature {
        r: Bytes32::new(signature.r().to_bytes().into()),
        s: Bytes32::new(signature.s().to_bytes().into()),
    }
}

fn contains_key(value: &serde_json::Value, key: &str) -> bool {
    match value {
        serde_json::Value::Object(object) => {
            object.contains_key(key) || object.values().any(|value| contains_key(value, key))
        }
        serde_json::Value::Array(values) => values.iter().any(|value| contains_key(value, key)),
        _ => false,
    }
}
