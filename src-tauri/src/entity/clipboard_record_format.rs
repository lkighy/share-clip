//! `SeaORM` Entity for clipboard_record_format.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "clipboard_record_format")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub clipboard_id: i32,
    pub format: String,
    pub format_name: Option<String>,
    #[sea_orm(column_type = "Binary(1)")]
    pub data: Vec<u8>,
    pub hash: Option<String>,
    pub size: Option<i64>,
    pub priority: i32,
    pub created_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
