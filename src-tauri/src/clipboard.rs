
// TODO: 1. 获取剪切板并更新数据

// TODO: 2. 从数据库中获取剪切板数据并给予给图形

use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;

// TODO: DEMO: 测试获取剪切板数据
#[tauri::command]
pub async fn get_clipboard_text(app: AppHandle) -> Result<String, String> {
    let data = app.clipboard().read_text();
    return match data {
        Ok(data) => {
            Ok(data)
        }
        Err(err) => {
            Err(err.to_string())
        }
    }
}