//! Private living-project records and the adopted-project execution pipeline.
//!
//! An adopted Xcode project is deliberately not a protocol Shot. This module
//! owns a versioned, owner-local pointer to source and append-only evolution
//! history without writing Tohseno metadata into the selected repository.

use crate::cable_genesis::{device_digest as companion_install_target_digest, CableGenesisStore};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tohseno_application::snapshot::EvolutionHistorySummary;
use tohseno_application::{
    ExecutionSummary, IconDescriptor, Presentation, PresentedState, ReferenceInput,
    ShotApplicationService, ShotKind, ShotSummary, SupportedCompanionAction,
};
use tohseno_engine::gates::device::{self, Device, DeviceInventoryState};
use tohseno_engine::harness::{build_evolution_command, HarnessSelection};
use tohseno_protocol::digest::sha256 as protocol_sha256;
use uuid::Uuid;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

const STORE_SCHEMA: &str = "tohseno.private-living-project-store/1";
const PROJECT_SCHEMA: &str = "tohseno.private-living-project/1";
const EVOLUTION_SCHEMA: &str = "tohseno.private-project-evolution/1";
const COMMAND_SCHEMA: &str = "tohseno.private-project-command-index/1";
const ADOPTION_SCHEMA: &str = "tohseno.project-adoption-result/1";
const MAX_RECORD_BYTES: u64 = 8 * 1024 * 1024;
const MAX_COMMAND_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_INTENTION_BYTES: usize = 1024 * 1024;
const MAX_EVOLUTIONS_PER_PROJECT: usize = 10_000;
const XCODE_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const HARNESS_TIMEOUT: Duration = Duration::from_secs(60 * 60);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum XcodeContainerKind {
    Project,
    Workspace,
}

impl XcodeContainerKind {
    fn flag(&self) -> &'static str {
        match self {
            Self::Project => "-project",
            Self::Workspace => "-workspace",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryInstruction {
    pub relative_path: String,
    pub sha256: String,
    pub byte_length: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitObservation {
    pub repository_root: String,
    pub revision: Option<String>,
    pub dirty: bool,
    pub dirty_paths: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectHarness {
    pub harness: String,
    pub model: String,
    pub route: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceInstallation {
    pub device_identifier_digest: String,
    pub device_name: String,
    pub os_version: Option<String>,
    #[serde(default)]
    pub short_version: Option<String>,
    pub build_number: Option<String>,
    pub installed_at: String,
    pub verified: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkImportKind {
    Install,
    Fork,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkProjectOrigin {
    pub kind: NetworkImportKind,
    pub parent_shot_id: String,
    pub parent_release_digest: String,
    pub source_artifact_sha256: String,
    pub builder_id: String,
    pub verified_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkDeliveryState {
    pub release_digest: String,
    pub status: String,
    pub local_bundle_identifier: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provisioning_expires_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<String>,
    pub updated_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectBuildState {
    pub status: String,
    pub checked_at: Option<String>,
    pub configuration: Option<String>,
    pub sdk: Option<String>,
    pub artifact_path: Option<String>,
    pub failure_category: Option<String>,
    pub summary: Option<String>,
}

impl Default for ProjectBuildState {
    fn default() -> Self {
        Self {
            status: "not_checked".into(),
            checked_at: None,
            configuration: None,
            sdk: None,
            artifact_path: None,
            failure_category: None,
            summary: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LivingProjectRecord {
    pub schema: String,
    pub revision: u64,
    pub project_id: String,
    pub display_name: String,
    pub source_path: String,
    pub container_path: String,
    pub container_kind: XcodeContainerKind,
    pub scheme: String,
    pub bundle_identifier: String,
    pub deployment_target: Option<String>,
    pub signing_team: Option<String>,
    pub product_name: String,
    pub wrapper_name: String,
    pub harness: ProjectHarness,
    pub original_intention: Option<String>,
    pub instructions: Vec<RepositoryInstruction>,
    pub git: Option<GitObservation>,
    pub current_source_state: String,
    /// Random public identity reserved exactly once when this project joins
    /// the person-to-person path. It is not derived from its name or Git.
    #[serde(default)]
    pub candidate_shot_id: Option<String>,
    #[serde(default)]
    pub latest_publication: Option<ProjectPublication>,
    #[serde(default)]
    pub network_origin: Option<NetworkProjectOrigin>,
    #[serde(default)]
    pub network_delivery: Option<NetworkDeliveryState>,
    pub build: ProjectBuildState,
    #[serde(default)]
    pub associated_companion_device_ids: Vec<String>,
    pub installations: Vec<DeviceInstallation>,
    pub latest_evolution_id: Option<String>,
    pub latest_evolution_status: Option<EvolutionStatus>,
    pub last_successful_connection: Option<String>,
    pub last_successful_installation: Option<String>,
    pub recovery: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectPublication {
    pub release_digest: String,
    pub public_checkpoint_digest: String,
    pub checkpoint_sequence: u64,
    pub status: String,
    pub public_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_slug: Option<String>,
    pub transaction_hash: Option<String>,
    pub updated_at: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvolutionStatus {
    Queued,
    Received,
    Working,
    Building,
    WaitingForUserAction,
    ReadyToInstall,
    Installing,
    Installed,
    Completed,
    Failed,
}

impl EvolutionStatus {
    fn execution_state(self) -> &'static str {
        match self {
            Self::Queued | Self::Received => "queued",
            Self::Working => "materializing",
            Self::Building => "building",
            Self::WaitingForUserAction | Self::ReadyToInstall => "waiting_for_device",
            Self::Installing => "installing",
            Self::Installed => "launching",
            Self::Completed => "accepted",
            Self::Failed => "failed",
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Received => "received",
            Self::Working => "working",
            Self::Building => "building",
            Self::WaitingForUserAction => "waiting_for_user_action",
            Self::ReadyToInstall => "ready_to_install",
            Self::Installing => "installing",
            Self::Installed => "installed",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

fn transition_allowed(from: EvolutionStatus, to: EvolutionStatus) -> bool {
    if from == to {
        return true;
    }
    match from {
        EvolutionStatus::Queued => {
            matches!(to, EvolutionStatus::Received | EvolutionStatus::Failed)
        }
        EvolutionStatus::Received => {
            matches!(to, EvolutionStatus::Working | EvolutionStatus::Failed)
        }
        EvolutionStatus::Working => {
            matches!(to, EvolutionStatus::Building | EvolutionStatus::Failed)
        }
        EvolutionStatus::Building => matches!(
            to,
            EvolutionStatus::WaitingForUserAction
                | EvolutionStatus::ReadyToInstall
                | EvolutionStatus::Installing
                | EvolutionStatus::Failed
        ),
        EvolutionStatus::WaitingForUserAction | EvolutionStatus::ReadyToInstall => matches!(
            to,
            EvolutionStatus::WaitingForUserAction
                | EvolutionStatus::ReadyToInstall
                | EvolutionStatus::Installing
                | EvolutionStatus::Failed
        ),
        EvolutionStatus::Installing => matches!(
            to,
            EvolutionStatus::WaitingForUserAction
                | EvolutionStatus::ReadyToInstall
                | EvolutionStatus::Installed
                | EvolutionStatus::Failed
        ),
        EvolutionStatus::Installed => {
            matches!(to, EvolutionStatus::Completed | EvolutionStatus::Failed)
        }
        EvolutionStatus::Completed | EvolutionStatus::Failed => false,
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvolutionEvent {
    pub sequence: u64,
    pub status: EvolutionStatus,
    pub at: String,
    pub summary: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildAttempt {
    pub started_at: String,
    pub completed_at: String,
    pub configuration: String,
    pub sdk: String,
    pub success: bool,
    pub artifact_path: Option<String>,
    pub category: Option<String>,
    pub summary: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestAttempt {
    pub started_at: String,
    pub completed_at: String,
    pub destination: Option<String>,
    pub success: bool,
    pub skipped: bool,
    pub category: Option<String>,
    pub summary: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstallationAttempt {
    pub attempted_at: String,
    pub device_identifier_digest: Option<String>,
    pub device_name: Option<String>,
    pub success: bool,
    pub verified: bool,
    pub category: Option<String>,
    pub summary: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectEvolutionRecord {
    pub schema: String,
    pub revision: u64,
    pub evolution_id: String,
    pub command_id: String,
    pub command_digest: String,
    pub project_id: String,
    pub originating_device_id: Option<String>,
    pub user_request: String,
    pub attachment_names: Vec<String>,
    pub received_at: String,
    pub starting_source_state: String,
    pub starting_git_revision: Option<String>,
    pub preexisting_dirty_paths: Vec<String>,
    pub harness: ProjectHarness,
    pub status: EvolutionStatus,
    pub events: Vec<EvolutionEvent>,
    pub observed_changed_files: Vec<String>,
    #[serde(default)]
    pub test_attempts: Vec<TestAttempt>,
    pub build_attempts: Vec<BuildAttempt>,
    pub installation_attempts: Vec<InstallationAttempt>,
    pub completion_summary: Option<String>,
    pub resulting_source_state: Option<String>,
    pub resulting_git_revision: Option<String>,
    pub failure_category: Option<String>,
    pub recovery_action: Option<String>,
    pub follow_up_to: Option<String>,
    pub rollback_available: bool,
    pub updated_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoreManifest {
    schema: String,
    revision: u64,
    created_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandIndex {
    schema: String,
    command_id: String,
    command_digest: String,
    project_id: String,
    evolution_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdoptionRequest {
    pub path: String,
    #[serde(default)]
    pub scheme: Option<String>,
    #[serde(default)]
    pub harness: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub network_origin: Option<NetworkProjectOrigin>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdoptionResult {
    pub schema: String,
    pub status: String,
    pub scheme_candidates: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<LivingProjectRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ProjectEvolutionRequest {
    pub command_id: String,
    pub project_id: String,
    pub base_source_state: String,
    pub intention: String,
    pub originating_device_id: Option<String>,
    pub references: Vec<ReferenceInput>,
    pub follow_up_to: Option<String>,
}

#[derive(Clone)]
pub struct LivingProjectService {
    root: PathBuf,
    application: ShotApplicationService,
    companion_install_target: CableGenesisStore,
    publication: Arc<Mutex<()>>,
    execution_lock: Arc<tokio::sync::Mutex<()>>,
}

impl LivingProjectService {
    pub fn open(
        service_root: &Path,
        application: ShotApplicationService,
        companion_install_target: CableGenesisStore,
    ) -> Result<Self, BoxError> {
        let root = service_root.join("living-projects-v1");
        ensure_private_directory(&root)?;
        for child in ["projects", "commands"] {
            ensure_private_directory(&root.join(child))?;
        }
        let manifest_path = root.join("store.json");
        match fs::symlink_metadata(&manifest_path) {
            Ok(_) => {
                let manifest: StoreManifest = read_json(&manifest_path)?;
                if manifest.schema != STORE_SCHEMA || manifest.revision != 1 {
                    return Err(
                        "unsupported living-project store version; migration is required".into(),
                    );
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                write_new(
                    &manifest_path,
                    &StoreManifest {
                        schema: STORE_SCHEMA.into(),
                        revision: 1,
                        created_at: now(),
                    },
                )?;
            }
            Err(error) => return Err(error.into()),
        }
        Ok(Self {
            root,
            application,
            companion_install_target,
            publication: Arc::new(Mutex::new(())),
            execution_lock: Arc::new(tokio::sync::Mutex::new(())),
        })
    }

    pub fn list_projects(&self) -> Result<Vec<LivingProjectRecord>, BoxError> {
        let mut values = Vec::new();
        for entry in fs::read_dir(self.root.join("projects"))?.take(10_000) {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let record: LivingProjectRecord = read_json(&path)?;
            validate_project(&record)?;
            values.push(record);
        }
        values.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.project_id.cmp(&right.project_id))
        });
        Ok(values)
    }

    pub fn project(&self, project_id: &str) -> Result<Option<LivingProjectRecord>, BoxError> {
        validate_id("project ID", project_id)?;
        let path = self.project_path(project_id)?;
        match fs::symlink_metadata(&path) {
            Ok(_) => {
                let value: LivingProjectRecord = read_json(&path)?;
                validate_project(&value)?;
                Ok(Some(value))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub fn record_publication(
        &self,
        project_id: &str,
        publication: ProjectPublication,
    ) -> Result<LivingProjectRecord, BoxError> {
        if publication.release_digest.len() != 66
            || publication.public_checkpoint_digest.len() != 66
            || publication.checkpoint_sequence == 0
            || publication.status != "published"
            || publication.public_url.as_deref().is_none_or(|value| {
                !value.starts_with("https://tohseno.com/") || value.contains(['?', '#'])
            })
        {
            return Err("public publication receipt is invalid".into());
        }
        let mut project = self.project(project_id)?.ok_or("unknown living project")?;
        if let Some(existing) = &project.latest_publication {
            if publication.checkpoint_sequence < existing.checkpoint_sequence {
                return Err("public publication receipt moved backwards".into());
            }
            if publication.checkpoint_sequence == existing.checkpoint_sequence
                && publication != *existing
            {
                return Err("public publication receipt conflicts at one checkpoint".into());
            }
        }
        project.latest_publication = Some(publication);
        project.revision = project
            .revision
            .checked_add(1)
            .ok_or("project revision overflowed")?;
        project.updated_at = now();
        self.replace_project(&project)?;
        Ok(project)
    }

    /// Build, locally sign, and install one already verified network import.
    /// No Builder credential or original Apple identity participates here.
    pub async fn install_network_project(
        &self,
        project_id: &str,
        mac_review_approved: bool,
    ) -> Result<LivingProjectRecord, BoxError> {
        let _execution = self.execution_lock.lock().await;
        let mut project = self.project(project_id)?.ok_or("unknown living project")?;
        let origin = project
            .network_origin
            .clone()
            .ok_or("this project is not a verified network import")?;
        let safety = tohseno_network::build_profile::classify_xcode_project(
            Path::new(&project.source_path),
            Path::new(&project.container_path),
        )?;
        match safety.classification {
            tohseno_network::catalog::BuildSafetyClassification::Unsupported => {
                return Err(format!(
                    "This app cannot be locally re-signed: {}",
                    safety.reasons.join("; ")
                )
                .into())
            }
            tohseno_network::catalog::BuildSafetyClassification::RequiresMacReview
                if !mac_review_approved =>
            {
                if let Some(delivery) = &mut project.network_delivery {
                    delivery.status = "requires_mac_review".into();
                    delivery.failure = Some(safety.reasons.join("; "));
                    delivery.updated_at = now();
                }
                project.revision += 1;
                project.updated_at = now();
                self.replace_project(&project)?;
                return Ok(project);
            }
            _ => {}
        }
        let team = tohseno_engine::gates::sign::development_team_profile()?;
        let delivery_root = self.project_data_directory(project_id)?.join(format!(
            "network-release-{}",
            origin.parent_release_digest.trim_start_matches("0x")
        ));
        ensure_private_directory(&delivery_root)?;
        let derived = delivery_root.join("derived-data");
        ensure_private_directory(&derived)?;
        let local_bundle = local_network_bundle_identifier(&origin.parent_shot_id);
        let mut arguments = xcode_build_arguments(
            &project,
            &derived,
            "Debug",
            "iphoneos",
            "generic/platform=iOS",
            true,
        );
        let build = arguments
            .pop()
            .ok_or("Xcode build arguments are incomplete")?;
        arguments.extend([
            OsString::from("-disableAutomaticPackageResolution"),
            OsString::from("-onlyUsePackageVersionsFromResolvedFile"),
            OsString::from("ENABLE_USER_SCRIPT_SANDBOXING=YES"),
            OsString::from("CODE_SIGN_STYLE=Automatic"),
            OsString::from(format!("DEVELOPMENT_TEAM={}", team.team_id)),
            OsString::from(format!("PRODUCT_BUNDLE_IDENTIFIER={local_bundle}")),
        ]);
        arguments.push(build);
        if let Some(delivery) = &mut project.network_delivery {
            delivery.status = "building".into();
            delivery.failure = None;
            delivery.updated_at = now();
        }
        project.revision += 1;
        project.updated_at = now();
        self.replace_project(&project)?;
        let log = delivery_root.join("xcodebuild.log");
        let status = run_logged_async(
            "xcodebuild",
            &arguments,
            Path::new(&project.source_path),
            &log,
            XCODE_TIMEOUT,
        )
        .await?;
        let artifact = find_built_app(&derived, &project.wrapper_name);
        if !status.success() || artifact.is_none() {
            let diagnostic = read_log_tail(&log, 8_000).unwrap_or_else(|_| status.to_string());
            let mut failed = self.project(project_id)?.ok_or("unknown living project")?;
            if let Some(delivery) = &mut failed.network_delivery {
                delivery.status = "failed".into();
                delivery.failure = Some(bounded_message(&diagnostic, 1_000));
                delivery.updated_at = now();
            }
            failed.revision += 1;
            failed.updated_at = now();
            self.replace_project(&failed)?;
            return Ok(failed);
        }
        let artifact = artifact.expect("checked above");
        verify_codesign(&artifact)?;
        let mut ready = self.project(project_id)?.ok_or("unknown living project")?;
        ready.build = ProjectBuildState {
            status: "buildable".into(),
            checked_at: Some(now()),
            configuration: Some("Debug".into()),
            sdk: Some("iphoneos".into()),
            artifact_path: Some(artifact.display().to_string()),
            failure_category: None,
            summary: Some("Xcode built and locally signed the verified network release.".into()),
        };
        if let Some(delivery) = &mut ready.network_delivery {
            delivery.status = "ready_for_iphone".into();
            delivery.local_bundle_identifier = local_bundle.clone();
            delivery.artifact_path = Some(artifact.display().to_string());
            delivery.provisioning_expires_at =
                tohseno_engine::gates::sign::provisioning_expiration(&artifact);
            delivery.failure = None;
            delivery.updated_at = now();
        }
        ready.revision += 1;
        ready.updated_at = now();
        self.replace_project(&ready)?;
        self.install_ready_network_project(&ready, &artifact, &local_bundle, team.provisioning)
            .await
    }

    async fn install_ready_network_project(
        &self,
        project: &LivingProjectRecord,
        artifact: &Path,
        local_bundle: &str,
        provisioning: tohseno_engine::gates::sign::ProvisioningKind,
    ) -> Result<LivingProjectRecord, BoxError> {
        let observed = tokio::task::spawn_blocking(device::inventory).await??;
        let device = match observed {
            DeviceInventoryState::Ready(devices) => {
                match self.select_companion_install_target(devices)? {
                    InstallTargetSelection::Ready(device) => device,
                    InstallTargetSelection::TargetUnreachable
                    | InstallTargetSelection::AssociationRequired => return Ok(project.clone()),
                }
            }
            DeviceInventoryState::DeviceUnreachable
            | DeviceInventoryState::TrustRequired
            | DeviceInventoryState::DeveloperModeRequired => return Ok(project.clone()),
        };
        if provisioning == tohseno_engine::gates::sign::ProvisioningKind::Free {
            let installed = tohseno_engine::gates::install::installed_candidate_apps(&device)?;
            if let Some(blocker) =
                tohseno_engine::gates::install::free_team_slot_blocker(&installed, local_bundle)
            {
                return Err(format!(
                    "Your Personal Team device limit is full; remove {} ({}) or use a paid development team",
                    blocker.name.as_deref().unwrap_or("an existing development app"), blocker.bundle_id
                )
                .into());
            }
        }
        let device_for_install = device.clone();
        let artifact = artifact.to_path_buf();
        let artifact_for_install = artifact.clone();
        let bundle = local_bundle.to_owned();
        tokio::task::spawn_blocking(move || {
            tohseno_engine::gates::install::install_owner_app(
                &device_for_install,
                &artifact_for_install,
                &bundle,
            )
        })
        .await??;
        let mut installed = self
            .project(&project.project_id)?
            .ok_or("unknown living project")?;
        let installed_at = now();
        let digest = device_digest(&device.identifier);
        let (short_version, build_number) = app_versions(artifact.as_path());
        installed
            .installations
            .retain(|value| value.device_identifier_digest != digest);
        installed.installations.push(DeviceInstallation {
            device_identifier_digest: digest,
            device_name: device.name,
            os_version: device.os_version,
            short_version,
            build_number,
            installed_at: installed_at.clone(),
            verified: true,
        });
        if let Some(delivery) = &mut installed.network_delivery {
            delivery.status = "installed".into();
            delivery.updated_at = installed_at.clone();
        }
        installed.last_successful_installation = Some(installed_at);
        installed.revision += 1;
        installed.updated_at = now();
        self.replace_project(&installed)?;
        Ok(installed)
    }

    pub fn evolutions(&self, project_id: &str) -> Result<Vec<ProjectEvolutionRecord>, BoxError> {
        let directory = self.evolution_directory(project_id)?;
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };
        let mut values = Vec::new();
        for entry in entries.take(MAX_EVOLUTIONS_PER_PROJECT) {
            let path = entry?.path().join("evolution.json");
            if path.is_file() {
                let record: ProjectEvolutionRecord = read_json(&path)?;
                validate_evolution(&record)?;
                values.push(record);
            }
        }
        values.sort_by(|left, right| left.received_at.cmp(&right.received_at));
        Ok(values)
    }

    pub fn adoption(&self, request: AdoptionRequest) -> Result<AdoptionResult, BoxError> {
        let selected = resolve_container(Path::new(&request.path))?;
        let source_root = selected
            .path
            .parent()
            .ok_or("selected Xcode container has no source root")?
            .to_path_buf();
        let schemes = list_schemes(&selected)?;
        if schemes.is_empty() {
            return Err("Xcode reported no shared or user-visible schemes for this project".into());
        }
        let mut app_settings = Vec::new();
        for scheme in schemes.iter().take(32) {
            if request
                .scheme
                .as_ref()
                .is_some_and(|chosen| chosen != scheme)
            {
                continue;
            }
            if let Ok(settings) = show_app_settings(&selected, &source_root, scheme) {
                app_settings.push((scheme.clone(), settings));
            }
        }
        let chosen = if let Some(scheme) = request.scheme.as_ref() {
            if !schemes.contains(scheme) {
                return Err("the selected scheme is not reported by this Xcode container".into());
            }
            app_settings
                .into_iter()
                .find(|(candidate, _)| candidate == scheme)
                .ok_or("the selected scheme does not contain an iOS application target")?
        } else if app_settings.len() == 1 {
            app_settings.remove(0)
        } else {
            let stem = selected
                .path
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or_default();
            let matching = app_settings
                .iter()
                .filter(|(scheme, _)| scheme.eq_ignore_ascii_case(stem))
                .cloned()
                .collect::<Vec<_>>();
            if matching.len() == 1 {
                matching[0].clone()
            } else {
                let candidates = if app_settings.is_empty() {
                    schemes
                } else {
                    app_settings.into_iter().map(|(scheme, _)| scheme).collect()
                };
                return Ok(AdoptionResult {
                    schema: ADOPTION_SCHEMA.into(),
                    status: "needs_scheme".into(),
                    scheme_candidates: candidates,
                    project: None,
                    message: Some("Choose the app scheme Tohseno should build.".into()),
                });
            }
        };
        let (scheme, settings) = chosen;
        let bundle_identifier = nonempty_setting(&settings, "PRODUCT_BUNDLE_IDENTIFIER")
            .ok_or("Xcode did not resolve an application bundle identifier")?;
        validate_bundle_identifier(&bundle_identifier)?;
        if let Some(mut existing) = self.list_projects()?.into_iter().find(|project| {
            Path::new(&project.container_path) == selected.path
                && project.scheme == scheme
                && project.bundle_identifier == bundle_identifier
        }) {
            if request.network_origin.is_some() && request.network_origin != existing.network_origin
            {
                return Err(
                    "this source root is already bound to a different network release".into(),
                );
            }
            if existing.candidate_shot_id.is_none() {
                existing.candidate_shot_id = Some(
                    tohseno_protocol::digest::ShotId::random()
                        .to_string()
                        .trim_start_matches("0x")
                        .to_owned(),
                );
            }
            existing.git = observe_git(&source_root)?;
            existing.current_source_state = source_state(&source_root, existing.git.as_ref())?;
            existing.instructions = discover_instructions(&source_root, existing.git.as_ref())?;
            existing.revision += 1;
            existing.updated_at = now();
            self.replace_project(&existing)?;
            let existing = self.verify_adoption_build(&existing)?;
            let existing = self.observe_existing_installation(existing)?;
            return Ok(AdoptionResult {
                schema: ADOPTION_SCHEMA.into(),
                status: "adopted".into(),
                scheme_candidates: Vec::new(),
                project: Some(existing),
                message: Some(
                    "This project was already adopted; its stable identity was preserved.".into(),
                ),
            });
        }
        let selection = crate::living_project::selection_for_request(
            &self.application,
            request.harness.as_deref(),
            request.model.as_deref(),
        )?;
        let project_id = format!("project_{}", Uuid::new_v4().simple());
        let created = now();
        let git = observe_git(&source_root)?;
        let current_source_state = source_state(&source_root, git.as_ref())?;
        let instructions = discover_instructions(&source_root, git.as_ref())?;
        let display_name = nonempty_setting(&settings, "PRODUCT_NAME")
            .or_else(|| nonempty_setting(&settings, "TARGET_NAME"))
            .unwrap_or_else(|| scheme.clone());
        let wrapper_name = nonempty_setting(&settings, "WRAPPER_NAME")
            .unwrap_or_else(|| format!("{display_name}.app"));
        let record = LivingProjectRecord {
            schema: PROJECT_SCHEMA.into(),
            revision: 1,
            project_id: project_id.clone(),
            display_name,
            source_path: source_root.display().to_string(),
            container_path: selected.path.display().to_string(),
            container_kind: selected.kind,
            scheme,
            bundle_identifier,
            deployment_target: nonempty_setting(&settings, "IPHONEOS_DEPLOYMENT_TARGET"),
            signing_team: nonempty_setting(&settings, "DEVELOPMENT_TEAM"),
            product_name: nonempty_setting(&settings, "PRODUCT_NAME")
                .unwrap_or_else(|| "App".into()),
            wrapper_name,
            harness: ProjectHarness {
                harness: selection.harness,
                model: selection.model,
                route: selection.route,
            },
            original_intention: None,
            instructions,
            git,
            current_source_state,
            candidate_shot_id: Some(
                tohseno_protocol::digest::ShotId::random()
                    .to_string()
                    .trim_start_matches("0x")
                    .to_owned(),
            ),
            latest_publication: None,
            network_origin: request.network_origin.clone(),
            network_delivery: request
                .network_origin
                .as_ref()
                .map(|origin| NetworkDeliveryState {
                    release_digest: origin.parent_release_digest.clone(),
                    status: "verified_source".into(),
                    local_bundle_identifier: local_network_bundle_identifier(
                        &origin.parent_shot_id,
                    ),
                    artifact_path: None,
                    provisioning_expires_at: None,
                    failure: None,
                    updated_at: created.clone(),
                }),
            build: ProjectBuildState::default(),
            associated_companion_device_ids: Vec::new(),
            installations: Vec::new(),
            latest_evolution_id: None,
            latest_evolution_status: None,
            last_successful_connection: None,
            last_successful_installation: None,
            recovery: None,
            created_at: created.clone(),
            updated_at: created,
        };
        let mut record = record;
        if record
            .network_origin
            .as_ref()
            .is_some_and(|origin| origin.kind == NetworkImportKind::Install)
        {
            record.candidate_shot_id = None;
        }
        validate_project(&record)?;
        let _guard = self
            .publication
            .lock()
            .map_err(|_| "living-project publication lock failed")?;
        write_new(&self.project_path(&project_id)?, &record)?;
        ensure_private_directory(&self.project_data_directory(&project_id)?)?;
        ensure_private_directory(&self.evolution_directory(&project_id)?)?;
        drop(_guard);
        let record = self.verify_adoption_build(&record)?;
        let record = self.observe_existing_installation(record)?;
        Ok(AdoptionResult {
            schema: ADOPTION_SCHEMA.into(),
            status: "adopted".into(),
            scheme_candidates: Vec::new(),
            project: Some(record),
            message: None,
        })
    }

    fn observe_existing_installation(
        &self,
        mut project: LivingProjectRecord,
    ) -> Result<LivingProjectRecord, BoxError> {
        let Ok(DeviceInventoryState::Ready(devices)) = device::inventory() else {
            return Ok(project);
        };
        let InstallTargetSelection::Ready(device) =
            self.select_companion_install_target(devices)?
        else {
            return Ok(project);
        };
        if !matches!(
            tohseno_engine::gates::install::is_bundle_installed(
                &device,
                &project.bundle_identifier,
            ),
            Ok(true)
        ) {
            return Ok(project);
        }
        let observed_at = now();
        let digest = device_digest(&device.identifier);
        project
            .installations
            .retain(|value| value.device_identifier_digest != digest);
        project.installations.push(DeviceInstallation {
            device_identifier_digest: digest,
            device_name: device.name,
            os_version: device.os_version,
            short_version: None,
            build_number: None,
            installed_at: observed_at.clone(),
            verified: true,
        });
        project.last_successful_connection = Some(observed_at);
        project.revision += 1;
        project.updated_at = now();
        self.replace_project(&project)?;
        Ok(project)
    }

    fn verify_adoption_build(
        &self,
        project: &LivingProjectRecord,
    ) -> Result<LivingProjectRecord, BoxError> {
        let derived = self
            .project_data_directory(&project.project_id)?
            .join("adoption-derived-data");
        ensure_private_directory(&derived)?;
        let log = self
            .project_data_directory(&project.project_id)?
            .join("adoption-build.log");
        let arguments = xcode_build_arguments(
            project,
            &derived,
            "Debug",
            "iphonesimulator",
            "generic/platform=iOS Simulator",
            false,
        );
        let result = run_logged_blocking(
            "xcodebuild",
            &arguments,
            Path::new(&project.source_path),
            &log,
            XCODE_TIMEOUT,
        );
        let mut updated = project.clone();
        updated.revision += 1;
        updated.updated_at = now();
        updated.build = match result {
            Ok(()) => ProjectBuildState {
                status: "buildable".into(),
                checked_at: Some(now()),
                configuration: Some("Debug".into()),
                sdk: Some("iphonesimulator".into()),
                artifact_path: find_built_app(&derived, &project.wrapper_name)
                    .map(|path| path.display().to_string()),
                failure_category: None,
                summary: Some("Xcode completed a real unsigned Simulator build.".into()),
            },
            Err(error) => {
                let summary = bounded_message(&error.to_string(), 1_000);
                ProjectBuildState {
                    status: "failed".into(),
                    checked_at: Some(now()),
                    configuration: Some("Debug".into()),
                    sdk: Some("iphonesimulator".into()),
                    artifact_path: None,
                    failure_category: Some(classify_xcode_failure(&summary).into()),
                    summary: Some(summary),
                }
            }
        };
        self.replace_project(&updated)?;
        Ok(updated)
    }

    pub fn submit_evolution(
        self: &Arc<Self>,
        request: ProjectEvolutionRequest,
    ) -> Result<ProjectEvolutionRecord, BoxError> {
        validate_id("command ID", &request.command_id)?;
        validate_id("project ID", &request.project_id)?;
        validate_intention(&request.intention)?;
        let project = self
            .project(&request.project_id)?
            .ok_or("unknown adopted project")?;
        let source = Path::new(&project.source_path);
        let observed_git = observe_git(source)?;
        let observed_source_state = source_state(source, observed_git.as_ref())?;
        let digest = command_digest(&request)?;
        let command_path = self.command_path(&request.command_id)?;
        let guard = self
            .publication
            .lock()
            .map_err(|_| "living-project publication lock failed")?;

        // Idempotency wins over freshness: a retried command must resolve to the
        // durable evolution it originally created even after that evolution has
        // changed the project's source state.
        if command_path.exists() {
            let existing: CommandIndex = read_json(&command_path)?;
            if existing.command_digest != digest || existing.project_id != request.project_id {
                return Err("command_id_conflict".into());
            }
            let record = self
                .evolution(&existing.project_id, &existing.evolution_id)?
                .ok_or("indexed project evolution is unavailable")?;
            let should_resume = matches!(
                record.status,
                EvolutionStatus::Queued | EvolutionStatus::Received
            );
            self.repair_project_link_unlocked(&record)?;
            drop(guard);
            if should_resume {
                self.spawn_evolution(record.project_id.clone(), record.evolution_id.clone());
            }
            return Ok(record);
        }

        let evolutions = self.evolutions(&request.project_id)?;
        if let Some(record) = evolutions
            .iter()
            .find(|record| record.command_id == request.command_id)
            .cloned()
        {
            if record.command_digest != digest {
                return Err("command_id_conflict".into());
            }
            write_new(
                &command_path,
                &CommandIndex {
                    schema: COMMAND_SCHEMA.into(),
                    command_id: request.command_id,
                    command_digest: digest,
                    project_id: record.project_id.clone(),
                    evolution_id: record.evolution_id.clone(),
                },
            )?;
            self.repair_project_link_unlocked(&record)?;
            let should_resume = matches!(
                record.status,
                EvolutionStatus::Queued | EvolutionStatus::Received
            );
            drop(guard);
            if should_resume {
                self.spawn_evolution(record.project_id.clone(), record.evolution_id.clone());
            }
            return Ok(record);
        }

        let mut project = self
            .project(&request.project_id)?
            .ok_or("unknown adopted project")?;
        if observed_source_state != project.current_source_state {
            project.git = observed_git;
            project.current_source_state = observed_source_state;
            project.revision += 1;
            project.updated_at = now();
            self.replace_project_unlocked(&project)?;
            return Err("stale_project_source_state".into());
        }
        if request.base_source_state != project.current_source_state {
            return Err("stale_project_source_state".into());
        }
        if evolutions.iter().any(|value| {
            !matches!(
                value.status,
                EvolutionStatus::Completed | EvolutionStatus::Failed
            )
        }) {
            return Err("project_has_active_evolution".into());
        }
        if evolutions.len() >= MAX_EVOLUTIONS_PER_PROJECT {
            return Err("project evolution history reached its local safety limit".into());
        }
        let evolution_id = format!("evolution_{}", Uuid::new_v4().simple());
        let originating_device_id = request.originating_device_id.clone();
        let directory = self.evolution_data_directory(&request.project_id, &evolution_id)?;
        ensure_private_directory(&directory)?;
        let attachment_directory = directory.join("attachments");
        ensure_private_directory(&attachment_directory)?;
        let mut attachment_names = Vec::new();
        for (index, reference) in request.references.iter().enumerate() {
            let extension = if reference.media_type == "image/png" {
                "png"
            } else {
                "jpg"
            };
            let name = format!("reference-{:02}.{extension}", index + 1);
            write_private_bytes(&attachment_directory.join(&name), &reference.bytes)?;
            attachment_names.push(name);
        }
        let git = observe_git(Path::new(&project.source_path))?;
        let received = now();
        let record = ProjectEvolutionRecord {
            schema: EVOLUTION_SCHEMA.into(),
            revision: 1,
            evolution_id: evolution_id.clone(),
            command_id: request.command_id.clone(),
            command_digest: digest.clone(),
            project_id: request.project_id.clone(),
            originating_device_id: request.originating_device_id,
            user_request: request.intention,
            attachment_names,
            received_at: received.clone(),
            starting_source_state: project.current_source_state.clone(),
            starting_git_revision: git.as_ref().and_then(|value| value.revision.clone()),
            preexisting_dirty_paths: git
                .as_ref()
                .map(|value| value.dirty_paths.clone())
                .unwrap_or_default(),
            harness: project.harness.clone(),
            status: EvolutionStatus::Queued,
            events: vec![EvolutionEvent {
                sequence: 1,
                status: EvolutionStatus::Queued,
                at: received.clone(),
                summary: "The authenticated request was persisted and queued on this Mac.".into(),
            }],
            observed_changed_files: Vec::new(),
            test_attempts: Vec::new(),
            build_attempts: Vec::new(),
            installation_attempts: Vec::new(),
            completion_summary: None,
            resulting_source_state: None,
            resulting_git_revision: None,
            failure_category: None,
            recovery_action: None,
            follow_up_to: request.follow_up_to,
            rollback_available: false,
            updated_at: received,
        };
        validate_evolution(&record)?;
        let index = CommandIndex {
            schema: COMMAND_SCHEMA.into(),
            command_id: request.command_id,
            command_digest: digest,
            project_id: request.project_id.clone(),
            evolution_id: evolution_id.clone(),
        };
        write_new(&directory.join("evolution.json"), &record)?;
        write_new(&command_path, &index)?;
        if let Some(device_id) = originating_device_id {
            validate_id("originating Companion device ID", &device_id)?;
            if !project.associated_companion_device_ids.contains(&device_id) {
                project.associated_companion_device_ids.push(device_id);
                project.associated_companion_device_ids.sort();
            }
            project.last_successful_connection = Some(now());
        }
        project.revision += 1;
        project.latest_evolution_id = Some(evolution_id.clone());
        project.latest_evolution_status = Some(EvolutionStatus::Queued);
        project.updated_at = now();
        self.replace_project_unlocked(&project)?;
        drop(guard);
        self.spawn_evolution(request.project_id, evolution_id);
        Ok(record)
    }

    fn spawn_evolution(self: &Arc<Self>, project_id: String, evolution_id: String) {
        let service = self.clone();
        tokio::spawn(async move {
            if let Err(error) = service
                .clone()
                .run_evolution(&project_id, &evolution_id)
                .await
            {
                let _ = service.fail_evolution(
                    &project_id,
                    &evolution_id,
                    "execution_failed",
                    &bounded_message(&error.to_string(), 1_000),
                    Some(
                        "Review the saved failure, fix the reported local requirement, then retry.",
                    ),
                );
            }
        });
    }

    pub fn evolution(
        &self,
        project_id: &str,
        evolution_id: &str,
    ) -> Result<Option<ProjectEvolutionRecord>, BoxError> {
        let path = self
            .evolution_data_directory(project_id, evolution_id)?
            .join("evolution.json");
        match fs::symlink_metadata(&path) {
            Ok(_) => {
                let value: ProjectEvolutionRecord = read_json(&path)?;
                validate_evolution(&value)?;
                Ok(Some(value))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    async fn run_evolution(
        self: Arc<Self>,
        project_id: &str,
        evolution_id: &str,
    ) -> Result<(), BoxError> {
        let _execution = self.execution_lock.lock().await;
        let current = self
            .evolution(project_id, evolution_id)?
            .ok_or("project evolution disappeared")?;
        if current.status == EvolutionStatus::Queued {
            self.transition(
                project_id,
                evolution_id,
                EvolutionStatus::Received,
                "The queued request is ready for the configured coding harness.",
            )?;
        } else if current.status != EvolutionStatus::Received {
            return Err("project evolution is not resumable from its current state".into());
        }
        match tokio::time::timeout(
            HARNESS_TIMEOUT,
            self.clone()
                .run_evolution_within_budget(project_id, evolution_id),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => {
                Err("project evolution exceeded its shared one-hour wall-clock budget".into())
            }
        }
    }

    async fn run_evolution_within_budget(
        self: Arc<Self>,
        project_id: &str,
        evolution_id: &str,
    ) -> Result<(), BoxError> {
        let project = self
            .project(project_id)?
            .ok_or("adopted project disappeared")?;
        let source = PathBuf::from(&project.source_path);
        let metadata = fs::symlink_metadata(&source).map_err(|error| {
            format!(
                "source folder is unavailable at {}: {error}",
                source.display()
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err("adopted source path is no longer a real directory".into());
        }
        self.transition(
            project_id,
            evolution_id,
            EvolutionStatus::Working,
            "The configured coding harness is evolving the selected project.",
        )?;
        let mut evolution = self
            .evolution(project_id, evolution_id)?
            .ok_or("project evolution disappeared")?;
        let directory = self.evolution_data_directory(project_id, evolution_id)?;
        let packet = directory.join("EXECUTION.md");
        let prior_context = self
            .evolutions(project_id)?
            .into_iter()
            .rev()
            .filter(|record| record.evolution_id != evolution.evolution_id)
            .take(3)
            .collect::<Vec<_>>();
        let packet_body = execution_packet(&project, &evolution, &prior_context);
        write_private_bytes(&packet, packet_body.as_bytes())?;
        let starting_non_git_inventory = if project.git.is_none() {
            Some(source_file_inventory(&source)?)
        } else {
            None
        };
        let image_paths = evolution
            .attachment_names
            .iter()
            .map(|name| directory.join("attachments").join(name))
            .collect::<Vec<_>>();
        let selection = HarnessSelection {
            harness: project.harness.harness.clone(),
            model: project.harness.model.clone(),
            route: project.harness.route.clone(),
            adapter: None,
        };
        let harness = build_evolution_command(&selection, &packet, &image_paths)
            .map_err(|error| format!("coding harness is unavailable: {error}"))?;
        let harness_log = directory.join("harness.log");
        let status = run_harness(harness, &source, &harness_log).await?;
        if !status.success() {
            return Err(
                format!("coding harness exited without completing the request ({status})").into(),
            );
        }
        let after_git = observe_git(&source)?;
        let observed = if let Some(git) = after_git.as_ref() {
            git.dirty_paths.clone()
        } else if let Some(before) = starting_non_git_inventory.as_ref() {
            changed_inventory_paths(before, &source_file_inventory(&source)?)
        } else {
            Vec::new()
        };
        evolution = self
            .evolution(project_id, evolution_id)?
            .ok_or("project evolution disappeared")?;
        evolution.observed_changed_files = observed;
        evolution.resulting_git_revision =
            after_git.as_ref().and_then(|value| value.revision.clone());
        self.replace_evolution(&evolution)?;

        self.transition(
            project_id,
            evolution_id,
            EvolutionStatus::Building,
            "Xcode is building and signing the evolved app.",
        )?;
        let test_attempt = run_project_tests(&project, &directory).await;
        let test_passed = test_attempt.success || test_attempt.skipped;
        let test_summary = test_attempt.summary.clone();
        let test_category = test_attempt.category.clone();
        let mut evolution = self
            .evolution(project_id, evolution_id)?
            .ok_or("project evolution disappeared")?;
        evolution.test_attempts.push(test_attempt);
        self.replace_evolution(&evolution)?;
        if !test_passed {
            self.fail_evolution(
                project_id,
                evolution_id,
                test_category.as_deref().unwrap_or("test_failure"),
                &test_summary,
                Some(
                    "Review the saved Xcode test log and repair the failing test before retrying.",
                ),
            )?;
            return Ok(());
        }
        self.transition(
            project_id,
            evolution_id,
            EvolutionStatus::Building,
            &test_summary,
        )?;
        let attempt_started = now();
        let derived = directory.join("derived-data");
        ensure_private_directory(&derived)?;
        let build_log = directory.join("xcodebuild.log");
        let arguments = xcode_build_arguments(
            &project,
            &derived,
            "Debug",
            "iphoneos",
            "generic/platform=iOS",
            true,
        );
        let build_result =
            run_logged_async("xcodebuild", &arguments, &source, &build_log, XCODE_TIMEOUT).await;
        let artifact = find_built_app(&derived, &project.wrapper_name);
        let mut evolution = self
            .evolution(project_id, evolution_id)?
            .ok_or("project evolution disappeared")?;
        match build_result {
            Ok(status) if status.success() && artifact.is_some() => {
                let artifact = artifact.expect("checked above");
                if let Err(error) = verify_codesign(&artifact) {
                    let diagnostic = bounded_message(&error.to_string(), 1_000);
                    evolution.build_attempts.push(BuildAttempt {
                        started_at: attempt_started,
                        completed_at: now(),
                        configuration: "Debug".into(),
                        sdk: "iphoneos".into(),
                        success: false,
                        artifact_path: None,
                        category: Some("signing_error".into()),
                        summary: diagnostic.clone(),
                    });
                    self.replace_evolution(&evolution)?;
                    self.fail_evolution(
                        project_id,
                        evolution_id,
                        "signing_error",
                        &diagnostic,
                        Some(recovery_for_category("signing_error")),
                    )?;
                    return Ok(());
                }
                evolution.build_attempts.push(BuildAttempt {
                    started_at: attempt_started,
                    completed_at: now(),
                    configuration: "Debug".into(),
                    sdk: "iphoneos".into(),
                    success: true,
                    artifact_path: Some(artifact.display().to_string()),
                    category: None,
                    summary: "Xcode built and codesign verified the device app.".into(),
                });
                let resulting_git = observe_git(&source)?;
                evolution.resulting_git_revision = resulting_git
                    .as_ref()
                    .and_then(|value| value.revision.clone());
                evolution.resulting_source_state =
                    Some(source_state(&source, resulting_git.as_ref())?);
                self.replace_evolution(&evolution)?;
                self.update_project_after_build(project_id, &evolution, &artifact)?;
                self.install_or_wait(project_id, evolution_id, &artifact)
                    .await
            }
            Ok(status) => {
                let diagnostic =
                    read_log_tail(&build_log, 8_000).unwrap_or_else(|_| status.to_string());
                let category = classify_xcode_failure(&diagnostic).to_string();
                evolution.build_attempts.push(BuildAttempt {
                    started_at: attempt_started,
                    completed_at: now(),
                    configuration: "Debug".into(),
                    sdk: "iphoneos".into(),
                    success: false,
                    artifact_path: None,
                    category: Some(category.clone()),
                    summary: bounded_message(&diagnostic, 1_000),
                });
                self.replace_evolution(&evolution)?;
                self.fail_evolution(
                    project_id,
                    evolution_id,
                    &category,
                    "Xcode did not produce a verified device app.",
                    Some(recovery_for_category(&category)),
                )?;
                Ok(())
            }
            Err(error) => {
                let diagnostic = error.to_string();
                let category = classify_xcode_failure(&diagnostic).to_string();
                evolution.build_attempts.push(BuildAttempt {
                    started_at: attempt_started,
                    completed_at: now(),
                    configuration: "Debug".into(),
                    sdk: "iphoneos".into(),
                    success: false,
                    artifact_path: None,
                    category: Some(category.clone()),
                    summary: bounded_message(&diagnostic, 1_000),
                });
                self.replace_evolution(&evolution)?;
                self.fail_evolution(
                    project_id,
                    evolution_id,
                    &category,
                    &diagnostic,
                    Some(recovery_for_category(&category)),
                )?;
                Ok(())
            }
        }
    }

    async fn install_or_wait(
        &self,
        project_id: &str,
        evolution_id: &str,
        artifact: &Path,
    ) -> Result<(), BoxError> {
        let observed = tokio::task::spawn_blocking(device::inventory).await??;
        let device = match observed {
            DeviceInventoryState::Ready(devices) => {
                match self.select_companion_install_target(devices)? {
                    InstallTargetSelection::Ready(device) => device,
                    InstallTargetSelection::TargetUnreachable => {
                        return self.ready_to_install(
                            project_id,
                            evolution_id,
                            "The iPhone that received Tohseno Companion is not reachable. Bring that phone within reach and unlock it; another visible iPhone will never be substituted.",
                        )
                    }
                    InstallTargetSelection::AssociationRequired => {
                        return self.ready_to_install(
                            project_id,
                            evolution_id,
                            "More than one iPhone is reachable and this older setup has no intended-phone association. Disconnect the other iPhones, then Tohseno will continue with the saved build.",
                        )
                    }
                }
            }
            DeviceInventoryState::DeviceUnreachable => {
                return self.ready_to_install(
                    project_id,
                    evolution_id,
                    "Bring the paired iPhone within reach, unlock it, and keep it on the same network as this Mac. Use USB only if Xcode has not paired it for Wi-Fi delivery.",
                )
            }
            DeviceInventoryState::TrustRequired => {
                return self.waiting_for_user(
                    project_id,
                    evolution_id,
                    "Unlock the iPhone, tap Trust This Computer, and enter its passcode.",
                )
            }
            DeviceInventoryState::DeveloperModeRequired => {
                return self.waiting_for_user(
                    project_id,
                    evolution_id,
                    "On iPhone open Settings → Privacy & Security → Developer Mode, enable it, and reconnect after restart.",
                )
            }
        };
        self.transition(
            project_id,
            evolution_id,
            EvolutionStatus::Installing,
            "Installing the verified build on the paired iPhone.",
        )?;
        let project = self
            .project(project_id)?
            .ok_or("adopted project disappeared")?;
        let artifact = artifact.to_path_buf();
        let bundle = project.bundle_identifier.clone();
        let installation = tokio::task::spawn_blocking(move || {
            tohseno_engine::gates::install::install_owner_app(&device, &artifact, &bundle)
                .map(|_| device)
        })
        .await?;
        match installation {
            Ok(device) => self.complete_installation(project_id, evolution_id, &device),
            Err(error) => {
                let summary = error.to_string();
                let category = classify_install_failure(&summary);
                let mut evolution = self
                    .evolution(project_id, evolution_id)?
                    .ok_or("project evolution disappeared")?;
                evolution.installation_attempts.push(InstallationAttempt {
                    attempted_at: now(),
                    device_identifier_digest: None,
                    device_name: None,
                    success: false,
                    verified: false,
                    category: Some(category.into()),
                    summary: bounded_message(&summary, 1_000),
                });
                self.replace_evolution(&evolution)?;
                if matches!(category, "device_locked" | "device_unavailable") {
                    self.ready_to_install(
                        project_id,
                        evolution_id,
                        "Unlock and reconnect the paired iPhone. Tohseno kept the verified build and will retry.",
                    )
                } else {
                    self.fail_evolution(
                        project_id,
                        evolution_id,
                        category,
                        &summary,
                        Some(recovery_for_category(category)),
                    )
                }
            }
        }
    }

    /// Resolve the physical destination independently of its current USB or
    /// local-network transport. New setup records bind the CoreDevice that
    /// received Companion; older records retain the exactly-one-device
    /// compatibility fallback from ADR 0036.
    fn select_companion_install_target(
        &self,
        devices: Vec<Device>,
    ) -> Result<InstallTargetSelection, BoxError> {
        let intended = self.companion_install_target.load()?.intended_device_digest;
        Ok(select_install_target(devices, intended.as_deref()))
    }

    fn complete_installation(
        &self,
        project_id: &str,
        evolution_id: &str,
        device: &Device,
    ) -> Result<(), BoxError> {
        let digest = device_digest(&device.identifier);
        let mut evolution = self
            .evolution(project_id, evolution_id)?
            .ok_or("project evolution disappeared")?;
        let (short_version, build_number) = evolution
            .build_attempts
            .iter()
            .rev()
            .find_map(|attempt| attempt.artifact_path.as_deref())
            .map(Path::new)
            .map(app_versions)
            .unwrap_or((None, None));
        evolution.installation_attempts.push(InstallationAttempt {
            attempted_at: now(),
            device_identifier_digest: Some(digest.clone()),
            device_name: Some(device.name.clone()),
            success: true,
            verified: true,
            category: None,
            summary: "devicectl installed the app and its exact bundle identifier appeared in the device inventory.".into(),
        });
        self.replace_evolution(&evolution)?;
        self.transition(
            project_id,
            evolution_id,
            EvolutionStatus::Installed,
            "The updated app is installed on the iPhone.",
        )?;
        let mut project = self
            .project(project_id)?
            .ok_or("adopted project disappeared")?;
        let installed_at = now();
        project
            .installations
            .retain(|value| value.device_identifier_digest != digest);
        project.installations.push(DeviceInstallation {
            device_identifier_digest: digest,
            device_name: device.name.clone(),
            os_version: device.os_version.clone(),
            short_version,
            build_number,
            installed_at: installed_at.clone(),
            verified: true,
        });
        project.last_successful_installation = Some(installed_at);
        project.revision += 1;
        project.updated_at = now();
        self.replace_project(&project)?;
        let mut evolution = self
            .evolution(project_id, evolution_id)?
            .ok_or("project evolution disappeared")?;
        evolution.completion_summary = Some(format!(
            "Applied the requested source change, built it with Xcode, and verified installation on {}.",
            device.name
        ));
        self.replace_evolution(&evolution)?;
        self.transition(
            project_id,
            evolution_id,
            EvolutionStatus::Completed,
            "The evolution completed and the updated app is ready to use.",
        )
    }

    fn ready_to_install(
        &self,
        project_id: &str,
        evolution_id: &str,
        action: &str,
    ) -> Result<(), BoxError> {
        self.transition(
            project_id,
            evolution_id,
            EvolutionStatus::ReadyToInstall,
            "The build is verified and waiting for the iPhone.",
        )?;
        let mut value = self
            .evolution(project_id, evolution_id)?
            .ok_or("project evolution disappeared")?;
        value.recovery_action = Some(action.into());
        self.replace_evolution(&value)
    }

    fn waiting_for_user(
        &self,
        project_id: &str,
        evolution_id: &str,
        action: &str,
    ) -> Result<(), BoxError> {
        self.transition(
            project_id,
            evolution_id,
            EvolutionStatus::WaitingForUserAction,
            action,
        )?;
        let mut value = self
            .evolution(project_id, evolution_id)?
            .ok_or("project evolution disappeared")?;
        value.recovery_action = Some(action.into());
        self.replace_evolution(&value)
    }

    pub async fn retry_ready_installations_once(&self) -> Result<usize, BoxError> {
        let mut candidates = Vec::new();
        for project in self.list_projects()? {
            let Some(evolution_id) = project.latest_evolution_id.as_deref() else {
                continue;
            };
            let Some(evolution) = self.evolution(&project.project_id, evolution_id)? else {
                continue;
            };
            if !matches!(
                evolution.status,
                EvolutionStatus::ReadyToInstall | EvolutionStatus::WaitingForUserAction
            ) {
                continue;
            }
            let artifact = evolution
                .build_attempts
                .iter()
                .rev()
                .find(|attempt| attempt.success)
                .and_then(|attempt| attempt.artifact_path.as_deref())
                .map(PathBuf::from);
            let Some(artifact) = artifact.filter(|path| path.is_dir()) else {
                continue;
            };
            candidates.push((project.project_id, evolution_id.to_owned(), artifact));
        }
        if candidates.is_empty() {
            return Ok(0);
        }
        let mut resumed = 0;
        for (project_id, evolution_id, artifact) in candidates {
            self.install_or_wait(&project_id, &evolution_id, &artifact)
                .await?;
            resumed += 1;
        }
        Ok(resumed)
    }

    pub async fn retry_ready_network_installations_once(&self) -> Result<usize, BoxError> {
        let provisioning = match tohseno_engine::gates::sign::development_team_profile() {
            Ok(team) => team.provisioning,
            Err(_) => return Ok(0),
        };
        let mut ready = Vec::new();
        for project in self.list_projects()? {
            let Some(delivery) = &project.network_delivery else {
                continue;
            };
            if delivery.status != "ready_for_iphone" {
                continue;
            }
            let Some(artifact) = delivery.artifact_path.as_deref().map(PathBuf::from) else {
                continue;
            };
            if artifact.is_dir() {
                let bundle = delivery.local_bundle_identifier.clone();
                ready.push((project, artifact, bundle));
            }
        }
        let mut resumed = 0;
        for (project, artifact, bundle) in ready {
            let updated = self
                .install_ready_network_project(&project, &artifact, &bundle, provisioning)
                .await?;
            if updated
                .network_delivery
                .as_ref()
                .is_some_and(|delivery| delivery.status == "installed")
            {
                resumed += 1;
            }
        }
        Ok(resumed)
    }

    pub fn recover_interrupted(self: &Arc<Self>) -> Result<usize, BoxError> {
        let mut recovered = 0;
        let mut queued = Vec::new();
        for project in self.list_projects()? {
            let Some(evolution_id) = project.latest_evolution_id.as_deref() else {
                continue;
            };
            let Some(mut evolution) = self.evolution(&project.project_id, evolution_id)? else {
                continue;
            };
            if matches!(
                evolution.status,
                EvolutionStatus::Working | EvolutionStatus::Building | EvolutionStatus::Installing
            ) {
                evolution.status = if evolution
                    .build_attempts
                    .iter()
                    .any(|attempt| attempt.success && attempt.artifact_path.is_some())
                {
                    EvolutionStatus::ReadyToInstall
                } else {
                    EvolutionStatus::Failed
                };
                evolution.failure_category = (evolution.status == EvolutionStatus::Failed)
                    .then(|| "interrupted_execution".into());
                evolution.recovery_action =
                    Some(if evolution.status == EvolutionStatus::ReadyToInstall {
                        "Reconnect and unlock the paired iPhone; the verified build was preserved."
                            .into()
                    } else {
                        "Retry this request. Tohseno preserved the source and interruption record."
                            .into()
                    });
                let recovered_status = evolution.status;
                push_event(
                    &mut evolution,
                    recovered_status,
                    "The Mac restarted during this evolution; durable state was recovered.",
                );
                self.replace_evolution(&evolution)?;
                let mut project = self
                    .project(&project.project_id)?
                    .ok_or("adopted project disappeared")?;
                project.latest_evolution_status = Some(recovered_status);
                project.revision += 1;
                project.updated_at = now();
                self.replace_project(&project)?;
                recovered += 1;
            } else if matches!(
                evolution.status,
                EvolutionStatus::Queued | EvolutionStatus::Received
            ) {
                queued.push((project.project_id.clone(), evolution_id.to_owned()));
            }
        }
        for (project_id, evolution_id) in queued {
            let service = self.clone();
            tokio::spawn(async move {
                if let Err(error) = service
                    .clone()
                    .run_evolution(&project_id, &evolution_id)
                    .await
                {
                    let _ = service.fail_evolution(
                        &project_id,
                        &evolution_id,
                        "execution_failed",
                        &bounded_message(&error.to_string(), 1_000),
                        Some("Review the saved failure and retry the request."),
                    );
                }
            });
            recovered += 1;
        }
        Ok(recovered)
    }

    pub fn summaries(&self) -> Result<Vec<ShotSummary>, BoxError> {
        let icon_bytes = include_bytes!("../../brand/logos/tohseno-app-icon-1024.png").to_vec();
        let icon_digest = protocol_sha256(&icon_bytes).to_string();
        let projects = self.list_projects()?;
        let mut summaries = Vec::with_capacity(projects.len());
        for (index, project) in projects.into_iter().enumerate() {
            let recent_evolutions = self
                .evolutions(&project.project_id)?
                .into_iter()
                .rev()
                .take(20)
                .map(|evolution| EvolutionHistorySummary {
                    evolution_id: evolution.evolution_id,
                    requested_at: evolution.received_at,
                    request_summary: bounded_message(&evolution.user_request, 2_048),
                    status: evolution.status.as_str().into(),
                    completion_summary: evolution
                        .completion_summary
                        .map(|value| bounded_message(&value, 2_048)),
                    installation_summary: evolution
                        .installation_attempts
                        .last()
                        .map(|attempt| bounded_message(&attempt.summary, 2_048))
                        .or(evolution.recovery_action),
                })
                .collect::<Vec<_>>();
            let execution = match project.latest_evolution_id.as_deref() {
                Some(id) => {
                    self.evolution(&project.project_id, id)?
                        .map(|evolution| ExecutionSummary {
                            execution_id: evolution.evolution_id,
                            shot_id: project.project_id.clone(),
                            state: evolution.status.execution_state().into(),
                            version_ordinal: evolution.revision.max(1),
                            started_at: evolution.received_at,
                            elapsed_seconds: 0,
                            updated_at: evolution.updated_at,
                            state_transition: None,
                        })
                }
                None => None,
            };
            let presentation = project_presentation(&project, execution.as_ref());
            summaries.push(ShotSummary {
                shot_id: project.project_id,
                display_name: project.display_name,
                bundle_identifier: Some(project.bundle_identifier),
                kind: ShotKind::AdoptedProject,
                source_state: Some(project.current_source_state),
                icon: IconDescriptor {
                    revision: icon_digest.clone(),
                    blob_id: icon_digest.clone(),
                    media_type: "image/png".into(),
                    byte_length: u64::try_from(icon_bytes.len()).unwrap_or(u64::MAX),
                    placeholder: true,
                    private_bytes: icon_bytes.clone(),
                },
                expression_id: None,
                latest_version_id: None,
                latest_version_ordinal: None,
                latest_version_created_at: None,
                execution,
                recent_evolutions,
                presentation,
                archived: false,
                retired: false,
                sort_index: u64::try_from(index).unwrap_or(u64::MAX),
                supported_companion_actions: vec![
                    SupportedCompanionAction::Read,
                    SupportedCompanionAction::Evolve,
                ],
            });
        }
        Ok(summaries)
    }

    pub fn merge_workspace(
        &self,
        snapshot: &mut tohseno_application::WorkspaceSnapshot,
    ) -> Result<(), BoxError> {
        let mut summaries = self.summaries()?;
        let offset = snapshot.shots.len() as u64;
        for (index, summary) in summaries.iter_mut().enumerate() {
            summary.sort_index = offset + u64::try_from(index).unwrap_or(u64::MAX);
            if let Some(execution) = summary
                .execution
                .clone()
                .filter(|value| !matches!(value.state.as_str(), "accepted" | "failed"))
            {
                snapshot.active_executions.push(execution);
            }
        }
        snapshot.shots.extend(summaries);
        Ok(())
    }

    fn transition(
        &self,
        project_id: &str,
        evolution_id: &str,
        status: EvolutionStatus,
        summary: &str,
    ) -> Result<(), BoxError> {
        let mut evolution = self
            .evolution(project_id, evolution_id)?
            .ok_or("project evolution disappeared")?;
        if !transition_allowed(evolution.status, status) {
            return Err(format!(
                "invalid project evolution transition: {:?} -> {:?}",
                evolution.status, status
            )
            .into());
        }
        evolution.status = status;
        evolution.revision += 1;
        evolution.updated_at = now();
        push_event(&mut evolution, status, summary);
        self.replace_evolution(&evolution)?;
        let mut project = self
            .project(project_id)?
            .ok_or("adopted project disappeared")?;
        project.latest_evolution_id = Some(evolution_id.into());
        project.latest_evolution_status = Some(status);
        project.revision += 1;
        project.updated_at = now();
        self.replace_project(&project)
    }

    fn fail_evolution(
        &self,
        project_id: &str,
        evolution_id: &str,
        category: &str,
        summary: &str,
        recovery: Option<&str>,
    ) -> Result<(), BoxError> {
        let mut evolution = self
            .evolution(project_id, evolution_id)?
            .ok_or("project evolution disappeared")?;
        if evolution.status == EvolutionStatus::Completed {
            return Ok(());
        }
        evolution.status = EvolutionStatus::Failed;
        evolution.failure_category = Some(category.into());
        evolution.recovery_action = recovery.map(str::to_owned);
        evolution.completion_summary = Some(bounded_message(summary, 1_000));
        evolution.revision += 1;
        evolution.updated_at = now();
        push_event(
            &mut evolution,
            EvolutionStatus::Failed,
            "The evolution stopped without claiming a build or installation succeeded.",
        );
        self.replace_evolution(&evolution)?;
        let mut project = self
            .project(project_id)?
            .ok_or("adopted project disappeared")?;
        project.latest_evolution_status = Some(EvolutionStatus::Failed);
        project.revision += 1;
        project.updated_at = now();
        self.replace_project(&project)
    }

    fn update_project_after_build(
        &self,
        project_id: &str,
        evolution: &ProjectEvolutionRecord,
        artifact: &Path,
    ) -> Result<(), BoxError> {
        let mut project = self
            .project(project_id)?
            .ok_or("adopted project disappeared")?;
        if let Some(state) = evolution.resulting_source_state.clone() {
            project.current_source_state = state;
        }
        project.git = observe_git(Path::new(&project.source_path))?;
        project.build = ProjectBuildState {
            status: "ready_to_install".into(),
            checked_at: Some(now()),
            configuration: Some("Debug".into()),
            sdk: Some("iphoneos".into()),
            artifact_path: Some(artifact.display().to_string()),
            failure_category: None,
            summary: Some("A signed device build passed local codesign verification.".into()),
        };
        project.revision += 1;
        project.updated_at = now();
        self.replace_project(&project)
    }

    fn replace_project(&self, project: &LivingProjectRecord) -> Result<(), BoxError> {
        let _guard = self
            .publication
            .lock()
            .map_err(|_| "living-project publication lock failed")?;
        self.replace_project_unlocked(project)
    }

    fn replace_project_unlocked(&self, project: &LivingProjectRecord) -> Result<(), BoxError> {
        validate_project(project)?;
        write_replace(&self.project_path(&project.project_id)?, project)
    }

    fn repair_project_link_unlocked(
        &self,
        evolution: &ProjectEvolutionRecord,
    ) -> Result<(), BoxError> {
        let mut project = self
            .project(&evolution.project_id)?
            .ok_or("adopted project disappeared")?;
        let mut changed = false;
        match project.latest_evolution_id.as_deref() {
            None => {
                project.latest_evolution_id = Some(evolution.evolution_id.clone());
                project.latest_evolution_status = Some(evolution.status);
                changed = true;
            }
            Some(current) if current == evolution.evolution_id => {
                if project.latest_evolution_status != Some(evolution.status) {
                    project.latest_evolution_status = Some(evolution.status);
                    changed = true;
                }
            }
            Some(_) => {}
        }
        if let Some(device_id) = evolution.originating_device_id.as_ref() {
            if !project.associated_companion_device_ids.contains(device_id) {
                project
                    .associated_companion_device_ids
                    .push(device_id.clone());
                project.associated_companion_device_ids.sort();
                project.last_successful_connection = Some(now());
                changed = true;
            }
        }
        if changed {
            project.revision += 1;
            project.updated_at = now();
            self.replace_project_unlocked(&project)?;
        }
        Ok(())
    }

    fn replace_evolution(&self, evolution: &ProjectEvolutionRecord) -> Result<(), BoxError> {
        validate_evolution(evolution)?;
        let _guard = self
            .publication
            .lock()
            .map_err(|_| "living-project publication lock failed")?;
        write_replace(
            &self
                .evolution_data_directory(&evolution.project_id, &evolution.evolution_id)?
                .join("evolution.json"),
            evolution,
        )
    }

    fn project_path(&self, project_id: &str) -> Result<PathBuf, BoxError> {
        validate_id("project ID", project_id)?;
        Ok(self
            .root
            .join("projects")
            .join(format!("{project_id}.json")))
    }

    fn project_data_directory(&self, project_id: &str) -> Result<PathBuf, BoxError> {
        validate_id("project ID", project_id)?;
        Ok(self.root.join("projects").join(project_id))
    }

    fn evolution_directory(&self, project_id: &str) -> Result<PathBuf, BoxError> {
        Ok(self.project_data_directory(project_id)?.join("evolutions"))
    }

    fn evolution_data_directory(
        &self,
        project_id: &str,
        evolution_id: &str,
    ) -> Result<PathBuf, BoxError> {
        validate_id("evolution ID", evolution_id)?;
        Ok(self.evolution_directory(project_id)?.join(evolution_id))
    }

    fn command_path(&self, command_id: &str) -> Result<PathBuf, BoxError> {
        validate_id("command ID", command_id)?;
        Ok(self
            .root
            .join("commands")
            .join(format!("{command_id}.json")))
    }
}

fn selection_for_request(
    application: &ShotApplicationService,
    harness: Option<&str>,
    model: Option<&str>,
) -> Result<HarnessSelection, BoxError> {
    if let Some(harness) = harness {
        return application
            .harness_selection(harness, model.unwrap_or("default"))
            .map_err(Into::into);
    }
    let defaults = application.factory_defaults();
    let harness = defaults
        .harness_id
        .ok_or("connect an authenticated coding harness before adopting a project")?;
    let model = model
        .map(str::to_owned)
        .or(defaults.model_id)
        .unwrap_or_else(|| "default".into());
    application
        .harness_selection(&harness, &model)
        .map_err(Into::into)
}

#[derive(Clone, Debug)]
struct SelectedContainer {
    path: PathBuf,
    kind: XcodeContainerKind,
}

fn resolve_container(path: &Path) -> Result<SelectedContainer, BoxError> {
    if !path.is_absolute() {
        return Err("choose an absolute Xcode project or workspace path".into());
    }
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("the selected Xcode container must be a real directory".into());
    }
    let canonical = fs::canonicalize(path)?;
    let extension = canonical.extension().and_then(|value| value.to_str());
    let kind = match extension {
        Some("xcodeproj") => XcodeContainerKind::Project,
        Some("xcworkspace") => XcodeContainerKind::Workspace,
        _ => {
            let mut containers = fs::read_dir(&canonical)?
                .take(256)
                .filter_map(Result::ok)
                .filter_map(|entry| {
                    let path = entry.path();
                    match path.extension().and_then(|value| value.to_str()) {
                        Some("xcworkspace") => Some((0, path, XcodeContainerKind::Workspace)),
                        Some("xcodeproj") => Some((1, path, XcodeContainerKind::Project)),
                        _ => None,
                    }
                })
                .collect::<Vec<_>>();
            containers
                .sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
            if containers.len() != 1 {
                return Err("choose one exact .xcodeproj or .xcworkspace".into());
            }
            let (_, path, kind) = containers.remove(0);
            return resolve_container_with_kind(path, kind);
        }
    };
    resolve_container_with_kind(canonical, kind)
}

fn resolve_container_with_kind(
    path: PathBuf,
    kind: XcodeContainerKind,
) -> Result<SelectedContainer, BoxError> {
    let metadata = fs::symlink_metadata(&path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("the selected Xcode container is unsafe".into());
    }
    Ok(SelectedContainer { path, kind })
}

fn list_schemes(container: &SelectedContainer) -> Result<Vec<String>, BoxError> {
    let output = Command::new("xcodebuild")
        .arg(container.kind.flag())
        .arg(&container.path)
        .args(["-list", "-json"])
        .stdin(Stdio::null())
        .output()?;
    require_bounded_output(&output)?;
    if !output.status.success() {
        return Err(format!(
            "Xcode could not inspect this container: {}",
            bounded_message(&String::from_utf8_lossy(&output.stderr), 1_000)
        )
        .into());
    }
    let value: Value = serde_json::from_slice(&output.stdout)?;
    let mut schemes = ["workspace", "project"]
        .iter()
        .filter_map(|key| value.get(key))
        .filter_map(|value| value.get("schemes"))
        .filter_map(Value::as_array)
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    schemes.sort();
    schemes.dedup();
    Ok(schemes)
}

fn show_app_settings(
    container: &SelectedContainer,
    source_root: &Path,
    scheme: &str,
) -> Result<BTreeMap<String, String>, BoxError> {
    let output = Command::new("xcodebuild")
        .arg(container.kind.flag())
        .arg(&container.path)
        .args([
            "-scheme",
            scheme,
            "-configuration",
            "Debug",
            "-sdk",
            "iphoneos",
        ])
        .args(["-showBuildSettings", "-json"])
        .current_dir(source_root)
        .stdin(Stdio::null())
        .output()?;
    require_bounded_output(&output)?;
    if !output.status.success() {
        return Err("Xcode could not resolve build settings for this scheme".into());
    }
    let entries: Vec<Value> = serde_json::from_slice(&output.stdout)?;
    for entry in entries {
        let Some(settings) = entry.get("buildSettings").and_then(Value::as_object) else {
            continue;
        };
        let product_type = settings.get("PRODUCT_TYPE").and_then(Value::as_str);
        let wrapper = settings.get("WRAPPER_EXTENSION").and_then(Value::as_str);
        if product_type == Some("com.apple.product-type.application") || wrapper == Some("app") {
            return Ok(settings
                .iter()
                .filter_map(|(key, value)| value.as_str().map(|value| (key.clone(), value.into())))
                .collect());
        }
    }
    Err("scheme does not resolve to an iOS application target".into())
}

fn nonempty_setting(settings: &BTreeMap<String, String>, key: &str) -> Option<String> {
    settings
        .get(key)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty() && !value.contains("$("))
        .map(str::to_owned)
}

async fn run_project_tests(project: &LivingProjectRecord, directory: &Path) -> TestAttempt {
    let started_at = now();
    let Some(destination) = available_iphone_simulator() else {
        return TestAttempt {
            started_at,
            completed_at: now(),
            destination: None,
            success: false,
            skipped: true,
            category: Some("simulator_unavailable".into()),
            summary: "No available iPhone Simulator was installed; the signed device build will still be verified, but no test success is claimed.".into(),
        };
    };
    let derived = directory.join("test-derived-data");
    let log = directory.join("xcode-test.log");
    let arguments = vec![
        project.container_kind.flag().into(),
        project.container_path.clone().into(),
        "-scheme".into(),
        project.scheme.clone().into(),
        "-configuration".into(),
        "Debug".into(),
        "-destination".into(),
        format!("id={destination}").into(),
        "-derivedDataPath".into(),
        derived.as_os_str().to_owned(),
        "CODE_SIGNING_ALLOWED=NO".into(),
        "test".into(),
    ];
    let result = run_logged_async(
        "xcodebuild",
        &arguments,
        Path::new(&project.source_path),
        &log,
        XCODE_TIMEOUT,
    )
    .await;
    match result {
        Ok(status) if status.success() => TestAttempt {
            started_at,
            completed_at: now(),
            destination: Some(destination),
            success: true,
            skipped: false,
            category: None,
            summary: "The selected scheme's tests passed on a real iPhone Simulator destination."
                .into(),
        },
        Ok(status) => {
            let diagnostic = read_log_tail(&log, 8_000).unwrap_or_else(|_| status.to_string());
            let lower = diagnostic.to_ascii_lowercase();
            let no_tests = lower.contains("not currently configured for the test action")
                || lower.contains("does not have any test targets")
                || lower.contains("test action is not configured")
                || lower.contains("no test bundles are available");
            TestAttempt {
                started_at,
                completed_at: now(),
                destination: Some(destination),
                success: false,
                skipped: no_tests,
                category: Some(if no_tests {
                    "tests_not_configured".into()
                } else {
                    "test_failure".into()
                }),
                summary: if no_tests {
                    "The selected scheme has no configured test action; no test success is claimed."
                        .into()
                } else {
                    bounded_message(&diagnostic, 1_000)
                },
            }
        }
        Err(error) => TestAttempt {
            started_at,
            completed_at: now(),
            destination: Some(destination),
            success: false,
            skipped: false,
            category: Some("test_failure".into()),
            summary: bounded_message(&error.to_string(), 1_000),
        },
    }
}

fn available_iphone_simulator() -> Option<String> {
    let output = Command::new("xcrun")
        .args(["simctl", "list", "devices", "available", "--json"])
        .stdin(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() || require_bounded_output(&output).is_err() {
        return None;
    }
    parse_available_iphone_simulator(&output.stdout)
}

fn parse_available_iphone_simulator(bytes: &[u8]) -> Option<String> {
    let value: Value = serde_json::from_slice(bytes).ok()?;
    let devices = value.get("devices")?.as_object()?;
    let mut candidates = devices
        .values()
        .filter_map(Value::as_array)
        .flatten()
        .filter(|device| {
            device
                .get("name")
                .and_then(Value::as_str)
                .is_some_and(|name| name.starts_with("iPhone"))
                && device
                    .get("isAvailable")
                    .and_then(Value::as_bool)
                    .unwrap_or(true)
        })
        .filter_map(|device| {
            device
                .get("udid")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.into_iter().next()
}

fn xcode_build_arguments(
    project: &LivingProjectRecord,
    derived: &Path,
    configuration: &str,
    sdk: &str,
    destination: &str,
    signing: bool,
) -> Vec<OsString> {
    let mut arguments = vec![
        project.container_kind.flag().into(),
        project.container_path.clone().into(),
        "-scheme".into(),
        project.scheme.clone().into(),
        "-configuration".into(),
        configuration.into(),
        "-sdk".into(),
        sdk.into(),
        "-destination".into(),
        destination.into(),
        "-derivedDataPath".into(),
        derived.as_os_str().to_owned(),
    ];
    if signing {
        arguments.push("-allowProvisioningUpdates".into());
        arguments.push("CODE_SIGNING_ALLOWED=YES".into());
        arguments.push("CODE_SIGNING_REQUIRED=YES".into());
    } else {
        arguments.push("CODE_SIGNING_ALLOWED=NO".into());
    }
    arguments.push("build".into());
    arguments
}

fn run_logged_blocking(
    program: &str,
    arguments: &[OsString],
    directory: &Path,
    log: &Path,
    timeout: Duration,
) -> Result<(), BoxError> {
    let stdout = create_log(log)?;
    let stderr = stdout.try_clone()?;
    let mut child = Command::new(program)
        .args(arguments)
        .current_dir(directory)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()?;
    let started = std::time::Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("{program} exceeded its bounded execution time").into());
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    if status.success() {
        Ok(())
    } else {
        let diagnostic = read_log_tail(log, 8_000).unwrap_or_else(|_| status.to_string());
        Err(bounded_message(&diagnostic, 1_000).into())
    }
}

async fn run_logged_async(
    program: &str,
    arguments: &[OsString],
    directory: &Path,
    log: &Path,
    timeout: Duration,
) -> Result<std::process::ExitStatus, BoxError> {
    let stdout = create_log(log)?;
    let stderr = stdout.try_clone()?;
    let mut child = tokio::process::Command::new(program)
        .args(arguments)
        .current_dir(directory)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .kill_on_drop(true)
        .spawn()?;
    match tokio::time::timeout(timeout, child.wait()).await {
        Ok(result) => Ok(result?),
        Err(_) => {
            let _ = child.kill().await;
            Err(format!("{program} exceeded its bounded execution time").into())
        }
    }
}

async fn run_harness(
    harness: tohseno_engine::HarnessCommand,
    directory: &Path,
    log: &Path,
) -> Result<std::process::ExitStatus, BoxError> {
    let stdout = create_log(log)?;
    let stderr = stdout.try_clone()?;
    let mut command = tokio::process::Command::new(harness.program);
    command
        .args(harness.arguments)
        .envs(harness.environment)
        .current_dir(directory)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .kill_on_drop(true);
    for name in harness.removed_environment {
        command.env_remove(name);
    }
    let mut child = command.spawn()?;
    match tokio::time::timeout(HARNESS_TIMEOUT, child.wait()).await {
        Ok(result) => Ok(result?),
        Err(_) => {
            let _ = child.kill().await;
            Err("coding harness exceeded the shared one-hour execution budget".into())
        }
    }
}

fn find_built_app(derived: &Path, wrapper_name: &str) -> Option<PathBuf> {
    let products = derived.join("Build/Products");
    let mut candidates = Vec::new();
    collect_built_apps(&products, 0, &mut candidates).ok()?;
    candidates.sort_by(|left, right| {
        let left_match = left.file_name().and_then(|value| value.to_str()) == Some(wrapper_name);
        let right_match = right.file_name().and_then(|value| value.to_str()) == Some(wrapper_name);
        right_match.cmp(&left_match).then_with(|| left.cmp(right))
    });
    candidates.into_iter().next()
}

fn app_versions(app: &Path) -> (Option<String>, Option<String>) {
    let info = app.join("Info.plist");
    (
        plist_value(&info, "CFBundleShortVersionString"),
        plist_value(&info, "CFBundleVersion"),
    )
}

fn plist_value(path: &Path, key: &str) -> Option<String> {
    let metadata = fs::symlink_metadata(path).ok()?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > 4 * 1024 * 1024
    {
        return None;
    }
    let output = Command::new("/usr/libexec/PlistBuddy")
        .args(["-c", &format!("Print :{key}")])
        .arg(path)
        .stdin(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() || require_bounded_output(&output).is_err() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty() && value.len() <= 256)
}

fn collect_built_apps(
    directory: &Path,
    depth: usize,
    values: &mut Vec<PathBuf>,
) -> std::io::Result<()> {
    if depth > 3 || values.len() >= 128 {
        return Ok(());
    }
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in entries.take(2_048) {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) == Some("app") {
            values.push(path);
        } else {
            collect_built_apps(&path, depth + 1, values)?;
        }
    }
    Ok(())
}

fn verify_codesign(app: &Path) -> Result<(), BoxError> {
    let status = Command::new("/usr/bin/codesign")
        .args(["--verify", "--deep", "--strict", "--verbose=2"])
        .arg(app)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if !status.success() {
        return Err("the built app did not pass local codesign verification".into());
    }
    Ok(())
}

fn observe_git(source: &Path) -> Result<Option<GitObservation>, BoxError> {
    let root = git_output(source, &["rev-parse", "--show-toplevel"])?;
    let Some(root) = root else { return Ok(None) };
    let root = root.trim();
    if root.is_empty() {
        return Ok(None);
    }
    let root_path = fs::canonicalize(root)?;
    if !source.starts_with(&root_path) && !fs::canonicalize(source)?.starts_with(&root_path) {
        return Err("Git reported a repository root unrelated to the selected source".into());
    }
    let revision = git_output(&root_path, &["rev-parse", "HEAD"])?
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    let status = Command::new("git")
        .args(["-C"])
        .arg(&root_path)
        .args(["status", "--porcelain=v1", "-z", "--untracked-files=all"])
        .stdin(Stdio::null())
        .output()?;
    require_bounded_output(&status)?;
    if !status.status.success() {
        return Err("Git could not inspect the selected working tree".into());
    }
    let dirty_paths = parse_git_status_paths(&status.stdout);
    Ok(Some(GitObservation {
        repository_root: root_path.display().to_string(),
        revision,
        dirty: !dirty_paths.is_empty(),
        dirty_paths,
    }))
}

fn git_output(source: &Path, arguments: &[&str]) -> Result<Option<String>, BoxError> {
    let output = Command::new("git")
        .args(["-C"])
        .arg(source)
        .args(arguments)
        .stdin(Stdio::null())
        .output()?;
    require_bounded_output(&output)?;
    if !output.status.success() {
        return Ok(None);
    }
    Ok(Some(String::from_utf8(output.stdout)?))
}

fn parse_git_status_paths(bytes: &[u8]) -> Vec<String> {
    let mut values = Vec::new();
    let mut entries = bytes.split(|byte| *byte == 0);
    while let Some(entry) = entries.next() {
        if entry.len() < 4 || entry[2] != b' ' {
            continue;
        }
        let renamed = matches!(entry[0], b'R' | b'C') || matches!(entry[1], b'R' | b'C');
        if let Ok(path) = String::from_utf8(entry[3..].to_vec()) {
            values.push(path);
        }
        if renamed {
            if let Some(original) = entries.next() {
                if let Ok(path) = String::from_utf8(original.to_vec()) {
                    values.push(path);
                }
            }
        }
    }
    values.sort();
    values.dedup();
    values.truncate(10_000);
    values
}

fn source_state(source: &Path, git: Option<&GitObservation>) -> Result<String, BoxError> {
    let mut hasher = Sha256::new();
    hasher.update(b"TOHSENO-PRIVATE-PROJECT-SOURCE-STATE-V1\0");
    if let Some(git) = git {
        hasher.update(git.revision.as_deref().unwrap_or("unborn").as_bytes());
        let mut content_budget = 256_u64 * 1024 * 1024;
        for path in &git.dirty_paths {
            hasher.update([0]);
            hasher.update(path.as_bytes());
            let relative = Path::new(path);
            if relative.is_absolute()
                || relative
                    .components()
                    .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
            {
                hasher.update(b"\0unsafe-git-path");
                continue;
            }
            let absolute = Path::new(&git.repository_root).join(relative);
            if let Ok(metadata) = fs::symlink_metadata(&absolute) {
                hasher.update(metadata.len().to_le_bytes());
                if metadata.file_type().is_symlink() {
                    hasher.update(b"\0symlink");
                } else if metadata.is_file()
                    && metadata.len() <= 16 * 1024 * 1024
                    && metadata.len() <= content_budget
                {
                    if let Ok(mut file) = open_regular_read(&absolute) {
                        let mut buffer = [0_u8; 32 * 1024];
                        loop {
                            let read = file.read(&mut buffer)?;
                            if read == 0 {
                                break;
                            }
                            hasher.update(&buffer[..read]);
                        }
                        content_budget = content_budget.saturating_sub(metadata.len());
                    }
                } else if let Ok(modified) = metadata.modified() {
                    if let Ok(since_epoch) = modified.duration_since(std::time::UNIX_EPOCH) {
                        hasher.update(since_epoch.as_nanos().to_le_bytes());
                    }
                }
            }
        }
    } else {
        let canonical = fs::canonicalize(source)?;
        hasher.update(canonical.as_os_str().as_encoded_bytes());
        let mut file_count = 0_usize;
        let mut content_budget = 256_u64 * 1024 * 1024;
        hash_source_tree(
            &canonical,
            &canonical,
            0,
            &mut file_count,
            &mut content_budget,
            &mut hasher,
        )?;
    }
    let digest = format!("{:x}", hasher.finalize());
    Ok(format!("state_{}", &digest[..32]))
}

fn hash_source_tree(
    root: &Path,
    directory: &Path,
    depth: usize,
    file_count: &mut usize,
    content_budget: &mut u64,
    hasher: &mut Sha256,
) -> Result<(), BoxError> {
    if depth > 32 {
        return Err("non-Git source tree exceeds the supported directory depth".into());
    }
    let mut entries = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let relative = path.strip_prefix(root).unwrap_or(&path);
        let metadata = fs::symlink_metadata(&path)?;
        hasher.update(relative.as_os_str().as_encoded_bytes());
        if metadata.file_type().is_symlink() {
            hasher.update(b"\0symlink");
            continue;
        }
        if metadata.is_dir() {
            let name = entry.file_name();
            if matches!(
                name.to_str(),
                Some(".git" | ".build" | ".tohseno" | "DerivedData" | "build")
            ) {
                hasher.update(b"\0ignored-build-state");
                continue;
            }
            hash_source_tree(root, &path, depth + 1, file_count, content_budget, hasher)?;
            continue;
        }
        if !metadata.is_file() {
            hasher.update(b"\0special");
            continue;
        }
        *file_count = file_count.saturating_add(1);
        if *file_count > 50_000 {
            return Err("non-Git source tree exceeds the supported file count".into());
        }
        hasher.update(metadata.len().to_le_bytes());
        if metadata.len() <= *content_budget && metadata.len() <= 16 * 1024 * 1024 {
            let mut file = open_regular_read(&path)?;
            let mut buffer = [0_u8; 32 * 1024];
            loop {
                let read = file.read(&mut buffer)?;
                if read == 0 {
                    break;
                }
                hasher.update(&buffer[..read]);
            }
            *content_budget = content_budget.saturating_sub(metadata.len());
        } else if let Ok(modified) = metadata.modified() {
            if let Ok(since_epoch) = modified.duration_since(std::time::UNIX_EPOCH) {
                hasher.update(since_epoch.as_nanos().to_le_bytes());
            }
        }
    }
    Ok(())
}

fn source_file_inventory(source: &Path) -> Result<BTreeMap<String, String>, BoxError> {
    let canonical = fs::canonicalize(source)?;
    let mut values = BTreeMap::new();
    let mut file_count = 0_usize;
    let mut content_budget = 256_u64 * 1024 * 1024;
    inventory_source_tree(
        &canonical,
        &canonical,
        0,
        &mut file_count,
        &mut content_budget,
        &mut values,
    )?;
    Ok(values)
}

fn inventory_source_tree(
    root: &Path,
    directory: &Path,
    depth: usize,
    file_count: &mut usize,
    content_budget: &mut u64,
    values: &mut BTreeMap<String, String>,
) -> Result<(), BoxError> {
    if depth > 32 {
        return Err("non-Git source tree exceeds the supported directory depth".into());
    }
    let mut entries = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let relative = path.strip_prefix(root).unwrap_or(&path);
        let relative_text = relative
            .to_str()
            .ok_or("non-Git source contains a non-UTF-8 path")?
            .to_owned();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            values.insert(relative_text, "symlink".into());
            continue;
        }
        if metadata.is_dir() {
            let name = entry.file_name();
            if matches!(
                name.to_str(),
                Some(".git" | ".build" | ".tohseno" | "DerivedData" | "build")
            ) {
                continue;
            }
            inventory_source_tree(root, &path, depth + 1, file_count, content_budget, values)?;
            continue;
        }
        if !metadata.is_file() {
            values.insert(relative_text, "special".into());
            continue;
        }
        *file_count = file_count.saturating_add(1);
        if *file_count > 50_000 {
            return Err("non-Git source tree exceeds the supported file count".into());
        }
        let mut hasher = Sha256::new();
        hasher.update(metadata.len().to_le_bytes());
        if metadata.len() <= *content_budget && metadata.len() <= 16 * 1024 * 1024 {
            let mut file = open_regular_read(&path)?;
            let mut buffer = [0_u8; 32 * 1024];
            loop {
                let read = file.read(&mut buffer)?;
                if read == 0 {
                    break;
                }
                hasher.update(&buffer[..read]);
            }
            *content_budget = content_budget.saturating_sub(metadata.len());
        } else if let Ok(modified) = metadata.modified() {
            if let Ok(since_epoch) = modified.duration_since(std::time::UNIX_EPOCH) {
                hasher.update(since_epoch.as_nanos().to_le_bytes());
            }
        }
        values.insert(relative_text, format!("{:x}", hasher.finalize()));
    }
    Ok(())
}

fn changed_inventory_paths(
    before: &BTreeMap<String, String>,
    after: &BTreeMap<String, String>,
) -> Vec<String> {
    let mut paths = before
        .keys()
        .chain(after.keys())
        .filter(|path| before.get(*path) != after.get(*path))
        .cloned()
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths.truncate(10_000);
    paths
}

fn discover_instructions(
    source: &Path,
    git: Option<&GitObservation>,
) -> Result<Vec<RepositoryInstruction>, BoxError> {
    let root = git
        .map(|value| PathBuf::from(&value.repository_root))
        .unwrap_or_else(|| source.to_path_buf());
    let candidates = [
        "AGENTS.md",
        "CLAUDE.md",
        "MASTER_PROMPT.md",
        "README.md",
        "README",
    ];
    let mut values = Vec::new();
    let mut directory = source.to_path_buf();
    loop {
        for name in candidates {
            let path = directory.join(name);
            let metadata = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.into()),
            };
            if metadata.file_type().is_symlink()
                || !metadata.is_file()
                || metadata.len() > 4 * 1024 * 1024
            {
                continue;
            }
            let bytes = fs::read(&path)?;
            values.push(RepositoryInstruction {
                relative_path: path
                    .strip_prefix(&root)
                    .unwrap_or(&path)
                    .display()
                    .to_string(),
                sha256: format!("{:x}", Sha256::digest(&bytes)),
                byte_length: metadata.len(),
            });
        }
        if directory == root || !directory.pop() || !directory.starts_with(&root) {
            break;
        }
    }
    values.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    values.dedup_by(|left, right| left.relative_path == right.relative_path);
    Ok(values)
}

fn execution_packet(
    project: &LivingProjectRecord,
    evolution: &ProjectEvolutionRecord,
    prior_context: &[ProjectEvolutionRecord],
) -> String {
    let instructions = if project.instructions.is_empty() {
        "- No named repository instruction file was discovered; inspect the repository before editing."
            .into()
    } else {
        project
            .instructions
            .iter()
            .map(|item| format!("- Read `{}` before editing.", item.relative_path))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let attachments = if evolution.attachment_names.is_empty() {
        "none".into()
    } else {
        evolution.attachment_names.join(", ")
    };
    let installed_context = if project.installations.is_empty() {
        "- No verified installation has been observed yet.".into()
    } else {
        project
            .installations
            .iter()
            .map(|installation| {
                format!(
                    "- {} · iOS {} · app {} ({}) · verified {}",
                    installation.device_name,
                    installation.os_version.as_deref().unwrap_or("unknown"),
                    installation.short_version.as_deref().unwrap_or("unknown"),
                    installation.build_number.as_deref().unwrap_or("unknown"),
                    installation.installed_at
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let history = if prior_context.is_empty() {
        "- No prior evolution is relevant.".into()
    } else {
        prior_context
            .iter()
            .map(|record| {
                format!(
                    "- {} [{}]: {} Result: {}",
                    record.evolution_id,
                    record.status.as_str(),
                    bounded_message(&record.user_request.replace('\n', " "), 500),
                    record
                        .completion_summary
                        .as_deref()
                        .map(|value| bounded_message(&value.replace('\n', " "), 500))
                        .unwrap_or_else(|| "No completion summary.".into())
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let container_flag = project.container_kind.flag();
    format!(
        "# Tohseno project evolution\n\nProject: {}\nStable project ID: {}\nBundle: {}\nSource root: {}\nXcode container: {} {}\nScheme: {}\nStarting source state: {}\nStarting Git revision: {}\nRequest received: {}\nFollow-up to: {}\nOriginal intention: {}\nAttachments passed separately to the harness: {}\nPre-existing dirty paths: {}\n\n## Known phone/install context\n\n{}\n\n## Relevant recent history\n\n{}\n\n## User request\n\n{}\n\n## Tohseno build after your work\n\nTohseno will run this argument-equivalent Xcode operation after you exit:\n`xcodebuild {} {} -scheme {} -configuration Debug -sdk iphoneos -destination generic/platform=iOS -allowProvisioningUpdates CODE_SIGNING_ALLOWED=YES CODE_SIGNING_REQUIRED=YES build`\n\n## Required conduct\n\n- Inspect the project before editing.\n- Respect repository instructions and preserve unrelated user changes.\n- Implement only the requested evolution in this existing source tree.\n- Run relevant tests, but do not run Xcode installation; Tohseno owns build and delivery after you exit.\n- Report what changed in your final output.\n- Do not commit, push, publish, deploy, clean, reset, checkout, or discard work.\n- Stop honestly if credentials, permissions, or a material product decision are required.\n\n## Repository instructions\n\n{}\n",
        project.display_name,
        project.project_id,
        project.bundle_identifier,
        project.source_path,
        container_flag,
        project.container_path,
        project.scheme,
        evolution.starting_source_state,
        evolution
            .starting_git_revision
            .as_deref()
            .unwrap_or("not a Git repository or unborn"),
        evolution.received_at,
        evolution.follow_up_to.as_deref().unwrap_or("none"),
        project.original_intention.as_deref().unwrap_or("unknown"),
        attachments,
        if evolution.preexisting_dirty_paths.is_empty() {
            "none recorded".into()
        } else {
            evolution.preexisting_dirty_paths.join(", ")
        },
        installed_context,
        history,
        evolution.user_request,
        container_flag,
        project.container_path,
        project.scheme,
        instructions
    )
}

fn project_presentation(
    project: &LivingProjectRecord,
    execution: Option<&ExecutionSummary>,
) -> Presentation {
    match project.latest_evolution_status {
        Some(EvolutionStatus::Received | EvolutionStatus::Queued) => Presentation {
            state: PresentedState::Waiting,
            headline: "Request received".into(),
            detail: Some("Waiting for the configured coding harness.".into()),
        },
        Some(EvolutionStatus::Working | EvolutionStatus::Building) => Presentation {
            state: PresentedState::Building,
            headline: "Evolving on your Mac".into(),
            detail: Some("Tohseno is changing and verifying this project.".into()),
        },
        Some(EvolutionStatus::ReadyToInstall | EvolutionStatus::WaitingForUserAction) => {
            Presentation {
                state: PresentedState::ReadyForPhone,
                headline: "Ready to install".into(),
                detail: Some("Make the paired iPhone reachable and unlock it to continue.".into()),
            }
        }
        Some(EvolutionStatus::Installing) => Presentation {
            state: PresentedState::Installing,
            headline: "Installing on iPhone".into(),
            detail: None,
        },
        Some(EvolutionStatus::Installed | EvolutionStatus::Completed) => Presentation {
            state: PresentedState::Installed,
            headline: "Installed".into(),
            detail: project.last_successful_installation.as_ref().map(|_| {
                "The exact bundle was verified in the paired iPhone's app inventory.".into()
            }),
        },
        Some(EvolutionStatus::Failed) => Presentation {
            state: PresentedState::Failed,
            headline: "Needs attention".into(),
            detail: Some("Open this app on the Mac to review the saved failure and retry.".into()),
        },
        None if project.build.status == "failed" => Presentation {
            state: PresentedState::Failed,
            headline: "Adopted · build needs attention".into(),
            detail: project.build.summary.clone(),
        },
        None => Presentation {
            state: PresentedState::Installed,
            headline: "Connected to source".into(),
            detail: execution.map(|_| "Evolution history is available on this Mac.".into()),
        },
    }
}

fn classify_xcode_failure(value: &str) -> &'static str {
    let lower = value.to_ascii_lowercase();
    if lower.contains("signing for")
        || lower.contains("provisioning profile")
        || lower.contains("requires a development team")
        || lower.contains("code signing")
    {
        "signing_error"
    } else if lower.contains("test failed") || lower.contains("failing tests") {
        "test_failure"
    } else if lower.contains("no such module")
        || lower.contains("swift compiler error")
        || lower.contains("compile") && lower.contains("error:")
    {
        "source_error"
    } else if lower.contains("timed out") || lower.contains("bounded execution time") {
        "build_timeout"
    } else {
        "build_error"
    }
}

fn classify_install_failure(value: &str) -> &'static str {
    let lower = value.to_ascii_lowercase();
    if lower.contains("locked") || lower.contains("could not be unlocked") {
        "device_locked"
    } else if lower.contains("developer mode") {
        "developer_mode_required"
    } else if lower.contains("trust") || lower.contains("pair") {
        "trust_required"
    } else if lower.contains("unavailable") || lower.contains("not found") {
        "device_unavailable"
    } else if lower.contains("signature") || lower.contains("provision") {
        "signing_error"
    } else {
        "installation_error"
    }
}

fn recovery_for_category(category: &str) -> &'static str {
    match category {
        "signing_error" => "Open Xcode Settings → Accounts and let Xcode manage signing for this project. Tohseno never asks for your Apple Account password.",
        "source_error" => "Review the saved Xcode log and ask the coding harness to repair the reported source error.",
        "test_failure" => "Review the failing test in the saved build log before retrying.",
        "device_locked" => "Unlock the paired iPhone and keep it awake while Tohseno retries installation.",
        "developer_mode_required" => "Enable Developer Mode in iPhone Settings → Privacy & Security, restart, and reconnect.",
        "trust_required" => "Connect the iPhone, unlock it, and tap Trust This Computer.",
        "device_unavailable" => "Make the paired iPhone reachable over Xcode-supported Wi-Fi or USB, then keep it unlocked.",
        _ => "Open the saved local log for the concrete Xcode or devicectl error, then retry.",
    }
}

fn command_digest(request: &ProjectEvolutionRequest) -> Result<String, BoxError> {
    let references = request
        .references
        .iter()
        .map(|reference| {
            json!({
                "name": reference.display_filename,
                "media_type": reference.media_type,
                "sha256": format!("{:x}", Sha256::digest(&reference.bytes)),
            })
        })
        .collect::<Vec<_>>();
    let value = json!({
        "project_id": request.project_id,
        "base_source_state": request.base_source_state,
        "intention": request.intention,
        "originating_device_id": request.originating_device_id,
        "references": references,
        "follow_up_to": request.follow_up_to,
    });
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(&value)?)))
}

fn device_digest(identifier: &str) -> String {
    let digest =
        protocol_sha256(format!("TOHSENO-PRIVATE-OWNER-DEVICE-V1\0{identifier}").as_bytes());
    digest.to_string().trim_start_matches("0x").into()
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum InstallTargetSelection {
    Ready(Device),
    /// A durable Companion bootstrap target exists, but it is not among the
    /// currently reachable CoreDevices. Never fall through to another phone.
    TargetUnreachable,
    /// Pre-association compatibility state with more than one visible phone.
    AssociationRequired,
}

fn select_install_target(
    mut devices: Vec<Device>,
    intended_device_digest: Option<&str>,
) -> InstallTargetSelection {
    if let Some(intended) = intended_device_digest {
        return devices
            .into_iter()
            .find(|device| companion_install_target_digest(&device.identifier) == intended)
            .map(InstallTargetSelection::Ready)
            .unwrap_or(InstallTargetSelection::TargetUnreachable);
    }
    if devices.len() == 1 {
        InstallTargetSelection::Ready(devices.remove(0))
    } else {
        InstallTargetSelection::AssociationRequired
    }
}

fn push_event(record: &mut ProjectEvolutionRecord, status: EvolutionStatus, summary: &str) {
    let sequence = record
        .events
        .last()
        .map(|event| event.sequence.saturating_add(1))
        .unwrap_or(1);
    record.events.push(EvolutionEvent {
        sequence,
        status,
        at: now(),
        summary: bounded_message(summary, 1_000),
    });
    if record.events.len() > 512 {
        record.events.drain(..record.events.len() - 512);
    }
}

fn validate_project(value: &LivingProjectRecord) -> Result<(), BoxError> {
    if value.schema != PROJECT_SCHEMA || value.revision == 0 {
        return Err("living-project record version is invalid".into());
    }
    validate_id("project ID", &value.project_id)?;
    validate_text("display name", &value.display_name, 256)?;
    validate_text("scheme", &value.scheme, 256)?;
    validate_bundle_identifier(&value.bundle_identifier)?;
    validate_id("source state", &value.current_source_state)?;
    if let Some(shot_id) = &value.candidate_shot_id {
        validate_hex_digest("candidate ShotID", shot_id)?;
    }
    if let Some(publication) = &value.latest_publication {
        validate_hex_digest(
            "publication release digest",
            publication.release_digest.trim_start_matches("0x"),
        )?;
        if publication.checkpoint_sequence == 0 {
            return Err("publication checkpoint sequence is invalid".into());
        }
        validate_text("publication status", &publication.status, 64)?;
    }
    if let Some(origin) = &value.network_origin {
        validate_prefixed_hex_digest("network parent ShotID", &origin.parent_shot_id)?;
        validate_prefixed_hex_digest(
            "network parent release digest",
            &origin.parent_release_digest,
        )?;
        validate_prefixed_hex_digest(
            "network source artifact digest",
            &origin.source_artifact_sha256,
        )?;
        if !origin.builder_id.starts_with("eip155:4663:0x") || origin.builder_id.len() != 54 {
            return Err("network BuilderID is invalid".into());
        }
        tohseno_companion::parse_timestamp(&origin.verified_at)?;
        if origin.kind == NetworkImportKind::Install && value.candidate_shot_id.is_some() {
            return Err("an install-only network import cannot claim a child ShotID".into());
        }
        if origin.kind == NetworkImportKind::Fork && value.candidate_shot_id.is_none() {
            return Err("a network fork must reserve a new child ShotID".into());
        }
    }
    if let Some(delivery) = &value.network_delivery {
        validate_prefixed_hex_digest("network delivery release", &delivery.release_digest)?;
        validate_text("network delivery status", &delivery.status, 64)?;
        validate_bundle_identifier(&delivery.local_bundle_identifier)?;
        if let Some(expires) = &delivery.provisioning_expires_at {
            tohseno_companion::parse_timestamp(expires)?;
        }
        if let Some(failure) = &delivery.failure {
            validate_text("network delivery failure", failure, 1_000)?;
        }
        tohseno_companion::parse_timestamp(&delivery.updated_at)?;
    }
    if value.installations.len() > 128
        || value.instructions.len() > 128
        || value.associated_companion_device_ids.len() > 128
    {
        return Err("living-project record exceeds its bounded collections".into());
    }
    for device_id in &value.associated_companion_device_ids {
        validate_id("associated Companion device ID", device_id)?;
    }
    for installation in &value.installations {
        validate_id(
            "physical device identifier digest",
            &installation.device_identifier_digest,
        )?;
        validate_text("physical device name", &installation.device_name, 256)?;
        tohseno_companion::parse_timestamp(&installation.installed_at)?;
        if let Some(version) = &installation.short_version {
            validate_text("installed app version", version, 256)?;
        }
        if let Some(build) = &installation.build_number {
            validate_text("installed app build", build, 256)?;
        }
    }
    Ok(())
}

fn validate_evolution(value: &ProjectEvolutionRecord) -> Result<(), BoxError> {
    if value.schema != EVOLUTION_SCHEMA || value.revision == 0 {
        return Err("project-evolution record version is invalid".into());
    }
    validate_id("project ID", &value.project_id)?;
    validate_id("evolution ID", &value.evolution_id)?;
    validate_id("command ID", &value.command_id)?;
    validate_hex_digest("command digest", &value.command_digest)?;
    validate_intention(&value.user_request)?;
    if value.attachment_names.len() > 8
        || value.events.len() > 512
        || value.test_attempts.len() > 64
        || value.build_attempts.len() > 64
        || value.installation_attempts.len() > 128
        || value.observed_changed_files.len() > 10_000
    {
        return Err("project-evolution record exceeds its bounded collections".into());
    }
    Ok(())
}

fn validate_hex_digest(label: &str, value: &str) -> Result<(), BoxError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(format!("{label} is invalid").into());
    }
    Ok(())
}

fn validate_prefixed_hex_digest(label: &str, value: &str) -> Result<(), BoxError> {
    let body = value
        .strip_prefix("0x")
        .ok_or_else(|| format!("{label} is invalid"))?;
    validate_hex_digest(label, body)
}

fn local_network_bundle_identifier(shot_id: &str) -> String {
    let body = shot_id.strip_prefix("0x").unwrap_or(shot_id);
    let short = body.get(..24).unwrap_or(body);
    format!("org.tohseno.genesis.network.s{short}")
}

fn validate_id(label: &str, value: &str) -> Result<(), BoxError> {
    if value.is_empty()
        || value.len() > 160
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(format!("{label} is invalid").into());
    }
    Ok(())
}

fn validate_text(label: &str, value: &str, maximum: usize) -> Result<(), BoxError> {
    if value.trim().is_empty() || value.len() > maximum || value.contains('\0') {
        return Err(format!("{label} is empty, oversized, or contains NUL").into());
    }
    Ok(())
}

fn validate_intention(value: &str) -> Result<(), BoxError> {
    validate_text("project evolution request", value, MAX_INTENTION_BYTES)
}

fn validate_bundle_identifier(value: &str) -> Result<(), BoxError> {
    if value.is_empty()
        || value.len() > 255
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
        || !value.contains('.')
    {
        return Err("application bundle identifier is invalid".into());
    }
    Ok(())
}

fn require_bounded_output(output: &std::process::Output) -> Result<(), BoxError> {
    if output.stdout.len().saturating_add(output.stderr.len()) > MAX_COMMAND_OUTPUT_BYTES {
        return Err("tool output exceeded its local inspection limit".into());
    }
    Ok(())
}

fn bounded_message(value: &str, maximum: usize) -> String {
    value.chars().take(maximum).collect()
}

fn now() -> String {
    OffsetDateTime::now_utc()
        .replace_nanosecond(0)
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
}

fn create_log(path: &Path) -> Result<File, BoxError> {
    if let Some(parent) = path.parent() {
        ensure_private_directory(parent)?;
    }
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    Ok(options.open(path)?)
}

fn open_regular_read(path: &Path) -> Result<File, BoxError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(format!(
            "source-state input is not a regular file: {}",
            path.display()
        )
        .into());
    }
    Ok(file)
}

fn read_log_tail(path: &Path, maximum: usize) -> Result<String, BoxError> {
    let bytes = fs::read(path)?;
    let start = bytes.len().saturating_sub(maximum);
    Ok(String::from_utf8_lossy(&bytes[start..]).into_owned())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, BoxError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > MAX_RECORD_BYTES
    {
        return Err(format!("private record is unsafe: {}", path.display()).into());
    }
    let bytes = fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn write_new<T: Serialize>(path: &Path, value: &T) -> Result<(), BoxError> {
    if path.exists() {
        return Err(format!("refusing to overwrite private record {}", path.display()).into());
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    write_private_bytes_new(path, &bytes)
}

fn write_replace<T: Serialize>(path: &Path, value: &T) -> Result<(), BoxError> {
    let bytes = serde_json::to_vec_pretty(value)?;
    if bytes.len() as u64 > MAX_RECORD_BYTES {
        return Err("private record exceeds its storage limit".into());
    }
    let parent = path.parent().ok_or("private record has no parent")?;
    ensure_private_directory(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        temporary
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    temporary.write_all(&bytes)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

fn write_private_bytes(path: &Path, bytes: &[u8]) -> Result<(), BoxError> {
    if path.exists() {
        let parent = path.parent().ok_or("private file has no parent")?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            temporary
                .as_file()
                .set_permissions(fs::Permissions::from_mode(0o600))?;
        }
        temporary.write_all(bytes)?;
        temporary.as_file().sync_all()?;
        temporary.persist(path).map_err(|error| error.error)?;
        return Ok(());
    }
    write_private_bytes_new(path, bytes)
}

fn write_private_bytes_new(path: &Path, bytes: &[u8]) -> Result<(), BoxError> {
    let parent = path.parent().ok_or("private file has no parent")?;
    ensure_private_directory(parent)?;
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

fn ensure_private_directory(path: &Path) -> Result<(), BoxError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(format!("private storage path is unsafe: {}", path.display()).into())
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(path)?;
        }
        Err(error) => return Err(error.into()),
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn physical_device(identifier: &str, transport: &str) -> Device {
        Device {
            identifier: identifier.into(),
            udid: Some(format!("udid-{identifier}")),
            name: format!("iPhone {identifier}"),
            product_type: Some("iPhone18,1".into()),
            marketing_name: Some("iPhone".into()),
            os_version: Some("26.0".into()),
            os_build: Some("23A000".into()),
            physical: true,
            transport: transport.into(),
        }
    }

    #[test]
    fn companion_bootstrap_target_wins_over_transport_and_inventory_order() {
        let intended = companion_install_target_digest("phone-b");
        let selected = select_install_target(
            vec![
                physical_device("phone-a", "usb"),
                physical_device("phone-b", "localNetwork"),
            ],
            Some(&intended),
        );
        let InstallTargetSelection::Ready(device) = selected else {
            panic!("the intended Companion phone should be selected");
        };
        assert_eq!(device.identifier, "phone-b");
        assert_eq!(device.transport, "localNetwork");
    }

    #[test]
    fn intended_phone_never_falls_through_to_another_reachable_phone() {
        let intended = companion_install_target_digest("phone-b");
        assert_eq!(
            select_install_target(vec![physical_device("phone-a", "usb")], Some(&intended)),
            InstallTargetSelection::TargetUnreachable
        );
    }

    #[test]
    fn legacy_target_selection_keeps_the_exactly_one_phone_fallback() {
        let only = physical_device("phone-a", "usb");
        assert_eq!(
            select_install_target(vec![only.clone()], None),
            InstallTargetSelection::Ready(only)
        );
        assert_eq!(
            select_install_target(
                vec![
                    physical_device("phone-a", "usb"),
                    physical_device("phone-b", "localNetwork"),
                ],
                None,
            ),
            InstallTargetSelection::AssociationRequired
        );
    }

    #[test]
    fn xcode_failure_categories_remain_actionable() {
        assert_eq!(
            classify_xcode_failure("Signing for App requires a development team"),
            "signing_error"
        );
        assert_eq!(
            classify_xcode_failure("Swift compiler error: missing member"),
            "source_error"
        );
        assert_eq!(classify_xcode_failure("command timed out"), "build_timeout");
        assert_eq!(
            classify_install_failure("device was not, or could not be, unlocked"),
            "device_locked"
        );
        assert_eq!(
            classify_install_failure("Developer Mode is disabled"),
            "developer_mode_required"
        );
    }

    #[test]
    fn simulator_inventory_selects_only_available_iphones_deterministically() {
        let inventory = br#"{
            "devices": {
                "com.apple.CoreSimulator.SimRuntime.iOS-26-0": [
                    {"name":"iPad Pro","udid":"IPAD","isAvailable":true},
                    {"name":"iPhone 17","udid":"PHONE-B","isAvailable":true},
                    {"name":"iPhone 16","udid":"PHONE-A","isAvailable":true},
                    {"name":"iPhone 15","udid":"PHONE-OLD","isAvailable":false}
                ]
            }
        }"#;
        assert_eq!(
            parse_available_iphone_simulator(inventory).as_deref(),
            Some("PHONE-A")
        );
        assert!(parse_available_iphone_simulator(br#"{"devices":{}}"#).is_none());
    }

    #[test]
    fn evolution_validation_requires_the_exact_command_digest() {
        assert!(validate_hex_digest("command digest", &"a".repeat(64)).is_ok());
        assert!(validate_hex_digest("command digest", &"A".repeat(64)).is_err());
        assert!(validate_hex_digest("command digest", &"a".repeat(63)).is_err());
    }

    #[test]
    fn git_status_parser_is_bounded_and_stable() {
        let paths = parse_git_status_paths(b" M Sources/App.swift\0?? New File.swift\0");
        assert_eq!(paths, ["New File.swift", "Sources/App.swift"]);
        let renamed = parse_git_status_paths(b"R  New.swift\0Old.swift\0");
        assert_eq!(renamed, ["New.swift", "Old.swift"]);
    }

    #[cfg(unix)]
    #[test]
    fn git_source_state_never_follows_a_dirty_symlink_outside_the_repository() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::NamedTempFile::new().unwrap();
        fs::write(outside.path(), b"private-one").unwrap();
        symlink(outside.path(), root.path().join("Reference")).unwrap();
        let observation = GitObservation {
            repository_root: root.path().display().to_string(),
            revision: Some("revision_fixture".into()),
            dirty: true,
            dirty_paths: vec!["Reference".into()],
        };
        let before = source_state(root.path(), Some(&observation)).unwrap();
        fs::write(outside.path(), b"private-two").unwrap();
        let after = source_state(root.path(), Some(&observation)).unwrap();
        assert_eq!(before, after);
    }

    #[test]
    fn non_git_source_state_detects_same_length_content_changes() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("ContentView.swift");
        fs::write(&source, b"Text(\"X\")").unwrap();
        let before = source_state(root.path(), None).unwrap();
        let before_inventory = source_file_inventory(root.path()).unwrap();
        fs::write(&source, b"Text(\"Y\")").unwrap();
        let after = source_state(root.path(), None).unwrap();
        let after_inventory = source_file_inventory(root.path()).unwrap();
        assert_ne!(before, after);
        assert_eq!(
            changed_inventory_paths(&before_inventory, &after_inventory),
            ["ContentView.swift"]
        );
    }

    #[test]
    fn versioned_store_fails_closed_on_unknown_schema() {
        let root = tempfile::tempdir().unwrap();
        let store = root.path().join("living-projects-v1");
        fs::create_dir_all(&store).unwrap();
        fs::write(
            store.join("store.json"),
            br#"{"schema":"future/9","revision":9,"created_at":"2026-08-30T00:00:00Z"}"#,
        )
        .unwrap();
        // Constructing the application service is intentionally outside this
        // storage-schema unit. The manifest parser itself is the migration
        // authority used by `open`.
        let manifest: StoreManifest = read_json(&store.join("store.json")).unwrap();
        assert_ne!(manifest.schema, STORE_SCHEMA);
        assert_ne!(manifest.revision, 1);
    }

    #[test]
    fn adopted_identifiers_are_not_protocol_digests() {
        let id = format!("project_{}", Uuid::nil().simple());
        validate_id("project ID", &id).unwrap();
        assert!(!id.starts_with("0x"));
        assert!(tohseno_protocol::digest::Bytes32::from_hex("Shot ID", &id).is_err());
    }

    #[test]
    fn evolution_state_machine_rejects_skips_and_terminal_reentry() {
        assert!(transition_allowed(
            EvolutionStatus::Received,
            EvolutionStatus::Working
        ));
        assert!(transition_allowed(
            EvolutionStatus::Installing,
            EvolutionStatus::ReadyToInstall
        ));
        assert!(transition_allowed(
            EvolutionStatus::Installed,
            EvolutionStatus::Completed
        ));
        assert!(!transition_allowed(
            EvolutionStatus::Received,
            EvolutionStatus::Completed
        ));
        assert!(!transition_allowed(
            EvolutionStatus::Completed,
            EvolutionStatus::Working
        ));
        assert!(!transition_allowed(
            EvolutionStatus::Failed,
            EvolutionStatus::Installing
        ));
    }

    #[test]
    fn private_records_round_trip_without_losing_source_or_install_state() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("project_fixture.json");
        let record = LivingProjectRecord {
            schema: PROJECT_SCHEMA.into(),
            revision: 1,
            project_id: "project_fixture".into(),
            display_name: "Fixture".into(),
            source_path: "/tmp/Fixture".into(),
            container_path: "/tmp/Fixture/Fixture.xcodeproj".into(),
            container_kind: XcodeContainerKind::Project,
            scheme: "Fixture".into(),
            bundle_identifier: "com.example.fixture".into(),
            deployment_target: Some("17.0".into()),
            signing_team: Some("ABCDE12345".into()),
            product_name: "Fixture".into(),
            wrapper_name: "Fixture.app".into(),
            harness: ProjectHarness {
                harness: "codex".into(),
                model: "default".into(),
                route: "local".into(),
            },
            original_intention: None,
            instructions: Vec::new(),
            git: None,
            current_source_state: "state_fixture".into(),
            candidate_shot_id: Some("11".repeat(32)),
            latest_publication: None,
            network_origin: None,
            network_delivery: None,
            build: ProjectBuildState::default(),
            associated_companion_device_ids: vec!["device_fixture".into()],
            installations: vec![DeviceInstallation {
                device_identifier_digest: "device_fixture".into(),
                device_name: "Owner iPhone".into(),
                os_version: Some("26.0".into()),
                short_version: Some("1.2".into()),
                build_number: Some("42".into()),
                installed_at: "2026-08-30T00:00:00Z".into(),
                verified: true,
            }],
            latest_evolution_id: None,
            latest_evolution_status: None,
            last_successful_connection: Some("2026-08-30T00:00:00Z".into()),
            last_successful_installation: Some("2026-08-30T00:00:00Z".into()),
            recovery: Some("Relink this source if it moves.".into()),
            created_at: "2026-08-30T00:00:00Z".into(),
            updated_at: "2026-08-30T00:00:00Z".into(),
        };
        write_new(&path, &record).unwrap();
        let reopened: LivingProjectRecord = read_json(&path).unwrap();
        validate_project(&reopened).unwrap();
        assert_eq!(reopened, record);
        assert!(reopened.installations[0].verified);

        let parent_shot = format!("0x{}", "22".repeat(32));
        let parent_release = format!("0x{}", "33".repeat(32));
        let origin = NetworkProjectOrigin {
            kind: NetworkImportKind::Fork,
            parent_shot_id: parent_shot.clone(),
            parent_release_digest: parent_release.clone(),
            source_artifact_sha256: format!("0x{}", "44".repeat(32)),
            builder_id: format!("eip155:4663:0x{}", "55".repeat(20)),
            verified_at: "2026-08-30T00:00:00Z".into(),
        };
        let mut fork = record.clone();
        fork.candidate_shot_id = Some("66".repeat(32));
        fork.network_origin = Some(origin.clone());
        validate_project(&fork).unwrap();
        assert_ne!(
            fork.candidate_shot_id.as_deref(),
            Some(parent_shot.trim_start_matches("0x"))
        );

        let mut install = fork;
        install.candidate_shot_id = None;
        install.network_origin = Some(NetworkProjectOrigin {
            kind: NetworkImportKind::Install,
            ..origin
        });
        validate_project(&install).unwrap();
        assert_eq!(
            install
                .network_origin
                .as_ref()
                .unwrap()
                .parent_release_digest,
            parent_release
        );
    }
}
