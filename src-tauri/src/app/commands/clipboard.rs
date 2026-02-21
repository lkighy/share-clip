use std::path::PathBuf;
use tauri::Manager;
use crate::db;
use crate::db::DbState;
use crate::entity::clipboard_record;
use crate::error::{ApiError, AppError};
use crate::platform::automation::{Automation, InjectContent};


// TODO: 查询列表
#[tauri::command]
pub async fn clipboard_list(app: tauri::AppHandle) -> Result<Vec<clipboard_record::Model>, AppError> {
    // TODO: 这里的处理方式应该时怎样的，应该从数据库中获取，
    let db = app.state::<DbState>();

    db::service::clipboard::list_records(&db, 10, 0).await.map_err(Into::into)
}


#[tauri::command]
pub async fn paste() {
    let mut auto = Automation::new();

    auto.inject(
        InjectContent::Files(vec![
            PathBuf::from("D:\\Documents\\code\\share-clip\\src-tauri\\Cargo.toml")
        ])
    ).expect("发生错误 .env");
}

// TODO: 测试对图片和文件的支持