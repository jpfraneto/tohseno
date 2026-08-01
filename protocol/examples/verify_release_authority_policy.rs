use std::{env, fs, process};
use tohseno_protocol::canonical;
use tohseno_protocol::contract_activation::ReleaseAuthorityPolicy;

fn main() {
    let mut arguments = env::args_os();
    let program = arguments.next().unwrap_or_default();
    let Some(path) = arguments.next() else {
        eprintln!("usage: {} POLICY.json", program.to_string_lossy());
        process::exit(2);
    };
    if arguments.next().is_some() {
        eprintln!("usage: {} POLICY.json", program.to_string_lossy());
        process::exit(2);
    }
    let raw = fs::read(&path).unwrap_or_else(|error| {
        eprintln!("cannot read {}: {error}", path.to_string_lossy());
        process::exit(1);
    });
    let policy: ReleaseAuthorityPolicy = canonical::from_slice(&raw).unwrap_or_else(|error| {
        eprintln!("invalid release-authority policy: {error}");
        process::exit(1);
    });
    let digest = policy.digest().unwrap_or_else(|error| {
        eprintln!("invalid release-authority policy: {error}");
        process::exit(1);
    });
    println!("{digest}");
}
