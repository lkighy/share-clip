# 文件分享与按需同步重构方案

## 1. 重构目标

当前 `src-tauri/src/server` 已经实现了远程文件浏览、下载、同步索引和后台 hash，但实现主轴已经偏向“文件同步服务器”。本项目原始设计应以“局域网分享授权 + 剪贴板联动 + 本地/远端分享目录”为核心，同步系统只是建立在 `LocalFiles` 和 `SharedFiles` 之上的子能力。

本次重构目标：

- 本机开启分享服务器，可设置访问密码。
- 支持授权控制：自动授权、手动确认授权、撤回授权。
- `LocalFiles` 作为本机正在分享的主文件/主文件夹/图片/目录入口。
- `SharedFiles` 作为来自其他主机的分享入口和本地缓存记录。
- `OutboundConnections` 表示“我连接上的远端分享用户”。
- `InboundConnections` 表示“我同意访问我分享内容的远端用户”。
- `ConnectionLog` 记录连接、授权、拒绝、撤回、下载、同步等事件。
- 剪贴板复制文件、图片、文件夹时，可按配置自动挂载到 `LocalFiles`。
- 一次复制多个文件时，每个文件独立挂载到 `LocalFiles`，并记录来源剪贴板 ID。
- 剪贴板发生变动时，自动取消上一条剪贴板来源的临时分享。
- 浏览器可直接浏览和下载文件，但不参与同步。
- 客户端同步采用“按需同步”：用户远程确实访问文件或主动点击“同步到本地”后才下载。

## 2. 当前偏移点

当前代码中的偏移主要在这里：

- `sync_roots` 从 `clipboard_record.is_shared = 1` 反推分享根目录。
- `/files/list` 和 `/download` 使用 `sync_roots` 作为授权后的可访问范围。
- `sync_file_index` 以绝对路径为主键，缺少与 `LocalFiles` 的明确归属关系。
- 授权模型没有真正参与 HTTP 请求链路。
- 浏览器下载、客户端同步、剪贴板分享都混在同一批接口和同一个 root 解析流程中。

重构后应改为：

- `LocalFiles` 是本机分享根的唯一事实来源。
- `sync_file_index` 是 `LocalFiles` 的子表或派生索引。
- `SharedFiles` 是远端分享入口和本地缓存状态的唯一事实来源。
- `clipboard_record.is_shared` 只表达剪贴板条目是否允许被分享，不再直接作为服务器分享根。
- 授权中间件先判定连接身份，再访问 `LocalFiles`。

## 3. 核心领域模型

### 3.1 LocalFiles：本机分享入口

`LocalFiles` 代表本机对外分享的主文件、主文件夹、图片等入口。

现有字段可继续使用：

- `id`：分享入口 ID，远端访问时使用这个 ID，而不是直接暴露绝对路径。
- `path`：本机绝对路径，仅本地保存。
- `type`：`0=file, 1=directory, 2=image, 3=video, 4=audio`。
- `source_clipboard_id`：来源剪贴板 ID 列表，建议继续保存 JSON 数组。
- `source_type`：`0=手动分享, 1=剪贴板自动分享`。
- `is_valid`：是否仍然有效。

建议补充字段：

```sql
ALTER TABLE local_files ADD COLUMN share_mode INTEGER NOT NULL DEFAULT 0;
ALTER TABLE local_files ADD COLUMN expires_at INTEGER;
ALTER TABLE local_files ADD COLUMN updated_at INTEGER;
```

语义：

- `share_mode=0`：手动长期分享。
- `share_mode=1`：剪贴板临时分享。
- `expires_at`：可选，用于临时分享过期。
- `updated_at`：便于客户端增量拉取分享列表。

### 3.2 SharedFiles：远端分享入口和本地缓存

`SharedFiles` 表示“别的主机分享给我的文件/文件夹入口”，不是同步完成后的全量文件树。尤其当远端分享的是文件夹时，`SharedFiles` 只记录这个远端分享根入口；文件夹内部的子文件、子文件夹、缓存状态和远端变化状态应交给 `shared_file_index` 这类派生子表记录。

现有字段可继续使用：

- `user_id`：远端用户或设备 ID。
- `id`：远端 `LocalFiles.id`。
- `path`：本地缓存路径。未缓存时可为空或保存预期缓存路径。
- `cache_status`：`0=NotCached, 1=Caching, 2=Cached, 3=Failed`。

建议补充字段：

```sql
ALTER TABLE shared_files ADD COLUMN remote_name TEXT;
ALTER TABLE shared_files ADD COLUMN remote_type INTEGER NOT NULL DEFAULT 0;
ALTER TABLE shared_files ADD COLUMN remote_size INTEGER;
ALTER TABLE shared_files ADD COLUMN remote_updated_at INTEGER;
ALTER TABLE shared_files ADD COLUMN last_accessed_at INTEGER;
ALTER TABLE shared_files ADD COLUMN sync_policy INTEGER NOT NULL DEFAULT 0;
```

语义：

- `sync_policy=0`：按需同步。
- `sync_policy=1`：用户主动选择保持本地同步。
- `last_accessed_at`：远程访问或本地打开时更新，可作为按需下载触发依据。

### 3.3 OutboundConnections：我连接的远端用户

`OutboundConnections` 保存我主动连接过、可浏览其分享内容的远端主机。

建议补充字段：

```sql
ALTER TABLE outbound_connections ADD COLUMN device_id TEXT;
ALTER TABLE outbound_connections ADD COLUMN display_name TEXT;
ALTER TABLE outbound_connections ADD COLUMN auth_token TEXT;
ALTER TABLE outbound_connections ADD COLUMN auth_status INTEGER NOT NULL DEFAULT 0;
ALTER TABLE outbound_connections ADD COLUMN last_connected_at INTEGER;
```

`auth_status`：

- `0=pending`
- `1=authorized`
- `2=denied`
- `3=revoked`

密码不建议长期明文保存。后续应改为保存派生值或只保存会话 token。

### 3.4 InboundConnections：我授权的访问者

`InboundConnections` 保存远端设备访问我本机分享服务器的授权状态。

建议补充字段：

```sql
ALTER TABLE inbound_connections ADD COLUMN device_id TEXT;
ALTER TABLE inbound_connections ADD COLUMN user_name TEXT;
ALTER TABLE inbound_connections ADD COLUMN auth_status INTEGER NOT NULL DEFAULT 0;
ALTER TABLE inbound_connections ADD COLUMN granted_at INTEGER;
ALTER TABLE inbound_connections ADD COLUMN revoked_at INTEGER;
ALTER TABLE inbound_connections ADD COLUMN last_seen_at INTEGER;
```

其中现有 `is_shared` 可兼容为“是否允许访问分享内容”，`is_trusted` 可兼容为“下次自动授权”。

### 3.5 ConnectionLog：审计日志

`ConnectionLog` 应记录所有关键行为：

- `connect_requested`
- `connect_authorized`
- `connect_denied`
- `connect_revoked`
- `share_list_viewed`
- `file_browsed`
- `file_downloaded`
- `sync_requested`
- `sync_completed`
- `sync_failed`

日志中应包含 `user_id/device_id/ip/timestamp/action`。后续可加 `target_id` 和 `detail_json`，用于记录访问了哪个 `LocalFiles.id` 或哪条远端文件。

## 4. 服务器访问模型

### 4.1 分享服务器配置

新增或扩展配置：

```rust
ShareServerConfig {
    enabled: bool,
    bind_ip: String,
    port: u16,
    password_enabled: bool,
    password_hash: Option<String>,
    auth_mode: AuthMode,
    browser_access_enabled: bool,
    sync_access_enabled: bool,
}

AuthMode {
    AutoAllow,
    RequireConfirm,
}
```

推荐策略：

- 无密码：只允许本机或已信任设备访问，除非用户明确开启无密码 LAN 分享。
- 有密码 + 自动授权：远端输入正确密码后自动进入 `InboundConnections.authorized`。
- 有密码 + 手动确认：远端输入密码后创建 pending 请求，Tauri 前端弹窗确认。
- 撤回授权：将 `InboundConnections.auth_status` 改为 `revoked`，现有会话 token 立即失效。

### 4.2 HTTP 请求身份

远端访问应先建立会话：

1. `POST /api/auth/request`
2. 服务端校验密码。
3. 根据 `auth_mode` 自动授权或创建 pending 请求。
4. 授权成功后返回短期 `session_token`。
5. 后续接口通过 `Authorization: Bearer <token>` 访问。

浏览器访问可使用 cookie session，客户端访问可使用 bearer token。

## 5. 剪贴板联动设计

### 5.1 自动分享策略

新增配置：

```rust
ClipboardShareConfig {
    auto_share_files: bool,
    auto_share_images: bool,
    auto_share_folders: bool,
    unshare_on_clipboard_change: bool,
}
```

行为：

- 复制文件：若 `auto_share_files=true`，每个文件独立写入 `LocalFiles`。
- 复制图片文件：若 `auto_share_images=true`，每个图片独立写入 `LocalFiles`，类型为 `image`。
- 复制文件夹：若 `auto_share_folders=true`，每个文件夹独立写入 `LocalFiles`，类型为 `directory`。
- 一次复制多个路径：每个路径一个 `LocalFiles` 记录。
- 每条记录都写入 `source_clipboard_id=[clipboard_record.id]`，`source_type=1`，`share_mode=1`。

### 5.2 剪贴板变动时取消分享

当监听到新的剪贴板条目时：

1. 保存新的 `clipboard_record`。
2. 查找上一条 `source_type=1/share_mode=1` 且来源于旧 clipboard id 的 `LocalFiles`。
3. 如果该 `LocalFiles` 只来自旧 clipboard id，则置 `is_valid=0`。
4. 如果同一路径也被手动分享或被多个 clipboard id 引用，则只移除旧 clipboard id，保留有效分享。
5. 刷新该 `LocalFiles` 对应的同步索引状态。

注意：

- 手动分享永不因剪贴板变化被取消。
- 文件夹内部文件变化不应取消文件夹分享，只更新索引。
- 文件本体被删除时才将对应 `LocalFiles.is_valid=0`。

## 6. 浏览器浏览与下载

浏览器访问目标是“可直接看、可下载”，不做同步协议。

浏览器不应直接暴露完整 `clipboard_record` 历史。浏览器访问的对象应是已经进入 `LocalFiles` 的分享内容：剪贴板中的文件、图片文件、文件夹只有在被用户点击分享或按自动分享配置挂载到 `LocalFiles` 后，才会出现在浏览器分享页中。这样可以避免把本地剪贴板历史、文本内容、未分享文件误暴露给局域网访问者。

建议接口：

```text
GET  /                 浏览器文件分享首页
GET  /api/shares       列出 LocalFiles 分享入口
GET  /api/shares/:id   浏览单个分享入口
GET  /api/files/:id/list?path=relative/path
GET  /api/files/:id/download?path=relative/path
```

设计原则：

- URL 使用 `LocalFiles.id` + 相对路径，不暴露本机绝对路径。
- 每次请求都通过授权中间件检查 `session_token`。
- 路径解析必须 canonicalize，并确认目标仍在 `LocalFiles.path` 内。
- 下载使用流式响应，支持 `Range`。
- 浏览器访问只更新访问日志和访问次数，不写入 `SharedFiles`。

### 6.1 浏览器访问剪贴板分享的边界

浏览器访问剪贴板相关内容时只允许访问“剪贴板派生出来的分享项”，即 `LocalFiles.source_type=1` 且 `LocalFiles.is_valid=1` 的记录，或者用户手动分享的 `source_type=0` 记录。浏览器不提供“查看剪贴板历史”的完整入口。

推荐规则：

- 文本、HTML、RTF 剪贴板记录默认不通过浏览器暴露，除非后续明确增加“文本分享”类型和专门的访问页。
- 复制文件、图片文件、文件夹后，如果用户点击分享或配置了自动分享，则每个路径写入 `LocalFiles`。
- 浏览器首页只列出 `LocalFiles` 分享入口，不查询 `clipboard_record`。
- 分享页可以显示来源标识，例如“来自剪贴板临时分享”，但不能返回原始 `clipboard_record.data`。
- 剪贴板变化导致临时分享失效后，浏览器再次访问该分享 ID 应返回 `404` 或失效页面。

### 6.2 浏览器密码与会话方案

浏览器访问推荐使用 cookie session，不建议把 token 放在 URL 中。URL token 容易进入浏览器历史、代理日志、截图和下载来源记录。

后续应增加浏览器认证接口：

```text
GET  /login
POST /api/browser/auth/login
POST /api/browser/auth/logout
GET  /api/browser/auth/status
```

处理流程：

1. 浏览器访问 `/`。
2. 服务端读取分享服务器配置。
3. 如果 `browser_access_enabled=false`，返回 `403`。
4. 如果 `share_server_password_enabled=false`，允许直接浏览 `LocalFiles` 分享列表。
5. 如果启用了密码且请求没有有效 cookie session，返回登录页或 `401`。
6. 用户提交密码到 `POST /api/browser/auth/login`。
7. 服务端校验 `share_server_password_hash`。
8. 校验通过后创建短期 browser session，并写入 `HttpOnly` cookie。
9. 后续 `/api/shares`、`/api/files/:id/list`、`/api/files/:id/download` 通过 cookie 鉴权。

密码保存和校验规则：

- 配置中只保存密码派生值，不保存明文密码。
- 推荐使用 `argon2` 或 `bcrypt` 保存 `share_server_password_hash`。
- cookie 应设置 `HttpOnly`，局域网 HTTP 场景下至少设置 `SameSite=Lax`。
- 如果后续支持 HTTPS，再启用 `Secure` cookie。
- 浏览器 session 应有过期时间，撤回授权或关闭服务器时立即失效。

浏览器与客户端同步的认证方式应分开：

- 浏览器：使用 cookie session，面向普通浏览和下载。
- 客户端：使用 `Authorization: Bearer <token>`，面向同步、diff、按需下载。
- 两者都应最终经过统一授权中间件，但 session/token 的提取方式不同。

手动确认授权模式下，浏览器密码通过后不应立即访问分享内容，而是创建 pending 访问请求：

1. 浏览器提交正确密码。
2. 服务端写入 `InboundConnections.auth_status=0`。
3. Tauri UI 收到 `share://inbound-requested`。
4. 用户确认后，服务端将该浏览器 session 标记为 authorized。
5. 用户拒绝或撤回后，该 session 访问分享 API 返回 `401/403`。

## 7. 客户端按需同步

客户端同步目标是“远端分享先展示，真正访问时才下载”。

### 7.1 远端分享发现

客户端连接远端后调用：

```text
GET /api/client/shares
```

返回远端 `LocalFiles` 摘要：

```json
{
  "id": "remote-local-file-id",
  "name": "Photos",
  "type": 1,
  "size": null,
  "updated_at": 1770000000
}
```

本机将其 upsert 到 `SharedFiles`：

- `user_id=远端用户 ID`
- `id=远端 LocalFiles.id`
- `cache_status=0`
- `sync_policy=0`

### 7.2 按需下载触发

触发下载的场景：

- 用户在本机 UI 中打开远端文件。
- 用户主动点击“同步到本地”。
- 客户端需要预览远端图片或读取文件内容。

不触发下载的场景：

- 只是浏览远端分享列表。
- 只是展开远端文件夹列表。
- 浏览器访问远端分享内容。

### 7.3 同步子表

同步系统应建立在 `LocalFiles` 和 `SharedFiles` 上。

建议替换当前过于独立的 `sync_roots` 设计，新增两个方向明确的子表。这里的核心原则是：`LocalFiles/SharedFiles` 只保存分享根，目录树展开后的每个子文件、子文件夹都进入索引子表。

```sql
CREATE TABLE IF NOT EXISTS local_file_index (
  local_file_id TEXT NOT NULL,
  relative_path TEXT NOT NULL,
  absolute_path TEXT NOT NULL,
  size INTEGER NOT NULL,
  mtime INTEGER NOT NULL,
  is_dir INTEGER NOT NULL DEFAULT 0,
  hash TEXT,
  dirty INTEGER NOT NULL DEFAULT 1,
  exists_flag INTEGER NOT NULL DEFAULT 1,
  last_seen_at INTEGER NOT NULL,
  last_hashed_at INTEGER,
  PRIMARY KEY (local_file_id, relative_path)
);

CREATE TABLE IF NOT EXISTS shared_file_index (
  user_id TEXT NOT NULL,
  shared_file_id TEXT NOT NULL,
  relative_path TEXT NOT NULL,
  name TEXT NOT NULL,
  is_dir INTEGER NOT NULL DEFAULT 0,
  local_cache_path TEXT,
  size INTEGER,
  mtime INTEGER,
  hash TEXT,
  remote_deleted INTEGER NOT NULL DEFAULT 0,
  cache_status INTEGER NOT NULL DEFAULT 0,
  last_accessed_at INTEGER,
  updated_at INTEGER,
  PRIMARY KEY (user_id, shared_file_id, relative_path)
);
```

`local_file_index` 对应本机 `LocalFiles` 的目录树索引。

`shared_file_index` 对应远端分享内容在本机的缓存状态。

当远端分享是文件夹时，本地需要记录它的子文件和子文件夹索引，但不需要立即下载文件内容：

- 展开远端目录时，客户端拉取远端 metadata，upsert 到 `shared_file_index`。
- `relative_path` 表示相对远端分享根的路径，避免保存或依赖远端绝对路径。
- `is_dir=1` 表示目录节点，目录节点不需要 `local_cache_path`。
- `cache_status` 表示本地是否已有内容缓存，而不是远端文件是否存在。
- `remote_deleted=1` 表示远端已删除，本地可在 UI 中隐藏、标记失效或提示缓存已脱离远端。
- 远端文件修改时，根据 `mtime/hash/size/updated_at` 将已缓存文件标记为 stale，等待用户再次打开或点击同步时重新下载。

建议 `cache_status` 使用：

```text
0 = NotCached
1 = Caching
2 = Cached
3 = Failed
4 = Stale
5 = RemoteDeleted
```

远端变化检测优先使用轮询式增量刷新：

```text
GET /api/client/shares/:id/index?path=relative/path&since=timestamp
```

后续可以增加 WebSocket 或 SSE，让远端服务器在 `LocalFiles` 对应索引变化时通知已授权客户端刷新 `shared_file_index`。

当前 `sync_file_index/sync_roots/sync_hash_queue` 可以作为过渡表，但最终应迁移到 `local_file_index` 这种以 `local_file_id` 为根的结构。

### 7.4 同步 API

建议接口：

```text
GET  /api/client/shares
GET  /api/client/shares/:id/index?path=relative/path
GET  /api/client/shares/:id/download?path=relative/path
POST /api/client/shares/:id/diff
```

规则：

- `index` 只返回 DB 中的 metadata，不触发重 IO。
- `download` 是真正下载点，触发 `file_downloaded/sync_requested` 日志。
- `diff` 用于用户选择“同步到本地”时计算缺失或变化文件。
- hash 仍由后台 worker 异步计算，HTTP 接口不直接算 hash。

## 8. 后台索引系统

`file-sync-architect.md` 中的性能规则继续保留，但作用范围收窄为 `LocalFiles` 的子系统：

- 启动时只扫描 `LocalFiles.is_valid=1` 的分享入口。
- 只读取 `size + mtime + is_dir`，不全量 hash。
- 文件变化由 `notify` watcher 标脏。
- hash worker 只处理 dirty 文件。
- 下载使用流式输出，不读入整文件。

重构重点：

- watcher 监听 `LocalFiles.path`，而不是 `sync_roots.root_path`。
- scanner upsert 到 `local_file_index`，而不是以绝对路径作为全局主键。
- 一个绝对路径可能同时属于多个 `LocalFiles`，应按 `local_file_id + relative_path` 建索引。
- 文件夹内部变化更新索引，不取消文件夹分享。
- 文件根路径被删除时，将对应 `LocalFiles.is_valid=0`。

## 9. UI 与数据同步

本项目需要明确设计 UI 与数据的同步方式。分享系统中很多变化不是 UI 主动触发的，例如剪贴板变化、文件 watcher 事件、hash worker 更新、远端授权请求、远端分享刷新、同步下载进度等。如果只依赖前端主动查询，UI 会出现状态滞后；如果把所有数据都通过事件直接推给前端，又会在大目录和高频任务下造成 WebView 压力。

Tauri v2 中建议采用以下组合：

- SQLite/SeaORM：领域数据的唯一事实来源。
- Tauri Commands：前端主动查询快照和执行业务操作。
- Tauri Events：Rust 后端通知 UI 某类数据已经变化。
- Tauri Channels：同步、下载、扫描、hash 等高频进度流。
- Tauri State：后端运行时状态，例如 server controller、session manager、sync runtime、event broadcaster。
- Store plugin：只保存轻量 UI 偏好或配置，不保存 `LocalFiles/SharedFiles/shared_file_index` 这类关系型业务数据。

### 9.1 数据所有权

数据所有权建议固定为：

```text
SQLite/SeaORM = 真实数据源
Rust Commands = 查询快照 + 执行业务操作
Rust Events = 通知 UI 哪类数据失效
Rust Channels = 同步/下载/扫描进度流
React State = 当前页面临时展示状态
```

前端不应把 `LocalFiles`、`SharedFiles`、`shared_file_index` 当成真实来源。前端只保存当前页面所需的 view state，例如筛选条件、选中项、展开状态、加载状态。

### 9.2 Commands：查询和业务操作

Commands 用于前端主动拉取数据或提交用户操作，例如：

```text
list_local_shared_files
add_manual_shared_paths
unshare_local_shared_file
list_remote_share_users
refresh_remote_shares
list_shared_file_index
start_sync_to_local
approve_inbound_connection
deny_inbound_connection
revoke_inbound_connection
set_share_server_password
set_share_auth_mode
```

原则：

- command 返回当前查询结果或操作结果。
- command 内部完成数据库写入和业务校验。
- command 写入数据库后，由后端统一发出对应事件。
- 大列表必须分页或按目录分层查询，不一次性返回完整远端文件树。

### 9.3 Events：数据失效通知

Events 用于通知前端“某类数据发生了变化”，而不是承载完整数据。

建议事件命名：

```text
share://local-files-changed
share://shared-files-changed
share://shared-file-index-changed
share://inbound-requested
share://connection-status-changed
clipboard://changed
sync://task-updated
server://status-changed
```

建议 payload：

```json
{
  "entity": "local_files",
  "ids": ["..."],
  "reason": "clipboard_changed",
  "version": 1770000000
}
```

前端收到事件后，根据当前页面决定是否重新调用 command 拉取最新数据：

- 当前在本地分享页：收到 `share://local-files-changed` 后刷新本页列表。
- 当前在远端分享页：收到 `share://shared-files-changed` 后刷新远端入口。
- 当前展开远端文件夹：收到 `share://shared-file-index-changed` 后刷新当前目录。
- 收到 `share://inbound-requested`：显示授权确认弹窗或通知。
- 收到 `server://status-changed`：刷新服务器开关和地址状态。

这样可以避免把大目录、大索引或频繁变化的进度数据直接塞进事件。

### 9.4 Channels：高频进度流

下载、同步、扫描、hash 这类任务会产生高频进度，不适合普通事件广播。Tauri v2 应使用 Channel 把进度流直接传给发起任务的前端调用方。

典型 command：

```text
start_sync_task(payload, on_event: Channel<SyncTaskEvent>)
download_remote_file(payload, on_event: Channel<DownloadEvent>)
scan_local_share(payload, on_event: Channel<ScanEvent>)
```

事件类型示例：

```text
Started
Progress
FileCompleted
Skipped
Failed
Finished
Cancelled
```

原则：

- Channel 用于任务实例级别的进度。
- 普通 Event 用于全局数据变化通知。
- 任务完成并写入数据库后，仍然要发普通 Event，通知其他页面刷新。

### 9.5 后端事件发布层

建议新增统一事件发布模块，避免业务代码到处直接调用 `app.emit`：

```text
src-tauri/src/app/events.rs
```

职责：

- 定义事件名常量。
- 定义事件 payload 结构。
- 提供 `emit_local_files_changed`、`emit_shared_files_changed`、`emit_inbound_requested` 等函数。
- 对高频事件做合并或节流。

示例职责划分：

```text
clipboard_storage 保存新剪贴板记录
  -> 写入 clipboard_record/local_files
  -> emit clipboard://changed
  -> emit share://local-files-changed

watcher 发现本地文件变化
  -> 更新 local_file_index
  -> emit share://local-files-changed 或 share://local-file-index-changed

远端授权请求进入 pending
  -> 写入 inbound_connections
  -> emit share://inbound-requested

同步任务完成
  -> 更新 shared_files/shared_file_index
  -> Channel 发送 Finished
  -> emit share://shared-file-index-changed
```

### 9.6 前端状态管理建议

当前前端是 React，可以先不引入复杂全局状态库，优先使用：

- 页面级 `useState/useReducer` 保存 view state。
- 自定义 hooks 封装 `invoke + listen`。
- 进入页面时先 `invoke` 拉快照。
- 页面挂载时订阅相关事件，卸载时取消监听。
- 收到事件后做防抖刷新，避免短时间大量 watcher 事件造成重复查询。

示例 hook 形态：

```text
useLocalSharedFiles()
useRemoteShareUsers()
useInboundRequests()
useSharedFileIndex(userId, sharedFileId, relativePath)
useShareServerStatus()
```

每个 hook 的职责：

- 首次加载调用 command。
- 监听相关事件。
- 事件触发后刷新当前数据。
- 暴露 loading/error/reload/actions。

### 9.7 同步一致性策略

建议使用“版本号 + 失效刷新”的方式保持 UI 一致：

- 每次后端写入关键表时生成 `version`，可用时间戳或递增序号。
- Event payload 携带 `version`。
- 前端只接受比当前页面缓存更新的版本。
- 对大目录索引使用分页、当前目录刷新或 `since` 增量刷新。
- 如果前端错过事件，页面重新进入时仍通过 command 拉取数据库快照，不依赖事件恢复状态。

这套模式能保证：数据库是最终一致的事实来源，UI 通过事件快速感知变化，通过 command 修正状态。

## 10. 模块重构建议

建议将 `src-tauri/src/server` 调整为：

```text
src-tauri/src/server/
  mod.rs
  service.rs
  state.rs
  auth/
    mod.rs
    password.rs
    session.rs
    guard.rs
  routes/
    mod.rs
    health.rs
    browser.rs
    client.rs
    auth.rs
  share/
    mod.rs
    local_share.rs
    browser_view.rs
    path_guard.rs
  sync/
    mod.rs
    scanner.rs
    watcher.rs
    hash_worker.rs
    diff.rs
    stream.rs
  repo/
    mod.rs
    local_files_repo.rs
    shared_files_repo.rs
    connection_repo.rs
    index_repo.rs
```

职责：

- `auth`：密码、授权、session token、请求中间件。
- `routes/browser.rs`：浏览器页面和浏览器 API。
- `routes/client.rs`：客户端发现、索引、按需下载、diff。
- `share/local_share.rs`：以 `LocalFiles` 为中心的分享入口 CRUD。
- `share/path_guard.rs`：相对路径解析和越权防护。
- `sync/*`：只负责索引、hash、watcher、diff，不负责授权决策。
- `repo/*`：隔离 SeaORM 查询，减少 route 中直接拼业务查询。

## 11. 与现有代码的具体调整

### 11.1 `src-tauri/src/server/sync.rs`

当前逻辑：

- `refresh_sync_roots_from_clipboard`
- `scan_roots_once`
- `run_watcher_loop`
- `run_hash_worker`

调整为：

- 删除或降级 `sync_roots` 作为过渡兼容。
- 新增 `refresh_local_indexes_from_local_files`。
- scanner 从 `local_files where is_valid=1` 加载分享入口。
- watcher 监听 `LocalFiles.path`。
- hash queue 使用 `local_file_id + relative_path`。
- 文件变化时只更新索引和有效性，不直接从 `clipboard_record` 反推 root。

### 11.2 `src-tauri/src/server/routes/clipboard.rs`

当前 `/clipboard/list` 可以保留，但它不是文件分享主入口。

调整：

- 剪贴板内容分享只服务于剪贴板历史浏览。
- 文件浏览下载改走 `LocalFiles` API。
- `load_shared_roots` 不应再从 `sync_roots` 读取，改为读取 `local_files`。

### 11.3 `src-tauri/src/server/routes/sync.rs`

当前 `/index`、`/diff`、`/download` 是同步接口雏形。

调整：

- 改为 `/api/client/shares/:id/index`。
- 参数使用 `share_id + relative_path`，不再接收本机绝对路径。
- 下载前校验授权和路径归属。
- 远端访问 `download` 时记录 `ConnectionLog`。

### 11.4 `src-tauri/src/db/service/clipboard.rs`

当前 `toggle_share` 会根据 `clipboard_record.is_shared` 写入 `LocalFiles`，这是可以保留的。

需要新增：

- 自动分享配置判断。
- 剪贴板变化时取消旧 clipboard 临时分享。
- `upsert_local_files_for_shared_clipboard` 应明确写入 `share_mode=1`。
- 手动分享写入 `share_mode=0`。

### 11.5 `src-tauri/src/app/commands/share_files.rs`

当前已有：

- `list_remote_share_users`
- `list_local_shared_files`
- `add_manual_shared_paths`
- `unshare_local_shared_file`

需要补充：

- 分享服务器密码设置。
- 授权模式设置。
- 待确认访问请求列表。
- 同意/拒绝/撤回 inbound 连接。
- 远端分享列表刷新。
- 点击“同步到本地”触发 `SharedFiles` 下载任务。

## 12. 数据迁移方案

新增 migration 建议命名：

```text
m20260513_000003_share_refactor.rs
```

迁移内容：

1. 给 `local_files` 添加 `share_mode/expires_at/updated_at`。
2. 给 `shared_files` 添加远端摘要和按需同步字段。
3. 给 `outbound_connections/inbound_connections` 添加授权状态字段。
4. 可选新增 `local_file_index/shared_file_index/share_sessions/share_server_settings`。
5. 保留现有 `sync_file_index/sync_roots/sync_hash_queue`，先不删除，避免破坏未提交代码。

过渡策略：

- 第一阶段让新 API 直接读取 `LocalFiles`。
- 第二阶段将 `sync_file_index` 数据迁移或重建到 `local_file_index`。
- 第三阶段确认无依赖后再删除 `sync_roots`。

## 13. 分阶段落地计划

### Phase 1：拉回分享主模型

- 新增 migration 和实体字段。
- 让服务器分享列表从 `LocalFiles.is_valid=1` 读取。
- 下载和浏览接口改成 `LocalFiles.id + relative_path`。
- 保留旧接口兼容，但内部转到新服务。

### Phase 2：授权链路

- 增加密码配置和 session。
- 实现 `AutoAllow/RequireConfirm`。
- 实现 inbound 授权、拒绝、撤回。
- 所有浏览/下载/同步接口接入授权中间件。

### Phase 3：剪贴板自动挂载

- 增加自动分享配置。
- 文件/图片/文件夹复制后自动写入 `LocalFiles`。
- 一次复制多个文件时分开挂载。
- 剪贴板变化时取消旧临时分享。
- 手动分享不受剪贴板变化影响。

### Phase 4：浏览器访问

- 实现浏览器首页和浏览 API。
- 浏览器首页只读取 `LocalFiles`，不直接暴露完整 `clipboard_record` 历史。
- 剪贴板来源的文件/图片/文件夹只有挂载到 `LocalFiles` 后才允许浏览器访问。
- 增加 `/login`、`/api/browser/auth/login`、`/api/browser/auth/logout`、`/api/browser/auth/status`。
- 启用密码时使用 `HttpOnly` cookie session；禁止 URL token。
- 密码只保存派生哈希，推荐 `argon2` 或 `bcrypt`。
- `browser_access_enabled=false` 时所有浏览器页面和浏览器 API 返回 `403`。
- `RequireConfirm` 模式下，浏览器密码正确后创建 pending inbound 请求，主机确认后才可访问。
- 浏览器 session 过期、关闭服务器、撤回授权后应立即失效。
- 支持目录浏览、文件下载、Range。
- 浏览器访问只下载，不写入 `SharedFiles`。

### Phase 5：客户端按需同步

- 实现远端 shares 发现。
- 将远端入口 upsert 到 `SharedFiles`。
- 浏览远端目录只拉 metadata。
- 打开文件或点击同步时才下载。
- 建立 `shared_file_index` 记录缓存状态。

### Phase 6：同步索引收口

- 将 `sync_file_index` 收敛为 `local_file_index`。
- scanner/watcher/hash worker 全部基于 `LocalFiles`。
- 完成 diff、hash 队列和缓存校验。

## 14. 验收标准

- 开启服务器后，未授权设备不能访问分享列表。
- 自动授权模式下，正确密码可直接访问。
- 手动确认模式下，远端请求会进入 pending，用户同意后才能访问。
- 撤回授权后，远端 token 失效，下载和同步接口返回未授权。
- 复制多个文件时，`LocalFiles` 中生成多条记录，并记录同一个剪贴板 ID。
- 剪贴板变动后，上一条剪贴板临时分享自动失效。
- 手动添加的分享不会因剪贴板变动失效。
- 浏览器能浏览和下载，但不会创建 `SharedFiles` 缓存记录。
- 客户端刷新远端分享列表只写入 `SharedFiles` 摘要，不下载内容。
- 用户打开远端文件或点击同步到本地时才开始下载。
- 大文件下载保持流式输出，内存不随文件大小线性增长。
- 启动扫描不计算全量 hash，hash 由后台 worker 异步补齐。
