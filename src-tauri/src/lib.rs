mod app;
mod db;
mod entity;
mod error;
mod models;
mod platform;
mod services;
mod utils;

use app::commands::clipboard;
use app::config::load_or_create_config;
use app::shortcuts::global::init_register_shortcut;
use app::ui::tray::init_menu;
use app::ui::window::init_app;
use db::{init_db, DbState};
use log::{error, info};
use tauri::Manager;

use crate::db::service::cleanup::{cleanup_invalid_items, cleanup_old_items};
use crate::services::clipboard_watcher::start_clipboard_watcher;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_x::init())
        .setup(|app| {
            let config = load_or_create_config();
            let db = tauri::async_runtime::block_on(init_db())
                .map_err(|err| format!("failed to initialize sqlite database: {err}"))?;

            let config_clone = config.clone();
            let db_clone = db.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = cleanup_old_items(&db_clone, &config_clone).await {
                    error!("cleanup_old_items failed: {e}");
                }
                if let Err(e) = cleanup_invalid_items(&db_clone, &config_clone).await {
                    error!("cleanup_invalid_items failed: {e}");
                }
            });

            app.manage(config);
            app.manage(DbState { conn: db });
            init_app(app);
            init_register_shortcut(app);
            init_menu(app);

            let app_handle = app.handle().clone();
            let shutdown = start_clipboard_watcher(app_handle);
            app.manage(shutdown);

            info!("share-clip started");
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            clipboard::clipboard_record_list,
            clipboard::paste_clipboard_record,
            clipboard::copy_clipboard_record,
            clipboard::toggle_favorite,
            clipboard::toggle_share,
            clipboard::delete_clipboard_record,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
