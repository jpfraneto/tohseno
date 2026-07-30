use crate::bankr_launch::{
    BankrLaunchService, DeployApprovalRequest, LaunchParameters, ShotAssociationEvidence,
    ShotLaunchBinding,
};
use crate::shot_execution_commands;
use crate::simulator::{self, SimulatorSession};
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use qrcode::render::svg;
use qrcode::{EcLevel, QrCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tohseno_engine::builder_identity::{
    BuilderDeploymentStatus, BuilderIdentity, BuilderIdentityManager,
};
use tohseno_engine::gates::apple_signing::AppleSigningState;
use tohseno_engine::gates::intent::Intent;
use tohseno_engine::gates::toolchain::ToolchainState;
use tohseno_engine::protocol_lifecycle::reference_fascia_root;
use tohseno_engine::verifier::{verify_shot_directory, VerificationStatus};
use tohseno_engine::{
    Engine, Event, EventBus, InitialExpressionPlan, Ledger, ShotLayout, ShotRequest,
};
use tohseno_protocol::builder::PAIRING_SCHEMA;
use tohseno_protocol::canonical;
use tohseno_protocol::conformance::{CheckStatus, ConformanceReport};
use tohseno_protocol::digest::{Address20, Bytes32};
use tohseno_protocol::fascia::FasciaManifest;
use tohseno_protocol::identity::{device_key_id, BuilderId, ROBINHOOD_CHAIN_ID};
use tohseno_protocol::lineage::AcceptedGenome;
use tohseno_protocol::ontology::{
    AvailabilityStatus, Expression, TokenAssociation, TokenAssociationOperation, VersionRecord,
    TOKEN_ASSOCIATION_SCHEMA,
};
use tohseno_protocol::record::ShotRecord;
use tohseno_protocol::signature::SignatureSidecar;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::task::JoinSet;

const INDEX: &str = include_str!("../../studio/index.html");
const STYLE: &str = include_str!("../../studio/style.css");
const SCRIPT: &str = include_str!("../../studio/app.js");
const BRAND_COLORS: &str = include_str!("../../brand/tokens/colors.css");
const CORE_CIRCLE: &[u8] = include_bytes!("../../brand/logos/tohseno-core-circle.svg");
const MICRO_CIRCLE: &[u8] = include_bytes!("../../brand/logos/tohseno-micro-circle.png");
const DEPLOYMENT_PLAN: &str =
    include_str!("../../contracts/deployments/robinhood-mainnet-genesis.json");
const MAX_BODY: usize = 160 * 1024 * 1024;
const MAX_HEADERS: usize = 32 * 1024;
const MAX_PROTOCOL_JSON: u64 = 4 * 1024 * 1024;
const MAX_PAIRING_PAYLOAD: usize = 4 * 1024;
const MAX_PAIRING_SVG: usize = 256 * 1024;
const PAIRING_FORMAT: &str = "tohseno-pairing-target-json";
const PAIRING_DESCRIPTION: &str = "Public BuilderID and Robinhood Chain pairing target context only; this is not a pairing request, authorization, signature, or secret.";
const PAIRING_QR_PATH: &str = "/api/protocol/pairing-target.svg";

#[derive(Clone)]
struct State {
    events: EventBus,
    press: Arc<Mutex<()>>,
    simulator: Arc<Mutex<Option<SimulatorSession>>>,
    bankr: BankrLaunchService,
    authority: String,
    origin: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BankrSimulationRequest {
    app_name: String,
    version_ordinal: u32,
    parameters: LaunchParameters,
}

#[derive(Debug, Deserialize)]
struct ShotSubmission {
    mode: ShotMode,
    app_name: String,
    prompt: String,
    #[serde(default)]
    accept_genome: bool,
    #[serde(default)]
    selected_feedback_actions: Vec<Bytes32>,
    harness: String,
    model: String,
    route: String,
    #[serde(default)]
    images: Vec<UploadedImage>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InitialPlanRequest {
    app_name: String,
    prompt: String,
}

#[derive(Debug, Serialize)]
struct InitialPlanResponse {
    genome: tohseno_protocol::Genome,
    genome_markdown: String,
    expression_plan: InitialExpressionPlan,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ShotMode {
    Create,
    Evolve,
}

#[derive(Debug, Deserialize)]
struct UploadedImage {
    name: String,
    data: String,
}

#[derive(Debug, Deserialize)]
struct SimulatorLaunch {
    app_name: String,
    shot: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FeedbackSubmission {
    app_name: String,
    version_ordinal: u64,
    text: String,
}

#[derive(Debug, Serialize)]
struct FeedbackSaved {
    feedback_id: String,
    action_commitment: String,
    private: bool,
    version_ordinal: u64,
}

#[derive(Debug, Serialize)]
struct LibraryResponse {
    apps: Vec<LibraryApp>,
    iphone_slots_used: usize,
    iphone_slot_limit: usize,
}

#[derive(Debug, Serialize)]
struct LibraryApp {
    name: String,
    latest_evolution: u32,
    shots: Vec<u32>,
    retired: bool,
    icon_url: String,
    folder: String,
    unrecorded_changes: bool,
    memory: Option<String>,
    expires_in_days: Option<i64>,
}

#[derive(Debug, Serialize)]
struct HarnessesResponse {
    harnesses: Vec<tohseno_engine::HarnessOption>,
}

#[derive(Debug, Serialize)]
struct OnboardingResponse {
    schema: &'static str,
    version: &'static str,
    first_run: bool,
    accepted_shots: usize,
    xcode: OnboardingCheck,
    apple_signing: OnboardingCheck,
    harness_ready: bool,
    ready_for_first_shot: bool,
}

#[derive(Debug, Serialize)]
struct OnboardingCheck {
    ready: bool,
    status: &'static str,
    detail: String,
}

#[derive(Debug, Serialize)]
struct ExecutionStateResponse {
    execution: tohseno_engine::PreparedExecution,
    events: Vec<tohseno_engine::ShotExecutionEvent>,
    completion: Option<tohseno_engine::CompletionRecord>,
}

#[derive(Debug, Serialize)]
struct ProtocolOverview {
    candidate_version: String,
    identity: IdentityFacts,
    pairing: PairingFacts,
    network: NetworkFacts,
    publish: PublishFacts,
}

#[derive(Debug, Serialize)]
struct IdentityFacts {
    status: &'static str,
    builder_id: Option<String>,
    account_address: Option<String>,
    deployment_status: Option<&'static str>,
    recovery_status: &'static str,
    device_keys: Vec<DeviceKeyFacts>,
    detail: String,
}

#[derive(Debug, Serialize)]
struct DeviceKeyFacts {
    label: &'static str,
    key_id: String,
    status: &'static str,
    security_level: String,
    test_only: bool,
}

#[derive(Debug, Serialize)]
struct PairingFacts {
    request_schema: &'static str,
    qr_available: bool,
    target_payload: Option<String>,
    qr_url: Option<&'static str>,
    limitation: &'static str,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PairingTargetPayload {
    format: String,
    version: u8,
    encodes: String,
    request_schema: String,
    builder_id: BuilderId,
    network: PairingTargetNetwork,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PairingTargetNetwork {
    chain_id: u64,
    builder_account: Address20,
    builder_account_factory: Address20,
}

#[derive(Debug, Serialize)]
struct NetworkFacts {
    chain_id: u64,
    name: String,
    connectivity: &'static str,
    p256verify: String,
    p256_status: &'static str,
    deployment_status: &'static str,
    deployment_evidence: bool,
    contracts: Vec<ContractFacts>,
}

#[derive(Debug, Serialize)]
struct ContractFacts {
    name: String,
    address: Option<String>,
    status: &'static str,
    transaction_hash: Option<String>,
}

#[derive(Debug, Serialize)]
struct PublishFacts {
    experimental: bool,
    enabled: bool,
    required_guard: &'static str,
    guard_present: bool,
    reason: String,
}

#[derive(Debug, Serialize)]
struct StudioNodeFacts {
    configured: bool,
    reachable: bool,
    identity: Option<String>,
    protocol_version: Option<String>,
    replicated_shots: Option<usize>,
    detail: String,
}

#[derive(Debug, Serialize)]
struct ShotProtocolFacts {
    app_name: String,
    ledger_shot: u32,
    current: bool,
    adoption_required: bool,
    local_state: &'static str,
    published_state: &'static str,
    source_published: bool,
    registry_head: Option<String>,
    transaction_hash: Option<String>,
    evolution: Option<EvolutionFacts>,
    signature: EvidenceFacts,
    fascia: FasciaFacts,
    conformance: ConformanceFacts,
    verification: VerificationFacts,
    handle: RelationFacts,
    appcoin: RelationFacts,
    #[serde(skip_serializing_if = "Option::is_none")]
    ontology: Option<ShotOntologyFacts>,
}

#[derive(Debug, Serialize)]
struct ShotOntologyFacts {
    status: &'static str,
    shot_id: String,
    original_intention: OriginalIntentionFacts,
    accepted_genome: AcceptedGenome,
    expression: Expression,
    version: VersionRecord,
    token_association: OntologyTokenAssociationFacts,
    lineage: OntologyLineageFacts,
    detail: &'static str,
}

#[derive(Debug, Serialize)]
struct OriginalIntentionFacts {
    status: &'static str,
    exact: String,
}

#[derive(Debug, Serialize)]
struct OntologyLineageFacts {
    sequence: u64,
    head: String,
    verification: &'static str,
    availability: &'static str,
}

#[derive(Debug, Serialize)]
struct OntologyTokenAssociationFacts {
    status: &'static str,
    current_action: Option<String>,
    chain_id: Option<u64>,
    token_address: Option<String>,
    symbol: Option<String>,
    anchor_declared: bool,
    anchor_verified: bool,
    history_count: usize,
    identity_role: &'static str,
}

#[derive(Debug, Serialize)]
struct EvolutionFacts {
    shot_id: String,
    builder_id: String,
    sequence: u32,
    previous: Option<String>,
    commitment: Option<String>,
    origin: &'static str,
}

#[derive(Debug, Serialize)]
struct EvidenceFacts {
    status: &'static str,
    signer_device_key_id: Option<String>,
    algorithm: Option<&'static str>,
    low_s: Option<bool>,
    detail: String,
}

#[derive(Debug, Serialize)]
struct FasciaFacts {
    status: &'static str,
    id: Option<String>,
    commitment: Option<String>,
    distribution: Option<String>,
    capabilities: Option<usize>,
    detail: String,
}

#[derive(Debug, Serialize)]
struct ConformanceFacts {
    status: &'static str,
    conformant: Option<bool>,
    passed: usize,
    failed: usize,
    not_checked: usize,
    detail: String,
}

#[derive(Debug, Serialize)]
struct VerificationFacts {
    status: &'static str,
    conformant: Option<bool>,
    passed: usize,
    failed: usize,
    not_checked: usize,
    evolution_commitment: Option<String>,
    detail: String,
}

#[derive(Debug, Serialize)]
struct RelationFacts {
    status: &'static str,
    value: Option<String>,
    detail: &'static str,
}

pub async fn serve(port: u16, events: EventBus) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(("127.0.0.1", port)).await?;
    let address = listener.local_addr()?;
    let url = format!("http://127.0.0.1:{}", address.port());
    events.emit(Event::status(format!("studio is ready at {url}.")));
    let _ = std::process::Command::new("open").arg(&url).spawn();
    let state = State {
        events,
        press: Arc::new(Mutex::new(())),
        simulator: Arc::new(Mutex::new(None)),
        bankr: BankrLaunchService::from_environment()?,
        authority: format!("127.0.0.1:{}", address.port()),
        origin: url,
    };
    let mut tasks = JoinSet::new();

    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let (socket, _) = accepted?;
                let state = state.clone();
                tasks.spawn(async move {
                    if let Err(error) = handle(socket, state).await {
                        eprintln!("studio: {error}");
                    }
                });
            }
            completed = tasks.join_next(), if !tasks.is_empty() => {
                let _ = completed;
            }
            signal = tokio::signal::ctrl_c() => {
                signal?;
                tasks.abort_all();
                while tasks.join_next().await.is_some() {}
                return Ok(());
            }
        }
    }
}

async fn handle(mut socket: TcpStream, state: State) -> Result<(), Box<dyn std::error::Error>> {
    let request = read_request(&mut socket).await?;
    if !request_has_expected_authority(&request, &state.authority) {
        respond(
            &mut socket,
            403,
            "text/plain; charset=utf-8",
            "forbidden host",
        )
        .await?;
        return Ok(());
    }
    if request.method == "POST" && !request_is_same_origin_json(&request, &state.origin) {
        respond(
            &mut socket,
            403,
            "text/plain; charset=utf-8",
            "same-origin Studio JSON request required",
        )
        .await?;
        return Ok(());
    }
    if request.method == "GET" && request.path == "/api/apps" {
        return serve_library(&mut socket).await;
    }
    if request.method == "GET" && request.path == "/api/harnesses" {
        return serve_harnesses(&mut socket, &state).await;
    }
    if request.method == "GET" && request.path == "/api/onboarding" {
        return serve_onboarding(&mut socket, &state).await;
    }
    if request.method == "GET" && request.path == "/api/protocol" {
        return serve_protocol_overview(&mut socket).await;
    }
    if request.method == "GET" && request.path == "/api/node" {
        return serve_node_status(&mut socket).await;
    }
    if request.method == "GET" && request.path == "/api/bankr/launch" {
        return serve_bankr_launch_status(&mut socket, &state).await;
    }
    if request.method == "GET" && request.path == PAIRING_QR_PATH {
        return serve_pairing_qr(&mut socket).await;
    }
    if request.method == "GET" && request.path.starts_with("/api/protocol/shot/") {
        return serve_shot_protocol(&mut socket, &request.path).await;
    }
    if request.method == "GET" && request.path.starts_with("/api/executions/") {
        return serve_execution_state(&mut socket, &request.path).await;
    }
    if request.method == "GET" && request.path.starts_with("/api/icon/") {
        return serve_icon(&mut socket, &request.path).await;
    }
    if request.method == "POST" && request.path == "/api/evolve" {
        return record_evolution(&mut socket, &request.body, &state).await;
    }
    if request.method == "POST" && request.path == "/api/feedback" {
        return save_feedback(&mut socket, &request.body, &state).await;
    }
    if request.method == "POST" && request.path == "/api/bankr/launch/simulate" {
        return simulate_bankr_launch(&mut socket, &request.body, &state).await;
    }
    if request.method == "POST" && request.path == "/api/bankr/launch/deploy" {
        return deploy_bankr_launch(&mut socket, &request.body, &state).await;
    }
    if request.method == "POST" && request.path == "/api/plan" {
        return serve_initial_plan(&mut socket, &request.body).await;
    }
    if request.method == "POST" && request.path == "/api/open" {
        return open_folder(&mut socket, &request.body).await;
    }
    if request.method == "POST" && request.path == "/api/refresh" {
        return refresh_app(&mut socket, &request.body, &state).await;
    }
    if request.method == "POST" && request.path == "/api/simulator/launch" {
        return launch_simulator(&mut socket, &request.body, &state).await;
    }
    if request.method == "GET" && request.path == "/api/simulator/screen" {
        return serve_simulator_screen(&mut socket, &state).await;
    }
    if request.method == "POST" && request.path == "/api/simulator/focus" {
        let _ = std::process::Command::new("open")
            .args(["-a", "Simulator"])
            .spawn();
        respond(
            &mut socket,
            200,
            "application/json; charset=utf-8",
            r#"{"focused":true}"#,
        )
        .await?;
        return Ok(());
    }

    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => respond(&mut socket, 200, "text/html; charset=utf-8", INDEX).await?,
        ("GET", "/style.css") => {
            respond(&mut socket, 200, "text/css; charset=utf-8", STYLE).await?
        }
        ("GET", "/app.js") => {
            respond(&mut socket, 200, "text/javascript; charset=utf-8", SCRIPT).await?
        }
        ("GET", "/brand/tokens/colors.css") => {
            respond(&mut socket, 200, "text/css; charset=utf-8", BRAND_COLORS).await?
        }
        ("GET", "/brand/logos/tohseno-core-circle.svg") => {
            respond_bytes(&mut socket, 200, "image/svg+xml", CORE_CIRCLE).await?
        }
        ("GET", "/brand/logos/tohseno-micro-circle.png") => {
            respond_bytes(&mut socket, 200, "image/png", MICRO_CIRCLE).await?
        }
        ("GET", "/events") => stream_events(socket, state.events).await?,
        ("POST", "/shots") => {
            let submission: ShotSubmission = match serde_json::from_slice(&request.body) {
                Ok(submission) => submission,
                Err(error) => {
                    respond(
                        &mut socket,
                        400,
                        "text/plain; charset=utf-8",
                        &format!("invalid shot: {error}"),
                    )
                    .await?;
                    return Ok(());
                }
            };
            if matches!(submission.mode, ShotMode::Create) && !submission.accept_genome {
                respond(
                    &mut socket,
                    422,
                    "text/plain; charset=utf-8",
                    "the initial Genome and Apple expression plan must be reviewed and explicitly accepted",
                )
                .await?;
                return Ok(());
            }
            if matches!(submission.mode, ShotMode::Create)
                && !submission.selected_feedback_actions.is_empty()
            {
                respond(
                    &mut socket,
                    422,
                    "text/plain; charset=utf-8",
                    "Feedback actions can be selected only for an evolution from an accepted Version",
                )
                .await?;
                return Ok(());
            }
            let staging = tempfile::tempdir()?;
            let image_paths = match stage_images(staging.path(), submission.images).await {
                Ok(paths) => paths,
                Err(error) => {
                    respond(
                        &mut socket,
                        422,
                        "text/plain; charset=utf-8",
                        &format!("reference images were rejected: {error}"),
                    )
                    .await?;
                    return Ok(());
                }
            };
            let events = state.events.clone();
            let press = state.press.clone();
            let _staging = staging;
            let _guard = press.lock().await;
            let request = ShotRequest {
                app_name: submission.app_name,
                intent: Intent::parse(&submission.prompt).with_images(image_paths),
                selected_feedback_actions: submission.selected_feedback_actions,
            };
            let outcome: Result<tohseno_engine::PreparedExecution, String> =
                async {
                    let engine =
                        Engine::discover(events.clone()).map_err(|error| error.to_string())?;
                    let selected = shot_execution_commands::selection(
                        &engine,
                        Some(&submission.harness),
                        Some(&submission.model),
                        Some(&submission.route),
                    )
                    .map_err(|error| error.to_string())?;
                    match submission.mode {
                        ShotMode::Create => {
                            let genome = Engine::propose_initial_genome(&request)
                                .map_err(|error| error.to_string())?;
                            let plan =
                                Engine::propose_initial_expression_plan(&request, &genome)
                                    .map_err(|error| error.to_string())?;
                            engine.create(&request).map_err(|error| error.to_string())?;
                            engine.accept_genome(
                                &request.app_name,
                                &genome,
                                "Owner reviewed and accepted the initial operational Genome in Studio.",
                                &[],
                            )
                            .map_err(|error| error.to_string())?;
                            engine
                                .declare_initial_expression(&request.app_name, &plan)
                                .map_err(|error| error.to_string())?;
                            let creation =
                                engine.conduct_accepted_creation(&request.app_name)
                                    .map_err(|error| error.to_string())?;
                            shot_execution_commands::prepare(
                                &engine,
                                &creation,
                                &request.app_name,
                                &selected,
                                true,
                                &events,
                            )
                            .map_err(|error| error.to_string())
                        }
                        ShotMode::Evolve => match engine
                            .evolve(&request)
                            .await
                            .map_err(|error| error.to_string())?
                        {
                            tohseno_engine::machine::Evolved::Conducted(creation) => {
                                shot_execution_commands::prepare(
                                    &engine,
                                    &creation,
                                    &request.app_name,
                                    &selected,
                                    true,
                                    &events,
                                )
                                .map_err(|error| error.to_string())
                            }
                            _ => Err("the requested intention did not produce a prepared execution"
                                .into()),
                        },
                    }
                }
                .await;
            match outcome {
                Ok(execution) => {
                    let body = serde_json::to_string(&execution)?;
                    respond(&mut socket, 201, "application/json; charset=utf-8", &body).await?;
                }
                Err(error) => {
                    events.emit(Event::status(format!("engine stopped: {error}")));
                    respond(
                        &mut socket,
                        422,
                        "text/plain; charset=utf-8",
                        &format!("Shot was not prepared: {error}"),
                    )
                    .await?;
                }
            }
        }
        _ => respond(&mut socket, 404, "text/plain; charset=utf-8", "not found").await?,
    }
    Ok(())
}

async fn serve_bankr_launch_status(
    socket: &mut TcpStream,
    state: &State,
) -> Result<(), Box<dyn std::error::Error>> {
    let body = serde_json::to_string(&state.bankr.status())?;
    respond(socket, 200, "application/json; charset=utf-8", &body).await?;
    Ok(())
}

async fn simulate_bankr_launch(
    socket: &mut TcpStream,
    body: &[u8],
    state: &State,
) -> Result<(), Box<dyn std::error::Error>> {
    let request = match serde_json::from_slice::<BankrSimulationRequest>(body) {
        Ok(request) => request,
        Err(error) => {
            respond(
                socket,
                400,
                "text/plain; charset=utf-8",
                &format!("invalid Bankr launch configuration: {error}"),
            )
            .await?;
            return Ok(());
        }
    };
    let shot = match bankr_shot_binding(&request.app_name, request.version_ordinal) {
        Ok(shot) => shot,
        Err(error) => {
            respond(
                socket,
                422,
                "text/plain; charset=utf-8",
                &format!("Bankr launch is not available for this Shot: {error}"),
            )
            .await?;
            return Ok(());
        }
    };
    match state.bankr.simulate(shot, request.parameters).await {
        Ok(approval) => {
            let body = serde_json::to_string(&approval)?;
            respond(socket, 200, "application/json; charset=utf-8", &body).await?;
        }
        Err(error) => {
            respond(
                socket,
                422,
                "text/plain; charset=utf-8",
                &format!("Bankr simulation was not approved: {}", error.message),
            )
            .await?;
        }
    }
    Ok(())
}

async fn deploy_bankr_launch(
    socket: &mut TcpStream,
    body: &[u8],
    state: &State,
) -> Result<(), Box<dyn std::error::Error>> {
    let approval = match serde_json::from_slice::<DeployApprovalRequest>(body) {
        Ok(approval) => approval,
        Err(error) => {
            respond(
                socket,
                400,
                "text/plain; charset=utf-8",
                &format!("invalid Bankr deployment approval: {error}"),
            )
            .await?;
            return Ok(());
        }
    };
    let approved_version = match u32::try_from(approval.shot.version_ordinal) {
        Ok(version) => version,
        Err(_) => {
            respond(
                socket,
                422,
                "text/plain; charset=utf-8",
                "Bankr deployment was not submitted: the approved version ordinal is invalid",
            )
            .await?;
            return Ok(());
        }
    };
    let authoritative = match bankr_shot_binding(&approval.shot.app_name, approved_version) {
        Ok(shot) if shot == approval.shot => shot,
        Ok(_) => {
            respond(
                socket,
                422,
                "text/plain; charset=utf-8",
                "Bankr deployment was not submitted: the approved Shot binding changed",
            )
            .await?;
            return Ok(());
        }
        Err(error) => {
            respond(
                socket,
                422,
                "text/plain; charset=utf-8",
                &format!("Bankr deployment was not submitted: {error}"),
            )
            .await?;
            return Ok(());
        }
    };
    debug_assert_eq!(authoritative, approval.shot);
    match state.bankr.deploy(approval).await {
        Ok(mut outcome) => {
            match record_bankr_shot_association(&outcome, state) {
                Ok(evidence) => outcome.shot_association = Some(evidence),
                Err(error) => outcome.warnings.push(format!(
                    "The token deployed, but its signed Shot association was not recorded: {error}"
                )),
            }
            let body = serde_json::to_string(&outcome)?;
            respond(socket, 201, "application/json; charset=utf-8", &body).await?;
        }
        Err(error) => {
            let prefix = if error.uncertain_deployment_outcome {
                "DEPLOYMENT OUTCOME UNKNOWN"
            } else {
                "Bankr deployment was not submitted"
            };
            respond(
                socket,
                if error.uncertain_deployment_outcome {
                    500
                } else {
                    422
                },
                "text/plain; charset=utf-8",
                &format!("{prefix}: {}", error.message),
            )
            .await?;
        }
    }
    Ok(())
}

fn bankr_shot_binding(app_name: &str, version_ordinal: u32) -> Result<ShotLaunchBinding, String> {
    tohseno_engine::ledger::validate_app_name(app_name).map_err(|error| error.to_string())?;
    if version_ordinal == 0 {
        return Err("version ordinal must be positive".into());
    }
    let ledger = Ledger::discover().map_err(|error| error.to_string())?;
    let app = ledger
        .load_app(app_name)
        .map_err(|error| error.to_string())?;
    if !ledger
        .list_evolutions(app_name)
        .map_err(|error| error.to_string())?
        .iter()
        .any(|evolution| evolution.number == version_ordinal)
    {
        return Err("the selected evolution does not exist".into());
    }
    let ontology = shot_ontology_facts(&ledger, &app.name, version_ordinal)
        .map_err(|error| error.to_string())?
        .ok_or("the selected Shot has no verified v2 identity")?;
    if ontology.token_association.status == "associated" {
        return Err(
            "this Shot already has a token association; Studio will not deploy an unbound replacement"
                .into(),
        );
    }
    Ok(ShotLaunchBinding {
        app_name: app.name,
        shot_id: ontology.shot_id,
        version_ordinal: u64::from(version_ordinal),
    })
}

fn record_bankr_shot_association(
    outcome: &crate::bankr_launch::DeploymentOutcome,
    state: &State,
) -> Result<ShotAssociationEvidence, String> {
    let version_ordinal = u32::try_from(outcome.shot.version_ordinal)
        .map_err(|_| "the deployed token has an invalid Shot version ordinal")?;
    let current = bankr_shot_binding(&outcome.shot.app_name, version_ordinal)?;
    if current != outcome.shot {
        return Err("the authoritative Shot binding changed during deployment".into());
    }
    let token_address = outcome
        .bankr_deployment
        .get("tokenAddress")
        .and_then(serde_json::Value::as_str)
        .ok_or("Bankr did not return a deployable token address")?;
    let token =
        serde_json::from_value::<Address20>(serde_json::Value::String(token_address.to_owned()))
            .map_err(|error| error.to_string())?;
    let engine = Engine::discover(state.events.clone()).map_err(|error| error.to_string())?;
    let receipt = engine
        .record_token_association(
            &outcome.shot.app_name,
            TokenAssociation {
                schema: TOKEN_ASSOCIATION_SCHEMA.into(),
                operation: TokenAssociationOperation::Associate,
                chain_id: outcome.parameters.chain.chain_id(),
                token,
                symbol: Some("TOHSENO".into()),
                anchor: None,
            },
            AvailabilityStatus::PubliclyAvailable,
        )
        .map_err(|error| error.to_string())?;
    if receipt.action.action.shot_id.to_string() != outcome.shot.shot_id {
        return Err("the signed association resolved to a different ShotID".into());
    }
    Ok(ShotAssociationEvidence {
        action_commitment: receipt.action_commitment.to_string(),
        lineage_head: receipt.lineage_head.to_string(),
        availability: "publicly_available",
        outbox_path: receipt.outbox_path.map(|path| path.display().to_string()),
    })
}

async fn serve_initial_plan(
    socket: &mut TcpStream,
    body: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let request: InitialPlanRequest = match serde_json::from_slice(body) {
        Ok(request) => request,
        Err(error) => {
            respond(
                socket,
                400,
                "text/plain; charset=utf-8",
                &format!("invalid plan request: {error}"),
            )
            .await?;
            return Ok(());
        }
    };
    let request = ShotRequest {
        app_name: request.app_name,
        intent: Intent::parse(&request.prompt),
        selected_feedback_actions: Vec::new(),
    };
    let genome = match Engine::propose_initial_genome(&request) {
        Ok(genome) => genome,
        Err(error) => {
            respond(
                socket,
                422,
                "text/plain; charset=utf-8",
                &format!("plan could not be produced: {error}"),
            )
            .await?;
            return Ok(());
        }
    };
    let expression_plan = Engine::propose_initial_expression_plan(&request, &genome)?;
    let body = serde_json::to_string(&InitialPlanResponse {
        genome_markdown: tohseno_engine::render_genome_document(&genome)?,
        genome,
        expression_plan,
    })?;
    respond(socket, 200, "application/json; charset=utf-8", &body).await?;
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
struct EvolveRequest {
    app_name: String,
    #[serde(default)]
    note: Option<String>,
}

/// Records the folder's current state as the next Evolution, exactly like
/// `tohseno evolve` with no intent.
async fn record_evolution(
    socket: &mut TcpStream,
    body: &[u8],
    state: &State,
) -> Result<(), Box<dyn std::error::Error>> {
    let request: EvolveRequest = serde_json::from_slice(body)?;
    respond(
        socket,
        202,
        "application/json; charset=utf-8",
        r#"{"accepted":true}"#,
    )
    .await?;
    socket.shutdown().await?;
    let events = state.events.clone();
    let press = state.press.clone();
    let _guard = press.lock().await;
    let outcome = match Engine::discover(events.clone()) {
        Ok(engine) => engine
            .record(&request.app_name, request.note.as_deref())
            .await
            .map(|_| ()),
        Err(error) => Err(error),
    };
    if let Err(error) = outcome {
        events.emit(Event::status(format!("engine stopped: {error}")));
    }
    Ok(())
}

/// Records private feedback only after the engine resolves the exact accepted
/// Expression and Version and signs the canonical lineage action.
async fn save_feedback(
    socket: &mut TcpStream,
    body: &[u8],
    state: &State,
) -> Result<(), Box<dyn std::error::Error>> {
    let request = match serde_json::from_slice::<FeedbackSubmission>(body) {
        Ok(request) => request,
        Err(error) => {
            respond(
                socket,
                400,
                "text/plain; charset=utf-8",
                &format!("invalid feedback: {error}"),
            )
            .await?;
            return Ok(());
        }
    };
    let _guard = state.press.lock().await;
    let result = Engine::discover(state.events.clone()).and_then(|engine| {
        engine.record_feedback(&request.app_name, request.version_ordinal, &request.text)
    });
    match result {
        Ok(stored) => {
            let body = serde_json::to_string(&FeedbackSaved {
                feedback_id: stored.feedback_id.to_string(),
                action_commitment: stored.action_commitment.to_string(),
                private: true,
                version_ordinal: request.version_ordinal,
            })?;
            respond(socket, 201, "application/json; charset=utf-8", &body).await?;
        }
        Err(error) => {
            respond(
                socket,
                422,
                "text/plain; charset=utf-8",
                &format!("feedback was not recorded: {error}"),
            )
            .await?;
        }
    }
    Ok(())
}

/// Re-signs and reinstalls the latest Evolution, exactly like `tohseno refresh`.
async fn refresh_app(
    socket: &mut TcpStream,
    body: &[u8],
    state: &State,
) -> Result<(), Box<dyn std::error::Error>> {
    let request: OpenFolderRequest = serde_json::from_slice(body)?;
    respond(
        socket,
        202,
        "application/json; charset=utf-8",
        r#"{"accepted":true}"#,
    )
    .await?;
    socket.shutdown().await?;
    let events = state.events.clone();
    let press = state.press.clone();
    let _guard = press.lock().await;
    let outcome = match Engine::discover(events.clone()) {
        Ok(engine) => engine.refresh(Some(&request.app_name)).await,
        Err(error) => Err(error),
    };
    if let Err(error) = outcome {
        events.emit(Event::status(format!("engine stopped: {error}")));
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
struct OpenFolderRequest {
    app_name: String,
}

/// Reveals the app's living folder in Finder.
async fn open_folder(
    socket: &mut TcpStream,
    body: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let request: OpenFolderRequest = serde_json::from_slice(body)?;
    tohseno_engine::ledger::validate_app_name(&request.app_name)?;
    let ledger = Ledger::discover()?;
    let folder = ledger.working_tree(&request.app_name);
    let _ = std::process::Command::new("open").arg(&folder).spawn();
    respond(
        socket,
        200,
        "application/json; charset=utf-8",
        r#"{"opened":true}"#,
    )
    .await?;
    Ok(())
}

async fn serve_harnesses(
    socket: &mut TcpStream,
    state: &State,
) -> Result<(), Box<dyn std::error::Error>> {
    let engine = Engine::discover(state.events.clone())?;
    let body = serde_json::to_string(&HarnessesResponse {
        harnesses: engine.harnesses(),
    })?;
    respond(socket, 200, "application/json; charset=utf-8", &body).await?;
    Ok(())
}

async fn serve_execution_state(
    socket: &mut TcpStream,
    path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let suffix = path
        .strip_prefix("/api/executions/")
        .ok_or("invalid execution path")?;
    let mut components = suffix.split('/');
    let app_name = components.next().ok_or("missing execution app")?;
    let execution_id = components.next().ok_or("missing execution identity")?;
    if components.next().is_some() {
        respond(
            socket,
            404,
            "text/plain; charset=utf-8",
            "invalid execution path",
        )
        .await?;
        return Ok(());
    }
    tohseno_engine::ledger::validate_app_name(app_name)?;
    let ledger = Ledger::discover()?;
    let repository = ledger.working_tree(app_name);
    let execution = match tohseno_engine::shot_execution::load_execution(&repository, execution_id)
    {
        Ok(execution) => execution,
        Err(error) => {
            respond(
                socket,
                404,
                "text/plain; charset=utf-8",
                &format!("execution is unavailable: {error}"),
            )
            .await?;
            return Ok(());
        }
    };
    let body = serde_json::to_string(&ExecutionStateResponse {
        events: tohseno_engine::shot_execution::read_events(&repository, execution_id)?,
        completion: tohseno_engine::shot_execution::load_completion(&repository, execution_id)?,
        execution,
    })?;
    respond(socket, 200, "application/json; charset=utf-8", &body).await?;
    Ok(())
}

async fn serve_protocol_overview(socket: &mut TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let ledger = Ledger::discover()?;
    let identity = load_identity_facts(&ledger);
    let (candidate_version, network) = deployment_facts();
    let pairing = pairing_facts(identity.as_ref().ok());
    let deployment_evidence = network.deployment_evidence;
    let guard_present =
        std::env::var("TOHSENO_ALLOW_EXPERIMENTAL_MAINNET").is_ok_and(|value| value == "1");
    let publish = PublishFacts {
        experimental: true,
        enabled: false,
        required_guard: "TOHSENO_ALLOW_EXPERIMENTAL_MAINNET=1",
        guard_present,
        reason: if !deployment_evidence {
            "Candidate contracts have no complete deployment receipt. Publishing is disabled."
                .into()
        } else if !guard_present {
            "The explicit experimental-mainnet guard is absent. Publishing is disabled.".into()
        } else {
            "Studio has no broadcast or relayer path in this candidate. Publishing is disabled."
                .into()
        },
    };
    let body = serde_json::to_string(&ProtocolOverview {
        candidate_version,
        identity: match identity {
            Ok(identity) => identity_facts(&identity),
            Err(detail) if detail == "not_initialized" => IdentityFacts {
                status: "not_initialized",
                builder_id: None,
                account_address: None,
                deployment_status: None,
                recovery_status: "pending",
                device_keys: Vec::new(),
                detail: "Your identity will be created by the first protocol Shot or `tohseno identity show`.".into(),
            },
            Err(detail) => IdentityFacts {
                status: "invalid_local_state",
                builder_id: None,
                account_address: None,
                deployment_status: None,
                recovery_status: "unknown",
                device_keys: Vec::new(),
                detail,
            },
        },
        pairing,
        network,
        publish,
    })?;
    respond(socket, 200, "application/json; charset=utf-8", &body).await?;
    Ok(())
}

async fn serve_node_status(socket: &mut TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let facts = studio_node_facts();
    let body = serde_json::to_string(&facts)?;
    respond(socket, 200, "application/json; charset=utf-8", &body).await?;
    Ok(())
}

fn studio_node_facts() -> StudioNodeFacts {
    let Some(root) = std::env::var_os("TOHSENO_NODE_ROOT").map(PathBuf::from) else {
        return StudioNodeFacts {
            configured: false,
            reachable: false,
            identity: None,
            protocol_version: None,
            replicated_shots: None,
            detail:
                "Set TOHSENO_NODE_ROOT to an existing node store to inspect its local contribution."
                    .into(),
        };
    };
    if !root.is_absolute() {
        return unavailable_node_facts(
            "TOHSENO_NODE_ROOT must be an absolute path to an existing node store.",
        );
    }
    match fs::symlink_metadata(&root) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => {
            return unavailable_node_facts(
                "The configured node root is not a real local directory.",
            )
        }
        Err(error) => {
            return unavailable_node_facts(&format!(
                "The configured node root is unavailable: {error}"
            ))
        }
    }
    let store = match tohseno_node::NodeStore::open(&root) {
        Ok(store) => store,
        Err(error) => {
            return unavailable_node_facts(&format!(
                "The configured node store did not validate: {error}"
            ))
        }
    };
    let info = match store.info() {
        Ok(info) => info,
        Err(error) => {
            return unavailable_node_facts(&format!(
                "The configured node status could not be derived: {error}"
            ))
        }
    };
    match store.integrity() {
        Ok(integrity) if integrity.ok => StudioNodeFacts {
            configured: true,
            reachable: true,
            identity: Some(info.node_id.to_string()),
            protocol_version: Some(format!(
                "{} {} · schema {}",
                info.lineage_protocol,
                info.lineage_protocol_version,
                info.supported_schema_versions
                    .iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            )),
            replicated_shots: Some(info.indexed_shots),
            detail: format!(
                "Integrity verified for {} stored public actions. This local view does not claim global completeness.",
                info.stored_actions
            ),
        },
        Ok(integrity) => unavailable_node_facts(&format!(
            "The node store is degraded: {} integrity issue(s).",
            integrity.issues.len()
        )),
        Err(error) => unavailable_node_facts(&format!(
            "The configured node integrity check failed: {error}"
        )),
    }
}

fn unavailable_node_facts(detail: &str) -> StudioNodeFacts {
    StudioNodeFacts {
        configured: true,
        reachable: false,
        identity: None,
        protocol_version: None,
        replicated_shots: None,
        detail: detail.into(),
    }
}

fn load_identity_facts(ledger: &Ledger) -> Result<BuilderIdentity, String> {
    let manager = BuilderIdentityManager::for_ledger(ledger);
    match fs::symlink_metadata(manager.path()) {
        Ok(_) => manager
            .load()
            .map_err(|error| format!("The local identity descriptor did not validate: {error}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err("not_initialized".into()),
        Err(error) => Err(format!(
            "The local identity descriptor could not be inspected: {error}"
        )),
    }
}

fn identity_facts(identity: &BuilderIdentity) -> IdentityFacts {
    IdentityFacts {
        status: if identity.test_only {
            "test_only"
        } else {
            "ready"
        },
        builder_id: Some(identity.builder_id.to_string()),
        account_address: Some(identity.account_address.to_string()),
        deployment_status: Some(match identity.deployment_status {
            BuilderDeploymentStatus::Predicted => "predicted",
            BuilderDeploymentStatus::Deployed => "deployed",
        }),
        recovery_status: if identity.recovery.is_some() {
            "local_backup_only_recovery_unavailable"
        } else {
            "not_configured"
        },
        device_keys: vec![DeviceKeyFacts {
            label: "This Mac",
            key_id: identity.device.key_id.to_string(),
            status: if identity.test_only {
                "test_only"
            } else {
                "initial_device_only"
            },
            security_level: identity.security_level.clone(),
            test_only: identity.test_only,
        }],
        detail: if identity.test_only {
            "This identity uses an explicit software test key and is not production authority."
                .into()
        } else {
            "Your local BuilderID descriptor and original DeviceKey validate. This candidate cannot authorize, revoke, or recover replacement keys.".into()
        },
    }
}

fn pairing_facts(identity: Option<&BuilderIdentity>) -> PairingFacts {
    let target_payload = identity
        .and_then(|identity| pairing_target_json(identity).ok())
        .filter(|payload| render_pairing_svg(payload).is_ok());
    let qr_available = target_payload.is_some();
    PairingFacts {
        request_schema: PAIRING_SCHEMA,
        qr_available,
        target_payload,
        qr_url: qr_available.then_some(PAIRING_QR_PATH),
        limitation: "The QR encodes public Builder target context only, never a key, signature, authorization, or secret. It is not a pairing request. GENESIS cannot complete DeviceKey authorization or accept a replacement key.",
    }
}

impl PairingTargetPayload {
    fn from_identity(identity: &BuilderIdentity) -> Result<Self, String> {
        identity
            .validate()
            .map_err(|error| format!("Builder identity: {error}"))?;
        let payload = Self {
            format: PAIRING_FORMAT.into(),
            version: 1,
            encodes: PAIRING_DESCRIPTION.into(),
            request_schema: PAIRING_SCHEMA.into(),
            builder_id: identity.builder_id,
            network: PairingTargetNetwork {
                chain_id: identity.chain_id,
                builder_account: identity.account_address,
                builder_account_factory: identity.factory_address,
            },
        };
        payload.validate()?;
        Ok(payload)
    }

    fn validate(&self) -> Result<(), String> {
        if self.format != PAIRING_FORMAT
            || self.version != 1
            || self.encodes != PAIRING_DESCRIPTION
            || self.request_schema != PAIRING_SCHEMA
        {
            return Err("Pairing target format metadata is invalid.".into());
        }
        self.builder_id
            .validate()
            .map_err(|error| format!("BuilderID: {error}"))?;
        if self.network.chain_id != ROBINHOOD_CHAIN_ID
            || self.network.builder_account != self.builder_id.account()
            || self
                .network
                .builder_account_factory
                .as_bytes()
                .iter()
                .all(|byte| *byte == 0)
        {
            return Err("Pairing target network does not bind to the BuilderID.".into());
        }
        Ok(())
    }
}

fn pairing_target_json(identity: &BuilderIdentity) -> Result<String, String> {
    let payload = PairingTargetPayload::from_identity(identity)?;
    let encoded =
        canonical::to_string(&payload).map_err(|error| format!("Pairing target JSON: {error}"))?;
    if encoded.len() > MAX_PAIRING_PAYLOAD {
        return Err("Pairing target exceeds the QR payload limit.".into());
    }
    Ok(encoded)
}

fn render_pairing_svg(payload: &str) -> Result<String, String> {
    if payload.is_empty() || payload.len() > MAX_PAIRING_PAYLOAD {
        return Err("Pairing target is empty or exceeds the QR payload limit.".into());
    }
    let code = QrCode::with_error_correction_level(payload.as_bytes(), EcLevel::Q)
        .map_err(|error| format!("Pairing QR: {error}"))?;
    let svg = code
        .render::<svg::Color>()
        .min_dimensions(320, 320)
        .dark_color(svg::Color("#050505"))
        .light_color(svg::Color("#f2ede4"))
        .quiet_zone(true)
        .build();
    if svg.len() > MAX_PAIRING_SVG {
        return Err("Rendered pairing QR exceeds the SVG output limit.".into());
    }
    Ok(svg)
}

async fn serve_pairing_qr(socket: &mut TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let ledger = Ledger::discover()?;
    let identity = match load_identity_facts(&ledger) {
        Ok(identity) => identity,
        Err(_) => {
            respond(
                socket,
                404,
                "text/plain; charset=utf-8",
                "pairing target unavailable",
            )
            .await?;
            return Ok(());
        }
    };
    let payload = pairing_target_json(&identity)?;
    let svg = render_pairing_svg(&payload)?;
    respond_pairing_svg(socket, svg.as_bytes()).await?;
    Ok(())
}

fn deployment_facts() -> (String, NetworkFacts) {
    let parsed = serde_json::from_str::<serde_json::Value>(DEPLOYMENT_PLAN).ok();
    let candidate_version = parsed
        .as_ref()
        .and_then(|value| value.pointer("/candidate/version"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown")
        .to_owned();
    let chain_id_valid = parsed
        .as_ref()
        .and_then(|value| value.pointer("/chain/chain_id"))
        .and_then(serde_json::Value::as_u64)
        == Some(ROBINHOOD_CHAIN_ID);
    let schema_valid = parsed
        .as_ref()
        .and_then(|value| value.get("schema"))
        .and_then(serde_json::Value::as_str)
        == Some("tohseno.deployment-plan/1");
    let chain_name = parsed
        .as_ref()
        .and_then(|value| value.pointer("/chain/name"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("Robinhood Chain mainnet")
        .to_owned();
    let p256verify = parsed
        .as_ref()
        .and_then(|value| value.pointer("/chain/p256verify"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("0x0000000000000000000000000000000000000100")
        .to_owned();

    let mut contracts = Vec::new();
    let mut deployed_count = 0_usize;
    let mut evidence_count = 0_usize;
    for name in ["BuilderAccountFactory", "ShotRegistry", "ShotRelations"] {
        let contract = parsed
            .as_ref()
            .and_then(|value| value.pointer(&format!("/contracts/{name}")));
        let address = contract
            .and_then(|value| value.get("planned_address"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        let deployed = contract
            .and_then(|value| value.get("deployed"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let transaction_hash = contract
            .and_then(|value| value.get("transaction_hash"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        let runtime_code_hash = contract
            .and_then(|value| value.get("runtime_code_hash"))
            .and_then(serde_json::Value::as_str);
        let evidence = deployed
            && transaction_hash
                .as_deref()
                .is_some_and(|value| valid_lower_hex(value, 32))
            && runtime_code_hash.is_some_and(|value| valid_lower_hex(value, 32))
            && address
                .as_deref()
                .is_some_and(|value| valid_lower_hex(value, 20));
        deployed_count += usize::from(deployed);
        evidence_count += usize::from(evidence);
        contracts.push(ContractFacts {
            name: name.to_owned(),
            address,
            status: if evidence {
                "deployment_recorded"
            } else if deployed {
                "incomplete_evidence"
            } else {
                "planned"
            },
            transaction_hash,
        });
    }
    let source_commit_valid = parsed
        .as_ref()
        .and_then(|value| value.get("source_commit"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(valid_git_commit);
    let deployment_evidence =
        schema_valid && chain_id_valid && evidence_count == contracts.len() && source_commit_valid;
    let deployment_status = if !schema_valid || !chain_id_valid {
        "invalid_embedded_evidence"
    } else if deployment_evidence {
        "deployment_recorded_not_queried"
    } else if deployed_count == 0 && evidence_count == 0 {
        "planned_undeployed"
    } else {
        "incomplete_evidence"
    };
    (
        candidate_version,
        NetworkFacts {
            chain_id: ROBINHOOD_CHAIN_ID,
            name: chain_name,
            connectivity: "not_queried",
            p256verify,
            p256_status: "not_queried",
            deployment_status,
            deployment_evidence,
            contracts,
        },
    )
}

fn valid_lower_hex(value: &str, bytes: usize) -> bool {
    value.len() == 2 + bytes * 2
        && value.starts_with("0x")
        && value[2..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && value[2..].bytes().any(|byte| byte != b'0')
}

fn valid_git_commit(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

async fn serve_library(socket: &mut TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let ledger = Ledger::discover()?;
    let records = ledger.list_apps()?;
    let iphone_slots_used = records
        .iter()
        .filter(|app| !app.retired && app.latest_evolution.is_some())
        .count();
    let mut apps = Vec::new();
    for app in records {
        let Some(latest_evolution) = app.latest_evolution else {
            continue;
        };
        let shots: Vec<u32> = ledger
            .list_evolutions(&app.name)?
            .into_iter()
            .map(|shot| shot.number)
            .collect();
        let folder = ledger.working_tree(&app.name);
        let latest = ledger.shot(&app.name, latest_evolution)?;
        let unrecorded_changes = match (
            tohseno_protocol::tree_hash::hash_working_tree(&folder),
            tohseno_protocol::tree_hash::hash_source_tree(&latest.source_path()),
        ) {
            (Ok(working), Ok(sealed)) => working.digest != sealed.digest,
            _ => false,
        };
        let memory = std::fs::symlink_metadata(folder.join("MEMORY.md"))
            .ok()
            .filter(|metadata| metadata.is_file() && metadata.len() <= 64 * 1024)
            .and_then(|_| std::fs::read_to_string(folder.join("MEMORY.md")).ok())
            .map(|text| text.chars().take(6000).collect());
        let artifact = latest.artifact_path().join(format!("{}.app", app.name));
        let expires_in_days = tohseno_engine::gates::sign::days_until_expiry(&artifact);
        apps.push(LibraryApp {
            icon_url: format!(
                "/api/icon/{app_name}/{latest_evolution}",
                app_name = app.name
            ),
            folder: folder.display().to_string(),
            unrecorded_changes,
            memory,
            expires_in_days,
            name: app.name,
            latest_evolution,
            shots,
            retired: app.retired,
        });
    }
    let body = serde_json::to_string(&LibraryResponse {
        apps,
        iphone_slots_used,
        iphone_slot_limit: 3,
    })?;
    respond(socket, 200, "application/json; charset=utf-8", &body).await?;
    Ok(())
}

async fn serve_onboarding(
    socket: &mut TcpStream,
    state: &State,
) -> Result<(), Box<dyn std::error::Error>> {
    let engine = Engine::discover(state.events.clone())?;
    let accepted_shots = engine
        .ledger()
        .list_apps()?
        .iter()
        .filter(|app| app.latest_evolution.is_some())
        .count();
    let harness_ready = engine.harnesses().iter().any(|harness| {
        harness.installed
            && harness.routes.iter().any(|route| {
                route.available
                    && route.billing == "subscription"
                    && route.estimated_additional_cost_usd == Some(0.0)
            })
    });
    let xcode = match tohseno_engine::gates::toolchain::check() {
        ToolchainState::Ready => OnboardingCheck {
            ready: true,
            status: "ready",
            detail: "The selected Xcode toolchain responds and can build native Apple expressions."
                .into(),
        },
        ToolchainState::Missing => OnboardingCheck {
            ready: false,
            status: "action_required",
            detail:
                "Install Xcode from the Mac App Store, open it once, and accept its setup prompts."
                    .into(),
        },
    };
    let apple_signing = match tohseno_engine::gates::apple_signing::check() {
        AppleSigningState::Ready { .. } => OnboardingCheck {
            ready: true,
            status: "ready",
            detail:
                "Xcode has an Apple Development identity associated with one of its signed-in teams."
                    .into(),
        },
        AppleSigningState::Missing => OnboardingCheck {
            ready: false,
            status: "action_required",
            detail: "In Xcode → Settings → Accounts, sign in and create an Apple Development certificate under Manage Certificates."
                .into(),
        },
    };
    let ready_for_first_shot = xcode.ready && apple_signing.ready && harness_ready;
    let body = serde_json::to_string(&OnboardingResponse {
        schema: "tohseno.studio-onboarding/1",
        version: env!("CARGO_PKG_VERSION"),
        first_run: accepted_shots == 0,
        accepted_shots,
        xcode,
        apple_signing,
        harness_ready,
        ready_for_first_shot,
    })?;
    respond(socket, 200, "application/json; charset=utf-8", &body).await?;
    Ok(())
}

async fn serve_shot_protocol(
    socket: &mut TcpStream,
    request_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut parts = request_path
        .trim_start_matches("/api/protocol/shot/")
        .split('/');
    let app_name = parts.next().ok_or("missing app name")?;
    if tohseno_engine::ledger::validate_app_name(app_name).is_err() {
        respond(socket, 400, "text/plain; charset=utf-8", "invalid app name").await?;
        return Ok(());
    }
    let shot_number = match parts.next().and_then(|value| value.parse::<u32>().ok()) {
        Some(number) if number > 0 => number,
        _ => {
            respond(
                socket,
                400,
                "text/plain; charset=utf-8",
                "invalid shot number",
            )
            .await?;
            return Ok(());
        }
    };
    if parts.next().is_some() {
        respond(socket, 404, "text/plain; charset=utf-8", "not found").await?;
        return Ok(());
    }

    let ledger = Ledger::discover()?;
    let app = match ledger.load_app(app_name) {
        Ok(app) => app,
        Err(_) => {
            respond(socket, 404, "text/plain; charset=utf-8", "not found").await?;
            return Ok(());
        }
    };
    let finish_evolutions = ledger.list_evolutions(app_name)?;
    let Some(shot) = finish_evolutions
        .into_iter()
        .find(|candidate| candidate.number == shot_number)
    else {
        respond(socket, 404, "text/plain; charset=utf-8", "not found").await?;
        return Ok(());
    };
    let ontology = shot_ontology_facts(&ledger, &app.name, shot_number)?;
    let body = serde_json::to_string(&shot_protocol_facts(
        &app.name,
        app.latest_evolution == Some(shot_number),
        &shot,
        ontology,
    ))?;
    respond(socket, 200, "application/json; charset=utf-8", &body).await?;
    Ok(())
}

fn shot_protocol_facts(
    app_name: &str,
    current: bool,
    shot: &tohseno_engine::Evolution,
    ontology: Option<ShotOntologyFacts>,
) -> ShotProtocolFacts {
    let record_path = Path::new("TOHSENO/shot.json");
    let record_exists = fs::symlink_metadata(shot.path.join(record_path)).is_ok();
    let record_result =
        read_protocol_json::<ShotRecord>(&shot.path, record_path).and_then(|record| {
            record
                .validate()
                .map_err(|error| format!("Shot record: {error}"))?;
            Ok(record)
        });
    let record = record_result.ok();

    let evolution = record.as_ref().map(|record| EvolutionFacts {
        shot_id: record.shot_id.to_string(),
        builder_id: record.builder_id.to_string(),
        sequence: record.sequence,
        previous: record.previous.map(|digest| digest.to_string()),
        commitment: record.commitment().ok().map(|digest| digest.to_string()),
        origin: if record.origin.is_some() {
            "legacy_adoption"
        } else if record.sequence == 1 {
            "create"
        } else {
            "evolve"
        },
    });

    let signature_result =
        read_protocol_json::<SignatureSidecar>(&shot.path, Path::new("TOHSENO/signature.json"))
            .and_then(|signature| {
                signature
                    .validate()
                    .map_err(|error| format!("Signature sidecar: {error}"))?;
                let record = record
                    .as_ref()
                    .ok_or_else(|| "A valid Shot record is required.".to_owned())?;
                record
                    .verify_signature(&signature)
                    .map_err(|error| format!("Signature verification: {error}"))?;
                Ok(signature)
            });
    let signature = match signature_result {
        Ok(signature) => EvidenceFacts {
            status: "valid",
            signer_device_key_id: Some(device_key_id(&signature.public_key).to_string()),
            algorithm: Some("p256"),
            low_s: Some(signature.low_s),
            detail: "The low-s DeviceKey signature verifies over the canonical Evolution record."
                .into(),
        },
        Err(error) => EvidenceFacts {
            status: if record_exists {
                "invalid"
            } else {
                "not_present"
            },
            signer_device_key_id: None,
            algorithm: None,
            low_s: None,
            detail: error,
        },
    };

    let fascia_result =
        read_protocol_json::<FasciaManifest>(&shot.path, Path::new("TOHSENO/fascia.json"))
            .and_then(|fascia| {
                fascia
                    .validate()
                    .map_err(|error| format!("Fascia manifest: {error}"))?;
                let record = record
                    .as_ref()
                    .ok_or_else(|| "A valid Shot record is required.".to_owned())?;
                if fascia.fascia != record.fascia
                    || fascia.distribution.bundle_id != record.bundle_id
                    || fascia.distribution.bundle_version != record.bundle_version
                {
                    return Err(
                        "Fascia identity or distribution does not bind to the Evolution.".into(),
                    );
                }
                Ok(fascia)
            });
    let fascia = match fascia_result {
        Ok(fascia) => FasciaFacts {
            status: "valid",
            id: Some(fascia.fascia),
            commitment: record
                .as_ref()
                .map(|record| record.fascia_sha256.to_string()),
            distribution: serde_json::to_value(fascia.distribution.state)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned)),
            capabilities: Some(fascia.capabilities.len()),
            detail: "The concrete Apple Fascia manifest validates and binds to this Evolution."
                .into(),
        },
        Err(error) => FasciaFacts {
            status: if record_exists {
                "invalid"
            } else {
                "not_present"
            },
            id: record.as_ref().map(|record| record.fascia.clone()),
            commitment: record
                .as_ref()
                .map(|record| record.fascia_sha256.to_string()),
            distribution: None,
            capabilities: None,
            detail: error,
        },
    };

    let conformance_result =
        read_protocol_json::<ConformanceReport>(&shot.path, Path::new("TOHSENO/conformance.json"))
            .and_then(|report| {
                report
                    .validate()
                    .map_err(|error| format!("Conformance receipt: {error}"))?;
                let record = record
                    .as_ref()
                    .ok_or_else(|| "A valid Shot record is required.".to_owned())?;
                if report.shot_id != record.shot_id || report.sequence != record.sequence {
                    return Err("Conformance receipt does not bind to this Evolution.".into());
                }
                Ok(report)
            });
    let conformance = match conformance_result {
        Ok(report) => {
            let (passed, failed, not_checked) = conformance_counts(&report);
            ConformanceFacts {
                status: if report.conformant { "pass" } else { "fail" },
                conformant: Some(report.conformant),
                passed,
                failed,
                not_checked,
                detail: if report.conformant {
                    "Every recorded conformance check passed.".into()
                } else {
                    "The stored conformance receipt records a failure or unchecked condition."
                        .into()
                },
            }
        }
        Err(error) => ConformanceFacts {
            status: if record_exists {
                "invalid"
            } else {
                "not_present"
            },
            conformant: None,
            passed: 0,
            failed: 0,
            not_checked: 0,
            detail: error,
        },
    };

    let verification = if !record_exists {
        VerificationFacts {
            status: "not_applicable",
            conformant: None,
            passed: 0,
            failed: 0,
            not_checked: 0,
            evolution_commitment: None,
            detail: "This is a legacy Shot. Use `tohseno adopt` to create an honest signed root Evolution.".into(),
        }
    } else {
        match reference_fascia_root() {
            Ok(reference) => {
                let report = verify_shot_directory(&shot.path, &reference);
                let passed = report
                    .checks
                    .iter()
                    .filter(|check| check.status == VerificationStatus::Pass)
                    .count();
                let failed = report
                    .checks
                    .iter()
                    .filter(|check| check.status == VerificationStatus::Fail)
                    .count();
                let not_checked = report
                    .checks
                    .iter()
                    .filter(|check| check.status == VerificationStatus::NotChecked)
                    .count();
                VerificationFacts {
                    status: if report.conformant { "pass" } else { "fail" },
                    conformant: Some(report.conformant),
                    passed,
                    failed,
                    not_checked,
                    evolution_commitment: report
                        .evolution_commitment
                        .map(|digest| digest.to_string()),
                    detail: if report.conformant {
                        "The offline engine verifier reproduced the signed Shot judgment.".into()
                    } else {
                        "The offline engine verifier found one or more failed checks.".into()
                    },
                }
            }
            Err(error) => VerificationFacts {
                status: "not_checked",
                conformant: None,
                passed: 0,
                failed: 0,
                not_checked: 1,
                evolution_commitment: record
                    .as_ref()
                    .and_then(|record| record.commitment().ok())
                    .map(|digest| digest.to_string()),
                detail: format!("Pinned Apple Fascia reference unavailable: {error}"),
            },
        }
    };

    let adoption_required = !record_exists;
    ShotProtocolFacts {
        app_name: app_name.to_owned(),
        ledger_shot: shot.number,
        current,
        adoption_required,
        local_state: "private",
        published_state: "not_published",
        source_published: false,
        registry_head: None,
        transaction_hash: None,
        evolution,
        signature,
        fascia,
        conformance,
        verification,
        handle: RelationFacts {
            status: "pending",
            value: None,
            detail: "No handle receipt exists in local evidence.",
        },
        appcoin: RelationFacts {
            status: "pending",
            value: None,
            detail: "No appcoin association receipt exists in local evidence.",
        },
        ontology,
    }
}

fn shot_ontology_facts(
    ledger: &Ledger,
    app_name: &str,
    version_ordinal: u32,
) -> Result<Option<ShotOntologyFacts>, Box<dyn std::error::Error>> {
    let root = ledger.working_tree(app_name);
    let layout = ShotLayout::at(&root);
    let lineage = layout.read_lineage()?;
    if lineage.is_empty() {
        return Ok(None);
    }
    let state = tohseno_protocol::reduce_lineage(&lineage)?;
    let mut matching = state.expressions.values().filter_map(|expression| {
        expression
            .versions
            .iter()
            .find(|version| version.ordinal == u64::from(version_ordinal))
            .map(|version| (expression.expression.clone(), version.clone()))
    });
    let Some((expression, version)) = matching.next() else {
        return Ok(None);
    };
    if matching.next().is_some() {
        return Err("multiple expressions claim the selected version ordinal".into());
    }
    let (acceptance_action, acceptance) = state
        .genome_acceptances
        .iter()
        .find(|(_, acceptance)| {
            acceptance.revision == version.genome_revision
                && acceptance.genome_digest == version.genome_digest
        })
        .ok_or("the selected v2 Version has no matching accepted Shot genome")?;
    let proposal = state
        .genome_proposals
        .get(&acceptance.proposal_action)
        .ok_or("the accepted Shot genome proposal is unavailable")?;
    let accepted_genome = AcceptedGenome {
        genome: proposal.proposed.clone(),
        proposal_action: acceptance.proposal_action,
        acceptance_action: *acceptance_action,
    };
    if accepted_genome.genome.digest()? != version.genome_digest {
        return Err("the selected Version does not bind the accepted Shot genome".into());
    }
    let intention_path = safe_regular_file(&root, Path::new("INTENTION.md"))
        .map_err(|error| format!("original intention: {error}"))?;
    let intention_metadata = fs::symlink_metadata(&intention_path)?;
    if intention_metadata.len() > MAX_PROTOCOL_JSON {
        return Err("original intention exceeds the Studio display limit".into());
    }
    let exact = String::from_utf8(fs::read(intention_path)?)
        .map_err(|_| "original intention is not valid UTF-8")?;
    let exact_digest = tohseno_protocol::digest::sha256(exact.as_bytes());
    let intention_matches = state.intention.as_ref().is_some_and(|intention| {
        intention.materials.iter().any(|material| {
            material.artifact.artifact.digest == exact_digest
                && material.artifact.artifact.byte_length
                    == u64::try_from(exact.len()).unwrap_or(u64::MAX)
        })
    });
    if !intention_matches {
        return Err("original intention bytes do not match signed lineage".into());
    }
    let token_association = match &state.token_association {
        Some(association) => {
            let current_action = state
                .token_history
                .last()
                .filter(|entry| entry.record == *association)
                .map(|entry| entry.action.to_string());
            OntologyTokenAssociationFacts {
                status: "associated",
                current_action,
                chain_id: Some(association.chain_id),
                token_address: Some(association.token.to_string()),
                symbol: association.symbol.clone(),
                anchor_declared: association.anchor.is_some(),
                // A declared anchor remains an unverified claim until a
                // chain-specific verifier checks its transaction and chain.
                anchor_verified: false,
                history_count: state.token_history.len(),
                identity_role: "relationship_only",
            }
        }
        None => OntologyTokenAssociationFacts {
            status: "absent",
            current_action: None,
            chain_id: None,
            token_address: None,
            symbol: None,
            anchor_declared: false,
            anchor_verified: false,
            history_count: state.token_history.len(),
            identity_role: "relationship_only",
        },
    };
    Ok(Some(ShotOntologyFacts {
        status: "verified",
        shot_id: state.shot_id.to_string(),
        original_intention: OriginalIntentionFacts {
            status: "locally_available",
            exact,
        },
        accepted_genome,
        expression,
        version,
        token_association,
        lineage: OntologyLineageFacts {
            sequence: state.sequence,
            head: state.head.to_string(),
            verification: "verified",
            availability: "locally_available",
        },
        detail:
            "Derived from the locally verified signed lineage; private bytes remain on this Mac.",
    }))
}

fn conformance_counts(report: &ConformanceReport) -> (usize, usize, usize) {
    let passed = report
        .checks
        .iter()
        .filter(|check| check.status == CheckStatus::Pass)
        .count();
    let failed = report
        .checks
        .iter()
        .filter(|check| check.status == CheckStatus::Fail)
        .count();
    let not_checked = report
        .checks
        .iter()
        .filter(|check| check.status == CheckStatus::NotChecked)
        .count();
    (passed, failed, not_checked)
}

fn read_protocol_json<T: DeserializeOwned>(root: &Path, relative: &Path) -> Result<T, String> {
    let path = safe_regular_file(root, relative)?;
    let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
    if metadata.len() > MAX_PROTOCOL_JSON {
        return Err(format!(
            "{} exceeds the protocol JSON size limit.",
            relative.display()
        ));
    }
    serde_json::from_slice(&fs::read(path).map_err(|error| error.to_string())?)
        .map_err(|error| format!("{} is not closed valid JSON: {error}", relative.display()))
}

fn safe_regular_file(root: &Path, relative: &Path) -> Result<PathBuf, String> {
    if !root.is_dir()
        || relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err("Protocol evidence path is unsafe.".into());
    }
    let mut path = root.to_path_buf();
    let components = relative.components().collect::<Vec<_>>();
    for (index, component) in components.iter().enumerate() {
        let std::path::Component::Normal(component) = component else {
            return Err("Protocol evidence path is unsafe.".into());
        };
        path.push(component);
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("{}: {error}", relative.display()))?;
        if metadata.file_type().is_symlink()
            || (index + 1 == components.len() && !metadata.is_file())
            || (index + 1 != components.len() && !metadata.is_dir())
        {
            return Err(format!(
                "{} must stay within the Shot as regular directories and a regular file.",
                relative.display()
            ));
        }
    }
    Ok(path)
}

async fn serve_icon(
    socket: &mut TcpStream,
    request_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut parts = request_path.trim_start_matches("/api/icon/").split('/');
    let app_name = parts.next().ok_or("missing app name")?;
    tohseno_engine::ledger::validate_app_name(app_name)?;
    let shot_number = parts.next().ok_or("missing shot number")?.parse::<u32>()?;
    if parts.next().is_some() {
        respond(socket, 404, "text/plain; charset=utf-8", "not found").await?;
        return Ok(());
    }
    let ledger = Ledger::discover()?;
    let shot = ledger.shot(app_name, shot_number)?;
    if !ledger
        .list_evolutions(app_name)?
        .iter()
        .any(|candidate| candidate.number == shot_number)
    {
        respond(socket, 404, "text/plain; charset=utf-8", "not found").await?;
        return Ok(());
    }
    let Some(icon) = find_app_icon(&shot.source_path())? else {
        respond(socket, 404, "text/plain; charset=utf-8", "not found").await?;
        return Ok(());
    };
    let content_type = match icon
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        _ => "image/png",
    };
    respond_bytes(socket, 200, content_type, &fs::read(icon)?).await?;
    Ok(())
}

async fn launch_simulator(
    socket: &mut TcpStream,
    body: &[u8],
    state: &State,
) -> Result<(), Box<dyn std::error::Error>> {
    let launch: SimulatorLaunch = match serde_json::from_slice(body) {
        Ok(launch) => launch,
        Err(error) => {
            respond(
                socket,
                400,
                "text/plain; charset=utf-8",
                &format!("invalid Simulator launch: {error}"),
            )
            .await?;
            return Ok(());
        }
    };
    tohseno_engine::ledger::validate_app_name(&launch.app_name)?;
    let _guard = state.press.lock().await;
    let ledger = Ledger::discover()?;
    match simulator::launch(&ledger, &state.events, &launch.app_name, launch.shot).await {
        Ok(session) => {
            *state.simulator.lock().await = Some(session);
            respond(
                socket,
                200,
                "application/json; charset=utf-8",
                r#"{"running":true}"#,
            )
            .await?;
        }
        Err(error) => {
            state
                .events
                .emit(Event::status(format!("Simulator stopped: {error}")));
            respond(socket, 500, "text/plain; charset=utf-8", &error.to_string()).await?;
        }
    }
    Ok(())
}

async fn serve_simulator_screen(
    socket: &mut TcpStream,
    state: &State,
) -> Result<(), Box<dyn std::error::Error>> {
    let session = state.simulator.lock().await.clone();
    let Some(session) = session else {
        respond(socket, 404, "text/plain; charset=utf-8", "not running").await?;
        return Ok(());
    };
    match simulator::screenshot(&session).await {
        Ok(image) => respond_bytes(socket, 200, "image/png", &image).await?,
        Err(error) => respond(socket, 500, "text/plain; charset=utf-8", &error.to_string()).await?,
    }
    Ok(())
}

fn find_app_icon(source: &Path) -> std::io::Result<Option<PathBuf>> {
    let mut candidates = Vec::new();
    collect_icons(source, false, &mut candidates)?;
    candidates.sort_by_key(|path| {
        fs::metadata(path)
            .map(|metadata| std::cmp::Reverse(metadata.len()))
            .unwrap_or(std::cmp::Reverse(0))
    });
    Ok(candidates.into_iter().next())
}

fn collect_icons(
    directory: &Path,
    inside_icon_directory: bool,
    candidates: &mut Vec<PathBuf>,
) -> std::io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        let is_icon_directory = inside_icon_directory || name.contains("appicon");
        if entry.file_type()?.is_dir() {
            collect_icons(&path, is_icon_directory, candidates)?;
        } else if is_icon_directory
            && path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    ["png", "jpg", "jpeg", "webp"]
                        .iter()
                        .any(|candidate| extension.eq_ignore_ascii_case(candidate))
                })
        {
            candidates.push(path);
        }
    }
    Ok(())
}

async fn stage_images(
    directory: &Path,
    images: Vec<UploadedImage>,
) -> Result<Vec<PathBuf>, String> {
    if images.len() > tohseno_engine::gates::intent::MAX_IMAGES {
        return Err("at most eight reference images may be attached".into());
    }
    let mut paths = Vec::new();
    for (index, image) in images.into_iter().enumerate() {
        let original = Path::new(&image.name)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("image");
        let extension_is_valid = Path::new(original)
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                ["png", "jpg", "jpeg", "heic", "webp"]
                    .iter()
                    .any(|candidate| extension.eq_ignore_ascii_case(candidate))
            });
        if !extension_is_valid {
            return Err(format!(
                "attachment {} has an unsupported filename or extension",
                index + 1
            ));
        }
        let image_directory = directory.join(index.to_string());
        tokio::fs::create_dir(&image_directory)
            .await
            .map_err(|error| error.to_string())?;
        let path = image_directory.join(original);
        let bytes = STANDARD
            .decode(image.data)
            .map_err(|error| error.to_string())?;
        if bytes.is_empty() {
            return Err(format!("attachment {} is empty", index + 1));
        }
        tokio::fs::write(&path, bytes)
            .await
            .map_err(|error| error.to_string())?;
        paths.push(path);
    }
    Ok(paths)
}

async fn stream_events(
    mut socket: TcpStream,
    events: EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    socket
        .write_all(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nContent-Security-Policy: default-src 'none'; connect-src 'self'; frame-ancestors 'none'\r\nReferrer-Policy: no-referrer\r\nConnection: keep-alive\r\nX-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\n\r\n",
        )
        .await?;
    let mut receiver = events.subscribe();
    loop {
        let event = match receiver.recv().await {
            Ok(event) => event,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                Event::status("the studio display skipped earlier lines.")
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => return Ok(()),
        };
        let encoded = serde_json::to_string(&event)?;
        if socket
            .write_all(format!("data: {encoded}\n\n").as_bytes())
            .await
            .is_err()
        {
            return Ok(());
        }
    }
}

struct Request {
    method: String,
    path: String,
    headers: BTreeMap<String, Vec<String>>,
    body: Vec<u8>,
}

async fn read_request(socket: &mut TcpStream) -> Result<Request, Box<dyn std::error::Error>> {
    let mut bytes = Vec::with_capacity(4096);
    let mut buffer = [0_u8; 8192];
    let header_end = loop {
        let read = socket.read(&mut buffer).await?;
        if read == 0 {
            return Err("connection closed before request headers".into());
        }
        bytes.extend_from_slice(&buffer[..read]);
        if let Some(position) = find_bytes(&bytes, b"\r\n\r\n") {
            if position + 4 > MAX_HEADERS {
                return Err("request headers are too large".into());
            }
            break position + 4;
        }
        if bytes.len() > MAX_HEADERS {
            return Err("request headers are too large".into());
        }
    };
    let headers = std::str::from_utf8(&bytes[..header_end])?;
    let mut lines = headers.lines();
    let request_line = lines.next().ok_or("missing request line")?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().ok_or("missing method")?.to_owned();
    let target = parts.next().ok_or("missing path")?;
    let version = parts.next().ok_or("missing HTTP version")?;
    if parts.next().is_some()
        || version != "HTTP/1.1"
        || !matches!(method.as_str(), "GET" | "POST")
        || !target.starts_with('/')
    {
        return Err("invalid HTTP request line".into());
    }
    let path = target.split('?').next().unwrap_or("/").to_owned();
    let mut header_map = BTreeMap::<String, Vec<String>>::new();
    for line in lines.filter(|line| !line.is_empty()) {
        let (name, value) = line.split_once(':').ok_or("invalid HTTP header")?;
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            || value.contains('\r')
            || value.contains('\n')
        {
            return Err("invalid HTTP header".into());
        }
        header_map
            .entry(name.to_ascii_lowercase())
            .or_default()
            .push(value.trim().to_owned());
    }
    if header_map.contains_key("transfer-encoding") {
        return Err("transfer encoding is not supported".into());
    }
    let content_length = match header_map.get("content-length") {
        None => 0,
        Some(values) if values.len() == 1 => values[0].parse::<usize>()?,
        Some(_) => return Err("ambiguous content length".into()),
    };
    if content_length > MAX_BODY {
        return Err("request body is too large".into());
    }
    while bytes.len() - header_end < content_length {
        let read = socket.read(&mut buffer).await?;
        if read == 0 {
            return Err("connection closed before request body".into());
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    Ok(Request {
        method,
        path,
        headers: header_map,
        body: bytes[header_end..header_end + content_length].to_vec(),
    })
}

fn request_has_expected_authority(request: &Request, authority: &str) -> bool {
    request
        .headers
        .get("host")
        .is_some_and(|values| values.len() == 1 && values[0] == authority)
}

fn request_is_same_origin_json(request: &Request, origin: &str) -> bool {
    let exact_header = |name: &str, expected: &str| {
        request
            .headers
            .get(name)
            .is_some_and(|values| values.len() == 1 && values[0] == expected)
    };
    let json_content_type = request
        .headers
        .get("content-type")
        .filter(|values| values.len() == 1)
        .and_then(|values| values.first())
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("application/json"));
    exact_header("origin", origin) && exact_header("x-tohseno-studio", "1") && json_content_type
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

async fn respond(
    socket: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &str,
) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        422 => "Unprocessable Content",
        500 => "Internal Server Error",
        _ => "Error",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nContent-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' blob: data:; connect-src 'self'; object-src 'none'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'\r\nReferrer-Policy: no-referrer\r\nX-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    socket.write_all(response.as_bytes()).await
}

async fn respond_bytes(
    socket: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let reason = if status == 200 { "OK" } else { "Error" };
    let headers = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nContent-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' blob: data:; connect-src 'self'; object-src 'none'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'\r\nReferrer-Policy: no-referrer\r\nX-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\nConnection: close\r\n\r\n",
        body.len()
    );
    socket.write_all(headers.as_bytes()).await?;
    socket.write_all(body).await
}

async fn respond_pairing_svg(socket: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
    if body.len() > MAX_PAIRING_SVG {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "pairing SVG exceeds output limit",
        ));
    }
    let headers = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: image/svg+xml; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nContent-Security-Policy: default-src 'none'; style-src 'none'; script-src 'none'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; sandbox\r\nCross-Origin-Resource-Policy: same-origin\r\nReferrer-Policy: no-referrer\r\nX-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\nConnection: close\r\n\r\n",
        body.len()
    );
    socket.write_all(headers.as_bytes()).await?;
    socket.write_all(body).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(headers: &[(&str, &str)]) -> Request {
        let mut collected = BTreeMap::<String, Vec<String>>::new();
        for (name, value) in headers {
            collected
                .entry((*name).to_owned())
                .or_default()
                .push((*value).to_owned());
        }
        Request {
            method: "POST".into(),
            path: "/shots".into(),
            headers: collected,
            body: Vec::new(),
        }
    }

    fn pairing_payload() -> PairingTargetPayload {
        PairingTargetPayload {
            format: PAIRING_FORMAT.into(),
            version: 1,
            encodes: PAIRING_DESCRIPTION.into(),
            request_schema: PAIRING_SCHEMA.into(),
            builder_id: BuilderId::new(Address20::from_bytes([1; 20])),
            network: PairingTargetNetwork {
                chain_id: ROBINHOOD_CHAIN_ID,
                builder_account: Address20::from_bytes([1; 20]),
                builder_account_factory: Address20::from_bytes([2; 20]),
            },
        }
    }

    #[test]
    fn locates_the_http_header_boundary() {
        assert_eq!(
            find_bytes(b"GET / HTTP/1.1\r\n\r\nbody", b"\r\n\r\n"),
            Some(14)
        );
    }

    #[test]
    fn selects_the_largest_image_from_the_app_icon_set() {
        let directory = tempfile::tempdir().unwrap();
        let icons = directory.path().join("Assets.xcassets/AppIcon.appiconset");
        fs::create_dir_all(&icons).unwrap();
        fs::write(icons.join("small.png"), [1_u8]).unwrap();
        fs::write(icons.join("large.png"), [1_u8; 64]).unwrap();
        fs::write(directory.path().join("unrelated.png"), [1_u8; 128]).unwrap();
        assert_eq!(
            find_app_icon(directory.path()).unwrap(),
            Some(icons.join("large.png"))
        );
    }

    #[test]
    fn mutation_requires_exact_host_origin_json_and_studio_header() {
        let valid = request(&[
            ("host", "127.0.0.1:7331"),
            ("origin", "http://127.0.0.1:7331"),
            ("content-type", "application/json; charset=utf-8"),
            ("x-tohseno-studio", "1"),
        ]);
        assert!(request_has_expected_authority(&valid, "127.0.0.1:7331"));
        assert!(request_is_same_origin_json(&valid, "http://127.0.0.1:7331"));

        let hostile = request(&[
            ("host", "127.0.0.1:7331"),
            ("origin", "https://example.invalid"),
            ("content-type", "application/json"),
            ("x-tohseno-studio", "1"),
        ]);
        assert!(!request_is_same_origin_json(
            &hostile,
            "http://127.0.0.1:7331"
        ));

        let duplicate_host = request(&[("host", "127.0.0.1:7331"), ("host", "example.invalid")]);
        assert!(!request_has_expected_authority(
            &duplicate_host,
            "127.0.0.1:7331"
        ));
    }

    #[test]
    fn feedback_submission_is_closed_and_exact_version_bound() {
        let request: FeedbackSubmission = serde_json::from_str(
            r#"{"app_name":"field-notebook","version_ordinal":2,"text":"The save affordance was unclear."}"#,
        )
        .unwrap();
        assert_eq!(request.app_name, "field-notebook");
        assert_eq!(request.version_ordinal, 2);
        assert_eq!(request.text, "The save affordance was unclear.");
        assert!(serde_json::from_str::<FeedbackSubmission>(
            r#"{"app_name":"field-notebook","version_ordinal":2,"text":"x","version_id":"invented"}"#
        )
        .is_err());
    }

    #[test]
    fn embedded_candidate_never_enables_studio_publish() {
        let (_, network) = deployment_facts();
        assert_eq!(network.chain_id, ROBINHOOD_CHAIN_ID);
        assert_eq!(network.connectivity, "not_queried");
        assert!(!network.deployment_evidence);
        assert_eq!(network.deployment_status, "planned_undeployed");
        assert_eq!(network.contracts.len(), 3);
    }

    #[test]
    fn pairing_target_is_strict_public_canonical_json() {
        let payload = pairing_payload();
        payload.validate().unwrap();
        let encoded = canonical::to_string(&payload).unwrap();
        assert!(encoded.len() <= MAX_PAIRING_PAYLOAD);
        assert!(encoded.contains(PAIRING_DESCRIPTION));
        assert!(!encoded.contains("local_key_tag"));
        assert!(!encoded.contains("\"private_key\":"));
        assert!(!encoded.contains("\"signature\":"));

        let decoded = canonical::from_slice::<PairingTargetPayload>(encoded.as_bytes()).unwrap();
        assert_eq!(decoded, payload);

        let mut hostile = serde_json::to_value(&payload).unwrap();
        hostile
            .as_object_mut()
            .unwrap()
            .insert("secret".into(), serde_json::Value::String("no".into()));
        assert!(canonical::from_slice::<PairingTargetPayload>(
            &serde_json::to_vec(&hostile).unwrap()
        )
        .is_err());
    }

    #[test]
    fn pairing_svg_is_deterministic_bounded_and_does_not_interpolate_input() {
        let encoded = canonical::to_string(&pairing_payload()).unwrap();
        let first = render_pairing_svg(&encoded).unwrap();
        let second = render_pairing_svg(&encoded).unwrap();
        assert_eq!(first, second);
        assert!(first.starts_with("<?xml version=\"1.0\""));
        assert!(first.contains("<svg "));
        assert!(first.len() <= MAX_PAIRING_SVG);
        assert!(!first.contains(&encoded));

        let hostile = r#"</path><script>alert("qr")</script>"#;
        let hostile_svg = render_pairing_svg(hostile).unwrap();
        assert!(!hostile_svg.contains("<script"));
        assert!(!hostile_svg.contains(hostile));
        assert!(render_pairing_svg(&"x".repeat(MAX_PAIRING_PAYLOAD + 1)).is_err());
    }

    #[test]
    fn pairing_qr_is_unavailable_without_validated_builder_context() {
        let facts = pairing_facts(None);
        assert!(!facts.qr_available);
        assert!(facts.qr_url.is_none());
        assert!(facts.target_payload.is_none());
    }

    #[test]
    fn protocol_assets_expose_plain_language_and_advanced_facts() {
        for marker in [
            "Your identity",
            "This Mac",
            "Private",
            "Conformance",
            "Pairing target QR",
            "Experimental publish",
            "Advanced inspector",
        ] {
            assert!(INDEX.contains(marker), "missing Studio marker: {marker}");
        }
        assert!(SCRIPT.contains("x-tohseno-studio"));
        assert!(SCRIPT.contains("No public registry receipt or transaction is present."));
    }

    #[cfg(unix)]
    #[test]
    fn protocol_reader_rejects_symlinked_evidence() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let shot = directory.path().join("shot");
        fs::create_dir_all(shot.join("TOHSENO")).unwrap();
        fs::write(directory.path().join("outside.json"), b"{}").unwrap();
        symlink(
            directory.path().join("outside.json"),
            shot.join("TOHSENO/shot.json"),
        )
        .unwrap();
        assert!(safe_regular_file(&shot, Path::new("TOHSENO/shot.json")).is_err());
    }
}
