use jsonschema::{Registry, Retrieve, Uri, Validator};
use serde_json::Value;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const DRAFT_2020_12: &str = "https://json-schema.org/draft/2020-12/schema";

#[derive(Debug)]
struct SchemaDocument {
    path: PathBuf,
    id: String,
    contents: Value,
}

#[derive(Clone, Copy, Debug)]
struct NoExternalRetrieval;

impl Retrieve for NoExternalRetrieval {
    fn retrieve(
        &self,
        uri: &Uri<String>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        Err(io::Error::other(format!(
            "schema conformance forbids external retrieval of {}",
            uri.as_str()
        ))
        .into())
    }
}

#[test]
fn committed_schemas_and_v2_vectors_are_executable_contracts() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let schemas = load_schemas(&manifest.join("schemas"));
    assert!(!schemas.is_empty(), "no committed schemas were discovered");

    for schema in &schemas {
        assert_eq!(
            schema.contents.get("$schema").and_then(Value::as_str),
            Some(DRAFT_2020_12),
            "{} does not declare Draft 2020-12",
            schema.path.display()
        );
        jsonschema::draft202012::meta::validate(&schema.contents).unwrap_or_else(|error| {
            panic!(
                "{} is not a valid Draft 2020-12 schema: {error}",
                schema.path.display()
            )
        });
    }

    let registry = in_memory_registry(&schemas);
    for schema in &schemas {
        compile(
            &schema.contents,
            &registry,
            &schema.path.display().to_string(),
        );
    }

    let signed_action_schema = schema_named(&schemas, "signed-lineage-action.schema.json");
    let signed_action_validator = compile(
        &signed_action_schema.contents,
        &registry,
        "signed lineage action schema",
    );
    let lineage_vectors = load_json(&manifest.join("test-vectors/lineage-v2.json"));
    let signed_actions = lineage_vectors
        .get("actions")
        .and_then(Value::as_array)
        .expect("lineage-v2.json must contain an actions array");
    assert!(
        !signed_actions.is_empty(),
        "lineage-v2.json must contain signed actions"
    );
    for (index, action) in signed_actions.iter().enumerate() {
        assert_valid(
            &signed_action_validator,
            action,
            &format!("lineage-v2 action {index}"),
        );
    }

    let app_metadata_schema = schema_named(&schemas, "app-metadata-v2.schema.json");
    let app_metadata_validator = compile(
        &app_metadata_schema.contents,
        &registry,
        "app metadata v2 schema",
    );
    let app_metadata = load_json(&manifest.join("test-vectors/app-metadata-v2.json"));
    assert_valid(
        &app_metadata_validator,
        &app_metadata,
        "app-metadata-v2 vector",
    );

    let builder_action_schema = schema_named(&schemas, "builder-account-action-v2.schema.json");
    let builder_action_validator = compile(
        &builder_action_schema.contents,
        &registry,
        "BuilderAccount action v2 schema",
    );
    let builder_vectors = load_json(&manifest.join("test-vectors/builder-account-v2.json"));
    let builder_actions = builder_vectors
        .get("actions")
        .and_then(Value::as_object)
        .expect("builder-account-v2.json must contain an actions object");
    for (name, vector) in builder_actions {
        let action = vector
            .get("value")
            .unwrap_or_else(|| panic!("{name} must contain a value"));
        assert_valid(
            &builder_action_validator,
            action,
            &format!("BuilderAccount v2 action {name}"),
        );
    }

    let registry_action_schema = schema_named(&schemas, "registry-action-v2.schema.json");
    let registry_action_validator = compile(
        &registry_action_schema.contents,
        &registry,
        "ShotRegistry action v2 schema",
    );
    let registration_commitment_schema =
        schema_named(&schemas, "shot-registration-commitment-v2.schema.json");
    let registration_commitment_validator = compile(
        &registration_commitment_schema.contents,
        &registry,
        "ShotRegistry registration commitment v2 schema",
    );
    let registry_vectors = load_json(&manifest.join("test-vectors/registry-v2.json"));
    let registry_actions = registry_vectors
        .get("actions")
        .and_then(Value::as_object)
        .expect("registry-v2.json must contain an actions object");
    for (name, vector) in registry_actions {
        let signed = vector
            .get("signed")
            .unwrap_or_else(|| panic!("{name} must contain a signed action"));
        assert_valid(
            &registry_action_validator,
            signed,
            &format!("ShotRegistry v2 action {name}"),
        );
    }
    assert_valid(
        &registration_commitment_validator,
        &registry_vectors["registration_commitment"]["value"],
        "ShotRegistry v2 registration commitment",
    );

    let public_checkpoint_schema = schema_named(&schemas, "public-checkpoint.schema.json");
    let public_checkpoint_validator = compile(
        &public_checkpoint_schema.contents,
        &registry,
        "public checkpoint schema",
    );
    let public_checkpoint_vectors =
        load_json(&manifest.join("test-vectors/public-checkpoint.json"));
    let public_checkpoints = public_checkpoint_vectors
        .get("checkpoints")
        .and_then(Value::as_array)
        .expect("public-checkpoint.json must contain a checkpoints array");
    for (index, vector) in public_checkpoints.iter().enumerate() {
        assert_valid(
            &public_checkpoint_validator,
            &vector["value"],
            &format!("public checkpoint {index}"),
        );
    }

    let generation_schema = schema_named(&schemas, "contract-generation-v1.schema.json");
    let generation_validator = compile(
        &generation_schema.contents,
        &registry,
        "contract generation v1 schema",
    );
    let generation_vectors = load_json(&manifest.join("test-vectors/contract-generation-v1.json"));
    let generation = &generation_vectors["definition"];
    assert_valid(
        &generation_validator,
        generation,
        "contract generation v1 definition",
    );

    let mut unknown_field = signed_actions[0].clone();
    unknown_field
        .as_object_mut()
        .expect("signed action must be an object")
        .insert("unknown_field".into(), Value::Bool(true));
    assert!(
        !signed_action_validator.is_valid(&unknown_field),
        "a signed action with an unknown top-level field passed"
    );

    let mut malformed = signed_actions[0].clone();
    *malformed
        .pointer_mut("/action/payload_digest")
        .expect("signed action must contain a payload digest") =
        Value::String("not-a-digest".into());
    assert!(
        !signed_action_validator.is_valid(&malformed),
        "a signed action with a malformed payload digest passed"
    );

    let mut missing_signature = signed_actions[0].clone();
    missing_signature
        .as_object_mut()
        .expect("signed action must be an object")
        .remove("signature");
    assert!(
        !signed_action_validator.is_valid(&missing_signature),
        "a signed action without its signature passed"
    );

    let mut unknown_app_field = app_metadata;
    unknown_app_field
        .as_object_mut()
        .expect("app metadata must be an object")
        .insert("unknown_field".into(), Value::Bool(true));
    assert!(
        !app_metadata_validator.is_valid(&unknown_app_field),
        "app metadata with an unknown field passed"
    );

    let mut malformed_recovery = builder_actions["initiate_recovery"]["value"].clone();
    *malformed_recovery
        .pointer_mut("/new_key_id")
        .expect("initiate recovery must contain new_key_id") = Value::String("not-a-key-id".into());
    assert!(
        !builder_action_validator.is_valid(&malformed_recovery),
        "malformed BuilderAccount v2 recovery action passed"
    );

    let mut downgraded_registry = registry_actions["register_shot"]["signed"].clone();
    *downgraded_registry
        .pointer_mut("/domain/version")
        .expect("registry action must contain a domain version") = Value::String("1".into());
    assert!(
        !registry_action_validator.is_valid(&downgraded_registry),
        "a downgraded ShotRegistry action passed"
    );

    let mut malformed_checkpoint = registry_actions["append_checkpoint"]["signed"].clone();
    *malformed_checkpoint
        .pointer_mut("/action/checkpoint_sequence")
        .expect("append action must contain checkpoint_sequence") = Value::Number(1.into());
    assert!(
        !registry_action_validator.is_valid(&malformed_checkpoint),
        "checkpoint one was accepted as an append"
    );

    let mut malformed_commitment = registry_vectors["registration_commitment"]["value"].clone();
    *malformed_commitment
        .pointer_mut("/chain_id")
        .expect("registration commitment must contain chain_id") = Value::Number(1.into());
    assert!(
        !registration_commitment_validator.is_valid(&malformed_commitment),
        "a wrong-chain registration commitment passed"
    );

    let mut leaking_checkpoint = public_checkpoints[0]["value"].clone();
    leaking_checkpoint
        .as_object_mut()
        .expect("public checkpoint must be an object")
        .insert(
            "lineage_head".into(),
            Value::String(
                "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            ),
        );
    assert!(
        !public_checkpoint_validator.is_valid(&leaking_checkpoint),
        "a private-lineage field passed the public checkpoint schema"
    );

    let mut mismatched_root = public_checkpoints[0]["value"].clone();
    *mismatched_root
        .pointer_mut("/previous_checkpoint")
        .expect("public checkpoint must contain previous_checkpoint") =
        Value::String("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into());
    assert!(
        !public_checkpoint_validator.is_valid(&mismatched_root),
        "checkpoint one was allowed to claim a predecessor"
    );

    let mut activation_claim = generation.clone();
    activation_claim
        .as_object_mut()
        .expect("contract generation must be an object")
        .insert("activation_block".into(), Value::Number(1.into()));
    assert!(
        !generation_validator.is_valid(&activation_claim),
        "an activation claim passed the immutable build-definition schema"
    );

    let mut legacy_p256_gas = generation.clone();
    *legacy_p256_gas
        .pointer_mut("/chain/p256_verifier/gas")
        .expect("contract generation must declare P256 gas") = Value::Number(3_450.into());
    assert!(
        !generation_validator.is_valid(&legacy_p256_gas),
        "legacy RIP-7212 gas passed an EIP-7951 generation definition"
    );
}

fn load_schemas(directory: &Path) -> Vec<SchemaDocument> {
    let mut paths = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", directory.display()))
        .map(|entry| entry.expect("failed to read schema directory entry").path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let contents = load_json(&path);
            let id = contents
                .get("$id")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("{} has no string $id", path.display()))
                .to_owned();
            SchemaDocument { path, id, contents }
        })
        .collect()
}

fn load_json(path: &Path) -> Value {
    let bytes =
        fs::read(path).unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

fn in_memory_registry(schemas: &[SchemaDocument]) -> Registry<'static> {
    let mut builder = Registry::new().retriever(NoExternalRetrieval);
    for schema in schemas {
        builder = builder
            .add(&schema.id, schema.contents.clone())
            .unwrap_or_else(|error| {
                panic!(
                    "failed to register {} as {}: {error}",
                    schema.path.display(),
                    schema.id
                )
            });
    }
    builder
        .prepare()
        .unwrap_or_else(|error| panic!("failed to prepare in-memory schema registry: {error}"))
}

fn schema_named<'a>(schemas: &'a [SchemaDocument], name: &str) -> &'a SchemaDocument {
    schemas
        .iter()
        .find(|schema| schema.path.file_name().and_then(|value| value.to_str()) == Some(name))
        .unwrap_or_else(|| panic!("missing committed schema {name}"))
}

fn compile(schema: &Value, registry: &Registry<'static>, label: &str) -> Validator {
    jsonschema::draft202012::options()
        .with_registry(registry)
        .with_retriever(NoExternalRetrieval)
        .build(schema)
        .unwrap_or_else(|error| panic!("failed to compile {label}: {error}"))
}

fn assert_valid(validator: &Validator, instance: &Value, label: &str) {
    let errors = validator
        .iter_errors(instance)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(
        errors.is_empty(),
        "{label} failed schema validation:\n{}",
        errors.join("\n")
    );
}
