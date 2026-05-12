//! `SeaORM` Entity for shared_file_index.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "shared_file_index")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub shared_file_id: String,
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub relative_path: String,
    #[sea_orm(column_type = "Text")]
    pub name: String,
    pub is_dir: i32,
    #[sea_orm(column_type = "Text", nullable)]
    pub local_cache_path: Option<String>,
    pub size: Option<i64>,
    pub mtime: Option<i64>,
    pub hash: Option<String>,
    pub remote_deleted: i32,
    pub cache_status: i32,
    pub last_accessed_at: Option<i64>,
    pub updated_at: Option<i64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
