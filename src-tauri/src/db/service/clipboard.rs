#![allow(dead_code)]


use crate::db::DbState;
use crate::entity::clipboard_record::{Model, ActiveModel, Entity};
use sea_orm::{ActiveModelTrait, DatabaseConnection, DbErr, EntityTrait, ModelTrait, Set};
use crate::db::repository::clipboard_record;
use crate::entity::prelude::ClipboardRecord;
use crate::error::AppError;
use crate::models::clipboard::{ClipboardResponse, ClipboardType};

// pub async fn create_record(
//     db: &DbState,
//     input: CreateClipboardRecordInput,
// ) -> Result<Model, DbErr> {
//     clipboard_record::create(&db.conn, input).await
// }
//
// pub async fn get_record(db: &DbState, id: &str) -> Result<Option<Model>, DbErr> {
//     clipboard_record::get_by_id(&db.conn, id).await
// }
//
pub async fn list_records(
    db: &DbState,
    page: u64,
    page_size: u64,
) -> Result<Vec<ClipboardResponse>, DbErr> {
    let page = page.max(1);
    let page_size = page_size.clamp(1, 100);

    let offset = (page - 1) * page_size;
    clipboard_record::list_latest(&db.conn, page_size, offset).await
}

// TODO: 获取数据
pub async fn get_and_validate_clipboard_record(
    db: &DbState,
    id: i32,
    auto_cleanup: bool,
) -> Result<Option<Model>, AppError> {
    // 查询条目
    let record = match Entity::find_by_id(id).one(&db.conn).await? {
        Some(r) => r,
        None => return Ok(None),
    };

    // 判断条目类型
    match record.r#type {
        // 处理文件或文件夹类型
        t if t == ClipboardType::File as i32 || t == ClipboardType::Folder as i32 => {
            // 解析路径列表（假设 data 存储的是 JSON 字符串的字节数组）
            let paths: Vec<String> = match &record.data {
                Some(bytes) => serde_json::from_slice(bytes).map_err(AppError::Json)?,
                None => {
                    // data 为空，视为无效，根据配置处理
                    return handle_invalid_entry(db, record, auto_cleanup).await;
                }
            };

            // 检查每个路径是否存在
            let any_missing = paths.iter().any(|p| !std::path::Path::new(p).exists());
            if any_missing {
                return handle_invalid_entry(db, record, auto_cleanup).await;
            }
        }

        // 处理图片类型
        t if t == ClipboardType::Image as i32 => {
            // 假设图片的 data 存储的是图片文件的路径（字符串），而不是二进制
            let path_str = match &record.data {
                Some(bytes) => String::from_utf8(bytes.clone()).map_err(AppError::from)?,
                None => {
                    // data 为空，视为无效
                    return handle_invalid_entry(db, record, auto_cleanup).await;
                }
            };
            let path = std::path::Path::new(&path_str);
            if !path.exists() {
                return handle_invalid_entry(db, record, auto_cleanup).await;
            }
        }

        // 其他类型（如文本、HTML、RTF 等）无需验证
        _ => {}
    }

    // 所有验证通过，返回有效条目
    Ok(Some(record))
}

/// 辅助函数：处理无效条目（删除或标记为无效）
async fn handle_invalid_entry(
    db: &DbState,
    record: Model,
    auto_cleanup: bool,
) -> Result<Option<Model>, AppError> {
    if auto_cleanup {
        record.delete(&db.conn).await?;
    } else {
        let mut active: ActiveModel = record.into();
        active.is_valid = Set(0);
        active.update(&db.conn).await?;
    }
    Ok(None)
}

/// 切换条目的收藏状态，返回新的收藏状态
pub async fn toggle_favorite(
    db: &DbState,
    id: i32,
) -> Result<bool, AppError> {
    // 1. 查找条目
    let record = Entity::find_by_id(id)
        .one(&db.conn)
        .await?
        .ok_or(AppError::NotFound)?;

    // 2. 计算新状态（取反）
    let new_favorite = if record.is_favorite == 1 { 0 } else { 1 };

    // 3. 更新数据库
    let mut active: crate::entity::clipboard_record::ActiveModel = record.into();
    active.is_favorite = Set(new_favorite);
    active.update(&db.conn).await?;

    // 4. 返回新状态（true/false 方便前端使用）
    Ok(new_favorite == 1)
}

/// 切换条目的分享状态，返回新的分享状态
pub async fn toggle_share(
    db: &DbState,
    id: i32,
) -> Result<bool, AppError> {
    let record = Entity::find_by_id(id)
        .one(&db.conn)
        .await?
        .ok_or(AppError::NotFound)?;

    let new_share = if record.is_shared == 1 { 0 } else { 1 };

    let mut active: ActiveModel = record.into();
    active.is_shared = Set(new_share);
    active.update(&db.conn).await?;

    Ok(new_share == 1)
}

/// 删除指定 ID 的条目
pub async fn delete_item(db: &DbState, id: i32, cache_dir: &str) -> Result<(), AppError> {
    // 先查询记录（获取 data 字段）
    let record = Entity::find_by_id(id)
        .one(&db.conn)
        .await?
        .ok_or(AppError::NotFound)?;

    // 如果是图片类型，尝试删除缓存文件（假设缓存文件以 hash 命名）
    if record.r#type == ClipboardType::Image as i32 {
        if let Some(data) = record.data.clone() {
            // 假设 data 存储的是文件路径
            let path_str = String::from_utf8(data).unwrap_or_default();
            let path = std::path::Path::new(&path_str);
            if path.exists() && path.starts_with(cache_dir) {
                let _ = std::fs::remove_file(path); // 忽略错误，可能文件已被删
            }
        }
    }

    // 最后删除数据库记录
    record.delete(&db.conn).await?;
    Ok(())
}