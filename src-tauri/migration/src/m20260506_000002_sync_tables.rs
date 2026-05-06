use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(SyncFileIndex::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SyncFileIndex::Path)
                            .text()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(SyncFileIndex::RootPath).text().not_null())
                    .col(ColumnDef::new(SyncFileIndex::Size).big_integer().not_null())
                    .col(ColumnDef::new(SyncFileIndex::Mtime).big_integer().not_null())
                    .col(
                        ColumnDef::new(SyncFileIndex::IsDir)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(SyncFileIndex::Hash).string())
                    .col(
                        ColumnDef::new(SyncFileIndex::Dirty)
                            .integer()
                            .not_null()
                            .default(1),
                    )
                    .col(
                        ColumnDef::new(SyncFileIndex::ExistsFlag)
                            .integer()
                            .not_null()
                            .default(1),
                    )
                    .col(ColumnDef::new(SyncFileIndex::LastSeenAt).big_integer().not_null())
                    .col(ColumnDef::new(SyncFileIndex::LastHashedAt).big_integer())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_sync_file_index_root_path")
                    .table(SyncFileIndex::Table)
                    .col(SyncFileIndex::RootPath)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_sync_file_index_dirty")
                    .table(SyncFileIndex::Table)
                    .col(SyncFileIndex::Dirty)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_sync_file_index_mtime")
                    .table(SyncFileIndex::Table)
                    .col(SyncFileIndex::Mtime)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(SyncRoots::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SyncRoots::RootPath)
                            .text()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(SyncRoots::SourceClipboardId).integer())
                    .col(
                        ColumnDef::new(SyncRoots::Enabled)
                            .integer()
                            .not_null()
                            .default(1),
                    )
                    .col(ColumnDef::new(SyncRoots::UpdatedAt).big_integer().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_sync_roots_enabled")
                    .table(SyncRoots::Table)
                    .col(SyncRoots::Enabled)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(SyncHashQueue::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SyncHashQueue::Path)
                            .text()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(SyncHashQueue::Priority).integer().not_null().default(0))
                    .col(ColumnDef::new(SyncHashQueue::CreatedAt).big_integer().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_sync_hash_queue_priority_created")
                    .table(SyncHashQueue::Table)
                    .col(SyncHashQueue::Priority)
                    .col(SyncHashQueue::CreatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SyncHashQueue::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(SyncRoots::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(SyncFileIndex::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum SyncFileIndex {
    Table,
    Path,
    RootPath,
    Size,
    Mtime,
    IsDir,
    Hash,
    Dirty,
    ExistsFlag,
    LastSeenAt,
    LastHashedAt,
}

#[derive(DeriveIden)]
enum SyncRoots {
    Table,
    RootPath,
    SourceClipboardId,
    Enabled,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum SyncHashQueue {
    Table,
    Path,
    Priority,
    CreatedAt,
}

