# Tauri + React + Typescript

This template should help get you started developing with Tauri, React and Typescript in Vite.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## `src-tauri/src` 目录说明

- `app/`: 应用层逻辑，包含业务命令、配置加载、快捷键行为、窗口与托盘 UI 相关代码。
- `platform/`: 平台能力封装，放置与操作系统相关的实现（如非激活窗口、系统信息与光标位置获取等）。
- `lib.rs`: Tauri 应用主入口，负责组装插件、注册命令、初始化配置与启动流程。
- `main.rs`: 可执行入口，调用 `share_clip_lib::run()` 启动应用。


## sea-orm 迁移方式
```shell
sea-orm-cli generate entity -u sqlite://share_clip.db?mode=rwc -o src/entity
```