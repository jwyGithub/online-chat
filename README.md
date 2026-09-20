# QuickChat

> 极简托盘 AI 对话工具 —— 一个快捷键随时唤出，用完即隐的桌面 AI 对话框。

QuickChat 是一款基于 [Tauri 2](https://tauri.app/) + Vue 3 构建的轻量级桌面应用。它常驻系统托盘，不占用 Dock / 任务栏，任何时候按下全局快捷键即可在屏幕中央弹出一个无边框、置顶的对话窗口，与 OpenAI 兼容的大模型进行流式对话，用完按同一快捷键即可隐藏。

## 功能特性

- **全局快捷键唤出/隐藏**：默认 `CmdOrCtrl+Shift+Space`，在任意应用中按下都能切换窗口显隐；快捷键可在设置中自定义并即时生效。
- **托盘常驻**：系统托盘图标 + 右键菜单（显示/隐藏、设置、退出）。macOS 下使用模板图标，随亮/暗菜单栏自动反色，并设为 Accessory 模式（不占 Dock）。
- **无边框沉浸窗口**：透明、圆角阴影、置顶、居中、不显示在任务栏，失焦可自动隐藏（可关闭）。
- **流式对话**：请求在 Rust 端发起（无 CORS 问题，API Key 不进入前端），基于 async-openai 解析 SSE，逐 token 推送到前端实时渲染，支持随时「停止生成」。
- **Markdown 渲染**：内置 Markdown / 流式 Markdown 渲染与 XSS 净化。
- **兼容任意 OpenAI 协议服务**：可自定义 `base_url`、`model`、`temperature`、`system_prompt`，接入 OpenAI 官方或任意兼容端点。
- **本地存储、隐私优先**：配置与 API Key 分别保存在应用本地配置目录，Key 单独存放且不向前端暴露明文。

## 技术栈

| 层 | 技术 |
| --- | --- |
| 前端 | Vue 3、TypeScript、Vite 8、Tailwind CSS v4、GSAP、marked + DOMPurify、markstream-vue |
| 后端（原生） | Rust、Tauri 2、tauri-plugin-global-shortcut、tauri-plugin-opener、async-openai、tokio |

## 环境要求

- [Node.js](https://nodejs.org/) ≥ 20（推荐 22）
- [pnpm](https://pnpm.io/) 10（本项目使用 lockfile v9）
- [Rust](https://www.rust-lang.org/) stable 工具链
- 各平台 Tauri 系统依赖，详见 [Tauri 前置条件](https://tauri.app/start/prerequisites/)
  - Linux 需额外安装 `libwebkit2gtk-4.1-dev`、`libayatana-appindicator3-dev` 等（托盘图标依赖）

## 开始使用

```bash
# 安装依赖
pnpm install

# 启动桌面开发环境（自动拉起 Vite 并编译 Rust）
pnpm tauri dev
```

首次运行后，通过托盘菜单或快捷键打开窗口，进入「设置」填写 API Key（以及可选的 base_url / 模型等），即可开始对话。

### 仅调试前端

```bash
pnpm dev        # 启动 Vite 开发服务器 (http://localhost:1420)
pnpm build      # 类型检查 + 构建前端产物到 dist/
```

## 打包构建

```bash
# 构建当前平台的安装包（产物在 src-tauri/target/release/bundle/）
pnpm tauri build
```

本仓库已配置多平台自动打包的 GitHub Actions（`.github/workflows/release.yml`）：推送形如 `v0.1.0` 的 tag（或在 Actions 页手动触发）即会为 macOS（Apple Silicon / Intel）、Linux、Windows 分别构建并汇总为一个 **草稿 Release**。

```bash
git tag v0.1.0
git push origin v0.1.0
```

> 提示：默认产物未做代码签名，macOS / Windows 首次打开可能出现安全提示，个人使用可忽略。

## 配置说明

设置项保存在应用配置目录的 `config.json`，API Key 单独保存在同目录的 `api_key` 文件中（与配置分离，避免被整体覆盖，且不向前端暴露明文）。

| 配置项 | 说明 | 默认值 |
| --- | --- | --- |
| `base_url` | OpenAI 兼容 API 地址 | `https://api.openai.com/v1` |
| `model` | 模型名称 | `gpt-4o-mini` |
| `temperature` | 采样温度 | `0.7` |
| `system_prompt` | 系统提示词 | 空 |
| `hotkey` | 全局快捷键 | `CmdOrCtrl+Shift+Space` |
| `hide_on_blur` | 窗口失焦时自动隐藏 | `true` |

配置目录位置随操作系统而定（Tauri `app_config_dir`），例如：

- macOS：`~/Library/Application Support/com.jiangwy.onlinechat/`
- Windows：`%APPDATA%\com.jiangwy.onlinechat\`
- Linux：`~/.config/com.jiangwy.onlinechat/`

## 项目结构

```
online-chat/
├── src/                      # Vue 前端
│   ├── views/                # ChatView（对话）、SettingsView（设置）
│   ├── composables/          # useChat / useConfig / useMotion
│   ├── components/           # Icon 等通用组件
│   └── App.vue
├── src-tauri/                # Rust 后端
│   ├── src/
│   │   ├── lib.rs            # 应用入口：窗口显隐、全局快捷键、托盘
│   │   ├── chat.rs           # OpenAI 兼容流式对话
│   │   ├── config.rs         # 配置读写
│   │   └── secret.rs         # API Key 本地存取
│   ├── capabilities/         # 权限声明
│   └── tauri.conf.json       # Tauri 配置
├── .github/workflows/        # 多平台打包工作流
└── package.json
```

## 安全说明

- 所有对话请求均由 Rust 后端发起，API Key 不会进入前端 WebView，也不会随请求暴露在浏览器网络层。
- API Key 以明文保存在本机应用配置目录下的独立文件中（非系统钥匙串），请自行确保设备安全。

## License

[MIT](./LICENSE) © 2026 jawyn
