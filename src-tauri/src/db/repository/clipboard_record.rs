#![allow(dead_code)]

use crate::entity::clipboard_record::{Column, Entity, Model};
use sea_orm::{DatabaseConnection, DbErr, EntityTrait, QueryOrder, QuerySelect};
// 假设事件枚举已包含以下变体

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

// 查询单个数据
pub async fn select_by_id(conn: &DatabaseConnection, id: i32) -> Result<Option<Model>, DbErr> {
    Entity::find_by_id(id).one(conn).await
}
