use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub harness: HarnessConfig,
    #[serde(default = "default_max_repair_passes")]
    pub max_repair_passes: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HarnessConfig {
    #[serde(default = "default_harness_command")]
    pub command: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            harness: HarnessConfig::default(),
            max_repair_passes: default_max_repair_passes(),
        }
    }
}

impl Default for HarnessConfig {
    fn default() -> Self {
        Self {
            command: default_harness_command(),
        }
    }
}

impl Config {
    pub fn load_or_create(root: &Path) -> Result<Self, ConfigError> {
        fs::create_dir_all(root)?;
        let path = root.join("config.toml");
        if !path.exists() {
            let config = Self::default();
            fs::write(&path, toml::to_string_pretty(&config)?)?;
            return Ok(config);
        }
        Ok(toml::from_str(&fs::read_to_string(path)?)?)
    }
}

fn default_harness_command() -> String {
    "claude".into()
}

fn default_max_repair_passes() -> u8 {
    8
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
        assert_eq!(config.max_repair_passes, 8);
        assert!(directory.path().join("config.toml").is_file());
    }
}
