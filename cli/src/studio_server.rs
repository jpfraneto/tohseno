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
use tohseno_engine::gates::intent::Intent;
use tohseno_engine::protocol_lifecycle::reference_fascia_root;
use tohseno_engine::verifier::{verify_shot_directory, VerificationStatus};
use tohseno_engine::{Engine, Event, EventBus, Ledger, ShotRequest};
use tohseno_protocol::builder::PAIRING_SCHEMA;
use tohseno_protocol::canonical;
use tohseno_protocol::conformance::{CheckStatus, ConformanceReport};
use tohseno_protocol::digest::Address20;
use tohseno_protocol::fascia::FasciaManifest;
use tohseno_protocol::identity::{device_key_id, BuilderId, ROBINHOOD_CHAIN_ID};
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
    authority: String,
    origin: String,
}

#[derive(Debug, Deserialize)]
struct ShotSubmission {
    mode: ShotMode,
    app_name: String,
    prompt: String,
    #[serde(default)]
    harness: Option<String>,
    #[serde(default)]
    images: Vec<UploadedImage>,
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

#[derive(Debug, Serialize)]
struct LibraryResponse {
    apps: Vec<LibraryApp>,
    iphone_slots_used: usize,
    iphone_slot_limit: usize,
}

#[derive(Debug, Serialize)]
struct LibraryApp {
    name: String,
    latest_shot: u32,
    shots: Vec<u32>,
    retired: bool,
    icon_url: String,
}

#[derive(Debug, Serialize)]
struct HarnessesResponse {
    harnesses: Vec<tohseno_engine::HarnessOption>,
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
    if request.method == "GET" && request.path == "/api/protocol" {
        return serve_protocol_overview(&mut socket).await;
    }
    if request.method == "GET" && request.path == PAIRING_QR_PATH {
        return serve_pairing_qr(&mut socket).await;
    }
    if request.method == "GET" && request.path.starts_with("/api/protocol/shot/") {
        return serve_shot_protocol(&mut socket, &request.path).await;
    }
    if request.method == "GET" && request.path.starts_with("/api/icon/") {
        return serve_icon(&mut socket, &request.path).await;
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
            let staging = tempfile::tempdir()?;
            let image_paths = stage_images(staging.path(), submission.images).await?;
            respond(
                &mut socket,
                202,
                "application/json; charset=utf-8",
                r#"{"accepted":true}"#,
            )
            .await?;
            socket.shutdown().await?;

            let events = state.events.clone();
            let press = state.press.clone();
            let _staging = staging;
            let _guard = press.lock().await;
            let request = ShotRequest {
                app_name: submission.app_name,
                intent: Intent::parse(&submission.prompt).with_images(image_paths),
                harness: submission.harness,
            };
            let outcome = match Engine::discover(events.clone()) {
                Ok(engine) => match submission.mode {
                    ShotMode::Create => engine.create(request).await.map(|_| ()),
                    ShotMode::Evolve => engine.evolve(request).await.map(|_| ()),
                },
                Err(error) => Err(error),
            };
            if let Err(error) = outcome {
                events.emit(Event::status(format!("engine stopped: {error}")));
            }
        }
        _ => respond(&mut socket, 404, "text/plain; charset=utf-8", "not found").await?,
    }
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
        .filter(|app| !app.retired && app.latest_shot.is_some())
        .count();
    let mut apps = Vec::new();
    for app in records {
        let Some(latest_shot) = app.latest_shot else {
            continue;
        };
        let shots = ledger
            .list_shots(&app.name)?
            .into_iter()
            .map(|shot| shot.number)
            .collect();
        apps.push(LibraryApp {
            icon_url: format!("/api/icon/{app_name}/{latest_shot}", app_name = app.name),
            name: app.name,
            latest_shot,
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
    let complete_shots = ledger.list_shots(app_name)?;
    let Some(shot) = complete_shots
        .into_iter()
        .find(|candidate| candidate.number == shot_number)
    else {
        respond(socket, 404, "text/plain; charset=utf-8", "not found").await?;
        return Ok(());
    };
    let body = serde_json::to_string(&shot_protocol_facts(
        &app.name,
        app.latest_shot == Some(shot_number),
        &shot,
    ))?;
    respond(socket, 200, "application/json; charset=utf-8", &body).await?;
    Ok(())
}

fn shot_protocol_facts(
    app_name: &str,
    current: bool,
    shot: &tohseno_engine::Shot,
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
    }
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
        .list_shots(app_name)?
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
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
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
            continue;
        }
        let image_directory = directory.join(index.to_string());
        tokio::fs::create_dir(&image_directory).await?;
        let path = image_directory.join(original);
        let bytes = STANDARD.decode(image.data)?;
        tokio::fs::write(&path, bytes).await?;
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
        202 => "Accepted",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
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
