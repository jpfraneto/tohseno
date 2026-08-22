//! Private hosted-checkout boundary for the Local Workspace Service.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use futures_util::StreamExt;
use reqwest::redirect::Policy;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};
use tohseno_application::{
    installation_binding, verify_receipt, EntitlementStore, SubscriptionPlan,
};
use tohseno_companion::identity::WorkspaceServiceIdentity;
use uuid::Uuid;

// WorkspaceServiceIdentity::sign appends the one domain separator byte. The
// server verifies the resulting `domain || 0x00 || payload` bytes directly.
const CLAIM_DOMAIN: &[u8] = b"tohseno.billing.checkout-claim.v1";
const BILLING_ORIGIN: &str = "https://tohseno.com";
const MAX_RESPONSE_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct CheckoutClaim {
    schema: &'static str,
    claim_id: String,
    installation_binding: String,
    signing_public_key_base64url: String,
    qualified_successful_days: u8,
    plan: SubscriptionPlan,
    issued_at: String,
    expires_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckoutClaimEnvelope {
    pub schema: String,
    pub payload_base64url: String,
    pub signature_base64url: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckoutSession {
    schema: String,
    checkout_url: String,
}

pub fn create_claim(
    workspace_id: &str,
    identity: &WorkspaceServiceIdentity,
    plan: SubscriptionPlan,
    now: OffsetDateTime,
) -> Result<CheckoutClaimEnvelope, Box<dyn std::error::Error + Send + Sync>> {
    let claim = CheckoutClaim {
        schema: "tohseno.private-checkout-claim/1",
        claim_id: format!("claim_{}", Uuid::new_v4().simple()),
        installation_binding: installation_binding(workspace_id)?,
        signing_public_key_base64url: identity.signing_public_key_base64url(),
        qualified_successful_days: 5,
        plan,
        issued_at: now.format(&Rfc3339)?,
        expires_at: (now + Duration::minutes(2)).format(&Rfc3339)?,
    };
    let payload = tohseno_protocol::canonical::to_vec(&claim)?;
    let signature = identity.sign(CLAIM_DOMAIN, &payload);
    Ok(CheckoutClaimEnvelope {
        schema: "tohseno.private-checkout-envelope/1".into(),
        payload_base64url: URL_SAFE_NO_PAD.encode(payload),
        signature_base64url: URL_SAFE_NO_PAD.encode(signature),
    })
}

pub async fn begin_checkout(
    workspace_id: &str,
    identity: &WorkspaceServiceIdentity,
    plan: SubscriptionPlan,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let claim = create_claim(workspace_id, identity, plan, OffsetDateTime::now_utc())?;
    let response = client()?
        .post(endpoint("checkout")?)
        .json(&serde_json::json!({ "claim": claim }))
        .send()
        .await?;
    let bytes = bounded_response(response).await?;
    let session: CheckoutSession = serde_json::from_slice(&bytes)?;
    if session.schema != "tohseno.private-checkout-session/1" {
        return Err("billing server returned an unsupported checkout session".into());
    }
    let url = reqwest::Url::parse(&session.checkout_url)?;
    if url.scheme() != "https"
        || url.host_str() != Some("checkout.stripe.com")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.fragment().is_some()
    {
        return Err("billing server returned an untrusted checkout URL".into());
    }
    Ok(url.into())
}

pub async fn refresh_entitlement(
    workspace_id: &str,
    identity: &WorkspaceServiceIdentity,
    entitlement: &EntitlementStore,
    verification_key_path: &Path,
    receipt_path: &Path,
) -> Result<tohseno_application::EntitlementStatus, Box<dyn std::error::Error + Send + Sync>> {
    let claim = create_claim(
        workspace_id,
        identity,
        SubscriptionPlan::Monthly,
        OffsetDateTime::now_utc(),
    )?;
    let response = client()?
        .post(endpoint("refresh")?)
        .json(&serde_json::json!({ "claim": claim }))
        .send()
        .await?;
    let receipt = bounded_response(response).await?;
    let key = read_verification_key(verification_key_path)?;
    let now = OffsetDateTime::now_utc();
    let subscription = verify_receipt(&receipt, &key, workspace_id, now)?;
    let status = entitlement
        .install_verified_subscription(subscription, now, now.date())
        .map_err(Box::<dyn std::error::Error + Send + Sync>::from)?;
    persist_receipt(receipt_path, &receipt)?;
    Ok(status)
}

pub fn read_verification_key(
    path: &Path,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| "billing is not active: the receipt verification key is not installed")?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > 256 {
        return Err("billing receipt verification key is unsafe".into());
    }
    let key = fs::read_to_string(path)?;
    let key = key.trim();
    let bytes = URL_SAFE_NO_PAD
        .decode(key)
        .map_err(|_| "billing receipt verification key is invalid")?;
    if bytes.len() != 33 {
        return Err("billing receipt verification key is invalid".into());
    }
    Ok(key.into())
}

async fn bounded_response(
    response: reqwest::Response,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    if !response.status().is_success() {
        return Err(format!(
            "billing server refused the request with HTTP {}",
            response.status()
        )
        .into());
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err("billing server response is oversized".into());
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err("billing server response is oversized".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    if bytes.is_empty() {
        return Err("billing server response is empty".into());
    }
    Ok(bytes)
}

fn client() -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .redirect(Policy::none())
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(15))
        .build()
}

fn endpoint(path: &str) -> Result<reqwest::Url, Box<dyn std::error::Error + Send + Sync>> {
    #[cfg(debug_assertions)]
    if let Ok(origin) = std::env::var("TOHSENO_TEST_BILLING_ORIGIN") {
        return loopback_test_endpoint(&origin, path);
    }
    Ok(reqwest::Url::parse(&format!(
        "{BILLING_ORIGIN}/api/billing/v1/{path}"
    ))?)
}

#[cfg(debug_assertions)]
fn loopback_test_endpoint(
    origin: &str,
    path: &str,
) -> Result<reqwest::Url, Box<dyn std::error::Error + Send + Sync>> {
    let parsed = reqwest::Url::parse(origin)?;
    if parsed.scheme() == "http"
        && matches!(parsed.host_str(), Some("127.0.0.1" | "::1" | "[::1]"))
        && parsed.port().is_some()
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && parsed.path() == "/"
        && parsed.query().is_none()
        && parsed.fragment().is_none()
    {
        return Ok(parsed.join(&format!("api/billing/v1/{path}"))?);
    }
    Err("test billing origin must be an exact loopback HTTP origin with an explicit port".into())
}

fn persist_receipt(
    path: &Path,
    bytes: &[u8],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if bytes.is_empty() || bytes.len() > MAX_RESPONSE_BYTES {
        return Err("verified receipt is empty or oversized".into());
    }
    let parent = path.parent().ok_or("billing receipt path has no parent")?;
    fs::create_dir_all(parent)?;
    let stage = PathBuf::from(format!(
        "{}.{}.tmp",
        path.display(),
        Uuid::new_v4().simple()
    ));
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let mut file = options.open(&stage)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&stage, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claim_contains_no_workspace_or_app_content() {
        let identity = WorkspaceServiceIdentity::from_secret_keys([1; 32], [2; 32]).unwrap();
        let now = OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap();
        let envelope = create_claim(
            "workspace_fixture",
            &identity,
            SubscriptionPlan::Yearly,
            now,
        )
        .unwrap();
        let payload = URL_SAFE_NO_PAD.decode(envelope.payload_base64url).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&payload).unwrap();
        assert_eq!(value["plan"], "yearly");
        assert_eq!(
            value["installation_binding"],
            installation_binding("workspace_fixture").unwrap()
        );
        assert!(!String::from_utf8(payload)
            .unwrap()
            .contains("workspace_fixture"));
    }

    #[test]
    fn verification_key_must_be_a_regular_compressed_p256_key() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("key");
        fs::write(&path, format!("{}\n", URL_SAFE_NO_PAD.encode([2_u8; 33]))).unwrap();
        assert!(read_verification_key(&path).is_ok());
        fs::write(&path, "UNCONFIGURED\n").unwrap();
        assert!(read_verification_key(&path).is_err());
    }

    #[test]
    fn test_billing_origin_cannot_smuggle_a_remote_authority() {
        assert_eq!(
            loopback_test_endpoint("http://127.0.0.1:12345", "refresh")
                .unwrap()
                .as_str(),
            "http://127.0.0.1:12345/api/billing/v1/refresh"
        );
        assert!(loopback_test_endpoint("http://127.0.0.1:80@evil.example", "refresh").is_err());
        assert!(loopback_test_endpoint("https://127.0.0.1:12345", "refresh").is_err());
    }
}
