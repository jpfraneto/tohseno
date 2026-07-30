use serde_json::Value;
use sha2::{Digest as _, Sha256};
use sha3::Keccak256;
use std::fs;
use std::path::{Path, PathBuf};
use tohseno_protocol::canonical;
use tohseno_protocol::contract_generation::{
    contract_source_tree_digest, predict_create2_address, ContractGeneration,
    CONTRACT_GENERATION_SCHEMA, CONTRACT_SOURCE_TREE_LAW, EIP7951_GAS,
};
use tohseno_protocol::digest::Bytes32;

fn vectors() -> Value {
    serde_json::from_str(include_str!("../test-vectors/contract-generation-v1.json")).unwrap()
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn generation_path() -> PathBuf {
    repository_root().join("contracts/generations/0.8.0")
}

fn decode<T: serde::de::DeserializeOwned>(value: &Value) -> T {
    serde_json::from_value(value.clone()).unwrap()
}

#[test]
fn frozen_generation_definition_has_exact_canonical_bytes_and_digest() {
    let vectors = vectors();
    assert_eq!(
        vectors["schema"],
        "tohseno.contract-generation-test-vector/1"
    );
    let definition: ContractGeneration = decode(&vectors["definition"]);
    definition.validate().unwrap();
    assert_eq!(definition.schema, CONTRACT_GENERATION_SCHEMA);
    assert_eq!(definition.generation, "0.8.0");
    assert_eq!(definition.chain.chain_id, 4663);
    assert_eq!(definition.chain.p256_verifier.gas, EIP7951_GAS);
    assert_eq!(
        definition.source.commit,
        "862ca6cd3d396271b56b336fee0513ddcf6ecc64"
    );
    assert_eq!(definition.source.tree_law, CONTRACT_SOURCE_TREE_LAW);

    assert_eq!(
        canonical::to_string(&definition).unwrap(),
        vectors["rfc8785"].as_str().unwrap()
    );
    let expected_digest: Bytes32 = decode(&vectors["definition_digest"]);
    assert_eq!(definition.digest().unwrap(), expected_digest);

    assert_eq!(
        Bytes32::new(Sha256::digest(vectors["rfc8785"].as_str().unwrap().as_bytes()).into()),
        expected_digest
    );

    let committed: Value =
        serde_json::from_slice(&fs::read(generation_path().join("generation.json")).unwrap())
            .unwrap();
    assert_eq!(committed, vectors["definition"]);
}

#[test]
fn every_declared_source_and_portable_artifact_matches_disk() {
    let definition: ContractGeneration = decode(&vectors()["definition"]);
    let contracts = repository_root().join("contracts");
    for source in &definition.source.files {
        assert_artifact(
            &contracts.join(&source.path),
            source.sha256,
            source.byte_length,
        );
    }
    assert_eq!(
        contract_source_tree_digest(&definition.source.files).unwrap(),
        definition.source.tree_sha256
    );

    let generation = generation_path();
    let artifacts = [
        &definition.contracts.builder_account.abi,
        &definition.contracts.builder_account_factory.abi,
        &definition.contracts.shot_registry.abi,
        definition
            .contracts
            .builder_account
            .creation_bytecode
            .as_ref()
            .unwrap(),
    ];
    for artifact in artifacts {
        assert_artifact(
            &generation.join(&artifact.path),
            artifact.sha256,
            artifact.byte_length,
        );
    }

    let creation_path = generation.join(
        &definition
            .contracts
            .builder_account
            .creation_bytecode
            .as_ref()
            .unwrap()
            .path,
    );
    let encoded = fs::read_to_string(creation_path).unwrap();
    let raw = decode_hex(encoded.trim()).unwrap();
    assert_eq!(
        Bytes32::new(Keccak256::digest(raw).into()),
        definition.contracts.builder_account.creation_code_keccak256
    );
}

#[test]
fn create2_coordinates_are_conditional_math_not_activation_evidence() {
    let definition: ContractGeneration = decode(&vectors()["definition"]);
    let create2 = &definition.create2;
    for coordinate in [&create2.builder_account_factory, &create2.shot_registry] {
        assert_eq!(
            predict_create2_address(
                create2.deployer,
                coordinate.salt,
                coordinate.init_code_keccak256
            ),
            coordinate.predicted_address
        );
    }

    let value = &vectors()["definition"];
    for forbidden in [
        "deployed",
        "deployment_status",
        "transaction_hash",
        "block_hash",
        "activation_block",
        "activation_authority",
        "trust_root",
        "signatures",
    ] {
        assert!(
            !contains_key(value, forbidden),
            "build definition contains forbidden activation field {forbidden}"
        );
    }

    let mut activation_claim = value.clone();
    activation_claim
        .as_object_mut()
        .unwrap()
        .insert("activation_block".into(), Value::Number(1.into()));
    assert!(canonical::from_slice::<ContractGeneration>(
        &serde_json::to_vec(&activation_claim).unwrap()
    )
    .is_err());
}

#[test]
fn generation_tampering_downgrade_and_artifact_confusion_fail() {
    let original: ContractGeneration = decode(&vectors()["definition"]);

    let mut changed = original.clone();
    changed.schema = "tohseno.contract-generation/0".into();
    assert!(changed.validate().is_err());

    changed = original.clone();
    changed.chain.p256_verifier.gas = 3_450;
    assert!(changed.validate().is_err());

    changed = original.clone();
    changed.source.tree_sha256 = Bytes32::new([0xaa; 32]);
    assert!(changed.validate().is_err());

    changed = original.clone();
    changed.source.files.swap(0, 1);
    assert!(changed.validate().is_err());

    changed = original.clone();
    changed.contracts.builder_account_factory.creation_bytecode =
        changed.contracts.builder_account.creation_bytecode.clone();
    assert!(changed.validate().is_err());

    changed = original.clone();
    std::mem::swap(
        &mut changed.contracts.builder_account.abi,
        &mut changed.contracts.builder_account_factory.abi,
    );
    assert!(changed.validate().is_err());

    changed = original.clone();
    changed
        .contracts
        .builder_account
        .creation_bytecode
        .as_mut()
        .unwrap()
        .path = "bytecode/Other.creation.hex".into();
    assert!(changed.validate().is_err());

    changed = original.clone();
    changed.create2.shot_registry.predicted_address =
        changed.create2.builder_account_factory.predicted_address;
    assert!(changed.validate().is_err());

    changed = original;
    changed.create2.shot_registry.init_code_keccak256 = Bytes32::new([0xbb; 32]);
    assert!(changed.validate().is_err());
}

#[test]
fn parsing_rejects_unknown_duplicate_and_path_traversal_fields() {
    let vectors = vectors();
    let mut traversal = vectors["definition"].clone();
    *traversal
        .pointer_mut("/contracts/builder_account/abi/path")
        .unwrap() = Value::String("../BuilderAccount.json".into());
    let parsed: ContractGeneration = serde_json::from_value(traversal).unwrap();
    assert!(parsed.validate().is_err());

    let duplicate =
        br#"{"schema":"tohseno.contract-generation/1","schema":"tohseno.contract-generation/1"}"#;
    assert!(canonical::from_slice::<ContractGeneration>(duplicate).is_err());
}

fn assert_artifact(path: &Path, expected_sha256: Bytes32, expected_byte_length: u64) {
    let bytes = fs::read(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    assert_eq!(
        bytes.len() as u64,
        expected_byte_length,
        "{}",
        path.display()
    );
    assert_eq!(
        Bytes32::new(Sha256::digest(&bytes).into()),
        expected_sha256,
        "{}",
        path.display()
    );
}

fn contains_key(value: &Value, key: &str) -> bool {
    match value {
        Value::Object(object) => {
            object.contains_key(key) || object.values().any(|value| contains_key(value, key))
        }
        Value::Array(values) => values.iter().any(|value| contains_key(value, key)),
        _ => false,
    }
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
