# 参与开发

感谢你愿意参与 Creeper Core。

## 环境

- Windows 10 1809+ / Windows 11
- Node.js 20+、pnpm 10+
- Rust 1.90+（MSVC toolchain）
- Visual Studio 2022/2026 Build Tools（C++）
- WebView2 Runtime

```powershell
pnpm install
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev.ps1
```

## 结构

```text
core/        Rust 引擎：版本、下载、启动、模组、备份、MSA
src-tauri/   Tauri 命令、窗口、系统集成
src/         React 界面、bridge、mock 预览
docs/        架构、设计、路线图、第三方说明
```

## 提交前

```powershell
cargo test -p creeper-core
pnpm build
```

- UI 改动请附截图或录屏，说明改动前后的行为。
- 不要在 UI 上放未实现的功能或假进度。
- 网络失败、取消、空状态和错误恢复要实际验证。
- 不提交 `.minecraft/`、`CreeperCore/`、`target/`、`dist/`、`node_modules/`、日志、令牌或个人路径。

## 提交信息

使用简短、以动词开头的说明，例如：

```text
fix: 等待游戏窗口后再显示启动完成
feat: 增加实例级 Java 覆盖
docs: 补充便携模式说明
```

## 许可证

提交代码即表示你同意以 MIT License 发布你的贡献。不要提交你无权授权的代码、素材或 Minecraft 游戏文件。
