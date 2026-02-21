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
                    .col(
                        ColumnDef::new(ClipboardRecord::Id)
                            .auto_increment()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ClipboardRecord::Type).string().not_null())
                    .col(binary_null(ClipboardRecord::Data))
                    .col(text_null(ClipboardRecord::FilePath))
                    .col(text_null(ClipboardRecord::Preview))
                    .col(big_integer_null(ClipboardRecord::Size))
                    .col(text_null(ClipboardRecord::Metadata))
                    .col(string_null(ClipboardRecord::SourceApp))
                    .col(
                        timestamp(ClipboardRecord::CreatedAt)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(timestamp_null(ClipboardRecord::LastAccessedAt))
                    .col(integer(ClipboardRecord::AccessCount).not_null().default(0))
                    .col(integer(ClipboardRecord::IsFavorite).not_null().default(0))
                    .col(integer(ClipboardRecord::SyncVersion).not_null().default(1))
                    .to_owned(),
            )
            .await?;

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
    // ID
    Id,
    // 类型
    Type,
    // 数据
    Data,
    // 文件路径
    FilePath,
    // 预览
    Preview,
    // 尺寸
    Size,
    // 元数据
    Metadata,
    // 来源应用
    SourceApp,
    // 创建时间
    CreatedAt,
    // 最后访问时间
    LastAccessedAt,
    // 被粘贴的次数
    AccessCount,
    // 收藏标志
    IsFavorite,
    // 数据版本号，避免冲出
    SyncVersion,
}
