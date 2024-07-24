use tauri::App;
use tauri_plugin_global_shortcut::GlobalShortcutExt;

// 绑定的数据有哪些呢
// 1. 注册快捷键
/// 关键字
pub fn register_clip_shortcut_key(app: &mut App, shortcut: &str) -> bool {
    if !app.global_shortcut().is_registered(shortcut) {
        return false;
    }

    //
    // 绑定按钮功能，该功能应该是用于监听复制的快捷键
    // TODO: 重置监听快捷键

    true
}
