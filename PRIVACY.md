# 隐私说明

Creeper Core 是一个本地优先的桌面启动器。默认情况下，启动器不会把账户、存档、日志或使用数据上传到本项目维护者。

## 启动器保存什么

在便携模式下，数据位于 `CreeperCore.exe` 旁边：

```text
CreeperCore/
  config.json        账户、设置、版本选择
  EBWebView/         Microsoft 登录窗口的缓存与 Cookie
  panic.log          仅在原生崩溃时生成
.minecraft/          游戏目录、版本、模组、存档、资源包
```

`config.json` 中可能包含：

- Microsoft 账户的 Minecraft access token 和 refresh token
- 账户名、UUID、账户类型
- 游戏目录、Java 路径、内存、JVM 参数、界面设置
- 版本级设置

**密码不会保存在启动器里。** 密码只输入给 Microsoft 官方登录页面。

## 网络请求

启动器会访问：

- Microsoft OAuth / Xbox Live / XSTS / Minecraft Services：登录、皮肤、披风
- Mojang 版本清单和资源 CDN，或用户选择的 BMCLAPI 镜像
- Fabric / Forge / NeoForge 的公开安装源
- Modrinth API 和 Modrinth CDN
- 可选的 MCIM 翻译接口
- 用户主动打开的外部链接

这些请求的必要信息（例如目标版本、项目 ID、令牌）会发送给对应服务。启动器没有自己的遥测服务。

## 日志与崩溃

- 游戏日志保存在本地游戏目录，用于崩溃识别和用户查看。
- `panic.log` 只记录 Rust panic 文本，保存在便携数据目录。
- 启动器不会自动上传日志或崩溃报告。
- 提交 Issue 前，请先删除日志中的账户名、路径、令牌和存档信息。

## 删除数据

关闭启动器后，删除 exe 旁边的 `CreeperCore` 和 `.minecraft` 即可。

如果你把游戏目录改到了别处，还需要删除设置里显示的那个游戏目录。

## Microsoft 登录

登录使用 Microsoft 官方 OAuth 授权码 + PKCE，并在内嵌 WebView 中完成。启动器不会看到或保存 Microsoft 密码。登录令牌的访问范围由 Microsoft 和目标服务决定。

本项目不是 Microsoft 或 Mojang 的官方产品。
