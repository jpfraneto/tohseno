use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tohseno_engine::{Engine, Event, EventBus, Ledger};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinSet;

const INDEX: &str = include_str!("../../studio/index.html");
const STYLE: &str = include_str!("../../studio/style.css");
const SCRIPT: &str = include_str!("../../studio/app.js");
const CORE_CIRCLE: &[u8] = include_bytes!("../../brand/logos/tohseno-core-circle.svg");
const MAX_HEADERS: usize = 32 * 1024;
const MAX_BODY: usize = 1024 * 1024;

#[derive(Clone)]
struct State {
    events: EventBus,
    authority: String,
    origin: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InitializeRequest {
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordRequest {
    app_name: String,
    #[serde(default)]
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenRequest {
    app_name: String,
}

#[derive(Debug, Serialize)]
struct LibraryResponse {
    apps: Vec<LibraryApp>,
}

#[derive(Debug, Serialize)]
struct LibraryApp {
    name: String,
    folder: String,
    latest_version: Option<u32>,
    versions: Vec<u32>,
    has_unrecorded_changes: bool,
    needs_attention: bool,
    read_only: bool,
}

pub async fn serve(port: u16, events: EventBus) -> Result<(), Box<dyn std::error::Error>> {
    serve_inner(port, events).await
}

async fn serve_inner(port: u16, events: EventBus) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(("127.0.0.1", port)).await?;
    let address = listener.local_addr()?;
    let origin = format!("http://127.0.0.1:{}", address.port());
    events.emit(Event::status(format!("Studio is ready at {origin}.")));
    if std::env::var("TOHSENO_STUDIO_NO_OPEN").as_deref() != Ok("1") {
        let _ = std::process::Command::new("open").arg(&origin).spawn();
    }
    let state = State {
        events,
        authority: format!("127.0.0.1:{}", address.port()),
        origin,
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
    if request.header("host") != Some(state.authority.as_str()) {
        return respond(
            &mut socket,
            403,
            "text/plain; charset=utf-8",
            b"forbidden host",
        )
        .await;
    }
    if request.method == "POST" && !valid_mutation(&request, &state.origin) {
        return respond(
            &mut socket,
            403,
            "text/plain; charset=utf-8",
            b"same-origin Studio JSON request required",
        )
        .await;
    }
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => {
            respond(
                &mut socket,
                200,
                "text/html; charset=utf-8",
                INDEX.as_bytes(),
            )
            .await?
        }
        ("GET", "/style.css") => {
            respond(
                &mut socket,
                200,
                "text/css; charset=utf-8",
                STYLE.as_bytes(),
            )
            .await?
        }
        ("GET", "/app.js") => {
            respond(
                &mut socket,
                200,
                "text/javascript; charset=utf-8",
                SCRIPT.as_bytes(),
            )
            .await?
        }
        ("GET", "/brand/logos/tohseno-core-circle.svg") => {
            respond(&mut socket, 200, "image/svg+xml", CORE_CIRCLE).await?
        }
        ("GET", "/api/studio-instance") => {
            respond(
                &mut socket,
                200,
                "application/json; charset=utf-8",
                br#"{"local":true}"#,
            )
            .await?
        }
        ("GET", "/api/apps") => serve_apps(&mut socket, &state).await?,
        ("POST", "/api/apps") => initialize_app(&mut socket, &request.body, &state).await?,
        ("POST", "/api/versions") => record_version(&mut socket, &request.body, &state).await?,
        ("POST", "/api/open") => open_folder(&mut socket, &request.body).await?,
        _ => respond(&mut socket, 404, "text/plain; charset=utf-8", b"not found").await?,
    }
    Ok(())
}

async fn serve_apps(
    socket: &mut TcpStream,
    state: &State,
) -> Result<(), Box<dyn std::error::Error>> {
    let engine = Engine::discover(state.events.clone())?;
    let ledger = engine.ledger();
    let mut apps = Vec::new();
    for app in ledger.list_apps()? {
        let versions = ledger
            .list_evolutions(&app.name)?
            .into_iter()
            .map(|version| version.number)
            .collect::<Vec<_>>();
        let read_only = !ledger
            .working_tree(&app.name)
            .join(".tohseno/recording-layer-v1")
            .is_file();
        let change_state = if read_only {
            Ok(false)
        } else {
            has_unrecorded_changes(&engine, &app.name)
        };
        apps.push(LibraryApp {
            has_unrecorded_changes: change_state.as_ref().copied().unwrap_or(false),
            needs_attention: change_state.is_err(),
            read_only,
            folder: ledger.working_tree(&app.name).display().to_string(),
            latest_version: app.latest_evolution,
            versions,
            name: app.name,
        });
    }
    let body = serde_json::to_vec(&LibraryResponse { apps })?;
    respond(socket, 200, "application/json; charset=utf-8", &body).await
}

fn has_unrecorded_changes(
    engine: &Engine,
    app_name: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let ledger = engine.ledger();
    let working = tohseno_engine::ledger::hash_app_tree(&ledger.working_tree(app_name))?;
    let Some(version) = ledger.latest_evolution(app_name)? else {
        return Ok(!working.entries.is_empty());
    };
    if !version.path.join("tree.sha256").is_file() {
        return Ok(false);
    }
    let recorded = engine.verify_recorded_version(&version)?;
    Ok(working.digest != recorded)
}

async fn initialize_app(
    socket: &mut TcpStream,
    body: &[u8],
    state: &State,
) -> Result<(), Box<dyn std::error::Error>> {
    let request: InitializeRequest = match serde_json::from_slice(body) {
        Ok(request) => request,
        Err(error) => return respond_error(socket, 400, &error.to_string()).await,
    };
    let engine = match Engine::discover(state.events.clone()) {
        Ok(engine) => engine,
        Err(error) => return respond_error(socket, 422, &error.to_string()).await,
    };
    let folder = match engine.initialize_app(&request.name) {
        Ok(folder) => folder,
        Err(error) => return respond_error(socket, 422, &error.to_string()).await,
    };
    let body = serde_json::to_vec(&serde_json::json!({
        "name": request.name,
        "folder": folder,
    }))?;
    respond(socket, 201, "application/json; charset=utf-8", &body).await
}

async fn record_version(
    socket: &mut TcpStream,
    body: &[u8],
    state: &State,
) -> Result<(), Box<dyn std::error::Error>> {
    let request: RecordRequest = match serde_json::from_slice(body) {
        Ok(request) => request,
        Err(error) => return respond_error(socket, 400, &error.to_string()).await,
    };
    let engine = match Engine::discover(state.events.clone()) {
        Ok(engine) => engine,
        Err(error) => return respond_error(socket, 422, &error.to_string()).await,
    };
    let version = match engine.record_version(&request.app_name, request.note.as_deref()) {
        Ok(version) => version,
        Err(error) => return respond_error(socket, 422, &error.to_string()).await,
    };
    let body = serde_json::to_vec(&serde_json::json!({ "version": version.number }))?;
    respond(socket, 201, "application/json; charset=utf-8", &body).await
}

async fn open_folder(
    socket: &mut TcpStream,
    body: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let request: OpenRequest = match serde_json::from_slice(body) {
        Ok(request) => request,
        Err(error) => return respond_error(socket, 400, &error.to_string()).await,
    };
    if let Err(error) = tohseno_engine::ledger::validate_app_name(&request.app_name) {
        return respond_error(socket, 422, &error.to_string()).await;
    }
    let ledger = match Ledger::discover() {
        Ok(ledger) => ledger,
        Err(error) => return respond_error(socket, 422, &error.to_string()).await,
    };
    if let Err(error) = ledger.load_app(&request.app_name) {
        return respond_error(socket, 422, &error.to_string()).await;
    }
    if let Err(error) = std::process::Command::new("open")
        .arg(ledger.working_tree(&request.app_name))
        .spawn()
    {
        return respond_error(socket, 422, &error.to_string()).await;
    }
    respond(
        socket,
        200,
        "application/json; charset=utf-8",
        br#"{"opened":true}"#,
    )
    .await
}

fn valid_mutation(request: &Request, origin: &str) -> bool {
    request.header("origin") == Some(origin)
        && request.header("x-tohseno-studio") == Some("1")
        && request
            .header("content-type")
            .is_some_and(|value| value.starts_with("application/json"))
}

struct Request {
    method: String,
    path: String,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).map(String::as_str)
    }
}

async fn read_request(socket: &mut TcpStream) -> Result<Request, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 8192];
    let header_end = loop {
        let read = socket.read(&mut buffer).await?;
        if read == 0 {
            return Err("connection closed before request headers".into());
        }
        bytes.extend_from_slice(&buffer[..read]);
        if bytes.len() > MAX_HEADERS {
            return Err("request headers are too large".into());
        }
        if let Some(end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break end + 4;
        }
    };
    let head = std::str::from_utf8(&bytes[..header_end])?;
    let mut lines = head.split("\r\n");
    let mut request_line = lines
        .next()
        .ok_or("missing request line")?
        .split_whitespace();
    let method = request_line.next().ok_or("missing method")?.to_owned();
    let raw_path = request_line.next().ok_or("missing path")?;
    let path = raw_path.split('?').next().unwrap_or(raw_path).to_owned();
    let mut headers = BTreeMap::new();
    for line in lines.filter(|line| !line.is_empty()) {
        let (name, value) = line.split_once(':').ok_or("invalid request header")?;
        headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_owned());
    }
    let length = headers
        .get("content-length")
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(0);
    if length > MAX_BODY {
        return Err("request body is too large".into());
    }
    let mut body = bytes[header_end..].to_vec();
    while body.len() < length {
        let read = socket.read(&mut buffer).await?;
        if read == 0 {
            return Err("connection closed before request body".into());
        }
        body.extend_from_slice(&buffer[..read]);
    }
    body.truncate(length);
    Ok(Request {
        method,
        path,
        headers,
        body,
    })
}

async fn respond(
    socket: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let reason = match status {
        200 => "OK",
        201 => "Created",
        400 => "Bad Request",
        422 => "Unprocessable Content",
        403 => "Forbidden",
        404 => "Not Found",
        _ => "Error",
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    socket.write_all(head.as_bytes()).await?;
    socket.write_all(body).await?;
    socket.shutdown().await?;
    Ok(())
}

async fn respond_error(
    socket: &mut TcpStream,
    status: u16,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let body = serde_json::to_vec(&serde_json::json!({ "error": message }))?;
    respond(socket, status, "application/json; charset=utf-8", &body).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_bodies_are_closed_to_factory_fields() {
        assert!(serde_json::from_str::<InitializeRequest>(r#"{"name":"anky"}"#).is_ok());
        assert!(serde_json::from_str::<RecordRequest>(
            r#"{"app_name":"anky","note":"first useful state"}"#
        )
        .is_ok());
        for field in [
            "intention",
            "references",
            "prompt",
            "images",
            "harness",
            "model",
            "route",
        ] {
            let create = format!(r#"{{"name":"anky","{field}":"x"}}"#);
            let record = format!(r#"{{"app_name":"anky","{field}":"x"}}"#);
            assert!(serde_json::from_str::<InitializeRequest>(&create).is_err());
            assert!(serde_json::from_str::<RecordRequest>(&record).is_err());
        }
    }

    #[test]
    fn studio_source_has_no_factory_routes() {
        let source = include_str!("studio_server.rs");
        for route in [
            ["/", "shots"].concat(),
            ["/api/", "executions"].concat(),
            ["/api/", "install"].concat(),
            ["/api/", "simulator"].concat(),
        ] {
            assert!(!source.contains(&route), "obsolete route remains: {route}");
        }
    }

    #[test]
    fn mutation_requires_local_origin_json_and_studio_header() {
        let request = Request {
            method: "POST".into(),
            path: "/api/versions".into(),
            headers: BTreeMap::from([
                ("origin".into(), "http://127.0.0.1:8888".into()),
                ("content-type".into(), "application/json".into()),
                ("x-tohseno-studio".into(), "1".into()),
            ]),
            body: Vec::new(),
        };
        assert!(valid_mutation(&request, "http://127.0.0.1:8888"));
        assert!(!valid_mutation(&request, "http://127.0.0.1:9999"));
    }

    #[test]
    fn bundled_frontend_names_only_the_recording_loop() {
        for marker in ["Apps", "Versions", "Open folder", "Record version"] {
            assert!(INDEX.contains(marker), "missing Studio marker: {marker}");
        }
        for removed in ["Preview", "Install", "Evolve app", "Harness", "Bankr"] {
            assert!(
                !INDEX.contains(removed),
                "obsolete Studio marker: {removed}"
            );
        }
    }
}
