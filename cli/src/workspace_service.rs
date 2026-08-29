//! Persistent loopback-only Local Workspace Service and Studio API.

use async_stream::stream;
use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Path as AxumPath, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use futures_util::Stream;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::convert::Infallible;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tohseno_application::{
    ApplicationError, CommandJournal, CommandOrigin, CreateShotCommand, EntitlementStore,
    EvolveShotCommand, JournalError, ReferenceInput, ShotApplicationService, SubscriptionPlan,
};
use tohseno_engine::shot_execution::{
    elapsed_seconds_between, load_completion, load_execution, read_events,
};
use tohseno_engine::{
    Config, CustomHarnessConfig, Engine, Event, EventBus, ExecutionPhase, HarnessAdapter,
    HarnessSelection, LocalEndpointConfig, LocalPendingIntention, PendingIntentionStore,
    ShotLayout,
};
use tohseno_protocol::digest::{Bytes32, ExpressionId, ShotId, VersionId};
use uuid::Uuid;

use crate::cable_genesis::{
    build_and_install_companion_with_progress, device_digest, launch_companion_bootstrap,
    project as project_genesis, CableGenesisStore, CableGenesisView, CompanionInstallState,
    GenesisObservation, COMPANION_BUILD_FAILURE, COMPANION_INSTALL_FAILURE,
    COMPANION_LAUNCH_FAILURE, COMPANION_PAIRING_FAILURE,
};
use crate::companion_service::{CompanionCoordinator, PairingCompletion, PairingSessionView};
use crate::device_readiness::{
    project as project_readiness, ReadinessStore, ReadinessView, VerificationState,
};
use crate::managed_compute::{
    bounded_source_bytes, estimate as estimate_managed_cost, ManagedClient, ManagedEstimate,
    ManagedModel,
};
use crate::native_session::{
    NativeSessionActivation, NativeSessionAuthority, NativeSessionChallenge,
    NativeSessionCredential,
};
use crate::service_commands::ServicePaths;
use crate::workspace_identity::{KeychainSecretStore, SecretStore, WorkspaceIdentity};

const RUNTIME_SCHEMA: &str = "tohseno.local-workspace-runtime/1";
const HEALTH_SCHEMA: &str = "tohseno.local-workspace-health/1";
const MAX_RUNTIME_BYTES: u64 = 64 * 1024;
const MAX_REFERENCE_TOTAL_BYTES: usize = 160 * 1024 * 1024;
// Base64url expands the decoded reference allowance by 4/3. The remaining
// space covers the exact intention and bounded JSON descriptors.
const MAX_API_BODY_BYTES: usize = 232 * 1024 * 1024;
const MAX_INTENTION_BYTES: usize = 4 * 1024 * 1024;
const MAX_HEADER_COUNT: usize = 128;
const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_SINGLE_HEADER_BYTES: usize = 8 * 1024;
pub const DEFAULT_SERVICE_PORT: u16 = 8888;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeRecord {
    pub schema: String,
    pub service_version: String,
    pub workspace_id: String,
    pub studio_device_id: String,
    pub origin: String,
    pub port: u16,
    pub process_id: u32,
    pub started_at: String,
    pub instance_id: String,
    pub csrf_token: String,
}

#[derive(Clone)]
struct WorkspaceState {
    runtime: RuntimeRecord,
    application: ShotApplicationService,
    companion: Arc<CompanionCoordinator>,
    events: EventBus,
    event_cursor: Arc<AtomicU64>,
    genesis: CableGenesisStore,
    readiness: ReadinessStore,
    entitlement: EntitlementStore,
    service_root: PathBuf,
    companion_project: PathBuf,
    readiness_project: PathBuf,
    workspace_identity: Arc<WorkspaceIdentity>,
    billing_verification_key: PathBuf,
    native_sessions: NativeSessionAuthority,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    fn bad(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code,
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message: message.into(),
        }
    }

    fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code,
            message: message.into(),
        }
    }

    fn application(error: ApplicationError) -> Self {
        match error {
            ApplicationError::Invalid(message) if message.starts_with("stale evolution:") => {
                Self::conflict("stale_base", message)
            }
            ApplicationError::Journal(JournalError::Conflict(message)) => {
                Self::conflict("command_conflict", message)
            }
            ApplicationError::Invalid(message)
            | ApplicationError::Journal(JournalError::Invalid(message)) => {
                Self::bad("invalid_request", message)
            }
            error => Self::internal(error),
        }
    }

    fn retire_application(error: ApplicationError) -> Self {
        match error {
            ApplicationError::Engine(tohseno_engine::EngineError::DeviceUnavailable(_)) => {
                Self::unavailable(
                    "Connect and unlock your iPhone before deleting an installed app.",
                )
            }
            ApplicationError::Engine(tohseno_engine::EngineError::Install(_)) => Self::unavailable(
                "Your iPhone could not remove this app. Keep it connected and unlocked, then try again.",
            ),
            ApplicationError::Invalid(message)
                if message.starts_with("this app is still building") =>
            {
                Self::conflict("app_busy", message)
            }
            ApplicationError::Invalid(message) if message == "app does not exist" => {
                Self::not_found(message)
            }
            error => Self::application(error),
        }
    }

    fn internal(error: impl std::fmt::Display) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "local_service_error",
            message: error.to_string(),
        }
    }

    fn unavailable(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "service_unavailable",
            message: message.into(),
        }
    }

    fn payment_required(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::PAYMENT_REQUIRED,
            code: "managed_balance_required",
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({
                "schema": "tohseno.local-api-error/1",
                "code": self.code,
                "message": self.message,
            })),
        )
            .into_response()
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApiReference {
    filename: String,
    media_type: String,
    origin: String,
    bytes_base64url: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateRequest {
    command_id: String,
    #[serde(default)]
    origin: ApiOrigin,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    harness: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    managed: Option<ManagedExecutionRequest>,
    intention: String,
    #[serde(default)]
    pending_intention_id: Option<String>,
    #[serde(default)]
    references: Vec<ApiReference>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvolveRequest {
    command_id: String,
    #[serde(default)]
    origin: ApiOrigin,
    base_expression_id: String,
    base_version_id: String,
    base_version_ordinal: u64,
    intention: String,
    #[serde(default)]
    selected_feedback_actions: Vec<String>,
    #[serde(default)]
    references: Vec<ApiReference>,
    #[serde(default)]
    harness: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    managed: Option<ManagedExecutionRequest>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ApiOrigin {
    #[default]
    Studio,
    Cli,
    Native,
}

impl ApiOrigin {
    fn command_origin(self) -> CommandOrigin {
        match self {
            Self::Studio => CommandOrigin::Studio,
            Self::Cli => CommandOrigin::Cli,
            Self::Native => CommandOrigin::Native,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyRequest {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BillingRequest {
    plan: SubscriptionPlan,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CustomHarnessRequest {
    id: String,
    label: String,
    executable: String,
    #[serde(default)]
    arguments: Vec<String>,
    models: Vec<String>,
    #[serde(default)]
    preferred: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LocalEndpointRequest {
    id: String,
    label: String,
    base_url: String,
    models: Vec<String>,
    #[serde(default)]
    credential_reference: Option<String>,
    consent_to_send_source: bool,
    privacy_mode: String,
    #[serde(default)]
    preferred: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManagedExecutionRequest {
    model: String,
    privacy: String,
    maximum_microusd: u64,
    explicit_consent: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManagedEstimateRequest {
    model: String,
    privacy: String,
    intention_bytes: u64,
    reference_bytes: u64,
    #[serde(default)]
    source_context_bytes: u64,
    #[serde(default)]
    shot_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManagedCheckoutRequest {
    pack_id: String,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    schema: &'static str,
    status: &'static str,
    service_version: String,
    workspace_id: String,
    studio_device_id: String,
    origin: String,
    instance_id: String,
}

/// Run the persistent service in the foreground. launchd owns this process in
/// installed builds; tests can inject an isolated root and secret store.
pub async fn run(port: Option<u16>, events: EventBus) -> Result<(), BoxError> {
    let paths = ServicePaths::discover().map_err(boxed)?;
    run_with(&paths, port, events, &KeychainSecretStore).await
}

pub async fn run_with(
    paths: &ServicePaths,
    port: Option<u16>,
    events: EventBus,
    secrets: &dyn SecretStore,
) -> Result<(), BoxError> {
    ensure_private_directory(&paths.service_state)?;
    let _lock = acquire_service_lock(&paths.service_state.join("workspace-service.lock"))?;
    // Only the process that owns the service lock may rotate launchd's log
    // files. A duplicate foreground invocation must be a read-only failure,
    // not a chance to rename files beneath the live service.
    rotate_operational_logs(&paths.logs)?;
    let workspace = Arc::new(WorkspaceIdentity::load_or_create(
        &paths.service_state,
        secrets,
    )?);
    let engine = Engine::discover(events.clone())?;
    let journal = CommandJournal::open(&paths.service_state)?;
    let entitlement = EntitlementStore::open(paths.service_state.clone())?;
    #[cfg(debug_assertions)]
    if std::env::var("TOHSENO_DEVELOPMENT_ENTITLEMENT").as_deref() == Ok("1") {
        entitlement.grant_development_at(OffsetDateTime::now_utc())?;
    }
    let genesis = CableGenesisStore::open(&paths.service_state)?;
    if genesis.recover_interrupted_install()? {
        events.emit(Event::status(
            "An interrupted iPhone setup is ready to continue.",
        ));
    }
    let readiness = ReadinessStore::open(&paths.service_state)?;
    if readiness.recover_interrupted()? {
        events.emit(Event::status(
            "An interrupted iPhone readiness check is ready to retry.",
        ));
    }
    let application = ShotApplicationService::new(
        engine,
        journal,
        events.clone(),
        workspace.record.workspace_id.clone(),
    )
    .with_entitlement(entitlement.clone());
    // Commands are durable before their semantic effects begin. Reconcile
    // interrupted admissions before accepting new HTTP or companion work so a
    // service restart cannot strand received, validated, accepted, or running
    // commands.
    application.recover_commands().await?;
    let companion = Arc::new(CompanionCoordinator::open(
        paths.service_state.clone(),
        workspace.clone(),
        application.clone(),
    )?);
    // A pre-1.0.0 paired installation keeps its identity and capability. Its
    // first 1.0.0 service observation becomes a deterministic trial anchor;
    // existing app count never fabricates successful days.
    if entitlement.state()?.phase == tohseno_application::EntitlementPhase::GenesisIncomplete
        && companion.devices()?.iter().any(|device| !device.revoked)
    {
        entitlement.migrate_existing_pairing_now()?;
    }
    let requested_port = port
        .or_else(|| {
            std::env::var("TOHSENO_SERVICE_PORT")
                .ok()
                .and_then(|value| value.parse().ok())
        })
        .unwrap_or(DEFAULT_SERVICE_PORT);
    let listener = tokio::net::TcpListener::bind(SocketAddr::new(
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        requested_port,
    ))
    .await?;
    let address = listener.local_addr()?;
    if !address.ip().is_loopback() {
        return Err("Local Workspace Service refused a non-loopback listener".into());
    }
    let csrf_token = random_token(32);
    let runtime = RuntimeRecord {
        schema: RUNTIME_SCHEMA.into(),
        service_version: env!("CARGO_PKG_VERSION").into(),
        workspace_id: workspace.record.workspace_id.clone(),
        studio_device_id: workspace.record.studio_device_id.clone(),
        origin: format!("http://127.0.0.1:{}", address.port()),
        port: address.port(),
        process_id: std::process::id(),
        started_at: now(),
        instance_id: format!("service_{}", Uuid::new_v4().simple()),
        csrf_token,
    };
    publish_runtime(&paths.service_state, &runtime)?;
    let companion_project = paths
        .install_root
        .join("current/share/companion/apple/TohsenoCompanion/App/TohsenoCompanion.xcodeproj");
    let readiness_project = paths.install_root.join("current/share/readiness/apple");
    let billing_verification_key = paths
        .install_root
        .join("current/share/billing/verification-key-p256.txt");
    #[cfg(debug_assertions)]
    let development_repository_root = (std::env::var("TOHSENO_DEVELOPMENT_SERVICE").as_deref()
        == Ok("1"))
    .then(|| {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .map(Path::to_path_buf)
            .ok_or("CLI source path has no repository root")
    })
    .transpose()?;
    #[cfg(debug_assertions)]
    let companion_project = development_repository_root
        .as_ref()
        .filter(|_| !companion_project.is_file())
        .map(|root| root.join("companion/apple/TohsenoCompanion/App/TohsenoCompanion.xcodeproj"))
        .unwrap_or(companion_project);
    #[cfg(debug_assertions)]
    let readiness_project = development_repository_root
        .as_ref()
        .filter(|_| !readiness_project.join("HelloWorld.xcodeproj").is_dir())
        .map(|root| root.join("engine/fixtures/hello-world"))
        .unwrap_or(readiness_project);
    #[cfg(debug_assertions)]
    let billing_verification_key = development_repository_root
        .as_ref()
        .filter(|_| !billing_verification_key.is_file())
        .map(|root| root.join("billing/verification-key-p256.txt"))
        .unwrap_or(billing_verification_key);
    let state = Arc::new(WorkspaceState {
        runtime: runtime.clone(),
        application,
        companion,
        events: events.clone(),
        event_cursor: Arc::new(AtomicU64::new(1)),
        genesis,
        readiness,
        entitlement,
        service_root: paths.service_state.clone(),
        companion_project,
        readiness_project,
        workspace_identity: workspace,
        billing_verification_key,
        native_sessions: NativeSessionAuthority::default(),
    });
    let reconciliation_companion = state.companion.clone();
    let reconciliation_application = state.application.clone();
    let reconciliation_events = events.clone();
    let reconciliation_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(2));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut last_workspace_digest = None;
        let mut last_fingerprint = None;
        let mut ticks_since_full_pass = WORKSPACE_SNAPSHOT_BACKSTOP_TICKS;
        loop {
            interval.tick().await;
            // Relay failures are transient operational state. The next bounded
            // pass retries without logging identifiers or private content.
            if reconciliation_companion
                .reconcile_pairing_sessions()
                .await
                .is_ok_and(|completed| completed > 0)
            {
                reconciliation_events.emit(Event::status("Companion pairing state changed."));
            }
            let _ = reconciliation_companion.reconcile_relay_once().await;
            let _ = reconciliation_companion.publish_workspace_changes().await;
            // Rebuilding the snapshot walks every app tree and reverifies every
            // lineage, so an unconditional pass here costs a fifth of a core
            // forever and grows with the workspace. The stat-only fingerprint
            // moves whenever private per-app state does; the slower backstop
            // still converges if a change leaves every timestamp untouched.
            let fingerprint = workspace_change_fingerprint(reconciliation_application.engine());
            ticks_since_full_pass = ticks_since_full_pass.saturating_add(1);
            let rebuild = fingerprint.is_none()
                || fingerprint != last_fingerprint
                || ticks_since_full_pass >= WORKSPACE_SNAPSHOT_BACKSTOP_TICKS;
            if !rebuild {
                continue;
            }
            ticks_since_full_pass = 0;
            last_fingerprint = fingerprint;
            if let Ok(snapshot) = reconciliation_application.workspace_snapshot().await {
                if let Ok(digest) = privacy_safe_workspace_digest(&snapshot) {
                    if last_workspace_digest.is_some_and(|previous| previous != digest) {
                        reconciliation_events.emit(Event::status("Local workspace state changed."));
                    }
                    last_workspace_digest = Some(digest);
                }
            }
        }
    });
    let app = router(state);
    events.emit(Event::status("Local Workspace Service is ready."));
    let result = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await;
    reconciliation_task.abort();
    let _ = reconciliation_task.await;
    remove_runtime_if_current(&paths.service_state, &runtime.instance_id)?;
    result.map_err(Into::into)
}

fn router(state: Arc<WorkspaceState>) -> Router {
    Router::new()
        .route("/", get(studio_index))
        .route("/create", get(studio_index))
        .route("/settings", get(studio_index))
        .route("/shots/{shot_id}", get(studio_index))
        .route("/app.js", get(studio_javascript))
        .route("/style.css", get(studio_stylesheet))
        .route("/tohseno-logo.png", get(studio_logo))
        .route("/pairing-seal.png", get(studio_pairing_seal))
        .route("/api/v1/health", get(health))
        .route("/api/v1/studio-session", get(studio_session))
        .route(
            "/api/v1/native-session/challenge",
            get(native_session_challenge),
        )
        .route("/api/v1/native-session", post(activate_native_session))
        .route("/api/v1/workspace", get(workspace))
        .route("/api/v1/factory-defaults", get(factory_defaults))
        .route(
            "/api/v1/intelligence/custom-harnesses",
            post(configure_custom_harness),
        )
        .route(
            "/api/v1/intelligence/local-endpoints",
            post(configure_local_endpoint),
        )
        .route("/api/v1/managed/status", get(managed_status))
        .route("/api/v1/managed/balance", get(managed_balance))
        .route("/api/v1/managed/catalog", get(managed_catalog))
        .route("/api/v1/managed/estimate", post(managed_estimate))
        .route("/api/v1/managed/checkout", post(managed_checkout))
        .route("/api/v1/readiness", get(readiness_status))
        .route("/api/v1/readiness/actions/{action}", post(readiness_action))
        .route("/api/v1/entitlement", get(entitlement_status))
        .route("/api/v1/billing/checkout", post(billing_checkout))
        .route("/api/v1/billing/refresh", post(billing_refresh))
        .route("/api/v1/genesis", get(genesis_status))
        .route("/api/v1/genesis/actions/{action}", post(genesis_action))
        .route(
            "/api/v1/pending-intentions/{pending_id}",
            get(pending_intention),
        )
        .route("/api/v1/shots", get(shots).post(create_shot))
        .route("/api/v1/shots/{shot_id}", delete(retire_shot))
        .route("/api/v1/shots/{shot_id}/restore", post(restore_shot))
        .route("/api/v1/shots/{shot_id}/icon", get(shot_icon))
        .route("/api/v1/shots/{shot_id}/preview", get(shot_preview))
        .route("/api/v1/shots/{shot_id}/receipt", get(shot_receipt))
        .route("/api/v1/shots/{shot_id}/activity", get(shot_activity))
        .route(
            "/api/v1/shots/{shot_id}/open-source",
            post(open_shot_source),
        )
        .route(
            "/api/v1/shots/{shot_id}/open-on-iphone",
            post(open_shot_on_iphone),
        )
        .route("/api/v1/shots/{shot_id}/evolutions", post(evolve_shot))
        .route("/api/v1/executions", get(executions))
        .route("/api/v1/executions/{execution_id}", get(execution))
        .route("/api/v1/events", get(events))
        .route(
            "/api/v1/companion/pairing-sessions",
            post(create_pairing_session),
        )
        .route(
            "/api/v1/companion/pairing-sessions/{session_id}",
            get(pairing_session).delete(cancel_pairing_session),
        )
        .route(
            "/api/v1/companion/pairing-sessions/{session_id}/respond",
            post(complete_pairing),
        )
        .route("/api/v1/companion/devices", get(devices))
        .route(
            "/api/v1/companion/devices/{device_id}",
            delete(revoke_device),
        )
        .route("/api/v1/companion/status", get(companion_status))
        .route(
            "/api/v1/companion/simulate/envelopes",
            post(simulate_envelope),
        )
        .layer(DefaultBodyLimit::max(MAX_API_BODY_BYTES))
        .layer(middleware::from_fn_with_state(state.clone(), security))
        .with_state(state)
}

async fn security(
    State(state): State<Arc<WorkspaceState>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if !headers_within_bounds(request.headers()) {
        return ApiError {
            status: StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE,
            code: "headers_too_large",
            message: "request headers exceed the local API budget".into(),
        }
        .into_response();
    }
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok());
    let expected_host = state.runtime.origin.trim_start_matches("http://");
    if host != Some(expected_host) {
        return ApiError {
            status: StatusCode::MISDIRECTED_REQUEST,
            code: "host_rejected",
            message: "unexpected Host header".into(),
        }
        .into_response();
    }
    let path = request.uri().path().to_owned();
    let native_authorization = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());
    let native_authenticated = match native_authorization {
        Some(authorization) => {
            let scope = if is_mutation(request.method()) {
                "factory.mutate"
            } else if path == "/api/v1/events" {
                "events.read"
            } else {
                "factory.read"
            };
            if state
                .native_sessions
                .authorize(authorization, &state.runtime.instance_id, scope)
                .is_err()
            {
                return ApiError {
                    status: StatusCode::FORBIDDEN,
                    code: "native_session_rejected",
                    message: "native client session is missing, expired, or out of scope".into(),
                }
                .into_response();
            }
            true
        }
        None => false,
    };
    if is_mutation(request.method()) {
        let is_native_activation = path == "/api/v1/native-session";
        let origin = request
            .headers()
            .get(header::ORIGIN)
            .and_then(|value| value.to_str().ok());
        let csrf = request
            .headers()
            .get("x-tohseno-csrf")
            .and_then(|value| value.to_str().ok());
        let content_type = request
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next());
        if !native_authenticated
            && !is_native_activation
            && origin != Some(state.runtime.origin.as_str())
        {
            return ApiError {
                status: StatusCode::FORBIDDEN,
                code: "origin_rejected",
                message: "mutation Origin does not match Studio".into(),
            }
            .into_response();
        }
        if !native_authenticated
            && !is_native_activation
            && csrf != Some(state.runtime.csrf_token.as_str())
        {
            return ApiError {
                status: StatusCode::FORBIDDEN,
                code: "csrf_rejected",
                message: "anti-CSRF token is missing or invalid".into(),
            }
            .into_response();
        }
        if content_type != Some("application/json") {
            return ApiError {
                status: StatusCode::UNSUPPORTED_MEDIA_TYPE,
                code: "content_type_rejected",
                message: "mutations require application/json".into(),
            }
            .into_response();
        }
    }
    let is_api = request.uri().path().starts_with("/api/");
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
    headers.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    headers.insert(
        "permissions-policy",
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );
    headers.insert(
        "content-security-policy",
        HeaderValue::from_static(
            "default-src 'self'; img-src 'self' data:; style-src 'self'; script-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'",
        ),
    );
    if is_api {
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        headers.insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    }
    response
}

fn headers_within_bounds(headers: &axum::http::HeaderMap) -> bool {
    if headers.len() > MAX_HEADER_COUNT {
        return false;
    }
    let mut total = 0_usize;
    for (name, value) in headers {
        if value.as_bytes().len() > MAX_SINGLE_HEADER_BYTES {
            return false;
        }
        total = total.saturating_add(name.as_str().len());
        total = total.saturating_add(value.as_bytes().len());
        if total > MAX_HEADER_BYTES {
            return false;
        }
    }
    true
}

fn is_mutation(method: &Method) -> bool {
    matches!(
        *method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    )
}

async fn health(State(state): State<Arc<WorkspaceState>>) -> Json<HealthResponse> {
    Json(HealthResponse {
        schema: HEALTH_SCHEMA,
        status: "healthy",
        service_version: state.runtime.service_version.clone(),
        workspace_id: state.runtime.workspace_id.clone(),
        studio_device_id: state.runtime.studio_device_id.clone(),
        origin: state.runtime.origin.clone(),
        instance_id: state.runtime.instance_id.clone(),
    })
}

async fn studio_session(
    State(state): State<Arc<WorkspaceState>>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let bootstrap = headers
        .get("x-tohseno-browser-bootstrap")
        .and_then(|value| value.to_str().ok());
    if bootstrap != Some(state.runtime.csrf_token.as_str()) {
        return Err(ApiError {
            status: StatusCode::FORBIDDEN,
            code: "studio_bootstrap_rejected",
            message: "open Studio through the installed TOHSENO application or CLI".into(),
        });
    }
    Ok(Json(json!({
        "schema": "tohseno.local-studio-session/1",
        "csrf_token": state.runtime.csrf_token,
        "origin": state.runtime.origin,
        "instance_id": state.runtime.instance_id,
    })))
}

async fn native_session_challenge(
    State(state): State<Arc<WorkspaceState>>,
) -> Result<Json<NativeSessionChallenge>, ApiError> {
    state
        .native_sessions
        .issue_challenge(&state.runtime.instance_id, OffsetDateTime::now_utc())
        .map(Json)
        .map_err(|error| ApiError::unavailable(error.to_string()))
}

async fn activate_native_session(
    State(state): State<Arc<WorkspaceState>>,
    Json(request): Json<NativeSessionActivation>,
) -> Result<Json<NativeSessionCredential>, ApiError> {
    state
        .native_sessions
        .activate(
            request,
            &state.workspace_identity.identity,
            &state.runtime.origin,
            &state.runtime.instance_id,
            OffsetDateTime::now_utc(),
        )
        .map(Json)
        .map_err(|error| ApiError {
            status: StatusCode::FORBIDDEN,
            code: "native_session_rejected",
            message: error.to_string(),
        })
}

async fn workspace(State(state): State<Arc<WorkspaceState>>) -> Result<Json<Value>, ApiError> {
    state
        .application
        .workspace_snapshot()
        .await
        .map(|snapshot| Json(json!(snapshot)))
        .map_err(ApiError::internal)
}

async fn factory_defaults(State(state): State<Arc<WorkspaceState>>) -> Json<Value> {
    Json(json!(state.application.factory_defaults()))
}

async fn managed_status(State(state): State<Arc<WorkspaceState>>) -> Result<Json<Value>, ApiError> {
    let client = ManagedClient::new(state.workspace_identity.identity.clone())
        .map_err(ApiError::internal)?;
    Ok(Json(json!({
        "schema": "tohseno.local-managed-status/1",
        "installation_binding": client.installation_binding(),
        "service_origin": client.origin(),
        "welcome_contact_url": configured_welcome_contact_url()?,
        "automatic_fallback": false,
    })))
}

async fn managed_balance(
    State(state): State<Arc<WorkspaceState>>,
) -> Result<Json<Value>, ApiError> {
    ManagedClient::new(state.workspace_identity.identity.clone())
        .map_err(ApiError::internal)?
        .balance()
        .await
        .map(|value| Json(json!(value)))
        .map_err(|error| ApiError::unavailable(error.to_string()))
}

async fn managed_catalog(
    State(state): State<Arc<WorkspaceState>>,
) -> Result<Json<Value>, ApiError> {
    ManagedClient::new(state.workspace_identity.identity.clone())
        .map_err(ApiError::internal)?
        .catalog()
        .await
        .map(|value| Json(json!(value)))
        .map_err(|error| ApiError::unavailable(error.to_string()))
}

async fn managed_estimate(
    State(state): State<Arc<WorkspaceState>>,
    Json(request): Json<ManagedEstimateRequest>,
) -> Result<Json<ManagedEstimate>, ApiError> {
    let client = ManagedClient::new(state.workspace_identity.identity.clone())
        .map_err(ApiError::internal)?;
    let model = managed_model(&client, &request.model, &request.privacy).await?;
    let source_context_bytes = match request.shot_id.as_deref() {
        Some(shot_id) => {
            let (name, _) = resolve_shot(&state.application, shot_id)?;
            bounded_source_bytes(&state.application.engine().ledger().working_tree(&name))
                .map_err(ApiError::internal)?
        }
        None => request.source_context_bytes,
    };
    estimate_managed_cost(
        &model,
        &request.privacy,
        request.intention_bytes,
        request.reference_bytes,
        source_context_bytes,
    )
    .map(Json)
    .map_err(|error| ApiError::bad("managed_estimate_invalid", error.to_string()))
}

async fn managed_checkout(
    State(state): State<Arc<WorkspaceState>>,
    Json(request): Json<ManagedCheckoutRequest>,
) -> Result<Json<Value>, ApiError> {
    ManagedClient::new(state.workspace_identity.identity.clone())
        .map_err(ApiError::internal)?
        .checkout(&request.pack_id)
        .await
        .map(|value| Json(json!(value)))
        .map_err(|error| ApiError::unavailable(error.to_string()))
}

async fn managed_model(
    client: &ManagedClient,
    model: &str,
    privacy: &str,
) -> Result<ManagedModel, ApiError> {
    let catalog = client
        .catalog()
        .await
        .map_err(|error| ApiError::unavailable(error.to_string()))?;
    catalog
        .models
        .into_iter()
        .find(|candidate| {
            candidate.model == model && candidate.privacy_tiers.iter().any(|tier| tier == privacy)
        })
        .ok_or_else(|| {
            ApiError::bad(
                "managed_route_unavailable",
                "the managed model or privacy tier is not currently advertised",
            )
        })
}

async fn managed_selection(
    state: &WorkspaceState,
    command_id: &str,
    request: &ManagedExecutionRequest,
    intention_bytes: u64,
    reference_bytes: u64,
    source_context_bytes: u64,
) -> Result<HarnessSelection, ApiError> {
    if !request.explicit_consent {
        return Err(ApiError::bad(
            "managed_consent_required",
            "confirm the displayed managed-compute maximum before submitting",
        ));
    }
    validate_identifier(command_id)?;
    let client = ManagedClient::new(state.workspace_identity.identity.clone())
        .map_err(ApiError::internal)?;
    let model = managed_model(&client, &request.model, &request.privacy).await?;
    let estimate = estimate_managed_cost(
        &model,
        &request.privacy,
        intention_bytes,
        reference_bytes,
        source_context_bytes,
    )
    .map_err(|error| ApiError::bad("managed_estimate_invalid", error.to_string()))?;
    if request.maximum_microusd < estimate.high_microusd || request.maximum_microusd > 100_000_000 {
        return Err(ApiError::bad(
            "managed_maximum_invalid",
            "the approved maximum must cover the current server-priced high estimate and may not exceed $100",
        ));
    }
    let balance = client
        .balance()
        .await
        .map_err(|error| ApiError::unavailable(error.to_string()))?;
    if balance.spendable_microusd < request.maximum_microusd as i64 {
        return Err(ApiError::payment_required(
            "Managed creation balance does not cover the approved maximum. Add balance or choose a local route.",
        ));
    }
    let execution_id = tohseno_application::execution_manager::command_execution_id(command_id);
    Ok(HarnessSelection {
        harness: "tohseno-managed".into(),
        model: request.model.clone(),
        route: format!("managed-{}", request.privacy),
        adapter: Some(HarnessAdapter::ManagedOpenAi {
            proxy_origin: client.origin().into(),
            command_id: command_id.into(),
            execution_id,
            privacy_mode: request.privacy.clone(),
            maximum_microusd: request.maximum_microusd,
            pricing_snapshot_at: estimate.pricing_snapshot_at,
            input_microusd_per_million: model.input_microusd_per_million,
            output_microusd_per_million: model.output_microusd_per_million,
            estimate_low_microusd: estimate.low_microusd,
            estimate_high_microusd: estimate.high_microusd,
        }),
    })
}

fn configured_welcome_contact_url() -> Result<Option<String>, ApiError> {
    #[cfg(debug_assertions)]
    let configured = std::env::var("TOHSENO_WELCOME_COMPUTE_URL")
        .ok()
        .or_else(|| option_env!("TOHSENO_WELCOME_COMPUTE_URL").map(str::to_owned));
    #[cfg(not(debug_assertions))]
    let configured = option_env!("TOHSENO_WELCOME_COMPUTE_URL").map(str::to_owned);
    let Some(value) = configured else {
        return Ok(None);
    };
    if value.len() > 2_048 || value.chars().any(char::is_control) {
        return Err(ApiError::internal(
            "welcome compute contact configuration is invalid",
        ));
    }
    let url = reqwest::Url::parse(&value)
        .map_err(|_| ApiError::internal("welcome compute contact configuration is invalid"))?;
    let safe_https = url.scheme() == "https"
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none();
    let safe_mail = url.scheme() == "mailto" && !url.path().is_empty() && url.fragment().is_none();
    if !safe_https && !safe_mail {
        return Err(ApiError::internal(
            "welcome compute contact configuration is invalid",
        ));
    }
    Ok(Some(value))
}

async fn configure_custom_harness(
    State(state): State<Arc<WorkspaceState>>,
    Json(request): Json<CustomHarnessRequest>,
) -> Result<Json<Value>, ApiError> {
    validate_identifier(&request.id)?;
    validate_text("custom harness label", &request.label, 160)?;
    let mut config = Config::load_or_default(state.application.engine().ledger().machine_root())
        .map_err(ApiError::internal)?;
    config
        .intelligence
        .custom_harnesses
        .retain(|existing| existing.id != request.id);
    config
        .intelligence
        .custom_harnesses
        .push(CustomHarnessConfig {
            id: request.id.clone(),
            label: request.label,
            executable: request.executable,
            arguments: request.arguments,
            models: request.models,
        });
    require_configured_harness(&config, &request.id)?;
    if request.preferred {
        config.intelligence.preferred_harness = Some(request.id.clone());
    }
    config
        .save(state.application.engine().ledger().machine_root())
        .map_err(ApiError::internal)?;
    Ok(Json(json!({
        "schema": "tohseno.intelligence-configuration-receipt/1",
        "harness_id": request.id,
        "restart_required": true,
    })))
}

async fn configure_local_endpoint(
    State(state): State<Arc<WorkspaceState>>,
    Json(request): Json<LocalEndpointRequest>,
) -> Result<Json<Value>, ApiError> {
    validate_identifier(&request.id)?;
    validate_text("local endpoint label", &request.label, 160)?;
    if !request.consent_to_send_source {
        return Err(ApiError::bad(
            "local_endpoint_consent_required",
            "confirm that app source may be sent to this configured local endpoint",
        ));
    }
    if let Some(reference) = request.credential_reference.as_deref() {
        validate_identifier(reference)?;
        KeychainSecretStore.get(reference).map_err(|_| {
            ApiError::bad(
                "credential_unavailable",
                "the local endpoint credential is not available in macOS Keychain",
            )
        })?;
    }
    probe_local_models(
        &request.base_url,
        request.credential_reference.as_deref(),
        &request.models,
    )
    .await?;
    let mut config = Config::load_or_default(state.application.engine().ledger().machine_root())
        .map_err(ApiError::internal)?;
    config
        .intelligence
        .local_endpoints
        .retain(|existing| existing.id != request.id);
    config
        .intelligence
        .local_endpoints
        .push(LocalEndpointConfig {
            id: request.id.clone(),
            label: request.label,
            base_url: request.base_url,
            models: request.models,
            credential_reference: request.credential_reference,
            consent_to_send_source: true,
            privacy_mode: request.privacy_mode,
        });
    require_configured_harness(&config, &request.id)?;
    if request.preferred {
        config.intelligence.preferred_harness = Some(request.id.clone());
    }
    config
        .save(state.application.engine().ledger().machine_root())
        .map_err(ApiError::internal)?;
    Ok(Json(json!({
        "schema": "tohseno.intelligence-configuration-receipt/1",
        "harness_id": request.id,
        "restart_required": true,
    })))
}

fn require_configured_harness(config: &Config, id: &str) -> Result<(), ApiError> {
    let options = tohseno_engine::harness::discover_harnesses(config);
    let option = options
        .iter()
        .find(|option| option.id == id)
        .ok_or_else(|| {
            ApiError::bad(
                "invalid_harness_configuration",
                "configured harness is invalid",
            )
        })?;
    if !option.installed || !option.routes.iter().any(|route| route.available) {
        return Err(ApiError::bad(
            "invalid_harness_configuration",
            "configured harness is unavailable or unsafe",
        ));
    }
    if options.iter().filter(|option| option.id == id).count() != 1 {
        return Err(ApiError::bad(
            "duplicate_harness_configuration",
            "configured harness ID is not unique",
        ));
    }
    Ok(())
}

async fn probe_local_models(
    base_url: &str,
    credential_reference: Option<&str>,
    requested: &[String],
) -> Result<(), ApiError> {
    let parsed = reqwest::Url::parse(base_url)
        .map_err(|_| ApiError::bad("invalid_local_endpoint", "local endpoint URL is invalid"))?;
    if parsed.scheme() != "http"
        || !matches!(parsed.host_str(), Some("127.0.0.1" | "localhost"))
        || parsed.port().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(ApiError::bad(
            "invalid_local_endpoint",
            "local endpoint must be explicit loopback HTTP with a port",
        ));
    }
    let endpoint = if base_url.trim_end_matches('/').ends_with("/v1") {
        format!("{}/models", base_url.trim_end_matches('/'))
    } else {
        format!("{}/v1/models", base_url.trim_end_matches('/'))
    };
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(ApiError::internal)?;
    let mut request = client.get(endpoint).header("accept", "application/json");
    let credential = credential_reference
        .map(|reference| KeychainSecretStore.get(reference))
        .transpose()
        .map_err(|_| {
            ApiError::bad(
                "credential_unavailable",
                "the local endpoint credential is unavailable",
            )
        })?;
    if let Some(credential) = credential.as_deref() {
        let credential = std::str::from_utf8(credential).map_err(|_| {
            ApiError::bad(
                "credential_invalid",
                "the local endpoint credential is invalid",
            )
        })?;
        request = request.bearer_auth(credential);
    }
    let response = request
        .send()
        .await
        .map_err(|_| ApiError::unavailable("the local model endpoint could not be reached"))?;
    if !response.status().is_success() {
        return Err(ApiError::unavailable(format!(
            "the local model endpoint returned HTTP {}",
            response.status()
        )));
    }
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    use futures_util::StreamExt as _;
    while let Some(chunk) = stream.next().await {
        let chunk =
            chunk.map_err(|_| ApiError::unavailable("the local model catalog was interrupted"))?;
        if body.len().saturating_add(chunk.len()) > 1024 * 1024 {
            return Err(ApiError::bad(
                "model_catalog_oversized",
                "the local model catalog is oversized",
            ));
        }
        body.extend_from_slice(&chunk);
    }
    let value: Value = serde_json::from_slice(&body).map_err(|_| {
        ApiError::bad(
            "model_catalog_invalid",
            "the local model catalog is invalid",
        )
    })?;
    let advertised = value
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            ApiError::bad(
                "model_catalog_invalid",
                "the local model catalog is invalid",
            )
        })?
        .iter()
        .filter_map(|model| model.get("id").and_then(Value::as_str))
        .collect::<std::collections::BTreeSet<_>>();
    if requested.is_empty()
        || requested.len() > 32
        || requested
            .iter()
            .any(|model| !advertised.contains(model.as_str()))
    {
        return Err(ApiError::bad(
            "model_not_advertised",
            "every selected model must be advertised by the local endpoint",
        ));
    }
    Ok(())
}

fn current_readiness_view(state: &WorkspaceState) -> Result<ReadinessView, ApiError> {
    let record = state.readiness.load().map_err(ApiError::internal)?;
    Ok(project_readiness(
        &record,
        &crate::device_readiness::observe(),
    ))
}

async fn readiness_status(
    State(state): State<Arc<WorkspaceState>>,
) -> Result<Json<ReadinessView>, ApiError> {
    current_readiness_view(&state).map(Json)
}

async fn readiness_action(
    State(state): State<Arc<WorkspaceState>>,
    AxumPath(action): AxumPath<String>,
    Json(_empty): Json<EmptyRequest>,
) -> Result<Json<ReadinessView>, ApiError> {
    match action.as_str() {
        "begin" => state.readiness.begin().map_err(ApiError::internal)?,
        "check" | "create_app" => {}
        "open_app_store" => {
            let status = std::process::Command::new("open")
                .arg("macappstore://itunes.apple.com/app/id497799835")
                .status()
                .map_err(ApiError::internal)?;
            if !status.success() {
                return Err(ApiError::internal("the App Store could not be opened"));
            }
        }
        "open_xcode" => {
            let status = std::process::Command::new("open")
                .args(["-a", "Xcode"])
                .status()
                .map_err(ApiError::internal)?;
            if !status.success() {
                return Err(ApiError::internal("Xcode could not be opened"));
            }
        }
        "verify_installation" => {
            let view = current_readiness_view(&state)?;
            if view.step != "verify_installation" {
                return Err(ApiError::conflict(
                    "readiness_not_ready",
                    "complete the current iPhone readiness step first",
                ));
            }
            let observed = crate::device_readiness::observe();
            let device = match observed.device {
                Some(tohseno_engine::gates::device::DeviceState::Ready(device)) => device,
                _ => {
                    return Err(ApiError::conflict(
                        "iphone_not_ready",
                        "the connected iPhone is not ready for installation",
                    ));
                }
            };
            let team_id = observed.signing_team.ok_or_else(|| {
                ApiError::conflict(
                    "apple_account_not_ready",
                    "add your Apple Account in Xcode first",
                )
            })?;
            state
                .readiness
                .verification(VerificationState::Building, None)
                .map_err(ApiError::internal)?;
            let store = state.readiness.clone();
            let project = state.readiness_project.clone();
            let service_root = state.service_root.clone();
            let events = state.events.clone();
            tokio::spawn(async move {
                let progress_store = store.clone();
                let outcome = tokio::task::spawn_blocking(move || {
                    crate::device_readiness::verify_installation(
                        &project,
                        &service_root,
                        &device,
                        &team_id,
                        move || {
                            progress_store.verification(VerificationState::Installing, None)?;
                            events.emit(Event::status(
                                "Readiness app signed; verifying iPhone installation.",
                            ));
                            Ok(())
                        },
                    )
                })
                .await;
                match outcome {
                    Ok(Ok(())) => {
                        let _ = store.verification(VerificationState::Verified, None);
                    }
                    Ok(Err(error)) => {
                        let _ =
                            store.verification(VerificationState::Failed, Some(&error.to_string()));
                    }
                    Err(error) => {
                        let _ =
                            store.verification(VerificationState::Failed, Some(&error.to_string()));
                    }
                }
            });
        }
        _ => return Err(ApiError::not_found("unknown readiness action")),
    }
    current_readiness_view(&state).map(Json)
}

async fn entitlement_status(
    State(state): State<Arc<WorkspaceState>>,
) -> Result<Json<Value>, ApiError> {
    state
        .application
        .entitlement_status()
        .map_err(ApiError::application)?
        .map(|status| Json(json!(status)))
        .ok_or_else(|| ApiError::internal("private entitlement authority is unavailable"))
}

async fn billing_checkout(
    State(state): State<Arc<WorkspaceState>>,
    Json(request): Json<BillingRequest>,
) -> Result<Json<Value>, ApiError> {
    let status = state.entitlement.status_now().map_err(ApiError::internal)?;
    if !status.purchase_allowed {
        return Err(ApiError::conflict(
            "purchase_unavailable",
            "TOHSENO Pro is available only after five successful days.",
        ));
    }
    // Refuse to send somebody to payment until this installed release already
    // has the public key required to verify the resulting receipt locally.
    crate::billing::read_verification_key(&state.billing_verification_key)
        .map_err(|error| ApiError::unavailable(error.to_string()))?;
    let checkout_url = crate::billing::begin_checkout(
        &state.runtime.workspace_id,
        &state.workspace_identity.identity,
        request.plan,
    )
    .await
    .map_err(|error| ApiError::unavailable(error.to_string()))?;
    Ok(Json(json!({
        "schema": "tohseno.local-checkout-continuation/1",
        "checkout_url": checkout_url,
    })))
}

async fn billing_refresh(
    State(state): State<Arc<WorkspaceState>>,
    Json(_empty): Json<EmptyRequest>,
) -> Result<Json<Value>, ApiError> {
    let status = crate::billing::refresh_entitlement(
        &state.runtime.workspace_id,
        &state.workspace_identity.identity,
        &state.entitlement,
        &state.billing_verification_key,
        &state.service_root.join("billing/receipt-v1.json"),
    )
    .await
    .map_err(|error| ApiError::unavailable(error.to_string()))?;
    state
        .events
        .emit(Event::status("TOHSENO entitlement refreshed."));
    // The receipt has already unlocked the local factory. A temporarily
    // unavailable private relay must not turn that durable success into a
    // failed refresh response; the Companion receives the same projection on
    // its next snapshot request.
    let _ = state.companion.publish_entitlement_to_all_devices().await;
    Ok(Json(json!(status)))
}

fn observe_genesis(state: &WorkspaceState) -> Result<GenesisObservation, ApiError> {
    use tohseno_engine::gates::{apple_signing, device, toolchain};
    let xcode_ready = toolchain::check() == toolchain::ToolchainState::Ready;
    let device_state = xcode_ready.then(device::check).and_then(Result::ok);
    let cable_visible = match device_state.as_ref() {
        Some(device::DeviceState::CableMissing) | None => device::cable_visible(),
        Some(_) => true,
    };
    let signing_ready = matches!(
        apple_signing::check(),
        apple_signing::AppleSigningState::Ready { .. }
    );
    let paired = state
        .companion
        .devices()
        .map_err(ApiError::internal)?
        .iter()
        .any(|device| !device.revoked);
    Ok(GenesisObservation {
        cable_visible,
        xcode_ready,
        device: device_state,
        signing_ready,
        paired,
    })
}

fn current_genesis_view(state: &WorkspaceState) -> Result<CableGenesisView, ApiError> {
    let observed = observe_genesis(state)?;
    let mut record = state.genesis.load().map_err(ApiError::internal)?;
    if observed.paired
        && matches!(
            record.companion_install,
            CompanionInstallState::WaitingForPairing | CompanionInstallState::Installed
        )
    {
        record = state
            .genesis
            .set_install_state(CompanionInstallState::Installed, None, None, None)
            .map_err(ApiError::internal)?;
        state
            .entitlement
            .complete_genesis_now()
            .map_err(ApiError::internal)?;
    }
    Ok(project_genesis(&record, &observed))
}

async fn genesis_status(
    State(state): State<Arc<WorkspaceState>>,
) -> Result<Json<CableGenesisView>, ApiError> {
    current_genesis_view(&state).map(Json)
}

async fn genesis_action(
    State(state): State<Arc<WorkspaceState>>,
    AxumPath(action): AxumPath<String>,
    Json(_empty): Json<EmptyRequest>,
) -> Result<Json<CableGenesisView>, ApiError> {
    match action.as_str() {
        "begin" => {
            state.genesis.begin().map_err(ApiError::internal)?;
        }
        "continue" => {
            state
                .genesis
                .acknowledge_unobservable_trust_guidance()
                .map_err(ApiError::internal)?;
        }
        // This action deliberately records nothing. It gives an explicit
        // human acknowledgement a fresh projection of the machine-observed
        // cable state without allowing the browser to claim that a device is
        // present.
        "check" => {}
        "open_app_store" => {
            let status = std::process::Command::new("open")
                .arg("macappstore://itunes.apple.com/app/id497799835")
                .status()
                .map_err(ApiError::internal)?;
            if !status.success() {
                return Err(ApiError::internal("the App Store could not be opened"));
            }
        }
        "open_xcode_accounts" => {
            let status = std::process::Command::new("open")
                .args(["-a", "Xcode"])
                .status()
                .map_err(ApiError::internal)?;
            if !status.success() {
                return Err(ApiError::internal("Xcode could not be opened"));
            }
        }
        "install_companion" | "retry_companion" => {
            let view = current_genesis_view(&state)?;
            if view.step != crate::cable_genesis::GenesisStep::InstallCompanion
                || view.primary_action != Some(action.as_str())
            {
                return Err(ApiError::conflict(
                    "genesis_not_ready",
                    "complete the current iPhone setup action first",
                ));
            }
            let relay = state.companion.relay_health().await.map_err(|_| {
                ApiError::unavailable(
                    "TOHSENO’s private iPhone connection is unavailable. Try again shortly.",
                )
            })?;
            if !relay.is_some_and(|health| health.ready) {
                return Err(ApiError::unavailable(
                    "TOHSENO’s private iPhone connection is unavailable. Try again shortly.",
                ));
            }
            let device = match tohseno_engine::gates::device::check().map_err(ApiError::internal)? {
                tohseno_engine::gates::device::DeviceState::Ready(device) => device,
                _ => {
                    return Err(ApiError::conflict(
                        "iphone_not_ready",
                        "the connected iPhone is not ready for installation",
                    ))
                }
            };
            if action == "retry_companion" {
                let record = state.genesis.load().map_err(ApiError::internal)?;
                if record.intended_device_digest.as_deref()
                    != Some(device_digest(&device.identifier).as_str())
                {
                    return Err(ApiError::conflict(
                        "iphone_changed",
                        "connect the same iPhone that received TOHSENO",
                    ));
                }
                state
                    .genesis
                    .set_install_state(CompanionInstallState::Launching, None, None, None)
                    .map_err(ApiError::internal)?;
                tokio::spawn(pair_and_launch_companion(
                    state.genesis.clone(),
                    state.companion.clone(),
                    state.service_root.clone(),
                    state.events.clone(),
                    device,
                ));
                return current_genesis_view(&state).map(Json);
            }
            let team_id = match tohseno_engine::gates::apple_signing::check() {
                tohseno_engine::gates::apple_signing::AppleSigningState::Ready {
                    team_id, ..
                } => team_id,
                tohseno_engine::gates::apple_signing::AppleSigningState::Missing => {
                    return Err(ApiError::conflict(
                        "apple_account_not_ready",
                        "add your Apple Account in Xcode first",
                    ));
                }
            };
            state
                .genesis
                .set_install_state(
                    CompanionInstallState::Building,
                    Some(&device.identifier),
                    None,
                    None,
                )
                .map_err(ApiError::internal)?;
            let genesis = state.genesis.clone();
            let companion = state.companion.clone();
            let project = state.companion_project.clone();
            let service_root = state.service_root.clone();
            let events = state.events.clone();
            tokio::spawn(async move {
                let build_device = device.clone();
                let build_project = project.clone();
                let build_root = service_root.clone();
                let install_genesis = genesis.clone();
                let install_events = events.clone();
                let built = tokio::task::spawn_blocking(move || {
                    build_and_install_companion_with_progress(
                        &build_project,
                        &build_root,
                        &build_device,
                        &team_id,
                        move || {
                            install_genesis.set_install_state(
                                CompanionInstallState::Installing,
                                None,
                                None,
                                None,
                            )?;
                            install_events.emit(Event::status(
                                "Companion build verified; installing on the iPhone.",
                            ));
                            Ok(())
                        },
                    )
                })
                .await;
                if !matches!(built, Ok(Ok(()))) {
                    let failed_during_install = genesis.load().is_ok_and(|record| {
                        record.companion_install == CompanionInstallState::Installing
                    });
                    let message = if failed_during_install {
                        COMPANION_INSTALL_FAILURE
                    } else {
                        COMPANION_BUILD_FAILURE
                    };
                    let _ = genesis.set_install_state(
                        CompanionInstallState::Failed,
                        None,
                        None,
                        Some(message),
                    );
                    events.emit(Event::status(message));
                    return;
                }
                pair_and_launch_companion(genesis, companion, service_root, events, device).await;
            });
        }
        _ => {
            return Err(ApiError::bad(
                "invalid_genesis_action",
                "genesis action is invalid",
            ))
        }
    }
    current_genesis_view(&state).map(Json)
}

async fn pair_and_launch_companion(
    genesis: CableGenesisStore,
    companion: Arc<CompanionCoordinator>,
    service_root: PathBuf,
    events: EventBus,
    device: tohseno_engine::gates::device::Device,
) {
    let session = match companion.create_pairing_session().await {
        Ok(session) => session,
        Err(_) => {
            let _ = genesis.set_install_state(
                CompanionInstallState::Failed,
                None,
                None,
                Some(COMPANION_PAIRING_FAILURE),
            );
            events.emit(Event::status(COMPANION_PAIRING_FAILURE));
            return;
        }
    };
    let session_id = session.session_id.clone();
    let expires_at = session.expires_at.clone();
    let _ = genesis.set_install_state(
        CompanionInstallState::Launching,
        None,
        Some(&session_id),
        None,
    );
    events.emit(Event::status(
        "Companion installed; opening it on the iPhone.",
    ));
    let invitation = session.pairing_uri;
    let launched = tokio::task::spawn_blocking(move || {
        launch_companion_bootstrap(&service_root, &device, &invitation)
    })
    .await;
    if matches!(launched, Ok(Ok(()))) {
        let _ =
            genesis.set_install_state(CompanionInstallState::WaitingForPairing, None, None, None);
        events.emit(Event::status("Companion is waiting for private pairing."));
        wait_for_companion_pairing(genesis, companion, events, session_id, expires_at).await;
    } else {
        let _ = genesis.set_install_state(
            CompanionInstallState::Failed,
            None,
            None,
            Some(COMPANION_LAUNCH_FAILURE),
        );
        events.emit(Event::status("Companion launch stopped safely."));
    }
}

async fn wait_for_companion_pairing(
    genesis: CableGenesisStore,
    companion: Arc<CompanionCoordinator>,
    events: EventBus,
    session_id: String,
    expires_at: String,
) {
    let Ok(expires_at) = OffsetDateTime::parse(&expires_at, &Rfc3339) else {
        return;
    };
    loop {
        if companion
            .devices()
            .is_ok_and(|devices| devices.iter().any(|device| !device.revoked))
        {
            return;
        }
        let Ok(record) = genesis.load() else {
            return;
        };
        if record.companion_install != CompanionInstallState::WaitingForPairing
            || record.pairing_session_id.as_deref() != Some(session_id.as_str())
        {
            return;
        }
        if OffsetDateTime::now_utc() > expires_at {
            let _ = genesis.set_install_state(
                CompanionInstallState::Failed,
                None,
                None,
                Some(COMPANION_PAIRING_FAILURE),
            );
            events.emit(Event::status(COMPANION_PAIRING_FAILURE));
            return;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

async fn shots(State(state): State<Arc<WorkspaceState>>) -> Result<Json<Value>, ApiError> {
    let snapshot = state
        .application
        .workspace_snapshot()
        .await
        .map_err(ApiError::application)?;
    Ok(Json(json!({
        "schema": "tohseno.local-shot-list/1",
        "shots": snapshot.shots,
    })))
}

async fn retire_shot(
    State(state): State<Arc<WorkspaceState>>,
    AxumPath(shot_id): AxumPath<String>,
) -> Result<Json<Value>, ApiError> {
    state
        .application
        .retire_shot(&shot_id)
        .await
        .map(|receipt| Json(json!(receipt)))
        .map_err(ApiError::retire_application)
}

async fn restore_shot(
    State(state): State<Arc<WorkspaceState>>,
    AxumPath(shot_id): AxumPath<String>,
) -> Result<Json<Value>, ApiError> {
    state
        .application
        .restore_shot(&shot_id)
        .await
        .map(|receipt| Json(json!(receipt)))
        .map_err(ApiError::retire_application)
}

async fn shot_icon(
    State(state): State<Arc<WorkspaceState>>,
    AxumPath(shot_id): AxumPath<String>,
) -> Result<Response, ApiError> {
    let icon = state
        .application
        .shot_icon(&shot_id)
        .map_err(ApiError::application)?
        .ok_or_else(|| ApiError::not_found("app does not exist"))?;
    if !matches!(icon.media_type.as_str(), "image/png" | "image/jpeg")
        || icon.private_bytes.is_empty()
        || u64::try_from(icon.private_bytes.len()).ok() != Some(icon.byte_length)
    {
        return Err(ApiError::internal("app icon is unavailable"));
    }
    let content_type = HeaderValue::from_str(&icon.media_type)
        .map_err(|_| ApiError::internal("app icon media type is invalid"))?;
    let mut response = Response::new(Body::from(icon.private_bytes));
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, content_type);
    Ok(response)
}

/// The owner's read-only disclosure for one app's latest execution.
///
/// Loopback and non-mutating, so it needs no anti-CSRF token; it is still
/// private material and is never rendered on the normal Create/Evolve path.
async fn shot_receipt(
    State(state): State<Arc<WorkspaceState>>,
    AxumPath(shot_id): AxumPath<String>,
) -> Result<Json<Value>, ApiError> {
    let receipt = state
        .application
        .execution_receipt(&shot_id)
        .map_err(ApiError::application)?
        .ok_or_else(|| ApiError::not_found("this app has no execution to explain yet"))?;
    Ok(Json(serde_json::to_value(receipt).map_err(|_| {
        ApiError::internal("the execution receipt could not be rendered")
    })?))
}

/// Durable semantic progress, bounded owner-local file names, and metered
/// usage for the native Build surface. Raw harness output remains in the
/// private on-disk operational log.
async fn shot_activity(
    State(state): State<Arc<WorkspaceState>>,
    AxumPath(shot_id): AxumPath<String>,
) -> Result<Json<Value>, ApiError> {
    let activity = state
        .application
        .execution_activity(&shot_id)
        .map_err(ApiError::application)?
        .ok_or_else(|| ApiError::not_found("this app has no execution activity yet"))?;
    Ok(Json(serde_json::to_value(activity).map_err(|_| {
        ApiError::internal("the execution activity could not be rendered")
    })?))
}

async fn shot_preview(
    State(state): State<Arc<WorkspaceState>>,
    AxumPath(shot_id): AxumPath<String>,
) -> Result<Response, ApiError> {
    let preview = state
        .application
        .shot_preview(&shot_id)
        .map_err(ApiError::application)?
        .ok_or_else(|| ApiError::not_found("app preview does not exist"))?;
    if preview.media_type != "image/png"
        || preview.private_bytes.is_empty()
        || u64::try_from(preview.private_bytes.len()).ok() != Some(preview.byte_length)
    {
        return Err(ApiError::internal("app preview is unavailable"));
    }
    let mut response = Response::new(Body::from(preview.private_bytes));
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static("image/png"));
    Ok(response)
}

async fn open_shot_source(
    State(state): State<Arc<WorkspaceState>>,
    AxumPath(shot_id): AxumPath<String>,
    Json(_empty): Json<EmptyRequest>,
) -> Result<Json<Value>, ApiError> {
    let (name, _) = resolve_shot(&state.application, &shot_id)?;
    let source = state.application.engine().ledger().working_tree(&name);
    let metadata = fs::symlink_metadata(&source).map_err(ApiError::internal)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ApiError::internal("the app source folder is unsafe"));
    }
    let status = std::process::Command::new("open")
        .arg("--")
        .arg(&source)
        .status()
        .map_err(ApiError::internal)?;
    if !status.success() {
        return Err(ApiError::internal(
            "the app source folder could not be opened",
        ));
    }
    Ok(Json(json!({
        "schema": "tohseno.open-source-receipt/1",
        "opened": true,
    })))
}

async fn open_shot_on_iphone(
    State(state): State<Arc<WorkspaceState>>,
    AxumPath(shot_id): AxumPath<String>,
    Json(_empty): Json<EmptyRequest>,
) -> Result<Json<Value>, ApiError> {
    let (name, _) = resolve_shot(&state.application, &shot_id)?;
    let app = state
        .application
        .engine()
        .ledger()
        .load_app(&name)
        .map_err(ApiError::internal)?;
    let device = match tohseno_engine::gates::device::check().map_err(ApiError::internal)? {
        tohseno_engine::gates::device::DeviceState::Ready(device) => device,
        _ => {
            return Err(ApiError::conflict(
                "iphone_not_ready",
                "connect and unlock your iPhone, then try again",
            ));
        }
    };
    tohseno_engine::gates::install::launch(&device, &app.bundle_id)
        .map_err(|error| ApiError::unavailable(error.to_string()))?;
    Ok(Json(json!({
        "schema": "tohseno.open-on-iphone-receipt/1",
        "opened": true,
    })))
}

async fn pending_intention(
    State(state): State<Arc<WorkspaceState>>,
    AxumPath(pending_id): AxumPath<String>,
) -> Result<Json<Value>, ApiError> {
    validate_pending_id(&pending_id)?;
    let store = PendingIntentionStore::for_ledger(state.application.engine().ledger());
    let pending = store.load(&pending_id).map_err(|_| {
        ApiError::not_found("pending intention does not exist or was already consumed")
    })?;
    let mut references = Vec::with_capacity(pending.references.len());
    for reference in &pending.references {
        let bytes = store
            .read_reference(&pending_id, reference.ordinal)
            .map_err(ApiError::internal)?;
        references.push(json!({
            "filename": reference.display_filename,
            "media_type": reference.media_type,
            "origin": pending_reference_origin(&pending_id, reference.ordinal),
            "bytes_base64url": URL_SAFE_NO_PAD.encode(bytes),
        }));
    }
    Ok(Json(json!({
        "schema": "tohseno.local-pending-intention-view/1",
        "pending_intention_id": pending.id,
        "suggested_name": suggest_pending_name(
            &pending.prompt,
            state.application.engine().ledger(),
        )?,
        "intention": pending.prompt,
        "references": references,
    })))
}

async fn create_shot(
    State(state): State<Arc<WorkspaceState>>,
    Json(request): Json<CreateRequest>,
) -> Result<Json<Value>, ApiError> {
    validate_text("intention", &request.intention, MAX_INTENTION_BYTES)?;
    let references = decode_references(request.references)?;
    let harness_selection = match (
        request.harness.as_deref(),
        request.model.as_deref(),
        request.managed.as_ref(),
    ) {
        (None, None, Some(managed)) => Some(
            managed_selection(
                &state,
                &request.command_id,
                managed,
                request.intention.len() as u64,
                references
                    .iter()
                    .map(|reference| reference.bytes.len() as u64)
                    .sum(),
                0,
            )
            .await?,
        ),
        (Some(harness), model, None) => Some(
            state
                .application
                .harness_selection(harness, model.unwrap_or("default"))
                .map_err(ApiError::application)?,
        ),
        (None, Some(_), None) => {
            return Err(ApiError::bad(
                "invalid_harness_selection",
                "a model can only be selected with its coding harness",
            ));
        }
        (None, None, None) => None,
        _ => {
            return Err(ApiError::bad(
                "ambiguous_intelligence_selection",
                "choose either one local/BYO route or explicit managed compute",
            ));
        }
    };
    let (name, name_was_supplied) = match request.name.as_deref().map(str::trim) {
        Some(value) if !value.is_empty() => (normalize_name(value)?, true),
        _ => (
            derive_technical_name(&request.intention, state.application.engine().ledger())?,
            false,
        ),
    };
    let pending = match request.pending_intention_id.as_deref() {
        Some(pending_id) => {
            validate_pending_id(pending_id)?;
            let store = PendingIntentionStore::for_ledger(state.application.engine().ledger());
            let pending = store.load(pending_id).map_err(|_| {
                ApiError::not_found("pending intention does not exist or was already consumed")
            })?;
            validate_pending_submission(&store, &pending, &request.intention, &references)?;
            Some(pending)
        }
        None => None,
    };
    let command_id = pending
        .as_ref()
        .map(|pending| format!("pending_intention_{}", pending.id))
        .unwrap_or(request.command_id);
    let receipt = state
        .application
        .create_shot(CreateShotCommand {
            command_id,
            origin: request.origin.command_origin(),
            origin_device_id: None,
            name,
            name_was_supplied,
            intention: request.intention,
            references,
            submitted_at: None,
            harness_selection,
        })
        .await
        .map_err(ApiError::application)?;
    if let Some(pending) = pending {
        PendingIntentionStore::for_ledger(state.application.engine().ledger())
            .consume_loaded(&pending)
            .map_err(ApiError::internal)?;
    }
    Ok(Json(json!(receipt)))
}

async fn evolve_shot(
    State(state): State<Arc<WorkspaceState>>,
    AxumPath(shot_id): AxumPath<String>,
    Json(request): Json<EvolveRequest>,
) -> Result<Json<Value>, ApiError> {
    let (name, parsed_shot) = resolve_shot(&state.application, &shot_id)?;
    let references = decode_references(request.references)?;
    let source_context_bytes =
        bounded_source_bytes(&state.application.engine().ledger().working_tree(&name))
            .map_err(ApiError::internal)?;
    let harness_selection = match (
        request.harness.as_deref(),
        request.model.as_deref(),
        request.managed.as_ref(),
    ) {
        (None, None, Some(managed)) => Some(
            managed_selection(
                &state,
                &request.command_id,
                managed,
                request.intention.len() as u64,
                references
                    .iter()
                    .map(|reference| reference.bytes.len() as u64)
                    .sum(),
                source_context_bytes,
            )
            .await?,
        ),
        (Some(harness), model, None) => Some(
            state
                .application
                .harness_selection(harness, model.unwrap_or("default"))
                .map_err(ApiError::application)?,
        ),
        (None, Some(_), None) => {
            return Err(ApiError::bad(
                "invalid_harness_selection",
                "a model can only be selected with its coding harness",
            ));
        }
        (None, None, None) => None,
        _ => {
            return Err(ApiError::bad(
                "ambiguous_intelligence_selection",
                "choose either one local/BYO route or explicit managed compute",
            ));
        }
    };
    let selected_feedback_actions = request
        .selected_feedback_actions
        .iter()
        .map(|value| Bytes32::from_hex("feedback action commitment", value))
        .collect::<Result<Vec<_>, _>>()
        .map_err(ApiError::internal)?;
    if request.intention.trim().is_empty() {
        if selected_feedback_actions.is_empty() {
            return Err(ApiError::bad(
                "invalid_evolution",
                "evolution requires an intention or selected Feedback action",
            ));
        }
    } else {
        validate_text("intention", &request.intention, MAX_INTENTION_BYTES)?;
    }
    state
        .application
        .evolve_shot(EvolveShotCommand {
            command_id: request.command_id,
            origin: request.origin.command_origin(),
            origin_device_id: None,
            name,
            base_expression_id: parse_expression_id(&request.base_expression_id)?,
            base_version_id: parse_version_id(&request.base_version_id)?,
            base_version_ordinal: request.base_version_ordinal,
            intention: request.intention,
            selected_feedback_actions,
            references,
            submitted_at: None,
            harness_selection,
        })
        .await
        .map(|receipt| {
            debug_assert_eq!(receipt.shot_id, parsed_shot);
            Json(json!(receipt))
        })
        .map_err(ApiError::application)
}

async fn executions(State(state): State<Arc<WorkspaceState>>) -> Result<Json<Value>, ApiError> {
    let snapshot = state
        .application
        .workspace_snapshot()
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(json!({
        "schema": "tohseno.local-execution-list/1",
        "executions": snapshot.active_executions,
    })))
}

async fn execution(
    State(state): State<Arc<WorkspaceState>>,
    AxumPath(execution_id): AxumPath<String>,
) -> Result<Json<Value>, ApiError> {
    validate_identifier(&execution_id)?;
    for app in state
        .application
        .engine()
        .ledger()
        .list_apps()
        .map_err(ApiError::internal)?
    {
        let repository = state.application.engine().ledger().working_tree(&app.name);
        let Ok(record) = load_execution(&repository, &execution_id) else {
            continue;
        };
        let completion = load_completion(&repository, &execution_id).map_err(ApiError::internal)?;
        // Publishing a completion record and advancing the durable execution
        // phase are two atomic writes. Never expose the narrow interval between
        // them as completion: callers using --wait must observe the terminal
        // phase, not merely the first of those writes.
        let terminal = execution_is_terminal(record.phase);
        let events = read_events(&repository, &execution_id).unwrap_or_default();
        let started_at = events
            .first()
            .map(|event| event.timestamp.as_str())
            .unwrap_or(record.prepared_at.as_str());
        let updated_at = events
            .last()
            .map(|event| event.timestamp.as_str())
            .unwrap_or(record.prepared_at.as_str());
        let elapsed_until = if terminal && completion.is_some() {
            updated_at.to_owned()
        } else {
            now()
        };
        return Ok(Json(json!({
            "schema": "tohseno.local-execution-status/1",
            "execution_id": record.execution_id,
            "shot_id": record.shot_id,
            "version_ordinal": record.version_ordinal,
            "state": privacy_safe_phase(record.phase),
            "started_at": started_at,
            "updated_at": updated_at,
            "elapsed_seconds": elapsed_seconds_between(started_at, &elapsed_until),
            "complete": terminal && completion.is_some(),
            "accepted": terminal && completion.as_ref().is_some_and(|value| {
                value.landed && value.outcome == tohseno_engine::ExecutionOutcome::Completed
            }),
        })));
    }
    Err(ApiError::not_found("execution does not exist"))
}

async fn events(
    State(state): State<Arc<WorkspaceState>>,
) -> Sse<impl Stream<Item = Result<SseEvent, Infallible>>> {
    let mut receiver = state.events.subscribe();
    let cursor = state.event_cursor.clone();
    let output = stream! {
        loop {
            match receiver.recv().await {
                Ok(Event::HarnessLine(_)) => continue,
                Ok(_) => {
                    let id = cursor.fetch_add(1, Ordering::Relaxed);
                    let data = json!({
                        "schema": "tohseno.local-workspace-event/1",
                        "event_id": format!("event_{id}"),
                        "event": "workspace.changed",
                    });
                    yield Ok(SseEvent::default().id(id.to_string()).event("workspace.changed").data(data.to_string()));
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    let id = cursor.fetch_add(1, Ordering::Relaxed);
                    let data = json!({
                        "schema": "tohseno.local-workspace-event/1",
                        "event_id": format!("event_{id}"),
                        "event": "workspace.reconcile",
                    });
                    yield Ok(SseEvent::default().id(id.to_string()).event("workspace.reconcile").data(data.to_string()));
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    };
    Sse::new(output).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keepalive"),
    )
}

async fn create_pairing_session(
    State(state): State<Arc<WorkspaceState>>,
    Json(_empty): Json<EmptyRequest>,
) -> Result<Json<PairingSessionView>, ApiError> {
    state
        .companion
        .create_pairing_session()
        .await
        .map(Json)
        .map_err(ApiError::internal)
}

async fn pairing_session(
    State(state): State<Arc<WorkspaceState>>,
    AxumPath(session_id): AxumPath<String>,
) -> Result<Json<PairingSessionView>, ApiError> {
    state
        .companion
        .pairing_session(&session_id)
        .await
        .map(Json)
        .map_err(ApiError::internal)
}

async fn cancel_pairing_session(
    State(state): State<Arc<WorkspaceState>>,
    AxumPath(session_id): AxumPath<String>,
) -> Result<StatusCode, ApiError> {
    state
        .companion
        .cancel_pairing_session(&session_id)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(ApiError::internal)
}

async fn complete_pairing(
    State(state): State<Arc<WorkspaceState>>,
    AxumPath(session_id): AxumPath<String>,
    Json(proof): Json<tohseno_companion::pairing::PairingProof>,
) -> Result<Json<PairingCompletion>, ApiError> {
    let completion = state
        .companion
        .complete_pairing(&session_id, proof)
        .await
        .map_err(ApiError::internal)?;
    state
        .events
        .emit(Event::status("Companion pairing state changed."));
    Ok(Json(completion))
}

async fn devices(State(state): State<Arc<WorkspaceState>>) -> Result<Json<Value>, ApiError> {
    let devices = state.companion.devices().map_err(ApiError::internal)?;
    Ok(Json(json!({
        "schema": "tohseno.companion-device-list/1",
        "devices": devices,
    })))
}

async fn revoke_device(
    State(state): State<Arc<WorkspaceState>>,
    AxumPath(device_id): AxumPath<String>,
) -> Result<Json<Value>, ApiError> {
    state
        .companion
        .revoke(&device_id)
        .await
        .map(|device| Json(json!(device)))
        .map_err(ApiError::internal)
}

async fn companion_status(
    State(state): State<Arc<WorkspaceState>>,
) -> Result<Json<Value>, ApiError> {
    let devices = state.companion.devices().map_err(ApiError::internal)?;
    let relay = state
        .companion
        .relay_health()
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(json!({
        "schema": "tohseno.companion-status/1",
        "workspace_id": state.companion.workspace_id(),
        "paired_devices": devices.iter().filter(|device| !device.revoked).count(),
        "revoked_devices": devices.iter().filter(|device| device.revoked).count(),
        "relay_id": "official-v1",
        "relay_connection": if relay.as_ref().is_some_and(|health| health.ready) { "ready" } else if relay.is_some() { "not_ready" } else { "configuration_required" },
        "relay": relay,
    })))
}

async fn simulate_envelope(
    State(state): State<Arc<WorkspaceState>>,
    Json(envelope): Json<tohseno_companion::envelope::OpaqueEnvelope>,
) -> Result<Json<Value>, ApiError> {
    state
        .companion
        .process_envelope(&envelope)
        .await
        .map(|receipt| Json(json!(receipt)))
        .map_err(ApiError::internal)
}

async fn studio_index() -> Html<&'static str> {
    Html(include_str!("../../studio/index.html"))
}

async fn studio_javascript() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("../../studio/app.js"),
    )
}

async fn studio_stylesheet() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../../studio/style.css"),
    )
}

async fn studio_logo() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "image/png"),
            (header::CACHE_CONTROL, "public, max-age=86400, immutable"),
        ],
        include_bytes!("../../brand/logos/tohseno-logo-final.png").as_slice(),
    )
}

async fn studio_pairing_seal() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "image/png"),
            (header::CACHE_CONTROL, "public, max-age=86400, immutable"),
        ],
        include_bytes!("../../brand/logos/tohseno-core-circle.png").as_slice(),
    )
}

pub fn runtime_path(service_root: &Path) -> PathBuf {
    service_root.join("runtime.json")
}

pub fn load_runtime(service_root: &Path) -> Result<RuntimeRecord, BoxError> {
    let bytes = read_bounded(&runtime_path(service_root), MAX_RUNTIME_BYTES)?;
    let runtime: RuntimeRecord = tohseno_protocol::canonical::from_slice(&bytes)?;
    validate_runtime(&runtime)?;
    Ok(runtime)
}

fn validate_runtime(runtime: &RuntimeRecord) -> Result<(), BoxError> {
    if runtime.schema != RUNTIME_SCHEMA
        || runtime.service_version.is_empty()
        || !runtime.workspace_id.starts_with("workspace_")
        || !runtime.studio_device_id.starts_with("device_")
        || runtime.origin != format!("http://127.0.0.1:{}", runtime.port)
        || runtime.port == 0
        || runtime.process_id == 0
        || runtime.csrf_token.len() < 32
    {
        return Err("Local Workspace Service runtime record is invalid".into());
    }
    tohseno_companion::parse_timestamp(&runtime.started_at)?;
    Ok(())
}

fn publish_runtime(service_root: &Path, runtime: &RuntimeRecord) -> Result<(), BoxError> {
    validate_runtime(runtime)?;
    let bytes = tohseno_protocol::canonical::to_vec(runtime)?;
    write_replace(&runtime_path(service_root), &bytes, 0o600)
}

fn remove_runtime_if_current(service_root: &Path, instance_id: &str) -> Result<(), BoxError> {
    let path = runtime_path(service_root);
    match load_runtime(service_root) {
        Ok(runtime) if runtime.instance_id == instance_id => {
            fs::remove_file(path)?;
            File::open(service_root)?.sync_all()?;
        }
        Ok(_) => {}
        Err(error)
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound) => {}
        Err(error) => return Err(error),
    }
    Ok(())
}

fn acquire_service_lock(path: &Path) -> Result<File, BoxError> {
    let mut options = OpenOptions::new();
    options.create(true).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let file = options.open(path)?;
    let outcome = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if outcome != 0 {
        return Err("another Local Workspace Service process already owns this workspace".into());
    }
    Ok(file)
}

fn resolve_shot(
    application: &ShotApplicationService,
    value: &str,
) -> Result<(String, ShotId), ApiError> {
    let shot_id = parse_shot_id(value)?;
    for app in application
        .engine()
        .ledger()
        .list_apps()
        .map_err(ApiError::internal)?
    {
        if app.shot_id == Some(shot_id) {
            return Ok((app.name, shot_id));
        }
    }
    Err(ApiError::not_found("Shot does not exist"))
}

fn parse_shot_id(value: &str) -> Result<ShotId, ApiError> {
    Bytes32::from_hex("Shot ID", value)
        .map(|bytes| ShotId::from_bytes(bytes.into_bytes()))
        .map_err(ApiError::internal)
}

fn parse_expression_id(value: &str) -> Result<ExpressionId, ApiError> {
    Bytes32::from_hex("Expression ID", value)
        .map(|bytes| ExpressionId::from_bytes(bytes.into_bytes()))
        .map_err(ApiError::internal)
}

fn parse_version_id(value: &str) -> Result<VersionId, ApiError> {
    Bytes32::from_hex("Version ID", value)
        .map(|bytes| VersionId::from_bytes(bytes.into_bytes()))
        .map_err(ApiError::internal)
}

fn normalize_name(value: &str) -> Result<String, ApiError> {
    let value = value.to_ascii_lowercase();
    tohseno_engine::ledger::validate_app_name(&value).map_err(ApiError::internal)?;
    Ok(value)
}

fn validate_pending_id(value: &str) -> Result<(), ApiError> {
    if value.len() != 32
        || value
            .bytes()
            .any(|byte| !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase())
    {
        Err(ApiError::bad(
            "invalid_pending_intention",
            "pending intention ID is invalid",
        ))
    } else {
        Ok(())
    }
}

fn pending_reference_origin(pending_id: &str, ordinal: usize) -> String {
    format!("pending:{pending_id}:{ordinal}")
}

fn suggest_pending_name(prompt: &str, ledger: &tohseno_engine::Ledger) -> Result<String, ApiError> {
    derive_technical_name(prompt, ledger)
}

fn derive_technical_name(
    prompt: &str,
    ledger: &tohseno_engine::Ledger,
) -> Result<String, ApiError> {
    const STOP: &[&str] = &[
        "a",
        "an",
        "and",
        "app",
        "application",
        "build",
        "create",
        "every",
        "for",
        "i",
        "iphone",
        "make",
        "me",
        "my",
        "native",
        "need",
        "of",
        "on",
        "only",
        "please",
        "simple",
        "that",
        "the",
        "to",
        "want",
        "with",
    ];
    let mut words = prompt
        .split(|character: char| !character.is_ascii_alphanumeric())
        .map(str::to_ascii_lowercase)
        .filter(|word| word.len() >= 2 && !STOP.contains(&word.as_str()))
        .take(3)
        .collect::<Vec<_>>();
    if words.is_empty() {
        words.push("new-intention".into());
    }
    let mut base = words.join("-");
    base.truncate(48);
    base = base.trim_matches('-').to_owned();
    if base.is_empty() || !base.as_bytes()[0].is_ascii_alphanumeric() {
        base = "new-intention".into();
    }
    let existing = ledger
        .list_apps()
        .map_err(ApiError::internal)?
        .into_iter()
        .map(|app| app.name)
        .collect::<std::collections::BTreeSet<_>>();
    if !existing.contains(&base) && tohseno_engine::ledger::validate_app_name(&base).is_ok() {
        return Ok(base);
    }
    let digest = tohseno_protocol::digest::sha256(prompt.as_bytes()).to_hex();
    for suffix in std::iter::once(digest[2..8].to_owned())
        .chain((2..=999).map(|ordinal| format!("{}-{ordinal}", &digest[2..8])))
    {
        let maximum_base = 62usize.saturating_sub(suffix.len());
        let candidate_base = base
            .get(..base.len().min(maximum_base))
            .unwrap_or(&base)
            .trim_end_matches('-');
        let candidate = format!("{candidate_base}-{suffix}");
        if !existing.contains(&candidate)
            && tohseno_engine::ledger::validate_app_name(&candidate).is_ok()
        {
            return Ok(candidate);
        }
    }
    Err(ApiError::internal(
        "could not reserve a unique technical app name",
    ))
}

fn validate_pending_submission(
    store: &PendingIntentionStore,
    pending: &LocalPendingIntention,
    intention: &str,
    references: &[ReferenceInput],
) -> Result<(), ApiError> {
    if intention != pending.prompt || references.len() != pending.references.len() {
        return Err(ApiError::bad(
            "pending_intention_mismatch",
            "submitted creation does not exactly match the pending intention",
        ));
    }
    for (expected, submitted) in pending.references.iter().zip(references) {
        let bytes = store
            .read_reference(&pending.id, expected.ordinal)
            .map_err(ApiError::internal)?;
        if submitted.display_filename != expected.display_filename
            || submitted.media_type != expected.media_type
            || submitted.origin != pending_reference_origin(&pending.id, expected.ordinal)
            || submitted.bytes != bytes
        {
            return Err(ApiError::bad(
                "pending_intention_mismatch",
                "submitted creation does not exactly match the pending intention",
            ));
        }
    }
    Ok(())
}

fn decode_references(values: Vec<ApiReference>) -> Result<Vec<ReferenceInput>, ApiError> {
    if values.len() > 8 {
        return Err(ApiError::bad(
            "too_many_references",
            "at most eight reference images are accepted",
        ));
    }
    let mut total = 0_usize;
    values
        .into_iter()
        .map(|value| {
            if value.filename.contains('/') || value.filename.contains('\\') {
                return Err(ApiError::bad(
                    "unsafe_reference_name",
                    "reference filename must not be a path",
                ));
            }
            let bytes = URL_SAFE_NO_PAD
                .decode(&value.bytes_base64url)
                .map_err(|_| {
                    ApiError::bad(
                        "invalid_reference",
                        "reference bytes are not canonical base64url",
                    )
                })?;
            if URL_SAFE_NO_PAD.encode(&bytes) != value.bytes_base64url
                || bytes.len() > 64 * 1024 * 1024
            {
                return Err(ApiError::bad(
                    "invalid_reference",
                    "reference bytes are invalid or too large",
                ));
            }
            total = total.saturating_add(bytes.len());
            if total > MAX_REFERENCE_TOTAL_BYTES {
                return Err(ApiError::bad(
                    "references_too_large",
                    "reference images exceed the combined byte limit",
                ));
            }
            Ok(ReferenceInput {
                display_filename: value.filename,
                media_type: value.media_type,
                origin: value.origin,
                bytes,
            })
        })
        .collect()
}

fn validate_text(label: &str, value: &str, maximum: usize) -> Result<(), ApiError> {
    if value.trim().is_empty() || value.len() > maximum {
        Err(ApiError::bad(
            "invalid_text",
            format!("{label} must contain 1..={maximum} UTF-8 bytes"),
        ))
    } else {
        Ok(())
    }
}

fn validate_identifier(value: &str) -> Result<(), ApiError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        Err(ApiError::bad("invalid_identifier", "identifier is invalid"))
    } else {
        Ok(())
    }
}

fn privacy_safe_phase(phase: ExecutionPhase) -> &'static str {
    match phase {
        ExecutionPhase::Prepared
        | ExecutionPhase::RunnerStarted
        | ExecutionPhase::TerminalOpened => "queued",
        ExecutionPhase::ExecutionStarted | ExecutionPhase::ContextLoaded => "planning",
        ExecutionPhase::Conception => "conception",
        ExecutionPhase::Materializing
        | ExecutionPhase::HarnessRunning
        | ExecutionPhase::WorkspaceChanged => "materializing",
        ExecutionPhase::Building => "building",
        ExecutionPhase::Testing => "testing",
        ExecutionPhase::Verifying | ExecutionPhase::ValidationStarted => "verifying",
        ExecutionPhase::Repairing => "repairing",
        ExecutionPhase::Installing => "installing",
        ExecutionPhase::Launching => "launching",
        ExecutionPhase::WaitingForDevice => "waiting_for_device",
        ExecutionPhase::ValidationCompleted => "verifying",
        ExecutionPhase::ExecutionCompleted => "accepted",
        ExecutionPhase::ExecutionFailed => "failed",
        ExecutionPhase::ExecutionCancelled => "cancelled",
    }
}

fn execution_is_terminal(phase: ExecutionPhase) -> bool {
    matches!(
        phase,
        ExecutionPhase::ExecutionCompleted
            | ExecutionPhase::ExecutionFailed
            | ExecutionPhase::ExecutionCancelled
    )
}

/// Reconciliation passes run every two seconds; a full snapshot rebuild is
/// forced at most this many passes apart even when nothing appears to change.
const WORKSPACE_SNAPSHOT_BACKSTOP_TICKS: u32 = 15;
/// Private per-app state lives directly under the app's metadata root, so the
/// fingerprint never descends into the source tree the harness writes.
const FINGERPRINT_MAX_DEPTH: usize = 3;
/// Bounds the stat cost per app so an unusual metadata tree cannot make the
/// cheap pass as expensive as the rebuild it exists to avoid.
const FINGERPRINT_MAX_ENTRIES: usize = 256;

/// A stat-only fingerprint of the private per-app state behind the workspace
/// snapshot. Metadata is published by atomic replacement, so a changed record
/// moves either its own timestamp or its parent directory's. This reads no
/// file contents and therefore carries no private material.
fn workspace_change_fingerprint(engine: &Engine) -> Option<Bytes32> {
    let ledger = engine.ledger();
    let mut stamps = String::new();
    for app in ledger.list_apps().ok()? {
        let root = ledger.working_tree(&app.name);
        stamp_path(&root, &mut stamps);
        let mut budget = FINGERPRINT_MAX_ENTRIES;
        stamp_tree(
            &ShotLayout::at(root).metadata_root(),
            0,
            &mut budget,
            &mut stamps,
        );
    }
    Some(tohseno_protocol::digest::sha256(stamps.as_bytes()))
}

fn stamp_path(path: &Path, output: &mut String) {
    use std::fmt::Write as _;
    let Ok(metadata) = fs::symlink_metadata(path) else {
        let _ = write!(output, "{}\u{0}absent\u{1}", path.display());
        return;
    };
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|since| since.as_nanos())
        .unwrap_or_default();
    let _ = write!(
        output,
        "{}\u{0}{modified}\u{0}{}\u{1}",
        path.display(),
        metadata.len()
    );
}

fn stamp_tree(directory: &Path, depth: usize, budget: &mut usize, output: &mut String) {
    stamp_path(directory, output);
    if depth >= FINGERPRINT_MAX_DEPTH || *budget == 0 {
        return;
    }
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    let mut children: Vec<PathBuf> = entries
        .take(*budget)
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .collect();
    children.sort();
    for path in children {
        if *budget == 0 {
            return;
        }
        *budget -= 1;
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_dir() => stamp_tree(&path, depth + 1, budget, output),
            _ => stamp_path(&path, output),
        }
    }
}

fn privacy_safe_workspace_digest(
    snapshot: &tohseno_application::WorkspaceSnapshot,
) -> Result<Bytes32, tohseno_protocol::ProtocolError> {
    #[derive(Serialize)]
    struct Projection<'a> {
        service_version: &'a str,
        shots: &'a [tohseno_application::ShotSummary],
        active_executions: &'a [tohseno_application::ExecutionSummary],
    }
    let bytes = tohseno_protocol::canonical::to_vec(&Projection {
        service_version: &snapshot.service_version,
        shots: &snapshot.shots,
        active_executions: &snapshot.active_executions,
    })?;
    Ok(tohseno_protocol::digest::sha256(&bytes))
}

fn random_token(bytes: usize) -> String {
    let mut value = vec![0_u8; bytes];
    OsRng.fill_bytes(&mut value);
    URL_SAFE_NO_PAD.encode(value)
}

fn now() -> String {
    OffsetDateTime::now_utc()
        .replace_nanosecond(0)
        .expect("zero nanoseconds")
        .format(&Rfc3339)
        .expect("UTC timestamp")
}

async fn shutdown_signal() {
    // launchd stops and restarts this service with SIGTERM, so handling only
    // SIGINT would skip graceful shutdown on the exact path the LaunchAgent
    // uses and strand a runtime record naming a process that no longer exists.
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::terminate()) {
            Ok(mut terminate) => {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = terminate.recv() => {}
                }
            }
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

fn rotate_operational_logs(logs: &Path) -> Result<(), BoxError> {
    ensure_private_directory(logs)?;
    for name in ["workspace-service.log", "workspace-service.error.log"] {
        let path = logs.join(name);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                return Err("service log path is unsafe".into());
            }
            Ok(metadata) if metadata.len() > 5 * 1024 * 1024 => {
                let rotated = logs.join(format!("{name}.previous"));
                if let Ok(existing) = fs::symlink_metadata(&rotated) {
                    if existing.file_type().is_symlink() || !existing.is_file() {
                        return Err("rotated service log path is unsafe".into());
                    }
                    fs::remove_file(&rotated)?;
                }
                fs::rename(&path, &rotated)?;
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn ensure_private_directory(path: &Path) -> Result<(), BoxError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err("service directory path is unsafe".into());
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir_all(path)?,
        Err(error) => return Err(error.into()),
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn read_bounded(path: &Path, maximum: u64) -> Result<Vec<u8>, BoxError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > maximum {
        return Err("service record is not a bounded regular file".into());
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
    if !opened.is_file() || opened.len() != metadata.len() {
        return Err("service record changed while opening".into());
    }
    let mut bytes = Vec::with_capacity(usize::try_from(opened.len()).unwrap_or(0));
    file.take(maximum + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > maximum {
        return Err("service record exceeds its bound".into());
    }
    Ok(bytes)
}

fn write_new(path: &Path, bytes: &[u8], mode: u32) -> Result<(), BoxError> {
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(mode)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    File::open(path.parent().ok_or("service record has no parent")?)?.sync_all()?;
    Ok(())
}

fn write_replace(path: &Path, bytes: &[u8], mode: u32) -> Result<(), BoxError> {
    let parent = path.parent().ok_or("service record has no parent")?;
    if let Ok(metadata) = fs::symlink_metadata(path) {
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err("service record target is unsafe".into());
        }
    }
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("record"),
        Uuid::new_v4().simple()
    ));
    write_new(&temporary, bytes, mode)?;
    fs::rename(&temporary, path)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

fn boxed(error: Box<dyn std::error::Error>) -> BoxError {
    error.to_string().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_api_rejects_oversized_or_excessive_headers() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("127.0.0.1:8888"));
        assert!(headers_within_bounds(&headers));

        headers.insert(
            "x-oversized",
            HeaderValue::from_bytes(&vec![b'a'; MAX_SINGLE_HEADER_BYTES + 1]).unwrap(),
        );
        assert!(!headers_within_bounds(&headers));

        let mut excessive = axum::http::HeaderMap::new();
        for index in 0..=MAX_HEADER_COUNT {
            let name = axum::http::HeaderName::from_bytes(format!("x-fixture-{index}").as_bytes())
                .unwrap();
            excessive.insert(name, HeaderValue::from_static("value"));
        }
        assert!(!headers_within_bounds(&excessive));
    }

    #[test]
    fn runtime_rejects_non_loopback_origins() {
        let runtime = RuntimeRecord {
            schema: RUNTIME_SCHEMA.into(),
            service_version: "0.9.0".into(),
            workspace_id: "workspace_fixture".into(),
            studio_device_id: "device_fixture".into(),
            origin: "http://0.0.0.0:8888".into(),
            port: 8888,
            process_id: 1,
            started_at: "2026-08-15T12:00:00Z".into(),
            instance_id: "service_fixture".into(),
            csrf_token: "x".repeat(32),
        };
        assert!(validate_runtime(&runtime).is_err());
    }

    #[test]
    fn reference_decoder_preserves_exact_bytes_and_origin() {
        let bytes = vec![0_u8, 1, 2, 255];
        let decoded = decode_references(vec![ApiReference {
            filename: "reference.png".into(),
            media_type: "image/png".into(),
            origin: "/private/input/reference.png".into(),
            bytes_base64url: URL_SAFE_NO_PAD.encode(&bytes),
        }])
        .unwrap();
        assert_eq!(decoded[0].bytes, bytes);
        assert_eq!(decoded[0].origin, "/private/input/reference.png");
    }

    #[test]
    fn pending_submission_requires_the_exact_imported_material() {
        let root = tempfile::tempdir().unwrap();
        let ledger = tohseno_engine::Ledger::at(root.path().join("data"));
        ledger.initialize().unwrap();
        let store = PendingIntentionStore::for_ledger(&ledger);
        let image = b"\x89PNG\r\n\x1a\nfixture".to_vec();
        let package = tohseno_engine::build_intent_package(
            "2026-08-15T12:00:00Z",
            "Remember every tree.",
            &[("tree.png".into(), "image/png".into(), image.clone())],
        )
        .unwrap();
        let pending = store
            .import_bytes(&package, tohseno_engine::PendingIntentionSource::Relay)
            .unwrap();
        let mut references = vec![ReferenceInput {
            display_filename: "tree.png".into(),
            media_type: "image/png".into(),
            origin: pending_reference_origin(&pending.id, 0),
            bytes: image,
        }];
        validate_pending_submission(&store, &pending, "Remember every tree.", &references).unwrap();
        references[0].bytes.push(0);
        assert!(
            validate_pending_submission(&store, &pending, "Remember every tree.", &references,)
                .is_err()
        );
    }

    #[test]
    fn pending_name_suggestion_is_stable_and_safe() {
        let root = tempfile::tempdir().unwrap();
        let ledger = tohseno_engine::Ledger::at(root.path().join("data"));
        ledger.initialize().unwrap();
        let first =
            suggest_pending_name("An app that remembers every tree I plant", &ledger).unwrap();
        let second =
            suggest_pending_name("An app that remembers every tree I plant", &ledger).unwrap();
        assert_eq!(first, "remembers-tree-plant");
        assert_eq!(first, second);
        tohseno_engine::ledger::validate_app_name(&first).unwrap();

        ledger
            .create_app(&first, "com.tohseno.test.remembers-tree-plant")
            .unwrap();
        let collision =
            derive_technical_name("An app that remembers every tree I plant", &ledger).unwrap();
        assert_ne!(collision, first);
        assert!(collision.starts_with("remembers-tree-plant-"));
        tohseno_engine::ledger::validate_app_name(&collision).unwrap();

        let reserved = derive_technical_name("Identity", &ledger).unwrap();
        assert_ne!(reserved, "identity");
        tohseno_engine::ledger::validate_app_name(&reserved).unwrap();
    }

    #[test]
    fn api_body_limit_carries_the_full_decoded_reference_allowance() {
        let encoded_references = MAX_REFERENCE_TOTAL_BYTES.div_ceil(3) * 4;
        assert!(MAX_API_BODY_BYTES > encoded_references + MAX_INTENTION_BYTES + 1024 * 1024);
    }

    #[test]
    fn completed_validation_remains_in_the_verifying_stage() {
        assert_eq!(
            privacy_safe_phase(ExecutionPhase::ValidationCompleted),
            "verifying"
        );
        assert!(!execution_is_terminal(ExecutionPhase::ValidationCompleted));
        assert!(execution_is_terminal(ExecutionPhase::ExecutionCompleted));
        assert!(execution_is_terminal(ExecutionPhase::ExecutionFailed));
        assert!(execution_is_terminal(ExecutionPhase::ExecutionCancelled));
    }

    #[test]
    fn studio_projection_digest_ignores_clock_but_detects_execution_state() {
        let execution = tohseno_application::ExecutionSummary {
            execution_id: "execution_fixture".into(),
            shot_id: "shot_fixture".into(),
            state: "queued".into(),
            version_ordinal: 1,
            started_at: "2026-08-16T00:00:00Z".into(),
            elapsed_seconds: 0,
            updated_at: "2026-08-16T00:00:00Z".into(),
            state_transition: None,
        };
        let mut first = tohseno_application::WorkspaceSnapshot {
            schema: "tohseno.companion-workspace-snapshot/1".into(),
            workspace_id: "workspace_fixture".into(),
            snapshot_version: 1,
            generated_at: "2026-08-16T00:00:00Z".into(),
            service_version: "0.9.0".into(),
            shots: Vec::new(),
            active_executions: vec![execution],
            device_capability_epoch: 0,
            next_cursor: 0,
        };
        let first_digest = privacy_safe_workspace_digest(&first).unwrap();
        first.generated_at = "2026-08-16T00:01:00Z".into();
        assert_eq!(privacy_safe_workspace_digest(&first).unwrap(), first_digest);
        first.active_executions[0].state = "planning".into();
        assert_ne!(privacy_safe_workspace_digest(&first).unwrap(), first_digest);
    }

    #[test]
    fn the_change_fingerprint_moves_for_added_replaced_and_removed_metadata() {
        let root = tempfile::tempdir().unwrap();
        let metadata = root.path().join(".tohseno");
        fs::create_dir_all(metadata.join("executions/one")).unwrap();
        fs::write(metadata.join("executions/one/execution.json"), b"running").unwrap();

        let stamp = |directory: &Path| {
            let mut output = String::new();
            let mut budget = FINGERPRINT_MAX_ENTRIES;
            stamp_tree(directory, 0, &mut budget, &mut output);
            output
        };

        let baseline = stamp(&metadata);
        assert_eq!(stamp(&metadata), baseline, "a quiet tree must not move");

        // Execution records are replaced in place as a run advances, so the
        // fingerprint has to notice a same-length rewrite, not just new files.
        fs::write(metadata.join("executions/one/execution.json"), b"waiting").unwrap();
        let rewritten = stamp(&metadata);
        assert_ne!(rewritten, baseline, "a replaced record must move");

        fs::write(metadata.join("executions/one/events.jsonl"), b"{}").unwrap();
        let added = stamp(&metadata);
        assert_ne!(added, rewritten, "an added record must move");

        fs::remove_file(metadata.join("executions/one/events.jsonl")).unwrap();
        assert_ne!(stamp(&metadata), added, "a removed record must move");
    }

    #[test]
    fn the_change_fingerprint_reads_no_file_contents_and_stays_bounded() {
        let root = tempfile::tempdir().unwrap();
        let metadata = root.path().join(".tohseno");
        fs::create_dir_all(&metadata).unwrap();
        for index in 0..(FINGERPRINT_MAX_ENTRIES * 2) {
            fs::write(metadata.join(format!("record-{index}")), b"private").unwrap();
        }
        let mut output = String::new();
        let mut budget = FINGERPRINT_MAX_ENTRIES;
        stamp_tree(&metadata, 0, &mut budget, &mut output);
        assert_eq!(budget, 0, "the walk must stop at its entry budget");
        assert!(
            !output.contains("private"),
            "the fingerprint must never carry file contents"
        );
    }

    #[cfg(unix)]
    #[test]
    fn service_lock_rejects_a_duplicate_process() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("service.lock");
        let first = acquire_service_lock(&path).unwrap();
        assert!(acquire_service_lock(&path).is_err());
        drop(first);
        assert!(acquire_service_lock(&path).is_ok());
    }
}
