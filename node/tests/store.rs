mod common;

use common::{
    availability_action, availability_after, availability_after_with_handling, bytes,
    ownership_action, root_action, TestKey,
};
use std::fs;
use tohseno_node::{AuthorityStatus, NodeError, NodeStore, SegmentStatus};
use tohseno_protocol::canonical;
use tohseno_protocol::digest::{Address20, Bytes32, ShotId};
use tohseno_protocol::identity::BuilderId;
use tohseno_protocol::lineage::{reduce_lineage, LineageAction, LineagePayload};
use tohseno_protocol::ontology::{AvailabilityStatus, ShotCommitment};

#[test]
fn identity_is_random_persistent_and_not_derived_from_storage_path() {
    let first_root = tempfile::tempdir().unwrap();
    let second_root = tempfile::tempdir().unwrap();
    let first = NodeStore::open(first_root.path()).unwrap();
    let first_id = first.identity().node_id;
    drop(first);
    assert_eq!(
        NodeStore::open(first_root.path())
            .unwrap()
            .identity()
            .node_id,
        first_id
    );
    assert_ne!(
        NodeStore::open(second_root.path())
            .unwrap()
            .identity()
            .node_id,
        first_id
    );
}

#[test]
fn rejects_private_and_tampered_but_preserves_an_unanchored_public_record() {
    let temporary = tempfile::tempdir().unwrap();
    let store = NodeStore::open(temporary.path()).unwrap();
    let key = TestKey::new(1);
    let shot_id = ShotId::from_bytes([0x11; 32]);

    let private = root_action(&key, shot_id, AvailabilityStatus::IntentionallyPrivate);
    assert!(matches!(
        store.ingest(&bytes(&private)),
        Err(NodeError::NotPublic)
    ));

    let root = root_action(&key, shot_id, AvailabilityStatus::PubliclyAvailable);
    let mut value: serde_json::Value = serde_json::from_slice(&bytes(&root)).unwrap();
    value["action"]["payload_digest"] =
        serde_json::Value::String(Bytes32::new([0xee; 32]).to_string());
    let tampered = canonical::to_vec(&value).unwrap();
    assert!(matches!(
        store.ingest(&tampered),
        Err(NodeError::Protocol(_))
    ));

    let missing_parent = key.sign(
        LineageAction::new(
            2,
            Some(Bytes32::new([0xaa; 32])),
            shot_id,
            key.builder,
            common::timestamp(2),
            AvailabilityStatus::PubliclyAvailable,
            LineagePayload::ArtifactAvailability(
                tohseno_protocol::ontology::ArtifactAvailabilityRecord {
                    target_role: "source".into(),
                    availability: tohseno_protocol::ontology::ArtifactAvailability::new(
                        tohseno_protocol::ontology::ArtifactDescriptor {
                            digest: Bytes32::new([0xbb; 32]),
                            media_type: "application/octet-stream".into(),
                            byte_length: 1,
                            name: None,
                        },
                        AvailabilityStatus::Unknown,
                    ),
                    observed_at: common::timestamp(2),
                },
            ),
        )
        .unwrap(),
    );
    let outcome = store.ingest(&bytes(&missing_parent)).unwrap();
    assert_eq!(
        outcome.validation.candidate_authority,
        AuthorityStatus::Unresolved
    );
    assert_eq!(
        outcome.validation.missing_parent,
        Some(Bytes32::new([0xaa; 32]))
    );
    assert_eq!(store.health().unwrap().stored_actions, 1);
}

#[test]
fn partial_artifacts_and_multiple_observed_heads_are_honest() {
    let temporary = tempfile::tempdir().unwrap();
    let store = NodeStore::open(temporary.path()).unwrap();
    let key = TestKey::new(2);
    let shot_id = ShotId::from_bytes([0x22; 32]);
    let root = root_action(&key, shot_id, AvailabilityStatus::PubliclyAvailable);
    store.ingest(&bytes(&root)).unwrap();

    let absent = availability_action(
        &key,
        &root,
        b"not here",
        AvailabilityStatus::Absent,
        "source archive",
    );
    let unknown = availability_action(
        &key,
        &root,
        b"also not here",
        AvailabilityStatus::Unknown,
        "reference image",
    );
    store.ingest(&bytes(&absent)).unwrap();
    store.ingest(&bytes(&unknown)).unwrap();

    let view = store.shot(shot_id).unwrap();
    assert_eq!(view.shot.roots.len(), 1);
    assert_eq!(view.shot.observed_heads.len(), 2);
    assert_eq!(view.shot.missing_artifacts.len(), 2);
    assert_eq!(view.actions.len(), 3);
}

#[test]
fn rebuild_is_deterministic_and_integrity_detects_disk_tampering() {
    let temporary = tempfile::tempdir().unwrap();
    let key = TestKey::new(3);
    let shot_id = ShotId::from_bytes([0x33; 32]);
    let root = root_action(&key, shot_id, AvailabilityStatus::PubliclyAvailable);
    let digest = root.commitment().unwrap();
    {
        let store = NodeStore::open(temporary.path()).unwrap();
        store.ingest(&bytes(&root)).unwrap();
        assert!(store.integrity().unwrap().ok);
    }
    let reopened = NodeStore::open(temporary.path()).unwrap();
    assert_eq!(reopened.shot(shot_id).unwrap().actions.len(), 1);
    assert!(reopened.rebuild().unwrap().ok);

    let hex = digest.to_hex();
    let path = temporary
        .path()
        .join("actions")
        .join(&hex[2..4])
        .join(format!("{}.json", &hex[2..]));
    let mut tampered = fs::read(&path).unwrap();
    tampered[0] ^= 1;
    fs::write(path, tampered).unwrap();
    let report = reopened.integrity().unwrap();
    assert!(!report.ok);
    assert!(!report.issues.is_empty());
}

#[test]
fn a_second_commitment_with_same_shot_is_an_observed_branch_not_a_winner() {
    let temporary = tempfile::tempdir().unwrap();
    let store = NodeStore::open(temporary.path()).unwrap();
    let first = TestKey::new(4);
    let second = TestKey::new(5);
    let shot_id = ShotId::from_bytes([0x44; 32]);
    let root_a = root_action(&first, shot_id, AvailabilityStatus::PubliclyAvailable);
    let root_b = second.sign(
        LineageAction::new(
            1,
            None,
            shot_id,
            second.builder,
            common::timestamp(1),
            AvailabilityStatus::PubliclyAvailable,
            LineagePayload::Commitment(ShotCommitment::new(
                Bytes32::new([0x92; 32]),
                second.builder,
                second.public.clone(),
                common::timestamp(1),
            )),
        )
        .unwrap(),
    );
    store.ingest(&bytes(&root_a)).unwrap();
    store.ingest(&bytes(&root_b)).unwrap();
    let shot = store.shot(shot_id).unwrap().shot;
    assert_eq!(shot.roots.len(), 2);
    assert_eq!(shot.observed_heads.len(), 2);
}

#[test]
fn public_action_after_an_unavailable_private_parent_is_preserved_without_authority() {
    let temporary = tempfile::tempdir().unwrap();
    let store = NodeStore::open(temporary.path()).unwrap();
    let key = TestKey::new(6);
    let shot_id = ShotId::from_bytes([0x45; 32]);
    let root = root_action(&key, shot_id, AvailabilityStatus::PubliclyAvailable);
    let private_parent = availability_after_with_handling(
        &key,
        &root,
        b"private middle",
        AvailabilityStatus::IntentionallyPrivate,
        "private evidence",
        AvailabilityStatus::IntentionallyPrivate,
    );
    let public_child = availability_after(
        &key,
        &private_parent,
        b"public observation",
        AvailabilityStatus::Unknown,
        "public evidence",
    );
    let private_digest = private_parent.commitment().unwrap();
    let child_digest = public_child.commitment().unwrap();

    store.ingest(&bytes(&root)).unwrap();
    assert!(matches!(
        store.ingest(&bytes(&private_parent)),
        Err(NodeError::NotPublic)
    ));
    let outcome = store.ingest(&bytes(&public_child)).unwrap();
    assert_eq!(outcome.validation.segment, SegmentStatus::Verified);
    assert_eq!(
        outcome.validation.neutral_authority,
        AuthorityStatus::Unresolved
    );
    assert_eq!(
        outcome.validation.candidate_authority,
        AuthorityStatus::Unresolved
    );
    assert!(!outcome.validation.authority_context_available);
    assert_eq!(outcome.validation.missing_parent, Some(private_digest));

    let view = store.shot(shot_id).unwrap();
    let child = view
        .actions
        .iter()
        .find(|action| action.digest == child_digest)
        .unwrap();
    assert_eq!(child.validation.missing_parent, Some(private_digest));
    assert_eq!(view.shot.validation.candidate_authority_verified, 0);
    assert_eq!(view.shot.validation.candidate_authority_unresolved, 2);
    assert_eq!(view.shot.missing_parents, vec![private_digest]);

    let integrity = store.integrity().unwrap();
    assert!(integrity.ok);
    assert_eq!(integrity.missing_parent_count, 1);
    assert_eq!(integrity.missing_parents[0].action_digest, child_digest);
    assert_eq!(integrity.missing_parents[0].missing_parent, private_digest);
}

#[test]
fn late_public_parent_promotes_an_unanchored_segment_deterministically() {
    let temporary = tempfile::tempdir().unwrap();
    let store = NodeStore::open(temporary.path()).unwrap();
    let key = TestKey::new(7);
    let shot_id = ShotId::from_bytes([0x46; 32]);
    let root = root_action(&key, shot_id, AvailabilityStatus::PubliclyAvailable);
    let parent = availability_after(
        &key,
        &root,
        b"parent",
        AvailabilityStatus::Unknown,
        "parent",
    );
    let child = availability_after(
        &key,
        &parent,
        b"child",
        AvailabilityStatus::Unknown,
        "child",
    );
    let parent_digest = parent.commitment().unwrap();
    let child_digest = child.commitment().unwrap();

    store.ingest(&bytes(&root)).unwrap();
    let unanchored = store.ingest(&bytes(&child)).unwrap();
    assert_eq!(
        unanchored.validation.candidate_authority,
        AuthorityStatus::Unresolved
    );
    assert_eq!(unanchored.validation.missing_parent, Some(parent_digest));

    store.ingest(&bytes(&parent)).unwrap();
    let promoted = store
        .shot(shot_id)
        .unwrap()
        .actions
        .into_iter()
        .find(|action| action.digest == child_digest)
        .unwrap();
    assert_eq!(
        promoted.validation.neutral_authority,
        AuthorityStatus::Verified
    );
    assert_eq!(
        promoted.validation.candidate_authority,
        AuthorityStatus::Unresolved
    );
    assert!(promoted
        .validation
        .detail
        .as_deref()
        .unwrap()
        .contains("no active release-authorized contract generation"));
    assert!(promoted.validation.authority_context_available);
    assert_eq!(promoted.validation.missing_parent, None);

    let before = store.shot(shot_id).unwrap();
    store.rebuild().unwrap();
    assert_eq!(store.shot(shot_id).unwrap(), before);
}

#[test]
fn complete_neutral_lineage_stays_candidate_unresolved_without_an_active_generation() {
    let temporary = tempfile::tempdir().unwrap();
    let store = NodeStore::open(temporary.path()).unwrap();
    assert_eq!(store.info().unwrap().active_generation, None);
    let original_owner = TestKey::new(19);
    let next_owner = TestKey::new(20);
    let shot_id = ShotId::from_bytes([0x4d; 32]);
    let root = root_action(
        &original_owner,
        shot_id,
        AvailabilityStatus::PubliclyAvailable,
    );
    let transfer = ownership_action(&original_owner, &next_owner, &root);
    let descendant = availability_after(
        &next_owner,
        &transfer,
        b"post-transfer artifact",
        AvailabilityStatus::Unknown,
        "post-transfer source",
    );

    let root_outcome = store.ingest(&bytes(&root)).unwrap();
    assert_eq!(
        root_outcome.validation.candidate_authority,
        AuthorityStatus::Unresolved
    );
    assert!(root_outcome
        .validation
        .detail
        .as_deref()
        .unwrap()
        .contains("retired v0.7 CREATE2 predictions"));

    for outcome in [
        store.ingest(&bytes(&transfer)).unwrap(),
        store.ingest(&bytes(&descendant)).unwrap(),
    ] {
        assert_eq!(
            outcome.validation.neutral_authority,
            AuthorityStatus::Verified
        );
        assert_eq!(
            outcome.validation.candidate_authority,
            AuthorityStatus::Unresolved
        );
        assert!(outcome.validation.authority_context_available);
        assert_eq!(outcome.validation.missing_parent, None);
        let detail = outcome.validation.detail.unwrap();
        assert!(detail.contains("neutrally valid"));
        assert!(detail.contains("no active release-authorized contract generation"));
    }

    let shot = store.shot(shot_id).unwrap().shot;
    assert_eq!(shot.validation.neutral_authority_verified, 3);
    assert_eq!(shot.validation.candidate_authority_verified, 0);
    assert_eq!(shot.validation.candidate_authority_unresolved, 3);
    assert_eq!(
        shot.authority_unresolved_heads,
        vec![descendant.commitment().unwrap()]
    );
    assert!(shot.authority_verified_heads.is_empty());

    let before = store.shot(shot_id).unwrap();
    store.rebuild().unwrap();
    assert_eq!(store.shot(shot_id).unwrap(), before);
}

#[test]
fn unauthorized_unanchored_segment_is_retained_but_never_gains_authority() {
    let temporary = tempfile::tempdir().unwrap();
    let store = NodeStore::open(temporary.path()).unwrap();
    let owner = TestKey::new(8);
    let attacker = TestKey::new(9);
    let shot_id = ShotId::from_bytes([0x47; 32]);
    let root = root_action(&owner, shot_id, AvailabilityStatus::PubliclyAvailable);
    let parent = availability_after(
        &owner,
        &root,
        b"authorized parent",
        AvailabilityStatus::Unknown,
        "parent",
    );
    let malicious = attacker.sign(
        LineageAction::new(
            3,
            Some(parent.commitment().unwrap()),
            shot_id,
            attacker.builder,
            common::timestamp(3),
            AvailabilityStatus::PubliclyAvailable,
            LineagePayload::ArtifactAvailability(
                tohseno_protocol::ontology::ArtifactAvailabilityRecord {
                    target_role: "forged".into(),
                    availability: tohseno_protocol::ontology::ArtifactAvailability::new(
                        tohseno_protocol::ontology::ArtifactDescriptor {
                            digest: Bytes32::new([0xcc; 32]),
                            media_type: "application/octet-stream".into(),
                            byte_length: 1,
                            name: None,
                        },
                        AvailabilityStatus::Unknown,
                    ),
                    observed_at: common::timestamp(3),
                },
            ),
        )
        .unwrap(),
    );
    let malicious_digest = malicious.commitment().unwrap();

    store.ingest(&bytes(&root)).unwrap();
    assert_eq!(
        store
            .ingest(&bytes(&malicious))
            .unwrap()
            .validation
            .candidate_authority,
        AuthorityStatus::Unresolved
    );
    store.ingest(&bytes(&parent)).unwrap();

    let rejected = store
        .shot(shot_id)
        .unwrap()
        .actions
        .into_iter()
        .find(|action| action.digest == malicious_digest)
        .unwrap();
    assert_eq!(rejected.validation.segment, SegmentStatus::Verified);
    assert_eq!(
        rejected.validation.neutral_authority,
        AuthorityStatus::Rejected
    );
    assert_eq!(
        rejected.validation.candidate_authority,
        AuthorityStatus::Rejected
    );
    assert!(rejected.validation.authority_context_available);
    assert!(store.action_bytes(malicious_digest).is_ok());

    store.rebuild().unwrap();
    let report = store.integrity().unwrap();
    assert!(report.ok);
    assert_eq!(report.validation.candidate_authority_rejected, 1);

    let directly_known = attacker.sign(
        LineageAction::new(
            3,
            Some(parent.commitment().unwrap()),
            shot_id,
            attacker.builder,
            common::timestamp(3),
            AvailabilityStatus::PubliclyAvailable,
            LineagePayload::ArtifactAvailability(
                tohseno_protocol::ontology::ArtifactAvailabilityRecord {
                    target_role: "second-forgery".into(),
                    availability: tohseno_protocol::ontology::ArtifactAvailability::new(
                        tohseno_protocol::ontology::ArtifactDescriptor {
                            digest: Bytes32::new([0xcd; 32]),
                            media_type: "application/octet-stream".into(),
                            byte_length: 1,
                            name: None,
                        },
                        AvailabilityStatus::Unknown,
                    ),
                    observed_at: common::timestamp(3),
                },
            ),
        )
        .unwrap(),
    );
    assert!(matches!(
        store.ingest(&bytes(&directly_known)),
        Err(NodeError::Causal(_))
    ));
}

#[test]
fn inactive_generation_preserves_a_neutral_self_declared_builder_as_unresolved() {
    let temporary = tempfile::tempdir().unwrap();
    let store = NodeStore::open(temporary.path()).unwrap();
    let key = TestKey::new(14);
    let shot_id = ShotId::from_bytes([0x48; 32]);
    let declared = BuilderId::new(Address20::from_bytes([0xfe; 20]));
    let commitment = ShotCommitment::new(
        Bytes32::new([0x91; 32]),
        declared,
        key.public.clone(),
        common::timestamp(1),
    );
    let action = key.sign(
        LineageAction::new(
            1,
            None,
            shot_id,
            declared,
            common::timestamp(1),
            AvailabilityStatus::PubliclyAvailable,
            LineagePayload::Commitment(commitment),
        )
        .unwrap(),
    );

    assert!(reduce_lineage(std::slice::from_ref(&action)).is_ok());
    let outcome = store.ingest(&bytes(&action)).unwrap();
    assert_eq!(
        outcome.validation.neutral_authority,
        AuthorityStatus::Verified
    );
    assert_eq!(
        outcome.validation.candidate_authority,
        AuthorityStatus::Unresolved
    );
    assert!(outcome
        .validation
        .detail
        .as_deref()
        .unwrap()
        .contains("no active release-authorized contract generation"));
    assert_eq!(store.health().unwrap().stored_actions, 1);
}

#[test]
fn file_ingestion_is_bounded_canonical_and_rejects_symlinks() {
    let temporary = tempfile::tempdir().unwrap();
    let inputs = tempfile::tempdir().unwrap();
    let store = NodeStore::open(temporary.path()).unwrap();
    let action = root_action(
        &TestKey::new(15),
        ShotId::from_bytes([0x49; 32]),
        AvailabilityStatus::PubliclyAvailable,
    );
    let canonical_path = inputs.path().join("action.json");
    fs::write(&canonical_path, bytes(&action)).unwrap();
    assert!(store.ingest_file(&canonical_path).unwrap().stored);

    let noncanonical = inputs.path().join("noncanonical.json");
    let mut padded = bytes(&action);
    padded.push(b'\n');
    fs::write(&noncanonical, padded).unwrap();
    assert!(matches!(
        store.ingest_file(&noncanonical),
        Err(NodeError::Protocol(_))
    ));

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let linked = inputs.path().join("linked.json");
        symlink(&canonical_path, &linked).unwrap();
        assert!(matches!(
            store.ingest_file(&linked),
            Err(NodeError::UnsafeStorage(_))
        ));
    }
}

#[test]
fn ingest_cli_preserves_one_eligible_evidence_record_without_promoting_authority() {
    let node_root = tempfile::tempdir().unwrap();
    let inputs = tempfile::tempdir().unwrap();
    let action = root_action(
        &TestKey::new(17),
        ShotId::from_bytes([0x4a; 32]),
        AvailabilityStatus::PubliclyAvailable,
    );
    let path = inputs.path().join("action.json");
    fs::write(&path, bytes(&action)).unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_tohseno-node"))
        .arg("--root")
        .arg(node_root.path())
        .arg("ingest")
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        value
            .pointer("/validation/signed_record")
            .and_then(serde_json::Value::as_str),
        Some("verified")
    );
    assert_eq!(
        value
            .pointer("/validation/candidate_authority")
            .and_then(serde_json::Value::as_str),
        Some("unresolved")
    );
    let reopened = NodeStore::open(node_root.path()).unwrap();
    assert_eq!(reopened.info().unwrap().active_generation, None);
    assert_eq!(reopened.health().unwrap().stored_actions, 1);
    let shot_id = action.action.shot_id;
    let view = reopened.shot(shot_id).unwrap();
    assert_eq!(view.shot.validation.candidate_authority_verified, 0);
    assert_eq!(view.shot.validation.candidate_authority_unresolved, 1);
    assert!(view.shot.authority_verified_heads.is_empty());
}

#[test]
fn a_late_parent_from_another_shot_reclassifies_the_orphan_as_rejected() {
    let temporary = tempfile::tempdir().unwrap();
    let store = NodeStore::open(temporary.path()).unwrap();
    let key = TestKey::new(18);
    let parent_shot = ShotId::from_bytes([0x4b; 32]);
    let child_shot = ShotId::from_bytes([0x4c; 32]);
    let parent = root_action(&key, parent_shot, AvailabilityStatus::PubliclyAvailable);
    let child = key.sign(
        LineageAction::new(
            2,
            Some(parent.commitment().unwrap()),
            child_shot,
            key.builder,
            common::timestamp(2),
            AvailabilityStatus::PubliclyAvailable,
            LineagePayload::ArtifactAvailability(
                tohseno_protocol::ontology::ArtifactAvailabilityRecord {
                    target_role: "cross-shot".into(),
                    availability: tohseno_protocol::ontology::ArtifactAvailability::new(
                        tohseno_protocol::ontology::ArtifactDescriptor {
                            digest: Bytes32::new([0xce; 32]),
                            media_type: "application/octet-stream".into(),
                            byte_length: 1,
                            name: None,
                        },
                        AvailabilityStatus::Unknown,
                    ),
                    observed_at: common::timestamp(2),
                },
            ),
        )
        .unwrap(),
    );
    let child_digest = child.commitment().unwrap();
    assert_eq!(
        store
            .ingest(&bytes(&child))
            .unwrap()
            .validation
            .candidate_authority,
        AuthorityStatus::Unresolved
    );
    store.ingest(&bytes(&parent)).unwrap();
    let rejected = store
        .shot(child_shot)
        .unwrap()
        .actions
        .into_iter()
        .find(|action| action.digest == child_digest)
        .unwrap();
    assert_eq!(rejected.validation.segment, SegmentStatus::Rejected);
    assert_eq!(
        rejected.validation.candidate_authority,
        AuthorityStatus::Rejected
    );
    assert!(rejected
        .validation
        .detail
        .as_deref()
        .unwrap()
        .contains("ShotID changed"));
}
