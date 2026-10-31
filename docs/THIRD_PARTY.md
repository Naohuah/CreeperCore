# 第三方组件与资产

Creeper Core 使用以下开源组件。实际依赖版本以 `Cargo.lock` 与 `pnpm-lock.yaml` 为准。

## 运行时 / 框架

| 组件 | 用途 | 许可证 |
| --- | --- | --- |
| [Tauri](https://tauri.app/) | 桌面窗口、WebView2 容器、IPC | Apache-2.0 / MIT |
| [Rust](https://www.rust-lang.org/) | 引擎语言 | Apache-2.0 / MIT |
| [React](https://react.dev/) | UI 运行时 | MIT |
| [TypeScript](https://www.typescriptlang.org/) | 类型系统 | Apache-2.0 |
| [Vite](https://vite.dev/) | 前端开发与构建 | MIT |
| [Zustand](https://zustand-demo.pmnd.rs/) | UI 状态 | MIT |
| [Lucide](https://lucide.dev/) | 线性图标 | ISC |
| [skinview3d](https://github.com/bs-community/skinview3d) | 3D 皮肤预览 | MIT |
| [Three.js](https://threejs.org/) | WebGL 渲染 | MIT |
| [MCIM 翻译接口](https://mod.mcimirror.top/) | Modrinth 项目简介中文翻译；运行时可选，失败回退英文 | 外部服务，不随安装包分发 |

## 主要 Rust crates

- `tokio` — 异步运行时（MIT）
- `reqwest` + `rustls` — HTTP（Apache-2.0 / MIT）
- `serde` / `serde_json` — 序列化（Apache-2.0 / MIT）
- `sha1`、`md-5` — 校验与离线 UUID（Apache-2.0 / MIT）
- `zip` — 快照与 JAR 元数据（MIT）
- `chrono` — 时间（Apache-2.0 / MIT）
- `walkdir` — 目录遍历（Unlicense / MIT）
- `sysinfo` — 系统内存信息（MIT）
- `uuid` — 本地档案 ID（Apache-2.0 / MIT）
- `toml` — Forge 元数据（Apache-2.0 / MIT）
- `futures-util` — 并发流（Apache-2.0 / MIT）
- `anyhow` — 错误上下文（Apache-2.0 / MIT）
- `regex` — Java 探测与日志分析（Apache-2.0 / MIT）

## 游戏相关

- Minecraft 名称、苦力怕形象及游戏资源归 **Mojang Studios / Microsoft** 所有。
- 本项目不包含任何 Minecraft 客户端 JAR、资源对象或账户令牌。
- 启动器从 Mojang 官方公开分发端点下载游戏文件；用户需自行遵守 Minecraft EULA。
- 苦力怕像素图由本项目根据苦力怕的公开形象以 16×16 SVG 像素网格重新绘制，用于界面装饰。若用于商业发行，请自行确认相关商标与素材使用边界。
- BMCLAPI 是社区镜像服务，不属于本项目；使用其下载源时请遵守其服务条款。
- Fabric 名称与加载器归 FabricMC 项目所有。

## 许可证文本

MIT 许可证正文见项目根目录 [LICENSE](../LICENSE)。
