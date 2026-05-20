#![allow(dead_code)]

use crate::entity::clipboard_record::{Column, Entity, Model};
use sea_orm::{
    ColumnTrait, DatabaseConnection, DbErr, EntityTrait, FromQueryResult, QueryFilter, QueryOrder,
    QuerySelect,
};
// 假设事件枚举已包含以下变体

#[derive(Debug, Clone, FromQueryResult)]
pub struct RecordSummary {
    pub id: i32,
    #[sea_orm(column_name = "type")]
    pub r#type: i32,
    pub preview: Option<String>,
    pub hash: Option<String>,
    pub size: Option<i64>,
    pub source_app: Option<String>,
    pub created_at: i64,
    pub last_accessed_at: i64,
    pub access_count: i32,
    pub is_favorite: i32,
    pub is_shared: i32,
    pub is_valid: i32,
}

impl RecordSummary {
    pub fn into_model(self) -> Model {
        Model {
            id: self.id,
            r#type: self.r#type,
            data: None,
            preview: self.preview,
            hash: self.hash,
            size: self.size,
            source_app: self.source_app,
            created_at: self.created_at,
            last_accessed_at: self.last_accessed_at,
            access_count: self.access_count,
            is_favorite: self.is_favorite,
            is_shared: self.is_shared,
            is_valid: self.is_valid,
        }
    }
}

// 获取列表
pub async fn list_latest(
    conn: &DatabaseConnection,
    limit: u64,
    offset: u64,
) -> Result<Vec<Model>, DbErr> {
    Entity::find()
        .order_by_desc(Column::IsFavorite)
        .order_by_desc(Column::LastAccessedAt)
        .order_by_desc(Column::CreatedAt)
        .offset(offset)
        .limit(limit)
        .all(conn)
        .await
}

pub async fn list_latest_summaries(
    conn: &DatabaseConnection,
    limit: u64,
    offset: u64,
) -> Result<Vec<RecordSummary>, DbErr> {
    Entity::find()
        .select_only()
        .column(Column::Id)
        .column(Column::Type)
        .column(Column::Preview)
        .column(Column::Hash)
        .column(Column::Size)
        .column(Column::SourceApp)
        .column(Column::CreatedAt)
        .column(Column::LastAccessedAt)
        .column(Column::AccessCount)
        .column(Column::IsFavorite)
        .column(Column::IsShared)
        .column(Column::IsValid)
        .order_by_desc(Column::IsFavorite)
        .order_by_desc(Column::LastAccessedAt)
        .order_by_desc(Column::CreatedAt)
        .offset(offset)
        .limit(limit)
        .into_model::<RecordSummary>()
        .all(conn)
        .await
}

pub async fn list_shared_summaries(
    conn: &DatabaseConnection,
    limit: u64,
    offset: u64,
) -> Result<Vec<RecordSummary>, DbErr> {
    Entity::find()
        .select_only()
        .column(Column::Id)
        .column(Column::Type)
        .column(Column::Preview)
        .column(Column::Hash)
        .column(Column::Size)
        .column(Column::SourceApp)
        .column(Column::CreatedAt)
        .column(Column::LastAccessedAt)
        .column(Column::AccessCount)
        .column(Column::IsFavorite)
        .column(Column::IsShared)
        .column(Column::IsValid)
        .filter(Column::IsShared.eq(1))
        .order_by_desc(Column::IsFavorite)
        .order_by_desc(Column::LastAccessedAt)
        .order_by_desc(Column::CreatedAt)
        .offset(offset)
        .limit(limit)
        .into_model::<RecordSummary>()
        .all(conn)
        .await
}

// 查询单个数据
pub async fn select_by_id(conn: &DatabaseConnection, id: i32) -> Result<Option<Model>, DbErr> {
    Entity::find_by_id(id).one(conn).await
}
