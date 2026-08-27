//! Verified loopback client used by CLI administration and product commands.

use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE, ORIGIN};
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::service_commands::{self, ServicePaths, SystemLaunchctl};
use crate::workspace_identity::KEYCHAIN_NOTICE;
use crate::workspace_service::{load_runtime, RuntimeRecord};

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// A long native build can briefly starve loopback responses on smaller Macs.
/// Execution status reads are idempotent, so tolerate a bounded run of
/// transport failures without turning healthy, durable work into a CLI error.
const MAX_CONSECUTIVE_EXECUTION_POLL_TRANSPORT_ERRORS: u8 = 6;

#[derive(Clone)]
pub struct ServiceClient {
    http: Client,
    runtime: RuntimeRecord,
}

impl ServiceClient {
    pub async fn connect() -> Result<Self, BoxError> {
        let paths = ServicePaths::discover().map_err(boxed)?;
        Self::connect_at(&paths.service_state).await
    }

    pub async fn connect_at(service_root: &Path) -> Result<Self, BoxError> {
        let runtime = load_runtime(service_root)?;
        let http = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(10))
            .build()?;
        let client = Self { http, runtime };
        client.verify_health().await?;
        Ok(client)
    }

    pub async fn ensure_running() -> Result<Self, BoxError> {
        if let Ok(client) = Self::connect().await {
            return Ok(client);
        }
        let paths = ServicePaths::discover().map_err(boxed)?;
        let error_log = paths.logs.join("workspace-service.error.log");
        // Only what this attempt writes can explain this attempt.
        let already_written = std::fs::metadata(&error_log)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        if std::env::var("TOHSENO_DEVELOPMENT_SERVICE").as_deref() == Ok("1") {
            let executable = std::env::current_exe()?;
            Command::new(executable)
                .args(["service", "run"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?;
        } else {
            service_commands::start(&paths, &SystemLaunchctl).map_err(boxed)?;
        }
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut last_error = String::from("service health did not become available");
        while Instant::now() < deadline {
            match Self::connect_at(&paths.service_state).await {
                Ok(client) => return Ok(client),
                Err(error) => last_error = error.to_string(),
            }
            tokio::time::sleep(Duration::from_millis(125)).await;
        }
        // A service that never answered is usually blocked rather than broken,
        // and the transport error only describes the symptom.
        if let Some(blocker) = startup_blocker(&error_log, already_written) {
            return Err(blocker.into());
        }
        Err(format!("Local Workspace Service did not become healthy: {last_error}").into())
    }

    pub fn runtime(&self) -> &RuntimeRecord {
        &self.runtime
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, BoxError> {
        let response = self.http.get(self.url(path)?).send().await?;
        decode(response).await
    }

    pub async fn post<T: Serialize, R: DeserializeOwned>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<R, BoxError> {
        let response = self
            .http
            .post(self.url(path)?)
            .headers(self.mutation_headers()?)
            .json(body)
            .send()
            .await?;
        decode(response).await
    }

    /// Repeat an exact command body once when the response is ambiguous. The
    /// create/evolve command ID is content-derived and the service journal is
    /// idempotent, so this recovers a receipt without admitting duplicate
    /// human work when the first response crosses the client timeout.
    pub async fn post_durable<T: Serialize, R: DeserializeOwned>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<R, BoxError> {
        match self.post(path, body).await {
            Ok(receipt) => Ok(receipt),
            Err(error) if is_ambiguous_transport_error(&error) => {
                match self.post(path, body).await {
                    Ok(receipt) => Ok(receipt),
                    Err(retry) if is_ambiguous_transport_error(&retry) => Err(std::io::Error::other(
                        format!(
                            "could not confirm durable command admission after an idempotent retry: {retry}"
                        ),
                    )
                    .into()),
                    Err(retry) => Err(retry),
                }
            }
            Err(error) => Err(error),
        }
    }

    pub async fn delete<R: DeserializeOwned>(&self, path: &str) -> Result<R, BoxError> {
        let response = self
            .http
            .delete(self.url(path)?)
            .headers(self.mutation_headers()?)
            .body("{}")
            .send()
            .await?;
        decode(response).await
    }

    #[cfg(test)]
    pub async fn wait_for_execution(&self, execution_id: &str) -> Result<Value, BoxError> {
        self.wait_for_execution_with_updates(execution_id, |_| {})
            .await
    }

    pub async fn execution_status(&self, execution_id: &str) -> Result<Value, BoxError> {
        self.get(&format!("/api/v1/executions/{execution_id}"))
            .await
    }

    pub async fn wait_for_execution_with_updates<F>(
        &self,
        execution_id: &str,
        mut update: F,
    ) -> Result<Value, BoxError>
    where
        F: FnMut(&Value),
    {
        let mut consecutive_transport_errors = 0_u8;
        loop {
            let value: Value = match self.execution_status(execution_id).await {
                Ok(value) => {
                    consecutive_transport_errors = 0;
                    value
                }
                Err(error)
                    if is_ambiguous_transport_error(&error)
                        && consecutive_transport_errors
                            < MAX_CONSECUTIVE_EXECUTION_POLL_TRANSPORT_ERRORS =>
                {
                    consecutive_transport_errors += 1;
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    continue;
                }
                Err(error) => return Err(error),
            };
            update(&value);
            if value.get("complete").and_then(Value::as_bool) == Some(true) {
                if value.get("accepted").and_then(Value::as_bool) == Some(true) {
                    return Ok(value);
                }
                return Err(format!(
                    "execution {execution_id} finished without an accepted Version"
                )
                .into());
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    pub fn open_studio(&self, route: &str) -> Result<(), BoxError> {
        let url = self.url(route)?;
        let status = Command::new("/usr/bin/open")
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err("macOS could not open TOHSENO Studio".into())
        }
    }

    async fn verify_health(&self) -> Result<(), BoxError> {
        let health: Value = self.get_unverified("/api/v1/health").await?;
        let expected = [
            ("schema", "tohseno.local-workspace-health/1"),
            ("status", "healthy"),
            ("workspace_id", self.runtime.workspace_id.as_str()),
            ("studio_device_id", self.runtime.studio_device_id.as_str()),
            ("origin", self.runtime.origin.as_str()),
            ("instance_id", self.runtime.instance_id.as_str()),
            ("service_version", env!("CARGO_PKG_VERSION")),
        ];
        if expected
            .iter()
            .any(|(field, value)| health.get(*field).and_then(Value::as_str) != Some(*value))
        {
            return Err("Local Workspace Service identity or version did not verify".into());
        }
        Ok(())
    }

    async fn get_unverified<T: DeserializeOwned>(&self, path: &str) -> Result<T, BoxError> {
        let response = self.http.get(self.url(path)?).send().await?;
        decode(response).await
    }

    fn mutation_headers(&self) -> Result<HeaderMap, BoxError> {
        let mut headers = HeaderMap::new();
        headers.insert(ORIGIN, HeaderValue::from_str(&self.runtime.origin)?);
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            "x-tohseno-csrf",
            HeaderValue::from_str(&self.runtime.csrf_token)?,
        );
        Ok(headers)
    }

    fn url(&self, path: &str) -> Result<String, BoxError> {
        if !path.starts_with('/')
            || path.starts_with("//")
            || path.contains('\r')
            || path.contains('\n')
        {
            return Err("invalid Local Workspace Service route".into());
        }
        Ok(format!("{}{}", self.runtime.origin, path))
    }
}

async fn decode<T: DeserializeOwned>(response: reqwest::Response) -> Result<T, BoxError> {
    if !response.status().is_success() {
        return Err(response_error(response).await.into());
    }
    Ok(response.json().await?)
}

async fn response_error(response: reqwest::Response) -> String {
    let status = response.status();
    let body = response.json::<Value>().await.ok();
    let message = body
        .as_ref()
        .and_then(|value| value.get("message"))
        .and_then(Value::as_str)
        .unwrap_or("Local Workspace Service rejected the request");
    format!("{message} ({status})")
}

fn boxed(error: Box<dyn std::error::Error>) -> BoxError {
    std::io::Error::other(error.to_string()).into()
}

fn is_ambiguous_transport_error(error: &BoxError) -> bool {
    error.downcast_ref::<reqwest::Error>().is_some()
}

/// The stated reason this start attempt is still waiting, read only from what
/// the attempt itself appended. Returns nothing when the service failed for
/// some reason it could not narrate.
fn startup_blocker(error_log: &Path, already_written: u64) -> Option<String> {
    let bytes = std::fs::read(error_log).ok()?;
    let appended = bytes.get(usize::try_from(already_written).ok()?..)?;
    String::from_utf8_lossy(appended)
        .lines()
        .rev()
        .find(|line| line.contains(KEYCHAIN_NOTICE))
        .map(ToOwned::to_owned)
}

#[derive(Debug, Serialize)]
pub struct ApiReference<'a> {
    pub filename: &'a str,
    pub media_type: &'a str,
    pub origin: &'a str,
    pub bytes_base64url: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn durable_post_recovers_the_receipt_after_the_first_response_times_out() {
        use axum::{extract::State, routing::post, Json, Router};
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };

        async fn command(State(requests): State<Arc<AtomicUsize>>) -> Json<serde_json::Value> {
            if requests.fetch_add(1, Ordering::SeqCst) == 0 {
                tokio::time::sleep(Duration::from_millis(80)).await;
            }
            Json(serde_json::json!({"execution_id": "execution_fixture"}))
        }

        let requests = Arc::new(AtomicUsize::new(0));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server_requests = requests.clone();
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new()
                    .route("/command", post(command))
                    .with_state(server_requests),
            )
            .await
            .unwrap();
        });
        let client = ServiceClient {
            http: Client::builder()
                .timeout(Duration::from_millis(30))
                .build()
                .unwrap(),
            runtime: RuntimeRecord {
                schema: "tohseno.local-workspace-runtime/1".into(),
                service_version: "0.9.0".into(),
                workspace_id: "workspace_fixture".into(),
                studio_device_id: "device_fixture".into(),
                origin: format!("http://{address}"),
                port: address.port(),
                process_id: 1,
                started_at: "2026-08-15T12:00:00Z".into(),
                instance_id: "service_fixture".into(),
                csrf_token: "x".repeat(32),
            },
        };

        let receipt: serde_json::Value = client
            .post_durable("/command", &serde_json::json!({"command_id": "stable"}))
            .await
            .unwrap();
        assert_eq!(receipt["execution_id"], "execution_fixture");
        assert_eq!(requests.load(Ordering::SeqCst), 2);
        server.abort();
    }

    #[tokio::test]
    async fn execution_wait_tolerates_a_transient_loopback_timeout() {
        use axum::{extract::State, routing::get, Json, Router};
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };

        async fn execution(State(requests): State<Arc<AtomicUsize>>) -> Json<serde_json::Value> {
            if requests.fetch_add(1, Ordering::SeqCst) == 0 {
                tokio::time::sleep(Duration::from_millis(80)).await;
            }
            Json(serde_json::json!({
                "complete": true,
                "accepted": true,
                "execution_id": "execution_fixture"
            }))
        }

        let requests = Arc::new(AtomicUsize::new(0));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server_requests = requests.clone();
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new()
                    .route("/api/v1/executions/execution_fixture", get(execution))
                    .with_state(server_requests),
            )
            .await
            .unwrap();
        });
        let client = ServiceClient {
            http: Client::builder()
                .timeout(Duration::from_millis(30))
                .build()
                .unwrap(),
            runtime: RuntimeRecord {
                schema: "tohseno.local-workspace-runtime/1".into(),
                service_version: "0.9.0".into(),
                workspace_id: "workspace_fixture".into(),
                studio_device_id: "device_fixture".into(),
                origin: format!("http://{address}"),
                port: address.port(),
                process_id: 1,
                started_at: "2026-08-15T12:00:00Z".into(),
                instance_id: "service_fixture".into(),
                csrf_token: "x".repeat(32),
            },
        };

        let result = client
            .wait_for_execution("execution_fixture")
            .await
            .unwrap();
        assert_eq!(result["accepted"], true);
        assert_eq!(requests.load(Ordering::SeqCst), 2);
        server.abort();
    }

    #[test]
    fn an_unanswered_keychain_dialog_is_reported_instead_of_the_transport_error() {
        let directory = tempfile::tempdir().unwrap();
        let log = directory.path().join("workspace-service.error.log");
        std::fs::write(&log, format!("{KEYCHAIN_NOTICE}\n")).unwrap();
        assert_eq!(startup_blocker(&log, 0).unwrap(), KEYCHAIN_NOTICE);
    }

    #[test]
    fn an_earlier_run_s_keychain_dialog_never_explains_this_one() {
        let directory = tempfile::tempdir().unwrap();
        let log = directory.path().join("workspace-service.error.log");
        let stale = format!("{KEYCHAIN_NOTICE}\n");
        std::fs::write(&log, &stale).unwrap();
        assert!(startup_blocker(&log, stale.len() as u64).is_none());
    }

    #[test]
    fn a_failure_the_service_could_not_narrate_stays_unexplained() {
        let directory = tempfile::tempdir().unwrap();
        let log = directory.path().join("workspace-service.error.log");
        std::fs::write(&log, b"thread 'main' panicked\n").unwrap();
        assert!(startup_blocker(&log, 0).is_none());
        assert!(startup_blocker(&directory.path().join("absent.log"), 0).is_none());
    }

    #[test]
    fn routes_cannot_escape_the_verified_origin() {
        let client = ServiceClient {
            http: Client::new(),
            runtime: RuntimeRecord {
                schema: "tohseno.local-workspace-runtime/1".into(),
                service_version: "0.9.0".into(),
                workspace_id: "workspace_fixture".into(),
                studio_device_id: "device_fixture".into(),
                origin: "http://127.0.0.1:8888".into(),
                port: 8888,
                process_id: 1,
                started_at: "2026-08-15T12:00:00Z".into(),
                instance_id: "service_fixture".into(),
                csrf_token: "x".repeat(32),
            },
        };
        assert!(client.url("https://example.com").is_err());
        assert!(client.url("//example.com").is_err());
        assert_eq!(
            client.url("/api/v1/health").unwrap(),
            "http://127.0.0.1:8888/api/v1/health"
        );
    }
}
