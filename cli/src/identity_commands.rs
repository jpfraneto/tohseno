use serde::Serialize;
use std::fs;
use std::io::{IsTerminal, Read};
use std::path::Path;
use tohseno_engine::builder_identity::{BuilderIdentity, BuilderIdentityManager};
use tohseno_engine::recovery::RecoveryVault;
use tohseno_engine::{Event, EventBus, Ledger};
use zeroize::Zeroizing;

pub fn show(bus: &EventBus, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let ledger = initialized_ledger()?;
    let identity = BuilderIdentityManager::for_ledger(&ledger).ensure()?;
    if json {
        print_json(&IdentityView::from(&identity))?;
    } else {
        bus.emit(Event::result(format!(
            "your TOHSENO identity is {}.",
            identity.builder_id
        )));
        bus.emit(Event::status(format!(
            "This Mac · {} · recovery {} · account {}.",
            identity.security_level,
            if identity.recovery.is_some() {
                "backup stored locally; account recovery unavailable"
            } else {
                "not configured"
            },
            match identity.deployment_status {
                tohseno_engine::builder_identity::BuilderDeploymentStatus::Predicted => "predicted",
                tohseno_engine::builder_identity::BuilderDeploymentStatus::Deployed => "deployed",
            }
        )));
    }
    Ok(())
}

pub fn devices(bus: &EventBus, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let ledger = initialized_ledger()?;
    let identity = BuilderIdentityManager::for_ledger(&ledger).ensure()?;
    let view = DeviceView {
        builder_id: identity.builder_id.to_string(),
        key_id: identity.device.key_id.to_string(),
        public_key: identity.device.public_key.clone(),
        planned_permissions: ["PROTOCOL", "DEVICE_ADMIN"],
        candidate_authority: "initial_device_only",
        replacement_status: "unsupported_without_authorization_proof",
        local: true,
        backend: identity.key_backend.clone(),
        test_only: identity.test_only,
        deployment_status: format!("{:?}", identity.deployment_status).to_lowercase(),
    };
    if json {
        print_json(&vec![view])?;
    } else {
        bus.emit(Event::result(
            "This Mac holds the only DeviceKey this candidate can verify.",
        ));
        bus.emit(Event::status(format!(
            "{} · {}{} · replacement and revocation unavailable",
            view.key_id,
            view.backend,
            if view.test_only { " · TEST ONLY" } else { "" }
        )));
    }
    Ok(())
}

pub fn backup(
    bus: &EventBus,
    json: bool,
    confirmed: bool,
    passphrase_file: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    if json {
        return Err("identity backup refuses --json because recovery words are secret".into());
    }
    if !confirmed {
        return Err(
            "re-run with --confirm after making sure nobody can see or record your screen".into(),
        );
    }
    let ledger = initialized_ledger()?;
    let manager = BuilderIdentityManager::for_ledger(&ledger);
    let mut identity = manager.ensure()?;
    let vault = RecoveryVault::at(ledger.machine_root().join("identity"));
    let passphrase = read_or_prompt_passphrase(passphrase_file, !vault.exists()?)?;
    let unlocked = if vault.exists()? {
        vault.backup(identity.builder_id, &passphrase)?
    } else {
        vault.create(identity.builder_id, &passphrase)?
    };
    if identity
        .recovery
        .as_ref()
        .is_some_and(|existing| existing != unlocked.authority())
    {
        return Err("encrypted recovery authority disagrees with builder.json".into());
    }
    identity.recovery = Some(unlocked.authority().clone());
    manager.save(&identity)?;

    eprintln!("Never paste these words into a website, generated app, chat, or support message.");
    eprintln!("Write them down offline in this exact order. TOHSENO will show them once here:");
    println!();
    println!("{}", unlocked.expose_mnemonic());
    println!();
    bus.emit(Event::result(
        "recovery is encrypted locally; public account configuration remains pending.",
    ));
    Ok(())
}

pub fn import_backup(
    bus: &EventBus,
    json: bool,
    confirmed: bool,
    mnemonic_file: Option<&Path>,
    passphrase_file: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    if json {
        return Err(
            "identity import-backup refuses --json because it accepts secret backup words".into(),
        );
    }
    if !confirmed {
        return Err(
            "re-run `identity import-backup --confirm` from a private screen after verifying the current BuilderID; this only imports an encrypted local backup".into(),
        );
    }
    let ledger = initialized_ledger()?;
    let manager = BuilderIdentityManager::for_ledger(&ledger);
    let mut identity = manager.ensure()?;
    let vault = RecoveryVault::at(ledger.machine_root().join("identity"));
    if vault.exists()? {
        return Err("a recovery vault already exists; use `identity backup` to verify it".into());
    }
    let words = if let Some(path) = mnemonic_file {
        read_secret_file(path, 4 * 1024)?
    } else {
        if !std::io::stdin().is_terminal() {
            return Err("backup words require a terminal or --mnemonic-file".into());
        }
        Zeroizing::new(rpassword::prompt_password("Recovery backup words: ")?)
    };
    let passphrase = read_or_prompt_passphrase(passphrase_file, true)?;
    let authority = vault.import(identity.builder_id, &words, &passphrase)?;
    identity.recovery = Some(authority);
    manager.save(&identity)?;
    bus.emit(Event::result(format!(
        "imported an encrypted local backup for the current BuilderID {}; this did not recover or rotate the account, authorize a replacement device, or submit an on-chain action.",
        identity.builder_id
    )));
    Ok(())
}

fn initialized_ledger() -> Result<Ledger, Box<dyn std::error::Error>> {
    let ledger = Ledger::discover()?;
    ledger.initialize()?;
    Ok(ledger)
}

fn read_or_prompt_passphrase(
    path: Option<&Path>,
    confirm_new: bool,
) -> Result<Zeroizing<String>, Box<dyn std::error::Error>> {
    if let Some(path) = path {
        return read_secret_file(path, 4 * 1024);
    }
    if !std::io::stdin().is_terminal() {
        return Err("vault passphrase requires a terminal or --passphrase-file".into());
    }
    let first = Zeroizing::new(rpassword::prompt_password("Recovery vault passphrase: ")?);
    if confirm_new {
        let second = Zeroizing::new(rpassword::prompt_password("Confirm passphrase: ")?);
        if *first != *second {
            return Err("passphrases did not match".into());
        }
    }
    Ok(first)
}

fn read_secret_file(
    path: &Path,
    limit: u64,
) -> Result<Zeroizing<String>, Box<dyn std::error::Error>> {
    if !path.is_absolute() {
        return Err("secret file paths must be absolute".into());
    }
    let initial = fs::symlink_metadata(path)?;
    if initial.file_type().is_symlink() || !initial.is_file() || initial.len() > limit {
        return Err("secret file must be a small regular non-symlinked file".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if initial.permissions().mode() & 0o077 != 0 {
            return Err("secret file permissions must not allow group or other access".into());
        }
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options.open(path)?;
    let opened = file.metadata()?;
    if !opened.is_file() || !same_file(&initial, &opened) {
        return Err("secret file changed while it was opened".into());
    }
    let mut bytes = Zeroizing::new(Vec::new());
    Read::take(&mut file, limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err("secret file exceeds its size limit".into());
    }
    let final_metadata = fs::symlink_metadata(path)?;
    if final_metadata.file_type().is_symlink()
        || !same_file(&initial, &final_metadata)
        || opened.len() != bytes.len() as u64
    {
        return Err("secret file changed while it was read".into());
    }
    let mut value = Zeroizing::new(String::from_utf8(bytes.to_vec())?);
    while value.ends_with('\n') || value.ends_with('\r') {
        value.pop();
    }
    if value.is_empty() {
        return Err("secret file is empty".into());
    }
    Ok(value)
}

#[cfg(unix)]
fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(not(unix))]
fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
        && left.file_type() == right.file_type()
}

fn print_json(value: &impl Serialize) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

#[derive(Serialize)]
struct IdentityView {
    builder_id: String,
    contract_generation: String,
    public_action_eligible: bool,
    chain_id: u64,
    account_address: String,
    factory_address: String,
    deployment_status: String,
    device_key_id: String,
    public_key: tohseno_protocol::signature::P256PublicKey,
    security_level: String,
    test_only: bool,
    recovery_status: &'static str,
    recovery_address: Option<String>,
}

impl From<&BuilderIdentity> for IdentityView {
    fn from(identity: &BuilderIdentity) -> Self {
        Self {
            builder_id: identity.builder_id.to_string(),
            contract_generation: identity.candidate_version.clone(),
            public_action_eligible: identity.is_current_generation().unwrap_or(false),
            chain_id: identity.chain_id,
            account_address: identity.account_address.to_string(),
            factory_address: identity.factory_address.to_string(),
            deployment_status: format!("{:?}", identity.deployment_status).to_lowercase(),
            device_key_id: identity.device.key_id.to_string(),
            public_key: identity.device.public_key.clone(),
            security_level: identity.security_level.clone(),
            test_only: identity.test_only,
            recovery_status: if identity.recovery.is_some() {
                "secured_locally_pending_onchain"
            } else {
                "pending"
            },
            recovery_address: identity
                .recovery
                .as_ref()
                .map(|recovery| recovery.address.to_string()),
        }
    }
}

#[derive(Serialize)]
struct DeviceView {
    builder_id: String,
    key_id: String,
    public_key: tohseno_protocol::signature::P256PublicKey,
    planned_permissions: [&'static str; 2],
    candidate_authority: &'static str,
    replacement_status: &'static str,
    local: bool,
    backend: String,
    test_only: bool,
    deployment_status: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn secret_file_reader_requires_a_private_regular_file_without_symlinks() {
        use std::os::unix::fs::{symlink, PermissionsExt};

        let temporary = tempfile::tempdir().unwrap();
        let secret = temporary.path().join("secret");
        fs::write(&secret, b"correct horse\n").unwrap();
        fs::set_permissions(&secret, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(&*read_secret_file(&secret, 64).unwrap(), "correct horse");

        let link = temporary.path().join("secret-link");
        symlink(&secret, &link).unwrap();
        assert!(read_secret_file(&link, 64).is_err());

        fs::set_permissions(&secret, fs::Permissions::from_mode(0o640)).unwrap();
        assert!(read_secret_file(&secret, 64).is_err());
    }
}
