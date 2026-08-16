use crate::command::{
    CommandKind, CommandOrigin, CommandRecord, CommandResult, CommandState, CommandStatus,
    COMMAND_SCHEMA, COMMAND_STATUS_SCHEMA,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tohseno_protocol::digest::{Bytes32, ExpressionId, ShotId, VersionId};
use tohseno_protocol::record::CanonicalTimestamp;
use uuid::Uuid;

const MAXIMUM_JSON_BYTES: u64 = 8 * 1024 * 1024;
const MAXIMUM_ATTACHMENT_BYTES: u64 = 64 * 1024 * 1024;
const MAXIMUM_TOTAL_ATTACHMENT_BYTES: u64 = 160 * 1024 * 1024;
const MAXIMUM_COMMANDS: usize = 100_000;
static TEMPORARY_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub enum JournalError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Protocol(tohseno_protocol::ProtocolError),
    Invalid(String),
    Conflict(String),
}

impl std::fmt::Display for JournalError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Json(error) => write!(formatter, "{error}"),
            Self::Protocol(error) => write!(formatter, "{error}"),
            Self::Invalid(message) | Self::Conflict(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for JournalError {}

impl From<std::io::Error> for JournalError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for JournalError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

impl From<tohseno_protocol::ProtocolError> for JournalError {
    fn from(value: tohseno_protocol::ProtocolError) -> Self {
        Self::Protocol(value)
    }
}

#[derive(Clone, Debug)]
pub struct CommandJournal {
    root: PathBuf,
}

#[derive(Clone, Debug)]
pub struct Admission {
    pub record: CommandRecord,
    pub status: CommandStatus,
    pub existing: bool,
    pub directory: PathBuf,
}

#[derive(Clone, Debug)]
pub struct AdmissionMetadata {
    pub command_id: String,
    pub command_kind: CommandKind,
    pub origin: CommandOrigin,
    pub origin_device_id: Option<String>,
    pub workspace_id: String,
    pub shot_id: Option<ShotId>,
    pub base_expression_id: Option<ExpressionId>,
    pub base_version_id: Option<VersionId>,
    pub submitted_at: Option<String>,
}

impl CommandJournal {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, JournalError> {
        let root = root.into();
        require_absolute(&root)?;
        ensure_private_directory(&root)?;
        ensure_private_directory(&root.join("command-journal"))?;
        Ok(Self {
            root: root.join("command-journal"),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn admit<T: Serialize>(
        &self,
        metadata: AdmissionMetadata,
        payload: &T,
    ) -> Result<Admission, JournalError> {
        self.admit_with_files(metadata, payload, &[])
    }

    pub fn admit_with_files<T: Serialize>(
        &self,
        metadata: AdmissionMetadata,
        payload: &T,
        files: &[(String, Vec<u8>)],
    ) -> Result<Admission, JournalError> {
        validate_identifier("command ID", &metadata.command_id)?;
        validate_identifier("workspace ID", &metadata.workspace_id)?;
        if let Some(device_id) = metadata.origin_device_id.as_deref() {
            validate_identifier("origin device ID", device_id)?;
        }
        validate_private_files(files)?;
        let payload_bytes = tohseno_protocol::canonical::to_vec(payload)?;
        if payload_bytes.len() as u64 > MAXIMUM_JSON_BYTES {
            return Err(JournalError::Invalid(
                "command payload exceeds the private journal limit".into(),
            ));
        }
        let payload_digest = Bytes32::new(Sha256::digest(&payload_bytes).into());
        let submitted_at = match metadata.submitted_at {
            Some(value) => {
                CanonicalTimestamp::parse(value.clone())?;
                value
            }
            None => now(),
        };
        let record = CommandRecord {
            schema: COMMAND_SCHEMA.into(),
            command_id: metadata.command_id,
            command_kind: metadata.command_kind,
            origin: metadata.origin,
            origin_device_id: metadata.origin_device_id,
            workspace_id: metadata.workspace_id,
            shot_id: metadata.shot_id,
            base_expression_id: metadata.base_expression_id,
            base_version_id: metadata.base_version_id,
            submitted_at,
            payload_digest,
        };
        let final_directory = self.command_directory(&record.command_id)?;
        match fs::symlink_metadata(&final_directory) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(JournalError::Invalid(
                    "command journal entry is not a real directory".into(),
                ));
            }
            Ok(_) => return self.read_existing(&record, &payload_bytes, files),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        if fs::read_dir(&self.root)?.take(MAXIMUM_COMMANDS).count() >= MAXIMUM_COMMANDS {
            return Err(JournalError::Invalid(
                "command journal is at capacity".into(),
            ));
        }
        let staging = self
            .root
            .join(format!(".staging-{}-{}", record.command_id, Uuid::new_v4()));
        fs::create_dir(&staging)?;
        set_private_directory(&staging)?;
        let initial = CommandStatus {
            schema: COMMAND_STATUS_SCHEMA.into(),
            command_id: record.command_id.clone(),
            state: CommandState::Received,
            updated_at: now(),
            result: None,
            rejection: None,
        };
        write_new_json(&staging.join("request.json"), &record)?;
        write_new_bytes(&staging.join("payload.json"), &payload_bytes)?;
        if !files.is_empty() {
            let inputs = staging.join("inputs");
            fs::create_dir(&inputs)?;
            set_private_directory(&inputs)?;
            for (name, bytes) in files {
                write_new_bytes(&inputs.join(name), bytes)?;
            }
            sync_directory(&inputs)?;
        }
        write_new_json(&staging.join("status.json"), &initial)?;
        sync_directory(&staging)?;
        match fs::rename(&staging, &final_directory) {
            Ok(()) => {
                sync_directory(&self.root)?;
                Ok(Admission {
                    record,
                    status: initial,
                    existing: false,
                    directory: final_directory,
                })
            }
            Err(error) => {
                // Directory rename collision errors differ by platform
                // (`AlreadyExists` on macOS and commonly `DirectoryNotEmpty`
                // on Linux). The published command directory is authoritative
                // in either case, so recheck it through the full idempotency
                // path instead of exposing a spurious admission failure.
                match fs::symlink_metadata(&final_directory) {
                    Ok(_) => {
                        remove_owned_staging(&staging)?;
                        self.read_existing(&record, &payload_bytes, files)
                    }
                    Err(observed) if observed.kind() == std::io::ErrorKind::NotFound => {
                        let _ = remove_owned_staging(&staging);
                        Err(error.into())
                    }
                    Err(observed) => {
                        let _ = remove_owned_staging(&staging);
                        Err(observed.into())
                    }
                }
            }
        }
    }

    pub fn transition(
        &self,
        command_id: &str,
        next: CommandState,
        receipt: Option<Value>,
        rejection: Option<String>,
    ) -> Result<CommandStatus, JournalError> {
        let _lock = self.lock_status(command_id)?;
        let (_, current) = self.load(command_id)?;
        if !current.state.may_transition_to(next) {
            return Err(JournalError::Conflict(format!(
                "command {command_id} cannot transition from {:?} to {:?}",
                current.state, next
            )));
        }
        let result = match (current.result.clone(), receipt) {
            (Some(existing), Some(candidate)) if existing.receipt != candidate => {
                return Err(JournalError::Conflict(format!(
                    "command {command_id} already has a different stable receipt"
                )));
            }
            (Some(existing), _) => Some(existing),
            (None, Some(receipt)) => Some(CommandResult { receipt }),
            (None, None) => None,
        };
        let rejection = rejection.or_else(|| current.rejection.clone());
        if next == CommandState::Completed && result.is_none() {
            return Err(JournalError::Invalid(
                "a completed command requires a stable receipt".into(),
            ));
        }
        if next == CommandState::Rejected && rejection.as_deref().is_none_or(str::is_empty) {
            return Err(JournalError::Invalid(
                "a rejected command requires an actionable reason".into(),
            ));
        }
        if current.state == next {
            if current.result == result && current.rejection == rejection {
                return Ok(current);
            }
            return Err(JournalError::Conflict(format!(
                "command {command_id} cannot replace data already published for state {next:?}"
            )));
        }
        let status = CommandStatus {
            schema: COMMAND_STATUS_SCHEMA.into(),
            command_id: command_id.into(),
            state: next,
            updated_at: now(),
            result,
            rejection,
        };
        let path = self.command_directory(command_id)?.join("status.json");
        write_replace_json(&path, &status)?;
        Ok(status)
    }

    pub fn load(&self, command_id: &str) -> Result<(CommandRecord, CommandStatus), JournalError> {
        let directory = self.command_directory(command_id)?;
        require_real_directory(&directory)?;
        let record: CommandRecord = read_json(&directory.join("request.json"))?;
        let status: CommandStatus = read_json(&directory.join("status.json"))?;
        validate_loaded(&record, &status, command_id)?;
        Ok((record, status))
    }

    pub fn payload<T: DeserializeOwned>(&self, command_id: &str) -> Result<T, JournalError> {
        let directory = self.command_directory(command_id)?;
        require_real_directory(&directory)?;
        let bytes = read_bounded(&directory.join("payload.json"), MAXIMUM_JSON_BYTES)?;
        let value = tohseno_protocol::canonical::from_slice(&bytes)?;
        let (record, _) = self.load(command_id)?;
        let digest = Bytes32::new(Sha256::digest(&bytes).into());
        if digest != record.payload_digest {
            return Err(JournalError::Invalid(
                "command payload no longer matches its admitted digest".into(),
            ));
        }
        Ok(value)
    }

    /// Reads one exact private attachment captured during admission. Callers
    /// must still compare it with the descriptor in the canonical payload.
    pub fn input(&self, command_id: &str, name: &str) -> Result<Vec<u8>, JournalError> {
        validate_file_name(name)?;
        let directory = self.command_directory(command_id)?;
        require_real_directory(&directory)?;
        read_bounded(
            &directory.join("inputs").join(name),
            MAXIMUM_ATTACHMENT_BYTES,
        )
    }

    pub fn write_operation_once<T: Serialize + DeserializeOwned + Eq>(
        &self,
        command_id: &str,
        operation: &str,
        value: &T,
    ) -> Result<T, JournalError> {
        validate_identifier("operation", operation)?;
        let directory = self.command_directory(command_id)?;
        require_real_directory(&directory)?;
        let operations = directory.join("operations");
        ensure_private_directory(&operations)?;
        let path = operations.join(format!("{operation}.json"));
        match write_new_json(&path, value) {
            Ok(()) => Ok(read_json(&path)?),
            Err(JournalError::Io(error)) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let existing: T = read_json(&path)?;
                if &existing != value {
                    return Err(JournalError::Conflict(format!(
                        "operation {operation} for command {command_id} already has a different result"
                    )));
                }
                Ok(existing)
            }
            Err(error) => Err(error),
        }
    }

    pub fn recoverable(&self) -> Result<Vec<CommandRecord>, JournalError> {
        let mut records = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if name.starts_with(".staging-") {
                continue;
            }
            let metadata = fs::symlink_metadata(entry.path())?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(JournalError::Invalid(
                    "command journal contains an unsafe entry".into(),
                ));
            }
            let (record, status) = self.load(name)?;
            if !status.state.terminal() {
                records.push(record);
            }
        }
        records.sort_by(|left, right| left.submitted_at.cmp(&right.submitted_at));
        Ok(records)
    }

    fn read_existing(
        &self,
        expected: &CommandRecord,
        payload_bytes: &[u8],
        files: &[(String, Vec<u8>)],
    ) -> Result<Admission, JournalError> {
        let (record, status) = self.load(&expected.command_id)?;
        let expected_identity = (
            &expected.command_kind,
            &expected.origin,
            &expected.origin_device_id,
            &expected.workspace_id,
            &expected.shot_id,
            &expected.base_expression_id,
            &expected.base_version_id,
            &expected.payload_digest,
        );
        let observed_identity = (
            &record.command_kind,
            &record.origin,
            &record.origin_device_id,
            &record.workspace_id,
            &record.shot_id,
            &record.base_expression_id,
            &record.base_version_id,
            &record.payload_digest,
        );
        if observed_identity != expected_identity {
            return Err(JournalError::Conflict(format!(
                "command ID {} is already bound to a different command",
                expected.command_id
            )));
        }
        let stored = read_bounded(
            &self
                .command_directory(&expected.command_id)?
                .join("payload.json"),
            MAXIMUM_JSON_BYTES,
        )?;
        if stored != payload_bytes {
            return Err(JournalError::Conflict(format!(
                "command ID {} payload bytes do not match its first admission",
                expected.command_id
            )));
        }
        let inputs = self.command_directory(&expected.command_id)?.join("inputs");
        let expected_names = files
            .iter()
            .map(|(name, _)| name.clone())
            .collect::<std::collections::BTreeSet<_>>();
        let mut observed_names = std::collections::BTreeSet::new();
        match fs::symlink_metadata(&inputs) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(JournalError::Invalid(
                    "command input collection is not a real directory".into(),
                ));
            }
            Ok(_) => {
                for entry in fs::read_dir(&inputs)? {
                    let entry = entry?;
                    let name = entry.file_name();
                    let name = name.to_str().ok_or_else(|| {
                        JournalError::Invalid("command input filename is not UTF-8".into())
                    })?;
                    validate_file_name(name)?;
                    let metadata = fs::symlink_metadata(entry.path())?;
                    if metadata.file_type().is_symlink() || !metadata.is_file() {
                        return Err(JournalError::Invalid(
                            "command input collection contains an unsafe entry".into(),
                        ));
                    }
                    observed_names.insert(name.to_owned());
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        if observed_names != expected_names {
            return Err(JournalError::Conflict(format!(
                "command ID {} private input set does not match its first admission",
                expected.command_id
            )));
        }
        for (name, expected_bytes) in files {
            let observed = read_bounded(&inputs.join(name), MAXIMUM_ATTACHMENT_BYTES)?;
            if &observed != expected_bytes {
                return Err(JournalError::Conflict(format!(
                    "command ID {} input {name} does not match its first admission",
                    expected.command_id
                )));
            }
        }
        Ok(Admission {
            directory: self.command_directory(&expected.command_id)?,
            record,
            status,
            existing: true,
        })
    }

    fn command_directory(&self, command_id: &str) -> Result<PathBuf, JournalError> {
        validate_identifier("command ID", command_id)?;
        Ok(self.root.join(command_id))
    }

    fn lock_status(&self, command_id: &str) -> Result<StatusLock, JournalError> {
        let directory = self.command_directory(command_id)?;
        require_real_directory(&directory)?;
        let mut options = OpenOptions::new();
        options.create(true).read(true).write(true);
        set_no_follow(&mut options);
        let file = options.open(directory.join("status.lock"))?;
        set_private_file(&file)?;
        lock_exclusive(&file)?;
        Ok(StatusLock(file))
    }
}

struct StatusLock(File);

#[cfg(unix)]
fn lock_exclusive(file: &File) -> Result<(), JournalError> {
    use std::os::fd::AsRawFd;
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error().into())
    }
}

#[cfg(not(unix))]
fn lock_exclusive(_file: &File) -> Result<(), JournalError> {
    Ok(())
}

#[cfg(unix)]
impl Drop for StatusLock {
    fn drop(&mut self) {
        use std::os::fd::AsRawFd;
        let _ = unsafe { libc::flock(self.0.as_raw_fd(), libc::LOCK_UN) };
    }
}

#[cfg(not(unix))]
impl Drop for StatusLock {
    fn drop(&mut self) {}
}

fn validate_private_files(files: &[(String, Vec<u8>)]) -> Result<(), JournalError> {
    if files.len() > 8 {
        return Err(JournalError::Invalid(
            "a command accepts at most eight private reference files".into(),
        ));
    }
    let mut total = 0_u64;
    let mut names = std::collections::BTreeSet::new();
    for (name, bytes) in files {
        validate_file_name(name)?;
        if !names.insert(name) {
            return Err(JournalError::Invalid(
                "private input filenames must be unique".into(),
            ));
        }
        let length = u64::try_from(bytes.len())
            .map_err(|_| JournalError::Invalid("private input is too large".into()))?;
        if length > MAXIMUM_ATTACHMENT_BYTES {
            return Err(JournalError::Invalid(
                "private input exceeds the per-file limit".into(),
            ));
        }
        total = total
            .checked_add(length)
            .ok_or_else(|| JournalError::Invalid("private input total overflowed".into()))?;
    }
    if total > MAXIMUM_TOTAL_ATTACHMENT_BYTES {
        return Err(JournalError::Invalid(
            "private inputs exceed the command total limit".into(),
        ));
    }
    Ok(())
}

fn validate_file_name(value: &str) -> Result<(), JournalError> {
    if value.is_empty()
        || value.len() > 128
        || matches!(value, "." | "..")
        || value
            .chars()
            .any(|character| character.is_control() || matches!(character, '/' | '\\'))
    {
        return Err(JournalError::Invalid(
            "invalid private input filename".into(),
        ));
    }
    Ok(())
}

fn validate_loaded(
    record: &CommandRecord,
    status: &CommandStatus,
    expected_id: &str,
) -> Result<(), JournalError> {
    if record.schema != COMMAND_SCHEMA
        || status.schema != COMMAND_STATUS_SCHEMA
        || record.command_id != expected_id
        || status.command_id != expected_id
    {
        return Err(JournalError::Invalid(
            "command journal identity or schema mismatch".into(),
        ));
    }
    Ok(())
}

fn validate_identifier(label: &str, value: &str) -> Result<(), JournalError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        || matches!(value, "." | "..")
    {
        return Err(JournalError::Invalid(format!("invalid {label}")));
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

fn require_absolute(path: &Path) -> Result<(), JournalError> {
    if !path.is_absolute() {
        return Err(JournalError::Invalid(
            "command journal root must be absolute".into(),
        ));
    }
    Ok(())
}

fn ensure_private_directory(path: &Path) -> Result<(), JournalError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(JournalError::Invalid(format!(
                "{} is not a real directory",
                path.display()
            )));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir_all(path)?,
        Err(error) => return Err(error.into()),
    }
    set_private_directory(path)?;
    Ok(())
}

fn require_real_directory(path: &Path) -> Result<(), JournalError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(JournalError::Invalid(format!(
            "{} is not a real directory",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(unix)]
fn set_private_directory(path: &Path) -> Result<(), JournalError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_directory(_path: &Path) -> Result<(), JournalError> {
    Ok(())
}

fn write_new_json<T: Serialize>(path: &Path, value: &T) -> Result<(), JournalError> {
    let bytes = tohseno_protocol::canonical::to_vec(value)?;
    write_new_bytes(path, &bytes)
}

fn write_new_bytes(path: &Path, bytes: &[u8]) -> Result<(), JournalError> {
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    set_no_follow(&mut options);
    let mut file = options.open(path)?;
    set_private_file(&file)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn write_replace_json<T: Serialize>(path: &Path, value: &T) -> Result<(), JournalError> {
    let parent = path
        .parent()
        .ok_or_else(|| JournalError::Invalid("journal file has no parent".into()))?;
    require_real_directory(parent)?;
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(JournalError::Invalid(
                "journal status target is unsafe".into(),
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let sequence = TEMPORARY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temporary = parent.join(format!(".status-{}-{sequence}.tmp", std::process::id()));
    write_new_json(&temporary, value)?;
    fs::rename(&temporary, path)?;
    sync_directory(parent)?;
    Ok(())
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, JournalError> {
    let bytes = read_bounded(path, MAXIMUM_JSON_BYTES)?;
    Ok(tohseno_protocol::canonical::from_slice(&bytes)?)
}

fn read_bounded(path: &Path, maximum: u64) -> Result<Vec<u8>, JournalError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > maximum {
        return Err(JournalError::Invalid(format!(
            "{} is not a bounded regular file",
            path.display()
        )));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    set_no_follow(&mut options);
    let file = options.open(path)?;
    let opened = file.metadata()?;
    if !opened.is_file() || opened.len() != metadata.len() {
        return Err(JournalError::Invalid(
            "journal file changed while it was opened".into(),
        ));
    }
    let mut bytes = Vec::with_capacity(usize::try_from(opened.len()).unwrap_or(0));
    file.take(maximum + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > maximum {
        return Err(JournalError::Invalid("journal file is too large".into()));
    }
    Ok(bytes)
}

#[cfg(unix)]
fn set_no_follow(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.custom_flags(libc::O_NOFOLLOW);
}

#[cfg(not(unix))]
fn set_no_follow(_options: &mut OpenOptions) {}

#[cfg(unix)]
fn set_private_file(file: &File) -> Result<(), JournalError> {
    use std::os::unix::fs::PermissionsExt;
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_file(_file: &File) -> Result<(), JournalError> {
    Ok(())
}

fn sync_directory(path: &Path) -> Result<(), JournalError> {
    File::open(path)?.sync_all()?;
    Ok(())
}

fn remove_owned_staging(path: &Path) -> Result<(), JournalError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || !path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(".staging-"))
    {
        return Err(JournalError::Invalid(
            "refusing to remove an unrecognized staging entry".into(),
        ));
    }
    fs::remove_dir_all(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Payload {
        intention: String,
    }

    fn metadata(id: &str) -> AdmissionMetadata {
        AdmissionMetadata {
            command_id: id.into(),
            command_kind: CommandKind::ShotCreate,
            origin: CommandOrigin::Conformance,
            origin_device_id: None,
            workspace_id: "workspace_fixture".into(),
            shot_id: None,
            base_expression_id: None,
            base_version_id: None,
            submitted_at: Some("2026-08-15T00:00:00Z".into()),
        }
    }

    #[test]
    fn admission_is_idempotent_and_conflicting_reuse_fails() {
        let root = tempfile::tempdir().unwrap();
        let journal = CommandJournal::open(root.path().join("service")).unwrap();
        let first = journal
            .admit(
                metadata("command_fixture"),
                &Payload {
                    intention: "one exact intention".into(),
                },
            )
            .unwrap();
        assert!(!first.existing);
        let second = journal
            .admit(
                metadata("command_fixture"),
                &Payload {
                    intention: "one exact intention".into(),
                },
            )
            .unwrap();
        assert!(second.existing);
        assert!(journal
            .admit(
                metadata("command_fixture"),
                &Payload {
                    intention: "different".into(),
                },
            )
            .is_err());
    }

    #[test]
    fn concurrent_identical_admission_has_one_stable_record() {
        let root = tempfile::tempdir().unwrap();
        let journal = CommandJournal::open(root.path().join("service")).unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
        let handles = (0..8)
            .map(|_| {
                let journal = journal.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    journal
                        .admit(
                            metadata("command_concurrent"),
                            &Payload {
                                intention: "one exact intention".into(),
                            },
                        )
                        .unwrap()
                })
            })
            .collect::<Vec<_>>();
        let admissions = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();

        assert_eq!(
            admissions
                .iter()
                .filter(|admission| !admission.existing)
                .count(),
            1
        );
        let (record, status) = journal.load("command_concurrent").unwrap();
        assert_eq!(record.command_id, "command_concurrent");
        assert_eq!(status.state, CommandState::Received);
    }

    #[test]
    fn admission_rejects_noncanonical_external_timestamp() {
        let root = tempfile::tempdir().unwrap();
        let journal = CommandJournal::open(root.path().join("service")).unwrap();
        let mut invalid = metadata("command_bad_time");
        invalid.submitted_at = Some("2026-08-15T00:00:00.000Z".into());
        assert!(journal
            .admit(
                invalid,
                &Payload {
                    intention: "one exact intention".into(),
                },
            )
            .is_err());
        assert!(!journal.root().join("command_bad_time").exists());
    }

    #[test]
    fn transitions_are_monotonic_and_completed_receipt_is_stable() {
        let root = tempfile::tempdir().unwrap();
        let journal = CommandJournal::open(root.path().join("service")).unwrap();
        journal
            .admit(
                metadata("command_transition"),
                &Payload {
                    intention: "x".into(),
                },
            )
            .unwrap();
        journal
            .transition("command_transition", CommandState::Validated, None, None)
            .unwrap();
        journal
            .transition("command_transition", CommandState::Accepted, None, None)
            .unwrap();
        journal
            .transition("command_transition", CommandState::Running, None, None)
            .unwrap();
        let receipt = serde_json::json!({"command_id":"command_transition"});
        journal
            .transition(
                "command_transition",
                CommandState::Completed,
                Some(receipt.clone()),
                None,
            )
            .unwrap();
        let (_, status) = journal.load("command_transition").unwrap();
        assert_eq!(status.result.unwrap().receipt, receipt);
        assert!(journal
            .transition("command_transition", CommandState::Running, None, None)
            .is_err());
        assert!(journal
            .transition(
                "command_transition",
                CommandState::Completed,
                Some(serde_json::json!({"command_id":"different"})),
                None,
            )
            .is_err());
        let (_, unchanged) = journal.load("command_transition").unwrap();
        assert_eq!(unchanged.result.unwrap().receipt, receipt);
    }

    #[test]
    fn admission_rechecks_the_complete_private_input_set() {
        let root = tempfile::tempdir().unwrap();
        let journal = CommandJournal::open(root.path().join("service")).unwrap();
        let payload = Payload {
            intention: "one exact intention".into(),
        };
        journal
            .admit_with_files(
                metadata("command_inputs"),
                &payload,
                &[("one.png".into(), vec![1, 2, 3])],
            )
            .unwrap();

        assert!(journal
            .admit_with_files(metadata("command_inputs"), &payload, &[])
            .is_err());
        fs::write(
            journal.root().join("command_inputs/inputs/unexpected.png"),
            [9],
        )
        .unwrap();
        assert!(journal
            .admit_with_files(
                metadata("command_inputs"),
                &payload,
                &[("one.png".into(), vec![1, 2, 3])],
            )
            .is_err());
    }

    #[test]
    fn running_receipt_survives_waiting_and_resume_transitions() {
        let root = tempfile::tempdir().unwrap();
        let journal = CommandJournal::open(root.path().join("service")).unwrap();
        journal
            .admit(
                metadata("command_waiting"),
                &Payload {
                    intention: "x".into(),
                },
            )
            .unwrap();
        journal
            .transition("command_waiting", CommandState::Validated, None, None)
            .unwrap();
        journal
            .transition("command_waiting", CommandState::Accepted, None, None)
            .unwrap();
        let receipt = serde_json::json!({"execution_id":"execution_fixture"});
        journal
            .transition(
                "command_waiting",
                CommandState::Running,
                Some(receipt.clone()),
                None,
            )
            .unwrap();
        journal
            .transition(
                "command_waiting",
                CommandState::WaitingForDevice,
                None,
                None,
            )
            .unwrap();
        journal
            .transition("command_waiting", CommandState::Running, None, None)
            .unwrap();

        let (_, status) = journal.load("command_waiting").unwrap();
        assert_eq!(status.result.unwrap().receipt, receipt);
    }

    #[cfg(unix)]
    #[test]
    fn journal_rejects_symlinked_command_entries() {
        let root = tempfile::tempdir().unwrap();
        let service = root.path().join("service");
        let journal = CommandJournal::open(&service).unwrap();
        std::os::unix::fs::symlink(root.path(), journal.root().join("command_link")).unwrap();
        assert!(journal.load("command_link").is_err());
    }
}
