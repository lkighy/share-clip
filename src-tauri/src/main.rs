// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clipboard;
mod entity;
mod config;
mod utils;

use tauri::{
    Manager,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{TrayIconBuilder},
};
use tauri_plugin_global_shortcut::{Modifiers, Code, GlobalShortcutExt};
use crate::utils::toggle_window_visibility;

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}



fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            // 检查快捷键注册
            let shortcut = "ctrl+alt+v";
            if app.global_shortcut().is_registered(shortcut) {
                let _ = app.global_shortcut().register(shortcut);
            }
            let _ = app.global_shortcut().on_shortcut(shortcut, |app, shortcut, _| {
                if let Some(window) = app.get_webview_window("index") {
                    toggle_window_visibility(&window);
                }
            });
            let shortcut = "f12";
            if app.global_shortcut().is_registered(shortcut) {
                let _ = app.global_shortcut().register(shortcut);
            }
            let _ = app.global_shortcut().on_shortcut(shortcut, |app, shortcut, _| {
               if let Some(window) = app.get_webview_window("index") {
                   if let Ok(true) = window.is_visible() {
                       window.open_devtools();
                   }
               }
            });

            // 设置剪切板
            let clipboard_item = MenuItemBuilder::with_id("clipboard", "剪切板").build(app)?;
            let setting_item = MenuItemBuilder::with_id("setting", "设置").build(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "退出").build(app)?;
            let menu = MenuBuilder::new(app).items(&[&clipboard_item, &setting_item, &quit_item]).build()?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().cloned().unwrap())
                .menu(&menu)
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "clipboard" => {
                        if let Some(window) = app.get_webview_window("index") {
                            toggle_window_visibility(&window);
                        }
                    }
                    "quit" => {
                        app.exit(0)
                    }
                    "setting" => {
                        if let Some(window) = app.get_webview_window("setting") {
                            toggle_window_visibility(&window);
                        }
                    }
                    _ => (),
                })
                .build(app)?;
            Ok(())
        })
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![greet, clipboard::get_clipboard_text])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
