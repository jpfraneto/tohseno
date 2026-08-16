//! Private browser-to-local intention handoff (ADR 0011 compatibility).
//!
//! Relay records remain transport inputs. They are imported into the durable
//! pending-intention store and are only consumed after the shared application
//! service has admitted the exact creation command.

use crate::service_client::ServiceClient;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use reqwest::{redirect::Policy, Client, Response, Url};
use serde::Deserialize;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, Read};
use std::path::Path;
use std::time::Duration;
use tohseno_engine::{
    decrypt_intent_envelope, Event, EventBus, Ledger, PendingIntentionSource,
    PendingIntentionState, PendingIntentionStore, INTENT_ENVELOPE_AAD,
};
use tohseno_protocol::digest::sha256;
use zeroize::Zeroize;

const OFFICIAL_RELAY_ORIGIN: &str = "https://tohseno.com";
const MAX_CONTROL_RESPONSE: usize = 128 * 1024;
const MAX_CHUNK_BYTES: usize = 1024 * 1024;
const MAX_CIPHERTEXT_BYTES: usize = 64 * 1024 * 1024 + 32;
const MAX_PACKAGE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug)]
struct ClaimToken {
    relay_id: String,
    claim_capability: String,
    key: Vec<u8>,
}

impl Drop for ClaimToken {
    fn drop(&mut self) {
        self.claim_capability.zeroize();
        self.key.zeroize();
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LeaseResponse {
    schema: String,
    lease_capability: String,
    lease_expires_at: u64,
    ciphertext_bytes: usize,
    chunk_count: usize,
    ciphertext_sha256: String,
    nonce: String,
    associated_data: String,
    chunks: Vec<LeaseChunk>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LeaseChunk {
    index: usize,
    byte_length: usize,
    sha256: String,
}

pub fn read_claim_token_from_stdin() -> Result<String, Box<dyn std::error::Error>> {
    let stdin = io::stdin();
    let mut reader = stdin.lock().take(512);
    let mut token = String::new();
    reader.read_line(&mut token)?;
    if token.len() >= 511 {
        return Err("claim token from stdin is too long".into());
    }
    Ok(token.trim().to_owned())
}

pub async fn claim(
    mut token_text: String,
    no_open: bool,
    events: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    let token = parse_claim_token(&token_text)?;
    token_text.zeroize();
    let origin = relay_origin()?;
    let client = Client::builder()
        .redirect(Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .build()?;
    let response = client
        .post(endpoint(&origin, &token.relay_id, "claim")?)
        .bearer_auth(&token.claim_capability)
        .header("Content-Type", "application/json")
        .body("{}")
        .send()
        .await?;
    let lease_bytes = success_bytes(response, MAX_CONTROL_RESPONSE).await?;
    let mut lease: LeaseResponse = serde_json::from_slice(&lease_bytes)
        .map_err(|_| "relay returned a malformed claim lease")?;
    validate_lease(&lease)?;
    let ciphertext = match download_ciphertext(&client, &origin, &token.relay_id, &lease).await {
        Ok(bytes) => bytes,
        Err(error) => {
            let _ = release(&client, &origin, &token.relay_id, &lease.lease_capability).await;
            lease.lease_capability.zeroize();
            return Err(error);
        }
    };
    let nonce = URL_SAFE_NO_PAD
        .decode(&lease.nonce)
        .map_err(|_| "relay returned a malformed encryption nonce")?;
    let mut plaintext = match decrypt_intent_envelope(
        &ciphertext,
        &token.key,
        &nonce,
        lease.associated_data.as_bytes(),
    ) {
        Ok(bytes) => bytes,
        Err(error) => {
            let _ = release(&client, &origin, &token.relay_id, &lease.lease_capability).await;
            lease.lease_capability.zeroize();
            return Err(error.into());
        }
    };
    let ledger = Ledger::discover()?;
    ledger.initialize()?;
    let store = PendingIntentionStore::for_ledger(&ledger);
    let pending = match store.import_bytes(&plaintext, PendingIntentionSource::Relay) {
        Ok(pending) => pending,
        Err(error) => {
            let _ = release(&client, &origin, &token.relay_id, &lease.lease_capability).await;
            lease.lease_capability.zeroize();
            return Err(error.into());
        }
    };
    plaintext.zeroize();
    let mut completed = false;
    for _ in 0..3 {
        if complete(&client, &origin, &token.relay_id, &lease.lease_capability)
            .await
            .is_ok()
        {
            completed = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    lease.lease_capability.zeroize();
    if !completed {
        events.emit(Event::status(
            "The intention is safe on this Mac, but relay deletion was not acknowledged. The ciphertext remains eligible for expiry cleanup; rerunning the copied command is safe.",
        ));
    }
    events.emit(Event::result(
        "Encrypted intention imported safely on this Mac.",
    ));
    open_pending(&pending.id, pending.state, no_open, events).await
}

pub async fn open_package(
    path: &Path,
    no_open: bool,
    events: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = read_bounded_regular(path, MAX_PACKAGE_BYTES)?;
    let ledger = Ledger::discover()?;
    ledger.initialize()?;
    let pending = PendingIntentionStore::for_ledger(&ledger)
        .import_bytes(&bytes, PendingIntentionSource::PortableFile)?;
    events.emit(Event::result(
        "Private intent package imported safely on this Mac.",
    ));
    open_pending(&pending.id, pending.state, no_open, events).await
}

async fn open_pending(
    id: &str,
    state: PendingIntentionState,
    no_open: bool,
    events: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    if state == PendingIntentionState::Consumed {
        events.emit(Event::status(
            "This exact intention was already imported and consumed.",
        ));
        return Ok(());
    }
    let route = format!("/create?pending={id}");
    if no_open {
        events.emit(Event::handoff(format!(
            "Open it later with: tohseno studio, then visit {route}"
        )));
        return Ok(());
    }
    let service = ServiceClient::ensure_running()
        .await
        .map_err(|error| error.to_string())?;
    service
        .open_studio(&route)
        .map_err(|error| error.to_string())?;
    events.emit(Event::handoff(
        "Studio opened the exact pending intention. TAKE THE SHOT when ready.",
    ));
    Ok(())
}

fn read_bounded_regular(path: &Path, maximum: u64) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let before = fs::symlink_metadata(path)?;
    if before.file_type().is_symlink() || !before.is_file() || before.len() > maximum {
        return Err("private intent package must be a regular file no larger than 64 MiB".into());
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
    if !opened.is_file() || opened.len() != before.len() {
        return Err("private intent package changed while it was opened".into());
    }
    let mut bytes = Vec::with_capacity(usize::try_from(opened.len())?);
    file.take(maximum + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > maximum || bytes.len() as u64 != opened.len() {
        return Err("private intent package changed while it was read".into());
    }
    Ok(bytes)
}

fn parse_claim_token(value: &str) -> Result<ClaimToken, Box<dyn std::error::Error>> {
    let segments = value.split('.').collect::<Vec<_>>();
    if segments.len() != 4
        || segments[0] != "ti1"
        || !valid_segment(segments[1], 32)
        || !valid_segment(segments[2], 43)
        || !valid_segment(segments[3], 43)
    {
        return Err("claim token is malformed or uses an unsupported version".into());
    }
    let key = URL_SAFE_NO_PAD
        .decode(segments[3])
        .map_err(|_| "claim token is malformed")?;
    if key.len() != 32 {
        return Err("claim token is malformed".into());
    }
    Ok(ClaimToken {
        relay_id: segments[1].into(),
        claim_capability: segments[2].into(),
        key,
    })
}

fn valid_segment(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn relay_origin() -> Result<Url, Box<dyn std::error::Error>> {
    let configured = std::env::var("TOHSENO_INTENT_RELAY_ORIGIN").ok();
    match configured {
        None => Ok(Url::parse(OFFICIAL_RELAY_ORIGIN)?),
        Some(value) => parse_relay_override(&value, cfg!(debug_assertions)),
    }
}

fn parse_relay_override(value: &str, debug_build: bool) -> Result<Url, Box<dyn std::error::Error>> {
    if !debug_build {
        return Err("relay origin override is unavailable in release builds".into());
    }
    let parsed = Url::parse(value)?;
    let loopback = parsed
        .host_str()
        .is_some_and(|host| matches!(host, "127.0.0.1" | "localhost" | "[::1]" | "::1"));
    if !loopback
        || !matches!(parsed.scheme(), "http" | "https")
        || parsed.username() != ""
        || parsed.password().is_some()
        || parsed.path() != "/"
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err("development relay override must be a bare loopback HTTP(S) origin".into());
    }
    Ok(parsed)
}

fn endpoint(origin: &Url, relay_id: &str, action: &str) -> Result<Url, Box<dyn std::error::Error>> {
    if !valid_segment(relay_id, 32)
        || !action
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-'))
    {
        return Err("relay endpoint is malformed".into());
    }
    Ok(origin.join(&format!("/api/intent-relay/records/{relay_id}/{action}"))?)
}

fn validate_lease(lease: &LeaseResponse) -> Result<(), Box<dyn std::error::Error>> {
    if lease.schema != "tohseno.intent-relay-lease/1"
        || !valid_segment(&lease.lease_capability, 43)
        || lease.lease_expires_at == 0
        || lease.ciphertext_bytes < 17
        || lease.ciphertext_bytes > MAX_CIPHERTEXT_BYTES
        || lease.chunk_count == 0
        || lease.chunk_count > 65
        || lease.chunks.len() != lease.chunk_count
        || lease.associated_data.as_bytes() != INTENT_ENVELOPE_AAD
        || URL_SAFE_NO_PAD
            .decode(&lease.nonce)
            .map_or(true, |nonce| nonce.len() != 12)
        || !valid_hex_digest(&lease.ciphertext_sha256)
    {
        return Err("relay returned an invalid claim lease".into());
    }
    let mut total = 0_usize;
    for (index, chunk) in lease.chunks.iter().enumerate() {
        if chunk.index != index
            || chunk.byte_length == 0
            || chunk.byte_length > MAX_CHUNK_BYTES
            || !valid_hex_digest(&chunk.sha256)
        {
            return Err("relay returned invalid encrypted chunk metadata".into());
        }
        total = total
            .checked_add(chunk.byte_length)
            .ok_or("relay ciphertext length overflowed")?;
    }
    if total != lease.ciphertext_bytes {
        return Err("relay ciphertext length is inconsistent".into());
    }
    Ok(())
}

async fn download_ciphertext(
    client: &Client,
    origin: &Url,
    relay_id: &str,
    lease: &LeaseResponse,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut ciphertext = Vec::with_capacity(lease.ciphertext_bytes);
    for chunk in &lease.chunks {
        let response = client
            .get(endpoint(
                origin,
                relay_id,
                &format!("claim/chunks/{:06}", chunk.index),
            )?)
            .bearer_auth(&lease.lease_capability)
            .send()
            .await?;
        let bytes = success_bytes(response, MAX_CHUNK_BYTES).await?;
        let digest = sha256(&bytes).to_hex();
        if bytes.len() != chunk.byte_length || digest.trim_start_matches("0x") != chunk.sha256 {
            return Err("encrypted relay chunk failed integrity validation".into());
        }
        ciphertext.extend_from_slice(&bytes);
    }
    if ciphertext.len() != lease.ciphertext_bytes
        || sha256(&ciphertext).to_hex().trim_start_matches("0x") != lease.ciphertext_sha256
    {
        return Err("encrypted relay ciphertext failed integrity validation".into());
    }
    Ok(ciphertext)
}

async fn complete(
    client: &Client,
    origin: &Url,
    relay_id: &str,
    lease: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    control_post(client, endpoint(origin, relay_id, "complete")?, lease).await
}

async fn release(
    client: &Client,
    origin: &Url,
    relay_id: &str,
    lease: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    control_post(client, endpoint(origin, relay_id, "release")?, lease).await
}

async fn control_post(
    client: &Client,
    url: Url,
    capability: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let response = client
        .post(url)
        .bearer_auth(capability)
        .header("Content-Type", "application/json")
        .body("{}")
        .send()
        .await?;
    success_bytes(response, MAX_CONTROL_RESPONSE).await?;
    Ok(())
}

async fn success_bytes(
    mut response: Response,
    limit: usize,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    if !response.status().is_success() {
        return Err(format!("relay request failed with status {}", response.status()).into());
    }
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err("relay response exceeded its size limit".into());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if body.len() + chunk.len() > limit {
            return Err("relay response exceeded its size limit".into());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn valid_hex_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claim_token_is_strict_and_has_no_origin() {
        let value = format!(
            "ti1.{}.{}.{}",
            "A".repeat(32),
            "B".repeat(43),
            URL_SAFE_NO_PAD.encode([0x33_u8; 32])
        );
        assert!(parse_claim_token(&value).is_ok());
        assert!(parse_claim_token(&format!("ti2.{}", &value[4..])).is_err());
        assert!(parse_claim_token("ti1.https://evil.example.x.y").is_err());
        assert!(parse_claim_token(&format!(
            "ti1.{}.{}.{};x",
            "A".repeat(32),
            "B".repeat(43),
            "C".repeat(43)
        ))
        .is_err());
    }

    #[test]
    fn malformed_key_is_rejected_without_echoing_input() {
        let error = parse_claim_token(&format!(
            "ti1.{}.{}.{}",
            "A".repeat(32),
            "B".repeat(43),
            "_".repeat(43)
        ))
        .unwrap_err();
        assert!(!error.to_string().contains("ti1."));
    }

    #[test]
    fn relay_override_is_loopback_only_and_unavailable_to_release_builds() {
        assert_eq!(
            parse_relay_override("http://127.0.0.1:3999", true)
                .unwrap()
                .as_str(),
            "http://127.0.0.1:3999/"
        );
        assert!(parse_relay_override("https://relay.evil.example", true).is_err());
        assert!(parse_relay_override("http://localhost:3999/path", true).is_err());
        assert!(parse_relay_override("http://name:secret@localhost:3999", true).is_err());
        assert!(parse_relay_override("http://127.0.0.1:3999", false).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn portable_package_reader_rejects_symlinks() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.tip");
        fs::write(&source, b"private").unwrap();
        let linked = root.path().join("linked.tip");
        symlink(&source, &linked).unwrap();
        assert!(read_bounded_regular(&linked, MAX_PACKAGE_BYTES).is_err());
    }
}
