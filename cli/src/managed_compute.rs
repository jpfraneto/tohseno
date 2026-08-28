//! Installation-bound client and versioned cost estimator for the optional
//! TOHSENO-managed inference service. The durable workspace identity signs
//! narrowly scoped requests; neither provider nor Stripe secrets exist here.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use futures_util::StreamExt as _;
use rand_core::{OsRng, RngCore};
use reqwest::{Client, StatusCode, Url};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tohseno_companion::identity::WorkspaceServiceIdentity;

use crate::service_commands::ServicePaths;
use crate::workspace_identity::{KeychainSecretStore, WorkspaceIdentity};

const CLAIM_DOMAIN: &[u8] = b"tohseno.managed.claim.v1\0";
const INSTALLATION_DOMAIN: &[u8] = b"tohseno.managed.installation.v1\0";
const DEFAULT_ORIGIN: &str = "https://tohseno.com";
const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
const MAX_MANAGED_MICROUSD: u64 = 100_000_000;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Clone)]
pub struct ManagedClient {
    origin: Url,
    http: Client,
    identity: Arc<WorkspaceServiceIdentity>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedModel {
    pub model: String,
    pub input_microusd_per_million: u64,
    pub output_microusd_per_million: u64,
    pub privacy_tiers: Vec<String>,
    pub snapshot_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedCatalog {
    pub schema: String,
    pub models: Vec<ManagedModel>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ManagedBalance {
    pub schema: String,
    pub installation_binding: String,
    pub paid_microusd: i64,
    pub promotional_microusd: i64,
    pub reserved_microusd: i64,
    pub spendable_microusd: i64,
    pub currency: String,
    pub transactions: Vec<serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedCheckout {
    pub schema: String,
    pub checkout_url: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ManagedReservation {
    pub capability: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedEstimate {
    pub schema: String,
    pub estimator_version: String,
    pub model: String,
    pub privacy: String,
    pub pricing_snapshot_at: String,
    pub low_microusd: u64,
    pub high_microusd: u64,
    pub recommended_maximum_microusd: u64,
    pub expected_input_tokens_low: u64,
    pub expected_input_tokens_high: u64,
    pub expected_output_tokens_low: u64,
    pub expected_output_tokens_high: u64,
    pub invocation_limit: u8,
    pub provenance: Vec<String>,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct ManagedClaim {
    schema: &'static str,
    claim_id: String,
    installation_binding: String,
    signing_public_key_base64url: String,
    action: String,
    request_digest: String,
    issued_at: String,
    expires_at: String,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct ManagedClaimEnvelope {
    schema: &'static str,
    payload_base64url: String,
    signature_base64url: String,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct SignedRequest<'a, T> {
    claim: ManagedClaimEnvelope,
    request: &'a T,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct EmptyRequest {}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct CheckoutRequest<'a> {
    pack_id: &'a str,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedReservationRequest<'a> {
    pub command_id: &'a str,
    pub execution_id: &'a str,
    pub model: &'a str,
    pub privacy: &'a str,
    pub maximum_microusd: u64,
    pub pricing_snapshot_at: &'a str,
    pub input_microusd_per_million: u64,
    pub output_microusd_per_million: u64,
}

impl ManagedClient {
    pub fn new(identity: Arc<WorkspaceServiceIdentity>) -> Result<Self, BoxError> {
        Self::new_at(configured_origin()?, identity)
    }

    fn new_at(origin: Url, identity: Arc<WorkspaceServiceIdentity>) -> Result<Self, BoxError> {
        validate_origin(&origin)?;
        let http = Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            origin,
            http,
            identity,
        })
    }

    pub fn load() -> Result<Self, BoxError> {
        let paths = ServicePaths::discover().map_err(|error| error.to_string())?;
        let workspace =
            WorkspaceIdentity::load_or_create(&paths.service_state, &KeychainSecretStore)?;
        Self::new(workspace.identity)
    }

    pub fn load_for_origin(expected_origin: &str) -> Result<Self, BoxError> {
        let client = Self::load()?;
        if client.origin() != expected_origin.trim_end_matches('/') {
            return Err("managed execution origin differs from its durable admission".into());
        }
        Ok(client)
    }

    pub fn origin(&self) -> &str {
        self.origin.as_str().trim_end_matches('/')
    }

    pub fn installation_binding(&self) -> String {
        installation_binding(&self.identity.signing_public_key())
    }

    pub async fn balance(&self) -> Result<ManagedBalance, BoxError> {
        self.signed_post("balance", &EmptyRequest {}).await
    }

    pub async fn catalog(&self) -> Result<ManagedCatalog, BoxError> {
        self.signed_post("catalog", &EmptyRequest {}).await
    }

    pub async fn checkout(&self, pack_id: &str) -> Result<ManagedCheckout, BoxError> {
        validate_identifier(pack_id, "balance pack")?;
        self.signed_post("checkout", &CheckoutRequest { pack_id })
            .await
    }

    pub async fn reserve(
        &self,
        request: &ManagedReservationRequest<'_>,
    ) -> Result<ManagedReservation, BoxError> {
        validate_identifier(request.command_id, "command")?;
        validate_identifier(request.execution_id, "execution")?;
        validate_token(request.model, "model")?;
        validate_privacy(request.privacy)?;
        if request.maximum_microusd == 0 || request.maximum_microusd > MAX_MANAGED_MICROUSD {
            return Err("managed maximum is invalid".into());
        }
        if request.pricing_snapshot_at.is_empty()
            || request.pricing_snapshot_at.len() > 64
            || request.pricing_snapshot_at.chars().any(char::is_control)
            || request.input_microusd_per_million == 0
            || request.output_microusd_per_million == 0
        {
            return Err("managed pricing snapshot is invalid".into());
        }
        self.signed_post("reserve", request).await
    }

    pub async fn completion(&self, capability: &str, body: &[u8]) -> Result<Vec<u8>, BoxError> {
        if capability.len() != 43
            || !capability
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return Err("managed capability is invalid".into());
        }
        if body.is_empty() || body.len() > 8 * 1024 * 1024 {
            return Err("managed request is empty or oversized".into());
        }
        let url = self.endpoint("chat/completions")?;
        let response = self
            .http
            .post(url)
            .bearer_auth(capability)
            .header("accept", "application/json")
            .header("content-type", "application/json")
            .body(body.to_vec())
            .send()
            .await?;
        let status = response.status();
        let bytes = bounded_response(response, MAX_RESPONSE_BYTES).await?;
        if !status.is_success() {
            let message = managed_error_message(&bytes)
                .unwrap_or_else(|| format!("managed compute returned HTTP {status}"));
            return Err(message.into());
        }
        Ok(bytes)
    }

    async fn signed_post<T: Serialize, R: DeserializeOwned>(
        &self,
        action: &str,
        request: &T,
    ) -> Result<R, BoxError> {
        validate_identifier(action, "managed action")?;
        let request_bytes = tohseno_protocol::canonical::to_vec(request)?;
        let envelope = self.claim(action, &request_bytes)?;
        let body = tohseno_protocol::canonical::to_vec(&SignedRequest {
            claim: envelope,
            request,
        })?;
        let response = self
            .http
            .post(self.endpoint(action)?)
            .header("accept", "application/json")
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await?;
        let status = response.status();
        let bytes = bounded_response(response, MAX_RESPONSE_BYTES).await?;
        if !status.is_success() {
            let fallback = if status == StatusCode::PAYMENT_REQUIRED {
                "Managed creation balance is insufficient. Add balance or choose a local route."
                    .into()
            } else {
                format!("managed service returned HTTP {status}")
            };
            return Err(managed_error_message(&bytes).unwrap_or(fallback).into());
        }
        Ok(serde_json::from_slice(&bytes)?)
    }

    fn claim(
        &self,
        action: &str,
        canonical_request: &[u8],
    ) -> Result<ManagedClaimEnvelope, BoxError> {
        let now = OffsetDateTime::now_utc();
        let mut random = [0_u8; 18];
        OsRng.fill_bytes(&mut random);
        let public_key = self.identity.signing_public_key();
        let claim = ManagedClaim {
            schema: "tohseno.private-managed-claim/1",
            claim_id: format!("claim_{}", URL_SAFE_NO_PAD.encode(random)),
            installation_binding: installation_binding(&public_key),
            signing_public_key_base64url: URL_SAFE_NO_PAD.encode(public_key),
            action: action.into(),
            request_digest: URL_SAFE_NO_PAD.encode(Sha256::digest(canonical_request)),
            issued_at: now.format(&Rfc3339)?,
            expires_at: (now + time::Duration::minutes(2)).format(&Rfc3339)?,
        };
        let payload = tohseno_protocol::canonical::to_vec(&claim)?;
        let signature = self.identity.sign(CLAIM_DOMAIN, &payload);
        Ok(ManagedClaimEnvelope {
            schema: "tohseno.private-managed-claim-envelope/1",
            payload_base64url: URL_SAFE_NO_PAD.encode(payload),
            signature_base64url: URL_SAFE_NO_PAD.encode(signature),
        })
    }

    fn endpoint(&self, action: &str) -> Result<Url, BoxError> {
        Ok(self.origin.join(&format!("/api/managed/v1/{action}"))?)
    }
}

pub fn estimate(
    model: &ManagedModel,
    privacy: &str,
    intention_bytes: u64,
    reference_bytes: u64,
    source_context_bytes: u64,
) -> Result<ManagedEstimate, BoxError> {
    validate_privacy(privacy)?;
    if !model.privacy_tiers.iter().any(|tier| tier == privacy) {
        return Err("the selected model does not advertise that privacy tier".into());
    }
    let admitted_bytes = intention_bytes
        .checked_add(reference_bytes)
        .and_then(|value| value.checked_add(source_context_bytes))
        .ok_or("managed estimate input is too large")?;
    if intention_bytes == 0
        || intention_bytes > 4 * 1024 * 1024
        || reference_bytes > 160 * 1024 * 1024
        || source_context_bytes > 512 * 1024 * 1024
    {
        return Err("managed estimate input is outside factory bounds".into());
    }
    // UTF-8 source averages fewer bytes per token than prose. Use a deliberately
    // conservative 3-byte divisor plus fixed factory instructions, then bound
    // one implementation and at most one repair as ADR 0019 requires.
    let context_tokens = admitted_bytes.saturating_add(2) / 3;
    let input_low = context_tokens.saturating_add(8_000);
    let input_high = context_tokens.saturating_add(16_000).saturating_mul(2);
    let output_low = 4_000;
    let output_high = 24_000;
    let low = priced(input_low, output_low, model);
    let high = priced(input_high, output_high, model).max(low);
    Ok(ManagedEstimate {
        schema: "tohseno.managed-estimate/1".into(),
        estimator_version: "managed-cost-v1".into(),
        model: model.model.clone(),
        privacy: privacy.into(),
        pricing_snapshot_at: model.snapshot_at.clone(),
        low_microusd: low,
        high_microusd: high,
        recommended_maximum_microusd: high,
        expected_input_tokens_low: input_low,
        expected_input_tokens_high: input_high,
        expected_output_tokens_low: output_low,
        expected_output_tokens_high: output_high,
        invocation_limit: 2,
        provenance: vec![
            "exact intention byte length".into(),
            "exact reference-image byte length".into(),
            "bounded existing-source byte length for evolution".into(),
            "server model price snapshot plus 20% TOHSENO retail margin".into(),
            "one implementation invocation and at most one repair".into(),
        ],
    })
}

pub fn bounded_source_bytes(root: &Path) -> Result<u64, BoxError> {
    fn visit(root: &Path, directory: &Path, total: &mut u64) -> Result<(), BoxError> {
        let mut entries = std::fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let relative = path.strip_prefix(root)?;
            let metadata = std::fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                continue;
            }
            let first = relative
                .components()
                .next()
                .and_then(|part| part.as_os_str().to_str());
            if matches!(
                first,
                Some(".git" | ".tohseno" | ".build" | "build" | "DerivedData")
            ) {
                continue;
            }
            if metadata.is_dir() {
                visit(root, &path, total)?;
            } else if metadata.is_file() {
                *total = total.saturating_add(metadata.len());
                if *total > 512 * 1024 * 1024 {
                    return Err("existing app source exceeds the managed estimate bound".into());
                }
            }
        }
        Ok(())
    }
    let canonical = root.canonicalize()?;
    let mut total = 0;
    visit(&canonical, &canonical, &mut total)?;
    Ok(total)
}

fn priced(input: u64, output: u64, model: &ManagedModel) -> u64 {
    input
        .saturating_mul(model.input_microusd_per_million)
        .saturating_add(output.saturating_mul(model.output_microusd_per_million))
        .saturating_add(999_999)
        / 1_000_000
}

fn configured_origin() -> Result<Url, BoxError> {
    #[cfg(debug_assertions)]
    if let Ok(value) = std::env::var("TOHSENO_MANAGED_ORIGIN") {
        return Ok(Url::parse(&value)?);
    }
    Ok(Url::parse(DEFAULT_ORIGIN)?)
}

fn validate_origin(origin: &Url) -> Result<(), BoxError> {
    let base_shape = origin.path() == "/"
        && origin.query().is_none()
        && origin.fragment().is_none()
        && origin.username().is_empty()
        && origin.password().is_none();
    let production = origin.as_str() == "https://tohseno.com/";
    #[cfg(debug_assertions)]
    let development = base_shape
        && origin.scheme() == "http"
        && matches!(origin.host_str(), Some("127.0.0.1" | "localhost"))
        && origin.port().is_some();
    #[cfg(not(debug_assertions))]
    let development = false;
    if !base_shape || (!production && !development) {
        return Err("managed service origin is not an approved release origin".into());
    }
    Ok(())
}

fn installation_binding(public_key: &[u8; 32]) -> String {
    let mut digest = Sha256::new();
    digest.update(INSTALLATION_DOMAIN);
    digest.update(public_key);
    URL_SAFE_NO_PAD.encode(digest.finalize())
}

fn validate_identifier(value: &str, label: &str) -> Result<(), BoxError> {
    if value.is_empty()
        || value.len() > 160
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(format!("{label} identifier is invalid").into());
    }
    Ok(())
}

fn validate_token(value: &str, label: &str) -> Result<(), BoxError> {
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

fn validate_privacy(value: &str) -> Result<(), BoxError> {
    if !matches!(value, "standard" | "zdr" | "private") {
        return Err("managed privacy tier is invalid".into());
    }
    Ok(())
}

async fn bounded_response(
    response: reqwest::Response,
    maximum: usize,
) -> Result<Vec<u8>, BoxError> {
    if response
        .content_length()
        .is_some_and(|length| length > maximum as u64)
    {
        return Err("managed service response is oversized".into());
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if bytes.len().saturating_add(chunk.len()) > maximum {
            return Err("managed service response is oversized".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn managed_error_message(bytes: &[u8]) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    value
        .get("error")
        .and_then(|error| {
            error
                .as_str()
                .or_else(|| error.get("message").and_then(serde_json::Value::as_str))
        })
        .filter(|message| !message.is_empty() && message.len() <= 1_000)
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installation_binding_is_domain_separated_and_opaque() {
        let identity = WorkspaceServiceIdentity::from_secret_keys([7; 32], [8; 32]).unwrap();
        let binding = installation_binding(&identity.signing_public_key());
        assert_eq!(binding.len(), 43);
        assert_ne!(binding, identity.signing_public_key_base64url());
    }

    #[test]
    fn estimate_is_versioned_bounded_and_includes_repair() {
        let model = ManagedModel {
            model: "fixture".into(),
            input_microusd_per_million: 120_000,
            output_microusd_per_million: 360_000,
            privacy_tiers: vec!["standard".into()],
            snapshot_at: "2026-08-27T00:00:00Z".into(),
        };
        let value = estimate(&model, "standard", 300, 900, 30_000).unwrap();
        assert_eq!(value.estimator_version, "managed-cost-v1");
        assert_eq!(value.invocation_limit, 2);
        assert!(value.high_microusd >= value.low_microusd);
        assert_eq!(value.recommended_maximum_microusd, value.high_microusd);
    }

    #[test]
    fn estimate_rejects_an_unadvertised_privacy_tier() {
        let model = ManagedModel {
            model: "fixture".into(),
            input_microusd_per_million: 1,
            output_microusd_per_million: 1,
            privacy_tiers: vec!["standard".into()],
            snapshot_at: "2026-08-27T00:00:00Z".into(),
        };
        assert!(estimate(&model, "private", 1, 0, 0).is_err());
    }
}
