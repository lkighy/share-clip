#![allow(dead_code)]

use crate::entity::clipboard_record::{self, Column, Entity, Model};
use sea_orm::prelude::DateTimeUtc;
use sea_orm::{ActiveModelTrait, ActiveValue::NotSet, ActiveValue::Set, ColumnTrait, DatabaseConnection, DbConn, DbErr, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use crate::entity::prelude::ClipboardRecord;
use crate::models::clipboard::ClipboardResponse;
// 假设事件枚举已包含以下变体

// 获取列表
pub async fn list_latest(
    conn: &DatabaseConnection,
    limit: u64,
    offset: u64,
) -> Result<Vec<ClipboardResponse>, DbErr> {
    Entity::find()
        .order_by_desc(Column::IsFavorite)
        .order_by_desc(Column::LastAccessedAt)
        .order_by_desc(Column::CreatedAt)
        .offset(offset)
        .limit(limit)
        .column(Column::Id)
        .column(Column::Type)
        .column(Column::Preview)
        .column(Column::Size)
        .column(Column::SourceApp)
        .column(Column::CreatedAt)
        .column(Column::LastAccessedAt)
        .column(Column::AccessCount)
        .column(Column::IsFavorite)
        .column(Column::IsShared)
        .column(Column::IsValid)
        .into_model::<ClipboardResponse>()
        .all(conn)
        .await
}

// 查询单个数据
pub async fn select_by_id(
    conn: &DatabaseConnection,
    id: i32,
) -> Result<Option<Model>, DbErr> {
    Entity::find_by_id(id).one(conn).await
}