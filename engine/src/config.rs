use crate::safe_file::read_bounded_utf8;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const MAX_CONFIG_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub harness: HarnessConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HarnessConfig {
    #[serde(default = "default_harness_command")]
    pub command: String,
}

impl Default for HarnessConfig {
    fn default() -> Self {
        Self {
            command: default_harness_command(),
        }
    }
}

impl Config {
    /// Loads an existing legacy harness configuration without creating one.
    /// The recording layer has no configuration ritual of its own.
    pub fn load_or_default(root: &Path) -> Result<Self, ConfigError> {
        let path = root.join("config.toml");
        let body = match read_bounded_utf8(&path, MAX_CONFIG_BYTES) {
            Ok(body) => body,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default())
            }
            Err(error) => return Err(error.into()),
        };
        Ok(toml::from_str(&body)?)
    }

    pub fn load_or_create(root: &Path) -> Result<Self, ConfigError> {
        fs::create_dir_all(root)?;
        let path = root.join("config.toml");
        let body = match read_bounded_utf8(&path, MAX_CONFIG_BYTES) {
            Ok(body) => body,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let config = Self::default();
                config.save(root)?;
                return Ok(config);
            }
            Err(error) => return Err(error.into()),
        };
        Ok(toml::from_str(&body)?)
    }

    pub fn save(&self, root: &Path) -> Result<(), ConfigError> {
        fs::create_dir_all(root)?;
        fs::write(root.join("config.toml"), toml::to_string_pretty(self)?)?;
        Ok(())
    }
}

fn default_harness_command() -> String {
    "claude".into()
}

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Decode(toml::de::Error),
    Encode(toml::ser::Error),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Decode(error) => write!(f, "{error}"),
            Self::Encode(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<std::io::Error> for ConfigError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(value: toml::de::Error) -> Self {
        Self::Decode(value)
    }
}

impl From<toml::ser::Error> for ConfigError {
    fn from(value: toml::ser::Error) -> Self {
        Self::Encode(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_plain_default_configuration() {
        let directory = tempfile::tempdir().unwrap();
        let config = Config::load_or_create(directory.path()).unwrap();
        assert_eq!(config.harness.command, "claude");
        assert!(directory.path().join("config.toml").is_file());
    }

    #[test]
    fn persists_a_selected_harness_without_losing_other_settings() {
        let directory = tempfile::tempdir().unwrap();
        let mut config = Config::default();
        config.harness.command = "/usr/local/bin/codex".into();
        config.save(directory.path()).unwrap();

        let loaded = Config::load_or_create(directory.path()).unwrap();
        assert_eq!(loaded.harness.command, "/usr/local/bin/codex");
    }

    #[test]
    fn recording_load_does_not_create_configuration() {
        let directory = tempfile::tempdir().unwrap();
        let config = Config::load_or_default(directory.path()).unwrap();
        assert_eq!(config.harness.command, "claude");
        assert!(!directory.path().join("config.toml").exists());
    }

    #[cfg(unix)]
    #[test]
    fn load_or_create_rejects_a_dangling_config_symlink_without_creating_its_target() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let outside = directory.path().join("outside.toml");
        symlink(&outside, directory.path().join("config.toml")).unwrap();

        assert!(Config::load_or_create(directory.path()).is_err());
        assert!(!outside.exists());
    }
}
