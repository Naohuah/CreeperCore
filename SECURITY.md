# 安全策略

## 报告问题

如果你发现安全问题，请不要直接公开包含利用细节的 Issue。请通过仓库维护者公开资料中的联系方式私下报告，或先提交一个不含敏感细节的 Issue，说明“需要私下联系”。

请包含：

- 受影响版本 / 提交
- 复现步骤
- 影响范围
- 你希望如何署名

## 不要提交

- `config.json` 或任何 access / refresh token
- `.minecraft` 目录、存档、皮肤缓存
- `CreeperCore/` 数据目录、`EBWebView/`
- `panic.log`、完整游戏日志、包含个人路径的截图
- Microsoft 账户密码、验证码或恢复码

## 依赖与构建

- Rust 依赖以 `Cargo.lock` 为准，前端依赖以 `pnpm-lock.yaml` 为准。
- 不提交构建产物、`node_modules`、`target`、`dist` 或 `release`。
- 发布前请运行 `cargo test -p creeper-core`、`pnpm build`，并检查依赖更新。

## 支持范围

当前只支持 Windows 10/11 便携版。未签名的 exe 可能被 SmartScreen 拦截；请只从本仓库 Releases 下载，并核对 SHA-256。
