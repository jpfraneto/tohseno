mod common;

use axum::body::Body;
use axum::extract::Path;
use axum::http::{Response, StatusCode};
use axum::routing::get;
use axum::{Json, Router};
use common::{
    availability_action, availability_after, availability_after_with_handling, bytes,
    ownership_action, root_action, TestKey,
};
use std::net::SocketAddr;
use tohseno_node::{
    serve, ActionReference, ActionValidation, AuthorityStatus, Node, NodeStore, Peer,
    SegmentStatus, ShotSummary, ShotView, SignedRecordStatus, SyncState, ValidationCounts,
    MAX_ACTION_BYTES,
};
use tohseno_protocol::digest::{Address20, ShotId};
use tohseno_protocol::lineage::{LineageAction, LineagePayload};
use tohseno_protocol::ontology::{
    AvailabilityStatus, TokenAssociation, TokenAssociationOperation, TOKEN_ASSOCIATION_SCHEMA,
};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

async fn spawn_node(node: Node) -> (SocketAddr, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let _ = serve(node, listener).await;
    });
    (address, task)
}

async fn spawn_router(router: Router) -> (SocketAddr, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    (address, task)
}

#[tokio::test]
async fn two_nodes_sync_explicitly_and_a_survivor_serves_its_possessed_lineage() {
    let source_root = tempfile::tempdir().unwrap();
    let source_store = NodeStore::open(source_root.path()).unwrap();
    let key = TestKey::new(10);
    let shot_id = ShotId::from_bytes([0x61; 32]);
    let root = root_action(&key, shot_id, AvailabilityStatus::PubliclyAvailable);
    let availability = availability_action(
        &key,
        &root,
        b"unavailable source",
        AvailabilityStatus::Unknown,
        "source",
    );
    source_store.ingest(&bytes(&root)).unwrap();
    source_store.ingest(&bytes(&availability)).unwrap();
    let root_digest = root.commitment().unwrap();

    let source = Node::new(source_store, Vec::new()).unwrap();
    let (source_address, source_task) = spawn_node(source).await;
    let destination_root = tempfile::tempdir().unwrap();
    let destination = Node::new(
        NodeStore::open(destination_root.path()).unwrap(),
        vec![Peer::parse(&format!("http://{source_address}")).unwrap()],
    )
    .unwrap();
    let report = destination.sync().await.unwrap();
    assert_eq!(report.state, SyncState::Succeeded);
    assert_eq!(report.peers[0].fetched, 2);
    assert_eq!(
        destination
            .store()
            .shot(shot_id)
            .unwrap()
            .shot
            .missing_artifacts
            .len(),
        1
    );

    let (destination_address, destination_task) = spawn_node(destination.clone()).await;
    source_task.abort();
    let client = reqwest::Client::new();
    let action_response = client
        .get(format!(
            "http://{destination_address}/v1/actions/{root_digest}"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(action_response.status(), StatusCode::OK);
    assert_eq!(action_response.bytes().await.unwrap(), bytes(&root));
    let shot_response = client
        .get(format!("http://{destination_address}/v1/shots/{shot_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(shot_response.status(), StatusCode::OK);
    destination_task.abort();
}

#[tokio::test]
async fn public_token_association_across_a_private_boundary_stays_unresolved_after_sync() {
    const BASE_CHAIN_ID: u64 = 8453;

    let key = TestKey::new(23);
    let shot_id = ShotId::from_bytes([0x67; 32]);
    let token = Address20::from_bytes([0xa7; 20]);
    let private_parent = root_action(&key, shot_id, AvailabilityStatus::IntentionallyPrivate);
    let private_parent_digest = private_parent.commitment().unwrap();
    let association = key.sign(
        LineageAction::new(
            2,
            Some(private_parent_digest),
            shot_id,
            key.builder,
            common::timestamp(2),
            AvailabilityStatus::PubliclyAvailable,
            LineagePayload::TokenAssociation(TokenAssociation {
                schema: TOKEN_ASSOCIATION_SCHEMA.into(),
                operation: TokenAssociationOperation::Associate,
                chain_id: BASE_CHAIN_ID,
                token,
                symbol: Some("ANKY".into()),
                anchor: None,
            }),
        )
        .unwrap(),
    );
    let association_digest = association.commitment().unwrap();
    let canonical_bytes = bytes(&association);

    assert_eq!(
        private_parent.action.availability,
        AvailabilityStatus::IntentionallyPrivate
    );
    assert_eq!(
        association.action.availability,
        AvailabilityStatus::PubliclyAvailable
    );
    assert_eq!(association.action.shot_id, shot_id);
    let LineagePayload::TokenAssociation(payload) = &association.action.payload else {
        panic!("fixture must contain a token association");
    };
    assert_eq!(payload.chain_id, BASE_CHAIN_ID);
    assert_eq!(payload.token, token);
    assert_ne!(shot_id.to_string(), token.to_string());

    let source_root = tempfile::tempdir().unwrap();
    let source_store = NodeStore::open(source_root.path()).unwrap();
    let outcome = source_store.ingest(&canonical_bytes).unwrap();
    assert!(outcome.stored);
    assert_eq!(outcome.shot_id, shot_id);
    assert_eq!(
        outcome.validation.signed_record,
        SignedRecordStatus::Verified
    );
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
    assert_eq!(
        outcome.validation.missing_parent,
        Some(private_parent_digest)
    );
    assert_eq!(source_store.health().unwrap().stored_actions, 1);
    assert!(!source_store.contains(private_parent_digest).unwrap());

    let source = Node::new(source_store, Vec::new()).unwrap();
    let (source_address, source_task) = spawn_node(source).await;
    let client = reqwest::Client::new();

    let action_response = client
        .get(format!(
            "http://{source_address}/v1/actions/{association_digest}"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(action_response.status(), StatusCode::OK);
    assert_eq!(
        action_response.bytes().await.unwrap().as_ref(),
        canonical_bytes.as_slice()
    );
    assert_eq!(
        client
            .get(format!(
                "http://{source_address}/v1/actions/{private_parent_digest}"
            ))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );

    let source_view: ShotView = client
        .get(format!("http://{source_address}/v1/shots/{shot_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(source_view.shot.shot_id, shot_id);
    assert_eq!(source_view.shot.action_count, 1);
    assert!(source_view.shot.roots.is_empty());
    assert_eq!(
        source_view.shot.missing_parents,
        vec![private_parent_digest]
    );
    assert_eq!(source_view.actions[0].shot_id, shot_id);
    assert_eq!(source_view.actions[0].digest, association_digest);
    assert_eq!(
        source_view.actions[0].validation.candidate_authority,
        AuthorityStatus::Unresolved
    );

    let destination_root = tempfile::tempdir().unwrap();
    let destination = Node::new(
        NodeStore::open(destination_root.path()).unwrap(),
        vec![Peer::parse(&format!("http://{source_address}")).unwrap()],
    )
    .unwrap();
    let report = destination.sync().await.unwrap();
    assert_eq!(report.state, SyncState::Succeeded);
    assert_eq!(report.peers[0].fetched, 1);
    assert_eq!(report.peers[0].rejected, 0);

    let destination_view = destination.store().shot(shot_id).unwrap();
    assert_eq!(destination_view.shot.shot_id, shot_id);
    assert_eq!(
        destination_view.shot.missing_parents,
        vec![private_parent_digest]
    );
    assert_eq!(destination_view.actions.len(), 1);
    let replicated = &destination_view.actions[0];
    assert_eq!(replicated.digest, association_digest);
    assert_eq!(
        replicated.validation.signed_record,
        SignedRecordStatus::Verified
    );
    assert_eq!(replicated.validation.segment, SegmentStatus::Verified);
    assert_eq!(
        replicated.validation.neutral_authority,
        AuthorityStatus::Unresolved
    );
    assert_eq!(
        replicated.validation.candidate_authority,
        AuthorityStatus::Unresolved
    );
    assert!(!replicated.validation.authority_context_available);
    assert_eq!(
        replicated.validation.missing_parent,
        Some(private_parent_digest)
    );
    assert_eq!(
        destination
            .store()
            .action_bytes(association_digest)
            .unwrap(),
        canonical_bytes
    );
    assert_eq!(destination.store().health().unwrap().stored_actions, 1);
    source_task.abort();
}

#[tokio::test]
async fn http_surface_is_bounded_and_rejects_private_replication() {
    let temporary = tempfile::tempdir().unwrap();
    let node = Node::new(NodeStore::open(temporary.path()).unwrap(), Vec::new()).unwrap();
    let (address, task) = spawn_node(node).await;
    let client = reqwest::Client::new();
    for path in [
        "v1/health",
        "v1/node",
        "v1/peers",
        "v1/shots",
        "v1/integrity",
        "v1/sync",
    ] {
        assert_eq!(
            client
                .get(format!("http://{address}/{path}"))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
    }
    let info: serde_json::Value = client
        .get(format!("http://{address}/v1/node"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        info.pointer("/contract_configuration/candidate_status")
            .and_then(serde_json::Value::as_str),
        Some("planned, undeployed, non-canonical and unaudited")
    );
    assert_eq!(
        info.pointer("/contract_configuration/builder_account_factory/deployed")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        info.pointer("/contract_configuration/chain_id")
            .and_then(serde_json::Value::as_u64),
        Some(4663)
    );
    assert_eq!(
        info.pointer("/contract_configuration/deployer_code_must_be_verified_before_broadcast")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert!(info
        .pointer("/contract_configuration/builder_account_factory/transaction_hash")
        .is_some_and(serde_json::Value::is_null));
    let authority_policy = info
        .pointer("/contract_configuration/initial_authority_policy")
        .and_then(serde_json::Value::as_str)
        .unwrap();
    assert!(authority_policy.contains("initial authority only"));
    assert!(authority_policy.contains("ownership-transfer authorization proof"));
    assert!(info
        .pointer("/agreement")
        .and_then(serde_json::Value::as_str)
        .unwrap()
        .contains("before ownership transfer"));

    let private = root_action(
        &TestKey::new(11),
        ShotId::from_bytes([0x62; 32]),
        AvailabilityStatus::IntentionallyPrivate,
    );
    assert_eq!(
        client
            .post(format!("http://{address}/v1/actions"))
            .header("content-type", "application/json")
            .body(bytes(&private))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        client
            .post(format!("http://{address}/v1/actions"))
            .header("content-type", "application/json")
            .body(vec![b'x'; MAX_ACTION_BYTES + 1])
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
    task.abort();
}

#[tokio::test]
async fn http_reports_transfer_branches_as_neutrally_valid_but_candidate_unresolved() {
    let temporary = tempfile::tempdir().unwrap();
    let node = Node::new(NodeStore::open(temporary.path()).unwrap(), Vec::new()).unwrap();
    let (address, task) = spawn_node(node).await;
    let client = reqwest::Client::new();
    let original_owner = TestKey::new(21);
    let next_owner = TestKey::new(22);
    let shot_id = ShotId::from_bytes([0x66; 32]);
    let root = root_action(
        &original_owner,
        shot_id,
        AvailabilityStatus::PubliclyAvailable,
    );
    let transfer = ownership_action(&original_owner, &next_owner, &root);
    let descendant = availability_after(
        &next_owner,
        &transfer,
        b"post-transfer public observation",
        AvailabilityStatus::Unknown,
        "post-transfer observation",
    );

    let root_response: serde_json::Value = client
        .post(format!("http://{address}/v1/actions"))
        .header("content-type", "application/json")
        .body(bytes(&root))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        root_response
            .pointer("/validation/candidate_authority")
            .and_then(serde_json::Value::as_str),
        Some("verified")
    );

    for action in [&transfer, &descendant] {
        let response: serde_json::Value = client
            .post(format!("http://{address}/v1/actions"))
            .header("content-type", "application/json")
            .body(bytes(action))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            response
                .pointer("/validation/neutral_authority")
                .and_then(serde_json::Value::as_str),
            Some("verified")
        );
        assert_eq!(
            response
                .pointer("/validation/candidate_authority")
                .and_then(serde_json::Value::as_str),
            Some("unresolved")
        );
        assert_eq!(
            response
                .pointer("/validation/authority_context_available")
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
        let detail = response
            .pointer("/validation/detail")
            .and_then(serde_json::Value::as_str)
            .unwrap();
        assert!(detail.contains("neutrally valid"));
        assert!(detail.contains("ownership-transfer authorization proof"));
    }

    let shot: serde_json::Value = client
        .get(format!("http://{address}/v1/shots/{shot_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        shot.pointer("/shot/validation/candidate_authority_verified")
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
    assert_eq!(
        shot.pointer("/shot/validation/candidate_authority_unresolved")
            .and_then(serde_json::Value::as_u64),
        Some(2)
    );
    task.abort();
}

#[tokio::test]
async fn oversized_malicious_peer_fails_without_importing_anything() {
    let info_root = tempfile::tempdir().unwrap();
    let info = NodeStore::open(info_root.path()).unwrap().info().unwrap();
    let oversized = vec![b' '; 4 * 1024 * 1024 + 1];
    let malicious = Router::new()
        .route(
            "/v1/node",
            get({
                let info = info.clone();
                move || {
                    let info = info.clone();
                    async move { Json(info) }
                }
            }),
        )
        .route(
            "/v1/shots",
            get(move || {
                let oversized = oversized.clone();
                async move { Response::new(Body::from(oversized)) }
            }),
        );
    let (address, task) = spawn_router(malicious).await;
    let victim_root = tempfile::tempdir().unwrap();
    let victim = Node::new(
        NodeStore::open(victim_root.path()).unwrap(),
        vec![Peer::parse(&format!("http://{address}")).unwrap()],
    )
    .unwrap();
    let report = victim.sync().await.unwrap();
    assert_eq!(report.state, SyncState::Failed);
    assert!(report.peers[0].error.as_deref().unwrap().contains("limit"));
    assert_eq!(victim.store().health().unwrap().stored_actions, 0);
    task.abort();
}

#[tokio::test]
async fn lying_peer_cannot_substitute_action_bytes_for_an_advertised_digest() {
    let info_root = tempfile::tempdir().unwrap();
    let info = NodeStore::open(info_root.path()).unwrap().info().unwrap();
    let key = TestKey::new(12);
    let advertised = root_action(
        &key,
        ShotId::from_bytes([0x63; 32]),
        AvailabilityStatus::PubliclyAvailable,
    );
    let substitute = root_action(
        &TestKey::new(13),
        ShotId::from_bytes([0x64; 32]),
        AvailabilityStatus::PubliclyAvailable,
    );
    let reference = ActionReference {
        digest: advertised.commitment().unwrap(),
        shot_id: advertised.action.shot_id,
        sequence: 1,
        previous: None,
        validation: fully_verified(),
    };
    let summary = ShotSummary {
        shot_id: advertised.action.shot_id,
        action_count: 1,
        roots: vec![reference.digest],
        observed_heads: vec![reference.digest],
        authority_verified_heads: vec![reference.digest],
        authority_unresolved_heads: Vec::new(),
        authority_rejected_heads: Vec::new(),
        validation: ValidationCounts {
            signed_records_verified: 1,
            segments_verified: 1,
            neutral_authority_verified: 1,
            candidate_authority_verified: 1,
            ..ValidationCounts::default()
        },
        missing_parents: Vec::new(),
        missing_artifacts: Vec::new(),
    };
    let view = ShotView {
        shot: summary.clone(),
        actions: vec![reference],
    };
    let malicious = Router::new()
        .route(
            "/v1/node",
            get({
                let info = info.clone();
                move || {
                    let info = info.clone();
                    async move { Json(info) }
                }
            }),
        )
        .route(
            "/v1/shots",
            get({
                let summary = summary.clone();
                move || {
                    let summary = summary.clone();
                    async move { Json(vec![summary]) }
                }
            }),
        )
        .route(
            "/v1/shots/{shot_id}",
            get({
                let view = view.clone();
                move |Path(_): Path<String>| {
                    let view = view.clone();
                    async move { Json(view) }
                }
            }),
        )
        .route(
            "/v1/actions/{digest}",
            get(move |Path(_): Path<String>| {
                let body = bytes(&substitute);
                async move { Response::new(Body::from(body)) }
            }),
        );
    let (address, task) = spawn_router(malicious).await;
    let victim_root = tempfile::tempdir().unwrap();
    let victim = Node::new(
        NodeStore::open(victim_root.path()).unwrap(),
        vec![Peer::parse(&format!("http://{address}")).unwrap()],
    )
    .unwrap();
    let report = victim.sync().await.unwrap();
    assert_eq!(report.state, SyncState::Failed);
    assert_eq!(report.peers[0].rejected, 1);
    assert_eq!(victim.store().health().unwrap().stored_actions, 0);
    task.abort();
}

#[tokio::test]
async fn sync_preserves_an_unanchored_public_segment_without_inventing_authority() {
    let source_root = tempfile::tempdir().unwrap();
    let source_store = NodeStore::open(source_root.path()).unwrap();
    let key = TestKey::new(16);
    let shot_id = ShotId::from_bytes([0x65; 32]);
    let root = root_action(&key, shot_id, AvailabilityStatus::PubliclyAvailable);
    let private_parent = availability_after_with_handling(
        &key,
        &root,
        b"private boundary",
        AvailabilityStatus::IntentionallyPrivate,
        "private boundary",
        AvailabilityStatus::IntentionallyPrivate,
    );
    let public_child = availability_after(
        &key,
        &private_parent,
        b"public continuation",
        AvailabilityStatus::Unknown,
        "public continuation",
    );
    let private_digest = private_parent.commitment().unwrap();
    let child_digest = public_child.commitment().unwrap();
    source_store.ingest(&bytes(&root)).unwrap();
    source_store.ingest(&bytes(&public_child)).unwrap();

    let source = Node::new(source_store, Vec::new()).unwrap();
    let (source_address, source_task) = spawn_node(source).await;
    let destination_root = tempfile::tempdir().unwrap();
    let destination = Node::new(
        NodeStore::open(destination_root.path()).unwrap(),
        vec![Peer::parse(&format!("http://{source_address}")).unwrap()],
    )
    .unwrap();
    let report = destination.sync().await.unwrap();
    assert_eq!(report.state, SyncState::Succeeded);
    assert_eq!(report.peers[0].fetched, 2);
    assert_eq!(report.peers[0].rejected, 0);

    let child = destination
        .store()
        .shot(shot_id)
        .unwrap()
        .actions
        .into_iter()
        .find(|action| action.digest == child_digest)
        .unwrap();
    assert_eq!(child.validation.segment, SegmentStatus::Verified);
    assert_eq!(
        child.validation.neutral_authority,
        AuthorityStatus::Unresolved
    );
    assert_eq!(
        child.validation.candidate_authority,
        AuthorityStatus::Unresolved
    );
    assert_eq!(child.validation.missing_parent, Some(private_digest));
    assert_eq!(
        destination
            .store()
            .integrity()
            .unwrap()
            .missing_parent_count,
        1
    );
    source_task.abort();
}

fn fully_verified() -> ActionValidation {
    ActionValidation {
        signed_record: SignedRecordStatus::Verified,
        segment: SegmentStatus::Verified,
        neutral_authority: AuthorityStatus::Verified,
        candidate_authority: AuthorityStatus::Verified,
        authority_context_available: true,
        missing_parent: None,
        detail: None,
    }
}
