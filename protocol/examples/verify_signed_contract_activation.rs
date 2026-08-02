use std::{env, fs, process};
use tohseno_protocol::canonical;
use tohseno_protocol::contract_activation::{ReleaseAuthorityPolicy, SignedContractActivation};
use tohseno_protocol::contract_generation::ContractGeneration;

fn usage(program: &std::ffi::OsString) -> ! {
    eprintln!(
        "usage: {} GENERATION.json POLICY.json SIGNED_ACTIVATION.json TRUSTED_POLICY_SHA256",
        program.to_string_lossy()
    );
    process::exit(2);
}

fn read(path: &std::ffi::OsString) -> Vec<u8> {
    fs::read(path).unwrap_or_else(|error| {
        eprintln!("cannot read {}: {error}", path.to_string_lossy());
        process::exit(1);
    })
}

fn main() {
    let mut arguments = env::args_os();
    let program = arguments.next().unwrap_or_default();
    let (Some(generation_path), Some(policy_path), Some(signed_path), Some(trusted)) = (
        arguments.next(),
        arguments.next(),
        arguments.next(),
        arguments.next(),
    ) else {
        usage(&program);
    };
    if arguments.next().is_some() {
        usage(&program);
    }

    let generation: ContractGeneration = canonical::from_slice(&read(&generation_path))
        .unwrap_or_else(|error| {
            eprintln!("invalid contract generation: {error}");
            process::exit(1);
        });
    let policy: ReleaseAuthorityPolicy =
        canonical::from_slice(&read(&policy_path)).unwrap_or_else(|error| {
            eprintln!("invalid release-authority policy: {error}");
            process::exit(1);
        });
    let signed: SignedContractActivation = canonical::from_slice(&read(&signed_path))
        .unwrap_or_else(|error| {
            eprintln!("invalid signed activation: {error}");
            process::exit(1);
        });

    let policy_digest = policy.digest().unwrap_or_else(|error| {
        eprintln!("invalid release-authority policy: {error}");
        process::exit(1);
    });
    if policy_digest.to_string() != trusted.to_string_lossy() {
        eprintln!(
            "policy digest {policy_digest} does not match the supplied trust root {}",
            trusted.to_string_lossy()
        );
        process::exit(1);
    }
    signed
        .verify_for_generation(&policy, &generation)
        .unwrap_or_else(|error| {
            eprintln!("signed activation rejected: {error}");
            process::exit(1);
        });
    let activation_digest = signed.activation.signing_digest().unwrap_or_else(|error| {
        eprintln!("signed activation rejected: {error}");
        process::exit(1);
    });

    println!("authority_policy_sha256: {policy_digest}");
    println!("activation_signing_sha256: {activation_digest}");
    println!("approvals_verified: {}", signed.approvals.len());
    println!("threshold: {}", policy.threshold);
}
