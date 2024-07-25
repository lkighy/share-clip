// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clipboard;
mod config;
mod entity;
mod utils;
mod shortcuts;

use crate::utils::toggle_window_visibility;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    Manager,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers};
use crate::clipboard::TaskManager;
use crate::config::{init_menu, init_register_shortcut};

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

// TODO: 这面应该添加一个存储全局的内容

fn main() {
    // TODO: 初始化配置文件，加载配置参数
    // TODO: 测试读取文件的机制
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            // TODO: 通过 tokio 定时执行的任务，以及设置
            // TODO: 获取配置文件中的date,如果存在的话
            let app_handle = app.app_handle().clone();
            let task = TaskManager::new(app_handle);
            // 启动任务，挂在任务
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
               task.start().await;
            });

            app.manage(task);
            // TODO 在这里创建一个 存储全局变量的 state 的任务，并用于启动或停止指定任务
            // app.manage()
            // 初始化快捷键注册
            init_register_shortcut(app);
            // 初始化菜单注册
            init_menu(app);
            Ok(())
        })
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            clipboard::get_clipboard_text
        ])
        .run(tauri::generate_context!())
        // TODO: 如果执行到这里可以进行指定的操作，例如释放指定资源等
        .expect("error while running tauri application");
}
