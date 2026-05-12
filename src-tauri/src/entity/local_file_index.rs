//! `SeaORM` Entity for local_file_index.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "local_file_index")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub local_file_id: String,
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub relative_path: String,
    #[sea_orm(column_type = "Text")]
    pub absolute_path: String,
    pub size: i64,
    pub mtime: i64,
    pub is_dir: i32,
    pub hash: Option<String>,
    pub dirty: i32,
    pub exists_flag: i32,
    pub last_seen_at: i64,
    pub last_hashed_at: Option<i64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
