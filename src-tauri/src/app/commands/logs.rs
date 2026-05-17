use log::info;
use std::fs;
use tauri::{command, AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

pub fn open_app_log_dir(app: &AppHandle) -> Result<(), String> {
    let log_dir = app.path().app_log_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&log_dir).map_err(|e| format!("failed to create log directory: {e}"))?;

    info!("opening log directory: {}", log_dir.display());
    app.opener()
        .open_path(log_dir.to_string_lossy().to_string(), None::<String>)
        .map_err(|e| e.to_string())
}

#[command]
pub fn open_log_dir(app: AppHandle) -> Result<(), String> {
    open_app_log_dir(&app)
}

#[command]
pub fn get_log_dir(app: AppHandle) -> Result<String, String> {
    let log_dir = app.path().app_log_dir().map_err(|e| e.to_string())?;
    Ok(log_dir.to_string_lossy().to_string())
}
