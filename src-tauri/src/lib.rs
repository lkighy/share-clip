mod app;
mod db;
mod entity;
mod platform;
mod error;
mod models;

use app::config::load_or_create_config;
use app::shortcuts::global::init_register_shortcut;
use app::ui::tray::init_menu;
use app::ui::window::init_app;
use app::commands::clipboard;
use db::{init_db, DbState};
use tauri::Manager;
// use crate::app::shortcuts::global::init_hide_register_shortcut_event;
// use crate::app::shortcuts::global::ShortcutState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        // .manage(ShortcutState {
        //     auto_hide: AtomicBool::new(false),
        //     listener_added: AtomicBool::new(false),
        // })
        .setup(|app| {
            let config = load_or_create_config();
            let db = tauri::async_runtime::block_on(init_db())
                .map_err(|err| format!("failed to initialize sqlite database: {err}"))?;

            app.manage(config);
            app.manage(DbState { conn: db });
            init_app(app);
            init_register_shortcut(app);
            // init_hide_register_shortcut_event(app);
            init_menu(app);
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            clipboard::paste,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
