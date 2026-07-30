use crate::fs::{create_new_atomic, ensure_real_directory, read_regular_limited};
use crate::model::{
    ActionReference, ActionValidation, AuthorityStatus, Health, IngestOutcome, IntegrityIssue,
    IntegrityReport, MissingParentReference, NodeIdentity, NodeInfo, SegmentStatus, ShotSummary,
    ShotView, SignedRecordStatus, ValidationCounts,
};
use crate::protocol_adapter::{parse_public, ValidatedAction};
use crate::{NodeError, Result, LINEAGE_PROTOCOL, LINEAGE_PROTOCOL_VERSION, NODE_PROTOCOL};
use rand_core::{OsRng, RngCore};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tohseno_protocol::canonical;
use tohseno_protocol::digest::{Bytes32, ShotId};
use tohseno_protocol::lineage::{
    reduce_lineage, verify_lineage_segment, SignedLineageAction, LINEAGE_SCHEMA_VERSION,
};

pub const MAX_ACTION_BYTES: usize = 256 * 1024;
pub const MAX_ACTIONS_PER_NODE: usize = 100_000;
pub const MAX_ACTIONS_PER_SHOT: usize = 10_000;
const MAX_IDENTITY_BYTES: usize = 4096;
const IDENTITY_SCHEMA: &str = "tohseno.node-identity/1";
const MAX_INTEGRITY_PARENT_FINDINGS: usize = 1024;
const GENERATION_POLICY: &str = "inactive: public candidate authority remains unresolved until a release-authorized contract generation is activated and independently verified";
const LEGACY_POLICY: &str = "v0.7 CREATE2 prediction is a retired offline-verification helper only; generic signed lineage remains neutral legacy evidence and can never establish active public candidate authority";

#[derive(Clone, Debug, Eq, PartialEq)]
struct ActionMeta {
    reference: ActionReference,
    path: PathBuf,
    signed: SignedLineageAction,
    missing_artifacts: Vec<crate::model::MissingArtifact>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ShotIndex {
    actions: BTreeMap<(u64, Bytes32), ActionMeta>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Index {
    actions: BTreeMap<Bytes32, ActionMeta>,
    shots: BTreeMap<ShotId, ShotIndex>,
    children: BTreeMap<Bytes32, BTreeSet<Bytes32>>,
    issues: Vec<IntegrityIssue>,
    checked_files: usize,
}

pub struct NodeStore {
    root: PathBuf,
    actions_root: PathBuf,
    identity: NodeIdentity,
    index: RwLock<Index>,
    mutation: Mutex<()>,
}

impl NodeStore {
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = ensure_real_directory(root.as_ref())?;
        let actions_root = ensure_real_directory(&root.join("actions"))?;
        let identity = load_or_create_identity(&root)?;
        let mut store = Self {
            root,
            actions_root,
            identity,
            index: RwLock::new(Index::default()),
            mutation: Mutex::new(()),
        };
        let scanned = store.scan_index()?;
        store.index = RwLock::new(scanned);
        Ok(store)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn identity(&self) -> &NodeIdentity {
        &self.identity
    }

    pub fn ingest_file(&self, path: impl AsRef<Path>) -> Result<IngestOutcome> {
        let bytes = read_regular_limited(path.as_ref(), MAX_ACTION_BYTES)?;
        let parsed = parse_public(&bytes)?;
        if parsed.canonical_bytes != bytes {
            return Err(NodeError::Protocol(
                "signed action file must contain canonical JSON bytes exactly".into(),
            ));
        }
        self.ingest(&bytes)
    }

    pub fn ingest(&self, bytes: &[u8]) -> Result<IngestOutcome> {
        let validated = parse_public(bytes)?;
        let digest = validated.digest;
        let shot_id = validated.shot_id;
        let sequence = validated.sequence;
        let _guard = self.mutation.lock().map_err(|_| NodeError::LockPoisoned)?;
        let mut index = self.index.write().map_err(|_| NodeError::LockPoisoned)?;
        if let Some(existing) = index.actions.get(&digest) {
            let stored = read_regular_limited(&existing.path, MAX_ACTION_BYTES)?;
            if stored != validated.canonical_bytes {
                return Err(NodeError::ContentCollision);
            }
            return Ok(IngestOutcome {
                digest,
                stored: false,
                shot_id,
                sequence,
                validation: existing.reference.validation.clone(),
            });
        }
        if index.actions.len() >= MAX_ACTIONS_PER_NODE && !index.actions.contains_key(&digest) {
            return Err(NodeError::ActionLimit {
                limit: MAX_ACTIONS_PER_NODE,
            });
        }
        let shot_count = index
            .shots
            .get(&validated.shot_id)
            .map(|shot| shot.actions.len())
            .unwrap_or(0);
        if shot_count >= MAX_ACTIONS_PER_SHOT && !index.actions.contains_key(&digest) {
            return Err(NodeError::ActionLimit {
                limit: MAX_ACTIONS_PER_SHOT,
            });
        }

        let validation = self.classify_candidate(&index, &validated);
        reject_known_invalid(&validation)?;
        let path = action_path(&self.actions_root, digest);
        let created = create_new_atomic(&path, &validated.canonical_bytes)?;
        if !created {
            let existing = read_regular_limited(&path, MAX_ACTION_BYTES)?;
            if existing != validated.canonical_bytes {
                return Err(NodeError::ContentCollision);
            }
        }
        add_stored(&mut index, validated, path, validation);
        self.reclassify_descendants(&mut index, digest);
        let validation = index
            .actions
            .get(&digest)
            .expect("newly added action is indexed")
            .reference
            .validation
            .clone();
        Ok(IngestOutcome {
            digest,
            stored: created,
            shot_id,
            sequence,
            validation,
        })
    }

    pub fn contains(&self, digest: Bytes32) -> Result<bool> {
        Ok(self
            .index
            .read()
            .map_err(|_| NodeError::LockPoisoned)?
            .actions
            .contains_key(&digest))
    }

    pub fn action_bytes(&self, digest: Bytes32) -> Result<Vec<u8>> {
        let path = {
            let index = self.index.read().map_err(|_| NodeError::LockPoisoned)?;
            index
                .actions
                .get(&digest)
                .map(|meta| meta.path.clone())
                .ok_or_else(|| NodeError::ActionMissing(digest.to_string()))?
        };
        let bytes = read_regular_limited(&path, MAX_ACTION_BYTES)?;
        let validated = parse_public(&bytes)?;
        if validated.digest != digest || validated.canonical_bytes != bytes {
            return Err(NodeError::ContentCollision);
        }
        Ok(bytes)
    }

    pub fn shots(&self) -> Result<Vec<ShotSummary>> {
        let index = self.index.read().map_err(|_| NodeError::LockPoisoned)?;
        Ok(index
            .shots
            .iter()
            .map(|(shot_id, shot)| summarize(*shot_id, shot))
            .collect())
    }

    pub fn shot(&self, shot_id: ShotId) -> Result<ShotView> {
        let index = self.index.read().map_err(|_| NodeError::LockPoisoned)?;
        let shot = index
            .shots
            .get(&shot_id)
            .ok_or_else(|| NodeError::ShotMissing(shot_id.to_string()))?;
        Ok(ShotView {
            shot: summarize(shot_id, shot),
            actions: shot
                .actions
                .values()
                .map(|meta| meta.reference.clone())
                .collect(),
        })
    }

    pub fn health(&self) -> Result<Health> {
        let index = self.index.read().map_err(|_| NodeError::LockPoisoned)?;
        Ok(Health {
            status: if index.issues.is_empty() {
                "ok"
            } else {
                "degraded"
            },
            node_id: self.identity.node_id,
            stored_actions: index.actions.len(),
            indexed_shots: index.shots.len(),
            integrity_ok: index.issues.is_empty(),
        })
    }

    pub fn info(&self) -> Result<NodeInfo> {
        let index = self.index.read().map_err(|_| NodeError::LockPoisoned)?;
        Ok(NodeInfo {
            schema: NODE_PROTOCOL,
            node_id: self.identity.node_id,
            created_at_unix: self.identity.created_at_unix,
            lineage_protocol: LINEAGE_PROTOCOL,
            lineage_protocol_version: LINEAGE_PROTOCOL_VERSION,
            supported_schema_versions: vec![LINEAGE_SCHEMA_VERSION],
            stored_actions: index.actions.len(),
            indexed_shots: index.shots.len(),
            active_generation: None,
            generation_policy: GENERATION_POLICY,
            legacy_policy: LEGACY_POLICY,
            agreement: "deterministic signed-record, available-segment, and neutral reducer validity for locally possessed ordinary lineage",
            non_agreement:
                "no active contract generation, public candidate authority, public-checkpoint inventory, universal head, artifact completeness, subjective coherence, or global consensus",
        })
    }

    /// Rebuilds every derived index from append-only action files.
    pub fn rebuild(&self) -> Result<IntegrityReport> {
        let _guard = self.mutation.lock().map_err(|_| NodeError::LockPoisoned)?;
        let scanned = self.scan_index()?;
        let report = report_for(&scanned, true);
        *self.index.write().map_err(|_| NodeError::LockPoisoned)? = scanned;
        Ok(report)
    }

    /// Revalidates disk independently and compares it with the live cache.
    pub fn integrity(&self) -> Result<IntegrityReport> {
        let _guard = self.mutation.lock().map_err(|_| NodeError::LockPoisoned)?;
        let scanned = self.scan_index()?;
        let current = self.index.read().map_err(|_| NodeError::LockPoisoned)?;
        let matches = current.actions == scanned.actions
            && current.shots == scanned.shots
            && current.children == scanned.children;
        Ok(report_for(&scanned, matches))
    }

    fn scan_index(&self) -> Result<Index> {
        let (paths, mut issues) = collect_action_paths(&self.actions_root)?;
        if paths.len() > MAX_ACTIONS_PER_NODE {
            return Err(NodeError::ActionLimit {
                limit: MAX_ACTIONS_PER_NODE,
            });
        }
        let checked_files = paths.len();
        let mut candidates = Vec::new();
        for path in paths {
            match read_regular_limited(&path, MAX_ACTION_BYTES)
                .and_then(|bytes| parse_public(&bytes).map(|action| (bytes, action)))
            {
                Ok((bytes, action)) => {
                    let expected = action_path(&self.actions_root, action.digest);
                    if expected != path {
                        issues.push(issue(
                            &path,
                            "content address does not match the action commitment",
                        ));
                    } else if bytes != action.canonical_bytes {
                        issues.push(issue(&path, "stored action is not canonical JSON"));
                    } else {
                        candidates.push((path, action));
                    }
                }
                Err(error) => issues.push(issue(&path, &error.to_string())),
            }
        }
        candidates.sort_by_key(|(_, action)| (action.sequence, action.digest));
        let mut index = Index {
            checked_files,
            issues,
            ..Index::default()
        };
        for (path, action) in candidates {
            if index.actions.contains_key(&action.digest) {
                index
                    .issues
                    .push(issue(&path, "duplicate content-addressed action"));
                continue;
            }
            if index
                .shots
                .get(&action.shot_id)
                .is_some_and(|shot| shot.actions.len() >= MAX_ACTIONS_PER_SHOT)
            {
                index.issues.push(issue(
                    &path,
                    &format!("Shot exceeds the {MAX_ACTIONS_PER_SHOT}-action limit"),
                ));
                continue;
            }
            let initial = unresolved_validation(
                action.previous,
                "authority context has not yet been derived from stored lineage",
            );
            add_stored(&mut index, action, path, initial);
        }
        let ordered = index
            .actions
            .values()
            .map(|meta| {
                (
                    meta.reference.sequence,
                    meta.reference.digest,
                    meta.reference.shot_id,
                )
            })
            .collect::<Vec<_>>();
        for (_, digest, shot_id) in ordered {
            let validation = self.classify_stored(&index, digest);
            update_validation(&mut index, shot_id, digest, validation);
        }
        Ok(index)
    }

    fn classify_candidate(&self, index: &Index, candidate: &ValidatedAction) -> ActionValidation {
        self.classify(
            index,
            candidate.digest,
            candidate.signed.clone(),
            candidate.previous,
        )
    }

    fn classify_stored(&self, index: &Index, digest: Bytes32) -> ActionValidation {
        let Some(candidate) = index.actions.get(&digest) else {
            return rejected_validation(
                false,
                None,
                "action disappeared while its derived index was rebuilt",
            );
        };
        self.classify(
            index,
            digest,
            candidate.signed.clone(),
            candidate.reference.previous,
        )
    }

    fn classify(
        &self,
        index: &Index,
        candidate_digest: Bytes32,
        candidate: SignedLineageAction,
        candidate_previous: Option<Bytes32>,
    ) -> ActionValidation {
        let mut branch = vec![candidate];
        let mut cursor = candidate_previous;
        let mut seen = HashSet::new();
        seen.insert(candidate_digest);
        let mut missing_parent = None;
        while let Some(digest) = cursor {
            if !seen.insert(digest) {
                return rejected_validation(false, None, "cycle in previous-action links");
            }
            if branch.len() >= MAX_ACTIONS_PER_SHOT {
                return rejected_validation(
                    false,
                    None,
                    "available lineage segment exceeds the per-Shot action limit",
                );
            }
            let Some(parent) = index.actions.get(&digest) else {
                missing_parent = Some(digest);
                break;
            };
            cursor = parent.reference.previous;
            branch.push(parent.signed.clone());
        }
        branch.reverse();
        let segment = match verify_lineage_segment(&branch, None) {
            Ok(segment) => segment,
            Err(error) => {
                return rejected_validation(
                    missing_parent.is_none()
                        && complete_commitment_shape(branch.first().expect("nonempty")),
                    missing_parent,
                    &format!("available segment is invalid: {error}"),
                )
            }
        };

        if let Some(parent) = missing_parent {
            let first = branch.first().expect("nonempty");
            if first.action.sequence == 1 {
                return rejected_validation(
                    false,
                    Some(parent),
                    "sequence 1 cannot have an unavailable predecessor",
                );
            }
            return unresolved_validation(
                Some(parent),
                &format!(
                    "signed record and available segment verify, but authority is unresolved until parent {parent} is available"
                ),
            );
        }

        match reduce_lineage(&branch) {
            Ok(_) => inactive_generation_validation(segment.authority_context_available),
            Err(error) => ActionValidation {
                signed_record: SignedRecordStatus::Verified,
                segment: SegmentStatus::Verified,
                neutral_authority: AuthorityStatus::Rejected,
                candidate_authority: AuthorityStatus::Rejected,
                authority_context_available: segment.authority_context_available,
                missing_parent: None,
                detail: Some(format!("complete prefix fails neutral reduction: {error}")),
            },
        }
    }

    fn reclassify_descendants(&self, index: &mut Index, root: Bytes32) {
        let mut pending = BTreeSet::from([root]);
        while let Some(digest) = pending.pop_first() {
            let Some(meta) = index.actions.get(&digest) else {
                continue;
            };
            let shot_id = meta.reference.shot_id;
            let children = index.children.get(&digest).cloned().unwrap_or_default();
            let validation = self.classify_stored(index, digest);
            update_validation(index, shot_id, digest, validation);
            pending.extend(children);
        }
    }
}

fn inactive_generation_validation(authority_context_available: bool) -> ActionValidation {
    ActionValidation {
        signed_record: SignedRecordStatus::Verified,
        segment: SegmentStatus::Verified,
        neutral_authority: AuthorityStatus::Verified,
        candidate_authority: AuthorityStatus::Unresolved,
        authority_context_available,
        missing_parent: None,
        detail: Some(
            "complete lineage is neutrally valid, but candidate authority is unresolved because this node has no active release-authorized contract generation; retired v0.7 CREATE2 predictions are offline evidence only"
                .into(),
        ),
    }
}

fn unresolved_validation(missing_parent: Option<Bytes32>, detail: &str) -> ActionValidation {
    ActionValidation {
        signed_record: SignedRecordStatus::Verified,
        segment: SegmentStatus::Verified,
        neutral_authority: AuthorityStatus::Unresolved,
        candidate_authority: AuthorityStatus::Unresolved,
        authority_context_available: false,
        missing_parent,
        detail: Some(detail.into()),
    }
}

fn rejected_validation(
    authority_context_available: bool,
    missing_parent: Option<Bytes32>,
    detail: &str,
) -> ActionValidation {
    ActionValidation {
        signed_record: SignedRecordStatus::Verified,
        segment: SegmentStatus::Rejected,
        neutral_authority: AuthorityStatus::Rejected,
        candidate_authority: AuthorityStatus::Rejected,
        authority_context_available,
        missing_parent,
        detail: Some(detail.into()),
    }
}

fn complete_commitment_shape(action: &SignedLineageAction) -> bool {
    action.action.sequence == 1
        && action.action.previous.is_none()
        && matches!(
            action.action.payload,
            tohseno_protocol::lineage::LineagePayload::Commitment(_)
        )
}

fn reject_known_invalid(validation: &ActionValidation) -> Result<()> {
    if validation.segment == SegmentStatus::Rejected
        || validation.candidate_authority == AuthorityStatus::Rejected
    {
        return Err(NodeError::Causal(
            validation
                .detail
                .clone()
                .unwrap_or_else(|| "lineage candidate was rejected".into()),
        ));
    }
    Ok(())
}

fn load_or_create_identity(root: &Path) -> Result<NodeIdentity> {
    let path = root.join("node.json");
    match fs::symlink_metadata(&path) {
        Ok(_) => {
            let bytes = read_regular_limited(&path, MAX_IDENTITY_BYTES)?;
            let identity: NodeIdentity = canonical::from_slice(&bytes)?;
            validate_identity(&identity)?;
            if canonical::to_vec(&identity)? != bytes {
                return Err(NodeError::UnsafeStorage(path));
            }
            Ok(identity)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut random = [0_u8; 32];
            OsRng.fill_bytes(&mut random);
            let identity = NodeIdentity {
                schema: IDENTITY_SCHEMA.into(),
                node_id: Bytes32::new(random),
                created_at_unix: now_unix(),
            };
            validate_identity(&identity)?;
            let bytes = canonical::to_vec(&identity)?;
            if create_new_atomic(&path, &bytes)? {
                Ok(identity)
            } else {
                let bytes = read_regular_limited(&path, MAX_IDENTITY_BYTES)?;
                let identity: NodeIdentity = canonical::from_slice(&bytes)?;
                validate_identity(&identity)?;
                Ok(identity)
            }
        }
        Err(error) => Err(error.into()),
    }
}

fn validate_identity(identity: &NodeIdentity) -> Result<()> {
    if identity.schema != IDENTITY_SCHEMA
        || identity.node_id == Bytes32::ZERO
        || identity.created_at_unix == 0
    {
        return Err(NodeError::Protocol("invalid node identity".into()));
    }
    Ok(())
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .max(1)
}

fn add_stored(
    index: &mut Index,
    action: ValidatedAction,
    path: PathBuf,
    validation: ActionValidation,
) {
    let reference = ActionReference {
        digest: action.digest,
        shot_id: action.shot_id,
        sequence: action.sequence,
        previous: action.previous,
        validation,
    };
    let meta = ActionMeta {
        reference: reference.clone(),
        path,
        signed: action.signed,
        missing_artifacts: action.missing_artifacts,
    };
    if let Some(previous) = action.previous {
        index
            .children
            .entry(previous)
            .or_default()
            .insert(action.digest);
    }
    let shot = index.shots.entry(action.shot_id).or_default();
    shot.actions
        .insert((action.sequence, action.digest), meta.clone());
    index.actions.insert(action.digest, meta);
}

fn update_validation(
    index: &mut Index,
    shot_id: ShotId,
    digest: Bytes32,
    validation: ActionValidation,
) {
    if let Some(meta) = index.actions.get_mut(&digest) {
        meta.reference.validation = validation.clone();
        let key = (meta.reference.sequence, digest);
        if let Some(shot_meta) = index
            .shots
            .get_mut(&shot_id)
            .and_then(|shot| shot.actions.get_mut(&key))
        {
            shot_meta.reference.validation = validation;
        }
    }
}

fn summarize(shot_id: ShotId, shot: &ShotIndex) -> ShotSummary {
    let mut missing = shot
        .actions
        .values()
        .flat_map(|meta| {
            meta.missing_artifacts.iter().cloned().map(|mut missing| {
                missing.candidate_authority = meta.reference.validation.candidate_authority;
                missing
            })
        })
        .collect::<Vec<_>>();
    missing.sort_by(|left, right| {
        (
            left.digest,
            &left.declared_status,
            &left.reason,
            left.observed_in_action,
        )
            .cmp(&(
                right.digest,
                &right.declared_status,
                &right.reason,
                right.observed_in_action,
            ))
    });
    missing.dedup();
    let roots = shot
        .actions
        .values()
        .filter(|meta| meta.reference.sequence == 1 && meta.reference.previous.is_none())
        .map(|meta| meta.reference.digest)
        .collect();
    let observed_heads = heads_for(shot, |_| true);
    let authority_verified_heads = heads_for(shot, |reference| {
        reference.validation.candidate_authority == AuthorityStatus::Verified
    });
    let authority_unresolved_heads = heads_for(shot, |reference| {
        reference.validation.candidate_authority == AuthorityStatus::Unresolved
    });
    let authority_rejected_heads = heads_for(shot, |reference| {
        reference.validation.candidate_authority == AuthorityStatus::Rejected
    });
    let validation = validation_counts(shot.actions.values().map(|meta| &meta.reference));
    let missing_parents = shot
        .actions
        .values()
        .filter_map(|meta| meta.reference.validation.missing_parent)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    ShotSummary {
        shot_id,
        action_count: shot.actions.len(),
        roots,
        observed_heads,
        authority_verified_heads,
        authority_unresolved_heads,
        authority_rejected_heads,
        validation,
        missing_parents,
        missing_artifacts: missing,
    }
}

fn heads_for(shot: &ShotIndex, include: impl Fn(&ActionReference) -> bool) -> Vec<Bytes32> {
    let included = shot
        .actions
        .values()
        .filter(|meta| include(&meta.reference))
        .map(|meta| meta.reference.digest)
        .collect::<BTreeSet<_>>();
    let parents_with_included_children = shot
        .actions
        .values()
        .filter(|meta| include(&meta.reference))
        .filter_map(|meta| meta.reference.previous)
        .filter(|parent| included.contains(parent))
        .collect::<BTreeSet<_>>();
    included
        .difference(&parents_with_included_children)
        .copied()
        .collect()
}

fn validation_counts<'a>(
    references: impl IntoIterator<Item = &'a ActionReference>,
) -> ValidationCounts {
    let mut counts = ValidationCounts::default();
    for reference in references {
        counts.signed_records_verified +=
            usize::from(reference.validation.signed_record == SignedRecordStatus::Verified);
        match reference.validation.segment {
            SegmentStatus::Verified => counts.segments_verified += 1,
            SegmentStatus::Rejected => counts.segments_rejected += 1,
        }
        match reference.validation.neutral_authority {
            AuthorityStatus::Verified => counts.neutral_authority_verified += 1,
            AuthorityStatus::Unresolved => counts.neutral_authority_unresolved += 1,
            AuthorityStatus::Rejected => counts.neutral_authority_rejected += 1,
        }
        match reference.validation.candidate_authority {
            AuthorityStatus::Verified => counts.candidate_authority_verified += 1,
            AuthorityStatus::Unresolved => counts.candidate_authority_unresolved += 1,
            AuthorityStatus::Rejected => counts.candidate_authority_rejected += 1,
        }
    }
    counts
}

fn action_path(root: &Path, digest: Bytes32) -> PathBuf {
    let hex = digest.to_hex();
    root.join(&hex[2..4]).join(format!("{}.json", &hex[2..]))
}

fn collect_action_paths(root: &Path) -> Result<(Vec<PathBuf>, Vec<IntegrityIssue>)> {
    ensure_real_directory(root)?;
    let mut paths = Vec::new();
    let mut issues = Vec::new();
    let mut buckets = fs::read_dir(root)?.collect::<std::io::Result<Vec<_>>>()?;
    buckets.sort_by_key(|entry| entry.file_name());
    for bucket in buckets {
        let name = bucket.file_name();
        let name = name.to_string_lossy();
        let kind = bucket.file_type()?;
        if !kind.is_dir()
            || name.len() != 2
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            issues.push(issue(
                &bucket.path(),
                "unexpected non-bucket entry in action store",
            ));
            continue;
        }
        let mut entries = fs::read_dir(bucket.path())?.collect::<std::io::Result<Vec<_>>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            if entry.file_type()?.is_file() {
                paths.push(entry.path());
            } else {
                issues.push(issue(
                    &entry.path(),
                    "action-store entries must be regular files",
                ));
            }
        }
    }
    Ok((paths, issues))
}

fn issue(path: &Path, detail: &str) -> IntegrityIssue {
    IntegrityIssue {
        path: path.display().to_string(),
        detail: detail.into(),
    }
}

fn report_for(index: &Index, memory_matches_disk: bool) -> IntegrityReport {
    let validation = validation_counts(index.actions.values().map(|meta| &meta.reference));
    let mut missing_parents = index
        .actions
        .values()
        .filter_map(|meta| {
            meta.reference
                .validation
                .missing_parent
                .map(|missing_parent| MissingParentReference {
                    shot_id: meta.reference.shot_id,
                    action_digest: meta.reference.digest,
                    missing_parent,
                })
        })
        .collect::<Vec<_>>();
    missing_parents.sort_by_key(|missing| {
        (
            missing.shot_id,
            missing.action_digest,
            missing.missing_parent,
        )
    });
    let missing_parent_count = missing_parents.len();
    let findings_truncated = missing_parents.len() > MAX_INTEGRITY_PARENT_FINDINGS;
    missing_parents.truncate(MAX_INTEGRITY_PARENT_FINDINGS);
    IntegrityReport {
        ok: index.issues.is_empty() && memory_matches_disk,
        checked_actions: index.checked_files,
        valid_actions: validation.segments_verified,
        indexed_actions: index.actions.len(),
        indexed_shots: index.shots.len(),
        validation,
        missing_parent_count,
        missing_parents,
        findings_truncated,
        memory_matches_disk,
        issues: index.issues.clone(),
    }
}
