use crate::{
    HardwareWallet, ProtocolCommand, PublicMutationArgs,
    EXACT_BUILDER_ACCOUNT_DEPLOYMENT_CONFIRMATION,
};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tohseno_engine::builder_identity::{BuilderDeploymentStatus, BuilderIdentityManager};
use tohseno_engine::page;
use tohseno_engine::protocol_lifecycle;
use tohseno_engine::public_actions::{
    self, bind_public_authority, persist_builder_deployment_request, persist_signed_package,
    plan_public_action, sign_planned_action, PlanOutcome, PublicActionRequest,
    SignedPublicActionPackage,
};
use tohseno_engine::public_network::{
    self, CurlTransport, DeploymentPlan, PublicCheckStatus, PublicShotVerificationReport,
    RelationRead, RpcUrl, DEFAULT_ROBINHOOD_RPC_URL,
};
use tohseno_engine::public_submission::{
    self, RelayerWallet, SubmissionConfig, SubmittedTransaction,
};
use tohseno_engine::verifier::{
    self, LineageVerificationReport, ShotVerificationReport, VerificationCheck, VerificationStatus,
};
use tohseno_engine::{Event, EventBus, Evolution, Ledger, ShotBodyVerification, ShotLayout};
use tohseno_protocol::fascia::FasciaManifest;
use tohseno_protocol::record::ShotRecord;
use tohseno_protocol::signature::SignatureSidecar;

const VECTORS: &str = include_str!("../../protocol/test-vectors/protocol-v1.json");
const DEPLOYMENT: &str = include_str!("../../contracts/deployments/robinhood-mainnet-v0.8.0.json");

pub fn protocol_command(
    command: ProtocolCommand,
    json: bool,
    bus: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        ProtocolCommand::Info => {
            let deployment: serde_json::Value = serde_json::from_str(DEPLOYMENT)?;
            let info = ProtocolInfo {
                protocol: "tohseno",
                candidate_version: "0.7.0",
                shot_schema: tohseno_protocol::record::SHOT_SCHEMA,
                compatibility_shot_schema: tohseno_protocol::record::SHOT_SCHEMA,
                lineage_protocol_version: tohseno_protocol::lineage::LINEAGE_PROTOCOL_VERSION,
                lineage_action_schema: tohseno_protocol::lineage::LINEAGE_ACTION_SCHEMA,
                lineage_schema_version: tohseno_protocol::lineage::LINEAGE_SCHEMA_VERSION,
                signature_schema: SignatureSidecar::SCHEMA,
                fascia: tohseno_protocol::record::APPLE_FASCIA_ID,
                app_metadata_schemas: [
                    tohseno_protocol::app_metadata::APP_METADATA_SCHEMA,
                    tohseno_protocol::app_metadata::APP_METADATA_V2_SCHEMA,
                ],
                chain_id: tohseno_protocol::identity::ROBINHOOD_CHAIN_ID,
                registry_chain_id: tohseno_protocol::identity::ROBINHOOD_CHAIN_ID,
                supported_token_association_chains: [
                    tohseno_protocol::identity::ROBINHOOD_CHAIN_ID,
                    8_453,
                ],
                canonical_release: false,
                deployment,
            };
            if json {
                print_json(&info)?;
            } else {
                bus.emit(Event::result(
                    "TOHSENO GENESIS 0.7.0 · protocol candidate, not canonical.",
                ));
                bus.emit(Event::status(
                    "Robinhood Chain 4663 · deterministic deployment planned, not deployed.",
                ));
            }
        }
        ProtocolCommand::Vectors => {
            if json {
                let vectors: serde_json::Value = serde_json::from_str(VECTORS)?;
                print_json(&vectors)?;
            } else {
                let value: serde_json::Value = serde_json::from_str(VECTORS)?;
                bus.emit(Event::result(
                    "the frozen cross-language protocol vectors are valid JSON.",
                ));
                bus.emit(Event::status(format!(
                    "{} · candidate 0.7.0",
                    value
                        .get("schema")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("protocol vector schema unavailable")
                )));
            }
        }
        ProtocolCommand::VerifyRecord { path } => {
            let (record, signature) = read_record_pair(&path)?;
            record.verify_signature(&signature)?;
            let result = RecordVerification {
                conformant: true,
                record_path: path.display().to_string(),
                shot_id: record.shot_id.to_string(),
                builder_id: record.builder_id.to_string(),
                sequence: record.sequence,
                commitment: record.commitment()?.to_string(),
                signer_key_id: tohseno_protocol::identity::device_key_id(&signature.public_key)
                    .to_string(),
            };
            if json {
                print_json(&result)?;
            } else {
                bus.emit(Event::result("record and low-s P-256 signature are valid."));
                bus.emit(Event::status(format!(
                    "Shot {} · Evolution {}.",
                    result.shot_id, result.sequence
                )));
            }
        }
    }
    Ok(())
}

pub fn inspect_target(
    target: &str,
    json: bool,
    bus: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    let shot_body = resolve_shot_body(target)?;
    let shot = match resolve_shot(target) {
        Ok(shot) => shot,
        Err(error) => {
            if let Some(body) = shot_body {
                let view = ShotBodyInspection::from_report(target, body);
                if json {
                    print_json(&view)?;
                } else {
                    bus.emit(Event::result(format!(
                        "Shot {} has a verified protocol {} body.",
                        view.report.shot_id, view.report.protocol_version
                    )));
                    bus.emit(Event::status(format!(
                        "{} signed action(s) · head {}.",
                        view.report.lineage_sequence, view.report.lineage_head
                    )));
                }
                return Ok(());
            }
            if report_unfinished_app(target, json, bus)? {
                return Err(
                    "no complete Shot yet; the attempt above is unfinished and unsigned".into(),
                );
            }
            return Err(error);
        }
    };
    if report_unfinished_app(target, json, bus)? {
        return Err("no complete Shot yet; the attempt above is unfinished and unsigned".into());
    }
    let fascia_reference = protocol_lifecycle::reference_fascia_root()?;
    let verification = verifier::verify_shot_directory(&shot.path, &fascia_reference);
    if !verification.conformant {
        return Err("Shot failed deterministic offline verification".into());
    }
    let record: ShotRecord =
        tohseno_protocol::canonical::from_slice(&fs::read(shot.path.join("TOHSENO/shot.json"))?)?;
    record.validate()?;
    let signature: SignatureSidecar = tohseno_protocol::canonical::from_slice(&fs::read(
        shot.path.join("TOHSENO/signature.json"),
    )?)?;
    record.verify_signature(&signature)?;
    let fascia: FasciaManifest =
        tohseno_protocol::canonical::from_slice(&fs::read(shot.path.join("TOHSENO/fascia.json"))?)?;
    fascia.validate()?;
    let conformance: tohseno_protocol::conformance::ConformanceReport =
        tohseno_protocol::canonical::from_slice(&fs::read(
            shot.path.join("TOHSENO/conformance.json"),
        )?)?;
    conformance.validate()?;
    let view = Inspection {
        app_name: shot.app_name,
        shot_directory: shot.path.display().to_string(),
        shot_id: record.shot_id.to_string(),
        builder_id: record.builder_id.to_string(),
        sequence: record.sequence,
        previous: record.previous.map(|value| value.to_string()),
        commitment: record.commitment()?.to_string(),
        source_tree_sha256: record.source_tree_sha256.to_string(),
        fascia_sha256: record.fascia_sha256.to_string(),
        signer_key_id: tohseno_protocol::identity::device_key_id(&signature.public_key).to_string(),
        conformant: conformance.conformant,
        public_state: "private",
        shot_body,
    };
    if json {
        print_json(&view)?;
    } else {
        bus.emit(Event::result(format!(
            "evolution {} is locally verified and private.",
            view.sequence
        )));
        bus.emit(Event::status(format!(
            "{} · {}",
            view.shot_id, view.builder_id
        )));
    }
    Ok(())
}

pub fn verify_target(
    target: &str,
    public: bool,
    json: bool,
    bus: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    let fascia_reference = protocol_lifecycle::reference_fascia_root()?;
    if report_unfinished_app(target, json, bus)? {
        return Err("no complete Shot yet; the attempt above is unfinished and unsigned".into());
    }
    let local = resolve_verification(target, &fascia_reference)?;
    let local_conformant = local.conformant();
    let public_check = if public && local_conformant {
        let record = record_for_target(target)?;
        let (transport, plan) = read_only_rpc()?;
        Some(public_network::verify_public_shot(
            &transport, &plan, &record,
        ))
    } else {
        None
    };
    let conformant = local_conformant && public_check.as_ref().is_none_or(|report| report.verified);
    let result = VerificationOutput {
        schema: "tohseno.cli-verification/1",
        conformant,
        local,
        public: public_check,
    };
    if json {
        print_json(&result)?;
    } else {
        render_verification(&result, bus);
    }
    if !result.conformant {
        return Err(if public && local_conformant {
            "public verification failed".into()
        } else {
            "offline Shot verification failed".into()
        });
    }
    Ok(())
}

pub fn build_page(
    app_name: &str,
    json: bool,
    bus: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    let ledger = Ledger::discover()?;
    let report = page::build(&ledger, app_name)?;
    if json {
        print_json(&report)?;
    } else {
        bus.emit(Event::result(format!(
            "private static Shot page built at {}.",
            report.output_path.display()
        )));
        bus.emit(Event::status(
            "generation did not publish source, registry state, or a public URL.",
        ));
    }
    Ok(())
}

pub fn network_status(json: bool, bus: &EventBus) -> Result<(), Box<dyn std::error::Error>> {
    let (transport, plan) = read_only_rpc()?;
    let status = public_network::network_status(&transport, &plan);
    if json {
        print_json(&status)?;
    } else {
        render_public_checks(&status.checks, bus);
        bus.emit(Event::status(if status.ready {
            "Robinhood Chain 4663 and every pinned candidate runtime are verified."
        } else {
            "read-only RPC completed; the candidate is not ready or remains undeployed."
        }));
    }
    Ok(())
}

pub fn registry_show(
    app_name: &str,
    json: bool,
    bus: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    let (shot, record) = verified_local_head(app_name)?;
    let (transport, plan) = read_only_rpc()?;
    let public = public_network::verify_public_shot(&transport, &plan, &record);
    let view = RegistryView {
        schema: "tohseno.registry-view/1",
        app_name: app_name.to_owned(),
        shot_id: record.shot_id.to_string(),
        local_head: record.commitment()?.to_string(),
        local_sequence: record.sequence,
        local_state: "private",
        registry_address: plan.contracts.shot_registry.planned_address.to_string(),
        registry_deployed: public.network.ready,
        public_checked: true,
        public_controller: public.observed.controller.map(|value| value.to_string()),
        public_head: public.observed.head.map(|value| value.to_string()),
        public_sequence: public.observed.sequence,
        transaction_hash: None,
        evidence_path: shot.path.join("TOHSENO/shot.json").display().to_string(),
        verification: public,
    };
    if json {
        print_json(&view)?;
    } else {
        bus.emit(Event::result(format!(
            "shot {} is locally verified and private.",
            record.sequence
        )));
        bus.emit(Event::status(if view.verification.verified {
            "the public controller, head, sequence, and relations binding match."
        } else {
            "read-only RPC found no matching public witness."
        }));
    }
    Ok(())
}

pub fn publish(
    app_name: &str,
    args: &PublicMutationArgs,
    json: bool,
    bus: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    prepare_public_action(
        "publish",
        app_name,
        args,
        PublicActionRequest::Publish,
        RelationRead::None,
        json,
        bus,
    )
}

pub fn claim_handle(
    handle: &str,
    app_name: &str,
    args: &PublicMutationArgs,
    json: bool,
    bus: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    let relation = RelationRead::Handle(public_actions::handle_hash(handle)?);
    prepare_public_action(
        "claim_handle",
        app_name,
        args,
        PublicActionRequest::ClaimHandle {
            handle: handle.into(),
        },
        relation,
        json,
        bus,
    )
}

pub fn associate_appcoin(
    app_name: &str,
    chain_id: u64,
    token_address: &str,
    args: &PublicMutationArgs,
    json: bool,
    bus: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    if chain_id == 0 {
        return Err("appcoin chain ID must not be zero".into());
    }
    let encoded = serde_json::to_string(token_address)?;
    let token: tohseno_protocol::digest::Address20 = serde_json::from_str(&encoded)?;
    if token.as_bytes().iter().all(|byte| *byte == 0) {
        return Err("appcoin token address must not be zero".into());
    }
    prepare_public_action(
        "associate_appcoin",
        app_name,
        args,
        PublicActionRequest::AssociateAppcoin { chain_id, token },
        RelationRead::Appcoin,
        json,
        bus,
    )
}

fn prepare_public_action(
    action: &'static str,
    app_name: &str,
    args: &PublicMutationArgs,
    request: PublicActionRequest,
    relation: RelationRead,
    json: bool,
    bus: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    let submission = submission_config(args)?;
    let ledger = Ledger::discover()?;
    let _public_lock = ledger.lock_public_actions()?;
    let _app_lock = ledger.lock_app(app_name)?;
    let (shot, record, record_signature) = verified_local_head_with_signature(app_name)?;
    let manager = BuilderIdentityManager::for_ledger(&ledger);
    let mut identity = manager.load()?;
    let authority = bind_public_authority(&identity, &record, &record_signature)?;
    let (transport, plan) = mutation_rpc(args)?;
    let mut read = public_network::read_public_preparation(
        &transport,
        &plan,
        &record,
        authority.account,
        authority.signer_key_id,
        relation,
    );
    let mut builder_transaction = None;
    let now = now_unix()?;
    let mut outcome = plan_public_action(
        &plan,
        &record,
        &authority,
        &read,
        request.clone(),
        args.deadline,
        now,
    )?;

    if let PlanOutcome::BuilderDeploymentRequired(deployment) = outcome.clone() {
        let artifact = persist_builder_deployment_request(&shot.path, &deployment)?;
        if submission.is_none() {
            return render_public_action_result(
                PublicActionResult {
                    schema: "tohseno.public-action-result/1",
                    action,
                    app_name: app_name.into(),
                    status: "builder_deployment_prepared",
                    artifact_path: Some(artifact.display().to_string()),
                    block_number: deployment.read_evidence.block_number,
                    signed: false,
                    submitted: false,
                    verified: false,
                    builder_transaction: None,
                    transaction: None,
                },
                json,
                bus,
            );
        }

        let fresh = public_network::read_public_preparation(
            &transport,
            &plan,
            &record,
            authority.account,
            authority.signer_key_id,
            relation,
        );
        let fresh_deployment = match plan_public_action(
            &plan,
            &record,
            &authority,
            &fresh,
            request.clone(),
            args.deadline,
            now_unix()?,
        )? {
            PlanOutcome::BuilderDeploymentRequired(value) => value,
            _ => {
                return Err(
                    "BuilderAccount state changed after preparation; no deployment was relayed"
                        .into(),
                )
            }
        };
        if !same_deployment_intent(&deployment, &fresh_deployment) {
            return Err(
                "BuilderAccount deployment intent changed after the final live read".into(),
            );
        }
        let receipt = with_builder_account_deployment_authorization(args, || {
            public_submission::submit_builder_account_deployment(
                submission.as_ref().expect("checked above"),
                &fresh_deployment,
                args.confirm_builder_account_deployment.as_deref(),
            )
        })?;
        builder_transaction = Some(receipt.clone());
        read = public_network::read_public_preparation(
            &transport,
            &plan,
            &record,
            authority.account,
            authority.signer_key_id,
            relation,
        );
        if read
            .block_number
            .is_none_or(|block| block < receipt.block_number)
        {
            return Err(format!(
                "BuilderAccount transaction {} mined, but the post-state read was not at or after its receipt block",
                receipt.transaction_hash
            )
            .into());
        }
        outcome = plan_public_action(
            &plan,
            &record,
            &authority,
            &read,
            request.clone(),
            args.deadline,
            now_unix()?,
        )
        .map_err(|error| {
            format!(
                "BuilderAccount transaction {} mined, but exact post-state verification failed: {error}",
                receipt.transaction_hash
            )
        })?;
        if matches!(outcome, PlanOutcome::BuilderDeploymentRequired(_)) {
            return Err(format!(
                "BuilderAccount transaction {} mined, but pinned runtime code is still absent",
                receipt.transaction_hash
            )
            .into());
        }
        identity.deployment_status = BuilderDeploymentStatus::Deployed;
        manager.save(&identity)?;
    } else if read
        .builder_account
        .as_ref()
        .is_some_and(|builder| builder.code_state == public_network::BuilderAccountCodeState::Exact)
        && identity.deployment_status != BuilderDeploymentStatus::Deployed
    {
        identity.deployment_status = BuilderDeploymentStatus::Deployed;
        manager.save(&identity)?;
    }

    match outcome {
        PlanOutcome::AlreadyCurrent(current) => render_public_action_result(
            PublicActionResult {
                schema: "tohseno.public-action-result/1",
                action,
                app_name: app_name.into(),
                status: "already_current",
                artifact_path: None,
                block_number: read.block_number,
                signed: false,
                submitted: false,
                verified: current.verified,
                builder_transaction,
                transaction: None,
            },
            json,
            bus,
        ),
        PlanOutcome::BuilderDeploymentRequired(_) => {
            Err("BuilderAccount remains undeployed after the guarded deployment path".into())
        }
        PlanOutcome::Action(planned) => {
            let package = sign_planned_action(&manager, &identity, *planned)?;
            let artifact = persist_signed_package(&shot.path, &package)?;
            let Some(submission) = submission.as_ref() else {
                return render_public_action_result(
                    PublicActionResult {
                        schema: "tohseno.public-action-result/1",
                        action,
                        app_name: app_name.into(),
                        status: "signed_preparation",
                        artifact_path: Some(artifact.display().to_string()),
                        block_number: package.read_evidence.block_number,
                        signed: true,
                        submitted: false,
                        verified: false,
                        builder_transaction,
                        transaction: None,
                    },
                    json,
                    bus,
                );
            };

            let fresh = public_network::read_public_preparation(
                &transport,
                &plan,
                &record,
                authority.account,
                authority.signer_key_id,
                relation,
            );
            if public_actions::verify_submitted_state(&package, &fresh, 0).is_ok() {
                return render_public_action_result(
                    PublicActionResult {
                        schema: "tohseno.public-action-result/1",
                        action,
                        app_name: app_name.into(),
                        status: "already_relayed_and_verified",
                        artifact_path: Some(artifact.display().to_string()),
                        block_number: fresh.block_number,
                        signed: true,
                        submitted: false,
                        verified: true,
                        builder_transaction,
                        transaction: None,
                    },
                    json,
                    bus,
                );
            }
            let fresh_plan = plan_public_action(
                &plan,
                &record,
                &authority,
                &fresh,
                request.clone(),
                args.deadline,
                now_unix()?,
            )?;
            let fresh_planned =
                match fresh_plan {
                    PlanOutcome::Action(value) => value,
                    _ => return Err(
                        "public state changed after signing; the prepared action was not relayed"
                            .into(),
                    ),
                };
            if !same_action_intent(&package, &fresh_planned) {
                return Err(
                    "nonce, controller, head, sequence, target, or calldata intent changed after signing; nothing was relayed"
                        .into(),
                );
            }
            let receipt = public_submission::submit_signed_action(submission, &package)?;
            let post = public_network::read_public_preparation(
                &transport,
                &plan,
                &record,
                authority.account,
                authority.signer_key_id,
                relation,
            );
            public_actions::verify_submitted_state(&package, &post, receipt.block_number).map_err(
                |error| {
                    format!(
                        "transaction {} mined, but exact post-state verification failed: {error}",
                        receipt.transaction_hash
                    )
                },
            )?;
            render_public_action_result(
                PublicActionResult {
                    schema: "tohseno.public-action-result/1",
                    action,
                    app_name: app_name.into(),
                    status: "submitted_and_verified",
                    artifact_path: Some(artifact.display().to_string()),
                    block_number: post.block_number,
                    signed: true,
                    submitted: true,
                    verified: true,
                    builder_transaction,
                    transaction: Some(receipt),
                },
                json,
                bus,
            )
        }
    }
}

fn with_builder_account_deployment_authorization<T>(
    args: &PublicMutationArgs,
    submit: impl FnOnce() -> Result<T, public_submission::PublicSubmissionError>,
) -> Result<T, Box<dyn std::error::Error>> {
    if args.confirm_builder_account_deployment.as_deref()
        != Some(EXACT_BUILDER_ACCOUNT_DEPLOYMENT_CONFIRMATION)
    {
        return Err(format!(
            "BuilderAccount is missing; refusing irreversible deployment before submission. Repeat with --confirm-builder-account-deployment \"{EXACT_BUILDER_ACCOUNT_DEPLOYMENT_CONFIRMATION}\""
        )
        .into());
    }
    Ok(submit()?)
}

fn submission_config(
    args: &PublicMutationArgs,
) -> Result<Option<SubmissionConfig>, Box<dyn std::error::Error>> {
    if !args.submit {
        return Ok(None);
    }
    let confirmation = args
        .confirm_experimental_mainnet
        .clone()
        .ok_or("--submit requires --confirm-experimental-mainnet")?;
    let wallet = match (&args.foundry_account, args.hardware_wallet) {
        (Some(name), None) => RelayerWallet::FoundryAccount(name.clone()),
        (None, Some(HardwareWallet::Ledger)) => RelayerWallet::Ledger,
        (None, Some(HardwareWallet::Trezor)) => RelayerWallet::Trezor,
        _ => {
            return Err(
                "--submit requires exactly one --foundry-account or --hardware-wallet".into(),
            )
        }
    };
    let config = SubmissionConfig {
        rpc_url: RpcUrl::parse(args.rpc_url.clone())?,
        confirmation,
        wallet,
    };
    config.validate()?;
    Ok(Some(config))
}

fn same_deployment_intent(
    left: &public_actions::BuilderAccountDeploymentRequest,
    right: &public_actions::BuilderAccountDeploymentRequest,
) -> bool {
    left.chain_id == right.chain_id
        && left.target == right.target
        && left.calldata == right.calldata
        && left.expected_account == right.expected_account
        && left.account_salt == right.account_salt
        && left.initial_key_id == right.initial_key_id
        && left.initial_public_key == right.initial_public_key
}

fn same_action_intent(
    package: &SignedPublicActionPackage,
    planned: &public_actions::PlannedPublicAction,
) -> bool {
    package.signed_action.domain == planned.domain
        && package.signed_action.action == planned.action
        && package.signed_action.signer == planned.signer
        && package.signed_action.authorization.digest == planned.digest
        && package.relay_argument == planned.relay_argument
        && package.expected_state == planned.expected_state
        && package.record_binding == planned.record_binding
}

fn render_public_action_result(
    result: PublicActionResult,
    json: bool,
    bus: &EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    if json {
        print_json(&result)?;
    } else {
        bus.emit(Event::result(format!(
            "{} · {}.",
            result.action, result.status
        )));
        if let Some(path) = &result.artifact_path {
            bus.emit(Event::status(format!("closed artifact: {path}")));
        }
        bus.emit(Event::status(format!(
            "signed={} · submitted={} · verified={}",
            result.signed, result.submitted, result.verified
        )));
    }
    Ok(())
}

fn mutation_rpc(
    args: &PublicMutationArgs,
) -> Result<(CurlTransport, DeploymentPlan), Box<dyn std::error::Error>> {
    let url = RpcUrl::parse(args.rpc_url.clone())?;
    let transport = CurlTransport::discover(url)?;
    let plan = public_network::embedded_deployment_plan()?;
    Ok((transport, plan))
}

fn now_unix() -> Result<u64, Box<dyn std::error::Error>> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}

fn verified_local_head_with_signature(
    app_name: &str,
) -> Result<(Evolution, ShotRecord, SignatureSidecar), Box<dyn std::error::Error>> {
    let (shot, record) = verified_local_head(app_name)?;
    let signature: SignatureSidecar = tohseno_protocol::canonical::from_slice(&fs::read(
        shot.path.join("TOHSENO/signature.json"),
    )?)?;
    record.verify_signature(&signature)?;
    Ok((shot, record, signature))
}

#[derive(Serialize)]
struct PublicActionResult {
    schema: &'static str,
    action: &'static str,
    app_name: String,
    status: &'static str,
    artifact_path: Option<String>,
    block_number: Option<u64>,
    signed: bool,
    submitted: bool,
    verified: bool,
    builder_transaction: Option<SubmittedTransaction>,
    transaction: Option<SubmittedTransaction>,
}

fn verified_local_head(
    app_name: &str,
) -> Result<(Evolution, ShotRecord), Box<dyn std::error::Error>> {
    tohseno_engine::ledger::validate_app_name(app_name)?;
    let fascia_reference = protocol_lifecycle::reference_fascia_root()?;
    let local = resolve_verification(app_name, &fascia_reference)?;
    if !local.conformant() {
        return Err("local Shot lineage is not conformant".into());
    }
    let ledger = Ledger::discover()?;
    let shot = ledger
        .latest_evolution(app_name)?
        .ok_or("app has no complete Shot")?;
    let record: ShotRecord =
        tohseno_protocol::canonical::from_slice(&fs::read(shot.path.join("TOHSENO/shot.json"))?)?;
    record.validate()?;
    Ok((shot, record))
}

fn record_for_target(target: &str) -> Result<ShotRecord, Box<dyn std::error::Error>> {
    let candidate = PathBuf::from(target);
    let shot = if fs::symlink_metadata(&candidate).is_ok() {
        resolve_shot(target)?
    } else {
        let ledger = Ledger::discover()?;
        ledger
            .latest_evolution(target)?
            .ok_or("app has no complete Shot")?
    };
    let record: ShotRecord =
        tohseno_protocol::canonical::from_slice(&fs::read(shot.path.join("TOHSENO/shot.json"))?)?;
    record.validate()?;
    Ok(record)
}

fn read_only_rpc() -> Result<(CurlTransport, DeploymentPlan), Box<dyn std::error::Error>> {
    let raw_url = match std::env::var("ROBINHOOD_RPC_URL") {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => DEFAULT_ROBINHOOD_RPC_URL.into(),
        Err(error) => return Err(format!("ROBINHOOD_RPC_URL is invalid: {error}").into()),
    };
    let url = RpcUrl::parse(raw_url)?;
    let transport = CurlTransport::discover(url)?;
    let plan = public_network::embedded_deployment_plan()?;
    Ok((transport, plan))
}

#[cfg(test)]
fn candidate_deployed(deployment: &serde_json::Value) -> bool {
    deployment
        .get("contracts")
        .and_then(serde_json::Value::as_object)
        .is_some_and(|contracts| {
            !contracts.is_empty()
                && contracts.values().all(|contract| {
                    contract
                        .get("deployed")
                        .and_then(serde_json::Value::as_bool)
                        == Some(true)
                        && contract
                            .get("transaction_hash")
                            .and_then(serde_json::Value::as_str)
                            .is_some()
                        && contract
                            .get("runtime_code_hash")
                            .and_then(serde_json::Value::as_str)
                            .is_some()
                })
        })
}

#[cfg(test)]
fn validate_handle(handle: &str) -> Result<(), Box<dyn std::error::Error>> {
    let valid = !handle.is_empty()
        && handle.len() <= 63
        && handle.is_ascii()
        && !handle.starts_with('-')
        && !handle.ends_with('-')
        && handle
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
    if !valid {
        return Err(
            "handle must be 1–63 lowercase ASCII letters, digits, or interior hyphens".into(),
        );
    }
    Ok(())
}

#[derive(serde::Serialize)]
struct UnfinishedAttempt {
    schema: &'static str,
    app_name: String,
    attempt_directory: String,
    stages: Vec<UnfinishedStage>,
}

#[derive(serde::Serialize)]
struct UnfinishedStage {
    id: &'static str,
    status: &'static str,
}

/// Speaks honestly about an app whose newest attempt never finalized, instead
/// of refusing with "no complete Shot". Returns true when it reported one.
fn report_unfinished_app(
    target: &str,
    json: bool,
    bus: &EventBus,
) -> Result<bool, Box<dyn std::error::Error>> {
    if fs::symlink_metadata(target).is_ok()
        || tohseno_engine::ledger::validate_app_name(target).is_err()
    {
        return Ok(false);
    }
    let ledger = Ledger::discover()?;
    if ledger.load_app(target).is_err() || !ledger.list_evolutions(target)?.is_empty() {
        return Ok(false);
    }
    let app_dir = ledger.root().join(target).join(".tohseno");
    let mut attempts: Vec<PathBuf> = Vec::new();
    for parent in [app_dir.join("evolutions"), app_dir.join("incomplete")] {
        let Ok(entries) = fs::read_dir(parent) else {
            continue;
        };
        for entry in entries.flatten() {
            if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                attempts.push(entry.path());
            }
        }
    }
    attempts.sort();
    let Some(attempt) = attempts.last() else {
        return Ok(false);
    };
    let has_project = fs::read_dir(attempt.join("src"))
        .map(|entries| {
            entries.flatten().any(|entry| {
                entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "xcodeproj")
            })
        })
        .unwrap_or(false);
    let has_artifact = fs::read_dir(attempt.join("artifact"))
        .map(|entries| {
            entries.flatten().any(|entry| {
                entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "app")
            })
        })
        .unwrap_or(false);
    let stage = |done: bool| if done { "pass" } else { "pending" };
    let stages = vec![
        UnfinishedStage {
            id: "intention.recorded",
            status: stage(attempt.join("prompt.md").is_file()),
        },
        UnfinishedStage {
            id: "world.generated",
            status: stage(has_project),
        },
        UnfinishedStage {
            id: "world.memory",
            status: stage(attempt.join("src/MEMORY.md").is_file()),
        },
        UnfinishedStage {
            id: "artifact.materialized",
            status: stage(has_artifact),
        },
        UnfinishedStage {
            id: "record.prepared",
            status: stage(attempt.join("TOHSENO/shot.json").is_file()),
        },
        UnfinishedStage {
            id: "record.signed",
            status: stage(attempt.join("TOHSENO/signature.json").is_file()),
        },
        UnfinishedStage {
            id: "conformance.receipt",
            status: stage(attempt.join("TOHSENO/conformance.json").is_file()),
        },
        UnfinishedStage {
            id: "evolution.finalized",
            status: stage(attempt.join(".complete").is_file()),
        },
    ];
    let view = UnfinishedAttempt {
        schema: "tohseno.cli-unfinished-attempt/1",
        app_name: target.into(),
        attempt_directory: attempt.display().to_string(),
        stages,
    };
    if json {
        print_json(&view)?;
    } else {
        bus.emit(Event::status(format!(
            "{target} · newest unfinished attempt"
        )));
        for stage in &view.stages {
            let marker = if stage.status == "pass" { "✓" } else { "–" };
            bus.emit(Event::status(format!("{marker} {}", stage.id)));
        }
        bus.emit(Event::status(format!(
            "next: `tohseno create {target}` — this attempt archives automatically."
        )));
    }
    Ok(true)
}

fn resolve_verification(
    target: &str,
    fascia_reference: &Path,
) -> Result<LocalVerification, Box<dyn std::error::Error>> {
    let shot_body = resolve_shot_body(target)?;
    let candidate = PathBuf::from(target);
    if fs::symlink_metadata(&candidate).is_ok() {
        return match resolve_shot(target) {
            Ok(shot) => {
                let expression = verifier::verify_shot_directory(&shot.path, fascia_reference);
                Ok(match shot_body {
                    Some(shot_body) => LocalVerification::EvolutionAndShotBody {
                        expression,
                        shot_body,
                    },
                    None => LocalVerification::Evolution(expression),
                })
            }
            Err(error) => shot_body.map(LocalVerification::ShotBody).ok_or(error),
        };
    }

    tohseno_engine::ledger::validate_app_name(target)?;
    let ledger = Ledger::discover()?;
    let shots = ledger.list_evolutions(target)?;
    if shots.is_empty() {
        return shot_body
            .map(LocalVerification::ShotBody)
            .ok_or_else(|| "app has no complete Shot".into());
    }
    let protocol_start = shots.iter().position(|shot| {
        fs::symlink_metadata(shot.path.join("TOHSENO"))
            .map(|metadata| metadata.is_dir() || metadata.file_type().is_symlink())
            .unwrap_or(false)
    });
    let roots = match protocol_start {
        Some(index) => shots[index..]
            .iter()
            .map(|shot| shot.path.clone())
            .collect::<Vec<_>>(),
        None => vec![shots.last().ok_or("app has no complete Shot")?.path.clone()],
    };
    let expression = verifier::verify_lineage_directories(&roots, fascia_reference);
    Ok(match shot_body {
        Some(shot_body) => LocalVerification::LineageAndShotBody {
            expression,
            shot_body,
        },
        None => LocalVerification::Lineage(expression),
    })
}

fn resolve_shot_body(
    target: &str,
) -> Result<Option<ShotBodyVerification>, Box<dyn std::error::Error>> {
    let candidate = PathBuf::from(target);
    if let Ok(metadata) = fs::symlink_metadata(&candidate) {
        if metadata.file_type().is_symlink() {
            return Err("Shot target must not be a symbolic link".into());
        }
        let start = if metadata.is_file() {
            candidate
                .parent()
                .ok_or("Shot target has no parent directory")?
                .to_path_buf()
        } else {
            candidate
        };
        let root = std::iter::successors(Some(start.as_path()), |path| path.parent())
            .take(5)
            .find(|path| {
                path.join(".tohseno/lineage.jsonl").is_file()
                    || path.join(".tohseno/legacy-v1.json").is_file()
            });
        return root
            .map(|root| ShotLayout::at(root).verify_shot_body(None))
            .transpose()
            .map_err(Into::into);
    }

    if tohseno_engine::ledger::validate_app_name(target).is_err() {
        return Ok(None);
    }
    let ledger = Ledger::discover()?;
    let app = match ledger.load_app(target) {
        Ok(app) => app,
        Err(tohseno_engine::LedgerError::AppMissing(_)) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let root = ledger.working_tree(target);
    if !root.join(".tohseno/lineage.jsonl").is_file()
        && !root.join(".tohseno/legacy-v1.json").is_file()
    {
        return Ok(None);
    }
    Ok(Some(
        ShotLayout::at(root).verify_shot_body(app.expression_id)?,
    ))
}

fn render_verification(report: &VerificationOutput, bus: &EventBus) {
    bus.emit(Event::status("TOHSENO SHOT"));
    for check in report.local.checks() {
        let marker = match check.status {
            VerificationStatus::Pass => "✓",
            VerificationStatus::Fail => "×",
            VerificationStatus::NotChecked => "–",
        };
        let detail = if check.status == VerificationStatus::Fail {
            format!(" — {}", check.observed)
        } else {
            String::new()
        };
        bus.emit(Event::status(format!("{marker} {}{detail}", check.id)));
    }
    if let Some(body) = report.local.shot_body() {
        bus.emit(Event::status(format!(
            "✓ lineage.v{} · {} signed action(s)",
            body.protocol_version, body.lineage_sequence
        )));
        bus.emit(Event::status(format!(
            "{} intention.exact-bytes",
            if body.intention_bytes_verified {
                "✓"
            } else {
                "–"
            }
        )));
        bus.emit(Event::status(format!(
            "{} genome.accepted",
            if body.genome_revision.is_some() {
                "✓"
            } else {
                "–"
            }
        )));
        bus.emit(Event::status(format!(
            "{} expression.embedded-identity",
            if body.selected_version_id.is_none() || body.embedded_metadata_verified {
                "✓"
            } else {
                "×"
            }
        )));
    }
    if let Some(public) = &report.public {
        render_public_checks(&public.network.checks, bus);
        render_public_checks(&public.checks, bus);
    }
    if report.conformant {
        bus.emit(Event::result("CONFORMANT · locally verified."));
    } else {
        bus.emit(Event::status("NONCONFORMANT"));
    }
}

fn render_public_checks(checks: &[public_network::PublicCheck], bus: &EventBus) {
    for check in checks {
        let marker = match check.status {
            PublicCheckStatus::Pass => "✓",
            PublicCheckStatus::Fail => "×",
            PublicCheckStatus::NotChecked => "–",
        };
        let detail = if check.status == PublicCheckStatus::Pass {
            String::new()
        } else {
            format!(" — {}", check.observed)
        };
        bus.emit(Event::status(format!("{marker} {}{detail}", check.id)));
    }
}

fn resolve_shot(target: &str) -> Result<Evolution, Box<dyn std::error::Error>> {
    let path = PathBuf::from(target);
    if let Ok(metadata) = fs::symlink_metadata(&path) {
        if metadata.file_type().is_symlink() {
            return Err("Shot target must not be a symbolic link".into());
        }
        let path = if metadata.is_file() {
            path.parent()
                .ok_or("record path has no parent")?
                .to_path_buf()
        } else {
            path
        };
        let shot_path = if path.ends_with("TOHSENO") {
            path.parent()
                .ok_or("TOHSENO path has no parent")?
                .to_path_buf()
        } else {
            path
        };
        if !shot_path.join("TOHSENO/shot.json").is_file() {
            return Err("path does not identify a completed Shot directory".into());
        }
        let number = shot_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("shot directory name is invalid")?
            .parse::<u32>()?;
        let app_name = shot_path
            .parent()
            .and_then(Path::parent)
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .ok_or("shot path does not have ledger anatomy")?
            .to_owned();
        return Ok(Evolution {
            app_name,
            number,
            path: shot_path,
        });
    }
    tohseno_engine::ledger::validate_app_name(target)?;
    let ledger = Ledger::discover()?;
    ledger
        .latest_evolution(target)?
        .ok_or_else(|| "app has no complete Shot".into())
}

fn read_record_pair(
    path: &Path,
) -> Result<(ShotRecord, SignatureSidecar), Box<dyn std::error::Error>> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("record path must be a regular non-symlinked file".into());
    }
    let record: ShotRecord = tohseno_protocol::canonical::from_slice(&fs::read(path)?)?;
    record.validate()?;
    let signature_path = path
        .parent()
        .ok_or("record has no parent directory")?
        .join("signature.json");
    let signature_metadata = fs::symlink_metadata(&signature_path)?;
    if signature_metadata.file_type().is_symlink() || !signature_metadata.is_file() {
        return Err("signature path must be a regular non-symlinked file".into());
    }
    let signature: SignatureSidecar =
        tohseno_protocol::canonical::from_slice(&fs::read(signature_path)?)?;
    Ok((record, signature))
}

fn print_json(value: &impl Serialize) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

#[derive(Serialize)]
struct ProtocolInfo {
    protocol: &'static str,
    candidate_version: &'static str,
    /// Frozen v1 field retained for existing machine consumers.
    shot_schema: &'static str,
    compatibility_shot_schema: &'static str,
    lineage_protocol_version: &'static str,
    lineage_action_schema: &'static str,
    lineage_schema_version: u32,
    signature_schema: &'static str,
    fascia: &'static str,
    app_metadata_schemas: [&'static str; 2],
    /// Frozen v1 field retained for existing machine consumers.
    chain_id: u64,
    registry_chain_id: u64,
    supported_token_association_chains: [u64; 2],
    canonical_release: bool,
    deployment: serde_json::Value,
}

#[derive(Serialize)]
struct RecordVerification {
    conformant: bool,
    record_path: String,
    shot_id: String,
    builder_id: String,
    sequence: u32,
    commitment: String,
    signer_key_id: String,
}

#[derive(Serialize)]
#[serde(tag = "scope", content = "report", rename_all = "snake_case")]
enum LocalVerification {
    Evolution(ShotVerificationReport),
    Lineage(LineageVerificationReport),
    ShotBody(ShotBodyVerification),
    EvolutionAndShotBody {
        expression: ShotVerificationReport,
        shot_body: ShotBodyVerification,
    },
    LineageAndShotBody {
        expression: LineageVerificationReport,
        shot_body: ShotBodyVerification,
    },
}

impl LocalVerification {
    fn conformant(&self) -> bool {
        match self {
            Self::Evolution(report) => report.conformant,
            Self::Lineage(report) => report.conformant,
            Self::ShotBody(_) => true,
            Self::EvolutionAndShotBody { expression, .. } => expression.conformant,
            Self::LineageAndShotBody { expression, .. } => expression.conformant,
        }
    }

    fn checks(&self) -> Vec<&VerificationCheck> {
        match self {
            Self::Evolution(report) => report.checks.iter().collect(),
            Self::Lineage(report) => report
                .shots
                .iter()
                .flat_map(|shot| shot.checks.iter())
                .chain(std::iter::once(&report.lineage))
                .collect(),
            Self::ShotBody(_) => Vec::new(),
            Self::EvolutionAndShotBody { expression, .. } => expression.checks.iter().collect(),
            Self::LineageAndShotBody { expression, .. } => expression
                .shots
                .iter()
                .flat_map(|shot| shot.checks.iter())
                .chain(std::iter::once(&expression.lineage))
                .collect(),
        }
    }

    fn shot_body(&self) -> Option<&ShotBodyVerification> {
        match self {
            Self::ShotBody(report) => Some(report),
            Self::EvolutionAndShotBody { shot_body, .. }
            | Self::LineageAndShotBody { shot_body, .. } => Some(shot_body),
            Self::Evolution(_) | Self::Lineage(_) => None,
        }
    }
}

#[derive(Serialize)]
struct VerificationOutput {
    schema: &'static str,
    conformant: bool,
    local: LocalVerification,
    #[serde(skip_serializing_if = "Option::is_none")]
    public: Option<PublicShotVerificationReport>,
}

#[derive(Serialize)]
struct RegistryView {
    schema: &'static str,
    app_name: String,
    shot_id: String,
    local_head: String,
    local_sequence: u32,
    local_state: &'static str,
    registry_address: String,
    registry_deployed: bool,
    public_checked: bool,
    public_controller: Option<String>,
    public_head: Option<String>,
    public_sequence: Option<u64>,
    transaction_hash: Option<String>,
    evidence_path: String,
    verification: PublicShotVerificationReport,
}

#[derive(Serialize)]
struct Inspection {
    app_name: String,
    shot_directory: String,
    shot_id: String,
    builder_id: String,
    sequence: u32,
    previous: Option<String>,
    commitment: String,
    source_tree_sha256: String,
    fascia_sha256: String,
    signer_key_id: String,
    conformant: bool,
    public_state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    shot_body: Option<ShotBodyVerification>,
}

#[derive(Serialize)]
struct ShotBodyInspection {
    schema: &'static str,
    target: String,
    local_state: &'static str,
    ownership_acquired: bool,
    source_materialized: bool,
    report: ShotBodyVerification,
}

impl ShotBodyInspection {
    fn from_report(target: &str, report: ShotBodyVerification) -> Self {
        Self {
            schema: "tohseno.cli-shot-body-inspection/1",
            target: target.into(),
            local_state: "verified_records",
            ownership_acquired: false,
            source_materialized: report.embedded_metadata_verified,
            report,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn submitting_args(builder_deployment_confirmation: Option<&str>) -> PublicMutationArgs {
        PublicMutationArgs {
            rpc_url: DEFAULT_ROBINHOOD_RPC_URL.into(),
            deadline: 2_000_000_000,
            submit: true,
            confirm_experimental_mainnet: Some(
                public_submission::EXACT_MAINNET_CONFIRMATION.into(),
            ),
            confirm_builder_account_deployment: builder_deployment_confirmation.map(str::to_owned),
            foundry_account: Some("genesis-relayer".into()),
            hardware_wallet: None,
        }
    }

    #[test]
    fn embedded_candidate_is_honestly_undeployed() {
        let deployment: serde_json::Value = serde_json::from_str(DEPLOYMENT).unwrap();
        assert!(!candidate_deployed(&deployment));
        let plan = public_network::embedded_deployment_plan().unwrap();
        assert!(!plan.contracts.shot_registry.deployed);
        assert!(plan.contracts.shot_registry.transaction_hash.is_none());
    }

    #[test]
    fn handles_follow_the_contract_vocabulary() {
        for valid in ["tohseno", "field-notebook", "a1"] {
            validate_handle(valid).unwrap();
        }
        for invalid in ["", "-tohseno", "tohseno-", "TOHSENO", "two words", "é"] {
            assert!(validate_handle(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn public_mutation_holds_the_app_lock_before_local_and_network_reads() {
        let source = include_str!("protocol_commands.rs");
        let start = source.find("fn prepare_public_action(").unwrap();
        let end = source[start..].find("fn submission_config(").unwrap() + start;
        let body = &source[start..end];
        let public_lock = body.find("ledger.lock_public_actions()").unwrap();
        let lock = body.find("ledger.lock_app(app_name)").unwrap();
        let local = body
            .find("verified_local_head_with_signature(app_name)")
            .unwrap();
        let network = body.find("read_public_preparation(").unwrap();
        assert!(public_lock < lock && lock < local && local < network);
        assert!(body.contains("let _public_lock ="));
        assert!(body.contains("let _app_lock ="));
    }

    #[test]
    fn builder_account_deployment_confirmation_refuses_before_submission() {
        let near_miss = format!("{EXACT_BUILDER_ACCOUNT_DEPLOYMENT_CONFIRMATION} ");
        for confirmation in [
            None,
            Some(public_submission::EXACT_MAINNET_CONFIRMATION),
            Some(near_miss.as_str()),
        ] {
            let submissions = Cell::new(0);
            let result = with_builder_account_deployment_authorization(
                &submitting_args(confirmation),
                || {
                    submissions.set(submissions.get() + 1);
                    Ok(())
                },
            );
            assert!(result.is_err());
            assert_eq!(submissions.get(), 0);
            let error = result.unwrap_err().to_string();
            assert!(error.contains("refusing irreversible deployment before submission"));
            assert!(error.contains("--confirm-builder-account-deployment"));
            assert!(error.contains(EXACT_BUILDER_ACCOUNT_DEPLOYMENT_CONFIRMATION));
        }
    }

    #[test]
    fn exact_builder_account_deployment_confirmation_allows_submission() {
        let submissions = Cell::new(0);
        with_builder_account_deployment_authorization(
            &submitting_args(Some(EXACT_BUILDER_ACCOUNT_DEPLOYMENT_CONFIRMATION)),
            || {
                submissions.set(submissions.get() + 1);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(submissions.get(), 1);
    }

    #[test]
    fn builder_account_deployment_guard_is_runtime_conditional_and_precedes_submit() {
        let source = include_str!("protocol_commands.rs");
        let start = source.find("fn prepare_public_action(").unwrap();
        let end = source[start..]
            .find("fn with_builder_account_deployment_authorization")
            .unwrap()
            + start;
        let body = &source[start..end];
        let missing_account_branch = body
            .find("if let PlanOutcome::BuilderDeploymentRequired")
            .unwrap();
        let preparation_only_return = body[missing_account_branch..]
            .find("if submission.is_none()")
            .unwrap()
            + missing_account_branch;
        let authorization = body[missing_account_branch..]
            .find("with_builder_account_deployment_authorization(args")
            .unwrap()
            + missing_account_branch;
        let deployment_submit = body[missing_account_branch..]
            .find("public_submission::submit_builder_account_deployment")
            .unwrap()
            + missing_account_branch;

        assert!(!body[..missing_account_branch]
            .contains("with_builder_account_deployment_authorization"));
        assert!(preparation_only_return < authorization);
        assert!(authorization < deployment_submit);
    }
}
