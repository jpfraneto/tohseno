use serde::de::DeserializeOwned;
use serde_json::Value;
use std::fs;
use std::path::Path;
use tohseno_protocol::actions::{type_hash, DeviceAction, Eip712Domain, PublicAction};
use tohseno_protocol::app_metadata::{AppMetadata, AppMetadataRegistryReference};
use tohseno_protocol::canonical;
use tohseno_protocol::continuity::{ContinuityEnvelope, ContinuityStatement};
use tohseno_protocol::digest::{sha256, Address20, Bytes32};
use tohseno_protocol::evolution::verify_lineage;
use tohseno_protocol::fascia_tree::hash_fascia_tree;
use tohseno_protocol::genesis::{genesis_image, genesis_input_sha256};
use tohseno_protocol::identity::{
    device_key_id, initial_builder_account_salt, installation_id, predict_builder_account,
    BuilderId,
};
use tohseno_protocol::record::ShotRecord;
use tohseno_protocol::signature::{
    decode_compact, verify_digest, P256PublicKey, P256Signature, SignatureSidecar,
};
use tohseno_protocol::tree_hash::hash_source_tree;

fn vectors() -> Value {
    serde_json::from_str(include_str!("../test-vectors/protocol-v1.json")).unwrap()
}

fn decode<T: DeserializeOwned>(value: &Value) -> T {
    serde_json::from_value(value.clone()).unwrap()
}

fn bytes(value: &Value) -> Bytes32 {
    decode(value)
}

#[test]
fn frozen_record_canonicalization_signature_and_negative_vectors_agree() {
    let vectors = vectors();
    assert_eq!(vectors["schema"], "tohseno.test-vectors/1");
    let record: ShotRecord = decode(&vectors["record"]["value"]);
    let expected = bytes(&vectors["record"]["sha256"]);
    assert_eq!(record.commitment().unwrap(), expected);
    assert_eq!(
        canonical::to_string(&record).unwrap(),
        vectors["record"]["rfc8785"].as_str().unwrap()
    );

    let sidecar: SignatureSidecar = decode(&vectors["record"]["sidecar"]);
    sidecar.verify(&record).unwrap();
    let normal_lineage = verify_lineage(&[(&record, &sidecar)]).unwrap();
    assert_eq!(normal_lineage.root_sequence, 1);
    assert_eq!(normal_lineage.legacy_latest_shot, None);

    let high_s: P256Signature = decode(&vectors["record"]["invalid_high_s_signature"]);
    assert!(!high_s.is_low_s());
    assert!(verify_digest(&sidecar.public_key, expected, &high_s).is_err());

    let mut mutated = record;
    mutated.slug = vectors["record"]["mutation"]["value"]
        .as_str()
        .unwrap()
        .into();
    assert_eq!(
        mutated.commitment().unwrap(),
        bytes(&vectors["record"]["mutated_sha256"])
    );
    assert!(sidecar.verify(&mutated).is_err());
}

#[test]
fn frozen_legacy_adoption_starts_a_real_protocol_lineage_at_n_plus_one() {
    let vectors = vectors();
    let adoption = &vectors["legacy_adoption"];
    let root: ShotRecord = decode(&adoption["root"]);
    let root_sidecar: SignatureSidecar = decode(&adoption["root_sidecar"]);
    let child: ShotRecord = decode(&adoption["child"]);
    let child_sidecar: SignatureSidecar = decode(&adoption["child_sidecar"]);

    assert_eq!(root.sequence, 8);
    assert_eq!(root.legacy_latest_shot(), Some(7));
    assert_eq!(root.previous, None);
    assert_eq!(root.commitment().unwrap(), bytes(&adoption["root_sha256"]));
    assert_eq!(
        child.commitment().unwrap(),
        bytes(&adoption["child_sha256"])
    );

    let verified = verify_lineage(&[(&root, &root_sidecar), (&child, &child_sidecar)]).unwrap();
    assert_eq!(verified.root_sequence, 8);
    assert_eq!(verified.legacy_latest_shot, Some(7));
    assert_eq!(verified.head().unwrap().sequence, 9);

    let mut gap = child.clone();
    gap.sequence = 10;
    gap.bundle_version = 10;
    assert!(verify_lineage(&[(&root, &root_sidecar), (&gap, &child_sidecar)]).is_err());

    let mut origin_on_child = child.clone();
    origin_on_child.origin = root.origin.clone();
    assert!(verify_lineage(&[(&root, &root_sidecar), (&origin_on_child, &child_sidecar)]).is_err());

    let mut fabricated_previous = root.clone();
    fabricated_previous.previous = Some(Bytes32::new([0xee; 32]));
    assert!(fabricated_previous.validate().is_err());
}

#[test]
fn frozen_identity_actions_and_create2_agree() {
    let vectors = vectors();
    let public_key: P256PublicKey = decode(&vectors["fixed_p256_key"]["public_key"]);
    assert_eq!(
        device_key_id(&public_key),
        bytes(&vectors["fixed_p256_key"]["device_key_id"])
    );
    assert_eq!(
        installation_id(&public_key),
        bytes(&vectors["fixed_p256_key"]["installation_id"])
    );

    for (type_string, expected) in vectors["eip712"]["type_hashes"].as_object().unwrap() {
        assert_eq!(type_hash(type_string), bytes(expected));
    }

    let registry: Eip712Domain = decode(&vectors["eip712"]["registry_domain"]["value"]);
    assert_eq!(
        registry.separator(),
        bytes(&vectors["eip712"]["registry_domain"]["separator"])
    );
    let create: PublicAction = decode(&vectors["eip712"]["create_shot"]["value"]);
    assert_eq!(
        create.struct_hash().unwrap(),
        bytes(&vectors["eip712"]["create_shot"]["struct_hash"])
    );
    let create_digest = bytes(&vectors["eip712"]["create_shot"]["digest"]);
    assert_eq!(create.digest(&registry).unwrap(), create_digest);
    let create_signature: P256Signature = decode(&vectors["eip712"]["create_shot"]["signature"]);
    verify_digest(&public_key, create_digest, &create_signature).unwrap();
    let compact = decode_hex(
        vectors["eip712"]["create_shot"]["compact_signature_hex"]
            .as_str()
            .unwrap(),
    );
    assert_eq!(
        decode_compact(&compact).unwrap(),
        (public_key.clone(), create_signature)
    );

    let builder_domain: Eip712Domain =
        decode(&vectors["eip712"]["builder_account_domain"]["value"]);
    let authorize: DeviceAction = decode(&vectors["eip712"]["authorize_device"]["value"]);
    assert_eq!(
        authorize.struct_hash().unwrap(),
        bytes(&vectors["eip712"]["authorize_device"]["struct_hash"])
    );
    assert_eq!(
        authorize.digest(&builder_domain).unwrap(),
        bytes(&vectors["eip712"]["authorize_device"]["digest"])
    );
    let recover: DeviceAction = decode(&vectors["eip712"]["recover_account"]["value"]);
    assert_eq!(
        recover.struct_hash().unwrap(),
        bytes(&vectors["eip712"]["recover_account"]["struct_hash"])
    );
    assert_eq!(
        recover.digest(&builder_domain).unwrap(),
        bytes(&vectors["eip712"]["recover_account"]["digest"])
    );
    let mut invalid_recovery = recover;
    if let DeviceAction::RecoverAccount { new_recovery, .. } = &mut invalid_recovery {
        *new_recovery = Address20::from_bytes([0; 20]);
    }
    assert!(invalid_recovery.validate().is_err());

    let create2 = &vectors["builder_account_create2"];
    let factory: Address20 = decode(&create2["factory"]);
    let salt = bytes(&create2["salt"]);
    let creation_bytecode = decode_hex(create2["creation_bytecode_hex"].as_str().unwrap());
    let predicted =
        predict_builder_account(factory, salt, &public_key, &creation_bytecode).unwrap();
    let expected: BuilderId = decode(&create2["predicted_builder_id"]);
    assert_eq!(predicted, expected);
}

#[test]
fn frozen_initial_builder_account_salt_agrees() {
    let fixture = include_str!("../test-vectors/builder-account-salt-v1.json");
    let vector: Value = serde_json::from_str(fixture).unwrap();
    assert_eq!(
        vector["schema"],
        "tohseno.builder-account-salt-test-vector/1"
    );
    assert_eq!(
        vector["law"],
        "sha256(\"TOHSENO-BUILDER-SALT-V1\\0\"||device_key_id)"
    );
    let public_key: P256PublicKey = decode(&vector["public_key"]);
    assert_eq!(device_key_id(&public_key), bytes(&vector["device_key_id"]));
    assert_eq!(
        initial_builder_account_salt(&public_key).unwrap(),
        bytes(&vector["account_salt"])
    );
}

#[test]
fn frozen_continuity_bytes_scope_and_negative_vectors_agree() {
    let vectors = vectors();
    let continuity = &vectors["continuity"];
    let canonical_vector = &continuity["canonical"];
    let statement: ContinuityStatement = decode(&canonical_vector["statement"]);
    statement.validate().unwrap();
    assert_eq!(
        canonical::to_string(&statement).unwrap(),
        canonical_vector["rfc8785"].as_str().unwrap()
    );
    assert_eq!(
        statement.digest().unwrap(),
        bytes(&canonical_vector["sha256"])
    );

    let envelope: ContinuityEnvelope = decode(&canonical_vector["envelope"]);
    assert_eq!(envelope.statement, statement);
    for boundary in continuity["active_window"].as_array().unwrap() {
        let now = boundary["now_unix"].as_u64().unwrap();
        let expected = boundary["valid"].as_bool().unwrap();
        assert_eq!(
            envelope.verify_at(now).is_ok(),
            expected,
            "active-window disagreement at {now}"
        );
    }

    let valid = continuity["valid_statements"].as_array().unwrap();
    assert_eq!(valid.len(), 2);
    for vector in valid {
        let candidate: ContinuityStatement = decode(&vector["statement"]);
        candidate
            .validate()
            .unwrap_or_else(|error| panic!("{}: {error}", vector["name"]));
    }

    let invalid = continuity["invalid_statements"].as_array().unwrap();
    assert_eq!(invalid.len(), 17);
    for vector in invalid {
        let encoded = vector["statement_json"].as_str().unwrap().as_bytes();
        let accepted = canonical::from_slice::<ContinuityStatement>(encoded)
            .and_then(|candidate| candidate.validate())
            .is_ok();
        assert!(!accepted, "{} unexpectedly passed", vector["name"]);
    }
}

#[test]
fn continuity_schema_freezes_the_cross_language_scope_law() {
    let schema: Value =
        serde_json::from_str(include_str!("../schemas/continuity.schema.json")).unwrap();
    let statement = &schema["properties"]["statement"]["properties"];
    let claims = &statement["claims"];
    assert_eq!(claims["minItems"], 1);
    assert_eq!(claims["maxItems"], 16);
    assert_eq!(claims["uniqueItems"], true);
    assert_eq!(claims["items"]["pattern"], "^[a-z0-9]+(?:[._-][a-z0-9]+)*$");
    assert!(claims["$comment"]
        .as_str()
        .unwrap()
        .contains("strict lexicographic ordering"));
    assert_eq!(
        statement["audience"]["properties"]["shot_id"]["$ref"],
        "common.schema.json#/$defs/nonzeroBytes32"
    );
    assert!(statement["audience"]["required"]
        .as_array()
        .unwrap()
        .contains(&Value::String("installation_id".into())));
    assert_eq!(
        statement["originating_shot_id"]["$ref"],
        "common.schema.json#/$defs/nonzeroBytes32"
    );
    assert_eq!(
        statement["issued_at"]["$ref"],
        "common.schema.json#/$defs/positiveSafeUint"
    );
    assert_eq!(
        statement["expires_at"]["$ref"],
        "common.schema.json#/$defs/positiveSafeUint"
    );
    assert_eq!(
        statement["nonce"]["allOf"][0]["$ref"],
        "common.schema.json#/$defs/bytes32"
    );
    assert_eq!(
        statement["nonce"]["allOf"][1]["not"]["const"],
        Bytes32::ZERO.to_string()
    );
}

#[test]
fn app_metadata_fixture_and_schema_freeze_the_engine_swift_contract() {
    let fixture = include_bytes!("../test-vectors/app-metadata-v1.json");
    let metadata: AppMetadata = serde_json::from_slice(fixture).unwrap();
    metadata.validate().unwrap();
    let mut encoded = serde_json::to_vec_pretty(&metadata).unwrap();
    encoded.push(b'\n');
    assert_eq!(encoded, fixture);

    let schema: Value =
        serde_json::from_str(include_str!("../schemas/app-metadata.schema.json")).unwrap();
    assert_eq!(
        schema["properties"]["schema"]["const"],
        "tohseno.app-metadata/1"
    );
    assert_eq!(
        schema["properties"]["factory"]["properties"]["source_commit"]["pattern"],
        "^[0-9a-f]{40}$"
    );
    assert_eq!(
        schema["properties"]["distribution"]["properties"]["supported_apple_surfaces"]["items"]
            ["enum"],
        serde_json::json!(["iphone", "ipad", "vision"])
    );
    assert_eq!(
        schema["properties"]["registry"]["oneOf"][1]["properties"]["chain_id"]["const"],
        4663
    );
    for field in [
        "evolution_commitment",
        "source_tree_sha256",
        "fascia_sha256",
    ] {
        assert_eq!(
            schema["properties"][field]["$ref"],
            "common.schema.json#/$defs/nonzeroBytes32"
        );
    }
    assert_eq!(
        schema["properties"]["registry"]["oneOf"][1]["properties"]["contract"]["$ref"],
        "common.schema.json#/$defs/nonzeroAddress20"
    );
    assert_eq!(
        schema["properties"]["distribution"]["properties"]["app_store_id"]["oneOf"][1]["$ref"],
        "common.schema.json#/$defs/positiveSafeUint"
    );

    let mut wrong_commit_width = metadata.clone();
    wrong_commit_width.factory.source_commit = "a".repeat(64);
    assert!(wrong_commit_width.validate().is_err());
    let mut invalid_bundle = metadata.clone();
    invalid_bundle.bundle_id = "example..app".into();
    assert!(invalid_bundle.validate().is_err());
    let baseline = metadata.clone();
    let mut repeated_surface = metadata;
    repeated_surface
        .distribution
        .supported_apple_surfaces
        .push(tohseno_protocol::fascia::AppleSurface::Iphone);
    assert!(repeated_surface.validate().is_err());
    let mut zero_commitment = baseline.clone();
    zero_commitment.evolution_commitment = Bytes32::ZERO;
    assert!(zero_commitment.validate().is_err());
    let mut zero_registry = baseline.clone();
    zero_registry.registry = Some(AppMetadataRegistryReference {
        chain_id: 4663,
        contract: Address20::from_bytes([0; 20]),
        transaction: Some(Bytes32::ZERO),
    });
    assert!(zero_registry.validate().is_err());
    let mut unsafe_app_store_id = baseline;
    unsafe_app_store_id.distribution.state = tohseno_protocol::fascia::DistributionState::AppStore;
    unsafe_app_store_id.distribution.app_store_id = Some(9_007_199_254_740_992);
    assert!(unsafe_app_store_id.validate().is_err());
}

#[test]
fn frozen_genesis_and_source_tree_streams_agree() {
    let vectors = vectors();
    let genesis = &vectors["genesis_input"];
    let prompt = decode_hex(genesis["prompt_hex"].as_str().unwrap());
    let images = genesis["images"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| {
            let raw = decode_hex(value["raw_hex"].as_str().unwrap());
            assert_eq!(sha256(&raw), bytes(&value["content_sha256"]));
            genesis_image(value["filename"].as_str().unwrap(), &raw).unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        genesis_input_sha256(&prompt, &images).unwrap(),
        bytes(&genesis["sha256"])
    );

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-vectors/source-tree");
    let observed = hash_source_tree(&root).unwrap();
    assert_eq!(observed.digest, bytes(&vectors["source_tree"]["sha256"]));
    assert_eq!(
        serde_json::to_value(observed.entries).unwrap(),
        vectors["source_tree"]["entries"]
    );

    let fascia_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-vectors/fascia-tree");
    let fascia = hash_fascia_tree(&fascia_root).unwrap();
    assert_eq!(fascia.digest, bytes(&vectors["fascia_tree"]["sha256"]));
    assert_eq!(
        serde_json::to_value(fascia.entries).unwrap(),
        vectors["fascia_tree"]["entries"]
    );
}

#[test]
fn every_committed_schema_is_draft_2020_12_and_closes_object_shapes() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("schemas");
    let mut count = 0;
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        count += 1;
        let value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            value["$schema"],
            "https://json-schema.org/draft/2020-12/schema",
            "{}",
            path.display()
        );
        assert!(value["$id"].is_string(), "{}", path.display());
        assert_closed_objects(&value, &path);
    }
    assert_eq!(count, 35);
}

fn assert_closed_objects(value: &Value, path: &Path) {
    match value {
        Value::Object(object) => {
            if object.get("type") == Some(&Value::String("object".into())) {
                assert_eq!(
                    object.get("additionalProperties"),
                    Some(&Value::Bool(false)),
                    "unclosed object schema in {}",
                    path.display()
                );
            }
            for child in object.values() {
                assert_closed_objects(child, path);
            }
        }
        Value::Array(array) => {
            for child in array {
                assert_closed_objects(child, path);
            }
        }
        _ => {}
    }
}

fn decode_hex(value: &str) -> Vec<u8> {
    let value = value.strip_prefix("0x").unwrap();
    assert_eq!(value.len() % 2, 0);
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| (nibble(pair[0]) << 4) | nibble(pair[1]))
        .collect()
}

fn nibble(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => panic!("invalid vector hex"),
    }
}
