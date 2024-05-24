// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clipboard;
mod entities;

use tauri::{
    Manager,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{TrayIconBuilder},
};
use tauri_plugin_global_shortcut::{Modifiers, Code, GlobalShortcutExt};

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}



fn main() {

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().with_shortcuts(["ctrl+v", "Command+v"]).unwrap()
            .with_handler(|app, shortcut, _event| {
                println!("触发快捷键按钮: {:?}", shortcut);
                if shortcut.matches(Modifiers::CONTROL, Code::KeyV) {
                    if let Some(index) = app.get_webview_window("index") {
                        let _ = index.show();
                        let _ = index.set_focus();
                    }
                }
        }).build())
        .setup(|app| {
            let _ = app.global_shortcut().register("ctrl+v");
            let _ = app.global_shortcut().register("Command+v");
            // 设置剪切板
            let setting_item = MenuItemBuilder::with_id("setting", "设置").build(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "退出").build(app)?;
            let menu = MenuBuilder::new(app).items(&[&setting_item, &quit_item]).build()?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().cloned().unwrap())
                .menu(&menu)
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "quit" => {
                        app.exit(0)
                    }
                    "setting" => {
                        if let Some(webview_window) = app.get_webview_window("setting") {
                            let _ = webview_window.show();
                            let _ = webview_window.set_focus();
                        }
                    }
                    _ => (),
                })
                // 这里是对系统托盘图标进行单机时的事件，如果需要可以在这里进行监听
                // .on_tray_icon_event(|tray, event| {
                //     if event.click_type == ClickType::Left {
                //         let app = tray.app_handle();
                //         if let Some(webview_window) = app.get_webview_window("index") {
                //             let _ = webview_window.show();
                //             let _ = webview_window.set_focus();
                //         }
                //     }
                // })
                .build(app)?;
            Ok(())
        })
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
