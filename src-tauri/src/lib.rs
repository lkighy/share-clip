mod app;
mod db;
mod entity;
mod error;
mod models;
mod platform;
mod server;
mod services;
mod utils;

use crate::app::commands::window;
use crate::db::service::cleanup::{cleanup_invalid_items, cleanup_old_items};
use crate::services::clipboard_watcher::start_clipboard_watcher;
use app::commands::{
    clipboard, config as config_commands, server as server_commands,
    share_files as share_files_commands,
};
use app::config::AppConfigStore;
use app::shortcuts::global::init_register_shortcut;
use app::ui::tray::init_menu;
use app::ui::window::init_app;
use db::{init_db, DbState};
use log::{error, info};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_x::init())
        .setup(|app| {
            let config_store = AppConfigStore::load();
            let config = config_store.get();
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

            app.manage(config_store);
            app.manage(server::ServerState::new(db.clone()));
            app.manage(DbState { conn: db });
            init_app(app);
            init_register_shortcut(&app.handle());
            init_menu(app);
            let pending_count = tauri::async_runtime::block_on(async {
                use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
                entity::inbound_connections::Entity::find()
                    .filter(entity::inbound_connections::Column::AuthStatus.eq(1))
                    .count(&app.state::<DbState>().conn)
                    .await
                    .unwrap_or(0)
            });
            app::ui::tray::update_connect_device_menu_pending_count(&app.handle(), pending_count);

            let app_handle = app.handle().clone();
            let shutdown = start_clipboard_watcher(app_handle);
            app.manage(shutdown);

            if config.auto_start_share_server {
                let server_state = app.state::<server::ServerState>();
                if let Err(e) = server_state.start(
                    &config.share_server_bind_ip,
                    config.share_server_port,
                    app.handle().clone(),
                ) {
                    error!("start share server failed: {}", e);
                }
                app::ui::tray::update_share_server_menu_label(&app.handle());
            }

            info!("share-clip started");
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            clipboard::clipboard_record_list,
            clipboard::paste_clipboard_record,
            clipboard::paste_remote_clipboard_content,
            clipboard::copy_clipboard_record,
            clipboard::copy_remote_clipboard_content,
            clipboard::toggle_favorite,
            clipboard::toggle_share,
            clipboard::delete_clipboard_record,
            config_commands::get_app_config,
            config_commands::get_local_device_info,
            config_commands::update_app_config,
            config_commands::get_share_server_ip_options,
            server_commands::start_share_server,
            server_commands::stop_share_server,
            server_commands::share_server_status,
            share_files_commands::list_remote_share_users,
            share_files_commands::list_local_shared_files,
            share_files_commands::upsert_remote_share_user,
            share_files_commands::update_remote_share_user_auth_status,
            share_files_commands::remove_remote_share_user,
            share_files_commands::list_inbound_connection_requests,
            share_files_commands::set_inbound_connection_auth_status,
            share_files_commands::reveal_shared_clipboard_item,
            share_files_commands::reveal_local_shared_file,
            share_files_commands::get_local_shared_file_thumbnail,
            share_files_commands::unshare_local_shared_file,
            share_files_commands::add_manual_shared_paths,
            share_files_commands::refresh_local_share_indexes,
            share_files_commands::get_remote_cache_status,
            share_files_commands::list_remote_cached_files,
            share_files_commands::cache_remote_shared_file,
            share_files_commands::reveal_remote_shared_cache,
            share_files_commands::remove_remote_shared_cache,
            window::operation_window,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
