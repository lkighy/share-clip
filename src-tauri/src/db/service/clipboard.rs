#![allow(dead_code)]

use crate::db::repository::clipboard_record::{
    self, CreateClipboardRecordInput, UpdateClipboardRecordInput,
};
use crate::db::DbState;
use crate::entity::clipboard_record::Model;
use sea_orm::DbErr;

pub async fn create_record(
    db: &DbState,
    input: CreateClipboardRecordInput,
) -> Result<Model, DbErr> {
    clipboard_record::create(&db.conn, input).await
}

pub async fn get_record(db: &DbState, id: &str) -> Result<Option<Model>, DbErr> {
    clipboard_record::get_by_id(&db.conn, id).await
}

pub async fn list_records(
    db: &DbState,
    limit: u64,
    offset: u64,
) -> Result<Vec<Model>, DbErr> {
    clipboard_record::list_latest(&db.conn, limit, offset).await
}

pub async fn update_record(
    db: &DbState,
    id: &str,
    patch: UpdateClipboardRecordInput,
) -> Result<Option<Model>, DbErr> {
    clipboard_record::update_by_id(&db.conn, id, patch).await
}

pub async fn delete_record(db: &DbState, id: &str) -> Result<u64, DbErr> {
    clipboard_record::delete_by_id(&db.conn, id).await
}
