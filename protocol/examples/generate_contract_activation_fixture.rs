use p256::ecdsa::signature::hazmat::PrehashSigner;
use p256::ecdsa::{Signature, SigningKey};
use rand_core::OsRng;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use tohseno_protocol::canonical;
use tohseno_protocol::contract_activation::{
    ActivatedContract, ChainBlock, ContractActivation, DeploymentObservation, ReleaseAuthority,
    ReleaseAuthorityApproval, ReleaseAuthorityPolicy, ReleaseAuthorityPurpose,
    SignedContractActivation, CONTRACT_ACTIVATION_PROTOCOL, CONTRACT_ACTIVATION_SCHEMA,
    RELEASE_AUTHORITY_POLICY_SCHEMA, SIGNED_CONTRACT_ACTIVATION_SCHEMA,
};
use tohseno_protocol::contract_generation::ContractGeneration;
use tohseno_protocol::digest::{sha256, Bytes32};
use tohseno_protocol::record::CanonicalTimestamp;
use tohseno_protocol::signature::{
    DetachedP256Signature, P256PublicKey, P256Signature, SignatureAlgorithm,
};

#[derive(Serialize)]
struct Fixture {
    schema: &'static str,
    generation_definition_sha256: Bytes32,
    authority_policy_sha256: Bytes32,
    activation_signing_sha256: Bytes32,
    p256_probe_raw: String,
    policy: ReleaseAuthorityPolicy,
    signed_activation: SignedContractActivation,
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("protocol crate has a repository parent")
        .to_path_buf()
}

fn public_key(signing_key: &SigningKey) -> P256PublicKey {
    let encoded = signing_key.verifying_key().to_encoded_point(false);
    let mut x = [0_u8; 32];
    x.copy_from_slice(encoded.x().expect("uncompressed key has x"));
    let mut y = [0_u8; 32];
    y.copy_from_slice(encoded.y().expect("uncompressed key has y"));
    P256PublicKey {
        x: Bytes32::new(x),
        y: Bytes32::new(y),
    }
}

fn sign(signing_key: &SigningKey, digest: Bytes32) -> P256Signature {
    let signature: Signature = signing_key
        .sign_prehash(digest.as_bytes())
        .expect("fixture prehash signs");
    let signature = signature.normalize_s().unwrap_or(signature);
    P256Signature {
        r: Bytes32::new(signature.r().to_bytes().into()),
        s: Bytes32::new(signature.s().to_bytes().into()),
    }
}

fn main() {
    let p256_probe_raw = fs::read_to_string(
        repository_root().join("contracts/audits/robinhood-p256-2026-07-30.json"),
    )
    .expect("read non-authorizing P-256 fixture evidence");
    let generation: ContractGeneration = canonical::from_slice(
        &fs::read(repository_root().join("contracts/generations/0.8.0/generation.json"))
            .expect("read generation"),
    )
    .expect("parse generation");

    let mut random = OsRng;
    let mut authority_pairs = (0..3)
        .map(|_| {
            let signing_key = SigningKey::random(&mut random);
            let authority = ReleaseAuthority::from_public_key(public_key(&signing_key))
                .expect("fixture authority");
            (authority, signing_key)
        })
        .collect::<Vec<_>>();
    authority_pairs.sort_by_key(|(authority, _)| authority.key_id);

    let policy = ReleaseAuthorityPolicy {
        schema: RELEASE_AUTHORITY_POLICY_SCHEMA.into(),
        protocol: CONTRACT_ACTIVATION_PROTOCOL.into(),
        protocol_major: 2,
        purpose: ReleaseAuthorityPurpose::ContractGenerationActivation,
        threshold: 2,
        authorities: authority_pairs
            .iter()
            .map(|(authority, _)| authority.clone())
            .collect(),
        issued_at: CanonicalTimestamp::parse("2026-07-31T20:00:00Z").expect("fixture timestamp"),
    };

    let activation = ContractActivation {
        schema: CONTRACT_ACTIVATION_SCHEMA.into(),
        protocol: CONTRACT_ACTIVATION_PROTOCOL.into(),
        protocol_major: 2,
        generation: generation.generation.clone(),
        activation_sequence: 1,
        previous_activation: None,
        generation_definition_sha256: generation.digest().expect("generation digest"),
        authority_policy_sha256: policy.digest().expect("policy digest"),
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
        p256_probe_sha256: sha256(p256_probe_raw.as_bytes()),
        issued_at: CanonicalTimestamp::parse("2026-07-31T21:00:00Z").expect("fixture timestamp"),
    };
    let signing_digest = activation.signing_digest().expect("activation digest");
    let approvals = authority_pairs
        .iter()
        .take(2)
        .map(|(authority, signing_key)| ReleaseAuthorityApproval {
            key_id: authority.key_id,
            authorization: DetachedP256Signature {
                algorithm: SignatureAlgorithm::P256,
                digest: signing_digest,
                signature: sign(signing_key, signing_digest),
                low_s: true,
            },
        })
        .collect();
    let signed_activation = SignedContractActivation {
        schema: SIGNED_CONTRACT_ACTIVATION_SCHEMA.into(),
        activation,
        approvals,
    };
    signed_activation
        .verify_for_generation(&policy, &generation)
        .expect("fixture verifies under Rust implementation");

    let fixture = Fixture {
        schema: "tohseno.contract-activation-fixture/1",
        generation_definition_sha256: generation.digest().unwrap(),
        authority_policy_sha256: policy.digest().unwrap(),
        activation_signing_sha256: signing_digest,
        p256_probe_raw,
        policy,
        signed_activation,
    };
    println!("{}", serde_json::to_string_pretty(&fixture).unwrap());
}
