use crate::living_project::{
    AdoptionRequest, AdoptionResult, LivingProjectRecord, LivingProjectService, NetworkImportKind,
    NetworkProjectOrigin, ProjectPublication,
};
use crate::service_client::ServiceClient;
use crate::service_commands::ServicePaths;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tohseno_companion::publication::{
    ApprovedClaimEdition, BuilderDeviceAnnouncement, BuilderDeviceSignature,
    ClaimEditionApprovalContext, ClaimEditionPolicySummary, PublicationApprovalRequest,
    PUBLICATION_APPROVAL_REQUEST_SCHEMA,
};
use tohseno_engine::Event;
use tohseno_network::build_profile::{classify_xcode_project, collect_dependency_locks};
use tohseno_network::catalog::{
    BuildSafety, BuildSafetyClassification, CatalogDisplay, CatalogGeneration,
    CatalogParentRelease, CatalogRelease, ReleasePermissions, SourceArtifact, SourceArtifactFormat,
    XcodeBuildRecipe, XcodeContainerKind as CatalogContainerKind, CATALOG_RELEASE_SCHEMA,
};
use tohseno_network::evidence::PublicReleaseEvidence;
use tohseno_network::snapshot::create_deterministic_snapshot;
use tohseno_network::snapshot::extract_verified_snapshot;
use tohseno_protocol::actions::{
    keccak256, Eip712Domain, RegistryActionV2, SHOT_REGISTRY_DOMAIN,
    SHOT_REGISTRY_V2_EIP712_VERSION,
};
use tohseno_protocol::canonical;
use tohseno_protocol::digest::{Address20, Bytes32, ShotId};
use tohseno_protocol::identity::{initial_builder_account_salt, predict_builder_account};
use tohseno_protocol::public_checkpoint::{PublicCheckpoint, PublicCheckpointWitness};
use tohseno_protocol::record::CanonicalTimestamp;
use tohseno_protocol::signature::{verify_digest, P256PublicKey, P256Signature};
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

type BoxError = Box<dyn std::error::Error>;

const REGISTRY_ADDRESS: &str = "0x3fe6508ba2660bc575080024f402c192a2e035a0";
const FACTORY_ADDRESS: &str = "0xb1bd208cd2af98e701f43d06aaa889d3a594df65";
const REGISTRY_ORIGIN: &str = "https://tohseno.com";
const ACTIVATION_DIGEST: &str =
    "0x2b640260595def403343810d0dc4ee231e1faff427581be4f7b40cff4c189d28";
const BUILDER_ACCOUNT_CREATION_HEX: &str =
    include_str!("../../contracts/generations/0.8.0/bytecode/BuilderAccount.creation.hex");
const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;
const ADOPTION_REQUEST_TIMEOUT: Duration = Duration::from_secs(35 * 60);
const ADOPTION_PROGRESS_INTERVAL: Duration = Duration::from_secs(10);

fn parse_claim_edition(
    kind: Option<&str>,
    maximum: Option<u64>,
    closes_at: Option<&str>,
) -> Result<Option<RequestedClaimEdition>, BoxError> {
    if kind.is_none() && maximum.is_none() && closes_at.is_none() {
        return Ok(None);
    }
    let kind = kind.ok_or("Use --claim-edition whenever Claim Edition bounds are supplied")?;
    let maximum = match maximum {
        Some(0) => return Err("--max-claims must be at least one".into()),
        Some(value) if value > MAX_SAFE_JSON_INTEGER => {
            return Err("--max-claims exceeds the exact supported integer bound".into())
        }
        value => value,
    };
    let closes = closes_at
        .map(|value| {
            let parsed = OffsetDateTime::parse(value, &Rfc3339)
                .map_err(|_| "--closes-at must be one exact RFC 3339 timestamp")?;
            if parsed.nanosecond() != 0
                || parsed.unix_timestamp() <= OffsetDateTime::now_utc().unix_timestamp()
                || parsed.unix_timestamp() as u64 > MAX_SAFE_JSON_INTEGER
            {
                return Err("--closes-at must be a whole-second future timestamp within the supported bound");
            }
            Ok(parsed.unix_timestamp() as u64)
        })
        .transpose()?;
    let (kind, max_claims, closes_at) = match (kind, maximum, closes) {
        ("open", None, None) => ("open", 0, 0),
        ("limited", Some(maximum), None) => ("limited", maximum, 0),
        ("limited", Some(maximum), Some(closes)) => ("limited_timed", maximum, closes),
        ("timed", None, Some(closes)) => ("timed", 0, closes),
        ("open", _, _) => return Err("Open Edition cannot use --max-claims or --closes-at".into()),
        ("limited", None, _) => return Err("Limited Edition requires --max-claims".into()),
        ("timed", Some(_), _) => {
            return Err(
                "Timed Edition cannot use --max-claims; use limited with both bounds".into(),
            )
        }
        ("timed", None, None) => return Err("Timed Edition requires --closes-at".into()),
        _ => return Err("unsupported Claim Edition policy".into()),
    };
    Ok(Some(RequestedClaimEdition {
        kind: kind.into(),
        max_claims,
        closes_at,
    }))
}

fn publication_app_slug(
    requested: Option<&str>,
    prior: Option<&str>,
    display_name: &str,
    shot_id: &str,
) -> Result<String, BoxError> {
    if let Some(prior) = prior {
        validate_app_slug(prior)?;
        if requested.is_some_and(|value| value != prior) {
            return Err("This app already shipped. Its human app slug is stable.".into());
        }
        return Ok(prior.into());
    }
    if let Some(value) = requested {
        validate_app_slug(value)?;
        return Ok(value.into());
    }

    let mut slug = String::new();
    let mut separator = false;
    for character in display_name.chars() {
        if character.is_ascii_alphanumeric() {
            if separator && !slug.is_empty() && slug.len() < 64 {
                slug.push('-');
            }
            separator = false;
            if slug.len() < 64 {
                slug.push(character.to_ascii_lowercase());
            }
        } else if !slug.is_empty() {
            separator = true;
        }
        if slug.len() == 64 {
            break;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.len() < 2 {
        slug = format!("app-{}", &shot_id[..8]);
    }
    validate_app_slug(&slug)?;
    Ok(slug)
}

fn validate_app_slug(value: &str) -> Result<(), BoxError> {
    let valid = (2..=64).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !value.starts_with('-')
        && !value.ends_with('-')
        && !value.contains("--");
    if !valid {
        return Err("--app-slug must be 2–64 lowercase letters, numbers, or single hyphens".into());
    }
    if [
        "api", "claims", "docs", "download", "healthz", "install", "privacy", "registry",
        "releases", "s",
    ]
    .contains(&value)
    {
        return Err("--app-slug is reserved by the Tohseno website".into());
    }
    Ok(())
}

fn unix_time(value: u64) -> String {
    OffsetDateTime::from_unix_timestamp(value as i64)
        .ok()
        .and_then(|time| time.format(&Rfc3339).ok())
        .unwrap_or_else(|| value.to_string())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectList {
    schema: String,
    projects: Vec<LivingProjectRecord>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PublicationPreparation {
    schema: String,
    job_id: String,
    project_id: String,
    shot_id: String,
    checkpoint_sequence: u64,
    public_checkpoint: PublicCheckpoint,
    public_checkpoint_digest: Bytes32,
    source_artifact_sha256: Bytes32,
    source_artifact_byte_length: u64,
    source_tree_sha256: Bytes32,
    source_file_count: u64,
    source_uncompressed_byte_length: u64,
    source_artifact_path: String,
    build_safety: BuildSafety,
    registry_origin: String,
    chain_id: u64,
    builder_account_factory: String,
    shot_registry: String,
    status: String,
    created_at: String,
    publication_kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    claim_edition: Option<RequestedClaimEdition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    approval_request: Option<PublicationApprovalRequest>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestedClaimEdition {
    kind: String,
    max_claims: u64,
    closes_at: u64,
}

impl RequestedClaimEdition {
    fn human_label(&self) -> String {
        match (self.max_claims, self.closes_at) {
            (0, 0) => "Open Edition · one per Tohseno identity".into(),
            (maximum, 0) => format!("Limited Edition · first {maximum} Tohseno identities"),
            (0, closes) => format!("Until {} · one per Tohseno identity", unix_time(closes)),
            (maximum, closes) => {
                format!(
                    "Limited Edition · first {maximum} identities · until {}",
                    unix_time(closes)
                )
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredPublicationApproval {
    schema: String,
    job_id: String,
    catalog: BuilderDeviceSignature,
    registry: BuilderDeviceSignature,
    #[serde(default)]
    claim_edition: Option<ApprovedClaimEdition>,
    approved_at: String,
    author_device_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RemotePublicationState {
    schema: String,
    staging_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    upload_token: Option<String>,
    source_uploaded: bool,
    publication_submitted: bool,
    remote_status: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct StagingReceipt {
    staging_id: String,
    upload_token: String,
}

#[derive(Debug, Deserialize)]
struct RegistryPublicationStatus {
    status: String,
    public_release: Option<serde_json::Value>,
    failure: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct FriendRouteObservation {
    slug: String,
    url: String,
    status: FriendRouteStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum FriendRouteStatus {
    Live,
    AwaitingReview,
    Conflict,
    Unavailable,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GlobalAliasPointer {
    schema: String,
    alias: String,
    shot_id: String,
    builder_id: String,
    request_id: String,
    claim_digest: String,
    approved_at: String,
}

impl GlobalAliasPointer {
    fn validate(&self, expected_alias: &str, expected_shot: &str) -> Result<(), BoxError> {
        let builder_address = self
            .builder_id
            .strip_prefix("eip155:4663:0x")
            .ok_or("friend route has an invalid BuilderID")?;
        if self.schema != "tohseno.global-alias/1"
            || self.alias != expected_alias
            || self.shot_id != expected_shot
            || builder_address.len() != 40
            || !builder_address
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
            || OffsetDateTime::parse(&self.approved_at, &Rfc3339).is_err()
        {
            return Err("friend route does not bind the expected app".into());
        }
        Bytes32::from_hex("friend route request ID", &self.request_id)?;
        Bytes32::from_hex("friend route claim digest", &self.claim_digest)?;
        Bytes32::from_hex("friend route ShotID", &self.shot_id)?;
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CurrentPublicRelease {
    schema: String,
    release_digest: String,
    route: String,
    release: CurrentPublicReleaseIdentity,
    chain: serde_json::Value,
    manifest_url: String,
    source_url: String,
    icon_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CurrentPublicReleaseIdentity {
    shot_id: String,
    source: CurrentPublicSourceIdentity,
    display: CurrentPublicDisplayIdentity,
}

#[derive(Debug, Deserialize)]
struct CurrentPublicSourceIdentity {
    sha256: String,
}

#[derive(Debug, Deserialize)]
struct CurrentPublicDisplayIdentity {
    icon_sha256: Option<String>,
}

impl CurrentPublicRelease {
    fn validate(&self, expected_shot: &str, expected_release: &str) -> Result<(), BoxError> {
        let shot = Bytes32::from_hex("friend route current ShotID", expected_shot)?.to_string();
        let release =
            Bytes32::from_hex("friend route current release", expected_release)?.to_string();
        let source = Bytes32::from_hex("friend route current source", &self.release.source.sha256)?
            .to_string();
        let icon = self
            .release
            .display
            .icon_sha256
            .as_deref()
            .map(|value| Bytes32::from_hex("friend route current icon", value))
            .transpose()?
            .map(|value| value.to_string());
        if self.schema != "tohseno.public-catalog-release/1"
            || self.release_digest != release
            || self.release.shot_id != shot
            || self.route != format!("/s/{}", shot.trim_start_matches("0x"))
            || self.manifest_url != format!("/api/registry/v1/releases/{release}")
            || self.source_url != format!("/api/registry/v1/blobs/{source}")
            || !self.chain.is_object()
            || self.icon_url != icon.map(|digest| format!("/api/registry/v1/blobs/{digest}"))
        {
            return Err("friend route does not bind the expected exact release".into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiveKind {
    Install,
    Fork,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct NetworkReceiveJob {
    schema: String,
    command_id: String,
    action: ReceiveKind,
    shot_id: String,
    release_digest: String,
    author_device_id: String,
    status: String,
    attempts: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    failure: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkReceiveResult {
    pub schema: String,
    pub action: String,
    pub shot_id: String,
    pub release_digest: String,
    pub builder_id: String,
    pub source_path: String,
    pub project_id: String,
    pub candidate_shot_id: Option<String>,
    pub build_safety: BuildSafety,
    pub installation_status: String,
}

#[derive(Debug, Deserialize)]
struct PublicCatalogSummary {
    schema: String,
    release_digest: String,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: u64,
    #[serde(default)]
    result: Option<serde_json::Value>,
    #[serde(default)]
    error: Option<serde_json::Value>,
}

pub async fn init(
    path: PathBuf,
    scheme: Option<String>,
    json_output: bool,
    bus: &tohseno_engine::EventBus,
) -> Result<(), BoxError> {
    let selected = absolute_existing_path(&path)?;
    bus.emit(Event::status(
        "Checking this app with Xcode… The first check can take several minutes while Xcode resolves packages and builds for Simulator.",
    ));
    let service = ServiceClient::ensure_running().await.map_err(to_box)?;
    let request = AdoptionRequest {
        path: selected.display().to_string(),
        scheme,
        harness: None,
        model: None,
        network_origin: None,
    };
    let adoption =
        service.post_with_timeout("/api/v1/projects/adopt", &request, ADOPTION_REQUEST_TIMEOUT);
    tokio::pin!(adoption);
    let started = Instant::now();
    let mut progress = tokio::time::interval(ADOPTION_PROGRESS_INTERVAL);
    progress.tick().await;
    let result: AdoptionResult = loop {
        tokio::select! {
            result = &mut adoption => break result.map_err(to_box)?,
            _ = progress.tick() => {
                bus.emit(Event::status(format!(
                    "Xcode is still building the app for Simulator… {} seconds elapsed. Keep this Terminal open.",
                    started.elapsed().as_secs()
                )));
            }
        }
    };
    if result.status == "needs_scheme" {
        let choices = result.scheme_candidates.join(", ");
        return Err(format!("Choose the app scheme with --scheme. Available: {choices}").into());
    }
    let project = result
        .project
        .ok_or("Xcode project adoption returned no project")?;
    let shot_id = project
        .candidate_shot_id
        .as_deref()
        .ok_or("project adoption did not reserve a candidate ShotID")?;
    if json_output {
        println!(
            "{}",
            serde_json::to_string(&json!({
                "schema": "tohseno.network-project-initialized/1",
                "project": project,
                "candidate_shot_id": format!("0x{shot_id}"),
                "next": "tohseno deploy",
            }))?
        );
    } else {
        bus.emit(Event::result(format!(
            "{} is connected. Candidate Shot {}…{}\nReady. Next: tohseno deploy",
            project.display_name,
            &shot_id[..8],
            &shot_id[shot_id.len() - 8..]
        )));
    }
    Ok(())
}

pub async fn receive(
    selector: &str,
    exact_release: Option<&str>,
    destination: Option<PathBuf>,
    kind: ReceiveKind,
    mac_review_approved: bool,
    json_output: bool,
    bus: &tohseno_engine::EventBus,
) -> Result<(), BoxError> {
    let shot_id = parse_public_shot_selector(selector)?;
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(10 * 60))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let origin = registry_origin();
    bus.emit(Event::status("Resolving the immutable network release…"));
    let release_digest = match exact_release {
        Some(value) => Bytes32::from_hex("release digest", value)?.to_string(),
        None => {
            let response = client
                .get(format!("{origin}/api/registry/v1/shots/{shot_id}"))
                .send()
                .await?;
            let summary: PublicCatalogSummary = response_json(response, 2 * 1024 * 1024).await?;
            if summary.schema != "tohseno.public-catalog-release/1" {
                return Err("Registry returned an unsupported Shot summary".into());
            }
            Bytes32::from_hex("release digest", &summary.release_digest)?.to_string()
        }
    };
    let response = client
        .get(format!(
            "{origin}/api/registry/v1/releases/{release_digest}"
        ))
        .send()
        .await?;
    let evidence: PublicReleaseEvidence = response_json(response, 2 * 1024 * 1024).await?;
    evidence.verify_static()?;
    if evidence.signed_manifest.release.shot_id.to_string() != shot_id
        || evidence.release_digest.to_string() != release_digest
    {
        return Err("Registry release differs from the requested Shot and release".into());
    }
    if kind == ReceiveKind::Fork && !evidence.signed_manifest.release.permissions.fork_allowed {
        return Err("This Builder authorized installation, but did not authorize forks".into());
    }
    verify_active_release_on_chain(&client, &evidence).await?;
    bus.emit(Event::status(
        "Builder signature and Robinhood Chain receipt verified. Downloading source…",
    ));
    let cache = verified_source_artifact(&client, &origin, &evidence).await?;
    let target = receive_destination(destination, &evidence)?;
    let expected_source = &evidence.signed_manifest.release.source;
    let materialized = !target.exists();
    if !materialized {
        let metadata = fs::symlink_metadata(&target)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err("the existing network source destination is unsafe".into());
        }
        let artifact = tempfile::NamedTempFile::new()?;
        let observed = create_deterministic_snapshot(&target, artifact.path())?;
        if observed.artifact_sha256 != expected_source.sha256
            || observed.artifact_byte_length != expected_source.byte_length
            || observed.source_tree_sha256 != expected_source.source_tree_sha256
            || observed.file_count != expected_source.file_count
            || observed.source_byte_length != expected_source.uncompressed_byte_length
        {
            return Err(format!(
                "{} already exists and differs from the exact network release; choose a new folder with --into",
                target.display()
            )
            .into());
        }
    } else {
        fs::create_dir(&target)?;
        let extraction =
            match extract_verified_snapshot(&cache, &target, Some(expected_source.sha256)) {
                Ok(value) => value,
                Err(error) => {
                    let _ = fs::remove_dir_all(&target);
                    return Err(error.into());
                }
            };
        if extraction.file_count != expected_source.file_count
            || extraction.source_byte_length != expected_source.uncompressed_byte_length
            || tohseno_protocol::tree_hash::hash_source_tree(&target)?.digest
                != expected_source.source_tree_sha256
        {
            let _ = fs::remove_dir_all(&target);
            return Err("extracted source tree differs from the signed manifest".into());
        }
    }
    let container = target.join(&evidence.signed_manifest.release.build.container_path);
    let observed_safety = classify_xcode_project(&target, &container)?;
    if observed_safety != evidence.signed_manifest.release.build.safety {
        if materialized {
            let _ = fs::remove_dir_all(&target);
        }
        return Err("downloaded build behavior differs from the signed manifest".into());
    }
    if collect_dependency_locks(&target)? != evidence.signed_manifest.release.build.dependency_locks
    {
        if materialized {
            let _ = fs::remove_dir_all(&target);
        }
        return Err("downloaded dependency locks differ from the signed manifest".into());
    }
    if observed_safety.classification == BuildSafetyClassification::Unsupported {
        return Err(format!(
            "This app cannot use the personal-install path: {}",
            observed_safety.reasons.join("; ")
        )
        .into());
    }
    if observed_safety.classification == BuildSafetyClassification::RequiresMacReview
        && !mac_review_approved
    {
        return Err(format!(
            "Requires review on your Mac: {}. Review the visible source at {} and repeat with --approve-mac-review.",
            observed_safety.reasons.join("; "),
            target.display()
        )
        .into());
    }
    let service = ServiceClient::ensure_running().await.map_err(to_box)?;
    let imported_at = canonical_now()?;
    let adoption: AdoptionResult = service
        .post(
            "/api/v1/projects/adopt",
            &AdoptionRequest {
                path: container.display().to_string(),
                scheme: Some(evidence.signed_manifest.release.build.scheme.clone()),
                harness: None,
                model: None,
                network_origin: Some(NetworkProjectOrigin {
                    kind: match kind {
                        ReceiveKind::Install => NetworkImportKind::Install,
                        ReceiveKind::Fork => NetworkImportKind::Fork,
                    },
                    parent_shot_id: shot_id.clone(),
                    parent_release_digest: release_digest.clone(),
                    source_artifact_sha256: expected_source.sha256.to_string(),
                    builder_id: evidence.signed_manifest.release.builder_id.to_string(),
                    verified_at: imported_at,
                }),
            },
        )
        .await
        .map_err(to_box)?;
    let project = adoption
        .project
        .ok_or("the verified network source could not be connected to this Mac")?;
    let project = if kind == ReceiveKind::Install {
        let result: serde_json::Value = service
            .post(
                &format!("/api/v1/projects/{}/network-install", project.project_id),
                &json!({ "mac_review_approved": mac_review_approved }),
            )
            .await
            .map_err(to_box)?;
        serde_json::from_value::<LivingProjectRecord>(
            result
                .get("project")
                .cloned()
                .ok_or("network installation response has no project")?,
        )?
    } else {
        project
    };
    let installation_status = project
        .network_delivery
        .as_ref()
        .map(|delivery| delivery.status.clone())
        .unwrap_or_else(|| "verified_source".into());
    let result = NetworkReceiveResult {
        schema: "tohseno.network-receive-result/1".into(),
        action: match kind {
            ReceiveKind::Install => "install".into(),
            ReceiveKind::Fork => "fork".into(),
        },
        shot_id,
        release_digest,
        builder_id: evidence.signed_manifest.release.builder_id.to_string(),
        source_path: target.display().to_string(),
        project_id: project.project_id,
        candidate_shot_id: project.candidate_shot_id.map(|value| format!("0x{value}")),
        build_safety: observed_safety,
        installation_status,
    };
    if json_output {
        println!("{}", serde_json::to_string(&result)?);
    } else {
        let outcome = match (kind, result.installation_status.as_str()) {
            (ReceiveKind::Install, "installed") => "Installed on your iPhone.",
            (ReceiveKind::Install, _) => {
                "Ready for your iPhone. Make the paired phone reachable and unlock it to finish installation."
            }
            (ReceiveKind::Fork, _) => {
                "Fork ready. It has a new local Shot identity and can be evolved or shipped."
            }
        };
        bus.emit(Event::result(format!(
            "✓ Builder verified\n✓ Registry receipt verified\n✓ Source verified\n{outcome}\n{}",
            result.source_path
        )));
    }
    Ok(())
}

pub struct DeployOptions<'a> {
    pub dry_run: bool,
    pub project_id: Option<&'a str>,
    pub claim_edition: Option<&'a str>,
    pub max_claims: Option<u64>,
    pub closes_at: Option<&'a str>,
    pub app_slug: Option<&'a str>,
}

pub async fn deploy(
    options: DeployOptions<'_>,
    json_output: bool,
    bus: &tohseno_engine::EventBus,
) -> Result<(), BoxError> {
    let DeployOptions {
        dry_run,
        project_id,
        claim_edition,
        max_claims,
        closes_at,
        app_slug: requested_app_slug,
    } = options;
    let service = ServiceClient::ensure_running().await.map_err(to_box)?;
    let projects: ProjectList = service.get("/api/v1/projects").await.map_err(to_box)?;
    validate_project_list(&projects)?;
    let project = match project_id {
        Some(id) => projects
            .projects
            .iter()
            .find(|project| project.project_id == id)
            .ok_or("The selected project is not connected to this Mac")?,
        None => select_current_project(&projects.projects)?,
    };
    let is_ship = project.latest_publication.is_none();
    let claim_flags_supplied =
        claim_edition.is_some() || max_claims.is_some() || closes_at.is_some();
    if !is_ship && claim_flags_supplied {
        return Err("This app already shipped. Its Claim Edition is permanent.".into());
    }
    let requested_claim_edition = if is_ship {
        parse_claim_edition(claim_edition, max_claims, closes_at)?
    } else {
        None
    };
    let shot_text = project
        .candidate_shot_id
        .as_deref()
        .ok_or("Run 'tohseno init' first so this app has one stable candidate ShotID")?;
    let shot_bytes = Bytes32::from_hex("candidate_shot_id", &format!("0x{shot_text}"))?;
    let shot_id = ShotId::from_bytes(shot_bytes.into_bytes());
    let app_slug = publication_app_slug(
        requested_app_slug,
        project
            .latest_publication
            .as_ref()
            .and_then(|publication| publication.app_slug.as_deref()),
        &project.display_name,
        shot_text,
    )?;
    let next_sequence = project
        .latest_publication
        .as_ref()
        .map(|release| release.checkpoint_sequence + 1)
        .unwrap_or(1);
    let previous = project
        .latest_publication
        .as_ref()
        .map(|release| {
            Bytes32::from_hex(
                "public_checkpoint_digest",
                &release.public_checkpoint_digest,
            )
        })
        .transpose()?;
    let timestamp = canonical_now()?;
    let registry: Address20 = serde_json::from_str(&format!("\"{REGISTRY_ADDRESS}\""))?;
    let checkpoint = PublicCheckpoint::new(
        PublicCheckpointWitness {
            contract_generation: "0.8.0".into(),
            chain_id: 4663,
            registry,
        },
        shot_id,
        next_sequence,
        previous,
        CanonicalTimestamp::parse(timestamp.clone())?,
    )?;
    let checkpoint_digest = checkpoint.commitment()?;
    let safety = classify_xcode_project(
        Path::new(&project.source_path),
        Path::new(&project.container_path),
    )?;
    if matches!(
        safety.classification,
        BuildSafetyClassification::Unsupported
    ) {
        return Err(format!(
            "This Xcode target is outside the safe automatic build profile: {}",
            safety.reasons.join("; ")
        )
        .into());
    }
    bus.emit(Event::status(format!(
        "Preparing {} · checking source and build behavior…",
        project.display_name
    )));
    let job_id = format!("publication_{}", Uuid::new_v4().simple());
    let (directory, temporary_guard) = if dry_run {
        let temporary = tempfile::tempdir()?;
        (temporary.path().to_path_buf(), Some(temporary))
    } else {
        let paths = ServicePaths::discover()?;
        let directory = paths
            .service_state
            .join("network-publications-v1")
            .join(&job_id);
        ensure_private_directory(&directory)?;
        (directory, None)
    };
    let artifact = directory.join("source.tar");
    let snapshot = create_deterministic_snapshot(Path::new(&project.source_path), &artifact)?;
    let approval_request = if dry_run {
        None
    } else {
        Some(
            build_approval_request(
                &job_id,
                project,
                shot_id,
                next_sequence,
                previous,
                checkpoint_digest,
                &snapshot,
                &safety,
                &timestamp,
                &directory,
                requested_claim_edition.as_ref(),
                &app_slug,
            )
            .await?,
        )
    };
    let preparation = PublicationPreparation {
        schema: "tohseno.publication-preparation/1".into(),
        job_id: job_id.clone(),
        project_id: project.project_id.clone(),
        shot_id: shot_id.to_string(),
        checkpoint_sequence: next_sequence,
        public_checkpoint: checkpoint,
        public_checkpoint_digest: checkpoint_digest,
        source_artifact_sha256: snapshot.artifact_sha256,
        source_artifact_byte_length: snapshot.artifact_byte_length,
        source_tree_sha256: snapshot.source_tree_sha256,
        source_file_count: snapshot.file_count,
        source_uncompressed_byte_length: snapshot.source_byte_length,
        source_artifact_path: artifact.display().to_string(),
        build_safety: safety,
        registry_origin: REGISTRY_ORIGIN.into(),
        chain_id: 4663,
        builder_account_factory: FACTORY_ADDRESS.into(),
        shot_registry: REGISTRY_ADDRESS.into(),
        status: if dry_run {
            "dry_run_complete".into()
        } else {
            "waiting_for_companion".into()
        },
        created_at: timestamp,
        publication_kind: if is_ship { "ship" } else { "update" }.into(),
        claim_edition: requested_claim_edition.clone(),
        approval_request,
    };
    if !dry_run {
        write_new_private(
            &directory.join("preparation.json"),
            &serde_json::to_vec(&preparation)?,
        )?;
        let request = preparation
            .approval_request
            .as_ref()
            .ok_or("publication approval request was not prepared")?;
        write_new_private(
            &directory.join("approval-request.json"),
            &serde_json::to_vec(request)?,
        )?;
    }
    if json_output {
        println!("{}", serde_json::to_string(&preparation)?);
    } else {
        let classification = format!("{:?}", preparation.build_safety.classification)
            .to_lowercase()
            .replace('_', " ");
        bus.emit(Event::status(format!(
            "✓ Xcode project\n✓ Scheme: {}\n✓ Publication snapshot: {} files\n✓ Source: {} bytes\n✓ High-confidence secrets: none\n✓ Build profile: {}\n✓ Source digest: {}\n✓ Public checkpoint: {}",
            project.scheme,
            preparation.source_file_count,
            preparation.source_uncompressed_byte_length,
            classification,
            preparation.source_artifact_sha256,
            preparation.public_checkpoint_digest,
        )));
        if is_ship {
            bus.emit(Event::status(format!(
                "Claim edition:\n{}",
                requested_claim_edition
                    .as_ref()
                    .map(RequestedClaimEdition::human_label)
                    .unwrap_or_else(|| "Choose the immutable policy on your iPhone".into())
            )));
        }
        if dry_run {
            bus.emit(Event::result(
                "Dry run complete. Nothing was uploaded, signed, or published.",
            ));
        } else {
            bus.emit(Event::result(
                "Waiting for approval on your iPhone… The durable request will resume safely after a restart.",
            ));
        }
    }
    drop(temporary_guard);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn build_approval_request(
    job_id: &str,
    project: &LivingProjectRecord,
    shot_id: ShotId,
    checkpoint_sequence: u64,
    previous_checkpoint: Option<Bytes32>,
    checkpoint_digest: Bytes32,
    snapshot: &tohseno_network::snapshot::SnapshotReport,
    safety: &BuildSafety,
    issued_at: &str,
    directory: &Path,
    requested_claim_edition: Option<&RequestedClaimEdition>,
    app_slug: &str,
) -> Result<PublicationApprovalRequest, BoxError> {
    let builder_device = load_builder_device(directory)?;
    if builder_device.test_only {
        return Err("The paired Companion has a test-only Builder DeviceKey; production publication requires a physical iPhone Secure Enclave key".into());
    }
    let public_key = P256PublicKey {
        x: Bytes32::from_hex("Builder DeviceKey x", &builder_device.x)?,
        y: Bytes32::from_hex("Builder DeviceKey y", &builder_device.y)?,
    };
    public_key.validate()?;
    let account_salt = initial_builder_account_salt(&public_key)?;
    let factory: Address20 = serde_json::from_str(&format!("\"{FACTORY_ADDRESS}\""))?;
    let creation = decode_hex(BUILDER_ACCOUNT_CREATION_HEX.trim())?;
    let builder_id = predict_builder_account(factory, account_salt, &public_key, &creation)?;
    let mut release_id_bytes = [0_u8; 32];
    use rand_core::RngCore as _;
    rand_core::OsRng.fill_bytes(&mut release_id_bytes);
    let source_root = Path::new(&project.source_path);
    let container_path = Path::new(&project.container_path)
        .strip_prefix(source_root)
        .map_err(|_| "the Xcode container is outside the adopted source root")?
        .to_str()
        .ok_or("the Xcode container path is not UTF-8")?
        .replace('\\', "/");
    let registry: Address20 = serde_json::from_str(&format!("\"{REGISTRY_ADDRESS}\""))?;
    let release = CatalogRelease {
        schema: CATALOG_RELEASE_SCHEMA.into(),
        generation: CatalogGeneration {
            contract_generation: "0.8.0".into(),
            chain_id: 4663,
            builder_account_factory: factory,
            shot_registry: registry,
            activation_signing_digest: Bytes32::from_hex(
                "activation_signing_digest",
                ACTIVATION_DIGEST,
            )?,
        },
        shot_id,
        builder_id,
        release_id: Bytes32::new(release_id_bytes),
        published_at: CanonicalTimestamp::parse(issued_at.to_owned())?,
        display: CatalogDisplay {
            name: project.display_name.clone(),
            description: "A native iPhone app shared person to person with Tohseno.".into(),
            icon_sha256: None,
            builder_handle: None,
            app_slug: Some(app_slug.into()),
        },
        source: SourceArtifact {
            format: SourceArtifactFormat::DeterministicTar,
            sha256: snapshot.artifact_sha256,
            byte_length: snapshot.artifact_byte_length,
            source_tree_sha256: snapshot.source_tree_sha256,
            file_count: snapshot.file_count,
            uncompressed_byte_length: snapshot.source_byte_length,
        },
        build: XcodeBuildRecipe {
            container_kind: match project.container_kind {
                crate::living_project::XcodeContainerKind::Project => CatalogContainerKind::Project,
                crate::living_project::XcodeContainerKind::Workspace => {
                    CatalogContainerKind::Workspace
                }
            },
            container_path,
            scheme: project.scheme.clone(),
            original_bundle_identifier: project.bundle_identifier.clone(),
            minimum_ios: project
                .deployment_target
                .clone()
                .unwrap_or_else(|| "17.0".into()),
            device_families: vec!["iphone".into()],
            dependency_locks: snapshot.dependency_locks.clone(),
            safety: safety.clone(),
        },
        permissions: ReleasePermissions {
            install_allowed: true,
            fork_allowed: true,
            distributor_rights_declared: true,
            spdx_license: None,
        },
        parent: match project.network_origin.as_ref() {
            Some(origin) if origin.kind == NetworkImportKind::Fork => Some(CatalogParentRelease {
                parent_shot_id: serde_json::from_str(&format!("\"{}\"", origin.parent_shot_id))?,
                parent_release_digest: Bytes32::from_hex(
                    "parent_release_digest",
                    &origin.parent_release_digest,
                )?,
            }),
            _ => None,
        },
        checkpoint_sequence,
        public_checkpoint_digest: checkpoint_digest,
    };
    release.validate()?;
    let issued = OffsetDateTime::parse(issued_at, &Rfc3339)?;
    let expires = issued + time::Duration::hours(23);
    let expires_at = expires.format(&Rfc3339)?;
    let action_nonce = checkpoint_sequence - 1;
    let action = if checkpoint_sequence == 1 {
        let mut salt_bytes = [0_u8; 32];
        rand_core::OsRng.fill_bytes(&mut salt_bytes);
        RegistryActionV2::RegisterShot {
            shot_id,
            controller: builder_id.account(),
            head: checkpoint_digest,
            salt: Bytes32::new(salt_bytes),
            nonce: 0,
            deadline: expires.unix_timestamp() as u64,
        }
    } else {
        RegistryActionV2::AppendCheckpoint {
            shot_id,
            previous_head: previous_checkpoint
                .ok_or("updated publication has no previous public checkpoint")?,
            new_head: checkpoint_digest,
            checkpoint_sequence,
            nonce: action_nonce,
            deadline: expires.unix_timestamp() as u64,
        }
    };
    let domain = Eip712Domain {
        name: SHOT_REGISTRY_DOMAIN.into(),
        version: SHOT_REGISTRY_V2_EIP712_VERSION.into(),
        chain_id: 4663,
        verifying_contract: registry,
    };
    let catalog_json = String::from_utf8(canonical::to_vec(&release)?)?;
    let action_json = String::from_utf8(canonical::to_vec(&action)?)?;
    let claim_edition = if checkpoint_sequence == 1 {
        Some(
            prepare_claim_edition_approval(
                shot_id,
                builder_id.account(),
                expires.unix_timestamp() as u64,
                requested_claim_edition,
            )
            .await?,
        )
    } else {
        None
    };
    let request = PublicationApprovalRequest {
        schema: PUBLICATION_APPROVAL_REQUEST_SCHEMA.into(),
        job_id: job_id.into(),
        app_name: project.display_name.clone(),
        source_file_count: snapshot.file_count,
        source_byte_length: snapshot.source_byte_length,
        install_allowed: true,
        fork_allowed: true,
        requested_route: format!("/s/{}", shot_id.to_string().trim_start_matches("0x")),
        chain_id: 4663,
        builder_account_factory: FACTORY_ADDRESS.into(),
        shot_registry: REGISTRY_ADDRESS.into(),
        builder_id: builder_id.to_string(),
        builder_device,
        shot_id: shot_id.to_string(),
        checkpoint_sequence,
        action_nonce,
        action_deadline: expires.unix_timestamp() as u64,
        catalog_release_json: catalog_json,
        catalog_digest: release.digest()?.to_string(),
        registry_action_json: action_json,
        registry_digest: action.digest(&domain)?.to_string(),
        issued_at: issued_at.into(),
        expires_at,
        publication_kind: Some(
            if checkpoint_sequence == 1 {
                "ship"
            } else {
                "update"
            }
            .into(),
        ),
        claim_edition,
    };
    request.validate()?;
    Ok(request)
}

async fn prepare_claim_edition_approval(
    shot_id: ShotId,
    controller: Address20,
    deadline: u64,
    requested: Option<&RequestedClaimEdition>,
) -> Result<ClaimEditionApprovalContext, BoxError> {
    let activation = tohseno_engine::claims_activation::resolve_claims_contract()?;
    if activation.state != tohseno_engine::claims_activation::ClaimsContractState::Active {
        return Err(format!(
            "First Ship is waiting for the signed Claims activation: {}",
            activation.inactive_reason()
        )
        .into());
    }
    let claims = activation
        .claims_contract
        .ok_or("active Claims resolution omitted its contract")?;
    let active_registry = activation
        .shot_registry
        .ok_or("active Claims resolution omitted its Registry")?;
    let expected_runtime = activation
        .runtime_code_keccak256
        .ok_or("active Claims resolution omitted its runtime hash")?;
    let activation_digest = activation
        .activation_signing_digest
        .ok_or("active Claims resolution omitted its signing digest")?;
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let rpc = robinhood_rpc_origin();
    if rpc_string(&client, &rpc, "eth_chainId", json!([])).await? != "0x1237" {
        return Err("Claims approval RPC is not Robinhood Chain 4663".into());
    }
    let code = rpc_string(
        &client,
        &rpc,
        "eth_getCode",
        json!([claims.to_string(), "latest"]),
    )
    .await?;
    let code = decode_rpc_hex(&code)?;
    if code.is_empty() || keccak256(&code) != expected_runtime {
        return Err("live Claims runtime differs from the signed activation".into());
    }
    let registry = rpc_string(
        &client,
        &rpc,
        "eth_call",
        json!([{
            "to": claims.to_string(),
            "data": abi_call("shotRegistry()", &[]),
        }, "latest"]),
    )
    .await?;
    let registry = decode_rpc_hex(&registry)?;
    if registry.len() != 32
        || registry[..12].iter().any(|byte| *byte != 0)
        || registry[12..] != active_registry.as_bytes()[..]
    {
        return Err("live Claims immutable Registry differs from activation".into());
    }
    let mut controller_word = [0_u8; 32];
    controller_word[12..].copy_from_slice(controller.as_bytes());
    let nonce = rpc_string(
        &client,
        &rpc,
        "eth_call",
        json!([{
            "to": claims.to_string(),
            "data": abi_call("editionNonces(address)", &[&controller_word]),
        }, "latest"]),
    )
    .await?;
    let nonce = decode_abi_u64(&nonce, "Claim Edition nonce")?;
    let edition = rpc_string(
        &client,
        &rpc,
        "eth_call",
        json!([{
            "to": claims.to_string(),
            "data": abi_call("claimEdition(bytes32)", &[shot_id.bytes().as_bytes()]),
        }, "latest"]),
    )
    .await?;
    let edition = decode_rpc_hex(&edition)?;
    if edition.len() != 160 || edition[..32].iter().any(|byte| *byte != 0) {
        return Err(
            "This Shot already has a Claim Edition; first-Ship policy cannot change".into(),
        );
    }
    Ok(ClaimEditionApprovalContext {
        claims_contract: claims.to_string(),
        claims_activation_signing_digest: activation_digest.to_string(),
        controller: controller.to_string(),
        edition_nonce: nonce,
        action_deadline: deadline,
        requested_policy: requested.map(|policy| ClaimEditionPolicySummary {
            kind: policy.kind.clone(),
            max_claims: policy.max_claims,
            closes_at: policy.closes_at,
        }),
    })
}

fn decode_abi_u64(value: &str, label: &str) -> Result<u64, BoxError> {
    let bytes = decode_rpc_hex(value)?;
    if bytes.len() != 32 || bytes[..24].iter().any(|byte| *byte != 0) {
        return Err(format!("RPC returned an invalid {label}").into());
    }
    Ok(u64::from_be_bytes(bytes[24..].try_into()?))
}

fn load_builder_device(job_directory: &Path) -> Result<BuilderDeviceAnnouncement, BoxError> {
    let service_root = job_directory
        .parent()
        .and_then(Path::parent)
        .ok_or("publication job has no service root")?;
    let root = service_root.join("network-builder-devices-v1");
    let mut values = Vec::new();
    for entry in fs::read_dir(&root).map_err(|_| {
        "Open Tohseno on your paired iPhone once so its public Builder DeviceKey can reach this Mac"
    })?.take(16) {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > 64 * 1024 {
            return Err("the local Builder DeviceKey store contains an unsafe record".into());
        }
        let value: BuilderDeviceAnnouncement = serde_json::from_slice(&fs::read(entry.path())?)?;
        value.validate()?;
        values.push(value);
    }
    values.sort_by(|left, right| left.key_id.cmp(&right.key_id));
    values.dedup_by(|left, right| left.key_id == right.key_id);
    match values.as_slice() {
        [value] => Ok(value.clone()),
        [] => Err("Open Tohseno on your paired iPhone once so its public Builder DeviceKey can reach this Mac".into()),
        _ => Err("More than one Companion Builder identity is paired; choose one in the native Profile before publishing".into()),
    }
}

fn decode_hex(value: &str) -> Result<Vec<u8>, BoxError> {
    let body = value
        .strip_prefix("0x")
        .ok_or("embedded creation bytecode is not 0x-prefixed")?;
    if body.is_empty() || !body.len().is_multiple_of(2) {
        return Err("embedded creation bytecode has invalid length".into());
    }
    let mut bytes = Vec::with_capacity(body.len() / 2);
    for index in (0..body.len()).step_by(2) {
        bytes.push(u8::from_str_radix(&body[index..index + 2], 16)?);
    }
    Ok(bytes)
}

/// Advance every locally approved publication by at most one bounded remote
/// operation. The workspace service calls this on its existing durable
/// reconciliation clock, so relaunches resume without a second approval.
pub async fn resume_publications_once(
    service_root: &Path,
    projects: &LivingProjectService,
) -> Result<usize, BoxError> {
    let root = service_root.join("network-publications-v1");
    let entries = match fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error.into()),
    };
    let mut advanced = 0;
    for entry in entries.take(128) {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "publication store contains a non-UTF-8 job")?;
        if metadata.file_type().is_symlink()
            || !metadata.is_dir()
            || !name.starts_with("publication_")
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err("publication store contains an unsafe job".into());
        }
        if path.join("completed.json").exists()
            || path.join("failure.json").exists()
            || !path.join("approval.json").is_file()
        {
            continue;
        }
        match advance_publication(&path, projects).await {
            Ok(true) => advanced += 1,
            Ok(false) => {}
            Err(error) => {
                let message = error.to_string();
                if error.downcast_ref::<reqwest::Error>().is_some()
                    || message.contains("temporarily")
                    || message.contains("not enabled")
                    || message.contains("connection")
                    || message.contains("timed out")
                {
                    continue;
                }
                let failure = json!({
                    "schema": "tohseno.publication-failure/1",
                    "failed_at": canonical_now()?,
                    "reason": message.chars().take(300).collect::<String>(),
                });
                let failure_path = path.join("failure.json");
                if !failure_path.exists() {
                    write_new_private(&failure_path, &serde_json::to_vec(&failure)?)?;
                }
            }
        }
    }
    Ok(advanced)
}

pub fn enqueue_receive_request(
    service_root: &Path,
    command_id: &str,
    action: ReceiveKind,
    shot_id: &str,
    release_digest: &str,
    author_device_id: &str,
) -> Result<(), BoxError> {
    let shot_id = parse_public_shot_selector(shot_id)?;
    let release_digest = Bytes32::from_hex("release digest", release_digest)?.to_string();
    if command_id.is_empty()
        || command_id.len() > 160
        || !command_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err("network receive command ID is invalid".into());
    }
    let root = service_root.join("network-receive-jobs-v1");
    ensure_private_directory(&root)?;
    let path = root.join(format!("{command_id}.json"));
    let timestamp = canonical_now()?;
    let job = NetworkReceiveJob {
        schema: "tohseno.network-receive-job/1".into(),
        command_id: command_id.into(),
        action,
        shot_id,
        release_digest,
        author_device_id: author_device_id.into(),
        status: "queued".into(),
        attempts: 0,
        result: None,
        failure: None,
        created_at: timestamp.clone(),
        updated_at: timestamp,
    };
    let bytes = serde_json::to_vec(&job)?;
    if path.exists() {
        let existing: NetworkReceiveJob = read_private_json(&path, 1024 * 1024)?;
        if existing.command_id != job.command_id
            || existing.action != job.action
            || existing.shot_id != job.shot_id
            || existing.release_digest != job.release_digest
            || existing.author_device_id != job.author_device_id
        {
            return Err("network receive command idempotency conflict".into());
        }
        return Ok(());
    }
    write_new_private(&path, &bytes)
}

/// Resume at most one phone-requested network receive through the same public
/// CLI path used by a person at Terminal. The subprocess talks back to this
/// service over its authenticated loopback API; no alternate installer exists.
pub async fn resume_receive_requests_once(service_root: &Path) -> Result<usize, BoxError> {
    let root = service_root.join("network-receive-jobs-v1");
    let entries = match fs::read_dir(&root) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error.into()),
    };
    for entry in entries.take(128) {
        let path = entry?.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > 1024 * 1024
        {
            return Err("network receive job store contains an unsafe record".into());
        }
        let mut job: NetworkReceiveJob = read_private_json(&path, 1024 * 1024)?;
        if job.schema != "tohseno.network-receive-job/1"
            || !matches!(job.status.as_str(), "queued" | "processing")
            || job.attempts >= 8
        {
            continue;
        }
        job.status = "processing".into();
        job.attempts += 1;
        job.updated_at = canonical_now()?;
        write_replace_private(&path, &serde_json::to_vec(&job)?)?;
        let executable = std::env::current_exe()?;
        let output_path = root.join(format!("{}.stdout", job.command_id));
        let error_path = root.join(format!("{}.stderr", job.command_id));
        let output_file = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&output_path)?;
        let error_file = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&error_path)?;
        let status = tokio::process::Command::new(executable)
            .arg("--json")
            .arg(match job.action {
                ReceiveKind::Install => "install",
                ReceiveKind::Fork => "fork",
            })
            .arg(&job.shot_id)
            .args(["--release", &job.release_digest])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::from(output_file))
            .stderr(std::process::Stdio::from(error_file))
            .status()
            .await?;
        let output = read_bounded_file(&output_path, 4 * 1024 * 1024).unwrap_or_default();
        let errors = read_bounded_file(&error_path, 16 * 1024).unwrap_or_default();
        let _ = fs::remove_file(&output_path);
        let _ = fs::remove_file(&error_path);
        if status.success() {
            let result: serde_json::Value = serde_json::from_slice(&output)?;
            job.status = "completed".into();
            job.result = Some(result);
            job.failure = None;
        } else {
            let message = String::from_utf8_lossy(&errors);
            let message = message
                .trim()
                .strip_prefix("tohseno: ")
                .unwrap_or(message.trim());
            job.failure = Some(message.chars().take(300).collect());
            job.status = if job.attempts >= 8 {
                "failed"
            } else {
                "queued"
            }
            .into();
        }
        job.updated_at = canonical_now()?;
        write_replace_private(&path, &serde_json::to_vec(&job)?)?;
        return Ok(1);
    }
    Ok(0)
}

async fn advance_publication(
    directory: &Path,
    projects: &LivingProjectService,
) -> Result<bool, BoxError> {
    let preparation: PublicationPreparation =
        read_private_json(&directory.join("preparation.json"), 2 * 1024 * 1024)?;
    let request: PublicationApprovalRequest =
        read_private_json(&directory.join("approval-request.json"), 1024 * 1024)?;
    let approval: StoredPublicationApproval =
        read_private_json(&directory.join("approval.json"), 256 * 1024)?;
    request.validate()?;
    if preparation.job_id != request.job_id
        || approval.schema != "tohseno.publication-approval/1"
        || approval.job_id != request.job_id
        || approval.catalog.signer != request.builder_device
        || approval.registry.signer != request.builder_device
        || approval.catalog.digest != request.catalog_digest
        || approval.registry.digest != request.registry_digest
        || approval.author_device_id.is_empty()
        || validate_stored_claim_edition(&request, approval.claim_edition.as_ref()).is_err()
    {
        return Err("stored publication approval does not bind the exact durable request".into());
    }
    tohseno_companion::parse_timestamp(&approval.approved_at)?;
    let release: serde_json::Value = serde_json::from_str(&request.catalog_release_json)?;
    let action: serde_json::Value = serde_json::from_str(&request.registry_action_json)?;
    let signer = json!({ "x": approval.catalog.signer.x, "y": approval.catalog.signer.y });
    let catalog = json!({
        "schema": "tohseno.signed-catalog-release/1",
        "release": release,
        "signer": signer,
        "authorization": {
            "algorithm": approval.catalog.algorithm,
            "digest": approval.catalog.digest,
            "signature": { "r": approval.catalog.r, "s": approval.catalog.s },
            "low_s": approval.catalog.low_s,
        },
    });
    let registry = json!({
        "schema": "tohseno.registry-action/2",
        "domain": {
            "name": SHOT_REGISTRY_DOMAIN,
            "version": SHOT_REGISTRY_V2_EIP712_VERSION,
            "chain_id": 4663,
            "verifying_contract": REGISTRY_ADDRESS,
        },
        "action": action,
        "signer": { "x": approval.registry.signer.x, "y": approval.registry.signer.y },
        "authorization": {
            "algorithm": approval.registry.algorithm,
            "digest": approval.registry.digest,
            "signature": { "r": approval.registry.r, "s": approval.registry.s },
            "low_s": approval.registry.low_s,
        },
    });
    let state_path = directory.join("remote-state.json");
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let origin = registry_origin();
    let mut state = if state_path.exists() {
        read_private_json::<RemotePublicationState>(&state_path, 64 * 1024)?
    } else {
        let response = client
            .post(format!("{origin}/api/registry/v1/staging"))
            .json(&json!({ "envelope": catalog }))
            .send()
            .await?;
        let receipt: StagingReceipt = response_json(response, 64 * 1024).await?;
        let state = RemotePublicationState {
            schema: "tohseno.remote-publication-state/1".into(),
            staging_id: receipt.staging_id,
            upload_token: Some(receipt.upload_token),
            source_uploaded: false,
            publication_submitted: false,
            remote_status: "staged".into(),
            updated_at: canonical_now()?,
        };
        write_replace_private(&state_path, &serde_json::to_vec(&state)?)?;
        return Ok(true);
    };
    if state.schema != "tohseno.remote-publication-state/1"
        || state.staging_id.len() != 32
        || !state
            .staging_id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("local remote-publication state is invalid".into());
    }
    let token = state
        .upload_token
        .as_deref()
        .ok_or("local publication upload authorization is unavailable")?;
    if !state.source_uploaded {
        let artifact = PathBuf::from(&preparation.source_artifact_path);
        let expected_digest = preparation.source_artifact_sha256;
        let expected_length = preparation.source_artifact_byte_length;
        let artifact_for_check = artifact.clone();
        tokio::task::spawn_blocking(move || {
            verify_artifact(&artifact_for_check, expected_digest, expected_length)
                .map_err(|error| error.to_string())
        })
        .await?
        .map_err(|error| -> BoxError { error.into() })?;
        let file = tokio::fs::File::open(&artifact).await?;
        let response = client
            .put(format!(
                "{origin}/api/registry/v1/staging/{}/source",
                state.staging_id
            ))
            .bearer_auth(token)
            .header(reqwest::header::CONTENT_LENGTH, expected_length)
            .body(reqwest::Body::wrap_stream(ReaderStream::new(file)))
            .send()
            .await?;
        require_success(response, 64 * 1024).await?;
        state.source_uploaded = true;
        state.remote_status = "source_staged".into();
        state.updated_at = canonical_now()?;
        write_replace_private(&state_path, &serde_json::to_vec(&state)?)?;
        return Ok(true);
    }
    if !state.publication_submitted {
        let publication_body = if let Some(claim_edition) = approval.claim_edition.as_ref() {
            json!({ "registry": registry, "claim_edition": claim_edition })
        } else {
            json!({ "registry": registry })
        };
        let response = client
            .post(format!(
                "{origin}/api/registry/v1/staging/{}/publish",
                state.staging_id
            ))
            .bearer_auth(token)
            .json(&publication_body)
            .send()
            .await?;
        let remote: RegistryPublicationStatus = response_json(response, 256 * 1024).await?;
        state.publication_submitted = true;
        state.remote_status = remote.status;
        state.updated_at = canonical_now()?;
        write_replace_private(&state_path, &serde_json::to_vec(&state)?)?;
        return Ok(true);
    }
    let response = client
        .get(format!(
            "{origin}/api/registry/v1/publications/{}",
            state.staging_id
        ))
        .bearer_auth(token)
        .send()
        .await?;
    let remote: RegistryPublicationStatus = response_json(response, 512 * 1024).await?;
    state.remote_status = remote.status.clone();
    state.updated_at = canonical_now()?;
    if remote.status == "failed" {
        return Err(remote
            .failure
            .unwrap_or_else(|| "Registry publication failed".into())
            .into());
    }
    if remote.status != "complete" {
        write_replace_private(&state_path, &serde_json::to_vec(&state)?)?;
        return Ok(false);
    }
    let public = remote
        .public_release
        .as_ref()
        .and_then(serde_json::Value::as_object)
        .ok_or("complete Registry job has no public release")?;
    let release_digest = public
        .get("release_digest")
        .and_then(serde_json::Value::as_str)
        .ok_or("public release has no digest")?;
    let release_digest_value = Bytes32::from_hex("release digest", release_digest)?;
    let route = public
        .get("route")
        .and_then(serde_json::Value::as_str)
        .ok_or("public release has no route")?;
    let chain = public
        .get("chain")
        .and_then(serde_json::Value::as_object)
        .ok_or("public release has no chain receipt")?;
    let transaction_hash = chain
        .get("transactionHash")
        .and_then(serde_json::Value::as_str)
        .ok_or("public release has no Registry transaction hash")?;
    let response = client
        .get(format!(
            "{origin}/api/registry/v1/releases/{release_digest}"
        ))
        .send()
        .await?;
    let evidence: PublicReleaseEvidence = response_json(response, 2 * 1024 * 1024).await?;
    evidence.verify_static()?;
    let verified_release = &evidence.signed_manifest.release;
    let expected_route = format!("/s/{}", preparation.shot_id.trim_start_matches("0x"));
    if evidence.release_digest != release_digest_value
        || request.catalog_digest != release_digest
        || verified_release.shot_id.to_string() != preparation.shot_id
        || verified_release.public_checkpoint_digest != preparation.public_checkpoint_digest
        || verified_release.checkpoint_sequence != preparation.checkpoint_sequence
        || evidence.chain.transaction_hash.to_string() != transaction_hash
        || route != expected_route
    {
        return Err("public Registry evidence differs from the exact approved publication".into());
    }
    verify_active_release_on_chain(&client, &evidence).await?;
    let _verified_public_source = verified_source_artifact(&client, &origin, &evidence).await?;
    let public_url = format!("{origin}{route}");
    projects
        .record_publication(
            &preparation.project_id,
            ProjectPublication {
                release_digest: release_digest.into(),
                public_checkpoint_digest: preparation.public_checkpoint_digest.to_string(),
                checkpoint_sequence: preparation.checkpoint_sequence,
                status: "published".into(),
                public_url: Some(public_url.clone()),
                app_slug: verified_release.display.app_slug.clone(),
                transaction_hash: Some(transaction_hash.into()),
                updated_at: canonical_now()?,
            },
        )
        .map_err(|error| -> BoxError { error.to_string().into() })?;
    state.upload_token = None;
    state.remote_status = "complete".into();
    write_replace_private(&state_path, &serde_json::to_vec(&state)?)?;
    write_new_private(
        &directory.join("completed.json"),
        &serde_json::to_vec(&json!({
            "schema": "tohseno.publication-completed/1",
            "job_id": request.job_id,
            "release_digest": release_digest,
            "transaction_hash": transaction_hash,
            "public_url": public_url,
            "completed_at": canonical_now()?,
        }))?,
    )?;
    Ok(true)
}

fn validate_stored_claim_edition(
    request: &PublicationApprovalRequest,
    approved: Option<&ApprovedClaimEdition>,
) -> Result<(), BoxError> {
    let Some(context) = request.claim_edition.as_ref() else {
        return if approved.is_none() {
            Ok(())
        } else {
            Err("Update approval unexpectedly contains a Claim Edition".into())
        };
    };
    let approved = approved.ok_or("Ship approval omitted its Claim Edition")?;
    approved.validate()?;
    if approved.signature.signer != request.builder_device
        || approved.action.shot_registry != request.shot_registry
        || approved.action.shot_id != request.shot_id
        || approved.action.controller != context.controller
        || approved.action.nonce != context.edition_nonce
        || approved.action.deadline != context.action_deadline
        || context.requested_policy.as_ref().is_some_and(|required| {
            required.kind != approved.policy.kind
                || required.max_claims != approved.policy.max_claims
                || required.closes_at != approved.policy.closes_at
        })
    {
        return Err("stored Claim Edition approval differs from the exact Ship request".into());
    }
    let claims: Address20 = serde_json::from_str(&format!("\"{}\"", context.claims_contract))?;
    let registry: Address20 =
        serde_json::from_str(&format!("\"{}\"", approved.action.shot_registry))?;
    let shot_id: ShotId = serde_json::from_str(&format!("\"{}\"", approved.action.shot_id))?;
    let controller: Address20 =
        serde_json::from_str(&format!("\"{}\"", approved.action.controller))?;
    let action = tohseno_network::claims::OpenClaimEditionAction {
        shot_registry: registry,
        shot_id,
        max_claims: approved.action.max_claims,
        closes_at: approved.action.closes_at,
        controller,
        nonce: approved.action.nonce,
        deadline: approved.action.deadline,
    };
    let domain = Eip712Domain {
        name: tohseno_network::claims::CLAIMS_DOMAIN.into(),
        version: tohseno_network::claims::CLAIMS_EIP712_VERSION.into(),
        chain_id: 4663,
        verifying_contract: claims,
    };
    let digest = action.digest(&domain, registry)?;
    if digest.to_string() != approved.digest || approved.signature.digest != approved.digest {
        return Err("stored Claim Edition digest differs from its action".into());
    }
    let key = P256PublicKey {
        x: Bytes32::from_hex("Claim Edition signer x", &approved.signature.signer.x)?,
        y: Bytes32::from_hex("Claim Edition signer y", &approved.signature.signer.y)?,
    };
    let signature = P256Signature {
        r: Bytes32::from_hex("Claim Edition signature r", &approved.signature.r)?,
        s: Bytes32::from_hex("Claim Edition signature s", &approved.signature.s)?,
    };
    verify_digest(&key, digest, &signature)?;
    Ok(())
}

fn verify_artifact(path: &Path, expected: Bytes32, expected_length: u64) -> Result<(), BoxError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() != expected_length
    {
        return Err("publication artifact changed after approval".into());
    }
    use sha2::Digest as _;
    let mut file = fs::File::open(path)?;
    let mut hasher = sha2::Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    if Bytes32::new(hasher.finalize().into()) != expected {
        return Err("publication artifact digest changed after approval".into());
    }
    Ok(())
}

fn read_private_json<T: for<'de> Deserialize<'de>>(
    path: &Path,
    maximum: u64,
) -> Result<T, BoxError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > maximum {
        return Err("publication record is not a bounded regular file".into());
    }
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

fn read_bounded_file(path: &Path, maximum: u64) -> Result<Vec<u8>, BoxError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > maximum {
        return Err("network job output exceeded its bound".into());
    }
    Ok(fs::read(path)?)
}

fn write_replace_private(path: &Path, bytes: &[u8]) -> Result<(), BoxError> {
    ensure_private_directory(path.parent().ok_or("publication state has no parent")?)?;
    if fs::symlink_metadata(path)
        .is_ok_and(|metadata| metadata.file_type().is_symlink() || !metadata.is_file())
    {
        return Err("publication state target is unsafe".into());
    }
    let temporary = path.with_extension(format!("{}.tmp", Uuid::new_v4().simple()));
    write_new_private(&temporary, bytes)?;
    fs::rename(&temporary, path)?;
    Ok(())
}

async fn response_json<T: for<'de> Deserialize<'de>>(
    response: reqwest::Response,
    maximum: usize,
) -> Result<T, BoxError> {
    let status = response.status();
    if response
        .content_length()
        .is_some_and(|length| length > maximum as u64)
    {
        return Err("Registry response exceeded its bound".into());
    }
    let bytes = response.bytes().await?;
    if bytes.len() > maximum {
        return Err("Registry response exceeded its bound".into());
    }
    if !status.is_success() {
        let reason = serde_json::from_slice::<serde_json::Value>(&bytes)
            .ok()
            .and_then(|value| value.get("error")?.as_str().map(str::to_owned))
            .unwrap_or_else(|| format!("Registry returned {status}"));
        return Err(reason.into());
    }
    Ok(serde_json::from_slice(&bytes)?)
}

async fn require_success(response: reqwest::Response, maximum: usize) -> Result<(), BoxError> {
    let _: serde_json::Value = response_json(response, maximum).await?;
    Ok(())
}

fn registry_origin() -> String {
    if cfg!(debug_assertions) {
        if let Ok(value) = std::env::var("TOHSENO_TEST_REGISTRY_ORIGIN") {
            if value.starts_with("http://127.0.0.1:") || value.starts_with("http://[::1]:") {
                return value.trim_end_matches('/').into();
            }
        }
    }
    REGISTRY_ORIGIN.into()
}

fn parse_public_shot_selector(value: &str) -> Result<String, BoxError> {
    let candidate = if value.starts_with("https://") || value.starts_with("tohseno://") {
        let url = reqwest::Url::parse(value)?;
        match url.scheme() {
            "https"
                if url.host_str() == Some("tohseno.com")
                    && url.username().is_empty()
                    && url.password().is_none()
                    && url.query().is_none()
                    && url.fragment().is_none() =>
            {
                url.path()
                    .strip_prefix("/s/")
                    .ok_or("use a canonical Tohseno Shot link")?
                    .to_owned()
            }
            "tohseno"
                if matches!(url.host_str(), Some("install" | "fork"))
                    && url.query().is_none()
                    && url.fragment().is_none() =>
            {
                url.path().trim_start_matches('/').to_owned()
            }
            _ => return Err("use a canonical https://tohseno.com/s/<ShotID> link".into()),
        }
    } else {
        value.strip_prefix("0x").unwrap_or(value).to_owned()
    };
    if candidate.len() != 64
        || !candidate
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        || candidate.bytes().all(|byte| byte == b'0')
    {
        return Err("ShotID must be 32 lowercase nonzero hex bytes".into());
    }
    Ok(format!("0x{candidate}"))
}

async fn verify_active_release_on_chain(
    client: &reqwest::Client,
    evidence: &PublicReleaseEvidence,
) -> Result<(), BoxError> {
    use tohseno_engine::contract_generation::{
        resolve_current_contract_generation, ContractGenerationState,
    };
    let active = resolve_current_contract_generation()?;
    if active.state != ContractGenerationState::Active {
        return Err(active.inactive_reason().into());
    }
    let release = &evidence.signed_manifest.release;
    let expected_factory = active
        .definition
        .create2
        .builder_account_factory
        .predicted_address;
    let expected_registry = active.definition.create2.shot_registry.predicted_address;
    if release.generation.contract_generation != active.definition.generation
        || release.generation.chain_id != active.definition.chain.chain_id
        || release.generation.builder_account_factory != expected_factory
        || release.generation.shot_registry != expected_registry
        || Some(release.generation.activation_signing_digest) != active.signed_activation_head
    {
        return Err(
            "catalog release is not bound to this client's active signed generation".into(),
        );
    }
    let rpc = robinhood_rpc_origin();
    let chain_hex = rpc_string(client, &rpc, "eth_chainId", json!([])).await?;
    if parse_hex_u64(&chain_hex)? != active.definition.chain.chain_id {
        return Err("Robinhood RPC returned the wrong chain ID".into());
    }
    for (label, address, expected_hash) in [
        (
            "BuilderAccountFactory",
            expected_factory,
            active
                .definition
                .contracts
                .builder_account_factory
                .runtime_code_keccak256,
        ),
        (
            "ShotRegistry",
            expected_registry,
            active
                .definition
                .contracts
                .shot_registry
                .runtime_code_keccak256,
        ),
    ] {
        let code = rpc_string(
            client,
            &rpc,
            "eth_getCode",
            json!([address.to_string(), "latest"]),
        )
        .await?;
        let bytes = decode_rpc_hex(&code)?;
        if bytes.is_empty() || keccak256(&bytes) != expected_hash {
            return Err(format!("live {label} runtime differs from the signed activation").into());
        }
    }
    let builder = release.builder_id.account();
    let builder_code = rpc_string(
        client,
        &rpc,
        "eth_getCode",
        json!([builder.to_string(), "latest"]),
    )
    .await?;
    if decode_rpc_hex(&builder_code)?.is_empty() {
        return Err("the signed BuilderAccount is not deployed".into());
    }
    let signer_key_id = tohseno_protocol::identity::device_key_id(&evidence.signed_manifest.signer);
    let authorized = rpc_string(
        client,
        &rpc,
        "eth_call",
        json!([{
            "to": builder.to_string(),
            "data": abi_call("isAuthorizedKey(bytes32)", &[signer_key_id.as_bytes()]),
        }, "latest"]),
    )
    .await?;
    let authorized = decode_rpc_hex(&authorized)?;
    if authorized.len() != 32
        || authorized[..31].iter().any(|byte| *byte != 0)
        || authorized[31] != 1
    {
        return Err("the manifest signer is not a current BuilderAccount authority".into());
    }
    verify_registry_receipt(client, &rpc, evidence, expected_registry).await?;
    let shot = rpc_string(
        client,
        &rpc,
        "eth_call",
        json!([{
            "to": expected_registry.to_string(),
            "data": abi_call("getShot(bytes32)", &[release.shot_id.bytes().as_bytes()]),
        }, "latest"]),
    )
    .await?;
    let shot = decode_rpc_hex(&shot)?;
    if shot.len() != 128 {
        return Err("live ShotRegistry returned an invalid Shot record".into());
    }
    let controller = Address20::from_bytes(shot[12..32].try_into()?);
    let head = Bytes32::new(shot[32..64].try_into()?);
    let sequence = parse_abi_u64(&shot[64..96])?;
    if controller != release.builder_id.account() || sequence < release.checkpoint_sequence {
        return Err("live ShotRegistry state does not extend this Builder release".into());
    }
    if sequence == release.checkpoint_sequence && head != release.public_checkpoint_digest {
        return Err("live ShotRegistry head differs at the signed checkpoint sequence".into());
    }
    Ok(())
}

async fn verify_registry_receipt(
    client: &reqwest::Client,
    rpc: &str,
    evidence: &PublicReleaseEvidence,
    registry: Address20,
) -> Result<(), BoxError> {
    let receipt = rpc_value(
        client,
        rpc,
        "eth_getTransactionReceipt",
        json!([evidence.chain.transaction_hash.to_string()]),
    )
    .await?;
    let receipt = receipt
        .as_object()
        .ok_or("Registry receipt is unavailable")?;
    let registry_text = registry.to_string();
    let block_hash_text = evidence.chain.block_hash.to_string();
    if receipt.get("status").and_then(serde_json::Value::as_str) != Some("0x1")
        || receipt
            .get("to")
            .and_then(serde_json::Value::as_str)
            .map(str::to_lowercase)
            .as_deref()
            != Some(registry_text.as_str())
        || receipt.get("blockHash").and_then(serde_json::Value::as_str)
            != Some(block_hash_text.as_str())
    {
        return Err("Registry transaction receipt differs from catalog evidence".into());
    }
    let receipt_block = receipt
        .get("blockNumber")
        .and_then(serde_json::Value::as_str)
        .ok_or("Registry receipt has no block number")?;
    if parse_hex_u64(receipt_block)?.to_string() != evidence.chain.block_number {
        return Err("Registry receipt block differs from catalog evidence".into());
    }
    let registered =
        keccak256(b"ShotRegistered(bytes32,address,bytes32,bytes32,uint64,uint64,address)")
            .to_string();
    let appended =
        keccak256(b"CheckpointAppended(bytes32,bytes32,bytes32,uint64,uint64,address)").to_string();
    let shot_topic = evidence.signed_manifest.release.shot_id.to_string();
    let head_topic = evidence
        .signed_manifest
        .release
        .public_checkpoint_digest
        .to_string();
    let matched = receipt
        .get("logs")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|logs| {
            logs.iter().any(|log| {
                let Some(log) = log.as_object() else { return false };
                if log
                    .get("address")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_lowercase)
                    .as_deref()
                    != Some(registry_text.as_str())
                {
                    return false;
                }
                let Some(topics) = log.get("topics").and_then(serde_json::Value::as_array) else {
                    return false;
                };
                topics.len() == 4
                    && matches!(topics[0].as_str(), Some(value) if value == registered || value == appended)
                    && topics[1].as_str() == Some(shot_topic.as_str())
                    && topics[3].as_str() == Some(head_topic.as_str())
            })
        });
    if !matched {
        return Err("Registry receipt does not contain the signed Shot checkpoint".into());
    }
    Ok(())
}

async fn verified_source_artifact(
    client: &reqwest::Client,
    origin: &str,
    evidence: &PublicReleaseEvidence,
) -> Result<PathBuf, BoxError> {
    let expected = &evidence.signed_manifest.release.source;
    let paths = ServicePaths::discover()?;
    let root = paths.service_state.join("network-source-cache-v1");
    ensure_private_directory(&root)?;
    let destination = root.join(expected.sha256.to_string().trim_start_matches("0x"));
    if destination.exists() {
        verify_artifact(&destination, expected.sha256, expected.byte_length)?;
        return Ok(destination);
    }
    let temporary = root.join(format!(
        "{}.{}.partial",
        expected.sha256,
        Uuid::new_v4().simple()
    ));
    let response = client
        .get(format!("{origin}{}", evidence.source_url))
        .send()
        .await?;
    if response.status() != reqwest::StatusCode::OK
        || response
            .content_length()
            .is_some_and(|length| length != expected.byte_length)
    {
        return Err("source server did not return the exact declared artifact length".into());
    }
    let mut file = tokio::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .await?;
    let mut stream = response.bytes_stream();
    let mut hasher = sha2::Sha256::new();
    use sha2::Digest as _;
    let mut length = 0_u64;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        length = length
            .checked_add(chunk.len() as u64)
            .ok_or("source artifact length overflowed")?;
        if length > expected.byte_length {
            let _ = tokio::fs::remove_file(&temporary).await;
            return Err("source artifact exceeded its declared length".into());
        }
        hasher.update(&chunk);
        file.write_all(&chunk).await?;
    }
    file.flush().await?;
    file.sync_all().await?;
    drop(file);
    if length != expected.byte_length || Bytes32::new(hasher.finalize().into()) != expected.sha256 {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err("downloaded source artifact failed SHA-256 verification".into());
    }
    fs::rename(&temporary, &destination)?;
    Ok(destination)
}

fn receive_destination(
    requested: Option<PathBuf>,
    evidence: &PublicReleaseEvidence,
) -> Result<PathBuf, BoxError> {
    if let Some(path) = requested {
        let absolute = if path.is_absolute() {
            path
        } else {
            std::env::current_dir()?.join(path)
        };
        let parent = absolute.parent().ok_or("destination has no parent")?;
        let parent = parent.canonicalize()?;
        return Ok(parent.join(
            absolute
                .file_name()
                .ok_or("destination has no folder name")?,
        ));
    }
    let home = PathBuf::from(std::env::var_os("HOME").ok_or("the home folder is unavailable")?);
    let root = home.join("Developer").join("Tohseno");
    fs::create_dir_all(&root)?;
    let metadata = fs::symlink_metadata(&root)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("the Tohseno source destination is unsafe".into());
    }
    let mut slug = evidence
        .signed_manifest
        .release
        .display
        .name
        .to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    slug = slug.trim_matches('-').chars().take(48).collect();
    if slug.is_empty() {
        slug = "app".into();
    }
    let shot = evidence.signed_manifest.release.shot_id.to_string();
    let destination = root.join(format!("{slug}-{}", &shot[2..10]));
    Ok(destination)
}

async fn rpc_string(
    client: &reqwest::Client,
    rpc: &str,
    method: &str,
    params: serde_json::Value,
) -> Result<String, BoxError> {
    rpc_value(client, rpc, method, params)
        .await?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("{method} returned a non-string result").into())
}

async fn rpc_value(
    client: &reqwest::Client,
    rpc: &str,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, BoxError> {
    let response = client
        .post(rpc)
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }))
        .send()
        .await?;
    let body: JsonRpcResponse = response_json(response, 2 * 1024 * 1024).await?;
    if body.jsonrpc != "2.0" || body.id != 1 || body.error.is_some() {
        return Err(format!("Robinhood RPC rejected {method}").into());
    }
    body.result
        .ok_or_else(|| format!("Robinhood RPC omitted the {method} result").into())
}

fn robinhood_rpc_origin() -> String {
    if cfg!(debug_assertions) {
        if let Ok(value) = std::env::var("TOHSENO_TEST_ROBINHOOD_RPC") {
            if value.starts_with("http://127.0.0.1:") || value.starts_with("http://[::1]:") {
                return value;
            }
        }
    }
    "https://rpc.mainnet.chain.robinhood.com".into()
}

fn abi_call(signature: &str, words: &[&[u8]]) -> String {
    let selector = keccak256(signature.as_bytes());
    let mut bytes = Vec::with_capacity(4 + words.len() * 32);
    bytes.extend_from_slice(&selector.as_bytes()[..4]);
    for word in words {
        debug_assert_eq!(word.len(), 32);
        bytes.extend_from_slice(word);
    }
    format!("0x{}", encode_hex(&bytes))
}

fn decode_rpc_hex(value: &str) -> Result<Vec<u8>, BoxError> {
    let body = value
        .strip_prefix("0x")
        .ok_or("RPC hex is not 0x-prefixed")?;
    if !body.len().is_multiple_of(2)
        || !body
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f' | b'A'..=b'F'))
    {
        return Err("RPC returned malformed hex".into());
    }
    (0..body.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&body[index..index + 2], 16).map_err(Into::into))
        .collect()
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        value.push(HEX[(byte >> 4) as usize] as char);
        value.push(HEX[(byte & 0x0f) as usize] as char);
    }
    value
}

fn parse_hex_u64(value: &str) -> Result<u64, BoxError> {
    let body = value
        .strip_prefix("0x")
        .ok_or("RPC quantity is not 0x-prefixed")?;
    if body.is_empty() || body.len() > 16 {
        return Err("RPC quantity is outside the u64 bound".into());
    }
    Ok(u64::from_str_radix(body, 16)?)
}

fn parse_abi_u64(word: &[u8]) -> Result<u64, BoxError> {
    if word.len() != 32 || word[..24].iter().any(|byte| *byte != 0) {
        return Err("ABI uint64 word is malformed".into());
    }
    Ok(u64::from_be_bytes(word[24..].try_into()?))
}

pub async fn status(json_output: bool, bus: &tohseno_engine::EventBus) -> Result<(), BoxError> {
    let service = ServiceClient::ensure_running().await.map_err(to_box)?;
    let projects: ProjectList = service.get("/api/v1/projects").await.map_err(to_box)?;
    validate_project_list(&projects)?;
    let selected = select_current_project(&projects.projects).ok();
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(3))
        .timeout(std::time::Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let origin = registry_origin();
    let registry = client
        .get(format!("{origin}/api/registry/v1/status"))
        .send()
        .await
        .ok()
        .is_some_and(|response| response.status().is_success());
    let friend_route = match selected {
        Some(project) => observe_friend_route(&client, &origin, project).await,
        None => None,
    };
    let value = json!({
        "schema": "tohseno.network-status/1",
        "project": selected,
        "friend_route": friend_route.as_ref(),
        "local_service": "ready",
        "companion_required_for_publish": true,
        "registry_available": registry,
        "chain_id": 4663,
        "contract_generation": "0.8.0",
        "shot_registry": REGISTRY_ADDRESS,
    });
    if json_output {
        println!("{}", serde_json::to_string(&value)?);
    } else if let Some(project) = selected {
        let publication = project.latest_publication.as_ref().map(|published| {
            let canonical = published.public_url.as_deref().unwrap_or("unavailable");
            match published.app_slug.as_deref() {
                Some(slug) => {
                    let route = friend_route
                        .as_ref()
                        .map(friend_route_copy)
                        .unwrap_or_else(|| {
                            format!("Friend route: {origin}/{slug} · unavailable")
                        });
                    format!("Public release: {canonical}\nRelease slug: {slug}\n{route}")
                }
                None => format!(
                    "Public release: {canonical}\nRelease slug: not set\nNext: publish an Update with 'tohseno deploy --app-slug your-app' before requesting a friend route."
                ),
            }
        });
        bus.emit(Event::result(format!(
            "{}\nLocal Mac ✓\nCandidate Shot {}\nRegistry {}\n{}",
            project.display_name,
            project
                .candidate_shot_id
                .as_deref()
                .map(|id| format!("0x{id}"))
                .unwrap_or_else(|| "not initialized".into()),
            if registry { "✓" } else { "unavailable" },
            publication.unwrap_or_else(|| "Next: tohseno deploy --dry-run".into()),
        )));
    } else {
        bus.emit(Event::result(format!(
            "Local Mac ✓\nRegistry {}\nRun 'tohseno init' inside an Xcode app.",
            if registry { "✓" } else { "unavailable" }
        )));
    }
    Ok(())
}

async fn observe_friend_route(
    client: &reqwest::Client,
    origin: &str,
    project: &LivingProjectRecord,
) -> Option<FriendRouteObservation> {
    let publication = project.latest_publication.as_ref()?;
    let slug = publication.app_slug.as_deref()?;
    let Some(shot) = project.candidate_shot_id.as_deref() else {
        return Some(FriendRouteObservation {
            slug: slug.into(),
            url: format!("{origin}/{slug}"),
            status: FriendRouteStatus::Conflict,
        });
    };
    Some(observe_friend_route_exact(client, origin, slug, shot, &publication.release_digest).await)
}

async fn observe_friend_route_exact(
    client: &reqwest::Client,
    origin: &str,
    slug: &str,
    shot: &str,
    release: &str,
) -> FriendRouteObservation {
    let url = format!("{origin}/{slug}");
    let expected_shot = match Bytes32::from_hex("friend route ShotID", shot) {
        Ok(value) => value.to_string(),
        Err(_) => {
            return FriendRouteObservation {
                slug: slug.into(),
                url,
                status: FriendRouteStatus::Conflict,
            }
        }
    };
    let expected_release = match Bytes32::from_hex("friend route release", release) {
        Ok(value) => value.to_string(),
        Err(_) => {
            return FriendRouteObservation {
                slug: slug.into(),
                url,
                status: FriendRouteStatus::Conflict,
            }
        }
    };
    let response = match client
        .get(format!("{origin}/api/registry/v1/aliases/{slug}"))
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => {
            return FriendRouteObservation {
                slug: slug.into(),
                url,
                status: FriendRouteStatus::Unavailable,
            }
        }
    };
    let status = if response.status() == reqwest::StatusCode::NOT_FOUND {
        FriendRouteStatus::AwaitingReview
    } else if response.status().is_success() {
        match response_json::<GlobalAliasPointer>(response, 64 * 1024).await {
            Ok(pointer) if pointer.validate(slug, &expected_shot).is_ok() => {
                let current = client
                    .get(format!("{origin}/api/registry/v1/shots/{expected_shot}"))
                    .send()
                    .await;
                match current {
                    Ok(current) if current.status().is_success() => {
                        match response_json::<CurrentPublicRelease>(current, 256 * 1024).await {
                            Ok(current)
                                if current.validate(&expected_shot, &expected_release).is_ok() =>
                            {
                                match client.head(&url).send().await {
                                    Ok(page) if page.status().is_success() => {
                                        FriendRouteStatus::Live
                                    }
                                    Ok(page) if page.status() == reqwest::StatusCode::NOT_FOUND => {
                                        FriendRouteStatus::Conflict
                                    }
                                    _ => FriendRouteStatus::Unavailable,
                                }
                            }
                            _ => FriendRouteStatus::Conflict,
                        }
                    }
                    Ok(current) if current.status() == reqwest::StatusCode::NOT_FOUND => {
                        FriendRouteStatus::Conflict
                    }
                    _ => FriendRouteStatus::Unavailable,
                }
            }
            _ => FriendRouteStatus::Conflict,
        }
    } else {
        FriendRouteStatus::Unavailable
    };
    FriendRouteObservation {
        slug: slug.into(),
        url,
        status,
    }
}

fn friend_route_copy(route: &FriendRouteObservation) -> String {
    match route.status {
        FriendRouteStatus::Live => format!(
            "Friend route: {} ✓\nNext: send this exact link to your friend.",
            route.url
        ),
        FriendRouteStatus::AwaitingReview => format!(
            "Friend route: {} · awaiting review\nNext: In Companion open Profile → Global alias request and select this exact app.",
            route.url
        ),
        FriendRouteStatus::Conflict => format!(
            "Friend route conflict: {} does not resolve to this exact release. Do not send it.",
            route.url
        ),
        FriendRouteStatus::Unavailable => format!(
            "Friend route check unavailable: {}. Verify Registry health before sending it.",
            route.url
        ),
    }
}

fn validate_project_list(value: &ProjectList) -> Result<(), BoxError> {
    if value.schema != "tohseno.living-project-list/1" || value.projects.len() > 10_000 {
        return Err("local project list is invalid".into());
    }
    Ok(())
}

fn select_current_project(
    projects: &[LivingProjectRecord],
) -> Result<&LivingProjectRecord, BoxError> {
    let current = std::env::current_dir()?.canonicalize()?;
    let mut matches = projects
        .iter()
        .filter(|project| {
            Path::new(&project.source_path)
                .canonicalize()
                .is_ok_and(|source| current.starts_with(&source) || source.starts_with(&current))
        })
        .collect::<Vec<_>>();
    matches.sort_by_key(|project| std::cmp::Reverse(project.source_path.len()));
    match matches.as_slice() {
        [project, ..] => Ok(project),
        [] if projects.len() == 1 => Ok(&projects[0]),
        [] => {
            Err("Run this command inside an initialized Xcode app, or run 'tohseno init'.".into())
        }
    }
}

fn absolute_existing_path(path: &Path) -> Result<PathBuf, BoxError> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let metadata = fs::symlink_metadata(&absolute)?;
    if metadata.file_type().is_symlink() || (!metadata.is_file() && !metadata.is_dir()) {
        return Err("Choose a real Xcode project, workspace, or containing directory.".into());
    }
    Ok(absolute.canonicalize()?)
}

fn canonical_now() -> Result<String, BoxError> {
    Ok(OffsetDateTime::now_utc()
        .replace_nanosecond(0)?
        .format(&Rfc3339)?)
}

fn ensure_private_directory(path: &Path) -> Result<(), BoxError> {
    fs::create_dir_all(path)?;
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("publication state path is unsafe".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn write_new_private(path: &Path, bytes: &[u8]) -> Result<(), BoxError> {
    use std::io::Write;
    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn to_box(error: Box<dyn std::error::Error + Send + Sync>) -> BoxError {
    error
}

#[cfg(test)]
mod claim_edition_tests {
    use super::*;

    #[test]
    fn exact_first_ship_policy_shapes_are_bounded() {
        assert_eq!(
            parse_claim_edition(Some("open"), None, None)
                .unwrap()
                .unwrap(),
            RequestedClaimEdition {
                kind: "open".into(),
                max_claims: 0,
                closes_at: 0,
            }
        );
        assert_eq!(
            parse_claim_edition(Some("limited"), Some(888), None)
                .unwrap()
                .unwrap()
                .kind,
            "limited"
        );
        assert_eq!(
            parse_claim_edition(Some("timed"), None, Some("2099-09-08T18:00:00Z"))
                .unwrap()
                .unwrap()
                .kind,
            "timed"
        );
        assert_eq!(
            parse_claim_edition(Some("limited"), Some(888), Some("2099-09-08T18:00:00Z"))
                .unwrap()
                .unwrap()
                .kind,
            "limited_timed"
        );
        assert!(parse_claim_edition(None, None, None).unwrap().is_none());
    }

    #[test]
    fn conflicting_or_ambiguous_policy_flags_fail() {
        for value in [
            parse_claim_edition(None, Some(2), None),
            parse_claim_edition(Some("open"), Some(2), None),
            parse_claim_edition(Some("open"), None, Some("2099-09-08T18:00:00Z")),
            parse_claim_edition(Some("limited"), None, None),
            parse_claim_edition(Some("timed"), Some(2), Some("2099-09-08T18:00:00Z")),
            parse_claim_edition(Some("timed"), None, None),
            parse_claim_edition(Some("limited"), Some(0), None),
        ] {
            assert!(value.is_err());
        }
    }

    #[test]
    fn app_slugs_are_stable_and_safe_for_human_routes() {
        let shot = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        assert_eq!(
            publication_app_slug(None, None, "Field Notebook", shot).unwrap(),
            "field-notebook"
        );
        assert_eq!(
            publication_app_slug(None, None, "🧭", shot).unwrap(),
            "app-01234567"
        );
        assert_eq!(
            publication_app_slug(None, Some("field-notebook"), "Renamed", shot).unwrap(),
            "field-notebook"
        );
        assert!(
            publication_app_slug(Some("renamed"), Some("field-notebook"), "Renamed", shot).is_err()
        );
        for invalid in ["A Name", "-name", "name-", "a", "registry"] {
            assert!(publication_app_slug(Some(invalid), None, "Ignored", shot).is_err());
        }
    }

    #[test]
    fn friend_route_evidence_must_bind_the_exact_slug_and_shot() {
        let shot = format!("0x{}", "11".repeat(32));
        let release = format!("0x{}", "66".repeat(32));
        let source = format!("0x{}", "88".repeat(32));
        let pointer = GlobalAliasPointer {
            schema: "tohseno.global-alias/1".into(),
            alias: "field-notebook".into(),
            shot_id: shot.clone(),
            builder_id: format!("eip155:4663:0x{}", "22".repeat(20)),
            request_id: format!("0x{}", "33".repeat(32)),
            claim_digest: format!("0x{}", "44".repeat(32)),
            approved_at: "2026-09-01T12:00:00.000Z".into(),
        };
        assert!(pointer.validate("field-notebook", &shot).is_ok());
        assert!(pointer.validate("another-app", &shot).is_err());
        assert!(pointer
            .validate("field-notebook", &format!("0x{}", "55".repeat(32)))
            .is_err());

        let current = CurrentPublicRelease {
            schema: "tohseno.public-catalog-release/1".into(),
            release_digest: release.clone(),
            route: format!("/s/{}", shot.trim_start_matches("0x")),
            release: CurrentPublicReleaseIdentity {
                shot_id: shot.clone(),
                source: CurrentPublicSourceIdentity {
                    sha256: source.clone(),
                },
                display: CurrentPublicDisplayIdentity { icon_sha256: None },
            },
            chain: json!({ "canonical": true }),
            manifest_url: format!("/api/registry/v1/releases/{release}"),
            source_url: format!("/api/registry/v1/blobs/{source}"),
            icon_url: None,
        };
        assert!(current.validate(&shot, &release).is_ok());
        assert!(current
            .validate(&shot, &format!("0x{}", "77".repeat(32)))
            .is_err());

        let live = FriendRouteObservation {
            slug: "field-notebook".into(),
            url: "https://tohseno.com/field-notebook".into(),
            status: FriendRouteStatus::Live,
        };
        assert!(friend_route_copy(&live).contains("send this exact link"));
        let mut conflict = live;
        conflict.status = FriendRouteStatus::Conflict;
        let copy = friend_route_copy(&conflict);
        assert!(copy.contains("exact release"));
        assert!(copy.contains("Do not send it"));
    }

    #[tokio::test]
    async fn friend_route_probe_requires_pointer_and_human_page_agreement() {
        use axum::routing::get;
        use axum::{Json, Router};

        let shot = format!("0x{}", "11".repeat(32));
        let release = format!("0x{}", "66".repeat(32));
        let source = format!("0x{}", "88".repeat(32));
        let pointer = json!({
            "schema": "tohseno.global-alias/1",
            "alias": "field-notebook",
            "shot_id": shot,
            "builder_id": format!("eip155:4663:0x{}", "22".repeat(20)),
            "request_id": format!("0x{}", "33".repeat(32)),
            "claim_digest": format!("0x{}", "44".repeat(32)),
            "approved_at": "2026-09-01T12:00:00.000Z"
        });
        let public_release = json!({
            "schema": "tohseno.public-catalog-release/1",
            "release_digest": release,
            "route": format!("/s/{}", shot.trim_start_matches("0x")),
            "release": {
                "shot_id": shot,
                "source": { "sha256": source },
                "display": { "icon_sha256": null }
            },
            "chain": { "canonical": true },
            "manifest_url": format!("/api/registry/v1/releases/{release}"),
            "source_url": format!("/api/registry/v1/blobs/{source}"),
            "icon_url": null
        });
        let shot_route = format!("/api/registry/v1/shots/{shot}");
        let application = Router::new()
            .route(
                "/api/registry/v1/aliases/field-notebook",
                get(move || async move { Json(pointer) }),
            )
            .route(
                &shot_route,
                get(move || async move { Json(public_release) }),
            )
            .route("/field-notebook", get(|| async { "exact app page" }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, application).await.unwrap() });
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();

        let live =
            observe_friend_route_exact(&client, &origin, "field-notebook", &shot, &release).await;
        assert_eq!(live.status, FriendRouteStatus::Live);
        let conflict = observe_friend_route_exact(
            &client,
            &origin,
            "field-notebook",
            &format!("0x{}", "55".repeat(32)),
            &release,
        )
        .await;
        assert_eq!(conflict.status, FriendRouteStatus::Conflict);
        let stale_release = observe_friend_route_exact(
            &client,
            &origin,
            "field-notebook",
            &shot,
            &format!("0x{}", "77".repeat(32)),
        )
        .await;
        assert_eq!(stale_release.status, FriendRouteStatus::Conflict);
        let awaiting =
            observe_friend_route_exact(&client, &origin, "not-approved", &shot, &release).await;
        assert_eq!(awaiting.status, FriendRouteStatus::AwaitingReview);
        server.abort();
    }
}
