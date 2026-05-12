use crate::models::window::WindowOperations;
use tauri::command;
use tauri::{AppHandle, Manager, Window};

/// 隐藏窗口
///
/// # 参数
/// - `operation`: 窗口操作，hide 或 close。
/// - `label`: 可选，窗口标签。若不传则隐藏当前窗口。
#[command]
pub async fn operation_window(
    app: AppHandle,
    operation: WindowOperations,
    label: String,
) -> Result<(), String> {
    let window: Window = app
        .get_window(&label)
        .ok_or_else(|| format!("窗口 '{}' 不存在", label))?;

    #[cfg(target_os = "windows")]
    if &label == "index" {
        if let WindowOperations::Hide = operation {
            crate::platform::non_activating::windows::hide_window();
        }
        return Ok(());
    }

    match operation {
        WindowOperations::Close => {
            window.close().map_err(|e| e.to_string())?;
        }
        WindowOperations::Hide => {
            window.hide().map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}
