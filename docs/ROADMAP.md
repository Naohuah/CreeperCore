# 路线图

> 原则：只有真正实现并验证过的能力，才从 ⏳ 移到 ✅。

## M0 · 本地核心（当前，v0.1.0）

**已完成并实测**

- [x] Tauri 2 + Rust + React 工程
- [x] 原版版本扫描 / 继承解析 / 规则求值
- [x] Mojang 版本清单、并行下载、SHA-1、原子写入、TNT Cache
- [x] BMCLAPI 下载源；Fabric Meta/Maven 镜像兜底
- [x] Java 探测、版本兼容选择
- [x] 原版启动（真实 Minecraft 26.2 启动到主菜单）
- [x] Fabric 安装（真实安装 0.20.1 + 26.2）
- [x] 离线账号、离线 UUID、档案切换
- [x] 版本级游戏目录隔离
- [x] 模组元数据 / 依赖检查 / 启用禁用 / 永久删除 / 导入
- [x] 时间机器：快照、恢复、恢复前安全快照、启动前自动快照
- [x] Creeper Sense 诊断 + 文件修复
- [x] 进程守护、实时日志、进程树终止、崩溃签名
- [x] 苦力怕视觉系统、窗口、启动面板、安装 dock、键盘快捷键
- [x] Windows 便携版构建流程
- [x] Microsoft OAuth 内嵌授权码窗口、Xbox / XSTS / Minecraft Services 接入
- [x] 侧栏 3D 皮肤预览、本地皮肤导入、离线占位

**明确未完成**

- [x] Microsoft OAuth / 正版在线会话
- [ ] Microsoft 登录真实账号联调与 Minecraft API 权限复核
- [x] Forge / NeoForge 安装器（官方 installer，Java 17+）
- [ ] Quilt 安装器
- [ ] OptiFine 安装引导
- [x] Modrinth 整合包导入（Fabric）
- [ ] CurseForge / 本地 ZIP 整合包导入
- [ ] Java 运行时自动下载
- [ ] 启动器自动更新
- [ ] 服务器列表 / 状态 ping / 一键进服
- [x] Modrinth 模组远程搜索、依赖安装与 SHA-1 校验
- [x] Modrinth 资源包 / 光影 / 数据包下载
- [x] Modrinth Fabric 整合包下载、独立版本与 overrides 安装
- [x] Forge / NeoForge 加载器安装（官方 installer，Java 17+）
- [ ] CurseForge 模组 / 整合包（需要 API Key）
- [ ] OptiFine 下载/安装引导
- [ ] Forge / NeoForge 整合包支持
- [ ] 多语言

## M1 · 在线身份与加载器

优先级最高，因为它决定“能不能当日常启动器”。

- Microsoft OAuth loopback 回调 / 内嵌登录窗口
- Minecraft access token 链、Xbox XSTS、profile 刷新
- 安全的 token 存储（Windows Credential Manager）
- 正版皮肤 / 披风展示
- Microsoft 登录真实账号联调、Minecraft API 权限复核与凭证加密
- Quilt 安装器
- Forge / NeoForge 安装器稳定性：公共镜像、安装器进度、失败回滚；当前走官方 installer 的 `--installClient`
- 在线登录的多人服务器联机验证
- 失败时回退离线档案，且明确提示原因

## M2 · 内容生态

- Modrinth / CurseForge 搜索、版本约束、依赖自动安装
- 整合包安装（manifest + overrides + 模组下载）
- 模组更新检查（基于 mod id + version）
- 资源包 / 光影包管理器
- 服务器列表、MOTD ping、玩家数
- 世界存档扫描与导入
- 截图管理器

## M3 · 平台化

- 自动更新（签名产物）
- 崩溃报告可选匿名上传（默认关闭）
- 主题系统（不只是换色，支持不同像素主题）
- Linux / macOS 构建与验证
- 启动性能指标（引擎初始化、Java 发现、classpath 构建）
- 下载器 Range 续传与多任务队列
- 插件 / 扩展 API（只读诊断事件优先）

## 技术验收指标

“比肩 / 超过 PCL2”不能只靠架构名词。M1 之后需要用可复现的基准回答：

| 维度 | 目标 |
| --- | --- |
| 冷启动到可点击启动页 | ≤ 800 ms（不含首次 Java 扫描） |
| Java 扫描 | 并行探测，常规机器 ≤ 1.5 s |
| 1 GB 文件下载吞吐 | 官方线路跑满带宽的 80%+ |
| 安装复用 | 二次安装不重复下载已校验对象 |
| 崩溃识别覆盖率 | 常见 6 类签名可解释 |
| 便携版体积 | 明显小于 Electron 等价方案 |
| 孤儿进程 | 正常退出 / 关闭窗口后为 0 |
| 数据安全 | 恢复走安全快照；删除前二次确认、永久生效；ZIP 穿越拦截 |

## 开发方法

每个里程碑遵循同一流程：

1. 在 `core/` 写可测试的纯逻辑；
2. 在 CLI 上跑通真实数据；
3. 在 Tauri command 中做编排；
4. 更新类型化 `bridge.ts`；
5. 最后才做 UI；
6. 加测试与一份可复现记录到 `docs/TESTING.md`；
7. 不在 UI 上展示未实现的能力。

## 不做什么

- 不捆绑 Minecraft 客户端或资源。
- 不绕过正版验证。
- 不伪装成 PCL2 或 Mojang 官方产品。
- 不在用户不知情的情况下上传存档、日志或账号信息。
