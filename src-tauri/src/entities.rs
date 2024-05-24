use sea_orm::{ActiveModelBehavior, DeriveActiveEnum, DeriveEntity, DeriveRelation, EnumIter};
use sea_orm::prelude::DateTime;
#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Display)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum ClipboardType {
    #[sea_orm(string_value = "Text")]
    Text,
    #[sea_orm(string_value = "Image")]
    Image,
    #[sea_orm(string_value = "File")]
    File,
}

// 剪切板结构
#[derive(Clone, Debug, DeriveEntity)]
#[sea_orm(table_name = "clipboard")]
pub struct ClipboardModel {
    #[sea_orm(primary_key)]
    pub id: i32,
    /// 复制的内容
    pub content: String,
    /// 复制的内容的类型
    pub content_type: ClipboardType,
    /// 创建时间
    pub created_at: DateTime,
    /// 更新时间，如果需要复制的内容显示在前面，则更新该字段
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ClipboardModel {}
