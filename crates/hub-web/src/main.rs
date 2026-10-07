//! Web shell: serves the UI, the JSON command endpoint, media and uploads on loopback.
//!
//! Development and test host for the exact UI and core the desktop/mobile app ships. Not a
//! deployment target: it binds to 127.0.0.1 and holds one signed-in account for everyone.

use anyhow::{Context, Result};
use clap::Parser;
use hub_catalog::Endpoints;
use hub_core::{Hub, HubError};
use hub_rpc::{Envelope, dispatch, parse_media_target, upload};
use hub_web::{content_disposition, content_type, media_cache_control, resolve_static};
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use tiny_http::{Header, Method, Request, Response, StatusCode};

/// Largest upload accepted in memory (see plan, risks).
const MAX_UPLOAD_BYTES: usize = 256 * 1024 * 1024;

#[derive(Parser)]
struct Cli {
    #[arg(long, default_value = "127.0.0.1:4560")]
    listen: String,
    /// Built UI (`ui/dist`).
    #[arg(long, default_value = "ui/dist")]
    ui_dir: PathBuf,
    /// Where the credentials rest between runs.
    #[arg(long)]
    secret_file: PathBuf,
}

/// A header from names and values this file builds. A value an upstream server controls
/// (content type) that is not valid ASCII degrades to a neutral one instead of panicking.
fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name, value)
        .or_else(|()| Header::from_bytes(name, "application/octet-stream"))
        .expect("fallback header is valid ASCII")
}

fn json_response(envelope: &Envelope) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(envelope.to_json())
        .with_header(header("Content-Type", "application/json"))
        .with_header(header("Cache-Control", "no-store"))
}

fn error_response(error: &HubError) -> Response<std::io::Cursor<Vec<u8>>> {
    let status = match error {
        HubError::NotSignedIn => 401,
        HubError::Forbidden { .. } => 403,
        HubError::NotFound { .. } | HubError::UnknownModule(_) => 404,
        HubError::BadArgs { .. } => 400,
        _ => 502,
    };
    Response::from_string(error.view().message).with_status_code(status)
}

fn read_body(request: &mut Request, limit: usize) -> Result<Vec<u8>, String> {
    let mut body = Vec::new();
    let mut limited = request
        .as_reader()
        .take(u64::try_from(limit).unwrap_or(u64::MAX) + 1);
    limited.read_to_end(&mut body).map_err(|e| e.to_string())?;
    if body.len() > limit {
        return Err("body too large".to_owned());
    }
    Ok(body)
}

fn handle(hub: &Hub, ui_dir: &std::path::Path, mut request: Request) {
    let url = request.url().to_owned();
    let method = request.method().clone();
    let range = request
        .headers()
        .iter()
        .find(|h| h.field.equiv("Range"))
        .map(|h| h.value.as_str().to_owned());
    let result = match (&method, url.as_str()) {
        (Method::Post, path) if path.starts_with("/rpc/") => {
            let command = path.trim_start_matches("/rpc/");
            let body = read_body(&mut request, 1024 * 1024).unwrap_or_default();
            let args: serde_json::Value =
                serde_json::from_slice(&body).unwrap_or(serde_json::json!({}));
            request.respond(json_response(&dispatch(hub, command, &args)))
        }
        (Method::Post, path) if path.starts_with("/upload/") => {
            match read_body(&mut request, MAX_UPLOAD_BYTES) {
                Ok(body) => {
                    let target = path.trim_start_matches("/upload");
                    request.respond(json_response(&upload(hub, target, body)))
                }
                Err(e) => request.respond(Response::from_string(e).with_status_code(413)),
            }
        }
        (Method::Get, path) if path.starts_with("/media/") => {
            let target = path.trim_start_matches("/media");
            match parse_media_target(target, range.as_deref())
                .and_then(|(module, req)| hub.media(&module, &req).map(|r| (req.op, r)))
            {
                Ok((op, media)) => {
                    let mut response = Response::new(
                        StatusCode(media.status),
                        vec![
                            header("Content-Type", &media.content_type),
                            header("Accept-Ranges", "bytes"),
                            header("Cache-Control", media_cache_control(&op)),
                        ],
                        media.body,
                        media.content_length.and_then(|n| usize::try_from(n).ok()),
                        None,
                    );
                    if let Some(range) = &media.content_range {
                        response.add_header(header("Content-Range", range));
                    }
                    if let (true, Some(name)) = (op == "download", &media.file_name) {
                        response
                            .add_header(header("Content-Disposition", &content_disposition(name)));
                    }
                    request.respond(response)
                }
                Err(e) => request.respond(error_response(&e)),
            }
        }
        (Method::Get, path) => {
            let path = path.split('?').next().unwrap_or("/");
            match resolve_static(ui_dir, path).and_then(|p| File::open(&p).ok().map(|f| (p, f))) {
                Some((file_path, file)) => request.respond(
                    Response::from_file(file)
                        .with_header(header("Content-Type", content_type(&file_path)))
                        .with_header(header("Cache-Control", "no-cache"))
                        .with_header(header("X-Content-Type-Options", "nosniff")),
                ),
                None => request.respond(Response::from_string("not found").with_status_code(404)),
            }
        }
        _ => request.respond(Response::from_string("method not allowed").with_status_code(405)),
    };
    if let Err(e) = result {
        eprintln!("[hub-web] respond failed for {url}: {e}");
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let hub = std::sync::Arc::new(hub_catalog::build(
        &Endpoints::production(),
        &cli.secret_file,
    ));
    let server = tiny_http::Server::http(&cli.listen)
        .map_err(|e| anyhow::anyhow!("{e}"))
        .with_context(|| format!("listening on {}", cli.listen))?;
    eprintln!(
        "[hub-web] http://{} (ui: {})",
        cli.listen,
        cli.ui_dir.display()
    );
    let server = std::sync::Arc::new(server);
    // One thread per request: media streams are long and must not block commands.
    let ui_dir = std::sync::Arc::new(cli.ui_dir);
    for request in server.incoming_requests() {
        let hub = std::sync::Arc::clone(&hub);
        let ui_dir = std::sync::Arc::clone(&ui_dir);
        std::thread::spawn(move || handle(&hub, &ui_dir, request));
    }
    Ok(())
}
