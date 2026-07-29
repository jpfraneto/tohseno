use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tohseno_protocol::digest::ShotId;
use tohseno_protocol::identity::BuilderId;

const APP_RECORD_LIMIT: u64 = 1024 * 1024;
const COMPLETE_MARKER: &[u8] = b"complete\n";
const DEFAULT_CANDIDATE_DATA_DIRECTORY: &str = ".tohseno-genesis";

#[derive(Debug)]
pub enum LedgerError {
    Io(std::io::Error),
    TomlDecode(toml::de::Error),
    TomlEncode(toml::ser::Error),
    InvalidName(String),
    AppExists(String),
    AppMissing(String),
    AppBusy(String),
    ShotFinalized(u32),
    Corrupt(String),
}

impl fmt::Display for LedgerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::TomlDecode(error) => write!(f, "{error}"),
            Self::TomlEncode(error) => write!(f, "{error}"),
            Self::InvalidName(name) => write!(f, "invalid app name: {name}"),
            Self::AppExists(name) => write!(f, "app already exists: {name}"),
            Self::AppMissing(name) => write!(f, "app does not exist: {name}"),
            Self::AppBusy(name) => {
                write!(f, "another TOHSENO process is already changing app: {name}")
            }
            Self::ShotFinalized(number) => write!(f, "shot {number} is immutable"),
            Self::Corrupt(message) => write!(f, "ledger is inconsistent: {message}"),
        }
    }
}

impl std::error::Error for LedgerError {}

impl From<std::io::Error> for LedgerError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<toml::de::Error> for LedgerError {
    fn from(value: toml::de::Error) -> Self {
        Self::TomlDecode(value)
    }
}

impl From<toml::ser::Error> for LedgerError {
    fn from(value: toml::ser::Error) -> Self {
        Self::TomlEncode(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AppRecord {
    pub name: String,
    pub bundle_id: String,
    pub created_at_unix: u64,
    pub latest_shot: Option<u32>,
    /// Absent only for ledgers created before the Genesis protocol candidate.
    #[serde(default)]
    pub shot_id: Option<ShotId>,
    /// Absent only for ledgers created before the Genesis protocol candidate.
    #[serde(default)]
    pub builder_id: Option<BuilderId>,
    #[serde(default)]
    pub retired: bool,
    #[serde(default)]
    pub parents: BTreeMap<String, u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Shot {
    pub app_name: String,
    pub number: u32,
    pub path: PathBuf,
}

impl Shot {
    pub fn prompt_path(&self) -> PathBuf {
        self.path.join("prompt.md")
    }

    pub fn images_path(&self) -> PathBuf {
        self.path.join("images")
    }

    pub fn source_path(&self) -> PathBuf {
        self.path.join("src")
    }

    pub fn artifact_path(&self) -> PathBuf {
        self.path.join("artifact")
    }

    pub fn build_log_path(&self) -> PathBuf {
        self.path.join("build.log")
    }

    pub fn harness_log_path(&self) -> PathBuf {
        self.path.join("harness.log")
    }

    fn complete_path(&self) -> PathBuf {
        self.path.join(".complete")
    }
}

#[derive(Clone, Debug)]
pub struct Ledger {
    root: PathBuf,
}

/// A process-scoped lease for one app's mutable lifecycle.
///
/// Engine entry points keep this value alive from their first ledger read
/// through finalization so a second process cannot archive or overwrite an
/// in-progress Shot.
pub struct AppLock {
    _file: File,
}

impl Ledger {
    pub fn discover() -> Result<Self, LedgerError> {
        if let Some(configured) = std::env::var_os("TOHSENO_DATA_ROOT") {
            let root = PathBuf::from(configured);
            if !root.is_absolute() {
                return Err(LedgerError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "TOHSENO_DATA_ROOT must be an absolute path",
                )));
            }
            return Ok(Self::at(root));
        }
        let home = std::env::var_os("HOME").ok_or_else(|| {
            LedgerError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "HOME is not set",
            ))
        })?;
        // This binary is the isolated Genesis candidate. The already-shipped
        // v0.6 binary retains its own `~/.tohseno` default; candidate code
        // must never discover or mutate that stable ledger implicitly.
        Ok(Self::at(default_candidate_data_root(Path::new(&home))))
    }

    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn initialize(&self) -> Result<(), LedgerError> {
        if self.root.exists() {
            require_real_directory(&self.root)?;
        } else {
            fs::create_dir_all(&self.root)?;
            require_real_directory(&self.root)?;
        }
        ensure_real_directory(&self.root.join("apps"))?;
        ensure_real_directory(&self.root.join("locks"))?;
        Ok(())
    }

    /// Acquires the non-blocking, cross-process lease for one app.
    pub fn lock_app(&self, app_name: &str) -> Result<AppLock, LedgerError> {
        validate_app_name(app_name)?;
        self.lock_named(app_name, app_name)
    }

    /// Serializes public actions that share BuilderAccount and registry nonces
    /// across otherwise independent apps.
    pub fn lock_public_actions(&self) -> Result<AppLock, LedgerError> {
        self.lock_named(".public-builder-account", "public BuilderAccount")
    }

    fn lock_named(&self, name: &str, busy_label: &str) -> Result<AppLock, LedgerError> {
        self.initialize()?;
        let locks = self.root.join("locks");
        require_real_directory(&locks)?;
        let path = locks.join(format!("{name}.lock"));
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                return Err(unsafe_path_error());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let mut options = OpenOptions::new();
        options.create(true).read(true).write(true);
        set_no_follow(&mut options);
        let file = options.open(path)?;
        match try_exclusive_lock(&file) {
            Ok(()) => Ok(AppLock { _file: file }),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                Err(LedgerError::AppBusy(busy_label.into()))
            }
            Err(error) => Err(error.into()),
        }
    }

    pub fn create_app(&self, name: &str, bundle_id: &str) -> Result<AppRecord, LedgerError> {
        validate_app_name(name)?;
        self.initialize()?;
        let app_dir = self.app_dir(name);
        match fs::symlink_metadata(&app_dir) {
            Ok(_) => return Err(LedgerError::AppExists(name.into())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        fs::create_dir(&app_dir)?;
        fs::create_dir(app_dir.join("shots"))?;
        let record = AppRecord {
            name: name.into(),
            bundle_id: bundle_id.into(),
            created_at_unix: now_unix(),
            latest_shot: None,
            shot_id: None,
            builder_id: None,
            retired: false,
            parents: BTreeMap::new(),
        };
        self.write_record(&record)?;
        Ok(record)
    }

    pub fn load_app(&self, name: &str) -> Result<AppRecord, LedgerError> {
        validate_app_name(name)?;
        require_real_directory(&self.root)?;
        require_real_directory(&self.root.join("apps"))?;
        for directory in [self.app_dir(name), self.app_dir(name).join("shots")] {
            match fs::symlink_metadata(&directory) {
                Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                    return Err(unsafe_path_error());
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Err(LedgerError::AppMissing(name.into()));
                }
                Err(error) => return Err(error.into()),
            }
        }
        let path = self.app_dir(name).join("app.toml");
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(LedgerError::AppMissing(name.into()));
            }
            Err(error) => return Err(error.into()),
        };
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() > APP_RECORD_LIMIT
        {
            return Err(LedgerError::Corrupt(format!(
                "{name}/app.toml is not a bounded regular file"
            )));
        }
        let encoded = read_unchanged_regular_file(&path, APP_RECORD_LIMIT, &metadata)?;
        let mut record =
            toml::from_str::<AppRecord>(std::str::from_utf8(&encoded).map_err(|error| {
                LedgerError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, error))
            })?)?;
        validate_app_record(&record, name)?;
        self.recover_interrupted_finalization(&mut record)?;
        Ok(record)
    }

    pub fn list_apps(&self) -> Result<Vec<AppRecord>, LedgerError> {
        self.initialize()?;
        let mut records = Vec::new();
        for entry in fs::read_dir(self.root.join("apps"))? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                if let Ok(record) = self.load_app(&entry.file_name().to_string_lossy()) {
                    records.push(record);
                }
            }
        }
        records.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(records)
    }

    /// Reserves the next integer shot. A reserved shot is writable until
    /// `finalize_shot`; finalized shots are rejected by all ledger write APIs.
    pub fn reserve_shot(&self, app_name: &str, parent: Option<u32>) -> Result<Shot, LedgerError> {
        let mut record = self.load_app(app_name)?;
        let shots_dir = self.app_dir(app_name).join("shots");
        let number = next_sequence(&record)?;
        let path = shots_dir.join(format!("{number:04}"));
        if path.exists() {
            self.archive_incomplete_shot(app_name, number, &path)?;
        }
        fs::create_dir(&path)?;
        for child in ["images", "src", "artifact"] {
            fs::create_dir(path.join(child))?;
        }
        if let Some(parent_number) = parent {
            record.parents.insert(number.to_string(), parent_number);
            self.write_record(&record)?;
        }
        Ok(Shot {
            app_name: app_name.into(),
            number,
            path,
        })
    }

    /// Preserves a failed or interrupted attempt without allowing it to consume
    /// the next protocol sequence number.
    fn archive_incomplete_shot(
        &self,
        app_name: &str,
        number: u32,
        path: &Path,
    ) -> Result<(), LedgerError> {
        let archive_root = self.app_dir(app_name).join("incomplete");
        fs::create_dir_all(&archive_root)?;
        let timestamp = now_unix();
        for ordinal in 1_u32.. {
            let destination =
                archive_root.join(format!("{number:04}-attempt-{timestamp}-{ordinal:04}"));
            if !destination.exists() {
                fs::rename(path, destination)?;
                return Ok(());
            }
        }
        unreachable!()
    }

    pub fn write_shot_file(
        &self,
        shot: &Shot,
        relative_path: impl AsRef<Path>,
        contents: &[u8],
    ) -> Result<(), LedgerError> {
        self.assert_writable(shot)?;
        let path = prepare_shot_path(shot, relative_path.as_ref())?;
        let mut options = OpenOptions::new();
        options.create(true).truncate(true).write(true);
        set_no_follow(&mut options);
        let mut file = options.open(path)?;
        file.write_all(contents)?;
        Ok(())
    }

    pub fn append_shot_log(
        &self,
        shot: &Shot,
        relative_path: &str,
        contents: &[u8],
    ) -> Result<(), LedgerError> {
        self.assert_writable(shot)?;
        let path = prepare_shot_path(shot, Path::new(relative_path))?;
        let mut options = OpenOptions::new();
        options.create(true).append(true);
        set_no_follow(&mut options);
        let mut file = options.open(path)?;
        file.write_all(contents)?;
        Ok(())
    }

    pub fn finalize_shot(&self, shot: &Shot) -> Result<(), LedgerError> {
        self.assert_writable(shot)?;
        let mut record = self.load_app(&shot.app_name)?;
        if next_sequence(&record)? != shot.number {
            return Err(LedgerError::Corrupt(format!(
                "shot {} is not the next append-only sequence",
                shot.number
            )));
        }
        self.publish_completion_marker(shot)?;
        record.latest_shot = Some(shot.number);
        self.write_record(&record)?;
        Ok(())
    }

    pub fn latest_shot(&self, app_name: &str) -> Result<Option<Shot>, LedgerError> {
        let record = self.load_app(app_name)?;
        Ok(record.latest_shot.map(|number| Shot {
            app_name: app_name.into(),
            number,
            path: self
                .app_dir(app_name)
                .join("shots")
                .join(format!("{number:04}")),
        }))
    }

    pub fn set_retired(&self, app_name: &str, retired: bool) -> Result<(), LedgerError> {
        let mut record = self.load_app(app_name)?;
        record.retired = retired;
        self.write_record(&record)
    }

    /// Binds a legacy-empty app record to its permanent protocol identity.
    ///
    /// Repeating the same binding is idempotent. A conflicting binding is
    /// rejected so a retry can never silently replace ShotID or BuilderID.
    pub fn bind_protocol_identity(
        &self,
        app_name: &str,
        shot_id: ShotId,
        builder_id: BuilderId,
    ) -> Result<AppRecord, LedgerError> {
        let mut record = self.load_app(app_name)?;
        match (record.shot_id, record.builder_id) {
            (None, None) => {
                record.shot_id = Some(shot_id);
                record.builder_id = Some(builder_id);
                self.write_record(&record)?;
                Ok(record)
            }
            (Some(existing_shot), Some(existing_builder))
                if existing_shot == shot_id && existing_builder == builder_id =>
            {
                Ok(record)
            }
            _ => Err(LedgerError::Corrupt(format!(
                "{app_name} has conflicting or partial protocol identity"
            ))),
        }
    }

    pub fn shot(&self, app_name: &str, number: u32) -> Result<Shot, LedgerError> {
        validate_app_name(app_name)?;
        if number == 0 {
            return Err(LedgerError::Corrupt(
                "shot sequence must be at least 1".into(),
            ));
        }
        let path = self
            .app_dir(app_name)
            .join("shots")
            .join(format!("{number:04}"));
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(LedgerError::Corrupt(format!(
                    "shot {number} is not a real directory"
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(LedgerError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("shot {number} does not exist"),
                )));
            }
            Err(error) => return Err(error.into()),
        }
        Ok(Shot {
            app_name: app_name.into(),
            number,
            path,
        })
    }

    pub fn list_shots(&self, app_name: &str) -> Result<Vec<Shot>, LedgerError> {
        let record = self.load_app(app_name)?;
        let shots_directory = self.app_dir(app_name).join("shots");
        let mut shots = Vec::new();
        for entry in fs::read_dir(shots_directory)? {
            let entry = entry?;
            let Ok(number) = entry.file_name().to_string_lossy().parse::<u32>() else {
                continue;
            };
            if !entry.file_type()?.is_dir() {
                return Err(LedgerError::Corrupt(format!(
                    "shot {number} is not a real directory"
                )));
            }
            let shot = Shot {
                app_name: app_name.into(),
                number,
                path: entry.path(),
            };
            if has_valid_completion_marker(&shot.path, number)? {
                if record.latest_shot.is_none_or(|latest| number > latest) {
                    return Err(LedgerError::Corrupt(format!(
                        "shot {number} is complete but app.toml does not recognize it"
                    )));
                }
                shots.push(shot);
            }
        }
        shots.sort_by_key(|shot| shot.number);
        Ok(shots)
    }

    fn assert_writable(&self, shot: &Shot) -> Result<(), LedgerError> {
        validate_app_name(&shot.app_name)?;
        if shot.number == 0 {
            return Err(LedgerError::Corrupt(
                "shot sequence must be at least 1".into(),
            ));
        }
        let shots = self.app_dir(&shot.app_name).join("shots");
        require_real_directory(&shots)?;
        let expected = shots.join(format!("{:04}", shot.number));
        if shot.path != expected {
            return Err(LedgerError::Corrupt(format!(
                "shot {} has a path outside its ledger sequence",
                shot.number
            )));
        }
        require_real_shot_directory(&shot.path, shot.number)?;
        match fs::symlink_metadata(shot.complete_path()) {
            Ok(_) => Err(LedgerError::ShotFinalized(shot.number)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    fn app_dir(&self, name: &str) -> PathBuf {
        self.root.join("apps").join(name)
    }

    /// `.complete` is the immutable commit point. If a process stops after
    /// durably creating it but before replacing app.toml, replay only that
    /// single next sequence into the derived app head.
    fn recover_interrupted_finalization(&self, record: &mut AppRecord) -> Result<(), LedgerError> {
        if let Some(number) = record.latest_shot {
            let current = self
                .app_dir(&record.name)
                .join("shots")
                .join(format!("{number:04}"));
            require_real_shot_directory(&current, number)?;
            if !has_valid_completion_marker(&current, number)? {
                return Err(LedgerError::Corrupt(format!(
                    "app.toml recognizes shot {number}, but its completion marker is missing"
                )));
            }
        }

        let Some(number) = record.latest_shot.unwrap_or(0).checked_add(1) else {
            return Ok(());
        };
        let path = self
            .app_dir(&record.name)
            .join("shots")
            .join(format!("{number:04}"));
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(LedgerError::Corrupt(format!(
                    "next shot {number} is not a real directory"
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        }
        if !has_valid_completion_marker(&path, number)? {
            return Ok(());
        }

        record.latest_shot = Some(number);
        self.write_record(record)
    }

    fn publish_completion_marker(&self, shot: &Shot) -> Result<(), LedgerError> {
        let marker = shot.complete_path();
        let prefix = format!(".complete.tmp-{}", std::process::id());
        for ordinal in 1_u32.. {
            let temporary = shot.path.join(format!("{prefix}-{ordinal}"));
            let mut options = OpenOptions::new();
            options.create_new(true).write(true);
            set_no_follow(&mut options);
            match options.open(&temporary) {
                Ok(mut file) => {
                    let result = (|| {
                        file.write_all(COMPLETE_MARKER)?;
                        file.sync_all()?;
                        drop(file);
                        match fs::symlink_metadata(&marker) {
                            Ok(_) => return Err(LedgerError::ShotFinalized(shot.number)),
                            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                            Err(error) => return Err(error.into()),
                        }
                        fs::rename(&temporary, &marker)?;
                        sync_directory(&shot.path)
                    })();
                    if result.is_err() {
                        let _ = fs::remove_file(&temporary);
                    }
                    return result;
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
        unreachable!()
    }

    fn write_record(&self, record: &AppRecord) -> Result<(), LedgerError> {
        validate_app_record(record, &record.name)?;
        let directory = self.app_dir(&record.name);
        require_real_directory(&directory)?;
        let path = directory.join("app.toml");
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                return Err(LedgerError::Corrupt(format!(
                    "{}/app.toml is unsafe",
                    record.name
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let encoded = toml::to_string_pretty(record)?;
        let file_name = format!(".app.toml.tmp-{}", std::process::id());
        for ordinal in 1_u32.. {
            let temporary = directory.join(format!("{file_name}-{ordinal}"));
            let mut options = OpenOptions::new();
            options.create_new(true).write(true);
            set_no_follow(&mut options);
            match options.open(&temporary) {
                Ok(mut file) => {
                    file.write_all(encoded.as_bytes())?;
                    file.sync_all()?;
                    if let Err(error) = fs::rename(&temporary, &path) {
                        let _ = fs::remove_file(&temporary);
                        return Err(error.into());
                    }
                    sync_directory(&directory)?;
                    return Ok(());
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
        unreachable!()
    }
}

fn next_sequence(record: &AppRecord) -> Result<u32, LedgerError> {
    record
        .latest_shot
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| LedgerError::Corrupt(format!("{} exhausted the shot sequence", record.name)))
}

fn default_candidate_data_root(home: &Path) -> PathBuf {
    home.join(DEFAULT_CANDIDATE_DATA_DIRECTORY)
}

fn require_real_shot_directory(path: &Path, number: u32) -> Result<(), LedgerError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            LedgerError::Corrupt(format!("shot {number} directory is missing"))
        } else {
            error.into()
        }
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(LedgerError::Corrupt(format!(
            "shot {number} is not a real directory"
        )));
    }
    Ok(())
}

fn ensure_real_directory(path: &Path) -> Result<(), LedgerError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(unsafe_path_error());
        }
        Ok(_) => return Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    match fs::create_dir(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    require_real_directory(path)
}

#[cfg(unix)]
fn try_exclusive_lock(file: &File) -> std::io::Result<()> {
    use std::os::fd::AsRawFd;

    // SAFETY: `file` owns a live descriptor for the duration of this call.
    // `flock` does not dereference user memory, and the lock is released when
    // the owned descriptor in `AppLock` closes.
    let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(not(unix))]
fn try_exclusive_lock(_file: &File) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "TOHSENO app lifecycle locking requires macOS or Unix",
    ))
}

fn has_valid_completion_marker(path: &Path, number: u32) -> Result<bool, LedgerError> {
    let marker = path.join(".complete");
    let metadata = match fs::symlink_metadata(&marker) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() != COMPLETE_MARKER.len() as u64
    {
        return Err(LedgerError::Corrupt(format!(
            "shot {number} has an invalid completion marker"
        )));
    }
    let bytes = read_unchanged_regular_file(&marker, COMPLETE_MARKER.len() as u64, &metadata)?;
    if bytes != COMPLETE_MARKER {
        return Err(LedgerError::Corrupt(format!(
            "shot {number} has an invalid completion marker"
        )));
    }
    Ok(true)
}

fn read_unchanged_regular_file(
    path: &Path,
    limit: u64,
    initial: &Metadata,
) -> Result<Vec<u8>, LedgerError> {
    if initial.file_type().is_symlink() || !initial.is_file() || initial.len() > limit {
        return Err(LedgerError::Corrupt(format!(
            "{} is not a bounded regular file",
            path.display()
        )));
    }

    let mut options = OpenOptions::new();
    options.read(true);
    set_no_follow(&mut options);
    let file = options.open(path)?;
    let opened = file.metadata()?;
    if !opened.is_file() || !same_file_version(initial, &opened) {
        return Err(LedgerError::Corrupt(format!(
            "{} changed while it was opened",
            path.display()
        )));
    }

    let mut bytes = Vec::with_capacity(usize::try_from(opened.len()).unwrap_or(0));
    file.take(limit + 1).read_to_end(&mut bytes)?;
    let final_metadata = fs::symlink_metadata(path)?;
    if final_metadata.file_type().is_symlink()
        || !final_metadata.is_file()
        || !same_file_version(&opened, &final_metadata)
        || bytes.len() as u64 != opened.len()
    {
        return Err(LedgerError::Corrupt(format!(
            "{} changed while it was read",
            path.display()
        )));
    }
    Ok(bytes)
}

#[cfg(unix)]
fn same_file_version(left: &Metadata, right: &Metadata) -> bool {
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
fn same_file_version(left: &Metadata, right: &Metadata) -> bool {
    left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
        && left.file_type() == right.file_type()
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), LedgerError> {
    let mut options = OpenOptions::new();
    options.read(true);
    set_no_follow(&mut options);
    let directory = options.open(path)?;
    if !directory.metadata()?.is_dir() {
        return Err(unsafe_path_error());
    }
    directory.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), LedgerError> {
    Ok(())
}

fn prepare_shot_path(shot: &Shot, relative_path: &Path) -> Result<PathBuf, LedgerError> {
    let components = relative_path
        .components()
        .map(|component| match component {
            std::path::Component::Normal(value) => Ok(value),
            _ => Err(unsafe_path_error()),
        })
        .collect::<Result<Vec<_>, _>>()?;
    if components.is_empty() {
        return Err(unsafe_path_error());
    }

    require_real_directory(&shot.path)?;
    let mut directory = shot.path.clone();
    for component in &components[..components.len() - 1] {
        directory.push(component);
        match fs::symlink_metadata(&directory) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(unsafe_path_error());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&directory)?;
                require_real_directory(&directory)?;
            }
            Err(error) => return Err(error.into()),
        }
    }

    Ok(directory.join(components[components.len() - 1]))
}

fn require_real_directory(path: &Path) -> Result<(), LedgerError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(unsafe_path_error());
    }
    Ok(())
}

fn unsafe_path_error() -> LedgerError {
    LedgerError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        "shot path must stay inside its directory without traversing symlinks",
    ))
}

fn validate_app_record(record: &AppRecord, expected_name: &str) -> Result<(), LedgerError> {
    validate_app_name(expected_name)?;
    validate_app_name(&record.name)?;
    if record.name != expected_name {
        return Err(LedgerError::Corrupt(format!(
            "app.toml names {} inside the {expected_name} directory",
            record.name
        )));
    }
    if record.bundle_id.is_empty()
        || record.bundle_id.len() > 255
        || record.bundle_id.chars().any(char::is_control)
    {
        return Err(LedgerError::Corrupt(format!(
            "{expected_name} has an invalid bundle identifier"
        )));
    }
    if record.created_at_unix == 0 || record.latest_shot == Some(0) {
        return Err(LedgerError::Corrupt(format!(
            "{expected_name} has invalid ledger chronology"
        )));
    }
    match (record.shot_id, record.builder_id) {
        (None, None) => {}
        (Some(shot_id), Some(builder_id))
            if !shot_id.is_zero() && builder_id.validate().is_ok() => {}
        _ => {
            return Err(LedgerError::Corrupt(format!(
                "{expected_name} has a partial or invalid protocol identity"
            )));
        }
    }
    for (child, parent) in &record.parents {
        let child = child.parse::<u32>().map_err(|_| {
            LedgerError::Corrupt(format!("{expected_name} has a non-integer parent key"))
        })?;
        if child == 0 || *parent == 0 || *parent >= child {
            return Err(LedgerError::Corrupt(format!(
                "{expected_name} has invalid parent linkage {parent} -> {child}"
            )));
        }
    }
    Ok(())
}

#[cfg(unix)]
fn set_no_follow(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.custom_flags(libc::O_NOFOLLOW).mode(0o600);
}

#[cfg(not(unix))]
fn set_no_follow(_options: &mut OpenOptions) {}

pub fn sanitize_component(value: &str) -> String {
    let mut sanitized = String::new();
    let mut last_was_dash = false;
    for character in value.chars().flat_map(char::to_lowercase) {
        let accepted = character.is_ascii_alphanumeric();
        if accepted {
            sanitized.push(character);
            last_was_dash = false;
        } else if !last_was_dash && !sanitized.is_empty() {
            sanitized.push('-');
            last_was_dash = true;
        }
    }
    sanitized.trim_matches('-').to_string()
}

pub fn validate_app_name(name: &str) -> Result<(), LedgerError> {
    if name.is_empty()
        || name.len() > 63
        || sanitize_component(name) != name
        || !name
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_alphanumeric())
    {
        return Err(LedgerError::InvalidName(name.into()));
    }
    Ok(())
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shots_are_integer_append_only_worlds() {
        let temporary = tempfile::tempdir().unwrap();
        let ledger = Ledger::at(temporary.path());
        ledger
            .create_app("paper-press", "com.tohseno.test.paper-press")
            .unwrap();
        let first = ledger.reserve_shot("paper-press", None).unwrap();
        assert_eq!(first.number, 1);
        ledger
            .write_shot_file(&first, "prompt.md", b"Make a press.")
            .unwrap();
        ledger.finalize_shot(&first).unwrap();
        assert!(matches!(
            ledger.write_shot_file(&first, "prompt.md", b"mutation"),
            Err(LedgerError::ShotFinalized(1))
        ));

        let second = ledger.reserve_shot("paper-press", Some(1)).unwrap();
        assert_eq!(second.number, 2);
        assert_eq!(
            ledger.load_app("paper-press").unwrap().parents.get("2"),
            Some(&1)
        );
        assert_eq!(
            ledger
                .list_shots("paper-press")
                .unwrap()
                .iter()
                .map(|shot| shot.number)
                .collect::<Vec<_>>(),
            [1]
        );
    }

    #[test]
    fn loading_an_app_that_was_never_created_reports_app_missing() {
        let temporary = tempfile::tempdir().unwrap();
        let ledger = Ledger::at(temporary.path());
        ledger.initialize().unwrap();
        assert!(matches!(
            ledger.load_app("brand-new"),
            Err(LedgerError::AppMissing(name)) if name == "brand-new"
        ));
    }

    #[test]
    fn names_are_filesystem_safe_and_stable() {
        assert_eq!(sanitize_component("My Great_App!"), "my-great-app");
        assert!(validate_app_name("replyguy-trencher").is_ok());
        assert!(validate_app_name("../replyguy").is_err());
    }

    #[test]
    fn candidate_default_cannot_alias_the_stable_data_root() {
        let home = Path::new("/Users/example");
        assert_eq!(
            default_candidate_data_root(home),
            Path::new("/Users/example/.tohseno-genesis")
        );
        assert_ne!(
            default_candidate_data_root(home),
            Path::new("/Users/example/.tohseno")
        );
    }

    #[cfg(unix)]
    #[test]
    fn app_lifecycle_lock_is_exclusive_until_its_owner_drops() {
        let temporary = tempfile::tempdir().unwrap();
        let first_ledger = Ledger::at(temporary.path());
        let second_ledger = Ledger::at(temporary.path());

        let first = first_ledger.lock_app("press").unwrap();
        assert!(matches!(
            second_ledger.lock_app("press"),
            Err(LedgerError::AppBusy(name)) if name == "press"
        ));
        assert!(second_ledger.lock_app("other-app").is_ok());

        drop(first);
        assert!(second_ledger.lock_app("press").is_ok());

        let public = first_ledger.lock_public_actions().unwrap();
        assert!(matches!(
            second_ledger.lock_public_actions(),
            Err(LedgerError::AppBusy(name)) if name == "public BuilderAccount"
        ));
        assert!(second_ledger.lock_app("public-builder-account").is_ok());
        drop(public);
        assert!(second_ledger.lock_public_actions().is_ok());
    }

    #[test]
    fn failed_attempts_do_not_skip_protocol_sequence_numbers() {
        let temporary = tempfile::tempdir().unwrap();
        let ledger = Ledger::at(temporary.path());
        ledger
            .create_app("press", "com.tohseno.test.press")
            .unwrap();

        let failed = ledger.reserve_shot("press", None).unwrap();
        ledger
            .write_shot_file(&failed, "build.log", b"failed")
            .unwrap();

        let retry = ledger.reserve_shot("press", None).unwrap();
        assert_eq!(retry.number, 1);
        assert!(!retry.path.join("build.log").exists());
        let archived = temporary.path().join("apps/press/incomplete");
        let attempts = fs::read_dir(archived).unwrap().collect::<Vec<_>>();
        assert_eq!(attempts.len(), 1);
        assert_eq!(
            fs::read(attempts[0].as_ref().unwrap().path().join("build.log")).unwrap(),
            b"failed"
        );
    }

    #[test]
    fn protocol_identity_can_only_be_bound_once() {
        use tohseno_protocol::digest::Address20;

        let temporary = tempfile::tempdir().unwrap();
        let ledger = Ledger::at(temporary.path());
        ledger
            .create_app("press", "com.tohseno.test.press")
            .unwrap();
        let shot_id = ShotId::from_bytes([1; 32]);
        let builder_id = BuilderId::new(Address20::from_bytes([2; 20]));

        ledger
            .bind_protocol_identity("press", shot_id, builder_id)
            .unwrap();
        ledger
            .bind_protocol_identity("press", shot_id, builder_id)
            .unwrap();
        assert!(ledger
            .bind_protocol_identity("press", ShotId::from_bytes([3; 32]), builder_id,)
            .is_err());
    }

    #[cfg(unix)]
    #[test]
    fn shot_writes_refuse_symlink_traversal() {
        use std::os::unix::fs::symlink;

        let temporary = tempfile::tempdir().unwrap();
        let ledger = Ledger::at(temporary.path().join("ledger"));
        ledger
            .create_app("press", "com.tohseno.test.press")
            .unwrap();
        let shot = ledger.reserve_shot("press", None).unwrap();

        let outside = temporary.path().join("outside");
        fs::create_dir(&outside).unwrap();
        symlink(&outside, shot.path.join("escape")).unwrap();
        assert!(ledger
            .write_shot_file(&shot, "escape/owned", b"escaped")
            .is_err());
        assert!(!outside.join("owned").exists());

        let outside_log = outside.join("log");
        fs::write(&outside_log, b"untouched").unwrap();
        symlink(&outside_log, shot.path.join("build.log")).unwrap();
        assert!(ledger
            .append_shot_log(&shot, "build.log", b"escaped")
            .is_err());
        assert_eq!(fs::read(outside_log).unwrap(), b"untouched");
    }

    #[test]
    fn corrupted_app_name_is_rejected_before_completion_marker() {
        let temporary = tempfile::tempdir().unwrap();
        let ledger = Ledger::at(temporary.path().join("ledger"));
        ledger
            .create_app("press", "com.tohseno.test.press")
            .unwrap();
        let shot = ledger.reserve_shot("press", None).unwrap();
        let record_path = temporary.path().join("ledger/apps/press/app.toml");
        let corrupted = fs::read_to_string(&record_path)
            .unwrap()
            .replace("name = \"press\"", "name = \"../outside\"");
        fs::write(record_path, corrupted).unwrap();

        assert!(ledger.finalize_shot(&shot).is_err());
        assert!(!shot.complete_path().exists());
        assert!(!temporary.path().join("ledger/apps/outside").exists());
    }

    #[test]
    fn interrupted_finalize_recovers_app_head_from_exact_marker() {
        let temporary = tempfile::tempdir().unwrap();
        let ledger = Ledger::at(temporary.path().join("ledger"));
        ledger
            .create_app("press", "com.tohseno.test.press")
            .unwrap();
        let shot = ledger.reserve_shot("press", None).unwrap();

        // Simulate a stop after the marker became durable but before app.toml
        // was atomically replaced.
        fs::write(shot.complete_path(), COMPLETE_MARKER).unwrap();
        let record_path = temporary.path().join("ledger/apps/press/app.toml");
        let before =
            toml::from_str::<AppRecord>(&fs::read_to_string(&record_path).unwrap()).unwrap();
        assert_eq!(before.latest_shot, None);

        let recovered = ledger.load_app("press").unwrap();
        assert_eq!(recovered.latest_shot, Some(1));
        let persisted =
            toml::from_str::<AppRecord>(&fs::read_to_string(&record_path).unwrap()).unwrap();
        assert_eq!(persisted.latest_shot, Some(1));
        assert_eq!(ledger.reserve_shot("press", Some(1)).unwrap().number, 2);
    }

    #[test]
    fn interrupted_finalize_refuses_a_tampered_marker() {
        let temporary = tempfile::tempdir().unwrap();
        let ledger = Ledger::at(temporary.path().join("ledger"));
        ledger
            .create_app("press", "com.tohseno.test.press")
            .unwrap();
        let shot = ledger.reserve_shot("press", None).unwrap();
        fs::write(shot.complete_path(), b"completE\n").unwrap();
        let record_path = temporary.path().join("ledger/apps/press/app.toml");
        let unchanged = fs::read(&record_path).unwrap();

        assert!(matches!(
            ledger.load_app("press"),
            Err(LedgerError::Corrupt(_))
        ));
        assert_eq!(fs::read(record_path).unwrap(), unchanged);
    }

    #[test]
    fn app_head_without_a_completion_marker_fails_closed() {
        let temporary = tempfile::tempdir().unwrap();
        let ledger = Ledger::at(temporary.path().join("ledger"));
        ledger
            .create_app("press", "com.tohseno.test.press")
            .unwrap();
        ledger.reserve_shot("press", None).unwrap();
        let mut record = ledger.load_app("press").unwrap();
        record.latest_shot = Some(1);
        ledger.write_record(&record).unwrap();

        assert!(matches!(
            ledger.load_app("press"),
            Err(LedgerError::Corrupt(_))
        ));
    }

    #[test]
    fn finalize_refuses_a_forged_shot_path() {
        let temporary = tempfile::tempdir().unwrap();
        let ledger = Ledger::at(temporary.path().join("ledger"));
        ledger
            .create_app("press", "com.tohseno.test.press")
            .unwrap();
        let outside = temporary.path().join("outside");
        fs::create_dir(&outside).unwrap();
        let forged = Shot {
            app_name: "press".into(),
            number: 1,
            path: outside.clone(),
        };

        assert!(matches!(
            ledger.finalize_shot(&forged),
            Err(LedgerError::Corrupt(_))
        ));
        assert!(!outside.join(".complete").exists());
    }

    #[cfg(unix)]
    #[test]
    fn interrupted_finalize_refuses_a_symlinked_marker() {
        use std::os::unix::fs::symlink;

        let temporary = tempfile::tempdir().unwrap();
        let ledger = Ledger::at(temporary.path().join("ledger"));
        ledger
            .create_app("press", "com.tohseno.test.press")
            .unwrap();
        let shot = ledger.reserve_shot("press", None).unwrap();
        let outside = temporary.path().join("outside-marker");
        fs::write(&outside, COMPLETE_MARKER).unwrap();
        symlink(&outside, shot.complete_path()).unwrap();

        assert!(matches!(
            ledger.load_app("press"),
            Err(LedgerError::Corrupt(_))
        ));
        assert_eq!(fs::read(outside).unwrap(), COMPLETE_MARKER);
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_app_record_is_never_followed() {
        use std::os::unix::fs::symlink;

        let temporary = tempfile::tempdir().unwrap();
        let ledger = Ledger::at(temporary.path().join("ledger"));
        ledger
            .create_app("press", "com.tohseno.test.press")
            .unwrap();
        let record_path = temporary.path().join("ledger/apps/press/app.toml");
        let outside = temporary.path().join("outside.toml");
        fs::write(&outside, fs::read(&record_path).unwrap()).unwrap();
        fs::remove_file(&record_path).unwrap();
        symlink(&outside, &record_path).unwrap();

        assert!(matches!(
            ledger.load_app("press"),
            Err(LedgerError::Corrupt(_))
        ));
    }
}
