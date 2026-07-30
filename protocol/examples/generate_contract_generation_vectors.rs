use serde_json::{json, Value};
use tohseno_protocol::canonical;
use tohseno_protocol::contract_generation::ContractGeneration;

fn main() {
    let bytes = include_bytes!("../../contracts/generations/0.8.0/generation.json");
    let definition: ContractGeneration = canonical::from_slice(bytes).unwrap();
    definition.validate().unwrap();
    let canonical = canonical::to_string(&definition).unwrap();
    let digest = definition.digest().unwrap();
    let value: Value = serde_json::from_slice(bytes).unwrap();

    let vector = json!({
        "schema": "tohseno.contract-generation-test-vector/1",
        "definition": value,
        "rfc8785": canonical,
        "definition_digest": digest
    });
    println!("{}", serde_json::to_string_pretty(&vector).unwrap());
}
