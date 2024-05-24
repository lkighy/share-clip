# Tauri + Vue 3

This template should help get you started developing with Tauri + Vue 3 in Vite. The template uses Vue 3 `<script setup>` SFCs, check out the [script setup docs](https://v3.vuejs.org/api/sfc-script-setup.html#sfc-script-setup) to learn more.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Volar](https://marketplace.visualstudio.com/items?itemName=Vue.volar) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## 项目目录结构
my-tauri-clipboard-app/
├── src/
│   ├── assets/                   # 静态资源（图标、图片等）
│   ├── components/               # Vue 组件
│   ├── views/                    # Vue 页面视图
│   ├── store/                    # Vuex 状态管理（如果使用 Vuex）
│   ├── router/                   # Vue Router 配置
│   ├── utils/                    # 实用工具函数
│   ├── App.vue                   # Vue 主应用组件
│   ├── main.js                   # Vue 入口文件
│   ├── setting.js                # 设置页面入口
│   ├── clipboard.js              # 与系统剪切板交互
│   └── http-server.js            # HTTP 服务器配置和处理
├── src-tauri/
│   ├── src/
│   │   ├── commands.rs            # 定义与前端交互的命令
│   │   ├── clipboard.rs           # 系统剪切板交互逻辑
│   │   ├── database.rs            # 数据库（SQLite）交互逻辑
│   │   ├── entities.rs            # 数据库模型
│   │   ├── http_server.rs         # HTTP 服务器逻辑
│   │   ├── shortcuts.rs           # 系统快捷键处理逻辑
│   │   └── main.rs                # Tauri 主入口文件
│   ├── tauri.conf.json            # Tauri 配置文件
├── public/                       # 公共文件（index.html 等）
├── package.json                  # 项目配置文件
├── Cargo.toml                    # Rust 项目配置文件
└── README.md                     # 项目说明文件
