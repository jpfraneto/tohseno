use crate::{Node, NodeError, Result, MAX_ACTION_BYTES};
use axum::body::{Body, Bytes};
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use tohseno_protocol::digest::{Bytes32, ShotId};
use tokio::net::TcpListener;

pub fn router(node: Node) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/node", get(node_info))
        .route("/v1/peers", get(peers))
        .route("/v1/shots", get(shots))
        .route("/v1/shots/{shot_id}", get(shot))
        .route("/v1/actions/{digest}", get(action))
        .route("/v1/actions", post(ingest))
        .route("/v1/integrity", get(integrity))
        .route("/v1/sync", get(sync_status).post(trigger_sync))
        .layer(DefaultBodyLimit::max(MAX_ACTION_BYTES))
        .with_state(node)
}

pub async fn serve(node: Node, listener: TcpListener) -> Result<()> {
    axum::serve(listener, router(node))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .map_err(NodeError::Io)
}

async fn health(State(node): State<Node>) -> ApiResult<impl IntoResponse> {
    Ok(Json(node.store().health()?))
}

async fn node_info(State(node): State<Node>) -> ApiResult<impl IntoResponse> {
    Ok(Json(node.store().info()?))
}

async fn peers(State(node): State<Node>) -> ApiResult<impl IntoResponse> {
    Ok(Json(node.peers()))
}

async fn shots(State(node): State<Node>) -> ApiResult<impl IntoResponse> {
    Ok(Json(node.store().shots()?))
}

async fn shot(
    State(node): State<Node>,
    Path(shot_id): Path<String>,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(node.store().shot(parse_shot_id(&shot_id)?)?))
}

async fn action(
    State(node): State<Node>,
    Path(digest): Path<String>,
) -> ApiResult<impl IntoResponse> {
    let digest = Bytes32::from_hex("action_digest", &digest)?;
    let bytes = node.store().action_bytes(digest)?;
    let mut response = Response::new(Body::from(bytes));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, immutable, max-age=31536000"),
    );
    response.headers_mut().insert(
        header::ETAG,
        HeaderValue::from_str(&format!("\"{digest}\"")).unwrap(),
    );
    Ok(response)
}

async fn ingest(State(node): State<Node>, body: Bytes) -> ApiResult<impl IntoResponse> {
    let outcome = node.store().ingest(&body)?;
    let status = if outcome.stored {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(outcome)))
}

async fn integrity(State(node): State<Node>) -> ApiResult<impl IntoResponse> {
    Ok(Json(node.store().integrity()?))
}

async fn sync_status(State(node): State<Node>) -> ApiResult<impl IntoResponse> {
    Ok(Json(node.sync_status().await))
}

async fn trigger_sync(State(node): State<Node>) -> ApiResult<impl IntoResponse> {
    Ok(Json(node.sync().await?))
}

fn parse_shot_id(value: &str) -> Result<ShotId> {
    serde_json::from_str(&format!("\"{value}\"")).map_err(NodeError::Json)
}

type ApiResult<T> = std::result::Result<T, ApiError>;

struct ApiError(NodeError);

impl From<NodeError> for ApiError {
    fn from(value: NodeError) -> Self {
        Self(value)
    }
}

impl From<tohseno_protocol::ProtocolError> for ApiError {
    fn from(value: tohseno_protocol::ProtocolError) -> Self {
        Self(NodeError::from(value))
    }
}

#[derive(Serialize)]
struct ErrorBody {
    code: &'static str,
    error: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code) = match &self.0 {
            NodeError::ActionTooLarge { .. } | NodeError::PeerResponseTooLarge { .. } => {
                (StatusCode::PAYLOAD_TOO_LARGE, "limit_exceeded")
            }
            NodeError::ActionMissing(_) | NodeError::ShotMissing(_) => {
                (StatusCode::NOT_FOUND, "not_found")
            }
            NodeError::SyncInProgress => (StatusCode::CONFLICT, "sync_in_progress"),
            NodeError::NotPublic
            | NodeError::Protocol(_)
            | NodeError::Causal(_)
            | NodeError::ContentCollision => (StatusCode::UNPROCESSABLE_ENTITY, "invalid_action"),
            NodeError::InvalidPeer(_) | NodeError::PeerLimit { .. } => {
                (StatusCode::BAD_REQUEST, "invalid_peer")
            }
            NodeError::Json(_) => (StatusCode::BAD_REQUEST, "invalid_json"),
            NodeError::PeerRequest(_) | NodeError::PeerResponse(_) => {
                (StatusCode::BAD_GATEWAY, "peer_failure")
            }
            NodeError::ActionLimit { .. } => (StatusCode::INSUFFICIENT_STORAGE, "storage_limit"),
            NodeError::Io(_) | NodeError::UnsafeStorage(_) | NodeError::LockPoisoned => {
                (StatusCode::INTERNAL_SERVER_ERROR, "internal")
            }
        };
        (
            status,
            Json(ErrorBody {
                code,
                error: self.0.to_string(),
            }),
        )
            .into_response()
    }
}
