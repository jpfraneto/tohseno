use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tohseno_companion::identity::WorkspaceServiceIdentity;
use zeroize::Zeroizing;

const WORKSPACE_SCHEMA: &str = "tohseno.local-workspace/1";
const KEYCHAIN_SERVICE: &str = "com.tohseno.workspace-service";
const VERIFICATION_KEYCHAIN_PREFIX: &str = "com.tohseno.workspace-service.verification.";
const MAX_WORKSPACE_RECORD_BYTES: u64 = 64 * 1024;
/// How long the workspace-secret read may block before the service says why.
/// macOS raises a Keychain authorization dialog whenever the requesting binary
/// is not on the item's access list, and `SecItemCopyMatching` does not return
/// until somebody answers it. Waiting silently is indistinguishable from a hang.
const KEYCHAIN_NOTICE_DELAY: Duration = Duration::from_secs(3);
/// Repeated so a long unanswered dialog stays visibly the cause, rather than
/// one line scrolled past at startup.
const KEYCHAIN_NOTICE_INTERVAL: Duration = Duration::from_secs(30);
pub const KEYCHAIN_NOTICE: &str = "macOS is asking permission to read the TOHSENO workspace key. Answer the Keychain dialog with Always Allow; the service cannot start until it is answered.";

pub trait SecretStore: Send + Sync {
    fn put(&self, reference: &str, value: &[u8]) -> Result<(), String>;
    fn get(&self, reference: &str) -> Result<Vec<u8>, String>;
    fn delete(&self, reference: &str) -> Result<(), String>;
}

#[derive(Clone, Debug, Default)]
pub struct MemorySecretStore {
    values: Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
}

impl SecretStore for MemorySecretStore {
    fn put(&self, reference: &str, value: &[u8]) -> Result<(), String> {
        self.values
            .lock()
            .map_err(|_| "test secret store lock failed".to_owned())?
            .insert(reference.into(), value.to_vec());
        Ok(())
    }

    fn get(&self, reference: &str) -> Result<Vec<u8>, String> {
        self.values
            .lock()
            .map_err(|_| "test secret store lock failed".to_owned())?
            .get(reference)
            .cloned()
            .ok_or_else(|| "secret does not exist".into())
    }

    fn delete(&self, reference: &str) -> Result<(), String> {
        self.values
            .lock()
            .map_err(|_| "test secret store lock failed".to_owned())?
            .remove(reference);
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct KeychainSecretStore;

impl SecretStore for KeychainSecretStore {
    fn put(&self, reference: &str, value: &[u8]) -> Result<(), String> {
        validate_reference(reference)?;
        #[cfg(target_os = "macos")]
        {
            if let Some(keychain) = verification_keychain()? {
                return keychain
                    .set_generic_password(&configured_keychain_service()?, reference, value)
                    .map_err(|_| "macOS Keychain refused the workspace secret".to_owned());
            }
            security_framework::passwords::set_generic_password(
                &configured_keychain_service()?,
                reference,
                value,
            )
            .map_err(|_| "macOS Keychain refused the workspace secret".to_owned())
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = value;
            Err("workspace secrets require the macOS Keychain".into())
        }
    }

    fn get(&self, reference: &str) -> Result<Vec<u8>, String> {
        validate_reference(reference)?;
        #[cfg(target_os = "macos")]
        {
            if let Some(keychain) = verification_keychain()? {
                let (password, _) = keychain
                    .find_generic_password(&configured_keychain_service()?, reference)
                    .map_err(|_| {
                        "workspace secret is unavailable in the macOS Keychain".to_owned()
                    })?;
                return Ok(password.as_ref().to_vec());
            }
            security_framework::passwords::get_generic_password(
                &configured_keychain_service()?,
                reference,
            )
            .map_err(|_| "workspace secret is unavailable in the macOS Keychain".to_owned())
        }
        #[cfg(not(target_os = "macos"))]
        {
            Err("workspace secrets require the macOS Keychain".into())
        }
    }

    fn delete(&self, reference: &str) -> Result<(), String> {
        validate_reference(reference)?;
        #[cfg(target_os = "macos")]
        {
            if let Some(keychain) = verification_keychain()? {
                let (_, item) = keychain
                    .find_generic_password(&configured_keychain_service()?, reference)
                    .map_err(|_| {
                        "workspace secret is unavailable in the macOS Keychain".to_owned()
                    })?;
                item.delete();
                return Ok(());
            }
            security_framework::passwords::delete_generic_password(
                &configured_keychain_service()?,
                reference,
            )
            .map_err(|_| "macOS Keychain refused workspace-secret removal".to_owned())
        }
        #[cfg(not(target_os = "macos"))]
        {
            Err("workspace secrets require the macOS Keychain".into())
        }
    }
}

#[cfg(target_os = "macos")]
fn verification_keychain(
) -> Result<Option<security_framework::os::macos::keychain::SecKeychain>, String> {
    use security_framework::os::macos::keychain::SecKeychain;
    use std::path::PathBuf;

    if std::env::var("TOHSENO_VERIFICATION_MODE").as_deref() != Ok("1") {
        return Ok(None);
    }
    let path = PathBuf::from(
        std::env::var_os("TOHSENO_VERIFICATION_KEYCHAIN_PATH")
            .ok_or("verification mode requires TOHSENO_VERIFICATION_KEYCHAIN_PATH")?,
    );
    if !path.is_absolute() {
        return Err("verification Keychain path must be absolute".into());
    }
    let metadata = fs::symlink_metadata(&path)
        .map_err(|_| "verification Keychain path is unavailable".to_owned())?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("verification Keychain path must be a regular file".into());
    }
    let mut keychain = SecKeychain::open(path)
        .map_err(|_| "verification Keychain could not be opened".to_owned())?;
    keychain
        .unlock(Some(""))
        .map_err(|_| "verification Keychain could not be unlocked".to_owned())?;
    Ok(Some(keychain))
}

fn configured_keychain_service() -> Result<String, String> {
    if std::env::var("TOHSENO_VERIFICATION_MODE").as_deref() != Ok("1") {
        return Ok(KEYCHAIN_SERVICE.into());
    }
    let service = std::env::var("TOHSENO_VERIFICATION_KEYCHAIN_SERVICE")
        .map_err(|_| "verification mode requires TOHSENO_VERIFICATION_KEYCHAIN_SERVICE")?;
    let suffix = service
        .strip_prefix(VERIFICATION_KEYCHAIN_PREFIX)
        .ok_or_else(|| "verification Keychain service has the wrong namespace".to_owned())?;
    if suffix.is_empty()
        || service.len() > 128
        || !suffix
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
    {
        return Err("verification Keychain service is invalid".into());
    }
    Ok(service)
}

/// Read one secret without letting an unanswered Keychain dialog look like a
/// freeze. The read stays on this thread; a watcher announces the delay, so a
/// service that appears stuck always states its own cause.
fn read_secret_announcing_delay(
    secrets: &dyn SecretStore,
    reference: &str,
    notice_delay: Duration,
    announce: impl Fn() + Send + 'static,
) -> Result<Vec<u8>, String> {
    let finished = Arc::new(AtomicBool::new(false));
    let watched = Arc::clone(&finished);
    let watcher = std::thread::spawn(move || {
        let started = Instant::now();
        let mut announced_at = None;
        while !watched.load(Ordering::Relaxed) {
            let due = match announced_at {
                None => started.elapsed() >= notice_delay,
                Some(last) => Instant::now().duration_since(last) >= KEYCHAIN_NOTICE_INTERVAL,
            };
            if due {
                announce();
                announced_at = Some(Instant::now());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    });
    let result = secrets.get(reference);
    finished.store(true, Ordering::Relaxed);
    let _ = watcher.join();
    result
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceIdentityRecord {
    pub schema: String,
    pub workspace_id: String,
    pub studio_device_id: String,
    pub studio_signing_public_key: String,
    pub studio_agreement_public_key: String,
    pub secret_reference: String,
    pub created_at: String,
}

pub struct WorkspaceIdentity {
    pub record: WorkspaceIdentityRecord,
    pub identity: Arc<WorkspaceServiceIdentity>,
}

impl WorkspaceIdentity {
    pub fn load_or_create(
        service_root: &Path,
        secrets: &dyn SecretStore,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        ensure_private_directory(service_root)?;
        let path = service_root.join("workspace.json");
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                Err("workspace identity path is unsafe".into())
            }
            Ok(_) => {
                let bytes = read_bounded(&path, MAX_WORKSPACE_RECORD_BYTES)?;
                let record: WorkspaceIdentityRecord =
                    tohseno_protocol::canonical::from_slice(&bytes)?;
                validate_record(&record)?;
                let secret = Zeroizing::new(
                    read_secret_announcing_delay(
                        secrets,
                        &record.secret_reference,
                        KEYCHAIN_NOTICE_DELAY,
                        || eprintln!("{KEYCHAIN_NOTICE}"),
                    )
                    .map_err(io_error)?,
                );
                if secret.len() != 64 {
                    return Err("workspace Keychain secret has the wrong length".into());
                }
                let mut secret_bytes = Zeroizing::new([0_u8; 64]);
                secret_bytes.copy_from_slice(&secret);
                let identity = WorkspaceServiceIdentity::from_secret_keys(
                    secret_bytes[..32].try_into().expect("slice length"),
                    secret_bytes[32..].try_into().expect("slice length"),
                )?;
                verify_identity(&record, &identity)?;
                Ok(Self {
                    record,
                    identity: Arc::new(identity),
                })
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let mut workspace_random = [0_u8; 18];
                let mut secret = Zeroizing::new([0_u8; 64]);
                OsRng.fill_bytes(&mut workspace_random);
                OsRng.fill_bytes(secret.as_mut());
                let identity = WorkspaceServiceIdentity::from_secret_keys(
                    secret[..32].try_into().expect("slice length"),
                    secret[32..].try_into().expect("slice length"),
                )?;
                let workspace_id =
                    format!("workspace_{}", URL_SAFE_NO_PAD.encode(workspace_random));
                let secret_reference = format!("workspace-seed:{workspace_id}");
                secrets
                    .put(&secret_reference, secret.as_ref())
                    .map_err(io_error)?;
                let record = WorkspaceIdentityRecord {
                    schema: WORKSPACE_SCHEMA.into(),
                    workspace_id,
                    studio_device_id: identity.device_id().into(),
                    studio_signing_public_key: identity.signing_public_key_base64url(),
                    studio_agreement_public_key: identity.agreement_public_key_base64url(),
                    secret_reference,
                    created_at: now(),
                };
                if let Err(error) = write_new_json(&path, &record) {
                    let _ = secrets.delete(&record.secret_reference);
                    return Err(error);
                }
                Ok(Self {
                    record,
                    identity: Arc::new(identity),
                })
            }
            Err(error) => Err(error.into()),
        }
    }
}

fn validate_record(record: &WorkspaceIdentityRecord) -> Result<(), String> {
    if record.schema != WORKSPACE_SCHEMA
        || !record.workspace_id.starts_with("workspace_")
        || !record.studio_device_id.starts_with("device_")
        || !record
            .secret_reference
            .starts_with("workspace-seed:workspace_")
        || record.workspace_id.len() > 128
        || record.studio_device_id.len() > 128
    {
        return Err("workspace identity record is invalid".into());
    }
    tohseno_companion::parse_timestamp(&record.created_at).map_err(|error| error.to_string())?;
    Ok(())
}

fn verify_identity(
    record: &WorkspaceIdentityRecord,
    identity: &WorkspaceServiceIdentity,
) -> Result<(), String> {
    if record.studio_device_id != identity.device_id()
        || record.studio_signing_public_key != identity.signing_public_key_base64url()
        || record.studio_agreement_public_key != identity.agreement_public_key_base64url()
    {
        return Err("workspace Keychain secret does not match public workspace identity".into());
    }
    Ok(())
}

fn io_error(message: String) -> std::io::Error {
    std::io::Error::other(message)
}

fn validate_reference(reference: &str) -> Result<(), String> {
    if reference.is_empty()
        || reference.len() > 200
        || !reference
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'))
    {
        return Err("invalid Keychain reference".into());
    }
    Ok(())
}

fn write_new_json<T: Serialize>(
    path: &Path,
    value: &T,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let bytes = tohseno_protocol::canonical::to_vec(value)?;
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let mut file = options.open(path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    File::open(path.parent().ok_or("workspace file has no parent")?)?.sync_all()?;
    Ok(())
}

fn read_bounded(
    path: &Path,
    maximum: u64,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    use std::io::Read;
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > maximum {
        return Err("workspace record is not a bounded regular file".into());
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let file = options.open(path)?;
    let opened = file.metadata()?;
    if !opened.is_file() || opened.len() != metadata.len() {
        return Err("workspace record changed while opening".into());
    }
    let mut bytes = Vec::with_capacity(usize::try_from(opened.len()).unwrap_or(0));
    file.take(maximum + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > maximum {
        return Err("workspace record exceeds its bound".into());
    }
    Ok(bytes)
}

fn ensure_private_directory(path: &Path) -> Result<(), std::io::Error> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "service state path is unsafe",
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir_all(path)?,
        Err(error) => return Err(error),
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn now() -> String {
    OffsetDateTime::now_utc()
        .replace_nanosecond(0)
        .expect("zero nanoseconds are valid")
        .format(&Rfc3339)
        .expect("UTC timestamps format")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A store whose read blocks, standing in for an unanswered macOS Keychain
    /// authorization dialog.
    struct SlowSecretStore(Duration);

    impl SecretStore for SlowSecretStore {
        fn put(&self, _reference: &str, _value: &[u8]) -> Result<(), String> {
            Ok(())
        }

        fn get(&self, _reference: &str) -> Result<Vec<u8>, String> {
            std::thread::sleep(self.0);
            Ok(b"secret".to_vec())
        }

        fn delete(&self, _reference: &str) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn a_blocked_secret_read_announces_its_cause_and_still_returns() {
        let announced = Arc::new(AtomicBool::new(false));
        let observed = Arc::clone(&announced);
        let secret = read_secret_announcing_delay(
            &SlowSecretStore(Duration::from_millis(300)),
            "workspace-seed:test",
            Duration::from_millis(20),
            move || observed.store(true, Ordering::Relaxed),
        )
        .unwrap();
        assert_eq!(secret, b"secret".to_vec());
        assert!(
            announced.load(Ordering::Relaxed),
            "a read that outlasts the notice delay must state why it is waiting"
        );
    }

    #[test]
    fn a_prompt_free_secret_read_stays_silent() {
        let announced = Arc::new(AtomicBool::new(false));
        let observed = Arc::clone(&announced);
        let store = MemorySecretStore::default();
        store.put("workspace-seed:test", b"secret").unwrap();
        let secret = read_secret_announcing_delay(
            &store,
            "workspace-seed:test",
            Duration::from_secs(30),
            move || observed.store(true, Ordering::Relaxed),
        )
        .unwrap();
        assert_eq!(secret, b"secret".to_vec());
        assert!(
            !announced.load(Ordering::Relaxed),
            "the ordinary authorized read must not narrate itself"
        );
    }

    #[test]
    fn injectable_store_round_trips_without_writing_secret_bytes() {
        let root = tempfile::tempdir().unwrap();
        let store = MemorySecretStore::default();
        let first = WorkspaceIdentity::load_or_create(root.path(), &store).unwrap();
        let record = fs::read_to_string(root.path().join("workspace.json")).unwrap();
        let secret = store.get(&first.record.secret_reference).unwrap();
        assert!(!record.contains(&URL_SAFE_NO_PAD.encode(secret)));
        let second = WorkspaceIdentity::load_or_create(root.path(), &store).unwrap();
        assert_eq!(first.record, second.record);
        assert_eq!(
            first.identity.signing_public_key(),
            second.identity.signing_public_key()
        );
    }

    #[cfg(unix)]
    #[test]
    fn workspace_record_symlink_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let service = root.path().join("service");
        fs::create_dir(&service).unwrap();
        std::os::unix::fs::symlink(root.path(), service.join("workspace.json")).unwrap();
        assert!(
            WorkspaceIdentity::load_or_create(&service, &MemorySecretStore::default()).is_err()
        );
    }
}
