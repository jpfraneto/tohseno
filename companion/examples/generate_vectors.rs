use tohseno_companion::vectors::deterministic_vectors;

fn main() {
    let vectors = deterministic_vectors().expect("deterministic vectors must generate");
    println!(
        "{}",
        serde_json::to_string_pretty(&vectors).expect("vectors must serialize")
    );
}
