use p256::ecdsa::signature::hazmat::PrehashSigner;
use p256::ecdsa::{Signature, SigningKey};
use serde_json::json;
use std::path::Path;
use tohseno_protocol::actions::{
    type_hash, DeviceAction, Eip712Domain, PublicAction, PublicState, APPEND_EVOLUTION_TYPE,
    ASSOCIATE_APPCOIN_TYPE, ATTEST_APP_STORE_TYPE, AUTHORIZE_DEVICE_TYPE, BUILDER_ACCOUNT_DOMAIN,
    CLAIM_HANDLE_TYPE, CREATE_SHOT_TYPE, EIP712_DOMAIN_TYPE, EIP712_VERSION, RECOVER_ACCOUNT_TYPE,
    RELEASE_HANDLE_TYPE, REMOVE_APPCOIN_TYPE, REVOKE_DEVICE_TYPE, SET_PUBLIC_STATE_TYPE,
    SET_RECOVERY_TYPE, SHOT_REGISTRY_DOMAIN, SHOT_RELATIONS_DOMAIN, TRANSFER_SHOT_TYPE,
};
use tohseno_protocol::canonical;
use tohseno_protocol::continuity::{
    ContinuityAudience, ContinuityEnvelope, ContinuityStatement, CONTINUITY_SCHEMA,
    CONTINUITY_STATEMENT_SCHEMA,
};
use tohseno_protocol::digest::{Address20, Bytes32, ShotId};
use tohseno_protocol::fascia_tree::{hash_fascia_tree, DEFAULT_FASCIA_EXCLUSIONS};
use tohseno_protocol::genesis::{genesis_image, genesis_input_sha256, GENESIS_INPUT_DOMAIN};
use tohseno_protocol::identity::{
    device_key_id, installation_id, predict_builder_account, BuilderId, InstallationIdentity,
    ROBINHOOD_CHAIN_ID,
};
use tohseno_protocol::record::{
    CanonicalTimestamp, FactoryDescriptor, ShotOrigin, ShotRecord, APPLE_FASCIA_ID, PROTOCOL_NAME,
    SHOT_SCHEMA,
};
use tohseno_protocol::signature::{
    encode_compact, DetachedP256Signature, P256PublicKey, P256Signature, SignatureAlgorithm,
    SignatureSidecar,
};
use tohseno_protocol::tree_hash::{
    hash_source_tree, SELF_REFERENTIAL_EXCLUSIONS, SOURCE_TREE_DOMAIN,
};

const P256_ORDER: [u8; 32] = [
    0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xbc, 0xe6, 0xfa, 0xad, 0xa7, 0x17, 0x9e, 0x84, 0xf3, 0xb9, 0xca, 0xc2, 0xfc, 0x63, 0x25, 0x51,
];

fn main() {
    // Test-only scalar. It is intentionally absent from the emitted vector.
    let signing_key = SigningKey::from_bytes((&[1_u8; 32]).into()).unwrap();
    let public_key = public_key(&signing_key);

    let record = ShotRecord {
        protocol: PROTOCOL_NAME.into(),
        schema: SHOT_SCHEMA.into(),
        shot_id: ShotId::from_bytes([0x11; 32]),
        slug: "quiet-field-notebook".into(),
        builder_id: BuilderId::new(address(0x22)),
        sequence: 1,
        previous: None,
        fascia: APPLE_FASCIA_ID.into(),
        bundle_id: "com.tohseno.quiet-field-notebook".into(),
        bundle_version: 1,
        genesis_input_sha256: Bytes32::new([0x33; 32]),
        source_tree_sha256: Bytes32::new([0x44; 32]),
        fascia_sha256: Bytes32::new([0x55; 32]),
        factory: FactoryDescriptor {
            implementation: "tohseno/genesis-factory".into(),
            version: "0.7.0".into(),
            source_commit: "0123456789abcdef0123456789abcdef01234567".into(),
        },
        created_at: CanonicalTimestamp::parse("2026-07-28T12:34:56Z").unwrap(),
        origin: None,
    };
    let record_canonical = canonical::to_vec(&record).unwrap();
    let record_digest = record.commitment().unwrap();
    let record_signature = sign(&signing_key, record_digest);
    let record_sidecar = SignatureSidecar {
        schema: SignatureSidecar::SCHEMA.into(),
        algorithm: SignatureAlgorithm::P256,
        digest: record_digest,
        public_key: public_key.clone(),
        signature: record_signature.clone(),
        low_s: true,
    };
    let high_s_signature = P256Signature {
        r: record_signature.r,
        s: subtract_be(P256_ORDER, record_signature.s.into_bytes()),
    };
    let mut mutated_record = record.clone();
    mutated_record.slug = "quiet-field-notebook-mutated".into();

    let adopted_root = ShotRecord {
        protocol: PROTOCOL_NAME.into(),
        schema: SHOT_SCHEMA.into(),
        shot_id: ShotId::from_bytes([0x12; 32]),
        slug: "adopted-field-notebook".into(),
        builder_id: BuilderId::new(address(0x22)),
        sequence: 8,
        previous: None,
        fascia: APPLE_FASCIA_ID.into(),
        bundle_id: "com.tohseno.adopted-field-notebook".into(),
        bundle_version: 8,
        genesis_input_sha256: Bytes32::new([0x34; 32]),
        source_tree_sha256: Bytes32::new([0x45; 32]),
        fascia_sha256: Bytes32::new([0x55; 32]),
        factory: FactoryDescriptor {
            implementation: "tohseno/genesis-factory".into(),
            version: "0.7.0".into(),
            source_commit: "0123456789abcdef0123456789abcdef01234567".into(),
        },
        created_at: CanonicalTimestamp::parse("2026-07-28T13:00:00Z").unwrap(),
        origin: Some(ShotOrigin::LegacyAdoption {
            legacy_latest_shot: 7,
            legacy_source_sha256: Bytes32::new([0xaa; 32]),
        }),
    };
    let adopted_root_digest = adopted_root.commitment().unwrap();
    let adopted_root_sidecar = signature_sidecar(&signing_key, &public_key, adopted_root_digest);
    let mut adopted_child = adopted_root.clone();
    adopted_child.sequence = 9;
    adopted_child.bundle_version = 9;
    adopted_child.previous = Some(adopted_root_digest);
    adopted_child.source_tree_sha256 = Bytes32::new([0x46; 32]);
    adopted_child.created_at = CanonicalTimestamp::parse("2026-07-28T14:00:00Z").unwrap();
    adopted_child.origin = None;
    let adopted_child_digest = adopted_child.commitment().unwrap();
    let adopted_child_sidecar = signature_sidecar(&signing_key, &public_key, adopted_child_digest);

    let registry_domain = Eip712Domain {
        name: SHOT_REGISTRY_DOMAIN.into(),
        version: EIP712_VERSION.into(),
        chain_id: ROBINHOOD_CHAIN_ID,
        verifying_contract: address(0x66),
    };
    let relations_domain = Eip712Domain {
        name: SHOT_RELATIONS_DOMAIN.into(),
        version: EIP712_VERSION.into(),
        chain_id: ROBINHOOD_CHAIN_ID,
        verifying_contract: address(0x77),
    };
    let builder_domain = Eip712Domain {
        name: BUILDER_ACCOUNT_DOMAIN.into(),
        version: EIP712_VERSION.into(),
        chain_id: ROBINHOOD_CHAIN_ID,
        verifying_contract: address(0x88),
    };
    let create = PublicAction::CreateShot {
        shot_id: record.shot_id,
        controller: address(0x22),
        head: record_digest,
        sequence: 1,
        public_state: PublicState::Published,
        content_commitment: Bytes32::new([0x99; 32]),
        nonce: 0,
        deadline: 2_000_000_000,
    };
    let create_struct_hash = create.struct_hash().unwrap();
    let create_digest = create.digest(&registry_domain).unwrap();
    let create_signature = sign(&signing_key, create_digest);

    let authorize = DeviceAction::AuthorizeDevice {
        account: address(0x88),
        key_id: device_key_id(&public_key),
        x: public_key.x,
        y: public_key.y,
        permissions: 3,
        nonce: 0,
        deadline: 2_000_000_000,
    };
    let authorize_struct_hash = authorize.struct_hash().unwrap();
    let authorize_digest = authorize.digest(&builder_domain).unwrap();

    let recover = DeviceAction::RecoverAccount {
        account: address(0x88),
        current_recovery: address(0xaa),
        new_recovery: address(0xbb),
        new_key_id: device_key_id(&public_key),
        new_x: public_key.x,
        new_y: public_key.y,
        nonce: 0,
        deadline: 2_000_000_000,
    };
    let recover_struct_hash = recover.struct_hash().unwrap();
    let recover_digest = recover.digest(&builder_domain).unwrap();

    let creation_bytecode = [0x60, 0x00, 0x60, 0x00];
    let factory = address(0xcc);
    let salt = Bytes32::new([0xdd; 32]);
    let predicted =
        predict_builder_account(factory, salt, &public_key, &creation_bytecode).unwrap();

    let continuity_statement = ContinuityStatement {
        schema: CONTINUITY_STATEMENT_SCHEMA.into(),
        issuer: InstallationIdentity::from_public_key(public_key.clone()).unwrap(),
        audience: ContinuityAudience {
            shot_id: ShotId::from_bytes([0x33; 32]),
            installation_id: None,
        },
        originating_shot_id: ShotId::from_bytes([0x11; 32]),
        claims: vec![
            "0_start".into(),
            "reading-progress".into(),
            "theme.preference".into(),
            "user_state".into(),
        ],
        nonce: Bytes32::new([0x22; 32]),
        issued_at: 1_000,
        expires_at: 2_000,
    };
    continuity_statement.validate().unwrap();
    let continuity_digest = continuity_statement.digest().unwrap();
    let continuity_envelope = ContinuityEnvelope {
        schema: CONTINUITY_SCHEMA.into(),
        statement: continuity_statement.clone(),
        signature: DetachedP256Signature {
            algorithm: SignatureAlgorithm::P256,
            digest: continuity_digest,
            signature: sign(&signing_key, continuity_digest),
            low_s: true,
        },
    };
    continuity_envelope.verify_at(1_000).unwrap();

    let mut maximum_claims = continuity_statement.clone();
    maximum_claims.claims = (0..16).map(|index| format!("claim{index:02}")).collect();
    maximum_claims.validate().unwrap();
    let mut recipient_scoped = continuity_statement.clone();
    recipient_scoped.audience.installation_id = Some(Bytes32::new([0x44; 32]));
    recipient_scoped.validate().unwrap();

    let invalid_key = P256PublicKey {
        x: Bytes32::ZERO,
        y: Bytes32::ZERO,
    };
    let invalid_continuity_statements = vec![
        mutated_continuity_statement("claims_out_of_order", &continuity_statement, |statement| {
            statement["claims"] = json!(["user_state", "theme.preference"]);
        }),
        mutated_continuity_statement("duplicate_claim", &continuity_statement, |statement| {
            statement["claims"] = json!(["reading.progress", "reading.progress"]);
        }),
        mutated_continuity_statement("too_many_claims", &continuity_statement, |statement| {
            statement["claims"] = json!((0..17)
                .map(|index| format!("claim{index:02}"))
                .collect::<Vec<_>>());
        }),
        mutated_continuity_statement("colon_claim", &continuity_statement, |statement| {
            statement["claims"] = json!(["reading:progress"]);
        }),
        mutated_continuity_statement("leading_separator", &continuity_statement, |statement| {
            statement["claims"] = json!(["_reading"]);
        }),
        mutated_continuity_statement("adjacent_separators", &continuity_statement, |statement| {
            statement["claims"] = json!(["reading._progress"]);
        }),
        mutated_continuity_statement("trailing_separator", &continuity_statement, |statement| {
            statement["claims"] = json!(["reading."]);
        }),
        mutated_continuity_statement(
            "zero_audience_shot_id",
            &continuity_statement,
            |statement| {
                statement["audience"]["shot_id"] = json!(Bytes32::ZERO);
            },
        ),
        mutated_continuity_statement(
            "zero_originating_shot_id",
            &continuity_statement,
            |statement| {
                statement["originating_shot_id"] = json!(Bytes32::ZERO);
            },
        ),
        mutated_continuity_statement("zero_nonce", &continuity_statement, |statement| {
            statement["nonce"] = json!(Bytes32::ZERO);
        }),
        mutated_continuity_statement("zero_issued_at", &continuity_statement, |statement| {
            statement["issued_at"] = json!(0);
        }),
        mutated_continuity_statement(
            "expiration_not_after_issue",
            &continuity_statement,
            |statement| {
                statement["expires_at"] = statement["issued_at"].clone();
            },
        ),
        mutated_continuity_statement(
            "expiration_exceeds_safe_integer",
            &continuity_statement,
            |statement| {
                statement["expires_at"] = json!(9_007_199_254_740_992_u64);
            },
        ),
        mutated_continuity_statement(
            "wrong_installation_id",
            &continuity_statement,
            |statement| {
                statement["issuer"]["installation_id"] = json!(Bytes32::new([0x55; 32]));
            },
        ),
        mutated_continuity_statement(
            "invalid_p256_public_key",
            &continuity_statement,
            |statement| {
                statement["issuer"]["installation_id"] = json!(installation_id(&invalid_key));
                statement["issuer"]["public_key"] = json!(invalid_key);
            },
        ),
        mutated_continuity_statement(
            "missing_explicit_audience_installation_id",
            &continuity_statement,
            |statement| {
                statement["audience"]
                    .as_object_mut()
                    .unwrap()
                    .remove("installation_id");
            },
        ),
        mutated_continuity_statement(
            "unknown_statement_field",
            &continuity_statement,
            |statement| {
                statement["unexpected"] = json!(true);
            },
        ),
    ];

    let genesis_prompt = b"Build a quiet field notebook.\n";
    let genesis_images = [
        genesis_image("cover.png", b"\x89PNG\r\nraw-a").unwrap(),
        genesis_image("reference.jpg", b"\xff\xd8raw-b\xff\xd9").unwrap(),
    ];
    let genesis_digest = genesis_input_sha256(genesis_prompt, &genesis_images).unwrap();

    let source_tree_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-vectors/source-tree");
    let source_tree = hash_source_tree(&source_tree_root).unwrap();
    let fascia_tree_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-vectors/fascia-tree");
    let fascia_tree = hash_fascia_tree(&fascia_tree_root).unwrap();

    let type_strings = [
        EIP712_DOMAIN_TYPE,
        CREATE_SHOT_TYPE,
        APPEND_EVOLUTION_TYPE,
        TRANSFER_SHOT_TYPE,
        SET_PUBLIC_STATE_TYPE,
        AUTHORIZE_DEVICE_TYPE,
        REVOKE_DEVICE_TYPE,
        SET_RECOVERY_TYPE,
        RECOVER_ACCOUNT_TYPE,
        CLAIM_HANDLE_TYPE,
        RELEASE_HANDLE_TYPE,
        ASSOCIATE_APPCOIN_TYPE,
        REMOVE_APPCOIN_TYPE,
        ATTEST_APP_STORE_TYPE,
    ];
    let type_hashes = type_strings
        .iter()
        .map(|value| (value.to_string(), type_hash(value)))
        .collect::<std::collections::BTreeMap<_, _>>();

    let compact = encode_compact(&public_key, &create_signature).unwrap();
    let vector = json!({
        "schema": "tohseno.test-vectors/1",
        "candidate": "0.7.0",
        "fixed_p256_key": {
            "public_key": public_key,
            "device_key_id": device_key_id(&public_key),
            "device_key_id_law": "keccak256(x32||y32)",
            "installation_id": installation_id(&public_key),
            "installation_id_law": "sha256(\"TOHSENO-INSTALLATION-ID-V1\\0\"||x32||y32)"
        },
        "fascia_tree": {
            "stream": "concatenated sorted u64be(path_len)||path||u64be(content_len)||raw_content; no prefix or count",
            "excluded_paths": DEFAULT_FASCIA_EXCLUSIONS,
            "entries": fascia_tree.entries,
            "sha256": fascia_tree.digest
        },
        "record": {
            "value": record,
            "rfc8785": String::from_utf8(record_canonical.clone()).unwrap(),
            "sha256": record_digest,
            "sidecar": record_sidecar,
            "mutation": {
                "field": "slug",
                "value": mutated_record.slug
            },
            "mutated_sha256": mutated_record.commitment().unwrap(),
            "invalid_high_s_signature": high_s_signature
        },
        "legacy_adoption": {
            "root": adopted_root,
            "root_sha256": adopted_root_digest,
            "root_sidecar": adopted_root_sidecar,
            "child": adopted_child,
            "child_sha256": adopted_child_digest,
            "child_sidecar": adopted_child_sidecar
        },
        "eip712": {
            "domain_type": EIP712_DOMAIN_TYPE,
            "type_hashes": type_hashes,
            "registry_domain": {
                "value": registry_domain,
                "separator": registry_domain.separator()
            },
            "relations_domain": {
                "value": relations_domain,
                "separator": relations_domain.separator()
            },
            "builder_account_domain": {
                "value": builder_domain,
                "separator": builder_domain.separator()
            },
            "create_shot": {
                "type_string": CREATE_SHOT_TYPE,
                "value": create,
                "struct_hash": create_struct_hash,
                "digest": create_digest,
                "signature": create_signature,
                "compact_signature_hex": hex(&compact)
            },
            "authorize_device": {
                "type_string": AUTHORIZE_DEVICE_TYPE,
                "value": authorize,
                "struct_hash": authorize_struct_hash,
                "digest": authorize_digest
            },
            "recover_account": {
                "type_string": RECOVER_ACCOUNT_TYPE,
                "value": recover,
                "new_recovery_must_be_nonzero": true,
                "struct_hash": recover_struct_hash,
                "digest": recover_digest
            }
        },
        "builder_account_create2": {
            "factory": factory,
            "salt": salt,
            "creation_bytecode_hex": hex(&creation_bytecode),
            "constructor_abi_words": ["x", "y"],
            "recovery_in_prediction": false,
            "predicted_builder_id": predicted
        },
        "continuity": {
            "active_window": [
                {"now_unix": 999, "valid": false},
                {"now_unix": 1_000, "valid": true},
                {"now_unix": 1_999, "valid": true},
                {"now_unix": 2_000, "valid": false}
            ],
            "canonical": {
                "envelope": continuity_envelope,
                "rfc8785": canonical::to_string(&continuity_statement).unwrap(),
                "sha256": continuity_digest,
                "statement": continuity_statement
            },
            "invalid_statements": invalid_continuity_statements,
            "valid_statements": [
                {
                    "name": "maximum_claim_count",
                    "statement": maximum_claims
                },
                {
                    "name": "recipient_scoped",
                    "statement": recipient_scoped
                }
            ]
        },
        "genesis_input": {
            "domain_hex": hex(GENESIS_INPUT_DOMAIN),
            "prompt_utf8": String::from_utf8(genesis_prompt.to_vec()).unwrap(),
            "prompt_hex": hex(genesis_prompt),
            "images": [
                {
                    "filename": genesis_images[0].filename,
                    "raw_hex": hex(b"\x89PNG\r\nraw-a"),
                    "content_sha256": genesis_images[0].content_sha256
                },
                {
                    "filename": genesis_images[1].filename,
                    "raw_hex": hex(b"\xff\xd8raw-b\xff\xd9"),
                    "content_sha256": genesis_images[1].content_sha256
                }
            ],
            "sha256": genesis_digest
        },
        "source_tree": {
            "domain_hex": hex(SOURCE_TREE_DOMAIN),
            "excluded_paths": SELF_REFERENTIAL_EXCLUSIONS,
            "entries": source_tree.entries,
            "sha256": source_tree.digest
        }
    });
    println!("{}", serde_json::to_string_pretty(&vector).unwrap());
}

fn address(byte: u8) -> Address20 {
    Address20::from_bytes([byte; 20])
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
    let bytes = signature.to_bytes();
    P256Signature {
        r: Bytes32::new(bytes[..32].try_into().unwrap()),
        s: Bytes32::new(bytes[32..].try_into().unwrap()),
    }
}

fn signature_sidecar(
    signing_key: &SigningKey,
    public_key: &P256PublicKey,
    digest: Bytes32,
) -> SignatureSidecar {
    SignatureSidecar {
        schema: SignatureSidecar::SCHEMA.into(),
        algorithm: SignatureAlgorithm::P256,
        digest,
        public_key: public_key.clone(),
        signature: sign(signing_key, digest),
        low_s: true,
    }
}

fn mutated_continuity_statement(
    name: &str,
    statement: &ContinuityStatement,
    mutate: impl FnOnce(&mut serde_json::Value),
) -> serde_json::Value {
    let mut value = serde_json::to_value(statement).unwrap();
    mutate(&mut value);
    json!({
        "name": name,
        "statement_json": serde_json::to_string(&value).unwrap()
    })
}

fn subtract_be(minuend: [u8; 32], subtrahend: [u8; 32]) -> Bytes32 {
    let mut output = [0_u8; 32];
    let mut borrow = 0_i16;
    for index in (0..32).rev() {
        let value = minuend[index] as i16 - subtrahend[index] as i16 - borrow;
        if value < 0 {
            output[index] = (value + 256) as u8;
            borrow = 1;
        } else {
            output[index] = value as u8;
            borrow = 0;
        }
    }
    assert_eq!(borrow, 0);
    Bytes32::new(output)
}

fn hex(bytes: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(2 + bytes.len() * 2);
    output.push_str("0x");
    for byte in bytes {
        output.push(TABLE[(byte >> 4) as usize] as char);
        output.push(TABLE[(byte & 0x0f) as usize] as char);
    }
    output
}
