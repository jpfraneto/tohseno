use serde_json::json;
use tohseno_protocol::canonical;
use tohseno_protocol::digest::{Address20, ShotId};
use tohseno_protocol::identity::ROBINHOOD_CHAIN_ID;
use tohseno_protocol::public_checkpoint::{
    PublicCheckpoint, PublicCheckpointWitness, PUBLIC_CHECKPOINT_CONTRACT_GENERATION,
};
use tohseno_protocol::record::CanonicalTimestamp;

fn main() {
    let witness = PublicCheckpointWitness {
        contract_generation: PUBLIC_CHECKPOINT_CONTRACT_GENERATION.into(),
        chain_id: ROBINHOOD_CHAIN_ID,
        registry: Address20::from_bytes([0x66; 20]),
    };
    let first = PublicCheckpoint::new(
        witness.clone(),
        ShotId::from_bytes([0x11; 32]),
        1,
        None,
        CanonicalTimestamp::parse("2026-07-30T12:00:00Z").unwrap(),
    )
    .unwrap();
    let second = PublicCheckpoint::new(
        witness,
        first.shot_id,
        2,
        Some(first.commitment().unwrap()),
        CanonicalTimestamp::parse("2026-07-30T13:00:00Z").unwrap(),
    )
    .unwrap();

    let vector = json!({
        "schema": "tohseno.public-checkpoint-test-vectors/1",
        "digest_law": "sha256(rfc8785(checkpoint))",
        "privacy_law": "only witness coordinates, ShotID, witness-local sequence, prior public checkpoint, fixed scope, and a newly declared canonical publication time enter the digest",
        "checkpoints": [
            {
                "value": first,
                "rfc8785": canonical::to_string(&first).unwrap(),
                "sha256": first.commitment().unwrap()
            },
            {
                "value": second,
                "rfc8785": canonical::to_string(&second).unwrap(),
                "sha256": second.commitment().unwrap()
            }
        ]
    });
    println!("{}", serde_json::to_string_pretty(&vector).unwrap());
}
