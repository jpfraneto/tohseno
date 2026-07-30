use p256::ecdsa::signature::hazmat::PrehashSigner;
use p256::ecdsa::{Signature, SigningKey};
use serde_json::Value;
use tohseno_protocol::app_metadata::{AppMetadata, AppMetadataV2, EmbeddedAppMetadata};
use tohseno_protocol::canonical;
use tohseno_protocol::digest::{sha256, Address20, Bytes32, ExpressionId, ShotId, VersionId};
use tohseno_protocol::identity::BuilderId;
use tohseno_protocol::lineage::{
    adapt_v1_lineage, apply_lineage_actions, reduce_lineage, verify_lineage_segment, LineageAction,
    LineagePayload, SignedLineageAction,
};
use tohseno_protocol::ontology::{
    capability_graph_digest, organ_acceptance_gate_name, ArtifactAvailability, ArtifactDescriptor,
    AvailabilityStatus, ChangeScope, DesiredChange, Evolution, EvolutionaryIntent, Expression,
    Feedback, Genome, GenomeAcceptance, GenomeProposal, MaterializationProvenance, Organ,
    OriginalMaterial, Ownership, ShotCommitment, TokenAssociation, TokenAssociationOperation,
    VerificationGate, VerificationResult, VersionRecord, Visibility, ARTIFACT_AVAILABILITY_SCHEMA,
    EVOLUTIONARY_INTENT_SCHEMA, EVOLUTION_SCHEMA, EXPRESSION_SCHEMA, FEEDBACK_SCHEMA,
    GENOME_ACCEPTANCE_SCHEMA, GENOME_SCHEMA, ORGAN_SCHEMA, OWNERSHIP_SCHEMA,
    TOKEN_ASSOCIATION_SCHEMA, VERIFICATION_RESULT_SCHEMA, VERSION_SCHEMA,
};
use tohseno_protocol::record::{CanonicalTimestamp, ShotRecord};
use tohseno_protocol::signature::{P256PublicKey, P256Signature, SignatureSidecar};

struct TestKey {
    signing: SigningKey,
    public: P256PublicKey,
    builder: BuilderId,
}

impl TestKey {
    fn new(byte: u8) -> Self {
        let signing = SigningKey::from_bytes((&[byte; 32]).into()).unwrap();
        let point = signing.verifying_key().to_encoded_point(false);
        let x: [u8; 32] = point.x().unwrap().to_vec().try_into().unwrap();
        let y: [u8; 32] = point.y().unwrap().to_vec().try_into().unwrap();
        let public = P256PublicKey {
            x: Bytes32::new(x),
            y: Bytes32::new(y),
        };
        Self {
            signing,
            public,
            builder: BuilderId::new(Address20::from_bytes([byte; 20])),
        }
    }

    fn sign(&self, action: LineageAction) -> SignedLineageAction {
        let digest = action.signing_digest().unwrap();
        let signature: Signature = self.signing.sign_prehash(digest.as_bytes()).unwrap();
        let signature = signature.normalize_s().unwrap_or(signature);
        action
            .attach_signature(
                self.public.clone(),
                P256Signature {
                    r: Bytes32::new(signature.r().to_bytes().into()),
                    s: Bytes32::new(signature.s().to_bytes().into()),
                },
            )
            .unwrap()
    }
}

fn timestamp(second: u64) -> CanonicalTimestamp {
    CanonicalTimestamp::parse(format!("2026-07-29T00:00:{second:02}Z")).unwrap()
}

fn artifact(bytes: &[u8], media_type: &str, status: AvailabilityStatus) -> ArtifactAvailability {
    ArtifactAvailability {
        schema: ARTIFACT_AVAILABILITY_SCHEMA.into(),
        artifact: ArtifactDescriptor {
            digest: sha256(bytes),
            media_type: media_type.into(),
            byte_length: bytes.len().try_into().unwrap(),
            name: None,
        },
        status,
        locations: Vec::new(),
    }
}

fn genome(revision: u64) -> Genome {
    Genome {
        schema: GENOME_SCHEMA.into(),
        revision,
        purpose: "Keep a private daily field note without attention extraction.".into(),
        intended_for: vec!["one owner".into()],
        essential_experience: vec!["writing opens immediately".into()],
        behavioral_invariants: vec!["notes remain available offline".into()],
        interaction_laws: vec!["one deliberate action starts a note".into()],
        aesthetic_principles: vec!["quiet, legible, and unhurried".into()],
        privacy_principles: vec!["content remains local by default".into()],
        ownership_principles: vec!["the Shot controller accepts continuity changes".into()],
        platform_commitments: vec!["native Apple software".into()],
        boundaries: vec!["no social feed".into()],
        non_goals: vec!["advertising".into()],
        required_capabilities: vec!["local_memory".into()],
        forbidden_transformations: vec!["never add attention-extractive ranking".into()],
        acceptance_principles: vec!["all deterministic gates pass".into()],
        freely_changeable: vec!["display name and visual treatment".into()],
    }
}

fn empty_capability_graph_digest() -> Bytes32 {
    capability_graph_digest(&[]).unwrap()
}

fn organ(
    expression_id: ExpressionId,
    organ_id: &str,
    dependencies: &[&str],
    acceptance_tests: &[&str],
) -> Organ {
    Organ {
        schema: ORGAN_SCHEMA.into(),
        expression_id,
        organ_id: organ_id.into(),
        provides: vec![format!("{organ_id}_capability")],
        owns_state: vec![],
        permissions: vec![],
        dependencies: dependencies.iter().map(|value| (*value).into()).collect(),
        emits: vec![],
        consumes: vec![],
        satisfies_genome_constraints: vec!["notes remain available offline".into()],
        acceptance_tests: acceptance_tests
            .iter()
            .map(|value| (*value).into())
            .collect(),
        platforms: vec!["ios".into()],
    }
}

fn signed_action(
    key: &TestKey,
    actions: &[SignedLineageAction],
    shot_id: ShotId,
    payload: LineagePayload,
) -> SignedLineageAction {
    signed_action_with_availability(
        key,
        actions,
        shot_id,
        AvailabilityStatus::PubliclyAvailable,
        payload,
    )
}

fn signed_action_with_availability(
    key: &TestKey,
    actions: &[SignedLineageAction],
    shot_id: ShotId,
    availability: AvailabilityStatus,
    payload: LineagePayload,
) -> SignedLineageAction {
    let sequence = u64::try_from(actions.len()).unwrap() + 1;
    let previous = actions
        .last()
        .map(SignedLineageAction::commitment)
        .transpose()
        .unwrap();
    key.sign(
        LineageAction::new(
            sequence,
            previous,
            shot_id,
            key.builder,
            timestamp(sequence),
            availability,
            payload,
        )
        .unwrap(),
    )
}

struct Lifecycle {
    actions: Vec<SignedLineageAction>,
    shot_id: ShotId,
    expression_id: ExpressionId,
    first_version_id: VersionId,
    second_version_id: VersionId,
    feedback_action: Bytes32,
}

fn lifecycle() -> Lifecycle {
    let key = TestKey::new(1);
    let shot_id = ShotId::from_bytes([0x11; 32]);
    let expression_id = ExpressionId::from_bytes([0x22; 32]);
    let raw = "I need a notebook that is mine, even when my request contradicts itself.";
    let intention = tohseno_protocol::ontology::IntentionRecord::new(
        vec![OriginalMaterial {
            artifact: artifact(
                raw.as_bytes(),
                "text/plain; charset=utf-8",
                AvailabilityStatus::IntentionallyPrivate,
            ),
            inline_text: Some(raw.into()),
        }],
        timestamp(1),
    );
    let commitment = ShotCommitment::new(
        intention.commitment().unwrap(),
        key.builder,
        key.public.clone(),
        timestamp(1),
    );
    let mut actions = Vec::new();
    actions.push(signed_action(
        &key,
        &actions,
        shot_id,
        LineagePayload::Commitment(commitment),
    ));
    actions.push(signed_action(
        &key,
        &actions,
        shot_id,
        LineagePayload::Intention(intention),
    ));

    let initial_genome = genome(1);
    actions.push(signed_action(
        &key,
        &actions,
        shot_id,
        LineagePayload::GenomeProposal(GenomeProposal::initial(
            initial_genome.clone(),
            "Operational proposal derived without replacing the exact source.".into(),
        )),
    ));
    let proposal_action = actions.last().unwrap().commitment().unwrap();
    actions.push(signed_action(
        &key,
        &actions,
        shot_id,
        LineagePayload::GenomeAcceptance(GenomeAcceptance {
            schema: GENOME_ACCEPTANCE_SCHEMA.into(),
            proposal_action,
            revision: 1,
            genome_digest: initial_genome.digest().unwrap(),
            accepted_at: timestamp(4),
        }),
    ));
    let acceptance_action = actions.last().unwrap().commitment().unwrap();
    actions.push(signed_action(
        &key,
        &actions,
        shot_id,
        LineagePayload::Expression(Expression {
            schema: EXPRESSION_SCHEMA.into(),
            expression_id,
            kind: "native_application".into(),
            name: "Field Note".into(),
            platforms: vec!["ios".into()],
            genome_revision: 1,
            genome_digest: initial_genome.digest().unwrap(),
            definition: artifact(
                b"native Apple expression plan",
                "application/json",
                AvailabilityStatus::PubliclyAvailable,
            ),
        }),
    ));

    let source_one = sha256(b"source state one");
    let first_version_id = VersionId::derive(
        shot_id,
        expression_id,
        1,
        initial_genome.digest().unwrap(),
        source_one,
    );
    let verification_one = VerificationResult {
        schema: VERIFICATION_RESULT_SCHEMA.into(),
        expression_id,
        candidate_version_id: first_version_id,
        genome_revision: 1,
        genome_digest: initial_genome.digest().unwrap(),
        source_digest: source_one,
        capability_graph_digest: empty_capability_graph_digest(),
        gates: vec![VerificationGate {
            name: "swift-test".into(),
            passed: true,
            deterministic: true,
            evidence: None,
        }],
        passed: true,
        known_incompleteness: vec![],
        verified_at: timestamp(6),
    };
    actions.push(signed_action(
        &key,
        &actions,
        shot_id,
        LineagePayload::VerificationResult(verification_one),
    ));
    let verification_one_action = actions.last().unwrap().commitment().unwrap();
    actions.push(signed_action(
        &key,
        &actions,
        shot_id,
        LineagePayload::Version(VersionRecord {
            schema: VERSION_SCHEMA.into(),
            version_id: first_version_id,
            expression_id,
            ordinal: 1,
            genome_revision: 1,
            genome_digest: initial_genome.digest().unwrap(),
            source_digest: source_one,
            provenance: MaterializationProvenance {
                factory: "tohseno/apple".into(),
                factory_version: "0.7.0".into(),
                factory_source_commit: Some("a".repeat(40)),
                template_digest: Bytes32::new([0x31; 32]),
                input_action: acceptance_action,
                deterministic: true,
            },
            capability_graph_digest: empty_capability_graph_digest(),
            verification_action: verification_one_action,
            known_incompleteness: vec![],
            build_identity: Some("1".into()),
            build_digest: Some(Bytes32::new([0x33; 32])),
            accepted_at: timestamp(7),
        }),
    ));
    actions.push(signed_action_with_availability(
        &key,
        &actions,
        shot_id,
        AvailabilityStatus::IntentionallyPrivate,
        LineagePayload::Feedback(Feedback {
            schema: FEEDBACK_SCHEMA.into(),
            expression_id,
            version_id: first_version_id,
            build_identity: Some("1".into()),
            author: None,
            visibility: Visibility::Private,
            text: Some("The first launch needs a clearer writing affordance.".into()),
            observations: vec![],
            attachments: vec![],
            observed_at: timestamp(8),
        }),
    ));
    let feedback_action = actions.last().unwrap().commitment().unwrap();
    actions.push(signed_action(
        &key,
        &actions,
        shot_id,
        LineagePayload::EvolutionaryIntent(EvolutionaryIntent {
            schema: EVOLUTIONARY_INTENT_SCHEMA.into(),
            expression_id,
            from_version_id: first_version_id,
            preserved_invariants: vec!["notes remain available offline".into()],
            desired_changes: vec![DesiredChange {
                scope: ChangeScope::Implementation,
                description: "Make the new-note affordance unmistakable.".into(),
            }],
            feedback_actions: vec![feedback_action],
            references: vec![],
            proposed_genome_action: None,
        }),
    ));
    let evolutionary_intent_action = actions.last().unwrap().commitment().unwrap();
    let source_two = sha256(b"source state two");
    let second_version_id = VersionId::derive(
        shot_id,
        expression_id,
        2,
        initial_genome.digest().unwrap(),
        source_two,
    );
    actions.push(signed_action(
        &key,
        &actions,
        shot_id,
        LineagePayload::VerificationResult(VerificationResult {
            schema: VERIFICATION_RESULT_SCHEMA.into(),
            expression_id,
            candidate_version_id: second_version_id,
            genome_revision: 1,
            genome_digest: initial_genome.digest().unwrap(),
            source_digest: source_two,
            capability_graph_digest: empty_capability_graph_digest(),
            gates: vec![VerificationGate {
                name: "swift-test".into(),
                passed: true,
                deterministic: true,
                evidence: None,
            }],
            passed: true,
            known_incompleteness: vec![],
            verified_at: timestamp(10),
        }),
    ));
    let verification_two_action = actions.last().unwrap().commitment().unwrap();
    actions.push(signed_action(
        &key,
        &actions,
        shot_id,
        LineagePayload::Version(VersionRecord {
            schema: VERSION_SCHEMA.into(),
            version_id: second_version_id,
            expression_id,
            ordinal: 2,
            genome_revision: 1,
            genome_digest: initial_genome.digest().unwrap(),
            source_digest: source_two,
            provenance: MaterializationProvenance {
                factory: "tohseno/apple".into(),
                factory_version: "0.7.0".into(),
                factory_source_commit: Some("a".repeat(40)),
                template_digest: Bytes32::new([0x31; 32]),
                input_action: evolutionary_intent_action,
                deterministic: true,
            },
            capability_graph_digest: empty_capability_graph_digest(),
            verification_action: verification_two_action,
            known_incompleteness: vec![],
            build_identity: Some("2".into()),
            build_digest: Some(Bytes32::new([0x34; 32])),
            accepted_at: timestamp(11),
        }),
    ));
    actions.push(signed_action(
        &key,
        &actions,
        shot_id,
        LineagePayload::Evolution(Evolution {
            schema: EVOLUTION_SCHEMA.into(),
            evolutionary_intent_action,
            expression_id,
            from_version_id: first_version_id,
            to_version_id: second_version_id,
            from_genome_digest: initial_genome.digest().unwrap(),
            to_genome_digest: initial_genome.digest().unwrap(),
            genome_acceptance_action: None,
            preserved_invariants: vec!["notes remain available offline".into()],
            completed_at: timestamp(12),
        }),
    ));

    Lifecycle {
        actions,
        shot_id,
        expression_id,
        first_version_id,
        second_version_id,
        feedback_action,
    }
}

#[test]
fn capability_graph_digest_is_order_independent_and_acceptance_sensitive() {
    let expression_id = ExpressionId::from_bytes([0x61; 32]);
    let identity = organ(
        expression_id,
        "installation_identity",
        &[],
        &["embedded identity matches the accepted version"],
    );
    let memory = organ(
        expression_id,
        "local_memory",
        &["installation_identity"],
        &["owner state round-trips without a network"],
    );

    let digest = capability_graph_digest(&[identity.clone(), memory.clone()]).unwrap();
    assert_eq!(
        digest,
        capability_graph_digest(&[memory.clone(), identity.clone()]).unwrap()
    );
    assert_eq!(
        digest.to_string(),
        "0xc6471e1e126c3480aa004940a31a47cdf9e8f6d812cf540597e1454fa1c6d950"
    );

    let mut changed_acceptance = memory.clone();
    changed_acceptance.acceptance_tests =
        vec!["owner state survives a terminated process without a network".into()];
    assert_ne!(
        digest,
        capability_graph_digest(&[identity.clone(), changed_acceptance]).unwrap()
    );

    let mut foreign = memory;
    foreign.expression_id = ExpressionId::from_bytes([0x62; 32]);
    assert!(capability_graph_digest(&[identity, foreign]).is_err());
}

#[test]
fn verification_and_version_bind_the_exact_organ_graph_and_acceptance_gates() {
    let lifecycle = lifecycle();
    let owner = TestKey::new(1);
    let mut prefix = lifecycle.actions[..5].to_vec();
    let memory = organ(
        lifecycle.expression_id,
        "local_memory",
        &[],
        &["owner state round-trips without a network"],
    );
    prefix.push(signed_action(
        &owner,
        &prefix,
        lifecycle.shot_id,
        LineagePayload::Organ(memory.clone()),
    ));
    let graph_digest = capability_graph_digest(std::slice::from_ref(&memory)).unwrap();
    let source_digest = sha256(b"organ-bound source");
    let version_id = VersionId::derive(
        lifecycle.shot_id,
        lifecycle.expression_id,
        1,
        genome(1).digest().unwrap(),
        source_digest,
    );
    let acceptance_gate = VerificationGate {
        name: organ_acceptance_gate_name(&memory, 0).unwrap(),
        passed: true,
        deterministic: true,
        evidence: None,
    };
    let verification = VerificationResult {
        schema: VERIFICATION_RESULT_SCHEMA.into(),
        expression_id: lifecycle.expression_id,
        candidate_version_id: version_id,
        genome_revision: 1,
        genome_digest: genome(1).digest().unwrap(),
        source_digest,
        capability_graph_digest: graph_digest,
        gates: vec![
            VerificationGate {
                name: "swift-test".into(),
                passed: true,
                deterministic: true,
                evidence: None,
            },
            acceptance_gate,
        ],
        passed: true,
        known_incompleteness: vec![],
        verified_at: timestamp(7),
    };

    let mut forged_verification = prefix.clone();
    let mut wrong_graph = verification.clone();
    wrong_graph.capability_graph_digest = Bytes32::new([0xfe; 32]);
    forged_verification.push(signed_action(
        &owner,
        &forged_verification,
        lifecycle.shot_id,
        LineagePayload::VerificationResult(wrong_graph),
    ));
    assert!(reduce_lineage(&forged_verification)
        .unwrap_err()
        .to_string()
        .contains("exact current Organ graph"));

    let mut missing_acceptance_gate = prefix.clone();
    let mut missing_gate = verification.clone();
    missing_gate.gates.pop();
    missing_acceptance_gate.push(signed_action(
        &owner,
        &missing_acceptance_gate,
        lifecycle.shot_id,
        LineagePayload::VerificationResult(missing_gate),
    ));
    assert!(reduce_lineage(&missing_acceptance_gate)
        .unwrap_err()
        .to_string()
        .contains("omits a declared Organ acceptance test gate"));

    let mut verified = prefix;
    verified.push(signed_action(
        &owner,
        &verified,
        lifecycle.shot_id,
        LineagePayload::VerificationResult(verification),
    ));
    let verification_action = verified.last().unwrap().commitment().unwrap();
    let version = VersionRecord {
        schema: VERSION_SCHEMA.into(),
        version_id,
        expression_id: lifecycle.expression_id,
        ordinal: 1,
        genome_revision: 1,
        genome_digest: genome(1).digest().unwrap(),
        source_digest,
        provenance: MaterializationProvenance {
            factory: "tohseno/apple".into(),
            factory_version: "0.7.0".into(),
            factory_source_commit: Some("a".repeat(40)),
            template_digest: Bytes32::new([0x31; 32]),
            input_action: lifecycle.actions[3].commitment().unwrap(),
            deterministic: true,
        },
        capability_graph_digest: graph_digest,
        verification_action,
        known_incompleteness: vec![],
        build_identity: Some("1".into()),
        build_digest: Some(Bytes32::new([0x33; 32])),
        accepted_at: timestamp(8),
    };

    let mut forged_version = verified.clone();
    let mut wrong_version_graph = version.clone();
    wrong_version_graph.capability_graph_digest = Bytes32::new([0xfd; 32]);
    forged_version.push(signed_action(
        &owner,
        &forged_version,
        lifecycle.shot_id,
        LineagePayload::Version(wrong_version_graph),
    ));
    assert!(reduce_lineage(&forged_version)
        .unwrap_err()
        .to_string()
        .contains("exactly match the referenced verification"));

    let mut graph_changed_after_verification = verified.clone();
    graph_changed_after_verification.push(signed_action(
        &owner,
        &graph_changed_after_verification,
        lifecycle.shot_id,
        LineagePayload::Organ(organ(
            lifecycle.expression_id,
            "navigation",
            &["local_memory"],
            &["the essential surface is directly reachable"],
        )),
    ));
    graph_changed_after_verification.push(signed_action(
        &owner,
        &graph_changed_after_verification,
        lifecycle.shot_id,
        LineagePayload::Version(version.clone()),
    ));
    assert!(reduce_lineage(&graph_changed_after_verification)
        .unwrap_err()
        .to_string()
        .contains("exact current Organ graph"));

    verified.push(signed_action(
        &owner,
        &verified,
        lifecycle.shot_id,
        LineagePayload::Version(version),
    ));
    assert_eq!(
        reduce_lineage(&verified)
            .unwrap()
            .expression(lifecycle.expression_id)
            .unwrap()
            .current_version,
        Some(version_id)
    );
}

#[test]
fn complete_lifecycle_preserves_intention_and_binds_exact_versions() {
    let lifecycle = lifecycle();
    let state = reduce_lineage(&lifecycle.actions).unwrap();
    assert_eq!(state.shot_id, lifecycle.shot_id);
    assert!(state.intention.as_ref().unwrap().materials[0]
        .inline_text
        .as_ref()
        .unwrap()
        .contains("contradicts itself"));
    let expression = state.expression(lifecycle.expression_id).unwrap();
    assert_eq!(expression.versions.len(), 2);
    assert_eq!(
        expression.current_version,
        Some(lifecycle.second_version_id)
    );
    assert_eq!(
        state
            .feedback
            .get(&lifecycle.feedback_action)
            .unwrap()
            .version_id,
        lifecycle.first_version_id
    );
    assert_eq!(state.evolutions.len(), 1);

    // Folder and display-name facts do not participate in Shot identity.
    let renamed_folder = "/tmp/a-completely-different-folder";
    assert!(!renamed_folder.contains(&state.shot_id.to_string()));
    assert_eq!(state.shot_id, lifecycle.shot_id);
}

#[test]
fn tampering_downgrade_unknown_fields_and_floating_feedback_are_rejected() {
    let lifecycle = lifecycle();
    let mut tampered = lifecycle.actions.clone();
    tampered[1].action.payload_digest = Bytes32::new([0xee; 32]);
    assert!(reduce_lineage(&tampered).is_err());

    let mut downgrade = lifecycle.actions[0].clone();
    downgrade.action.protocol_version = "1".into();
    assert!(downgrade.verify().is_err());

    let mut value = serde_json::to_value(&lifecycle.actions[0]).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .insert("unexpected".into(), Value::Bool(true));
    assert!(
        canonical::from_slice::<SignedLineageAction>(&serde_json::to_vec(&value).unwrap()).is_err()
    );
    let mut payload_unknown = serde_json::to_value(&lifecycle.actions[0]).unwrap();
    payload_unknown["action"]["payload"]
        .as_object_mut()
        .unwrap()
        .insert("unexpected".into(), Value::Bool(true));
    assert!(canonical::from_slice::<SignedLineageAction>(
        &serde_json::to_vec(&payload_unknown).unwrap()
    )
    .is_err());

    let mut floating = lifecycle.actions.clone();
    if let LineagePayload::Feedback(feedback) = &mut floating[7].action.payload {
        feedback.version_id = VersionId::from_bytes([0xff; 32]);
    }
    assert!(reduce_lineage(&floating).is_err());
}

#[test]
fn feedback_visibility_must_exactly_match_action_availability() {
    let lifecycle = lifecycle();
    let key = TestKey::new(1);
    let private_feedback = match &lifecycle.actions[7].action.payload {
        LineagePayload::Feedback(feedback) => feedback.clone(),
        _ => panic!("lifecycle action 8 must be feedback"),
    };

    let private_marked_public = LineageAction::new(
        1,
        None,
        lifecycle.shot_id,
        key.builder,
        timestamp(1),
        AvailabilityStatus::PubliclyAvailable,
        LineagePayload::Feedback(private_feedback.clone()),
    )
    .unwrap_err();
    assert!(private_marked_public
        .to_string()
        .contains("must exactly match feedback visibility"));

    let mut public_feedback = private_feedback;
    public_feedback.visibility = Visibility::Public;
    let public_marked_private = LineageAction::new(
        1,
        None,
        lifecycle.shot_id,
        key.builder,
        timestamp(1),
        AvailabilityStatus::IntentionallyPrivate,
        LineagePayload::Feedback(public_feedback),
    )
    .unwrap_err();
    assert!(public_marked_private
        .to_string()
        .contains("must exactly match feedback visibility"));
}

#[test]
fn partial_segment_is_honest_about_missing_authority_context() {
    let lifecycle = lifecycle();
    let segment = verify_lineage_segment(&lifecycle.actions[5..], None).unwrap();
    assert!(!segment.complete_from_commitment);
    assert!(!segment.authority_context_available);
    assert_eq!(segment.shot_id, lifecycle.shot_id);
}

#[test]
fn failed_verification_never_becomes_an_accepted_version() {
    let lifecycle = lifecycle();
    let mut actions = lifecycle.actions[..5].to_vec();
    let key = TestKey::new(1);
    let source = sha256(b"failed source");
    let candidate = VersionId::derive(
        lifecycle.shot_id,
        lifecycle.expression_id,
        1,
        genome(1).digest().unwrap(),
        source,
    );
    actions.push(signed_action(
        &key,
        &actions,
        lifecycle.shot_id,
        LineagePayload::VerificationResult(VerificationResult {
            schema: VERIFICATION_RESULT_SCHEMA.into(),
            expression_id: lifecycle.expression_id,
            candidate_version_id: candidate,
            genome_revision: 1,
            genome_digest: genome(1).digest().unwrap(),
            source_digest: source,
            capability_graph_digest: empty_capability_graph_digest(),
            gates: vec![VerificationGate {
                name: "swift-test".into(),
                passed: false,
                deterministic: true,
                evidence: None,
            }],
            passed: false,
            known_incompleteness: vec!["compiler error".into()],
            verified_at: timestamp(6),
        }),
    ));
    let failed_verification = actions.last().unwrap().commitment().unwrap();
    actions.push(signed_action(
        &key,
        &actions,
        lifecycle.shot_id,
        LineagePayload::Version(VersionRecord {
            schema: VERSION_SCHEMA.into(),
            version_id: candidate,
            expression_id: lifecycle.expression_id,
            ordinal: 1,
            genome_revision: 1,
            genome_digest: genome(1).digest().unwrap(),
            source_digest: source,
            provenance: MaterializationProvenance {
                factory: "tohseno/apple".into(),
                factory_version: "1".into(),
                factory_source_commit: None,
                template_digest: Bytes32::new([1; 32]),
                input_action: failed_verification,
                deterministic: true,
            },
            capability_graph_digest: empty_capability_graph_digest(),
            verification_action: failed_verification,
            known_incompleteness: vec!["compiler error".into()],
            build_identity: None,
            build_digest: None,
            accepted_at: timestamp(7),
        }),
    ));
    assert!(reduce_lineage(&actions).is_err());
    let mut prefix = reduce_lineage(&actions[..5]).unwrap();
    let unchanged = prefix.clone();
    assert!(apply_lineage_actions(&mut prefix, &actions[5..]).is_err());
    assert_eq!(prefix, unchanged);
}

#[test]
fn genome_mutation_requires_explicit_proposal_acceptance_and_authority() {
    let lifecycle = lifecycle();
    let owner = TestKey::new(1);
    let outsider = TestKey::new(2);
    let current_genome = genome(1);
    let initial_acceptance_action = lifecycle.actions[3].commitment().unwrap();
    let mut revised_genome = genome(2);
    revised_genome.purpose =
        "Keep a private daily field note with owner-requested encrypted export.".into();
    let proposal = GenomeProposal {
        schema: tohseno_protocol::ontology::GENOME_PROPOSAL_SCHEMA.into(),
        base_revision: Some(1),
        base_genome_digest: Some(current_genome.digest().unwrap()),
        proposed: revised_genome.clone(),
        rationale: "The owner explicitly requested portable encrypted export.".into(),
        mutation_summary: vec!["Permit an owner-controlled encrypted export.".into()],
    };

    let mut unauthorized = lifecycle.actions.clone();
    unauthorized.push(signed_action(
        &outsider,
        &unauthorized,
        lifecycle.shot_id,
        LineagePayload::GenomeProposal(proposal.clone()),
    ));
    assert!(reduce_lineage(&unauthorized).is_err());

    let mut actions = lifecycle.actions;
    actions.push(signed_action(
        &owner,
        &actions,
        lifecycle.shot_id,
        LineagePayload::GenomeProposal(proposal),
    ));
    let proposal_action = actions.last().unwrap().commitment().unwrap();
    actions.push(signed_action(
        &owner,
        &actions,
        lifecycle.shot_id,
        LineagePayload::EvolutionaryIntent(EvolutionaryIntent {
            schema: EVOLUTIONARY_INTENT_SCHEMA.into(),
            expression_id: lifecycle.expression_id,
            from_version_id: lifecycle.second_version_id,
            preserved_invariants: vec!["notes remain available offline".into()],
            desired_changes: vec![DesiredChange {
                scope: ChangeScope::Genome,
                description: "Permit a private encrypted export.".into(),
            }],
            feedback_actions: vec![],
            references: vec![],
            proposed_genome_action: Some(proposal_action),
        }),
    ));
    let intent_action = actions.last().unwrap().commitment().unwrap();
    actions.push(signed_action(
        &owner,
        &actions,
        lifecycle.shot_id,
        LineagePayload::GenomeAcceptance(GenomeAcceptance {
            schema: GENOME_ACCEPTANCE_SCHEMA.into(),
            proposal_action,
            revision: 2,
            genome_digest: revised_genome.digest().unwrap(),
            accepted_at: timestamp(15),
        }),
    ));
    let acceptance_action = actions.last().unwrap().commitment().unwrap();
    let source = sha256(b"source state three");
    let third_version_id = VersionId::derive(
        lifecycle.shot_id,
        lifecycle.expression_id,
        3,
        revised_genome.digest().unwrap(),
        source,
    );
    actions.push(signed_action(
        &owner,
        &actions,
        lifecycle.shot_id,
        LineagePayload::VerificationResult(VerificationResult {
            schema: VERIFICATION_RESULT_SCHEMA.into(),
            expression_id: lifecycle.expression_id,
            candidate_version_id: third_version_id,
            genome_revision: 2,
            genome_digest: revised_genome.digest().unwrap(),
            source_digest: source,
            capability_graph_digest: empty_capability_graph_digest(),
            gates: vec![VerificationGate {
                name: "swift-test".into(),
                passed: true,
                deterministic: true,
                evidence: None,
            }],
            passed: true,
            known_incompleteness: vec![],
            verified_at: timestamp(16),
        }),
    ));
    let verification_action = actions.last().unwrap().commitment().unwrap();
    actions.push(signed_action(
        &owner,
        &actions,
        lifecycle.shot_id,
        LineagePayload::Version(VersionRecord {
            schema: VERSION_SCHEMA.into(),
            version_id: third_version_id,
            expression_id: lifecycle.expression_id,
            ordinal: 3,
            genome_revision: 2,
            genome_digest: revised_genome.digest().unwrap(),
            source_digest: source,
            provenance: MaterializationProvenance {
                factory: "tohseno/apple".into(),
                factory_version: "0.7.0".into(),
                factory_source_commit: Some("a".repeat(40)),
                template_digest: Bytes32::new([0x31; 32]),
                input_action: intent_action,
                deterministic: true,
            },
            capability_graph_digest: empty_capability_graph_digest(),
            verification_action,
            known_incompleteness: vec![],
            build_identity: Some("3".into()),
            build_digest: Some(Bytes32::new([0x36; 32])),
            accepted_at: timestamp(17),
        }),
    ));
    let evolution = Evolution {
        schema: EVOLUTION_SCHEMA.into(),
        evolutionary_intent_action: intent_action,
        expression_id: lifecycle.expression_id,
        from_version_id: lifecycle.second_version_id,
        to_version_id: third_version_id,
        from_genome_digest: current_genome.digest().unwrap(),
        to_genome_digest: revised_genome.digest().unwrap(),
        genome_acceptance_action: Some(acceptance_action),
        preserved_invariants: vec!["notes remain available offline".into()],
        completed_at: timestamp(18),
    };

    let mut wrong_acceptance = actions.clone();
    let mut wrong_evolution = evolution.clone();
    wrong_evolution.genome_acceptance_action = Some(initial_acceptance_action);
    wrong_acceptance.push(signed_action(
        &owner,
        &wrong_acceptance,
        lifecycle.shot_id,
        LineagePayload::Evolution(wrong_evolution),
    ));
    let error = reduce_lineage(&wrong_acceptance).unwrap_err();
    assert!(error
        .to_string()
        .contains("does not accept the intent's exact proposal"));

    actions.push(signed_action(
        &owner,
        &actions,
        lifecycle.shot_id,
        LineagePayload::Evolution(evolution),
    ));

    let state = reduce_lineage(&actions).unwrap();
    assert_eq!(state.accepted_genome.as_ref().unwrap().genome.revision, 2);
    assert_eq!(
        state
            .expression(lifecycle.expression_id)
            .unwrap()
            .current_version,
        Some(third_version_id)
    );
}

#[test]
fn genome_scoped_intent_cannot_complete_as_an_unchanged_evolution() {
    let lifecycle = lifecycle();
    let owner = TestKey::new(1);
    let shot_id = lifecycle.shot_id;
    let expression_id = lifecycle.expression_id;
    let first_version_id = lifecycle.first_version_id;
    let current_genome = genome(1);
    let mut revised_genome = genome(2);
    revised_genome.purpose =
        "Keep a private daily field note with owner-requested encrypted export.".into();
    let mut actions = lifecycle.actions[..8].to_vec();

    actions.push(signed_action(
        &owner,
        &actions,
        shot_id,
        LineagePayload::GenomeProposal(GenomeProposal {
            schema: tohseno_protocol::ontology::GENOME_PROPOSAL_SCHEMA.into(),
            base_revision: Some(1),
            base_genome_digest: Some(current_genome.digest().unwrap()),
            proposed: revised_genome,
            rationale: "The owner proposed encrypted export.".into(),
            mutation_summary: vec!["Permit an owner-controlled encrypted export.".into()],
        }),
    ));
    let proposal_action = actions.last().unwrap().commitment().unwrap();
    actions.push(signed_action(
        &owner,
        &actions,
        shot_id,
        LineagePayload::EvolutionaryIntent(EvolutionaryIntent {
            schema: EVOLUTIONARY_INTENT_SCHEMA.into(),
            expression_id,
            from_version_id: first_version_id,
            preserved_invariants: vec!["notes remain available offline".into()],
            desired_changes: vec![DesiredChange {
                scope: ChangeScope::Genome,
                description: "Permit a private encrypted export.".into(),
            }],
            feedback_actions: vec![],
            references: vec![],
            proposed_genome_action: Some(proposal_action),
        }),
    ));
    let intent_action = actions.last().unwrap().commitment().unwrap();

    let source = sha256(b"unchanged-genome source state two");
    let second_version_id = VersionId::derive(
        shot_id,
        expression_id,
        2,
        current_genome.digest().unwrap(),
        source,
    );
    actions.push(signed_action(
        &owner,
        &actions,
        shot_id,
        LineagePayload::VerificationResult(VerificationResult {
            schema: VERIFICATION_RESULT_SCHEMA.into(),
            expression_id,
            candidate_version_id: second_version_id,
            genome_revision: 1,
            genome_digest: current_genome.digest().unwrap(),
            source_digest: source,
            capability_graph_digest: empty_capability_graph_digest(),
            gates: vec![VerificationGate {
                name: "swift-test".into(),
                passed: true,
                deterministic: true,
                evidence: None,
            }],
            passed: true,
            known_incompleteness: vec![],
            verified_at: timestamp(11),
        }),
    ));
    let verification_action = actions.last().unwrap().commitment().unwrap();
    actions.push(signed_action(
        &owner,
        &actions,
        shot_id,
        LineagePayload::Version(VersionRecord {
            schema: VERSION_SCHEMA.into(),
            version_id: second_version_id,
            expression_id,
            ordinal: 2,
            genome_revision: 1,
            genome_digest: current_genome.digest().unwrap(),
            source_digest: source,
            provenance: MaterializationProvenance {
                factory: "tohseno/apple".into(),
                factory_version: "0.7.0".into(),
                factory_source_commit: Some("a".repeat(40)),
                template_digest: Bytes32::new([0x31; 32]),
                input_action: intent_action,
                deterministic: true,
            },
            capability_graph_digest: empty_capability_graph_digest(),
            verification_action,
            known_incompleteness: vec![],
            build_identity: Some("2".into()),
            build_digest: Some(Bytes32::new([0x34; 32])),
            accepted_at: timestamp(12),
        }),
    ));
    actions.push(signed_action(
        &owner,
        &actions,
        shot_id,
        LineagePayload::Evolution(Evolution {
            schema: EVOLUTION_SCHEMA.into(),
            evolutionary_intent_action: intent_action,
            expression_id,
            from_version_id: first_version_id,
            to_version_id: second_version_id,
            from_genome_digest: current_genome.digest().unwrap(),
            to_genome_digest: current_genome.digest().unwrap(),
            genome_acceptance_action: None,
            preserved_invariants: vec!["notes remain available offline".into()],
            completed_at: timestamp(13),
        }),
    ));

    let error = reduce_lineage(&actions).unwrap_err();
    assert!(error
        .to_string()
        .contains("cannot complete without its genome mutation"));
}

#[test]
fn ownership_transfer_switches_signer_and_old_signer_is_rejected() {
    let old = TestKey::new(3);
    let new = TestKey::new(4);
    let shot_id = ShotId::from_bytes([0x44; 32]);
    let intention = tohseno_protocol::ontology::IntentionRecord::new(
        vec![OriginalMaterial {
            artifact: artifact(
                b"ownership test",
                "text/plain",
                AvailabilityStatus::IntentionallyPrivate,
            ),
            inline_text: Some("ownership test".into()),
        }],
        timestamp(1),
    );
    let commitment = ShotCommitment::new(
        intention.commitment().unwrap(),
        old.builder,
        old.public.clone(),
        timestamp(1),
    );
    let mut actions = vec![signed_action(
        &old,
        &[],
        shot_id,
        LineagePayload::Commitment(commitment),
    )];
    actions.push(signed_action(
        &old,
        &actions,
        shot_id,
        LineagePayload::Ownership(Ownership {
            schema: OWNERSHIP_SCHEMA.into(),
            previous_controller: old.builder,
            new_controller: new.builder,
            new_controller_key: new.public.clone(),
            reason: "Owner-authorized transfer.".into(),
            effective_at: timestamp(2),
        }),
    ));
    actions.push(signed_action(
        &new,
        &actions,
        shot_id,
        LineagePayload::TokenAssociation(TokenAssociation {
            schema: TOKEN_ASSOCIATION_SCHEMA.into(),
            operation: TokenAssociationOperation::Associate,
            chain_id: 8453,
            token: Address20::from_bytes([0xaa; 20]),
            symbol: Some("ANKY".into()),
            anchor: None,
        }),
    ));
    assert_eq!(
        reduce_lineage(&actions)
            .unwrap()
            .token_association
            .unwrap()
            .chain_id,
        8453
    );

    let mut old_signer = actions[..2].to_vec();
    old_signer.push(signed_action(
        &old,
        &old_signer,
        shot_id,
        LineagePayload::TokenAssociation(TokenAssociation {
            schema: TOKEN_ASSOCIATION_SCHEMA.into(),
            operation: TokenAssociationOperation::Associate,
            chain_id: 8453,
            token: Address20::from_bytes([0xaa; 20]),
            symbol: None,
            anchor: None,
        }),
    ));
    assert!(reduce_lineage(&old_signer).is_err());
}

#[test]
fn token_relation_is_replaceable_history_not_shot_identity() {
    let owner = TestKey::new(5);
    let shot_id = ShotId::from_bytes([0x55; 32]);
    let intention = tohseno_protocol::ontology::IntentionRecord::new(
        vec![OriginalMaterial {
            artifact: artifact(
                b"token test",
                "text/plain",
                AvailabilityStatus::IntentionallyPrivate,
            ),
            inline_text: Some("token test".into()),
        }],
        timestamp(1),
    );
    let mut actions = vec![signed_action(
        &owner,
        &[],
        shot_id,
        LineagePayload::Commitment(ShotCommitment::new(
            intention.commitment().unwrap(),
            owner.builder,
            owner.public.clone(),
            timestamp(1),
        )),
    )];
    for token_byte in [0xa1, 0xa2] {
        actions.push(signed_action(
            &owner,
            &actions,
            shot_id,
            LineagePayload::TokenAssociation(TokenAssociation {
                schema: TOKEN_ASSOCIATION_SCHEMA.into(),
                operation: TokenAssociationOperation::Associate,
                chain_id: 8453,
                token: Address20::from_bytes([token_byte; 20]),
                symbol: Some("ANKY".into()),
                anchor: None,
            }),
        ));
    }
    let state = reduce_lineage(&actions).unwrap();
    assert_eq!(state.shot_id, shot_id);
    assert_eq!(state.token_history.len(), 2);
    assert_eq!(
        state.token_association.unwrap().token,
        Address20::from_bytes([0xa2; 20])
    );

    let mut mismatched_removal = actions.clone();
    mismatched_removal.push(signed_action(
        &owner,
        &mismatched_removal,
        shot_id,
        LineagePayload::TokenAssociation(TokenAssociation {
            schema: TOKEN_ASSOCIATION_SCHEMA.into(),
            operation: TokenAssociationOperation::Remove,
            chain_id: 8453,
            token: Address20::from_bytes([0xa1; 20]),
            symbol: None,
            anchor: None,
        }),
    ));
    assert!(reduce_lineage(&mismatched_removal)
        .unwrap_err()
        .to_string()
        .contains("must exactly match"));

    actions.push(signed_action(
        &owner,
        &actions,
        shot_id,
        LineagePayload::TokenAssociation(TokenAssociation {
            schema: TOKEN_ASSOCIATION_SCHEMA.into(),
            operation: TokenAssociationOperation::Remove,
            chain_id: 8453,
            token: Address20::from_bytes([0xa2; 20]),
            symbol: None,
            anchor: None,
        }),
    ));
    let removed = reduce_lineage(&actions).unwrap();
    assert_eq!(removed.shot_id, shot_id);
    assert!(removed.token_association.is_none());
    assert_eq!(removed.token_history.len(), 3);

    actions.push(signed_action(
        &owner,
        &actions,
        shot_id,
        LineagePayload::TokenAssociation(TokenAssociation {
            schema: TOKEN_ASSOCIATION_SCHEMA.into(),
            operation: TokenAssociationOperation::Remove,
            chain_id: 8453,
            token: Address20::from_bytes([0xa2; 20]),
            symbol: None,
            anchor: None,
        }),
    ));
    assert!(reduce_lineage(&actions)
        .unwrap_err()
        .to_string()
        .contains("cannot remove a missing"));
}

#[test]
fn frozen_v1_is_projected_without_rewriting_or_inventing_genome() {
    let vectors: Value =
        serde_json::from_str(include_str!("../test-vectors/protocol-v1.json")).unwrap();
    let record: ShotRecord = serde_json::from_value(vectors["record"]["value"].clone()).unwrap();
    let signature: SignatureSidecar =
        serde_json::from_value(vectors["record"]["sidecar"].clone()).unwrap();
    let original_record_bytes = canonical::to_vec(&record).unwrap();
    let original_signature_bytes = canonical::to_vec(&signature).unwrap();
    let adapted = adapt_v1_lineage(&[(&record, &signature)]).unwrap();
    assert_eq!(adapted.entries[0].record, record);
    assert_eq!(adapted.entries[0].signature, signature);
    assert_eq!(
        canonical::to_vec(&adapted.entries[0].record).unwrap(),
        original_record_bytes
    );
    assert_eq!(
        canonical::to_vec(&adapted.entries[0].signature).unwrap(),
        original_signature_bytes
    );
    assert_eq!(adapted.genome_availability, AvailabilityStatus::Unknown);
}

#[test]
fn app_metadata_v1_bytes_stay_frozen_while_v2_binds_neutral_identity() {
    let v1: AppMetadata =
        serde_json::from_str(include_str!("../test-vectors/app-metadata-v1.json")).unwrap();
    let frozen = include_bytes!("../test-vectors/app-metadata-v1.json");
    assert_eq!(
        serde_json::from_slice::<Value>(frozen).unwrap()["schema"],
        "tohseno.app-metadata/1"
    );
    let expression_id = ExpressionId::from_bytes([0x77; 32]);
    let genome_digest = Bytes32::new([0x78; 32]);
    let version_id = VersionId::derive(
        v1.shot_id,
        expression_id,
        1,
        genome_digest,
        v1.source_tree_sha256,
    );
    let v2 = AppMetadataV2::from_v1(
        &v1,
        expression_id,
        version_id,
        1,
        1,
        genome_digest,
        8,
        Bytes32::new([0x79; 32]),
        Some(Bytes32::new([0x7a; 32])),
    )
    .unwrap();
    v2.validate().unwrap();
    assert_eq!(v2.schema, "tohseno.app-metadata/2");
    assert_eq!(v2.version_id, version_id);
    assert_eq!(v1.schema, "tohseno.app-metadata/1");
    let fixture = include_str!("../test-vectors/app-metadata-v2.json");
    let decoded: AppMetadataV2 = serde_json::from_str(fixture).unwrap();
    assert_eq!(decoded, v2);
    assert_eq!(
        fixture,
        format!("{}\n", serde_json::to_string_pretty(&v2).unwrap())
    );

    let dispatched_v1 = EmbeddedAppMetadata::decode_transport_json(frozen).unwrap();
    assert!(matches!(dispatched_v1, EmbeddedAppMetadata::V1(_)));
    assert_eq!(dispatched_v1.schema(), "tohseno.app-metadata/1");
    let dispatched_v2 = EmbeddedAppMetadata::decode_transport_json(fixture.as_bytes()).unwrap();
    assert!(matches!(dispatched_v2, EmbeddedAppMetadata::V2(_)));
    assert_eq!(dispatched_v2.schema(), "tohseno.app-metadata/2");

    let mut mismatched_bundle_version = v2.clone();
    mismatched_bundle_version.bundle_version = 2;
    assert!(mismatched_bundle_version.validate().is_err());

    let unknown_schema = fixture.replace("tohseno.app-metadata/2", "tohseno.app-metadata/999");
    assert!(EmbeddedAppMetadata::decode_transport_json(unknown_schema.as_bytes()).is_err());

    let v2_with_v1_field = fixture.replace(
        "\"protocol_version\": \"2\",",
        "\"protocol_version\": \"2\", \"sequence\": 1,",
    );
    assert!(EmbeddedAppMetadata::decode_transport_json(v2_with_v1_field.as_bytes()).is_err());

    let frozen_v1 = String::from_utf8(frozen.to_vec()).unwrap();
    let v1_with_v2_field = frozen_v1.replace(
        "\"schema\": \"tohseno.app-metadata/1\",",
        "\"schema\": \"tohseno.app-metadata/1\", \"protocol_version\": \"2\",",
    );
    assert!(EmbeddedAppMetadata::decode_transport_json(v1_with_v2_field.as_bytes()).is_err());

    let duplicate_schema = fixture.replace(
        "\"schema\": \"tohseno.app-metadata/2\",",
        "\"schema\": \"tohseno.app-metadata/2\", \"schema\": \"tohseno.app-metadata/1\",",
    );
    assert!(EmbeddedAppMetadata::decode_transport_json(duplicate_schema.as_bytes()).is_err());
}

#[test]
fn frozen_v2_lineage_fixture_verifies_and_reduces_cross_language_bytes() {
    let fixture = include_str!("../test-vectors/lineage-v2.json");
    let value: Value = serde_json::from_str(fixture).unwrap();
    assert_eq!(value["schema"], "tohseno.lineage-test-vectors/2");
    let actions: Vec<SignedLineageAction> =
        serde_json::from_value(value["actions"].clone()).unwrap();
    let commitments: Vec<Bytes32> = serde_json::from_value(value["commitments"].clone()).unwrap();
    assert_eq!(actions.len(), 2);
    for (action, expected) in actions.iter().zip(commitments) {
        assert_eq!(action.commitment().unwrap(), expected);
    }
    let state = reduce_lineage(&actions).unwrap();
    assert_eq!(state.sequence, 2);
    assert_eq!(state.availability.len(), 1);
    assert_eq!(
        fixture,
        format!("{}\n", serde_json::to_string_pretty(&value).unwrap())
    );
}
