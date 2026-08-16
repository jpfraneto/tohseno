//! Durable machine-local pending intentions.
//!
//! Pending intentions are private user state under the ledger's machine root.
//! They are transport inputs, not canonical Shots, and survive program removal.

use crate::intent_package::{parse_intent_package, IntentPackage, IntentPackageError};
use crate::ledger::{Ledger, LedgerError};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const STORE_DIRECTORY: &str = "pending-intentions";
const RECORD_SCHEMA: &str = "tohseno.local-pending-intention/1";
const RECEIPT_SCHEMA: &str = "tohseno.local-pending-intention-receipt/1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PendingIntentionSource {
    Relay,
    PortableFile,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PendingIntentionState {
    Ready,
    Consumed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalPendingReference {
    pub ordinal: usize,
    pub display_filename: String,
    pub media_type: String,
    pub byte_length: u64,
    pub sha256: String,
    storage_file: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalPendingIntention {
    pub schema: String,
    pub id: String,
    pub package_schema: String,
    pub package_sha256: String,
    pub prompt: String,
    pub prompt_sha256: String,
    pub references: Vec<LocalPendingReference>,
    pub imported_at: String,
    pub source: PendingIntentionSource,
    pub state: PendingIntentionState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub import_receipt: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    schema: String,
    package_sha256: String,
    pending_id: String,
    state: PendingIntentionState,
    imported_at: String,
}

#[derive(Debug)]
pub enum PendingIntentionError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Ledger(LedgerError),
    Package(IntentPackageError),
    Invalid(String),
}

impl std::fmt::Display for PendingIntentionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Json(error) => write!(formatter, "{error}"),
            Self::Ledger(error) => write!(formatter, "{error}"),
            Self::Package(error) => write!(formatter, "{error}"),
            Self::Invalid(reason) => formatter.write_str(reason),
        }
    }
}

impl std::error::Error for PendingIntentionError {}

impl From<std::io::Error> for PendingIntentionError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}
impl From<serde_json::Error> for PendingIntentionError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}
impl From<LedgerError> for PendingIntentionError {
    fn from(value: LedgerError) -> Self {
        Self::Ledger(value)
    }
}
impl From<IntentPackageError> for PendingIntentionError {
    fn from(value: IntentPackageError) -> Self {
        Self::Package(value)
    }
}

#[derive(Clone, Debug)]
pub struct PendingIntentionStore {
    root: PathBuf,
}

impl PendingIntentionStore {
    pub fn for_ledger(ledger: &Ledger) -> Self {
        Self {
            root: ledger.machine_root().join(STORE_DIRECTORY),
        }
    }

    pub fn import_bytes(
        &self,
        bytes: &[u8],
        source: PendingIntentionSource,
    ) -> Result<LocalPendingIntention, PendingIntentionError> {
        let package = parse_intent_package(bytes)?;
        self.import(&package, source)
    }

    pub fn import(
        &self,
        package: &IntentPackage,
        source: PendingIntentionSource,
    ) -> Result<LocalPendingIntention, PendingIntentionError> {
        self.initialize()?;
        let _lock = self.acquire_lock()?;
        if let Some(existing) = self.find_by_digest(&package.package_sha256)? {
            return Ok(existing);
        }
        let id = random_id();
        let imported_at = now_unix_seconds()?;
        let stage = self.root.join(format!(".stage-{id}"));
        create_private_directory(&stage)?;
        let result = (|| {
            let references_dir = stage.join("references");
            create_private_directory(&references_dir)?;
            let references = package
                .references
                .iter()
                .map(|reference| {
                    let storage_file = format!("{:06}", reference.ordinal);
                    write_private_new(&references_dir.join(&storage_file), &reference.bytes)?;
                    Ok(LocalPendingReference {
                        ordinal: reference.ordinal,
                        display_filename: reference.display_filename.clone(),
                        media_type: reference.media_type.clone(),
                        byte_length: reference.bytes.len() as u64,
                        sha256: reference.sha256.clone(),
                        storage_file,
                    })
                })
                .collect::<Result<Vec<_>, PendingIntentionError>>()?;
            File::open(&references_dir)?.sync_all()?;
            let record = LocalPendingIntention {
                schema: RECORD_SCHEMA.into(),
                id: id.clone(),
                package_schema: crate::intent_package::INTENT_PACKAGE_SCHEMA.into(),
                package_sha256: package.package_sha256.clone(),
                prompt: package.prompt.clone(),
                prompt_sha256: digest_hex(package.prompt.as_bytes()),
                references,
                imported_at: imported_at.clone(),
                source,
                state: PendingIntentionState::Ready,
                import_receipt: None,
            };
            write_private_new(&stage.join("record.json"), &serde_json::to_vec(&record)?)?;
            File::open(&stage)?.sync_all()?;
            let records = self.root.join("records");
            let destination = records.join(&id);
            fs::rename(&stage, &destination)?;
            File::open(&records)?.sync_all()?;
            self.write_receipt(&Receipt {
                schema: RECEIPT_SCHEMA.into(),
                package_sha256: package.package_sha256.clone(),
                pending_id: id,
                state: PendingIntentionState::Ready,
                imported_at,
            })?;
            Ok(record)
        })();
        if stage.exists() {
            let _ = fs::remove_dir_all(&stage);
        }
        result
    }

    pub fn load(&self, id: &str) -> Result<LocalPendingIntention, PendingIntentionError> {
        validate_id(id)?;
        let directory = self.root.join("records").join(id);
        require_real_directory(&directory)?;
        let record = read_private_json::<LocalPendingIntention>(&directory.join("record.json"))?;
        validate_record(&record, id)?;
        if record.state != PendingIntentionState::Ready {
            return Err(PendingIntentionError::Invalid(
                "pending intention is not ready".into(),
            ));
        }
        Ok(record)
    }

    pub fn read_reference(
        &self,
        id: &str,
        ordinal: usize,
    ) -> Result<Vec<u8>, PendingIntentionError> {
        let record = self.load(id)?;
        let reference = record.references.get(ordinal).ok_or_else(|| {
            PendingIntentionError::Invalid("pending intention reference does not exist".into())
        })?;
        let path = self
            .root
            .join("records")
            .join(id)
            .join("references")
            .join(&reference.storage_file);
        let bytes = read_private_bounded(&path, reference.byte_length)?;
        if bytes.len() as u64 != reference.byte_length {
            return Err(PendingIntentionError::Invalid(
                "pending intention reference storage is unsafe or corrupt".into(),
            ));
        }
        crate::shot_layout::validate_private_reference_bytes(
            &reference.display_filename,
            &reference.media_type,
            &bytes,
        )
        .map_err(|error| PendingIntentionError::Invalid(error.to_string()))?;
        let digest = tohseno_protocol::digest::sha256(&bytes).to_hex();
        if digest.trim_start_matches("0x") != reference.sha256 {
            return Err(PendingIntentionError::Invalid(
                "pending intention reference digest changed".into(),
            ));
        }
        Ok(bytes)
    }

    pub fn materialize_references(
        &self,
        id: &str,
        destination: &Path,
    ) -> Result<Vec<PathBuf>, PendingIntentionError> {
        if destination.exists() {
            return Err(PendingIntentionError::Invalid(
                "reference staging destination already exists".into(),
            ));
        }
        create_private_directory(destination)?;
        let record = self.load(id)?;
        let mut paths = Vec::with_capacity(record.references.len());
        for reference in &record.references {
            let bytes = self.read_reference(id, reference.ordinal)?;
            let path = destination.join(&reference.display_filename);
            write_private_new(&path, &bytes)?;
            paths.push(path);
        }
        File::open(destination)?.sync_all()?;
        Ok(paths)
    }

    pub fn consume(&self, id: &str) -> Result<(), PendingIntentionError> {
        let record = self.load(id)?;
        self.consume_loaded(&record)
    }

    /// Consume an exact record idempotently after its factory command has been
    /// durably admitted. Supplying the loaded record lets a concurrent retry
    /// recognize the already-consumed receipt without re-reading deleted
    /// private content.
    pub fn consume_loaded(
        &self,
        expected: &LocalPendingIntention,
    ) -> Result<(), PendingIntentionError> {
        self.initialize()?;
        validate_record(expected, &expected.id)?;
        let _lock = self.acquire_lock()?;
        let receipt_path = self
            .root
            .join("receipts")
            .join(format!("{}.json", expected.package_sha256));
        match fs::symlink_metadata(&receipt_path) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                return Err(PendingIntentionError::Invalid(
                    "pending intention receipt is unsafe".into(),
                ));
            }
            Ok(_) => {
                let receipt = read_private_json::<Receipt>(&receipt_path)?;
                if receipt.schema == RECEIPT_SCHEMA
                    && receipt.package_sha256 == expected.package_sha256
                    && receipt.pending_id == expected.id
                    && receipt.state == PendingIntentionState::Consumed
                {
                    let directory = self.root.join("records").join(&expected.id);
                    if fs::symlink_metadata(&directory).is_ok() {
                        require_real_directory(&directory)?;
                        let current = self.load(&expected.id)?;
                        if current != *expected {
                            return Err(PendingIntentionError::Invalid(
                                "pending intention changed before consumption".into(),
                            ));
                        }
                        fs::remove_dir_all(&directory)?;
                        File::open(self.root.join("records"))?.sync_all()?;
                    }
                    return Ok(());
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let record = self.load(&expected.id)?;
        if record != *expected {
            return Err(PendingIntentionError::Invalid(
                "pending intention changed before consumption".into(),
            ));
        }
        let receipt = Receipt {
            schema: RECEIPT_SCHEMA.into(),
            package_sha256: record.package_sha256,
            pending_id: record.id.clone(),
            state: PendingIntentionState::Consumed,
            imported_at: record.imported_at,
        };
        self.write_receipt(&receipt)?;
        let directory = self.root.join("records").join(&record.id);
        require_real_directory(&directory)?;
        fs::remove_dir_all(&directory)?;
        File::open(self.root.join("records"))?.sync_all()?;
        Ok(())
    }

    fn acquire_lock(&self) -> Result<File, PendingIntentionError> {
        let path = self.root.join("store.lock");
        let mut options = OpenOptions::new();
        options.create(true).read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
        }
        let file = options.open(path)?;
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
            if result != 0 {
                return Err(std::io::Error::last_os_error().into());
            }
        }
        Ok(file)
    }

    fn initialize(&self) -> Result<(), PendingIntentionError> {
        ensure_private_directory(&self.root)?;
        ensure_private_directory(&self.root.join("records"))?;
        ensure_private_directory(&self.root.join("receipts"))?;
        Ok(())
    }

    fn find_by_digest(
        &self,
        digest: &str,
    ) -> Result<Option<LocalPendingIntention>, PendingIntentionError> {
        validate_digest(digest)?;
        let receipt_path = self.root.join("receipts").join(format!("{digest}.json"));
        if receipt_path.exists() {
            let receipt = read_private_json::<Receipt>(&receipt_path)?;
            if receipt.schema != RECEIPT_SCHEMA || receipt.package_sha256 != digest {
                return Err(PendingIntentionError::Invalid(
                    "pending intention receipt is corrupt".into(),
                ));
            }
            if receipt.state == PendingIntentionState::Ready {
                return self.load(&receipt.pending_id).map(Some);
            }
            return Ok(Some(LocalPendingIntention {
                schema: RECORD_SCHEMA.into(),
                id: receipt.pending_id,
                package_schema: crate::intent_package::INTENT_PACKAGE_SCHEMA.into(),
                package_sha256: receipt.package_sha256,
                prompt: String::new(),
                prompt_sha256: digest_hex(b""),
                references: Vec::new(),
                imported_at: receipt.imported_at,
                source: PendingIntentionSource::PortableFile,
                state: PendingIntentionState::Consumed,
                import_receipt: Some("already-consumed".into()),
            }));
        }
        // Recover idempotently from a crash after the record rename but before
        // receipt publication.
        for entry in fs::read_dir(self.root.join("records"))? {
            let entry = entry?;
            let Some(id) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if validate_id(&id).is_err() {
                continue;
            }
            let record = self.load(&id)?;
            if record.package_sha256 == digest {
                return Ok(Some(record));
            }
        }
        Ok(None)
    }

    fn write_receipt(&self, receipt: &Receipt) -> Result<(), PendingIntentionError> {
        validate_digest(&receipt.package_sha256)?;
        let path = self
            .root
            .join("receipts")
            .join(format!("{}.json", receipt.package_sha256));
        write_private_replace(&path, &serde_json::to_vec(receipt)?)
    }
}

fn validate_record(
    record: &LocalPendingIntention,
    expected_id: &str,
) -> Result<(), PendingIntentionError> {
    if record.schema != RECORD_SCHEMA
        || record.package_schema != crate::intent_package::INTENT_PACKAGE_SCHEMA
        || record.id != expected_id
    {
        return Err(PendingIntentionError::Invalid(
            "pending intention record is corrupt".into(),
        ));
    }
    validate_id(&record.id)?;
    validate_digest(&record.package_sha256)?;
    if record.prompt.trim().is_empty()
        || record.references.len() > 8
        || digest_hex(record.prompt.as_bytes()) != record.prompt_sha256
    {
        return Err(PendingIntentionError::Invalid(
            "pending intention content is invalid".into(),
        ));
    }
    for (ordinal, reference) in record.references.iter().enumerate() {
        if reference.ordinal != ordinal || reference.storage_file != format!("{ordinal:06}") {
            return Err(PendingIntentionError::Invalid(
                "pending intention reference order is corrupt".into(),
            ));
        }
    }
    Ok(())
}

fn digest_hex(bytes: &[u8]) -> String {
    tohseno_protocol::digest::sha256(bytes)
        .to_hex()
        .trim_start_matches("0x")
        .to_owned()
}

fn random_id() -> String {
    let mut bytes = [0_u8; 16];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn validate_id(id: &str) -> Result<(), PendingIntentionError> {
    if id.len() != 32
        || id
            .as_bytes()
            .iter()
            .any(|byte| !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase())
    {
        return Err(PendingIntentionError::Invalid(
            "pending intention ID is malformed".into(),
        ));
    }
    Ok(())
}

fn validate_digest(digest: &str) -> Result<(), PendingIntentionError> {
    if digest.len() != 64
        || digest
            .as_bytes()
            .iter()
            .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(byte))
    {
        return Err(PendingIntentionError::Invalid(
            "pending intention digest is malformed".into(),
        ));
    }
    Ok(())
}

fn now_unix_seconds() -> Result<String, PendingIntentionError> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| PendingIntentionError::Invalid(error.to_string()))?
        .as_secs()
        .to_string())
}

fn ensure_private_directory(path: &Path) -> Result<(), PendingIntentionError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(PendingIntentionError::Invalid(
                "pending intention path is unsafe".into(),
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            create_private_directory(path)?
        }
        Err(error) => return Err(error.into()),
    }
    set_private_directory(path)?;
    Ok(())
}

fn require_real_directory(path: &Path) -> Result<(), PendingIntentionError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(PendingIntentionError::Invalid(
            "pending intention path is unsafe".into(),
        ));
    }
    Ok(())
}

fn create_private_directory(path: &Path) -> Result<(), PendingIntentionError> {
    fs::create_dir(path)?;
    set_private_directory(path)
}

#[cfg(unix)]
fn set_private_directory(path: &Path) -> Result<(), PendingIntentionError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_directory(_path: &Path) -> Result<(), PendingIntentionError> {
    Ok(())
}

fn write_private_new(path: &Path, bytes: &[u8]) -> Result<(), PendingIntentionError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn write_private_replace(path: &Path, bytes: &[u8]) -> Result<(), PendingIntentionError> {
    let parent = path
        .parent()
        .ok_or_else(|| PendingIntentionError::Invalid("pending receipt has no parent".into()))?;
    require_real_directory(parent)?;
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(PendingIntentionError::Invalid(
                "pending receipt target is unsafe".into(),
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let temporary = parent.join(format!(".receipt-{}", random_id()));
    write_private_new(&temporary, bytes)?;
    fs::rename(&temporary, path)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

fn read_private_json<T: for<'de> Deserialize<'de>>(
    path: &Path,
) -> Result<T, PendingIntentionError> {
    Ok(serde_json::from_slice(&read_private_bounded(
        path,
        2 * 1024 * 1024,
    )?)?)
}

fn read_private_bounded(path: &Path, maximum: u64) -> Result<Vec<u8>, PendingIntentionError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > maximum {
        return Err(PendingIntentionError::Invalid(
            "pending intention metadata is unsafe or too large".into(),
        ));
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
        return Err(PendingIntentionError::Invalid(
            "pending intention record changed while opening".into(),
        ));
    }
    let mut bytes = Vec::with_capacity(usize::try_from(opened.len()).unwrap_or(0));
    file.take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > maximum || bytes.len() as u64 != opened.len() {
        return Err(PendingIntentionError::Invalid(
            "pending intention record changed while reading".into(),
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intent_package::build_intent_package;

    fn package() -> Vec<u8> {
        build_intent_package(
            "2026-08-03T00:00:00Z",
            "Remember every tree.",
            &[(
                "tree.png".into(),
                "image/png".into(),
                b"\x89PNG\r\n\x1a\ntree".to_vec(),
            )],
        )
        .unwrap()
    }

    #[test]
    fn import_is_private_persistent_and_idempotent() {
        let temporary = tempfile::tempdir().unwrap();
        let ledger = Ledger::at(temporary.path().join("data"));
        ledger.initialize().unwrap();
        let store = PendingIntentionStore::for_ledger(&ledger);
        let first = store
            .import_bytes(&package(), PendingIntentionSource::PortableFile)
            .unwrap();
        let second = store
            .import_bytes(&package(), PendingIntentionSource::PortableFile)
            .unwrap();
        assert_eq!(first.id, second.id);
        assert_eq!(
            store.load(&first.id).unwrap().prompt,
            "Remember every tree."
        );
        assert_eq!(
            store.read_reference(&first.id, 0).unwrap(),
            b"\x89PNG\r\n\x1a\ntree"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(
                temporary
                    .path()
                    .join("data/pending-intentions/records")
                    .join(&first.id)
                    .join("record.json"),
            )
            .unwrap()
            .permissions()
            .mode();
            assert_eq!(mode & 0o077, 0);
        }
    }

    #[test]
    fn consume_removes_private_copy_and_keeps_idempotence_receipt() {
        let temporary = tempfile::tempdir().unwrap();
        let ledger = Ledger::at(temporary.path().join("data"));
        ledger.initialize().unwrap();
        let store = PendingIntentionStore::for_ledger(&ledger);
        let first = store
            .import_bytes(&package(), PendingIntentionSource::Relay)
            .unwrap();
        store.consume(&first.id).unwrap();
        assert!(store.load(&first.id).is_err());
        let retried = store
            .import_bytes(&package(), PendingIntentionSource::Relay)
            .unwrap();
        assert_eq!(retried.id, first.id);
        assert_eq!(retried.state, PendingIntentionState::Consumed);
    }

    #[test]
    fn consume_loaded_is_idempotent_for_concurrent_command_retries() {
        let temporary = tempfile::tempdir().unwrap();
        let ledger = Ledger::at(temporary.path().join("data"));
        ledger.initialize().unwrap();
        let store = PendingIntentionStore::for_ledger(&ledger);
        let pending = store
            .import_bytes(&package(), PendingIntentionSource::Relay)
            .unwrap();
        store.consume_loaded(&pending).unwrap();
        store.consume_loaded(&pending).unwrap();
        assert!(store.load(&pending.id).is_err());
    }

    #[test]
    fn rejects_a_locally_tampered_prompt_record() {
        let temporary = tempfile::tempdir().unwrap();
        let ledger = Ledger::at(temporary.path().join("data"));
        ledger.initialize().unwrap();
        let store = PendingIntentionStore::for_ledger(&ledger);
        let imported = store
            .import_bytes(&package(), PendingIntentionSource::Relay)
            .unwrap();
        let record_path = temporary
            .path()
            .join("data/pending-intentions/records")
            .join(&imported.id)
            .join("record.json");
        let mut record: serde_json::Value =
            serde_json::from_slice(&fs::read(&record_path).unwrap()).unwrap();
        record["prompt"] = "tampered".into();
        fs::write(&record_path, serde_json::to_vec(&record).unwrap()).unwrap();
        assert!(store.load(&imported.id).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_store() {
        use std::os::unix::fs::symlink;
        let temporary = tempfile::tempdir().unwrap();
        let ledger = Ledger::at(temporary.path().join("data"));
        ledger.initialize().unwrap();
        let elsewhere = temporary.path().join("elsewhere");
        fs::create_dir(&elsewhere).unwrap();
        symlink(&elsewhere, temporary.path().join("data/pending-intentions")).unwrap();
        let store = PendingIntentionStore::for_ledger(&ledger);
        assert!(store
            .import_bytes(&package(), PendingIntentionSource::Relay)
            .is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlinked_reference_component() {
        use std::os::unix::fs::symlink;
        let temporary = tempfile::tempdir().unwrap();
        let ledger = Ledger::at(temporary.path().join("data"));
        ledger.initialize().unwrap();
        let store = PendingIntentionStore::for_ledger(&ledger);
        let pending = store
            .import_bytes(&package(), PendingIntentionSource::Relay)
            .unwrap();
        let reference = temporary
            .path()
            .join("data/pending-intentions/records")
            .join(&pending.id)
            .join("references/000000");
        let original = temporary.path().join("original.png");
        fs::rename(&reference, &original).unwrap();
        symlink(&original, &reference).unwrap();
        assert!(store.read_reference(&pending.id, 0).is_err());
    }
}
