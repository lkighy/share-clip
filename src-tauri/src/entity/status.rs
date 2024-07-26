use sea_orm::{DeriveActiveEnum, EnumIter};

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum ClipboardType {
    #[sea_orm(string_value = "Html")]
    Html,
    /// 富文本
    #[sea_orm(string_value = "RichText")]
    RichText,
    /// 普通文本
    #[sea_orm(string_value = "Text")]
    Text,
    /// 图片
    #[sea_orm(string_value = "Image")]
    Image,
    /// 文件
    #[sea_orm(string_value = "File")]
    File,
}
