#![allow(dead_code)]


use crate::db::DbState;
use crate::entity::clipboard_record::Model;
use sea_orm::DbErr;
use crate::db::repository::clipboard_record;
use crate::entity::prelude::ClipboardRecord;
use crate::models::clipboard::ClipboardResponse;

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
pub async fn find_clipboard_record(
    db: &DbState,
    id: i32,
) -> Result<Option<Model>, DbErr> {
    clipboard_record::select_by_id(&db.conn, id).await
}

// pub async fn update_record(
//     db: &DbState,
//     id: &str,
//     patch: UpdateClipboardRecordInput,
// ) -> Result<Option<Model>, DbErr> {
//     clipboard_record::update_by_id(&db.conn, id, patch).await
// }
//
// pub async fn delete_record(db: &DbState, id: &str) -> Result<u64, DbErr> {
//     clipboard_record::delete_by_id(&db.conn, id).await
// }
