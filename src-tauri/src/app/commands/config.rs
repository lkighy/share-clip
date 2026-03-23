use tauri::Manager;
use tauri_plugin_global_shortcut::GlobalShortcutExt;

use crate::app::config::{AppConfig, AppConfigStore, AppConfigUpdate};
use crate::app::shortcuts::global::init_register_shortcut;
use crate::error::{ApiError, AppError};

#[tauri::command]
pub fn get_app_config(app: tauri::AppHandle) -> Result<AppConfig, ApiError> {
    let store = app.state::<AppConfigStore>();
    Ok(store.get())
}

#[tauri::command]
pub fn update_app_config(
    app: tauri::AppHandle,
    payload: AppConfigUpdate,
) -> Result<AppConfig, ApiError> {
    let store = app.state::<AppConfigStore>();
    let previous = store.get();
    let updated = store
        .update_with(payload)
        .map_err(|err| AppError::InvalidInput(err.to_string()))?;

    let _ = app.global_shortcut().unregister(previous.shortcut.trim());
    init_register_shortcut(&app);

    if let Some(window) = app.get_window("index") {
        let _ = window.set_size(tauri::LogicalSize::new(
            updated.clipboard_window_width.max(200) as f64,
            updated.clipboard_window_height.max(120) as f64,
        ));
    }

    Ok(updated)
}
