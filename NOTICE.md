# 声明与第三方说明

Creeper Core 是一个独立的 Minecraft 启动器，不是 Mojang Studios、Microsoft、PCL2 或其作者的官方产品，也没有获得它们的背书。

## Minecraft

- “Minecraft”、游戏内名称、角色形象、纹理、音效和相关商标归 Mojang Studios / Microsoft 所有。
- 本仓库不包含 Minecraft 客户端 JAR、资源对象、账户令牌或其他游戏本体文件。
- 启动器只从 Mojang 官方公开分发端点（或用户自行选择的镜像）下载游戏文件；使用游戏需遵守 Minecraft EULA。
- `docs/screenshots` 中的游戏画面、角色与名称仅用于展示启动器功能，不随 MIT 许可授权。

## 图标

苦力怕图标是根据苦力怕的公开形象，用 16×16 像素网格重新绘制的界面装饰，不使用 Mojang 原始纹理文件。商业使用或再分发前，请自行确认商标与素材使用边界。

## 第三方组件

运行时和构建依赖的许可证以 `Cargo.lock`、`pnpm-lock.yaml` 为准。主要组件：

| 组件 | 用途 | 许可证 |
| --- | --- | --- |
| Tauri | 桌面窗口 / WebView2 / IPC | Apache-2.0 / MIT |
| React / React DOM | UI | MIT |
| TypeScript | 类型系统 | Apache-2.0 |
| Vite | 前端构建 | MIT |
| Zustand | UI 状态 | MIT |
| Lucide | 线性图标 | ISC |
| skinview3d / Three.js | 3D 皮肤预览 | MIT |
| Tokio / reqwest / serde / zip / chrono / uuid / toml 等 | Rust 引擎 | 以各 crate 声明为准 |
| MCIM 翻译接口 | 可选的 Modrinth 简介翻译 | 外部服务，不随包分发 |

完整说明见 [docs/THIRD_PARTY.md](docs/THIRD_PARTY.md)；该文件与本节冲突时，以第三方项目自身的许可证为准。

## 截图中的个人数据

公开截图展示的是开发测试账户和测试环境，不包含 access token、refresh token、密码、私钥或完整配置。若你提交截图，请先移除账户 UUID、路径、日志和令牌等个人信息。
