use std::env;
use std::fs;
use std::process::ExitCode;

use tohseno_network::claims_activation::SignedClaimsActivation;
use tohseno_protocol::canonical;
use tohseno_protocol::contract_activation::ReleaseAuthorityPolicy;
use tohseno_protocol::digest::Bytes32;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Claims activation verification failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    if arguments.len() != 3 {
        return Err(
            "usage: verify_signed_claims_activation POLICY SIGNED_ACTIVATION TRUSTED_POLICY_SHA256"
                .into(),
        );
    }

    let policy_bytes = fs::read(&arguments[0])?;
    let signed_bytes = fs::read(&arguments[1])?;
    let policy: ReleaseAuthorityPolicy = canonical::from_slice(&policy_bytes)?;
    let signed: SignedClaimsActivation = canonical::from_slice(&signed_bytes)?;
    let trusted_policy = Bytes32::from_hex("trusted_policy_sha256", &arguments[2])?;

    if policy.digest()? != trusted_policy {
        return Err("the supplied authority policy differs from the explicit trust root".into());
    }
    signed.verify(&policy)?;
    let digest = signed.activation.signing_digest()?;

    println!("claims_activation_signing_sha256: {digest}");
    println!("claims_contract: {}", signed.activation.claims_contract);
    println!("shot_registry: {}", signed.activation.shot_registry);
    println!(
        "runtime_code_keccak256: {}",
        signed.activation.runtime_code_keccak256
    );
    println!(
        "deployment_block: {}",
        signed.activation.deployment.block_number
    );
    println!("approvals_verified: {}", signed.approvals.len());
    Ok(())
}
