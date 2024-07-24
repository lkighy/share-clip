use sea_orm::{DeriveActiveEnum, EnumIter};

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum ClipboardType {
    #[sea_orm(string_value = "Text")]
    Text,
    #[sea_orm(string_value = "Image")]
    Image,
    #[sea_orm(string_value = "File")]
    File,
}
