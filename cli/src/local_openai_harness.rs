//! Narrow, bundled adapter for an explicitly configured loopback
//! OpenAI-compatible model. It is not a general command runner: the model may
//! return bounded UTF-8 file replacements under the current app repository,
//! and every path is revalidated without a shell.

use reqwest::Url;
use serde::Deserialize;
use serde_json::json;
use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::managed_compute::{ManagedClient, ManagedReservationRequest};
use crate::workspace_identity::{KeychainSecretStore, SecretStore};

const MAX_CONTEXT_BYTES: usize = 4 * 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
const MAX_FILES: usize = 256;
const MAX_FILE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Deserialize)]
struct CompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Debug, Deserialize)]
struct Message {
    content: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FilePlan {
    files: Vec<FileReplacement>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileReplacement {
    path: String,
    content: String,
}

pub async fn run(
    base_url: &str,
    model: &str,
    privacy: &str,
    credential_reference: Option<&str>,
    instruction: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    validate_endpoint(base_url)?;
    validate_token(model, "model")?;
    if !matches!(privacy, "local" | "standard" | "zdr" | "private") {
        return Err("local model privacy mode is invalid".into());
    }
    if instruction.is_empty() || instruction.len() > 64 * 1024 {
        return Err("local model instruction is invalid".into());
    }
    let root = std::env::current_dir()?.canonicalize()?;
    let context = source_context(&root)?;
    let endpoint = completion_endpoint(base_url)?;
    let body = implementation_body(model, privacy, instruction, &context);
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15 * 60))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let mut request = client
        .post(endpoint)
        .header("accept", "application/json")
        .header("content-type", "application/json")
        .json(&body);
    let credential = credential_reference
        .map(
            |reference| -> Result<Zeroizing<String>, Box<dyn std::error::Error>> {
                validate_token(reference, "credential reference")?;
                let bytes = KeychainSecretStore.get(reference).map_err(|_| {
                    "local endpoint credential is unavailable in the macOS Keychain"
                })?;
                let value =
                    String::from_utf8(bytes).map_err(|_| "local endpoint credential is invalid")?;
                if value.is_empty()
                    || value.len() > 16 * 1024
                    || value.chars().any(char::is_control)
                {
                    return Err("local endpoint credential is invalid".into());
                }
                Ok(Zeroizing::new(value))
            },
        )
        .transpose()?;
    if let Some(credential) = credential.as_deref() {
        request = request.bearer_auth(credential);
    }
    let response = request.send().await?;
    if !response.status().is_success() {
        return Err(format!("local model endpoint returned HTTP {}", response.status()).into());
    }
    let bytes = response.bytes().await?;
    if bytes.is_empty() || bytes.len() > MAX_RESPONSE_BYTES {
        return Err("local model response is empty or oversized".into());
    }
    apply_completion(&root, &bytes)?;
    Ok(())
}

pub struct ManagedRunRequest<'a> {
    pub proxy_origin: &'a str,
    pub reservation: ManagedReservationRequest<'a>,
    pub instruction: &'a str,
}

pub async fn run_managed(request: ManagedRunRequest<'_>) -> Result<(), Box<dyn std::error::Error>> {
    let reservation = &request.reservation;
    validate_token(reservation.model, "model")?;
    validate_token(reservation.command_id, "command identifier")?;
    validate_token(reservation.execution_id, "execution identifier")?;
    if !matches!(reservation.privacy, "standard" | "zdr" | "private") {
        return Err("managed privacy mode is invalid".into());
    }
    if reservation.maximum_microusd == 0 || reservation.maximum_microusd > 100_000_000 {
        return Err("managed maximum is invalid".into());
    }
    if reservation.pricing_snapshot_at.is_empty()
        || reservation.pricing_snapshot_at.len() > 64
        || reservation
            .pricing_snapshot_at
            .chars()
            .any(char::is_control)
        || reservation.input_microusd_per_million == 0
        || reservation.output_microusd_per_million == 0
    {
        return Err("managed pricing snapshot is invalid".into());
    }
    if request.instruction.is_empty() || request.instruction.len() > 64 * 1024 {
        return Err("managed model instruction is invalid".into());
    }
    let root = std::env::current_dir()?.canonicalize()?;
    let context = source_context(&root)?;
    let body = serde_json::to_vec(&implementation_body(
        reservation.model,
        reservation.privacy,
        request.instruction,
        &context,
    ))?;
    let client = ManagedClient::load_for_origin(request.proxy_origin)
        .map_err(|error| -> Box<dyn std::error::Error> { error.to_string().into() })?;
    // A repair is a distinct harness invocation. Re-reserving the same durable
    // command reuses its existing hold and issues a fresh one-use capability.
    let admitted = client
        .reserve(reservation)
        .await
        .map_err(|error| -> Box<dyn std::error::Error> { error.to_string().into() })?;
    let bytes = client
        .completion(&admitted.capability, &body)
        .await
        .map_err(|error| -> Box<dyn std::error::Error> { error.to_string().into() })?;
    apply_completion(&root, &bytes)?;
    Ok(())
}

fn implementation_body(
    model: &str,
    privacy: &str,
    instruction: &str,
    context: &str,
) -> serde_json::Value {
    json!({
        "model": model,
        "privacy": privacy,
        "stream": false,
        "temperature": 0,
        "max_tokens": 12000,
        "messages": [
            {
                "role": "system",
                "content": "You are the bounded implementation adapter inside TOHSENO. Return only strict JSON with exactly one key, files. files is an array of {path,content} UTF-8 replacements relative to the repository. Do not use markdown fences, delete files, write .git, or write private .tohseno data except .tohseno/state-transition-v1.json. Complete the supplied task in one pass; the factory independently builds and verifies it."
            },
            {
                "role": "user",
                "content": format!("INSTRUCTION\n{instruction}\n\nCURRENT REPOSITORY\n{context}")
            }
        ]
    })
}

fn apply_completion(root: &Path, bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let completion: CompletionResponse = serde_json::from_slice(bytes)?;
    if completion.choices.len() != 1 {
        return Err("local model response must contain exactly one choice".into());
    }
    let plan: FilePlan = serde_json::from_str(&completion.choices[0].message.content)
        .map_err(|_| "local model did not return the required strict file plan")?;
    apply_plan(root, plan)
}

fn completion_endpoint(base_url: &str) -> Result<Url, Box<dyn std::error::Error>> {
    let suffix = if base_url.ends_with("/v1") {
        "/chat/completions"
    } else {
        "/v1/chat/completions"
    };
    Ok(Url::parse(&format!("{base_url}{suffix}"))?)
}

fn validate_endpoint(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    if base_url.len() > 512 || base_url.contains(['?', '#', '@']) || base_url.ends_with('/') {
        return Err("local endpoint URL is invalid".into());
    }
    let url = Url::parse(base_url)?;
    if url.scheme() != "http"
        || !matches!(url.host_str(), Some("127.0.0.1" | "localhost"))
        || url.port().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("local endpoint must be explicit loopback HTTP with a port".into());
    }
    Ok(())
}

fn validate_token(value: &str, label: &str) -> Result<(), Box<dyn std::error::Error>> {
    if value.is_empty()
        || value.len() > 128
        || value
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return Err(format!("{label} is invalid").into());
    }
    Ok(())
}

fn source_context(root: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let mut paths = Vec::new();
    collect_paths(root, root, &mut paths)?;
    paths.sort();
    let mut context = String::new();
    for relative in paths.into_iter().take(512) {
        let path = root.join(&relative);
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.len() as usize > 512 * 1024 {
            continue;
        }
        let Ok(body) = fs::read_to_string(&path) else {
            continue;
        };
        let entry = format!("\n--- {} ---\n{}\n", relative.display(), body);
        if context.len().saturating_add(entry.len()) > MAX_CONTEXT_BYTES {
            break;
        }
        context.push_str(&entry);
    }
    if context.is_empty() {
        return Err("the app repository has no readable implementation context".into());
    }
    Ok(context)
}

fn collect_paths(
    root: &Path,
    directory: &Path,
    output: &mut Vec<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    if output.len() >= 512 {
        return Ok(());
    }
    let mut entries = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let relative = path.strip_prefix(root)?;
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        let first = relative
            .components()
            .next()
            .and_then(|component| match component {
                Component::Normal(value) => value.to_str(),
                _ => None,
            })
            .unwrap_or_default();
        if matches!(first, ".git" | ".build" | "build" | "DerivedData") {
            continue;
        }
        if first == ".tohseno" && !allowed_private_context(relative) {
            continue;
        }
        if metadata.is_dir() {
            collect_paths(root, &path, output)?;
        } else if metadata.is_file() && is_text_source(&path) {
            output.push(relative.to_path_buf());
        }
        if output.len() >= 512 {
            break;
        }
    }
    Ok(())
}

fn allowed_private_context(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(OsStr::to_str),
        Some("TASK.md" | "EVOLUTION_INTENT.md" | "APP_GENOME.md" | "state-transition-v1.json")
    )
}

fn is_text_source(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(OsStr::to_str)
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some(
            "swift"
                | "m"
                | "mm"
                | "h"
                | "plist"
                | "pbxproj"
                | "xcconfig"
                | "json"
                | "md"
                | "txt"
                | "yaml"
                | "yml"
                | "strings"
                | "entitlements"
        )
    )
}

fn apply_plan(root: &Path, plan: FilePlan) -> Result<(), Box<dyn std::error::Error>> {
    if plan.files.is_empty() || plan.files.len() > MAX_FILES {
        return Err("local model file plan has an invalid file count".into());
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut total = 0usize;
    for replacement in plan.files {
        if replacement.content.len() > MAX_FILE_BYTES {
            return Err("local model replacement is oversized".into());
        }
        total = total
            .checked_add(replacement.content.len())
            .ok_or("local model output overflow")?;
        if total > MAX_RESPONSE_BYTES {
            return Err("local model replacements are oversized".into());
        }
        let relative = safe_relative(&replacement.path)?;
        if !seen.insert(relative.clone()) {
            return Err("local model repeated one file path".into());
        }
        let destination = root.join(&relative);
        require_safe_ancestors(
            root,
            destination.parent().ok_or("replacement has no parent")?,
        )?;
        if let Ok(metadata) = fs::symlink_metadata(&destination) {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err("local model replacement target is unsafe".into());
            }
        }
        write_atomic(&destination, replacement.content.as_bytes())?;
    }
    Ok(())
}

fn safe_relative(value: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if value.is_empty() || value.len() > 240 || value.contains('\0') {
        return Err("local model path is invalid".into());
    }
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("local model path must be a plain relative path".into());
    }
    let first = path
        .components()
        .next()
        .and_then(|component| match component {
            Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .unwrap_or_default();
    if first == ".git"
        || (first == ".tohseno" && path != Path::new(".tohseno/state-transition-v1.json"))
    {
        return Err("local model path enters protected factory state".into());
    }
    Ok(path.to_path_buf())
}

fn require_safe_ancestors(root: &Path, parent: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let relative = parent.strip_prefix(root)?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err("replacement parent is invalid".into());
        };
        current.push(name);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err("replacement parent is unsafe".into())
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir(&current)?,
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let parent = path.parent().ok_or("replacement has no parent")?;
    let temporary = parent.join(format!(".tohseno-model-{}.tmp", Uuid::new_v4().simple()));
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let mut file = options.open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    if let Ok(metadata) = fs::metadata(path) {
        fs::set_permissions(&temporary, metadata.permissions())?;
    }
    fs::rename(&temporary, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_and_paths_fail_closed() {
        assert!(validate_endpoint("http://127.0.0.1:11434/v1").is_ok());
        for unsafe_url in [
            "https://127.0.0.1:11434",
            "http://example.com:80",
            "http://127.0.0.1.evil:80",
            "http://user@127.0.0.1:80",
        ] {
            assert!(validate_endpoint(unsafe_url).is_err());
        }
        assert!(safe_relative("Sources/App.swift").is_ok());
        assert!(safe_relative(".tohseno/state-transition-v1.json").is_ok());
        for unsafe_path in [
            "../secret",
            "/tmp/file",
            ".git/config",
            ".tohseno/identity.json",
        ] {
            assert!(safe_relative(unsafe_path).is_err());
        }
    }

    #[test]
    fn file_plan_never_invokes_a_shell_or_escapes_the_repository() {
        let directory = tempfile::tempdir().unwrap();
        apply_plan(
            directory.path(),
            FilePlan {
                files: vec![FileReplacement {
                    path: "Sources/App.swift".into(),
                    content: "import SwiftUI\n".into(),
                }],
            },
        )
        .unwrap();
        assert_eq!(
            fs::read_to_string(directory.path().join("Sources/App.swift")).unwrap(),
            "import SwiftUI\n"
        );
    }
}
