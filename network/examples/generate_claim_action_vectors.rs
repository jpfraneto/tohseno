use serde_json::json;
use tohseno_network::claims::{
    ClaimSoftwareAction, OpenClaimEditionAction, CLAIMS_DOMAIN, CLAIMS_EIP712_VERSION,
    CLAIM_SOFTWARE_TYPE, OPEN_CLAIM_EDITION_TYPE,
};
use tohseno_protocol::actions::{keccak256, type_hash, Eip712Domain};
use tohseno_protocol::digest::{Address20, Bytes32, ShotId};
use tohseno_protocol::identity::ROBINHOOD_CHAIN_ID;

fn main() {
    let claims_contract = address(0x66);
    let registry = Address20::from_bytes([
        0x3f, 0xe6, 0x50, 0x8b, 0xa2, 0x66, 0x0b, 0xc5, 0x75, 0x08, 0x00, 0x24, 0xf4, 0x02, 0xc1,
        0x92, 0xa2, 0xe0, 0x35, 0xa0,
    ]);
    let domain = Eip712Domain {
        name: CLAIMS_DOMAIN.into(),
        version: CLAIMS_EIP712_VERSION.into(),
        chain_id: ROBINHOOD_CHAIN_ID,
        verifying_contract: claims_contract,
    };
    let open = OpenClaimEditionAction {
        shot_registry: registry,
        shot_id: ShotId::from_bytes([0x11; 32]),
        max_claims: 888,
        closes_at: 2_000_000_000,
        controller: address(0x22),
        nonce: 7,
        deadline: 2_000_000_100,
    };
    let claim = ClaimSoftwareAction {
        shot_registry: registry,
        shot_id: ShotId::from_bytes([0x11; 32]),
        claimant: address(0x44),
        release_digest: bytes32(0x55),
        checkpoint_digest: bytes32(0x77),
        gesture_commitment: Bytes32::from_hex(
            "gesture",
            "0x23ff9441e61d47a40c542827940bf16cf1f96311e8435c0b8920e97e97861e87",
        )
        .expect("gesture"),
        nonce: 9,
        deadline: 2_000_000_100,
    };
    let fixture = json!({
        "schema": "tohseno.claim-action-vectors/1",
        "domain": domain,
        "domain_separator": domain.separator(),
        "shot_registry": registry,
        "open_claim_edition": {
            "type": OPEN_CLAIM_EDITION_TYPE,
            "type_hash": type_hash(OPEN_CLAIM_EDITION_TYPE),
            "action": open,
            "struct_hash": open.struct_hash(registry).expect("open struct"),
            "digest": open.digest(&domain, registry).expect("open digest")
        },
        "claim_software": {
            "type": CLAIM_SOFTWARE_TYPE,
            "type_hash": type_hash(CLAIM_SOFTWARE_TYPE),
            "action": claim,
            "struct_hash": claim.struct_hash(registry).expect("claim struct"),
            "digest": claim.digest(&domain, registry).expect("claim digest")
        },
        "fixture_sha3_sanity": keccak256(b"TOHSENO Claims v1")
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&fixture).expect("serialize")
    );
}

fn address(byte: u8) -> Address20 {
    Address20::from_bytes([byte; 20])
}

fn bytes32(byte: u8) -> Bytes32 {
    Bytes32::new([byte; 32])
}
