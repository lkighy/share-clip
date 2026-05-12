use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 剪贴板记录主表
        manager
            .create_table(
                Table::create()
                    .table(ClipboardRecord::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ClipboardRecord::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ClipboardRecord::Type).integer().not_null())
                    .col(ColumnDef::new(ClipboardRecord::Data).binary())
                    .col(ColumnDef::new(ClipboardRecord::Preview).text())
                    .col(ColumnDef::new(ClipboardRecord::Hash).string().unique_key())
                    .col(ColumnDef::new(ClipboardRecord::Size).big_integer())
                    .col(ColumnDef::new(ClipboardRecord::SourceApp).string())
                    .col(
                        ColumnDef::new(ClipboardRecord::CreatedAt)
                            .big_integer()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(ClipboardRecord::LastAccessedAt)
                            .big_integer()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(ClipboardRecord::AccessCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ClipboardRecord::IsFavorite)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ClipboardRecord::IsShared)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ClipboardRecord::IsValid)
                            .integer()
                            .not_null()
                            .default(1),
                    )
                    .to_owned(),
            )
            .await?;

        // 剪贴板记录索引
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
            .await?;

        // 本地文件映射表（用于维护剪贴板条目与文件实体关系）
        manager
            .create_table(
                Table::create()
                    .table(LocalFiles::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(LocalFiles::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(LocalFiles::Path).text().not_null())
                    .col(ColumnDef::new(LocalFiles::Type).integer().not_null().default(0))
                    .col(ColumnDef::new(LocalFiles::CreatedAt).big_integer().not_null())
                    .col(ColumnDef::new(LocalFiles::AccessCount).integer().not_null().default(0))
                    .col(ColumnDef::new(LocalFiles::IsValid).integer().not_null().default(1))
                    .col(ColumnDef::new(LocalFiles::Size).big_integer())
                    .col(ColumnDef::new(LocalFiles::SourceClipboardId).text())
                    .col(ColumnDef::new(LocalFiles::SourceType).integer().not_null().default(0))
                    .col(ColumnDef::new(LocalFiles::IsFavorite).integer().not_null().default(0))
                    .col(ColumnDef::new(LocalFiles::ShareMode).integer().not_null().default(0))
                    .col(ColumnDef::new(LocalFiles::ExpiresAt).big_integer())
                    .col(ColumnDef::new(LocalFiles::UpdatedAt).big_integer())
                    .to_owned(),
            )
            .await?;

        // 本地文件路径索引
        manager
            .create_index(
                Index::create()
                    .name("idx_local_files_path")
                    .table(LocalFiles::Table)
                    .col(LocalFiles::Path)
                    .to_owned(),
            )
            .await?;

        // 分享给我的文件缓存表
        manager
            .create_table(
                Table::create()
                    .table(SharedFiles::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(SharedFiles::UserId).string().not_null())
                    .col(ColumnDef::new(SharedFiles::Id).string().not_null())
                    .col(ColumnDef::new(SharedFiles::Path).text().not_null())
                    .col(ColumnDef::new(SharedFiles::Type).integer().not_null().default(0))
                    .col(ColumnDef::new(SharedFiles::CreatedAt).big_integer().not_null())
                    .col(ColumnDef::new(SharedFiles::AccessCount).integer().not_null().default(0))
                    .col(ColumnDef::new(SharedFiles::Size).big_integer())
                    .col(ColumnDef::new(SharedFiles::CacheStatus).integer().not_null().default(0))
                    .col(ColumnDef::new(SharedFiles::RemoteName).text())
                    .col(ColumnDef::new(SharedFiles::RemoteType).integer().not_null().default(0))
                    .col(ColumnDef::new(SharedFiles::RemoteSize).big_integer())
                    .col(ColumnDef::new(SharedFiles::RemoteUpdatedAt).big_integer())
                    .col(ColumnDef::new(SharedFiles::LastAccessedAt).big_integer())
                    .col(ColumnDef::new(SharedFiles::SyncPolicy).integer().not_null().default(0))
                    .primary_key(
                        Index::create()
                            .col(SharedFiles::UserId)
                            .col(SharedFiles::Id),
                    )
                    .to_owned(),
            )
            .await?;

        // 分享文件时间索引
        manager
            .create_index(
                Index::create()
                    .name("idx_shared_files_created_at")
                    .table(SharedFiles::Table)
                    .col(SharedFiles::CreatedAt)
                    .to_owned(),
            )
            .await?;

        // 我主动连接的远端用户信息
        manager
            .create_table(
                Table::create()
                    .table(OutboundConnections::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OutboundConnections::UserId)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(OutboundConnections::UserName).string().not_null())
                    .col(ColumnDef::new(OutboundConnections::Password).string())
                    .col(ColumnDef::new(OutboundConnections::Ip).string().not_null())
                    .col(ColumnDef::new(OutboundConnections::DeviceId).text())
                    .col(ColumnDef::new(OutboundConnections::DisplayName).text())
                    .col(ColumnDef::new(OutboundConnections::AuthToken).text())
                    .col(ColumnDef::new(OutboundConnections::AuthStatus).integer().not_null().default(0))
                    .col(ColumnDef::new(OutboundConnections::LastConnectedAt).big_integer())
                    .to_owned(),
            )
            .await?;

        // 连接到我的用户设备信息
        manager
            .create_table(
                Table::create()
                    .table(InboundConnections::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(InboundConnections::UserId)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(InboundConnections::IsShared).integer().not_null().default(0))
                    .col(ColumnDef::new(InboundConnections::IsTrusted).integer().not_null().default(0))
                    .col(ColumnDef::new(InboundConnections::Ip).string().not_null())
                    .col(ColumnDef::new(InboundConnections::DeviceId).text())
                    .col(ColumnDef::new(InboundConnections::UserName).text())
                    .col(ColumnDef::new(InboundConnections::AuthStatus).integer().not_null().default(0))
                    .col(ColumnDef::new(InboundConnections::GrantedAt).big_integer())
                    .col(ColumnDef::new(InboundConnections::RevokedAt).big_integer())
                    .col(ColumnDef::new(InboundConnections::LastSeenAt).big_integer())
                    .to_owned(),
            )
            .await?;

        // 连接操作日志
        manager
            .create_table(
                Table::create()
                    .table(ConnectionLog::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(ConnectionLog::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(ConnectionLog::UserId).string().not_null())
                    .col(ColumnDef::new(ConnectionLog::DeviceId).string())
                    .col(ColumnDef::new(ConnectionLog::Action).string().not_null())
                    .col(ColumnDef::new(ConnectionLog::Timestamp).big_integer().not_null())
                    .col(ColumnDef::new(ConnectionLog::Ip).string())
                    .to_owned(),
            )
            .await?;

        // 按用户和时间查询日志的复合索引
        manager
            .create_index(
                Index::create()
                    .name("idx_connection_log_user_time")
                    .table(ConnectionLog::Table)
                    .col(ConnectionLog::UserId)
                    .col(ConnectionLog::Timestamp)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(LocalFileIndex::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(LocalFileIndex::LocalFileId).string().not_null())
                    .col(ColumnDef::new(LocalFileIndex::RelativePath).text().not_null())
                    .col(ColumnDef::new(LocalFileIndex::AbsolutePath).text().not_null())
                    .col(ColumnDef::new(LocalFileIndex::Size).big_integer().not_null())
                    .col(ColumnDef::new(LocalFileIndex::Mtime).big_integer().not_null())
                    .col(ColumnDef::new(LocalFileIndex::IsDir).integer().not_null().default(0))
                    .col(ColumnDef::new(LocalFileIndex::Hash).string())
                    .col(ColumnDef::new(LocalFileIndex::Dirty).integer().not_null().default(1))
                    .col(ColumnDef::new(LocalFileIndex::ExistsFlag).integer().not_null().default(1))
                    .col(ColumnDef::new(LocalFileIndex::LastSeenAt).big_integer().not_null())
                    .col(ColumnDef::new(LocalFileIndex::LastHashedAt).big_integer())
                    .primary_key(
                        Index::create()
                            .col(LocalFileIndex::LocalFileId)
                            .col(LocalFileIndex::RelativePath),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_local_file_index_absolute_path")
                    .table(LocalFileIndex::Table)
                    .col(LocalFileIndex::AbsolutePath)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_local_file_index_dirty")
                    .table(LocalFileIndex::Table)
                    .col(LocalFileIndex::Dirty)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(SharedFileIndex::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(SharedFileIndex::UserId).string().not_null())
                    .col(ColumnDef::new(SharedFileIndex::SharedFileId).string().not_null())
                    .col(ColumnDef::new(SharedFileIndex::RelativePath).text().not_null())
                    .col(ColumnDef::new(SharedFileIndex::Name).text().not_null())
                    .col(ColumnDef::new(SharedFileIndex::IsDir).integer().not_null().default(0))
                    .col(ColumnDef::new(SharedFileIndex::LocalCachePath).text())
                    .col(ColumnDef::new(SharedFileIndex::Size).big_integer())
                    .col(ColumnDef::new(SharedFileIndex::Mtime).big_integer())
                    .col(ColumnDef::new(SharedFileIndex::Hash).string())
                    .col(ColumnDef::new(SharedFileIndex::RemoteDeleted).integer().not_null().default(0))
                    .col(ColumnDef::new(SharedFileIndex::CacheStatus).integer().not_null().default(0))
                    .col(ColumnDef::new(SharedFileIndex::LastAccessedAt).big_integer())
                    .col(ColumnDef::new(SharedFileIndex::UpdatedAt).big_integer())
                    .primary_key(
                        Index::create()
                            .col(SharedFileIndex::UserId)
                            .col(SharedFileIndex::SharedFileId)
                            .col(SharedFileIndex::RelativePath),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_shared_file_index_cache_status")
                    .table(SharedFileIndex::Table)
                    .col(SharedFileIndex::CacheStatus)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SharedFileIndex::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(LocalFileIndex::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ConnectionLog::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(InboundConnections::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(OutboundConnections::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(SharedFiles::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(LocalFiles::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ClipboardRecord::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ClipboardRecord {
    Table,
    // 自增主键
    Id,
    // 0=text, 1=html, 2=rtf, 3=image, 4=file, 5=folder
    Type,
    // 原始数据（二进制）
    Data,
    // UI 预览数据
    Preview,
    // 去重哈希
    Hash,
    // 原始大小
    Size,
    // 来源应用
    SourceApp,
    // 创建时间（时间戳）
    CreatedAt,
    // 最近访问时间（时间戳）
    LastAccessedAt,
    // 访问次数
    AccessCount,
    // 是否收藏：0/1
    IsFavorite,
    // 是否分享：0/1
    IsShared,
    // 是否有效：0/1
    IsValid,
}

#[derive(DeriveIden)]
enum LocalFiles {
    Table,
    // 文件ID（uuid）
    Id,
    // 文件路径
    Path,
    // 数据类型：0=file, 1=directory, 2=image, 3=video, 4=audio
    Type,
    // 创建时间（时间戳）
    CreatedAt,
    // 访问次数
    AccessCount,
    // 是否有效：0/1
    IsValid,
    // 文件大小
    Size,
    // 来源剪贴板ID列表（JSON）
    SourceClipboardId,
    // 来源类型：0=直接文件，1=剪贴板条目
    SourceType,
    // 是否收藏：0/1
    IsFavorite,
    // 分享模式：0=手动长期分享，1=剪贴板临时分享
    ShareMode,
    // 临时分享过期时间
    ExpiresAt,
    // 更新时间
    UpdatedAt,
}

#[derive(DeriveIden)]
enum SharedFiles {
    Table,
    // 来源用户ID（machine-uid）
    UserId,
    // 文件ID（uuid）
    Id,
    // 本地缓存路径
    Path,
    // 数据类型：0=file, 1=directory, 2=image, 3=video, 4=audio
    Type,
    // 创建时间（时间戳）
    CreatedAt,
    // 访问次数
    AccessCount,
    // 文件大小
    Size,
    // 缓存状态：0=NotCached, 1=Caching, 2=Cached, 3=Failed
    CacheStatus,
    // 远端展示名
    RemoteName,
    // 远端类型
    RemoteType,
    // 远端大小
    RemoteSize,
    // 远端更新时间
    RemoteUpdatedAt,
    // 最近访问时间
    LastAccessedAt,
    // 同步策略：0=按需同步，1=保持同步
    SyncPolicy,
}

#[derive(DeriveIden)]
enum OutboundConnections {
    Table,
    // 远端用户ID
    UserId,
    // 远端用户名称
    UserName,
    // 连接密码（可为空）
    Password,
    // 远端IP
    Ip,
    DeviceId,
    DisplayName,
    AuthToken,
    AuthStatus,
    LastConnectedAt,
}

#[derive(DeriveIden)]
enum InboundConnections {
    Table,
    // 对端用户ID
    UserId,
    // 是否授权共享：0/1
    IsShared,
    // 是否信任设备：0/1
    IsTrusted,
    // 对端IP
    Ip,
    DeviceId,
    UserName,
    AuthStatus,
    GrantedAt,
    RevokedAt,
    LastSeenAt,
}

#[derive(DeriveIden)]
pub enum ConnectionLog {
    Table,
    // 日志ID（uuid）
    Id,
    // 用户ID
    UserId,
    // 设备ID
    DeviceId,
    // 动作类型
    Action,
    // 发生时间（时间戳）
    Timestamp,
    // 对端IP
    Ip,
}

#[derive(DeriveIden)]
enum LocalFileIndex {
    Table,
    LocalFileId,
    RelativePath,
    AbsolutePath,
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
enum SharedFileIndex {
    Table,
    UserId,
    SharedFileId,
    RelativePath,
    Name,
    IsDir,
    LocalCachePath,
    Size,
    Mtime,
    Hash,
    RemoteDeleted,
    CacheStatus,
    LastAccessedAt,
    UpdatedAt,
}
