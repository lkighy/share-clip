use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ClipboardRecord::Table)
                    .if_not_exists()
                    // 主键
                    .col(
                        ColumnDef::new(ClipboardRecord::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    // 类型: 0=text,1=html,2=image,3=file,4=folder
                    .col(ColumnDef::new(ClipboardRecord::Type).integer().not_null())
                    // 小数据（文本/HTML/小图片）
                    .col(ColumnDef::new(ClipboardRecord::Data).binary())
                    // UI 预览
                    .col(ColumnDef::new(ClipboardRecord::Preview).text())
                    // hash 唯一约束
                    .col(ColumnDef::new(ClipboardRecord::Hash).string().unique_key())
                    // 原始数据大小
                    .col(ColumnDef::new(ClipboardRecord::Size).big_integer())
                    // 来源应用
                    .col(ColumnDef::new(ClipboardRecord::SourceApp).string())
                    // 创建时间
                    .col(
                        ColumnDef::new(ClipboardRecord::CreatedAt)
                            .big_integer()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    // 最后访问时间
                    .col(ColumnDef::new(ClipboardRecord::LastAccessedAt).big_integer().not_null().default(Expr::current_timestamp()))
                    // 被粘贴的次数
                    .col(ColumnDef::new(ClipboardRecord::AccessCount).integer().not_null().default(0))
                    // 收藏标志
                    .col(ColumnDef::new(ClipboardRecord::IsFavorite).integer().not_null().default(0))
                    // 是否共享
                    .col(ColumnDef::new(ClipboardRecord::IsShared).integer().not_null().default(0))
                    .to_owned(),
            )
            .await?;

        // 索引
        manager
            .create_index(
                Index::create()
                    .name("idx_clipboard_record_created_at")
                    .table(ClipboardRecord::Table)
                    .col(ClipboardRecord::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_clipboard_record_hash")
                    .table(ClipboardRecord::Table)
                    .col(ClipboardRecord::Hash)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_clipboard_record_type_created")
                    .table(ClipboardRecord::Table)
                    .col(ClipboardRecord::Type)
                    .col(ClipboardRecord::CreatedAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ClipboardRecord::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ClipboardRecord {
    Table,
    Id,
    // 0=text, 1=html, 2=rtf, 3=image, 4=file, 5=folder
    Type,
    // 小数据（文本 / HTML / 小图片）
    Data,
    // UI 预览
    Preview,
    // 内容哈希，用于去重
    Hash,
    // 原始数据大小
    Size,
    // 来源应用
    SourceApp,
    // 创建时间
    CreatedAt,
    // 最后访问时间
    LastAccessedAt,
    // 访问次数
    AccessCount,
    // 是否收藏
    IsFavorite,
    // 是否分享
    IsShared,
}