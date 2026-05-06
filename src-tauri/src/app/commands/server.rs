use serde::Serialize;
use tauri::Manager;

use crate::app::config::AppConfigStore;

#[derive(Serialize)]
pub struct ShareServerStatus {
    pub running: bool,
}

#[tauri::command]
pub fn start_share_server(
    app: tauri::AppHandle,
    bind_ip: Option<String>,
    port: Option<u16>,
) -> Result<ShareServerStatus, String> {
    let config = app.state::<AppConfigStore>().get();
    let state = app.state::<crate::server::ServerState>();
    let bind_ip = bind_ip.unwrap_or(config.share_server_bind_ip);
    let port = port.unwrap_or(config.share_server_port);
    state.start(&bind_ip, port)?;
    Ok(ShareServerStatus { running: true })
}

#[tauri::command]
pub fn stop_share_server(app: tauri::AppHandle) -> Result<ShareServerStatus, String> {
    let state = app.state::<crate::server::ServerState>();
    state.stop()?;
    Ok(ShareServerStatus { running: false })
}

#[tauri::command]
pub fn share_server_status(app: tauri::AppHandle) -> Result<ShareServerStatus, String> {
    let state = app.state::<crate::server::ServerState>();
    Ok(ShareServerStatus {
        running: state.is_running()?,
    })
}
