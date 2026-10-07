//! Tauri shell: desktop and mobile host for the Kvasir Hub UI.
//!
//! Thin by design. It owns three things the core must not know: where files live on the
//! device (`app_data_dir`), the IPC commands the UI calls, and the `hubmedia://` scheme that
//! lets `<audio>`/`<img>` load authenticated media. All behaviour comes from `hub-rpc`.

mod media;

use hub_catalog::Endpoints;
use hub_core::Hub;
use std::sync::Arc;
use tauri::ipc::{InvokeBody, Request};
use tauri::{Manager, State};

/// The one Hub of the running app.
struct AppHub(Arc<Hub>);

#[tauri::command]
async fn hub_rpc(
    hub: State<'_, AppHub>,
    command: String,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let hub = Arc::clone(&hub.0);
    // The core blocks on the network: keep it off the async runtime's threads.
    tauri::async_runtime::spawn_blocking(move || hub_rpc::dispatch(&hub, &command, &args).to_value())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn hub_upload(hub: State<'_, AppHub>, request: Request<'_>) -> Result<serde_json::Value, String> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("upload needs a raw body".to_owned());
    };
    let target = request
        .headers()
        .get("x-hub-target")
        .and_then(|v| v.to_str().ok())
        .ok_or("missing x-hub-target")?
        .to_owned();
    let (hub, bytes) = (Arc::clone(&hub.0), bytes.clone());
    tauri::async_runtime::spawn_blocking(move || hub_rpc::upload(&hub, &target, bytes).to_value())
        .await
        .map_err(|e| e.to_string())
}

/// Saves a file from Files into the downloads folder; returns where it went.
#[tauri::command]
async fn hub_save(
    app: tauri::AppHandle,
    hub: State<'_, AppHub>,
    repo: String,
    path: String,
) -> Result<String, String> {
    let folder = app
        .path()
        .download_dir()
        .or_else(|_| app.path().app_data_dir().map(|d| d.join("downloads")))
        .map_err(|e| e.to_string())?;
    let hub = Arc::clone(&hub.0);
    tauri::async_runtime::spawn_blocking(move || media::save_download(&hub, &folder, &repo, &path))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let secrets = app.path().app_data_dir()?.join("credentials.json");
            let hub = hub_catalog::build(&Endpoints::production(), &secrets);
            app.manage(AppHub(Arc::new(hub)));
            Ok(())
        })
        .register_asynchronous_uri_scheme_protocol("hubmedia", |context, request, responder| {
            let app = context.app_handle().clone();
            // One thread per request: media is long-lived and must not stall the UI.
            std::thread::spawn(move || {
                let hub = app.state::<AppHub>();
                responder.respond(media::serve(&hub.0, &request));
            });
        })
        .invoke_handler(tauri::generate_handler![hub_rpc, hub_upload, hub_save])
        .run(tauri::generate_context!())
        .expect("error while running the Kvasir app");
}
