//! Bootstrap invoked only as a child of the native macOS application.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use std::fs;
use std::path::{Path, PathBuf};

use crate::native_session::{NativeSessionActivation, NativeSessionProof, NATIVE_SIGNATURE_DOMAIN};
use crate::service_client::ServiceClient;
use crate::service_commands::ServicePaths;
use crate::workspace_identity::{KeychainSecretStore, WorkspaceIdentity};

const MAX_TEAM_ID_BYTES: u64 = 32;

pub async fn issue_session(
) -> Result<crate::native_session::NativeSessionCredential, Box<dyn std::error::Error + Send + Sync>>
{
    verify_native_parent()?;
    crate::native_install::install_bundled_core_if_present()?;
    let service = ServiceClient::ensure_running().await?;
    let challenge = service.native_session_challenge().await?;
    let paths =
        ServicePaths::discover().map_err(|error| std::io::Error::other(error.to_string()))?;
    let workspace = WorkspaceIdentity::load_or_create(&paths.service_state, &KeychainSecretStore)?;
    if challenge.instance_id != service.runtime().instance_id
        || challenge.client_id != crate::native_session::NATIVE_CLIENT_ID
    {
        return Err("native session challenge belongs to another service or client".into());
    }
    let proof = NativeSessionProof::from(&challenge);
    let bytes = tohseno_protocol::canonical::to_vec(&proof)?;
    let activation = NativeSessionActivation {
        proof,
        signature_base64url: URL_SAFE_NO_PAD
            .encode(workspace.identity.sign(NATIVE_SIGNATURE_DOMAIN, &bytes)),
    };
    let credential = service.activate_native_session(&activation).await?;
    if credential.instance_id != service.runtime().instance_id
        || credential.origin != service.runtime().origin
        || credential.client_id != crate::native_session::NATIVE_CLIENT_ID
        || credential.token_type != "TohsenoNative"
        || !credential
            .scopes
            .iter()
            .any(|scope| scope == "factory.read")
        || !credential
            .scopes
            .iter()
            .any(|scope| scope == "factory.mutate")
    {
        return Err("Local Workspace Service returned an invalid native session".into());
    }
    Ok(credential)
}

#[cfg(target_os = "macos")]
fn verify_native_parent() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Source checkouts need to launch the SwiftPM executable without an Apple
    // distribution signature. This branch is compiled out of release builds.
    if cfg!(debug_assertions)
        && std::env::var("TOHSENO_DEVELOPMENT_NATIVE_CLIENT").as_deref() == Ok("1")
    {
        return Ok(());
    }
    use security_framework::os::macos::code_signing::{Flags, GuestAttributes, SecCode};

    let team_id = read_native_team_id(&native_requirement_path()?)?;
    let team_requirement =
        format!("anchor apple generic and certificate leaf[subject.OU] = \"{team_id}\"").parse()?;
    let parent_requirement = format!(
        "identifier \"com.tohseno.mac\" and anchor apple generic and certificate leaf[subject.OU] = \"{team_id}\""
    )
    .parse()?;
    let validation =
        Flags::STRICT_VALIDATE | Flags::CHECK_TRUSTED_ANCHORS | Flags::NO_NETWORK_ACCESS;
    SecCode::for_self(Flags::NONE)?.check_validity(validation, &team_requirement)?;
    let mut attributes = GuestAttributes::new();
    let parent = unsafe { libc::getppid() };
    if parent <= 1 {
        return Err("native session helper has no valid parent process".into());
    }
    attributes.set_pid(parent);
    let parent_code = SecCode::copy_guest_with_attribues(None, &attributes, Flags::NONE)?;
    parent_code.check_validity(validation, &parent_requirement)?;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn verify_native_parent() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    Err("native client sessions require macOS code-signing validation".into())
}

fn native_requirement_path() -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
    #[cfg(debug_assertions)]
    if let Some(path) = std::env::var_os("TOHSENO_TEST_NATIVE_REQUIREMENT") {
        let path = PathBuf::from(path);
        if !path.is_absolute() {
            return Err("test native requirement path must be absolute".into());
        }
        return Ok(path);
    }
    let executable = std::env::current_exe()?;
    let helpers = executable
        .parent()
        .ok_or("native helper executable has no parent directory")?;
    let contents = helpers
        .parent()
        .ok_or("native helper is not inside an application bundle")?;
    if helpers.file_name().and_then(|name| name.to_str()) != Some("Helpers")
        || contents.file_name().and_then(|name| name.to_str()) != Some("Contents")
    {
        return Err("native helper is not in Tohseno.app/Contents/Helpers".into());
    }
    Ok(contents.join("Resources/native-client-requirement.txt"))
}

fn read_native_team_id(path: &Path) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() == 0
        || metadata.len() > MAX_TEAM_ID_BYTES
    {
        return Err("native client code requirement is unsafe".into());
    }
    let value = fs::read_to_string(path)?;
    let value = value.trim();
    if value.len() != 10
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
    {
        return Err("native client Team ID is invalid".into());
    }
    Ok(value.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_team_id_is_exact_and_bounded() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("requirement");
        fs::write(&path, "TEAM123456\n").unwrap();
        assert_eq!(read_native_team_id(&path).unwrap(), "TEAM123456");
        for invalid in ["team123456\n", "TEAM12345\n", "TEAM1234567\n"] {
            fs::write(&path, invalid).unwrap();
            assert!(read_native_team_id(&path).is_err());
        }
    }
}
