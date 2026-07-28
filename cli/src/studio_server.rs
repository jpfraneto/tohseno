use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tohseno_engine::gates::intent::Intent;
use tohseno_engine::{Engine, Event, EventBus, ShotRequest};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

const INDEX: &str = include_str!("../../studio/index.html");
const STYLE: &str = include_str!("../../studio/style.css");
const SCRIPT: &str = include_str!("../../studio/app.js");
const MAX_BODY: usize = 160 * 1024 * 1024;

#[derive(Clone)]
struct State {
    events: EventBus,
    press: Arc<Mutex<()>>,
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

pub async fn serve(port: u16, events: EventBus) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(("127.0.0.1", port)).await?;
    let address = listener.local_addr()?;
    let url = format!("http://127.0.0.1:{}", address.port());
    events.emit(Event::status(format!("studio is ready at {url}.")));
    let _ = std::process::Command::new("open").arg(&url).spawn();
    let state = State {
        events,
        press: Arc::new(Mutex::new(())),
    };

    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let (socket, _) = accepted?;
                let state = state.clone();
                tokio::spawn(async move {
                    if let Err(error) = handle(socket, state).await {
                        eprintln!("studio: {error}");
                    }
                });
            }
            signal = tokio::signal::ctrl_c() => {
                signal?;
                return Ok(());
            }
        }
    }
}

async fn handle(mut socket: TcpStream, state: State) -> Result<(), Box<dyn std::error::Error>> {
    let request = read_request(&mut socket).await?;
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

            let events = state.events.clone();
            let press = state.press.clone();
            tokio::spawn(async move {
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
            });
        }
        _ => respond(&mut socket, 404, "text/plain; charset=utf-8", "not found").await?,
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
    let path = parts.next().ok_or("missing path")?.to_owned();
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
        _ => "Error",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    socket.write_all(response.as_bytes()).await
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
}
