use std::collections::BTreeSet;
use tauri::Manager;
use tauri_plugin_global_shortcut::GlobalShortcutExt;

use crate::app::config::{is_private_ipv4, scan_lan_devices, LanDiscoveredDevice};
use crate::app::config::{AppConfig, AppConfigStore, AppConfigUpdate};
use crate::app::shortcuts::global::init_register_shortcut;
use crate::error::{ApiError, AppError};

#[tauri::command]
pub fn get_app_config(app: tauri::AppHandle) -> Result<AppConfig, ApiError> {
    let store = app.state::<AppConfigStore>();
    Ok(store.get())
}

#[tauri::command]
pub fn get_local_device_info(app: tauri::AppHandle) -> Result<serde_json::Value, ApiError> {
    let config = app.state::<AppConfigStore>().get();
    Ok(serde_json::json!({
        "device_id": config.local_device_id,
        "device_name": config.local_device_name,
    }))
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

    if previous.shortcut != updated.shortcut {
        let _ = app.global_shortcut().unregister(previous.shortcut.trim());
        init_register_shortcut(&app);
    }

    let window_size_changed = previous.clipboard_window_width != updated.clipboard_window_width
        || previous.clipboard_window_height != updated.clipboard_window_height;
    if window_size_changed {
        if let Some(window) = app.get_window("index") {
            let _ = window.set_size(tauri::LogicalSize::new(
                updated.clipboard_window_width.max(200) as f64,
                updated.clipboard_window_height.max(120) as f64,
            ));
        }
    }

    let server_state = app.state::<crate::server::ServerState>();
    let was_running = server_state.is_running().unwrap_or(false);
    let server_bind_changed = previous.share_server_bind_ip != updated.share_server_bind_ip
        || previous.share_server_port != updated.share_server_port;
    if was_running && server_bind_changed {
        let _ = server_state.stop();
        server_state
            .start(
                &updated.share_server_bind_ip,
                updated.share_server_port,
                app.clone(),
            )
            .map_err(AppError::InvalidInput)?;
    }
    crate::app::ui::tray::update_share_server_menu_label(&app);
    crate::app::events::emit_app_config_changed(&app, "app_config_updated");

    Ok(updated)
}

#[tauri::command]
pub fn get_share_server_ip_options() -> Result<Vec<String>, ApiError> {
    let mut ips = BTreeSet::new();
    ips.insert("127.0.0.1".to_string());
    ips.insert("0.0.0.0".to_string());

    let addrs = if_addrs::get_if_addrs().map_err(|e| AppError::InvalidInput(e.to_string()))?;
    for iface in addrs {
        if let std::net::IpAddr::V4(ipv4) = iface.ip() {
            if is_private_ipv4(ipv4) {
                ips.insert(ipv4.to_string());
            }
        }
    }
    Ok(ips.into_iter().collect())
}

#[tauri::command]
pub async fn scan_lan_share_devices(
    app: tauri::AppHandle,
) -> Result<Vec<LanDiscoveredDevice>, ApiError> {
    let config = app.state::<AppConfigStore>().get();
    scan_lan_devices(config.share_server_port, &config.local_device_id)
        .await
        .map_err(|err| AppError::InvalidInput(err.to_string()).into())
}
