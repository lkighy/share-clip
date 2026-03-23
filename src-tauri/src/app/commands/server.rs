use serde::Serialize;
use tauri::Manager;

#[derive(Serialize)]
pub struct ShareServerStatus {
    pub running: bool,
}

#[tauri::command]
pub fn start_share_server(app: tauri::AppHandle, port: u16) -> Result<ShareServerStatus, String> {
    let state = app.state::<crate::server::ServerState>();
    state.start(port)?;
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
