use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tohseno_protocol::digest::{ExpressionId, ShotId};
use tohseno_protocol::identity::BuilderId;
use tohseno_protocol::lineage::AdaptedV1Lineage;
use tohseno_protocol::record::ShotRecord;
use tohseno_protocol::signature::SignatureSidecar;

const APP_RECORD_LIMIT: u64 = 1024 * 1024;
const COMPLETE_MARKER: &[u8] = b"complete\n";
const DEFAULT_MACHINE_DATA_DIRECTORY: &str = ".tohseno";
const DEFAULT_FAMILY_DIRECTORY: &str = "Desktop/Tohseno";
/// The in-folder ledger every Shot carries (ADR 0003).
const APP_LEDGER_DIRECTORY: &str = ".tohseno";
/// Names an app can never take when apps share a directory with machine
/// state (`TOHSENO_DATA_ROOT` mode) or with ledger internals.
const RESERVED_APP_NAMES: &[&str] = &["identity", "locks", "walls", "apps", "config", "incomplete"];

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
    /// The current human-facing folder name. This may change without
    /// changing Shot identity, bundle identity, or the Xcode target name.
    pub name: String,
    /// The Xcode scheme/product name chosen when the first expression was
    /// created. Older app.toml files omit it, in which case `name` is the
    /// compatible target name until the folder is renamed.
    #[serde(default)]
    pub target_name: Option<String>,
    pub bundle_id: String,
    pub created_at_unix: u64,
    /// Accepts the pre-rename key so no existing ledger's head is zeroed.
    #[serde(alias = "latest_shot")]
    pub latest_evolution: Option<u32>,
    /// Absent only for ledgers created before the Genesis protocol candidate.
    #[serde(default)]
    pub shot_id: Option<ShotId>,
    /// Absent only for ledgers created before the Genesis protocol candidate.
    #[serde(default)]
    pub builder_id: Option<BuilderId>,
    /// Stable identity of the native Apple expression represented by this
    /// working folder. Absent only until an older ledger is migrated.
    #[serde(default)]
    pub expression_id: Option<ExpressionId>,
    #[serde(default)]
    pub retired: bool,
    #[serde(default)]
    pub parents: BTreeMap<String, u32>,
}

impl AppRecord {
    pub fn target_name(&self) -> &str {
        self.target_name.as_deref().unwrap_or(&self.name)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Evolution {
    pub app_name: String,
    pub number: u32,
    pub path: PathBuf,
}

impl Evolution {
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
    /// The family home: every child is one app's visible working folder.
    root: PathBuf,
    /// Machine-scoped state (identity, config, locks, walls) — the
    /// `~/.gitconfig` of TOHSENO, never the home of apps.
    machine_root: PathBuf,
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
        let home = Path::new(&home);
        let family = match std::env::var_os("TOHSENO_HOME") {
            Some(configured) => {
                let family = PathBuf::from(configured);
                if !family.is_absolute() {
                    return Err(LedgerError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "TOHSENO_HOME must be an absolute path",
                    )));
                }
                family
            }
            None => home.join(DEFAULT_FAMILY_DIRECTORY),
        };
        Ok(Self::at_homes(family, default_machine_data_root(home)))
    }

    /// Resolves the ledger for a specific app folder, wherever it stands —
    /// the folder's parent becomes the family home for this invocation.
    pub fn for_app_folder(folder: &Path) -> Result<(Self, String), LedgerError> {
        let canonical = fs::canonicalize(folder)?;
        let name = canonical
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| LedgerError::InvalidName(canonical.display().to_string()))?
            .to_owned();
        validate_app_name(&name)?;
        let parent = canonical
            .parent()
            .ok_or_else(|| LedgerError::Corrupt("app folder has no parent".into()))?
            .to_path_buf();
        let machine_root = match std::env::var_os("TOHSENO_DATA_ROOT") {
            Some(configured) => {
                let machine_root = PathBuf::from(configured);
                if !machine_root.is_absolute() {
                    return Err(LedgerError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "TOHSENO_DATA_ROOT must be an absolute path",
                    )));
                }
                machine_root
            }
            None => {
                let home = std::env::var_os("HOME").ok_or_else(|| {
                    LedgerError::Io(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "HOME is not set",
                    ))
                })?;
                default_machine_data_root(Path::new(&home))
            }
        };
        Ok((Self::at_homes(parent, machine_root), name))
    }

    pub fn at(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            machine_root: root.clone(),
            root,
        }
    }

    pub fn at_homes(root: impl Into<PathBuf>, machine_root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            machine_root: machine_root.into(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn machine_root(&self) -> &Path {
        &self.machine_root
    }

    /// The app's visible folder: its living working tree.
    pub fn working_tree(&self, app_name: &str) -> PathBuf {
        self.app_dir(app_name)
    }

    pub fn initialize(&self) -> Result<(), LedgerError> {
        for root in [&self.root, &self.machine_root] {
            if root.exists() {
                require_real_directory(root)?;
            } else {
                fs::create_dir_all(root)?;
                require_real_directory(root)?;
            }
        }
        ensure_real_directory(&self.machine_root.join("locks"))?;
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
        let locks = self.machine_root.join("locks");
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
        fs::create_dir(self.tohseno_dir(name))?;
        fs::create_dir(self.evolutions_dir(name))?;
        let record = AppRecord {
            name: name.into(),
            target_name: Some(name.into()),
            bundle_id: bundle_id.into(),
            created_at_unix: now_unix(),
            latest_evolution: None,
            shot_id: None,
            builder_id: None,
            expression_id: None,
            retired: false,
            parents: BTreeMap::new(),
        };
        self.write_record(&record)?;
        Ok(record)
    }

    /// Adopts an existing plain folder in the family home as a Shot: the
    /// folder gains its `.tohseno/` ledger and nothing else changes.
    pub fn adopt_app(&self, name: &str, bundle_id: &str) -> Result<AppRecord, LedgerError> {
        validate_app_name(name)?;
        self.initialize()?;
        let app_dir = self.app_dir(name);
        require_real_directory(&app_dir)?;
        match fs::symlink_metadata(self.tohseno_dir(name)) {
            Ok(_) => return Err(LedgerError::AppExists(name.into())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        fs::create_dir(self.tohseno_dir(name))?;
        fs::create_dir(self.evolutions_dir(name))?;
        let record = AppRecord {
            name: name.into(),
            target_name: Some(name.into()),
            bundle_id: bundle_id.into(),
            created_at_unix: now_unix(),
            latest_evolution: None,
            shot_id: None,
            builder_id: None,
            expression_id: None,
            retired: false,
            parents: BTreeMap::new(),
        };
        self.write_record(&record)?;
        Ok(record)
    }

    pub fn load_app(&self, name: &str) -> Result<AppRecord, LedgerError> {
        validate_app_name(name)?;
        require_real_directory(&self.root)?;
        for directory in [
            self.app_dir(name),
            self.tohseno_dir(name),
            self.evolutions_dir(name),
        ] {
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
        let path = self.tohseno_dir(name).join("app.toml");
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
        validate_app_record_at(&record, name)?;
        let renamed = record.name != name;
        if renamed {
            if record.target_name.is_none() {
                record.target_name = Some(record.name.clone());
            }
            record.name = name.into();
        }
        self.recover_interrupted_finalization(&mut record)?;
        if renamed {
            self.write_record(&record)?;
        }
        Ok(record)
    }

    pub fn list_apps(&self) -> Result<Vec<AppRecord>, LedgerError> {
        self.initialize()?;
        let mut records = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            // A folder is an app exactly when it carries its own ledger.
            if !entry
                .path()
                .join(APP_LEDGER_DIRECTORY)
                .join("app.toml")
                .is_file()
            {
                continue;
            }
            if let Ok(record) = self.load_app(name) {
                records.push(record);
            }
        }
        records.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(records)
    }

    /// Reserves the next integer shot. A reserved shot is writable until
    /// `finalize_evolution`; finalized shots are rejected by all ledger write APIs.
    pub fn reserve_evolution(
        &self,
        app_name: &str,
        parent: Option<u32>,
    ) -> Result<Evolution, LedgerError> {
        let mut record = self.load_app(app_name)?;
        let evolutions_dir = self.evolutions_dir(app_name);
        let number = next_sequence(&record)?;
        let path = evolutions_dir.join(format!("{number:04}"));
        if path.exists() {
            self.archive_incomplete_evolution(app_name, number, &path)?;
        }
        fs::create_dir(&path)?;
        for child in ["images", "src", "artifact"] {
            fs::create_dir(path.join(child))?;
        }
        if let Some(parent_number) = parent {
            record.parents.insert(number.to_string(), parent_number);
            self.write_record(&record)?;
        }
        Ok(Evolution {
            app_name: app_name.into(),
            number,
            path,
        })
    }

    /// Preserves a failed or interrupted attempt without allowing it to consume
    /// the next protocol sequence number.
    fn archive_incomplete_evolution(
        &self,
        app_name: &str,
        number: u32,
        path: &Path,
    ) -> Result<(), LedgerError> {
        let archive_root = self.tohseno_dir(app_name).join("incomplete");
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

    pub fn write_evolution_file(
        &self,
        shot: &Evolution,
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

    pub fn append_evolution_log(
        &self,
        shot: &Evolution,
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

    pub fn finalize_evolution(&self, shot: &Evolution) -> Result<(), LedgerError> {
        self.assert_writable(shot)?;
        let mut record = self.load_app(&shot.app_name)?;
        if next_sequence(&record)? != shot.number {
            return Err(LedgerError::Corrupt(format!(
                "shot {} is not the next append-only sequence",
                shot.number
            )));
        }
        self.publish_completion_marker(shot)?;
        record.latest_evolution = Some(shot.number);
        self.write_record(&record)?;
        Ok(())
    }

    pub fn latest_evolution(&self, app_name: &str) -> Result<Option<Evolution>, LedgerError> {
        let record = self.load_app(app_name)?;
        Ok(record.latest_evolution.map(|number| Evolution {
            app_name: app_name.into(),
            number,
            path: self.evolutions_dir(app_name).join(format!("{number:04}")),
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
                record.expression_id = Some(ExpressionId::random());
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

    /// Fill the expression identity for a pre-v2 ledger exactly once.
    ///
    /// Native Shots use a random ExpressionID at creation. A frozen v1
    /// lineage uses the protocol's deterministic compatibility derivation.
    pub fn bind_expression_identity(
        &self,
        app_name: &str,
        expression_id: ExpressionId,
    ) -> Result<AppRecord, LedgerError> {
        if expression_id.is_zero() {
            return Err(LedgerError::Corrupt(
                "expression identity must not be zero".into(),
            ));
        }
        let mut record = self.load_app(app_name)?;
        match record.expression_id {
            None => {
                record.expression_id = Some(expression_id);
                self.write_record(&record)?;
                Ok(record)
            }
            Some(existing) if existing == expression_id => Ok(record),
            Some(_) => Err(LedgerError::Corrupt(format!(
                "{app_name} already has a different ExpressionID"
            ))),
        }
    }

    pub fn shot(&self, app_name: &str, number: u32) -> Result<Evolution, LedgerError> {
        validate_app_name(app_name)?;
        if number == 0 {
            return Err(LedgerError::Corrupt(
                "shot sequence must be at least 1".into(),
            ));
        }
        let path = self.evolutions_dir(app_name).join(format!("{number:04}"));
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
        Ok(Evolution {
            app_name: app_name.into(),
            number,
            path,
        })
    }

    pub fn list_evolutions(&self, app_name: &str) -> Result<Vec<Evolution>, LedgerError> {
        let record = self.load_app(app_name)?;
        let evolutions_directory = self.evolutions_dir(app_name);
        let mut shots = Vec::new();
        for entry in fs::read_dir(evolutions_directory)? {
            let entry = entry?;
            let Ok(number) = entry.file_name().to_string_lossy().parse::<u32>() else {
                continue;
            };
            if !entry.file_type()?.is_dir() {
                return Err(LedgerError::Corrupt(format!(
                    "shot {number} is not a real directory"
                )));
            }
            let shot = Evolution {
                app_name: app_name.into(),
                number,
                path: entry.path(),
            };
            if has_valid_completion_marker(&shot.path, number)? {
                if record.latest_evolution.is_none_or(|latest| number > latest) {
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

    /// Verify and project the frozen signed v1 Evolution chain without
    /// rewriting, re-signing, or inventing historical genome facts.
    ///
    /// This is the compatibility boundary for current Shot repositories.
    /// The returned protocol adapter marks intention/genome availability
    /// honestly and derives stable neutral Expression/Version IDs.
    pub fn adapt_v1_lineage(&self, app_name: &str) -> Result<AdaptedV1Lineage, LedgerError> {
        let evolutions = self.list_evolutions(app_name)?;
        if evolutions.is_empty() {
            return Err(LedgerError::Corrupt(format!(
                "{app_name} has no completed v1 Evolution to migrate"
            )));
        }
        let mut records = Vec::with_capacity(evolutions.len());
        let mut signatures = Vec::with_capacity(evolutions.len());
        for evolution in evolutions {
            records.push(read_bounded_json::<ShotRecord>(
                &evolution.path.join("TOHSENO/shot.json"),
            )?);
            signatures.push(read_bounded_json::<SignatureSidecar>(
                &evolution.path.join("TOHSENO/signature.json"),
            )?);
        }
        let entries = records
            .iter()
            .zip(&signatures)
            .collect::<Vec<(&ShotRecord, &SignatureSidecar)>>();
        tohseno_protocol::adapt_v1_lineage(&entries)
            .map_err(|error| LedgerError::Corrupt(format!("v1 lineage is invalid: {error}")))
    }

    /// Idempotently bind a current ledger to the identities proven by its
    /// frozen v1 chain. Signed v1 records remain untouched.
    pub fn migrate_v1_identity(&self, app_name: &str) -> Result<AdaptedV1Lineage, LedgerError> {
        let adapted = self.adapt_v1_lineage(app_name)?;
        let mut app = self.load_app(app_name)?;
        if app.shot_id.is_some_and(|value| value != adapted.shot_id)
            || app
                .builder_id
                .is_some_and(|value| value != adapted.controller)
            || app
                .expression_id
                .is_some_and(|value| value != adapted.expression_id)
        {
            return Err(LedgerError::Corrupt(format!(
                "{app_name} identity conflicts with its verified v1 lineage"
            )));
        }
        let changed =
            app.shot_id.is_none() || app.builder_id.is_none() || app.expression_id.is_none();
        app.shot_id = Some(adapted.shot_id);
        app.builder_id = Some(adapted.controller);
        app.expression_id = Some(adapted.expression_id);
        if changed {
            self.write_record(&app)?;
        }
        Ok(adapted)
    }

    /// Copy one or every v0.6 app from the hidden stable ledger into the
    /// folder-shaped 0.7 family without modifying or deleting the old bytes.
    ///
    /// The destination is assembled and validated under a private sibling
    /// directory, then renamed into place. Existing destination folders,
    /// symlinks, special files, and malformed legacy records fail closed.
    pub fn migrate_legacy_v0_6_apps(
        &self,
        selected: Option<&str>,
    ) -> Result<Vec<String>, LedgerError> {
        let legacy_apps = self.machine_root.join("apps");
        let metadata = match fs::symlink_metadata(&legacy_apps) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(LedgerError::Corrupt(
                "legacy v0.6 apps path is not a real directory".into(),
            ));
        }

        self.initialize()?;
        let names = if let Some(name) = selected {
            validate_app_name(name)?;
            vec![name.to_owned()]
        } else {
            let mut names = Vec::new();
            for entry in fs::read_dir(&legacy_apps)? {
                let entry = entry?;
                let name = entry.file_name();
                let Some(name) = name.to_str() else {
                    return Err(LedgerError::Corrupt(
                        "legacy v0.6 app name is not UTF-8".into(),
                    ));
                };
                validate_app_name(name)?;
                let metadata = entry.metadata()?;
                if !metadata.is_dir() || entry.file_type()?.is_symlink() {
                    return Err(LedgerError::Corrupt(format!(
                        "legacy v0.6 app {name} is not a real directory"
                    )));
                }
                names.push(name.to_owned());
            }
            names.sort();
            names
        };

        let mut migrated = Vec::with_capacity(names.len());
        for name in names {
            let source_app = legacy_apps.join(&name);
            require_real_directory(&source_app)?;
            let destination = self.working_tree(&name);
            if fs::symlink_metadata(&destination).is_ok() {
                return Err(LedgerError::AppExists(name));
            }

            let record_path = source_app.join("app.toml");
            let record_metadata = fs::symlink_metadata(&record_path)?;
            if record_metadata.file_type().is_symlink()
                || !record_metadata.is_file()
                || record_metadata.len() > APP_RECORD_LIMIT
            {
                return Err(LedgerError::Corrupt(format!(
                    "legacy v0.6 app {name} has an unsafe app.toml"
                )));
            }
            let encoded =
                read_unchanged_regular_file(&record_path, APP_RECORD_LIMIT, &record_metadata)?;
            let record =
                toml::from_str::<AppRecord>(std::str::from_utf8(&encoded).map_err(|error| {
                    LedgerError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, error))
                })?)?;
            if record.name != name {
                return Err(LedgerError::Corrupt(format!(
                    "legacy v0.6 app directory {name} contains record for {}",
                    record.name
                )));
            }
            let latest = record.latest_evolution.ok_or_else(|| {
                LedgerError::Corrupt(format!(
                    "legacy v0.6 app {name} has no completed Shot to migrate"
                ))
            })?;
            let latest_source = source_app
                .join("shots")
                .join(format!("{latest:04}"))
                .join("src");
            require_real_directory(&latest_source)?;

            let staging_parent = reserve_legacy_migration_stage(&self.root)?;
            let staged_app = staging_parent.join(&name);
            let result = (|| -> Result<(), LedgerError> {
                fs::create_dir(&staged_app)?;
                copy_legacy_tree(&latest_source, &staged_app, true)?;
                let staged_ledger = staged_app.join(APP_LEDGER_DIRECTORY);
                fs::create_dir(&staged_ledger)?;
                fs::copy(&record_path, staged_ledger.join("app.toml"))?;
                copy_legacy_tree(
                    &source_app.join("shots"),
                    &staged_ledger.join("evolutions"),
                    false,
                )?;

                let validator = Ledger::at_homes(staging_parent.clone(), self.machine_root.clone());
                let validated = validator.load_app(&name)?;
                if validated.latest_evolution != Some(latest) {
                    return Err(LedgerError::Corrupt(format!(
                        "legacy v0.6 app {name} changed during migration"
                    )));
                }
                fs::rename(&staged_app, &destination)?;
                Ok(())
            })();
            let cleanup_result = fs::remove_dir_all(&staging_parent);
            if let Err(error) = result {
                let _ = cleanup_result;
                return Err(error);
            }
            cleanup_result?;
            migrated.push(name);
        }
        Ok(migrated)
    }

    fn assert_writable(&self, shot: &Evolution) -> Result<(), LedgerError> {
        validate_app_name(&shot.app_name)?;
        if shot.number == 0 {
            return Err(LedgerError::Corrupt(
                "shot sequence must be at least 1".into(),
            ));
        }
        let shots = self.evolutions_dir(&shot.app_name);
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
        self.root.join(name)
    }

    fn tohseno_dir(&self, name: &str) -> PathBuf {
        self.app_dir(name).join(APP_LEDGER_DIRECTORY)
    }

    fn evolutions_dir(&self, name: &str) -> PathBuf {
        self.tohseno_dir(name).join("evolutions")
    }

    /// The private briefing home inside the app's own ledger.
    pub fn briefing_dir(&self, name: &str) -> PathBuf {
        self.tohseno_dir(name)
    }

    /// `.complete` is the immutable commit point. If a process stops after
    /// durably creating it but before replacing app.toml, replay only that
    /// single next sequence into the derived app head.
    fn recover_interrupted_finalization(&self, record: &mut AppRecord) -> Result<(), LedgerError> {
        if let Some(number) = record.latest_evolution {
            let current = self
                .evolutions_dir(&record.name)
                .join(format!("{number:04}"));
            require_real_shot_directory(&current, number)?;
            if !has_valid_completion_marker(&current, number)? {
                return Err(LedgerError::Corrupt(format!(
                    "app.toml recognizes shot {number}, but its completion marker is missing"
                )));
            }
        }

        let Some(number) = record.latest_evolution.unwrap_or(0).checked_add(1) else {
            return Ok(());
        };
        let path = self
            .evolutions_dir(&record.name)
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

        record.latest_evolution = Some(number);
        self.write_record(record)
    }

    fn publish_completion_marker(&self, shot: &Evolution) -> Result<(), LedgerError> {
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
        let directory = self.tohseno_dir(&record.name);
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

impl Ledger {
    /// Copies the living working tree into a reserved shot's `src/`,
    /// skipping everything a sealed world forbids (the in-folder ledger,
    /// VCS state, Finder droppings, logs, user-local Xcode state).
    pub fn snapshot_working_tree(&self, shot: &Evolution) -> Result<(), LedgerError> {
        self.assert_writable(shot)?;
        let source = self.working_tree(&shot.app_name);
        require_real_directory(&source)?;
        copy_working_entries(&source, &source, &shot.source_path())?;
        Ok(())
    }

    /// Makes the working tree exactly mirror one shot's sealed world.
    /// Entries a sealed world forbids (`.tohseno`, `.git`, `.DS_Store`, …)
    /// are preserved untouched; everything else is replaced.
    pub fn checkout_working_tree(&self, shot: &Evolution) -> Result<(), LedgerError> {
        validate_app_name(&shot.app_name)?;
        let expected = self
            .evolutions_dir(&shot.app_name)
            .join(format!("{:04}", shot.number));
        if shot.path != expected {
            return Err(LedgerError::Corrupt(format!(
                "shot {} has a path outside its ledger sequence",
                shot.number
            )));
        }
        let target = self.working_tree(&shot.app_name);
        require_real_directory(&target)?;
        // Copy first, prune second: a failure mid-checkout leaves the folder
        // with everything it had plus part of the seal — never gutted.
        let mut sealed = std::collections::BTreeSet::new();
        collect_relative_paths(&shot.source_path(), &shot.source_path(), &mut sealed)?;
        copy_sealed_entries(&shot.source_path(), &target)?;
        prune_extraneous_entries(&target, &target, &sealed)?;
        Ok(())
    }
}

fn collect_relative_paths(
    root: &Path,
    directory: &Path,
    paths: &mut std::collections::BTreeSet<String>,
) -> Result<(), LedgerError> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let relative = entry
            .path()
            .strip_prefix(root)
            .map_err(|_| LedgerError::Corrupt("sealed world walked outside itself".into()))?
            .components()
            .map(|component| component.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        if entry.file_type()?.is_dir() {
            collect_relative_paths(root, &entry.path(), paths)?;
        }
        paths.insert(relative);
    }
    Ok(())
}

/// Removes entries the seal does not contain, preserving forbidden entries
/// (secrets, VCS state, the in-folder ledger, user-local Xcode state) and
/// names strict hashing could not express, at **any** depth. A directory
/// that still shelters preserved entries survives.
fn prune_extraneous_entries(
    root: &Path,
    directory: &Path,
    sealed: &std::collections::BTreeSet<String>,
) -> Result<(), LedgerError> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let Some(_) = entry.file_name().to_str() else {
            continue;
        };
        let relative = entry
            .path()
            .strip_prefix(root)
            .map_err(|_| LedgerError::Corrupt("working tree walked outside itself".into()))?
            .components()
            .map(|component| component.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        if tohseno_protocol::tree_hash::forbidden_source_reason(&relative).is_some()
            || crate::shot_layout::is_shot_level_path(&relative)
            || relative.chars().any(char::is_control)
        {
            continue;
        }
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            prune_extraneous_entries(root, &entry.path(), sealed)?;
            if !sealed.contains(&relative) && fs::read_dir(entry.path())?.next().is_none() {
                fs::remove_dir(entry.path())?;
            }
        } else if !sealed.contains(&relative) {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

/// Working-tree → snapshot copy: refuses symlinks, skips forbidden entries.
fn copy_working_entries(
    root: &Path,
    directory: &Path,
    destination: &Path,
) -> Result<(), LedgerError> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let relative = entry
            .path()
            .strip_prefix(root)
            .map_err(|_| LedgerError::Corrupt("working tree walked outside itself".into()))?
            .components()
            .map(|component| component.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        // Forbidden entries are skipped before the symlink refusal so a
        // symlinked `.git` (worktrees) cannot block a snapshot. Engine-owned
        // protocol sidecars are also skipped: a sealed world must never
        // carry unsigned bytes under those exact names. Names strict hashing
        // could not express (control characters, non-UTF-8) are left behind
        // rather than sealed.
        if tohseno_protocol::tree_hash::forbidden_source_reason(&relative).is_some()
            || tohseno_protocol::tree_hash::exclusion_reason(&relative).is_some()
            || crate::shot_layout::is_shot_level_path(&relative)
            || relative.chars().any(char::is_control)
            || entry.file_name().to_str().is_none()
        {
            continue;
        }
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(LedgerError::Corrupt(format!(
                "refusing symlink in the working tree: {}",
                entry.path().display()
            )));
        }
        let target = destination.join(entry.file_name());
        if file_type.is_dir() {
            copy_working_entries(root, &entry.path(), &target)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Sealed snapshot → working tree copy: the source is already clean.
fn copy_sealed_entries(source: &Path, destination: &Path) -> Result<(), LedgerError> {
    match fs::symlink_metadata(destination) {
        Ok(metadata) if !metadata.is_dir() => {
            fs::remove_file(destination)?;
        }
        _ => {}
    }
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(LedgerError::Corrupt(format!(
                "refusing symlink in a sealed world: {}",
                entry.path().display()
            )));
        }
        let target = destination.join(entry.file_name());
        if file_type.is_dir() {
            copy_sealed_entries(&entry.path(), &target)?;
        } else if file_type.is_file() {
            if fs::symlink_metadata(&target)
                .map(|metadata| metadata.is_dir())
                .unwrap_or(false)
            {
                fs::remove_dir_all(&target)?;
            }
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn next_sequence(record: &AppRecord) -> Result<u32, LedgerError> {
    record
        .latest_evolution
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| LedgerError::Corrupt(format!("{} exhausted the shot sequence", record.name)))
}

fn reserve_legacy_migration_stage(root: &Path) -> Result<PathBuf, LedgerError> {
    for ordinal in 1_u32..=1_000 {
        let path = root.join(format!(
            ".tohseno-v0.6-migration-{}-{}-{ordinal:04}",
            std::process::id(),
            now_unix()
        ));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err(LedgerError::Corrupt(
        "could not reserve a private legacy migration directory".into(),
    ))
}

fn copy_legacy_tree(
    source: &Path,
    destination: &Path,
    reject_embedded_ledger: bool,
) -> Result<(), LedgerError> {
    require_real_directory(source)?;
    match fs::symlink_metadata(destination) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(unsafe_path_error());
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(destination)?;
        }
        Err(error) => return Err(error.into()),
    }
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let file_name = entry.file_name();
        if reject_embedded_ledger && file_name == APP_LEDGER_DIRECTORY {
            return Err(LedgerError::Corrupt(
                "legacy source contains an unexpected embedded .tohseno ledger".into(),
            ));
        }
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_symlink() {
            return Err(LedgerError::Corrupt(format!(
                "legacy v0.6 tree contains symlink {}",
                entry.path().display()
            )));
        }
        let target = destination.join(&file_name);
        if metadata.is_dir() {
            copy_legacy_tree(&entry.path(), &target, false)?;
        } else if metadata.is_file() {
            fs::copy(entry.path(), target)?;
        } else {
            return Err(LedgerError::Corrupt(format!(
                "legacy v0.6 tree contains special file {}",
                entry.path().display()
            )));
        }
    }
    Ok(())
}

fn default_machine_data_root(home: &Path) -> PathBuf {
    home.join(DEFAULT_MACHINE_DATA_DIRECTORY)
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

fn read_bounded_json<T: DeserializeOwned>(path: &Path) -> Result<T, LedgerError> {
    const PROTOCOL_JSON_LIMIT: u64 = 4 * 1024 * 1024;
    let metadata = fs::symlink_metadata(path)?;
    let bytes = read_unchanged_regular_file(path, PROTOCOL_JSON_LIMIT, &metadata)?;
    serde_json::from_slice(&bytes).map_err(|error| {
        LedgerError::Corrupt(format!(
            "{} is not canonical protocol JSON: {error}",
            path.display()
        ))
    })
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

fn prepare_shot_path(shot: &Evolution, relative_path: &Path) -> Result<PathBuf, LedgerError> {
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
    validate_app_record_at(record, expected_name)?;
    if record.name != expected_name {
        return Err(LedgerError::Corrupt(format!(
            "app.toml names {} inside the {expected_name} directory",
            record.name
        )));
    }
    Ok(())
}

fn validate_app_record_at(record: &AppRecord, expected_name: &str) -> Result<(), LedgerError> {
    validate_app_name(expected_name)?;
    validate_app_name(&record.name)?;
    if let Some(target_name) = &record.target_name {
        validate_app_name(target_name)?;
    }
    if record.expression_id.is_some_and(ExpressionId::is_zero) {
        return Err(LedgerError::Corrupt(
            "app.toml contains a zero ExpressionID".into(),
        ));
    }
    if record.bundle_id.is_empty()
        || record.bundle_id.len() > 255
        || record.bundle_id.chars().any(char::is_control)
    {
        return Err(LedgerError::Corrupt(format!(
            "{expected_name} has an invalid bundle identifier"
        )));
    }
    if record.created_at_unix == 0 || record.latest_evolution == Some(0) {
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
        || RESERVED_APP_NAMES.contains(&name)
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
        let first = ledger.reserve_evolution("paper-press", None).unwrap();
        assert_eq!(first.number, 1);
        ledger
            .write_evolution_file(&first, "prompt.md", b"Make a press.")
            .unwrap();
        ledger.finalize_evolution(&first).unwrap();
        assert!(matches!(
            ledger.write_evolution_file(&first, "prompt.md", b"mutation"),
            Err(LedgerError::ShotFinalized(1))
        ));

        let second = ledger.reserve_evolution("paper-press", Some(1)).unwrap();
        assert_eq!(second.number, 2);
        assert_eq!(
            ledger.load_app("paper-press").unwrap().parents.get("2"),
            Some(&1)
        );
        assert_eq!(
            ledger
                .list_evolutions("paper-press")
                .unwrap()
                .iter()
                .map(|shot| shot.number)
                .collect::<Vec<_>>(),
            [1]
        );
    }

    #[test]
    fn apps_are_visible_folders_carrying_their_own_ledger() {
        let temporary = tempfile::tempdir().unwrap();
        let ledger = Ledger::at(temporary.path());
        ledger
            .create_app("press", "com.tohseno.test.press")
            .unwrap();
        assert_eq!(ledger.working_tree("press"), temporary.path().join("press"));
        assert!(temporary.path().join("press/.tohseno/app.toml").is_file());
        assert!(temporary.path().join("press/.tohseno/evolutions").is_dir());
        // A random visible folder without a ledger is not an app.
        fs::create_dir(temporary.path().join("holiday-photos")).unwrap();
        let names = ledger
            .list_apps()
            .unwrap()
            .into_iter()
            .map(|app| app.name)
            .collect::<Vec<_>>();
        assert_eq!(names, ["press"]);
        assert!(matches!(
            ledger.create_app("identity", "com.tohseno.test.identity"),
            Err(LedgerError::InvalidName(_))
        ));
    }

    #[test]
    fn legacy_v0_6_migration_copies_into_a_visible_folder_without_deleting_history() {
        let temporary = tempfile::tempdir().unwrap();
        let family = temporary.path().join("family");
        let machine = temporary.path().join("machine");
        let legacy = machine.join("apps/field-notebook");
        fs::create_dir_all(legacy.join("shots/0001/src")).unwrap();
        fs::create_dir_all(legacy.join("shots/0001/images")).unwrap();
        fs::create_dir_all(legacy.join("shots/0001/artifact")).unwrap();
        fs::write(
            legacy.join("app.toml"),
            "name = \"field-notebook\"\n\
             bundle_id = \"com.tohseno.test.field-notebook\"\n\
             created_at_unix = 1\n\
             latest_shot = 1\n\
             retired = false\n",
        )
        .unwrap();
        fs::write(
            legacy.join("shots/0001/src/App.swift"),
            b"struct FieldNotebook {}\n",
        )
        .unwrap();
        fs::write(legacy.join("shots/0001/.complete"), COMPLETE_MARKER).unwrap();

        let ledger = Ledger::at_homes(&family, &machine);
        let migrated = ledger.migrate_legacy_v0_6_apps(None).unwrap();
        assert_eq!(migrated, ["field-notebook"]);
        assert_eq!(
            fs::read(family.join("field-notebook/App.swift")).unwrap(),
            b"struct FieldNotebook {}\n"
        );
        assert!(family
            .join("field-notebook/.tohseno/evolutions/0001/.complete")
            .is_file());
        assert!(legacy.join("shots/0001/src/App.swift").is_file());
        assert!(matches!(
            ledger.migrate_legacy_v0_6_apps(Some("field-notebook")),
            Err(LedgerError::AppExists(_))
        ));
    }

    #[test]
    fn snapshot_and_checkout_keep_the_working_tree_and_seal_in_agreement() {
        let temporary = tempfile::tempdir().unwrap();
        let ledger = Ledger::at(temporary.path());
        ledger
            .create_app("press", "com.tohseno.test.press")
            .unwrap();
        let working = ledger.working_tree("press");
        fs::write(working.join("App.swift"), b"struct App {}\n").unwrap();
        fs::create_dir(working.join("Assets")).unwrap();
        fs::write(working.join("Assets/a.json"), b"{}\n").unwrap();
        // Junk a living folder accumulates never enters a sealed world.
        fs::write(working.join(".DS_Store"), b"finder\n").unwrap();
        fs::create_dir(working.join(".git")).unwrap();
        fs::write(working.join(".git/HEAD"), b"ref\n").unwrap();
        // Shot-level working surfaces remain beside the expression but never
        // enter its immutable v1 source world.
        fs::write(working.join("INTENTION.md"), b"exact private intention\n").unwrap();
        fs::write(working.join("GENOME.md"), b"# Accepted genome\n").unwrap();
        fs::create_dir_all(working.join("feedback/versions/0001")).unwrap();
        fs::write(
            working.join("feedback/versions/0001/note.txt"),
            b"private feedback\n",
        )
        .unwrap();

        let shot = ledger.reserve_evolution("press", None).unwrap();
        ledger.snapshot_working_tree(&shot).unwrap();
        assert!(shot.source_path().join("App.swift").is_file());
        assert!(!shot.source_path().join(".DS_Store").exists());
        assert!(!shot.source_path().join(".git").exists());
        assert!(!shot.source_path().join(".tohseno").exists());
        assert!(!shot.source_path().join("INTENTION.md").exists());
        assert!(!shot.source_path().join("GENOME.md").exists());
        assert!(!shot.source_path().join("feedback").exists());
        assert_eq!(
            crate::shot_layout::hash_expression_working_tree(&working)
                .unwrap()
                .digest,
            tohseno_protocol::tree_hash::hash_source_tree(&shot.source_path())
                .unwrap()
                .digest
        );

        // The sealed world changes; checkout mirrors it, preserving junk —
        // including forbidden entries nested inside replaceable directories.
        fs::write(shot.source_path().join("App.swift"), b"struct App2 {}\n").unwrap();
        fs::write(working.join("scratch.swift"), b"let x = 1\n").unwrap();
        fs::create_dir_all(working.join("press.xcodeproj/xcuserdata")).unwrap();
        fs::write(
            working.join("press.xcodeproj/xcuserdata/state.plist"),
            b"user\n",
        )
        .unwrap();
        fs::create_dir_all(working.join("Certs")).unwrap();
        fs::write(working.join("Certs/key.pem"), b"secret\n").unwrap();
        ledger.checkout_working_tree(&shot).unwrap();
        assert_eq!(
            fs::read(working.join("App.swift")).unwrap(),
            b"struct App2 {}\n"
        );
        assert!(!working.join("scratch.swift").exists());
        assert!(working.join(".git/HEAD").is_file());
        assert!(working.join(".tohseno/app.toml").is_file());
        assert_eq!(
            fs::read(working.join("INTENTION.md")).unwrap(),
            b"exact private intention\n"
        );
        assert!(working.join("feedback/versions/0001/note.txt").is_file());
        assert!(working
            .join("press.xcodeproj/xcuserdata/state.plist")
            .is_file());
        assert!(working.join("Certs/key.pem").is_file());
    }

    #[test]
    fn folder_rename_and_move_preserve_shot_and_expression_target_identity() {
        let temporary = tempfile::tempdir().unwrap();
        let first_home = temporary.path().join("first");
        let second_home = temporary.path().join("second");
        let machine = temporary.path().join("machine");
        fs::create_dir_all(&first_home).unwrap();
        fs::create_dir_all(&second_home).unwrap();
        let ledger = Ledger::at_homes(&first_home, &machine);
        ledger.initialize().unwrap();
        ledger
            .create_app("quiet-place", "com.tohseno.test.quiet-place")
            .unwrap();
        let shot_id = tohseno_protocol::digest::ShotId::from_bytes([0x44; 32]);
        let builder_id = tohseno_protocol::identity::BuilderId::new(
            tohseno_protocol::digest::Address20::from_bytes([0x55; 20]),
        );
        let bound = ledger
            .bind_protocol_identity("quiet-place", shot_id, builder_id)
            .unwrap();
        let expression_id = bound.expression_id.unwrap();
        let first = ledger.reserve_evolution("quiet-place", None).unwrap();
        ledger
            .write_evolution_file(&first, "prompt.md", b"original")
            .unwrap();
        ledger.finalize_evolution(&first).unwrap();

        let moved = second_home.join("calm-home");
        fs::rename(first_home.join("quiet-place"), &moved).unwrap();
        let moved_ledger = Ledger::at_homes(&second_home, &machine);
        let record = moved_ledger.load_app("calm-home").unwrap();
        assert_eq!(record.name, "calm-home");
        assert_eq!(record.target_name(), "quiet-place");
        assert_eq!(record.shot_id, Some(shot_id));
        assert_eq!(record.builder_id, Some(builder_id));
        assert_eq!(record.expression_id, Some(expression_id));
        assert_eq!(record.latest_evolution, Some(1));
        assert_eq!(
            moved_ledger
                .latest_evolution("calm-home")
                .unwrap()
                .unwrap()
                .number,
            1
        );

        let persisted = fs::read_to_string(moved.join(".tohseno/app.toml")).unwrap();
        assert!(persisted.contains("name = \"calm-home\""));
        assert!(persisted.contains("target_name = \"quiet-place\""));
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
    fn stable_machine_state_and_visible_family_roots_are_distinct() {
        let home = Path::new("/Users/example");
        assert_eq!(
            default_machine_data_root(home),
            Path::new("/Users/example/.tohseno")
        );
        assert_ne!(
            default_machine_data_root(home),
            home.join(DEFAULT_FAMILY_DIRECTORY)
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

        let failed = ledger.reserve_evolution("press", None).unwrap();
        ledger
            .write_evolution_file(&failed, "build.log", b"failed")
            .unwrap();

        let retry = ledger.reserve_evolution("press", None).unwrap();
        assert_eq!(retry.number, 1);
        assert!(!retry.path.join("build.log").exists());
        let archived = temporary.path().join("press/.tohseno/incomplete");
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
        let shot = ledger.reserve_evolution("press", None).unwrap();

        let outside = temporary.path().join("outside");
        fs::create_dir(&outside).unwrap();
        symlink(&outside, shot.path.join("escape")).unwrap();
        assert!(ledger
            .write_evolution_file(&shot, "escape/owned", b"escaped")
            .is_err());
        assert!(!outside.join("owned").exists());

        let outside_log = outside.join("log");
        fs::write(&outside_log, b"untouched").unwrap();
        symlink(&outside_log, shot.path.join("build.log")).unwrap();
        assert!(ledger
            .append_evolution_log(&shot, "build.log", b"escaped")
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
        let shot = ledger.reserve_evolution("press", None).unwrap();
        let record_path = temporary.path().join("ledger/press/.tohseno/app.toml");
        let corrupted = fs::read_to_string(&record_path)
            .unwrap()
            .replace("name = \"press\"", "name = \"../outside\"");
        fs::write(record_path, corrupted).unwrap();

        assert!(ledger.finalize_evolution(&shot).is_err());
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
        let shot = ledger.reserve_evolution("press", None).unwrap();

        // Simulate a stop after the marker became durable but before app.toml
        // was atomically replaced.
        fs::write(shot.complete_path(), COMPLETE_MARKER).unwrap();
        let record_path = temporary.path().join("ledger/press/.tohseno/app.toml");
        let before =
            toml::from_str::<AppRecord>(&fs::read_to_string(&record_path).unwrap()).unwrap();
        assert_eq!(before.latest_evolution, None);

        let recovered = ledger.load_app("press").unwrap();
        assert_eq!(recovered.latest_evolution, Some(1));
        let persisted =
            toml::from_str::<AppRecord>(&fs::read_to_string(&record_path).unwrap()).unwrap();
        assert_eq!(persisted.latest_evolution, Some(1));
        assert_eq!(
            ledger.reserve_evolution("press", Some(1)).unwrap().number,
            2
        );
    }

    #[test]
    fn interrupted_finalize_refuses_a_tampered_marker() {
        let temporary = tempfile::tempdir().unwrap();
        let ledger = Ledger::at(temporary.path().join("ledger"));
        ledger
            .create_app("press", "com.tohseno.test.press")
            .unwrap();
        let shot = ledger.reserve_evolution("press", None).unwrap();
        fs::write(shot.complete_path(), b"completE\n").unwrap();
        let record_path = temporary.path().join("ledger/press/.tohseno/app.toml");
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
        ledger.reserve_evolution("press", None).unwrap();
        let mut record = ledger.load_app("press").unwrap();
        record.latest_evolution = Some(1);
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
        let forged = Evolution {
            app_name: "press".into(),
            number: 1,
            path: outside.clone(),
        };

        assert!(matches!(
            ledger.finalize_evolution(&forged),
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
        let shot = ledger.reserve_evolution("press", None).unwrap();
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
        let record_path = temporary.path().join("ledger/press/.tohseno/app.toml");
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
