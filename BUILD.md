# 构建与开发

本文档记录分享剪切板的本地开发、编译、发布和数据库迁移说明。README 只保留面向使用者的应用介绍，构建类内容统一放在这里。

## 环境要求

- Node.js LTS
- pnpm
- Rust stable
- Tauri 2 所需的系统依赖

Linux 和 macOS 的系统依赖会随 Tauri 版本变化，安装前建议参考 Tauri 官方 prerequisites：

https://v2.tauri.app/start/prerequisites/

## 安装依赖

```shell
pnpm install
```

## 开发运行

```shell
pnpm tauri dev
```

## 前端构建

```shell
pnpm build
```

## Rust 检查

```shell
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo check --manifest-path src-tauri/Cargo.toml
```

## 桌面应用打包

```shell
pnpm tauri build
```

生成的安装包位于 `src-tauri/target/release/bundle/` 下，具体子目录取决于目标平台和打包格式。

## Linux 和 macOS

Windows 本机不能可靠地直接验证 Linux/macOS 桌面包。建议：

- Linux：使用 WSL2、Linux VM、容器或 GitHub Actions。
- macOS：使用真实 Mac 或 GitHub Actions 的 macOS runner。

发布前至少应在目标平台执行：

```shell
pnpm install --frozen-lockfile
pnpm build
cargo check --manifest-path src-tauri/Cargo.toml
pnpm tauri build
```

## GitHub Release

推荐使用 `tauri-apps/tauri-action` 在 GitHub Actions 中构建并上传安装包。

https://github.com/tauri-apps/tauri-action

建议发布流程：

```shell
git tag v0.1.0
git push origin v0.1.0
```

由 tag 触发 release workflow，在 Windows、Linux 和 macOS 上分别构建安装包。

## `src-tauri/src` 目录说明

- `app/`：应用层逻辑，包含业务命令、配置加载、快捷键行为、窗口与托盘 UI 相关代码。
- `platform/`：平台能力封装，放置与操作系统相关的实现，如非激活窗口、系统信息与光标位置获取等。
- `server/`：共享服务器、文件访问和远程连接相关逻辑。
- `services/`：剪切板监听、剪切板存储等后台服务。
- `db/`：数据库连接、仓储和服务层。
- `entity/`：SeaORM 实体。
- `lib.rs`：Tauri 应用主入口，负责组装插件、注册命令、初始化配置与启动流程。
- `main.rs`：可执行入口，调用 `share_clip_lib::run()` 启动应用。

## SeaORM 迁移与实体生成

在 `src-tauri` 目录下执行：

```shell
sea-orm-cli generate entity -u sqlite://share_clip.db?mode=rwc -o src/entity
```
