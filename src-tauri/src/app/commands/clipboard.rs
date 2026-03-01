use std::path::PathBuf;
use tauri::Manager;
use crate::db;
use crate::db::DbState;
use crate::entity::clipboard_record;
use crate::error::{ApiError, AppError};
use crate::models::clipboard::{ClipboardResponse, ClipboardType};
use crate::platform::automation::{Automation, InjectContent};


// TODO: 查询列表
#[tauri::command]
pub async fn clipboard_record_list(app: tauri::AppHandle, page: u64, page_size: u64) -> Result<Vec<ClipboardResponse>, ApiError> {
    let db = app.state::<DbState>();

    let records = db::service::clipboard::list_records(&db, page, page_size).await.map_err(AppError::from)?;
    Ok(records)
}

// TODO: 查询剪切板数据的接口
#[tauri::command]
pub async fn paste(app: tauri::AppHandle, id: i32) -> Result<(), ApiError> {
    // TODO: 查询数据
    let db = app.state::<DbState>();
    let record = db::service::clipboard::find_clipboard_record(&db, id).await.map_err(AppError::from)?;

    let record = if let Some(record) = record {
        record
    } else {
        // TODO: 描述数据不存在
        return Ok(())
    };

    let mut auto = Automation::new();

    match record.r#type {
        t if t == ClipboardType::Text as i32 => {
            let data = String::from_utf8(record.data.unwrap_or_default()).expect("UTF8");
            auto.inject(
                InjectContent::Text(data)
            ).expect("inject");
        }
        t if t == ClipboardType::Html as i32 => {
            let data = String::from_utf8(record.data.unwrap_or_default()).expect("UTF8");
            auto.inject(
                InjectContent::Html(data)
            ).expect("inject");
        }
        t if t == ClipboardType::Rtf as i32 => {
            let data = String::from_utf8(record.data.unwrap_or_default()).expect("UTF8");
            auto.inject(
                InjectContent::Rtf(data)
            ).expect("inject");
        }
        t if t == ClipboardType::Image as i32 => {
            let data = String::from_utf8(record.data.unwrap_or_default()).expect("UTF8");
            auto.inject(
                InjectContent::Files(vec![PathBuf::from(data)])
            ).expect("inject");
        }
        t if t == ClipboardType::File as i32 || t == ClipboardType::Folder as i32 => {
            let data = String::from_utf8(record.data.unwrap_or_default()).expect("UTF8");
            let files: Vec<String> = serde_json::from_str(&data).expect("JSON");

            auto.inject(
                InjectContent::Files(files.into_iter().map(PathBuf::from).collect())
            ).expect("inject");
            // TODO: 再执行成功后应该更新的数据有：
        }
        _ => {}
    }

    // auto.inject(
    //     InjectContent::Files(vec![
    //         PathBuf::from("D:\\Documents\\code\\share-clip\\src-tauri\\Cargo.toml")
    //     ])
    // ).expect("发生错误 .env");

    Ok(())
}

// TODO: 测试对图片和文件的支持
