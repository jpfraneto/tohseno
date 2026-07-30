use tohseno_protocol::app_metadata::{AppMetadata, AppMetadataV2};
use tohseno_protocol::digest::{Bytes32, ExpressionId, VersionId};

fn main() {
    let v1: AppMetadata =
        serde_json::from_str(include_str!("../test-vectors/app-metadata-v1.json")).unwrap();
    let expression_id = ExpressionId::from_bytes([0x77; 32]);
    let genome_digest = Bytes32::new([0x78; 32]);
    let version_id = VersionId::derive(
        v1.shot_id,
        expression_id,
        1,
        genome_digest,
        v1.source_tree_sha256,
    );
    let v2 = AppMetadataV2::from_v1(
        &v1,
        expression_id,
        version_id,
        1,
        1,
        genome_digest,
        8,
        Bytes32::new([0x79; 32]),
        Some(Bytes32::new([0x7a; 32])),
    )
    .unwrap();
    println!("{}", serde_json::to_string_pretty(&v2).unwrap());
}
