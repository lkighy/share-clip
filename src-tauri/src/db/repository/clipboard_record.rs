#![allow(dead_code)]

use crate::entity::clipboard_record::{self, Column, Entity, Model};
use sea_orm::prelude::DateTimeUtc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::NotSet, ActiveValue::Set, ColumnTrait, DatabaseConnection,
    DbErr, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};

pub struct CreateClipboardRecordInput {
    pub id: String,
    pub r#type: String,
    pub data: Option<Vec<u8>>,
    pub file_path: Option<String>,
    pub preview: Option<String>,
    pub size: Option<i64>,
    pub metadata: Option<String>,
    pub source_app: Option<String>,
    pub created_at: Option<DateTimeUtc>,
    pub last_accessed_at: Option<DateTimeUtc>,
    pub access_count: Option<i32>,
    pub is_favorite: Option<i32>,
    pub sync_status: Option<String>,
    pub sync_version: Option<i32>,
}

#[derive(Default)]
pub struct UpdateClipboardRecordInput {
    pub r#type: Option<String>,
    pub data: Option<Option<Vec<u8>>>,
    pub file_path: Option<Option<String>>,
    pub preview: Option<Option<String>>,
    pub size: Option<Option<i64>>,
    pub metadata: Option<Option<String>>,
    pub source_app: Option<Option<String>>,
    pub last_accessed_at: Option<Option<DateTimeUtc>>,
    pub access_count: Option<i32>,
    pub is_favorite: Option<i32>,
    pub sync_status: Option<String>,
    pub sync_version: Option<i32>,
}

pub async fn create(
    conn: &DatabaseConnection,
    input: CreateClipboardRecordInput,
) -> Result<Model, DbErr> {
    let inserted_id = input.id.clone();
    let mut active = clipboard_record::ActiveModel {
        id: Set(input.id),
        r#type: Set(input.r#type),
        data: Set(input.data),
        file_path: Set(input.file_path),
        preview: Set(input.preview),
        size: Set(input.size),
        metadata: Set(input.metadata),
        source_app: Set(input.source_app),
        last_accessed_at: Set(input.last_accessed_at),
        ..Default::default()
    };

    if let Some(created_at) = input.created_at {
        active.created_at = Set(created_at);
    }
    if let Some(access_count) = input.access_count {
        active.access_count = Set(access_count);
    }
    if let Some(is_favorite) = input.is_favorite {
        active.is_favorite = Set(is_favorite);
    }
    if let Some(sync_status) = input.sync_status {
        active.sync_status = Set(sync_status);
    }
    if let Some(sync_version) = input.sync_version {
        active.sync_version = Set(sync_version);
    }

    active.insert(conn).await?;
    get_by_id(conn, &inserted_id).await?.ok_or_else(|| {
        DbErr::Custom("clipboard_record inserted but not found when reading back".to_string())
    })
}

pub async fn get_by_id(conn: &DatabaseConnection, id: &str) -> Result<Option<Model>, DbErr> {
    Entity::find_by_id(id.to_owned()).one(conn).await
}

pub async fn list_latest(
    conn: &DatabaseConnection,
    limit: u64,
    offset: u64,
) -> Result<Vec<Model>, DbErr> {
    Entity::find()
        .order_by_desc(Column::CreatedAt)
        .offset(offset)
        .limit(limit)
        .all(conn)
        .await
}

pub async fn update_by_id(
    conn: &DatabaseConnection,
    id: &str,
    patch: UpdateClipboardRecordInput,
) -> Result<Option<Model>, DbErr> {
    let Some(model) = get_by_id(conn, id).await? else {
        return Ok(None);
    };

    let mut active: clipboard_record::ActiveModel = model.into();

    if let Some(v) = patch.r#type {
        active.r#type = Set(v);
    }
    if let Some(v) = patch.data {
        active.data = Set(v);
    }
    if let Some(v) = patch.file_path {
        active.file_path = Set(v);
    }
    if let Some(v) = patch.preview {
        active.preview = Set(v);
    }
    if let Some(v) = patch.size {
        active.size = Set(v);
    }
    if let Some(v) = patch.metadata {
        active.metadata = Set(v);
    }
    if let Some(v) = patch.source_app {
        active.source_app = Set(v);
    }
    if let Some(v) = patch.last_accessed_at {
        active.last_accessed_at = Set(v);
    }
    if let Some(v) = patch.access_count {
        active.access_count = Set(v);
    }
    if let Some(v) = patch.is_favorite {
        active.is_favorite = Set(v);
    }
    if let Some(v) = patch.sync_status {
        active.sync_status = Set(v);
    }
    if let Some(v) = patch.sync_version {
        active.sync_version = Set(v);
    }

    active.update(conn).await.map(Some)
}

pub async fn delete_by_id(conn: &DatabaseConnection, id: &str) -> Result<u64, DbErr> {
    let result = Entity::delete_many()
        .filter(Column::Id.eq(id.to_owned()))
        .exec(conn)
        .await?;
    Ok(result.rows_affected)
}

pub async fn clear_all(conn: &DatabaseConnection) -> Result<u64, DbErr> {
    let result = Entity::delete_many().exec(conn).await?;
    Ok(result.rows_affected)
}

pub fn reset_defaults_for_insert(active: &mut clipboard_record::ActiveModel) {
    // Keep database defaults for these fields unless caller explicitly sets them.
    active.created_at = NotSet;
    active.access_count = NotSet;
    active.is_favorite = NotSet;
    active.sync_status = NotSet;
    active.sync_version = NotSet;
}
