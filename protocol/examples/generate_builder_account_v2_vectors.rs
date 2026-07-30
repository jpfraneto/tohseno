use serde_json::{json, Value};
use tohseno_protocol::actions::{
    type_hash, BuilderAccountActionAuthority, BuilderAccountActionV2, Eip712Domain,
    BUILDER_ACCOUNT_DOMAIN, CANCEL_RECOVERY_TYPE, CHANGE_RECOVERY_TYPE, EIP712_VERSION,
    INITIATE_RECOVERY_TYPE,
};
use tohseno_protocol::digest::{Address20, Bytes32};
use tohseno_protocol::identity::{device_key_id, ROBINHOOD_CHAIN_ID};
use tohseno_protocol::signature::P256PublicKey;

fn main() {
    let domain = Eip712Domain {
        name: BUILDER_ACCOUNT_DOMAIN.into(),
        version: EIP712_VERSION.into(),
        chain_id: ROBINHOOD_CHAIN_ID,
        verifying_contract: address(0x88),
    };
    let replacement = P256PublicKey {
        x: Bytes32::from_hex(
            "x",
            "0x6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296",
        )
        .unwrap(),
        y: Bytes32::from_hex(
            "y",
            "0x4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5",
        )
        .unwrap(),
    };
    let change = BuilderAccountActionV2::ChangeRecovery {
        account: address(0x88),
        current_recovery: address(0xaa),
        new_recovery: address(0xbb),
        nonce: 7,
        deadline: 2_000_000_000,
    };
    let initiate = BuilderAccountActionV2::InitiateRecovery {
        account: address(0x88),
        current_recovery: address(0xaa),
        new_recovery: address(0xbb),
        new_key_id: device_key_id(&replacement),
        new_x: replacement.x,
        new_y: replacement.y,
        nonce: 3,
        deadline: 2_000_000_000,
    };
    let recovery_id = initiate.digest(&domain).unwrap();
    let cancel = BuilderAccountActionV2::CancelRecovery {
        account: address(0x88),
        recovery_id,
        nonce: 8,
        deadline: 2_000_000_100,
    };

    let vectors = json!({
        "schema": "tohseno.builder-account-v2-test-vectors/1",
        "contract_generation": "0.8.0",
        "recovery_delay_seconds": 259200,
        "domain": {
            "value": domain,
            "separator": domain.separator()
        },
        "type_hashes": {
            (CHANGE_RECOVERY_TYPE): type_hash(CHANGE_RECOVERY_TYPE),
            (INITIATE_RECOVERY_TYPE): type_hash(INITIATE_RECOVERY_TYPE),
            (CANCEL_RECOVERY_TYPE): type_hash(CANCEL_RECOVERY_TYPE)
        },
        "actions": {
            "change_recovery": action_vector(&change, &domain),
            "initiate_recovery": action_vector(&initiate, &domain),
            "cancel_recovery": action_vector(&cancel, &domain)
        }
    });
    println!("{}", serde_json::to_string_pretty(&vectors).unwrap());
}

fn action_vector(action: &BuilderAccountActionV2, domain: &Eip712Domain) -> Value {
    json!({
        "value": action,
        "type_string": action.type_string(),
        "authority": match action.authority() {
            BuilderAccountActionAuthority::DeviceAdmin => "DEVICE_ADMIN",
            BuilderAccountActionAuthority::RecoveryAuthority => "RECOVERY_AUTHORITY"
        },
        "struct_hash": action.struct_hash().unwrap(),
        "digest": action.digest(domain).unwrap()
    })
}

fn address(byte: u8) -> Address20 {
    Address20::from_bytes([byte; 20])
}
