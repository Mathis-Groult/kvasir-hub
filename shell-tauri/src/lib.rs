//! Tauri shell: desktop and mobile host for the Kvasir Hub UI.
//!
//! Thin by design. It owns three things the core must not know: where files live on the
//! device (`app_data_dir`), the IPC commands the UI calls, and the start of the loopback
//! media server (`hub-media-http`) that lets `<audio>`/`<img>` load authenticated media. All behaviour comes from `hub-rpc`.

use hub_catalog::Endpoints;
use hub_core::Hub;
use std::sync::Arc;
use tauri::ipc::{InvokeBody, Request};
use tauri::{Manager, State};

/// The one Hub of the running app.
struct AppHub(Arc<Hub>);

/// The loopback server that feeds `<audio>`, `<img>` and downloads.
struct AppMedia(hub_media_http::MediaServer);

#[tauri::command]
async fn hub_rpc(
    hub: State<'_, AppHub>,
    command: String,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let hub = Arc::clone(&hub.0);
    // The core blocks on the network: keep it off the async runtime's threads.
    tauri::async_runtime::spawn_blocking(move || {
        hub_rpc::dispatch(&hub, &command, &args).to_value()
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn hub_upload(
    hub: State<'_, AppHub>,
    request: Request<'_>,
) -> Result<serde_json::Value, String> {
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

/// Base URL of the loopback media server (`http://127.0.0.1:<port>/<token>`).
#[tauri::command]
fn hub_media_base(media: State<'_, AppMedia>) -> String {
    media.0.base_url().to_owned()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let secrets = app.path().app_data_dir()?.join("credentials.json");
            let hub = hub_catalog::build(&Endpoints::production(), &secrets);
            let hub = Arc::new(hub);
            let media = hub_media_http::spawn(Arc::clone(&hub))?;
            app.manage(AppHub(hub));
            app.manage(AppMedia(media));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            hub_rpc,
            hub_upload,
            hub_media_base
        ])
        .run(tauri::generate_context!())
        .expect("error while running the Kvasir app");
}
