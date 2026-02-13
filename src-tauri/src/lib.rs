mod app;
mod plugins;

use app::commands::greet;
use app::config::load_or_create_config;
use app::shortcut::init_register_shortcut;
use app::tray::init_menu;
use app::window::init_app;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let config = load_or_create_config();
            app.manage(config);
            init_app(app);
            init_register_shortcut(app);
            init_menu(app);
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
