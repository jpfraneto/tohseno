use p256::ecdsa::signature::hazmat::PrehashSigner;
use p256::ecdsa::{Signature, SigningKey};
use serde_json::{json, Value};
use tohseno_protocol::actions::{
    type_hash, Eip712Domain, RegistryActionV2, SignedRegistryActionV2, APPEND_CHECKPOINT_V2_TYPE,
    REGISTER_SHOT_V2_TYPE, REGISTRY_ACTION_V2_SCHEMA, SHOT_REGISTRATION_COMMITMENT_V2_TYPE,
    SHOT_REGISTRY_DOMAIN, SHOT_REGISTRY_V2_EIP712_VERSION, TRANSFER_SHOT_V2_TYPE,
};
use tohseno_protocol::digest::{Address20, Bytes32, ShotId};
use tohseno_protocol::identity::ROBINHOOD_CHAIN_ID;
use tohseno_protocol::signature::{
    encode_compact, DetachedP256Signature, P256PublicKey, P256Signature, SignatureAlgorithm,
};

fn main() {
    // Test-only scalar. It is intentionally absent from the emitted vector.
    let signing_key = SigningKey::from_bytes((&[1_u8; 32]).into()).unwrap();
    let signer = public_key(&signing_key);
    let domain = Eip712Domain {
        name: SHOT_REGISTRY_DOMAIN.into(),
        version: SHOT_REGISTRY_V2_EIP712_VERSION.into(),
        chain_id: ROBINHOOD_CHAIN_ID,
        verifying_contract: address(0x66),
    };
    let register = RegistryActionV2::RegisterShot {
        shot_id: ShotId::from_bytes([0x11; 32]),
        controller: address(0x88),
        head: Bytes32::new([0x22; 32]),
        salt: Bytes32::new([0x33; 32]),
        nonce: 0,
        deadline: 2_000_000_000,
    };
    let append = RegistryActionV2::AppendCheckpoint {
        shot_id: ShotId::from_bytes([0x11; 32]),
        previous_head: Bytes32::new([0x22; 32]),
        new_head: Bytes32::new([0x44; 32]),
        checkpoint_sequence: 2,
        nonce: 1,
        deadline: 2_000_000_100,
    };
    let transfer = RegistryActionV2::TransferShot {
        shot_id: ShotId::from_bytes([0x11; 32]),
        current_controller: address(0x88),
        new_controller: address(0x99),
        current_head: Bytes32::new([0x44; 32]),
        checkpoint_sequence: 2,
        nonce: 2,
        deadline: 2_000_000_200,
    };
    let commitment = register.registration_commitment(&domain).unwrap();

    let vector = json!({
        "schema": "tohseno.registry-v2-test-vectors/1",
        "contract_generation": "0.8.0",
        "commit_window": {
            "minimum_age_seconds": 60,
            "maximum_age_seconds": 86_400,
            "inclusive": true
        },
        "domain": {
            "value": domain,
            "separator": domain.separator()
        },
        "type_hashes": {
            (SHOT_REGISTRATION_COMMITMENT_V2_TYPE): type_hash(SHOT_REGISTRATION_COMMITMENT_V2_TYPE),
            (REGISTER_SHOT_V2_TYPE): type_hash(REGISTER_SHOT_V2_TYPE),
            (APPEND_CHECKPOINT_V2_TYPE): type_hash(APPEND_CHECKPOINT_V2_TYPE),
            (TRANSFER_SHOT_V2_TYPE): type_hash(TRANSFER_SHOT_V2_TYPE)
        },
        "registration_commitment": {
            "value": commitment,
            "hash": commitment.commitment().unwrap()
        },
        "actions": {
            "register_shot": action_vector(&register, &domain, &signing_key, &signer),
            "append_checkpoint": action_vector(&append, &domain, &signing_key, &signer),
            "transfer_shot": action_vector(&transfer, &domain, &signing_key, &signer)
        }
    });
    println!("{}", serde_json::to_string_pretty(&vector).unwrap());
}

fn action_vector(
    action: &RegistryActionV2,
    domain: &Eip712Domain,
    signing_key: &SigningKey,
    signer: &P256PublicKey,
) -> Value {
    let digest = action.digest(domain).unwrap();
    let signature = sign(signing_key, digest);
    let signed = SignedRegistryActionV2 {
        schema: REGISTRY_ACTION_V2_SCHEMA.into(),
        domain: domain.clone(),
        action: action.clone(),
        signer: signer.clone(),
        authorization: DetachedP256Signature {
            algorithm: SignatureAlgorithm::P256,
            digest,
            signature: signature.clone(),
            low_s: true,
        },
    };
    signed.verify().unwrap();
    json!({
        "value": action,
        "type_string": action.type_string(),
        "struct_hash": action.struct_hash().unwrap(),
        "digest": digest,
        "signature": signature,
        "compact_signature_hex": hex(&encode_compact(signer, &signed.authorization.signature).unwrap()),
        "signed": signed
    })
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

fn address(byte: u8) -> Address20 {
    Address20::from_bytes([byte; 20])
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(2 + bytes.len() * 2);
    output.push_str("0x");
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}
