use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ClipboardRecordFormat::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ClipboardRecordFormat::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ClipboardRecordFormat::ClipboardId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ClipboardRecordFormat::Format)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ClipboardRecordFormat::FormatName).string())
                    .col(
                        ColumnDef::new(ClipboardRecordFormat::Data)
                            .binary()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ClipboardRecordFormat::Hash).string())
                    .col(ColumnDef::new(ClipboardRecordFormat::Size).big_integer())
                    .col(
                        ColumnDef::new(ClipboardRecordFormat::Priority)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ClipboardRecordFormat::CreatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_clipboard_record_format_clipboard_id")
                            .from(
                                ClipboardRecordFormat::Table,
                                ClipboardRecordFormat::ClipboardId,
                            )
                            .to(ClipboardRecord::Table, ClipboardRecord::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_clipboard_record_format_clipboard_id")
                    .table(ClipboardRecordFormat::Table)
                    .col(ClipboardRecordFormat::ClipboardId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_clipboard_record_format_unique")
                    .table(ClipboardRecordFormat::Table)
                    .col(ClipboardRecordFormat::ClipboardId)
                    .col(ClipboardRecordFormat::Format)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(ClipboardRecordFormat::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum ClipboardRecord {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum ClipboardRecordFormat {
    Table,
    Id,
    ClipboardId,
    Format,
    FormatName,
    Data,
    Hash,
    Size,
    Priority,
    CreatedAt,
}
