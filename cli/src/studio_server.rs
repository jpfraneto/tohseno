use crate::simulator::{self, SimulatorSession};
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tohseno_engine::gates::intent::Intent;
use tohseno_engine::{Engine, Event, EventBus, Ledger, ShotRequest};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::task::JoinSet;

const INDEX: &str = include_str!("../../studio/index.html");
const STYLE: &str = include_str!("../../studio/style.css");
const SCRIPT: &str = include_str!("../../studio/app.js");
const MAX_BODY: usize = 160 * 1024 * 1024;

#[derive(Clone)]
struct State {
    events: EventBus,
    press: Arc<Mutex<()>>,
    simulator: Arc<Mutex<Option<SimulatorSession>>>,
}

#[derive(Debug, Deserialize)]
struct ShotSubmission {
    mode: ShotMode,
    app_name: String,
    prompt: String,
    #[serde(default)]
    images: Vec<UploadedImage>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ShotMode {
    Create,
    Evolve,
}

#[derive(Debug, Deserialize)]
struct UploadedImage {
    name: String,
    data: String,
}

#[derive(Debug, Deserialize)]
struct SimulatorLaunch {
    app_name: String,
    shot: u32,
}

#[derive(Debug, Serialize)]
struct LibraryResponse {
    apps: Vec<LibraryApp>,
    iphone_slots_used: usize,
    iphone_slot_limit: usize,
}

#[derive(Debug, Serialize)]
struct LibraryApp {
    name: String,
    latest_shot: u32,
    shots: Vec<u32>,
    retired: bool,
    icon_url: String,
}

pub async fn serve(port: u16, events: EventBus) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(("127.0.0.1", port)).await?;
    let address = listener.local_addr()?;
    let url = format!("http://127.0.0.1:{}", address.port());
    events.emit(Event::status(format!("studio is ready at {url}.")));
    let _ = std::process::Command::new("open").arg(&url).spawn();
    let state = State {
        events,
        press: Arc::new(Mutex::new(())),
        simulator: Arc::new(Mutex::new(None)),
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
    if request.method == "GET" && request.path == "/api/apps" {
        return serve_library(&mut socket).await;
    }
    if request.method == "GET" && request.path.starts_with("/api/icon/") {
        return serve_icon(&mut socket, &request.path).await;
    }
    if request.method == "POST" && request.path == "/api/simulator/launch" {
        return launch_simulator(&mut socket, &request.body, &state).await;
    }
    if request.method == "GET" && request.path == "/api/simulator/screen" {
        return serve_simulator_screen(&mut socket, &state).await;
    }
    if request.method == "POST" && request.path == "/api/simulator/focus" {
        let _ = std::process::Command::new("open")
            .args(["-a", "Simulator"])
            .spawn();
        respond(
            &mut socket,
            200,
            "application/json; charset=utf-8",
            r#"{"focused":true}"#,
        )
        .await?;
        return Ok(());
    }

    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => respond(&mut socket, 200, "text/html; charset=utf-8", INDEX).await?,
        ("GET", "/style.css") => {
            respond(&mut socket, 200, "text/css; charset=utf-8", STYLE).await?
        }
        ("GET", "/app.js") => {
            respond(&mut socket, 200, "text/javascript; charset=utf-8", SCRIPT).await?
        }
        ("GET", "/events") => stream_events(socket, state.events).await?,
        ("POST", "/shots") => {
            let submission: ShotSubmission = match serde_json::from_slice(&request.body) {
                Ok(submission) => submission,
                Err(error) => {
                    respond(
                        &mut socket,
                        400,
                        "text/plain; charset=utf-8",
                        &format!("invalid shot: {error}"),
                    )
                    .await?;
                    return Ok(());
                }
            };
            let staging = tempfile::tempdir()?;
            let image_paths = stage_images(staging.path(), submission.images).await?;
            respond(
                &mut socket,
                202,
                "application/json; charset=utf-8",
                r#"{"accepted":true}"#,
            )
            .await?;
            socket.shutdown().await?;

            let events = state.events.clone();
            let press = state.press.clone();
            let _staging = staging;
            let _guard = press.lock().await;
            let request = ShotRequest {
                app_name: submission.app_name,
                intent: Intent::parse(&submission.prompt).with_images(image_paths),
            };
            let outcome = match Engine::discover(events.clone()) {
                Ok(engine) => match submission.mode {
                    ShotMode::Create => engine.create(request).await.map(|_| ()),
                    ShotMode::Evolve => engine.evolve(request).await.map(|_| ()),
                },
                Err(error) => Err(error),
            };
            if let Err(error) = outcome {
                events.emit(Event::status(format!("engine stopped: {error}")));
            }
        }
        _ => respond(&mut socket, 404, "text/plain; charset=utf-8", "not found").await?,
    }
    Ok(())
}

async fn serve_library(socket: &mut TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let ledger = Ledger::discover()?;
    let records = ledger.list_apps()?;
    let iphone_slots_used = records
        .iter()
        .filter(|app| !app.retired && app.latest_shot.is_some())
        .count();
    let mut apps = Vec::new();
    for app in records {
        let Some(latest_shot) = app.latest_shot else {
            continue;
        };
        let shots = ledger
            .list_shots(&app.name)?
            .into_iter()
            .map(|shot| shot.number)
            .collect();
        apps.push(LibraryApp {
            icon_url: format!("/api/icon/{app_name}/{latest_shot}", app_name = app.name),
            name: app.name,
            latest_shot,
            shots,
            retired: app.retired,
        });
    }
    let body = serde_json::to_string(&LibraryResponse {
        apps,
        iphone_slots_used,
        iphone_slot_limit: 3,
    })?;
    respond(socket, 200, "application/json; charset=utf-8", &body).await?;
    Ok(())
}

async fn serve_icon(
    socket: &mut TcpStream,
    request_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut parts = request_path.trim_start_matches("/api/icon/").split('/');
    let app_name = parts.next().ok_or("missing app name")?;
    tohseno_engine::ledger::validate_app_name(app_name)?;
    let shot_number = parts.next().ok_or("missing shot number")?.parse::<u32>()?;
    if parts.next().is_some() {
        respond(socket, 404, "text/plain; charset=utf-8", "not found").await?;
        return Ok(());
    }
    let ledger = Ledger::discover()?;
    let shot = ledger.shot(app_name, shot_number)?;
    if !ledger
        .list_shots(app_name)?
        .iter()
        .any(|candidate| candidate.number == shot_number)
    {
        respond(socket, 404, "text/plain; charset=utf-8", "not found").await?;
        return Ok(());
    }
    let Some(icon) = find_app_icon(&shot.source_path())? else {
        respond(socket, 404, "text/plain; charset=utf-8", "not found").await?;
        return Ok(());
    };
    let content_type = match icon
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        _ => "image/png",
    };
    respond_bytes(socket, 200, content_type, &fs::read(icon)?).await?;
    Ok(())
}

async fn launch_simulator(
    socket: &mut TcpStream,
    body: &[u8],
    state: &State,
) -> Result<(), Box<dyn std::error::Error>> {
    let launch: SimulatorLaunch = match serde_json::from_slice(body) {
        Ok(launch) => launch,
        Err(error) => {
            respond(
                socket,
                400,
                "text/plain; charset=utf-8",
                &format!("invalid Simulator launch: {error}"),
            )
            .await?;
            return Ok(());
        }
    };
    tohseno_engine::ledger::validate_app_name(&launch.app_name)?;
    let _guard = state.press.lock().await;
    let ledger = Ledger::discover()?;
    match simulator::launch(&ledger, &state.events, &launch.app_name, launch.shot).await {
        Ok(session) => {
            *state.simulator.lock().await = Some(session);
            respond(
                socket,
                200,
                "application/json; charset=utf-8",
                r#"{"running":true}"#,
            )
            .await?;
        }
        Err(error) => {
            state
                .events
                .emit(Event::status(format!("Simulator stopped: {error}")));
            respond(socket, 500, "text/plain; charset=utf-8", &error.to_string()).await?;
        }
    }
    Ok(())
}

async fn serve_simulator_screen(
    socket: &mut TcpStream,
    state: &State,
) -> Result<(), Box<dyn std::error::Error>> {
    let session = state.simulator.lock().await.clone();
    let Some(session) = session else {
        respond(socket, 404, "text/plain; charset=utf-8", "not running").await?;
        return Ok(());
    };
    match simulator::screenshot(&session).await {
        Ok(image) => respond_bytes(socket, 200, "image/png", &image).await?,
        Err(error) => respond(socket, 500, "text/plain; charset=utf-8", &error.to_string()).await?,
    }
    Ok(())
}

fn find_app_icon(source: &Path) -> std::io::Result<Option<PathBuf>> {
    let mut candidates = Vec::new();
    collect_icons(source, false, &mut candidates)?;
    candidates.sort_by_key(|path| {
        fs::metadata(path)
            .map(|metadata| std::cmp::Reverse(metadata.len()))
            .unwrap_or(std::cmp::Reverse(0))
    });
    Ok(candidates.into_iter().next())
}

fn collect_icons(
    directory: &Path,
    inside_icon_directory: bool,
    candidates: &mut Vec<PathBuf>,
) -> std::io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        let is_icon_directory = inside_icon_directory || name.contains("appicon");
        if entry.file_type()?.is_dir() {
            collect_icons(&path, is_icon_directory, candidates)?;
        } else if is_icon_directory
            && path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    ["png", "jpg", "jpeg", "webp"]
                        .iter()
                        .any(|candidate| extension.eq_ignore_ascii_case(candidate))
                })
        {
            candidates.push(path);
        }
    }
    Ok(())
}

async fn stage_images(
    directory: &Path,
    images: Vec<UploadedImage>,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut paths = Vec::new();
    for (index, image) in images.into_iter().enumerate() {
        let original = Path::new(&image.name)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("image");
        let extension_is_valid = Path::new(original)
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                ["png", "jpg", "jpeg", "heic", "webp"]
                    .iter()
                    .any(|candidate| extension.eq_ignore_ascii_case(candidate))
            });
        if !extension_is_valid {
            continue;
        }
        let image_directory = directory.join(index.to_string());
        tokio::fs::create_dir(&image_directory).await?;
        let path = image_directory.join(original);
        let bytes = STANDARD.decode(image.data)?;
        tokio::fs::write(&path, bytes).await?;
        paths.push(path);
    }
    Ok(paths)
}

async fn stream_events(
    mut socket: TcpStream,
    events: EventBus,
) -> Result<(), Box<dyn std::error::Error>> {
    socket
        .write_all(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: keep-alive\r\nX-Content-Type-Options: nosniff\r\n\r\n",
        )
        .await?;
    let mut receiver = events.subscribe();
    loop {
        let event = match receiver.recv().await {
            Ok(event) => event,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                Event::status("the studio display skipped earlier lines.")
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => return Ok(()),
        };
        let encoded = serde_json::to_string(&event)?;
        if socket
            .write_all(format!("data: {encoded}\n\n").as_bytes())
            .await
            .is_err()
        {
            return Ok(());
        }
    }
}

struct Request {
    method: String,
    path: String,
    body: Vec<u8>,
}

async fn read_request(socket: &mut TcpStream) -> Result<Request, Box<dyn std::error::Error>> {
    let mut bytes = Vec::with_capacity(4096);
    let mut buffer = [0_u8; 8192];
    let header_end = loop {
        let read = socket.read(&mut buffer).await?;
        if read == 0 {
            return Err("connection closed before request headers".into());
        }
        bytes.extend_from_slice(&buffer[..read]);
        if bytes.len() > MAX_BODY {
            return Err("request is too large".into());
        }
        if let Some(position) = find_bytes(&bytes, b"\r\n\r\n") {
            break position + 4;
        }
    };
    let headers = std::str::from_utf8(&bytes[..header_end])?;
    let mut lines = headers.lines();
    let request_line = lines.next().ok_or("missing request line")?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().ok_or("missing method")?.to_owned();
    let path = parts
        .next()
        .ok_or("missing path")?
        .split('?')
        .next()
        .unwrap_or("/")
        .to_owned();
    let content_length = lines
        .find_map(|line| {
            line.split_once(':').and_then(|(name, value)| {
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
        })
        .unwrap_or(0);
    if content_length > MAX_BODY {
        return Err("request body is too large".into());
    }
    while bytes.len() - header_end < content_length {
        let read = socket.read(&mut buffer).await?;
        if read == 0 {
            return Err("connection closed before request body".into());
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    Ok(Request {
        method,
        path,
        body: bytes[header_end..header_end + content_length].to_vec(),
    })
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

async fn respond(
    socket: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &str,
) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Error",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    socket.write_all(response.as_bytes()).await
}

async fn respond_bytes(
    socket: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let reason = if status == 200 { "OK" } else { "Error" };
    let headers = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
        body.len()
    );
    socket.write_all(headers.as_bytes()).await?;
    socket.write_all(body).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locates_the_http_header_boundary() {
        assert_eq!(
            find_bytes(b"GET / HTTP/1.1\r\n\r\nbody", b"\r\n\r\n"),
            Some(14)
        );
    }

    #[test]
    fn selects_the_largest_image_from_the_app_icon_set() {
        let directory = tempfile::tempdir().unwrap();
        let icons = directory.path().join("Assets.xcassets/AppIcon.appiconset");
        fs::create_dir_all(&icons).unwrap();
        fs::write(icons.join("small.png"), [1_u8]).unwrap();
        fs::write(icons.join("large.png"), [1_u8; 64]).unwrap();
        fs::write(directory.path().join("unrelated.png"), [1_u8; 128]).unwrap();
        assert_eq!(
            find_app_icon(directory.path()).unwrap(),
            Some(icons.join("large.png"))
        );
    }
}
