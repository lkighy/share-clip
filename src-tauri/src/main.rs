// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clipboard;
mod entity;
mod config;

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
    // .with_shortcuts(["alt+shift+v"]).unwrap()
    //     .with_handler(|app, shortcut, _event| {
    //         println!("触发快捷键按钮: {:?}", shortcut);
    //         if shortcut.matches(Modifiers::META, Code::KeyV) {
    //             if let Some(index) = app.get_webview_window("index") {
    //                 let _ = index.show();
    //                 let _ = index.set_focus();
    //             }
    //         }
    //     })
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            // TODO: 注册数据的问题，还有什么内容的处理问题
            // TODO: 注册的按钮应该处理的方式
            // 1. 唤醒的是唤出剪切板
            // 2. 唤醒的是唤出文件分享
            // 检查快捷键注册
            // let shortcut = "ctrl+v";
            // if app.global_shortcut().is_registered(shortcut) {
            //     let _ = app.global_shortcut().register(shortcut);
            // }
            // let shortcut = "Command+v";
            // if app.global_shortcut().is_registered(shortcut) {
            //     let _ = app.global_shortcut().register(shortcut);
            // }
            println!("ctrl+v 按钮注册：{:?}",  app.global_shortcut().is_registered("ctrl+v"));
            println!("alt+shift+v 按钮注册：{:?}",  app.global_shortcut().is_registered("alt+shift+v"));
            println!("command+v 按钮注册：{:?}",  app.global_shortcut().is_registered("command+v"));

            let shortcut = "ctrl+c";
            // app.global_shortcut().on
            match app.global_shortcut().on_shortcut("command+c", |app, shortcut, _event| {
               println!("触发了复制内容：{:?}", shortcut);
            }) {
                Ok(_) => {
                    println!("快捷键 {} 注册成功", shortcut);
                }
                Err(e) => {
                    eprintln!("注册快捷键 {} 失败：{:?}", shortcut, e);
                }
            }

            let shortcut = "command+v";
            match app.global_shortcut().on_shortcut(shortcut, |app, shortcut, _event| {
                println!("触发快捷键按钮: {:?}", shortcut);
            }) {
                Ok(_) => {
                    println!("快捷键 {} 注册成功", shortcut);
                }
                Err(e) => {
                    eprintln!("注册快捷键 {} 失败：{:?}", shortcut, e);
                }
            }

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
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
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
                .build(app)?;
            Ok(())
        })
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![greet, clipboard::get_clipboard_text])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
