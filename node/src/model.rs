use serde::{Deserialize, Serialize};
use tohseno_protocol::digest::{Address20, Bytes32, ShotId};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeIdentity {
    pub schema: String,
    pub node_id: Bytes32,
    pub created_at_unix: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Health {
    pub status: &'static str,
    pub node_id: Bytes32,
    pub stored_actions: usize,
    pub indexed_shots: usize,
    pub integrity_ok: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NodeInfo {
    pub schema: &'static str,
    pub node_id: Bytes32,
    pub created_at_unix: u64,
    pub lineage_protocol: &'static str,
    pub lineage_protocol_version: &'static str,
    pub supported_schema_versions: Vec<u32>,
    pub stored_actions: usize,
    pub indexed_shots: usize,
    pub contract_configuration: CandidateContractConfiguration,
    pub agreement: &'static str,
    pub non_agreement: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateContractConfiguration {
    pub schema: String,
    pub candidate_version: String,
    pub candidate_status: String,
    pub chain_name: String,
    pub chain_id: u64,
    pub p256verify: Address20,
    pub create2_deployer: Address20,
    pub deployer_code_must_be_verified_before_broadcast: bool,
    pub builder_account_factory: PlannedContract,
    pub shot_registry: PlannedContract,
    pub shot_relations: PlannedContract,
    pub builder_account_creation_bytecode_sha256: Bytes32,
    pub initial_authority_policy: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedContract {
    pub deployment_order: u8,
    pub planned_address: Address20,
    pub salt: Bytes32,
    pub init_code_hash: Bytes32,
    pub deployed: bool,
    pub runtime_code_hash: Option<Bytes32>,
    pub transaction_hash: Option<Bytes32>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PeerDescription {
    pub url: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignedRecordStatus {
    Verified,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SegmentStatus {
    Verified,
    Rejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorityStatus {
    Verified,
    Unresolved,
    Rejected,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionValidation {
    /// Closed schema, payload digest, commitment, and action signature.
    pub signed_record: SignedRecordStatus,
    /// Cryptographic and causal validity of the locally available contiguous
    /// segment. An unavailable boundary is reported separately.
    pub segment: SegmentStatus,
    /// Neutral reducer authority, which trusts the commitment's declared
    /// controller/key binding when a complete prefix is available.
    pub neutral_authority: AuthorityStatus,
    /// Candidate policy authority. GENESIS verifies the initial pinned
    /// BuilderAccount factory/salt/creation-bytecode prediction, but does not
    /// yet define an ownership-transfer authorization proof.
    pub candidate_authority: AuthorityStatus,
    pub authority_context_available: bool,
    pub missing_parent: Option<Bytes32>,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionReference {
    pub digest: Bytes32,
    pub shot_id: ShotId,
    pub sequence: u64,
    pub previous: Option<Bytes32>,
    pub validation: ActionValidation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MissingArtifact {
    pub digest: Bytes32,
    pub declared_status: String,
    pub reason: String,
    pub observed_in_action: Bytes32,
    pub candidate_authority: AuthorityStatus,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShotSummary {
    pub shot_id: ShotId,
    pub action_count: usize,
    pub roots: Vec<Bytes32>,
    pub observed_heads: Vec<Bytes32>,
    pub authority_verified_heads: Vec<Bytes32>,
    pub authority_unresolved_heads: Vec<Bytes32>,
    pub authority_rejected_heads: Vec<Bytes32>,
    pub validation: ValidationCounts,
    pub missing_parents: Vec<Bytes32>,
    pub missing_artifacts: Vec<MissingArtifact>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShotView {
    pub shot: ShotSummary,
    pub actions: Vec<ActionReference>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct IngestOutcome {
    pub digest: Bytes32,
    pub stored: bool,
    pub shot_id: ShotId,
    pub sequence: u64,
    pub validation: ActionValidation,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationCounts {
    pub signed_records_verified: usize,
    pub segments_verified: usize,
    pub segments_rejected: usize,
    pub neutral_authority_verified: usize,
    pub neutral_authority_unresolved: usize,
    pub neutral_authority_rejected: usize,
    pub candidate_authority_verified: usize,
    pub candidate_authority_unresolved: usize,
    pub candidate_authority_rejected: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MissingParentReference {
    pub shot_id: ShotId,
    pub action_digest: Bytes32,
    pub missing_parent: Bytes32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct IntegrityIssue {
    pub path: String,
    pub detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct IntegrityReport {
    pub ok: bool,
    pub checked_actions: usize,
    pub valid_actions: usize,
    pub indexed_actions: usize,
    pub indexed_shots: usize,
    pub validation: ValidationCounts,
    pub missing_parent_count: usize,
    pub missing_parents: Vec<MissingParentReference>,
    pub findings_truncated: bool,
    pub memory_matches_disk: bool,
    pub issues: Vec<IntegrityIssue>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncState {
    Never,
    Running,
    Succeeded,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PeerSyncResult {
    pub peer: String,
    pub fetched: usize,
    pub already_present: usize,
    pub rejected: usize,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SyncReport {
    pub state: SyncState,
    pub started_at_unix: Option<u64>,
    pub completed_at_unix: Option<u64>,
    pub peers: Vec<PeerSyncResult>,
}

impl Default for SyncReport {
    fn default() -> Self {
        Self {
            state: SyncState::Never,
            started_at_unix: None,
            completed_at_unix: None,
            peers: Vec::new(),
        }
    }
}
