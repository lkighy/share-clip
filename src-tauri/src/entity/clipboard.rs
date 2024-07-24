use chrono::NaiveDateTime;
// use sea_orm::{ActiveModelBehavior, DeriveEntityModel, DeriveRelation, EnumIter};
use crate::entity::status::ClipboardType;
use sea_orm::entity::prelude::*;
use sea_orm::EntityTrait;
use sea_orm::PrimaryKeyTrait;
// 剪切板结构
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "clipboard")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i32,
    /// 复制的内容
    pub content: String,
    /// 复制的内容的类型
    pub content_type: ClipboardType,
    /// 创建时间
    pub created_at: NaiveDateTime,
    /// 更新时间，如果需要复制的内容显示在前面，则更新该字段
    pub updated_at: NaiveDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
