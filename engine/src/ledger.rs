use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub enum LedgerError {
    Io(std::io::Error),
    TomlDecode(toml::de::Error),
    TomlEncode(toml::ser::Error),
    InvalidName(String),
    AppExists(String),
    AppMissing(String),
    ShotFinalized(u32),
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
            Self::ShotFinalized(number) => write!(f, "shot {number} is immutable"),
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

impl Ledger {
    pub fn discover() -> Result<Self, LedgerError> {
        let home = std::env::var_os("HOME").ok_or_else(|| {
            LedgerError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "HOME is not set",
            ))
        })?;
        Ok(Self::at(PathBuf::from(home).join(".tohseno")))
    }

    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn initialize(&self) -> Result<(), LedgerError> {
        fs::create_dir_all(self.root.join("apps"))?;
        Ok(())
    }

    pub fn create_app(&self, name: &str, bundle_id: &str) -> Result<AppRecord, LedgerError> {
        validate_app_name(name)?;
        self.initialize()?;
        let app_dir = self.app_dir(name);
        if app_dir.exists() {
            return Err(LedgerError::AppExists(name.into()));
        }
        fs::create_dir(&app_dir)?;
        fs::create_dir(app_dir.join("shots"))?;
        let record = AppRecord {
            name: name.into(),
            bundle_id: bundle_id.into(),
            created_at_unix: now_unix(),
            latest_shot: None,
            parents: BTreeMap::new(),
        };
        self.write_record(&record)?;
        Ok(record)
    }

    pub fn load_app(&self, name: &str) -> Result<AppRecord, LedgerError> {
        let path = self.app_dir(name).join("app.toml");
        if !path.exists() {
            return Err(LedgerError::AppMissing(name.into()));
        }
        Ok(toml::from_str(&fs::read_to_string(path)?)?)
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
        let number = next_shot_number(&shots_dir)?;
        let path = shots_dir.join(format!("{number:04}"));
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

    pub fn write_shot_file(
        &self,
        shot: &Shot,
        relative_path: impl AsRef<Path>,
        contents: &[u8],
    ) -> Result<(), LedgerError> {
        self.assert_writable(shot)?;
        let relative_path = relative_path.as_ref();
        if relative_path.is_absolute()
            || relative_path
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err(LedgerError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "shot path must stay inside its directory",
            )));
        }
        let path = shot.path.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, contents)?;
        Ok(())
    }

    pub fn append_shot_log(
        &self,
        shot: &Shot,
        relative_path: &str,
        contents: &[u8],
    ) -> Result<(), LedgerError> {
        self.assert_writable(shot)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(shot.path.join(relative_path))?;
        file.write_all(contents)?;
        Ok(())
    }

    pub fn finalize_shot(&self, shot: &Shot) -> Result<(), LedgerError> {
        self.assert_writable(shot)?;
        let mut marker = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(shot.complete_path())?;
        marker.write_all(b"complete\n")?;
        let mut record = self.load_app(&shot.app_name)?;
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

    fn assert_writable(&self, shot: &Shot) -> Result<(), LedgerError> {
        if shot.complete_path().exists() {
            Err(LedgerError::ShotFinalized(shot.number))
        } else {
            Ok(())
        }
    }

    fn app_dir(&self, name: &str) -> PathBuf {
        self.root.join("apps").join(name)
    }

    fn write_record(&self, record: &AppRecord) -> Result<(), LedgerError> {
        let encoded = toml::to_string_pretty(record)?;
        fs::write(self.app_dir(&record.name).join("app.toml"), encoded)?;
        Ok(())
    }
}

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

fn next_shot_number(shots_dir: &Path) -> Result<u32, LedgerError> {
    let mut latest = 0;
    for entry in fs::read_dir(shots_dir)? {
        let name = entry?.file_name();
        if let Ok(number) = name.to_string_lossy().parse::<u32>() {
            latest = latest.max(number);
        }
    }
    Ok(latest + 1)
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
    }

    #[test]
    fn names_are_filesystem_safe_and_stable() {
        assert_eq!(sanitize_component("My Great_App!"), "my-great-app");
        assert!(validate_app_name("replyguy-trencher").is_ok());
        assert!(validate_app_name("../replyguy").is_err());
    }
}
