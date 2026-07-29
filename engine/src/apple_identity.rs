//! Narrow process bridge to the bundled Apple-native DeviceKey helper.
//!
//! The helper is resolved relative to the running TOHSENO executable (or by
//! one explicit override used by tests and source builds). It is never found
//! through `PATH`, so an unrelated executable cannot become signing authority.

use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tohseno_protocol::digest::Bytes32;
use tohseno_protocol::signature::{P256PublicKey, P256Signature};

const SCHEMA: &str = "tohseno.apple-identity/1";

#[derive(Clone, Debug)]
pub struct AppleIdentityBridge {
    executable: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AppleDeviceIdentity {
    pub tag: String,
    pub backend: String,
    pub test_only: bool,
    pub security_level: String,
    #[serde(rename = "key_id")]
    pub helper_key_id: String,
    pub public_key: P256PublicKey,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppleDeviceSignature {
    pub identity: AppleDeviceIdentity,
    pub algorithm: String,
    pub digest: Bytes32,
    pub signature: P256Signature,
    pub low_s: bool,
}

#[derive(Debug)]
pub enum AppleIdentityError {
    Io(std::io::Error),
    HelperMissing,
    HelperFailure { code: String, message: String },
    InvalidResponse(String),
}

impl std::fmt::Display for AppleIdentityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::HelperMissing => write!(
                formatter,
                "the bundled tohseno-apple-identity helper is missing"
            ),
            Self::HelperFailure { code, message } => {
                write!(
                    formatter,
                    "Apple identity helper failed ({code}): {message}"
                )
            }
            Self::InvalidResponse(message) => {
                write!(
                    formatter,
                    "Apple identity helper returned invalid JSON: {message}"
                )
            }
        }
    }
}

impl std::error::Error for AppleIdentityError {}

impl From<std::io::Error> for AppleIdentityError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl AppleIdentityBridge {
    pub fn discover() -> Result<Self, AppleIdentityError> {
        if let Some(explicit) = std::env::var_os("TOHSENO_APPLE_IDENTITY_HELPER") {
            return Self::at(PathBuf::from(explicit));
        }

        let current = std::env::current_exe()?;
        let sibling = current
            .parent()
            .ok_or(AppleIdentityError::HelperMissing)?
            .join("tohseno-apple-identity");
        if executable_file(&sibling) {
            return Ok(Self {
                executable: sibling,
            });
        }

        // Source-checkout fallback. Release artifacts always take the sibling
        // path above, preserving the relative-discovery security boundary.
        if cfg!(debug_assertions) {
            let package = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../apple-identity/.build/debug/tohseno-apple-identity");
            if executable_file(&package) {
                return Ok(Self {
                    executable: package,
                });
            }
        }
        Err(AppleIdentityError::HelperMissing)
    }

    pub fn at(executable: impl Into<PathBuf>) -> Result<Self, AppleIdentityError> {
        let executable = executable.into();
        if !executable.is_absolute() || !executable_file(&executable) {
            return Err(AppleIdentityError::HelperMissing);
        }
        Ok(Self { executable })
    }

    pub fn executable(&self) -> &Path {
        &self.executable
    }

    pub fn create(
        &self,
        tag: &str,
        software_test: bool,
    ) -> Result<AppleDeviceIdentity, AppleIdentityError> {
        let mut arguments = vec!["create", "--tag", tag];
        if software_test {
            arguments.extend(["--backend", "software-test"]);
        }
        self.invoke("create", &arguments)
    }

    pub fn public(&self, tag: &str) -> Result<AppleDeviceIdentity, AppleIdentityError> {
        self.invoke("public", &["public", "--tag", tag])
    }

    pub fn sign(
        &self,
        tag: &str,
        digest: Bytes32,
    ) -> Result<AppleDeviceSignature, AppleIdentityError> {
        self.invoke(
            "sign",
            &["sign", "--tag", tag, "--digest", &digest.to_string()],
        )
    }

    fn invoke<T: DeserializeOwned>(
        &self,
        expected_command: &str,
        arguments: &[&str],
    ) -> Result<T, AppleIdentityError> {
        let output = Command::new(&self.executable)
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;
        if !output.status.success() {
            let failure = serde_json::from_slice::<FailureEnvelope>(&output.stderr)
                .map_err(|error| AppleIdentityError::InvalidResponse(error.to_string()))?;
            if failure.schema != SCHEMA || failure.ok {
                return Err(AppleIdentityError::InvalidResponse(
                    "failure envelope schema or status is wrong".into(),
                ));
            }
            return Err(AppleIdentityError::HelperFailure {
                code: failure.error.code,
                message: failure.error.message,
            });
        }
        if !output.stderr.is_empty() {
            return Err(AppleIdentityError::InvalidResponse(
                "successful helper call wrote to stderr".into(),
            ));
        }
        let success = serde_json::from_slice::<SuccessEnvelope<T>>(&output.stdout)
            .map_err(|error| AppleIdentityError::InvalidResponse(error.to_string()))?;
        if success.schema != SCHEMA || !success.ok || success.command != expected_command {
            return Err(AppleIdentityError::InvalidResponse(
                "success envelope schema, status, or command is wrong".into(),
            ));
        }
        Ok(success.result)
    }
}

fn executable_file(path: &Path) -> bool {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return false;
    };
    metadata.is_file() && !metadata.file_type().is_symlink()
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SuccessEnvelope<T> {
    schema: String,
    ok: bool,
    command: String,
    result: T,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FailureEnvelope {
    schema: String,
    ok: bool,
    error: FailureDetail,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FailureDetail {
    code: String,
    message: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fn helper(script: &str) -> (tempfile::TempDir, AppleIdentityBridge) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("helper");
        fs::write(&path, script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        let bridge = AppleIdentityBridge::at(path).unwrap();
        (directory, bridge)
    }

    #[test]
    fn parses_exact_public_response() {
        let script = format!(
            "#!/bin/sh\nprintf '%s\\n' '{}'\n",
            r#"{"command":"public","ok":true,"result":{"backend":"software_test","key_id":"sha256:test","public_key":{"x":"0x6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296","y":"0x4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5"},"security_level":"software_test","tag":"test.tag","test_only":true},"schema":"tohseno.apple-identity/1"}"#
        );
        let (_directory, bridge) = helper(&script);
        let identity = bridge.public("test.tag").unwrap();
        assert!(identity.test_only);
        assert_eq!(identity.tag, "test.tag");
        identity.public_key.validate().unwrap();
    }

    #[test]
    fn surfaces_structured_helper_failure_without_fallback() {
        let script = "#!/bin/sh\nprintf '%s\\n' '{\"error\":{\"code\":\"identity_not_found\",\"message\":\"missing\"},\"ok\":false,\"schema\":\"tohseno.apple-identity/1\"}' >&2\nexit 1\n";
        let (_directory, bridge) = helper(script);
        assert!(matches!(
            bridge.public("test.tag"),
            Err(AppleIdentityError::HelperFailure { code, .. })
                if code == "identity_not_found"
        ));
    }

    #[test]
    fn rejects_relative_and_symlinked_helper_paths() {
        assert!(matches!(
            AppleIdentityBridge::at(PathBuf::from("helper")),
            Err(AppleIdentityError::HelperMissing)
        ));
        let directory = tempfile::tempdir().unwrap();
        let real = directory.path().join("real");
        let link = directory.path().join("link");
        fs::write(&real, "#!/bin/sh\n").unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();
        assert!(matches!(
            AppleIdentityBridge::at(link),
            Err(AppleIdentityError::HelperMissing)
        ));
    }
}
