# 架构 · Creeper Core

> 版本：0.1.0（M0）  
> 目标：让启动器核心可以脱离 UI 被阅读、测试和复用。

## 1. 分层

```text
┌────────────────────────────────────────────────────────────┐
│ React 19 UI                                                 │
│ pages / components / Zustand store / bridge.ts              │
└──────────────────────────┬─────────────────────────────────┘
                           │ typed IPC (invoke + events)
┌──────────────────────────▼─────────────────────────────────┐
│ Tauri 2 Shell (src-tauri)                                   │
│ commands.rs  state.rs  capabilities  window / CSP            │
└──────────────────────────┬─────────────────────────────────┘
                           │ plain Rust API
┌──────────────────────────▼─────────────────────────────────┐
│ creeper-core (core/)                                        │
│ version  java  download  launch  diagnose  mods  backups    │
└────────────────────────────────────────────────────────────┘
```

边界规则：

- `core/` 不依赖 Tauri，也不依赖 React。
- `src-tauri/` 只做状态、IPC 编排、窗口与事件；不重复实现业务规则。
- `src/` 不直接访问文件系统或启动 Java；全部经 `bridge.ts` 的命令白名单。
- 浏览器里的 mock backend 只用于 UI 开发，Tauri 中永不启用。判断依据是 `window.__TAURI_INTERNALS__`。

## 2. 关键数据流

### 2.1 启动

```text
launch_game(versionId)
  → AppConfig (account / memory / isolation / java)
  → version::resolve_version(root, id)
      → 读取 versions/<id>/<id>.json
      → 沿 inheritsFrom 向上最多 12 层
      → 合并 libraries / arguments / downloads / logging
  → version::classpath_entries
      → rules_allow(os, arch, features)
      → library_files() 过滤 native classifier
  → java::validate_java_for_version
  → launch::build_launch_plan
      → natives 提取
      → 占位符替换
      → 组装 argv
  → tokio::process::Command 直接 spawn
  → pump_stream stdout/stderr
  → process monitor
```

**为什么不通过 shell**：Minecraft 的 classpath 可能很长，路径中可能有空格与本地化字符。直接把 argv 交给 `Command` 可以避免 Windows quoting 与注入。

### 2.2 安装

```text
install_version(id)
  → fetch_manifest
  → fetch version JSON（SHA-1 验证）
  → 写入 versions/<id>/<id>.json（原子）
  → resolve_version
  → sync_resolved_files
      → client jar
      → libraries（并行、校验、atomic rename）
      → asset index + 5k+ objects（内容寻址）
      → log4j XML
  → InstallProgress events
```

进度是**阶段式字节进度**：

- `resolve`
- `client`
- `libraries`
- `assets`
- `logging`
- `done`

`InstallProgress.percent` 在当前阶段内按已处理字节计算。UI 将其显示为任务进度；回退不会破坏已下载文件。

### 2.3 恢复快照

```text
restore_backup
  → 从 index 取 BackupInfo
  → 自动 create_backup("恢复前自动快照")
  → 打开 ZIP
  → 每个 entry:
       replace('\\','/')
       util::is_safe_relative()
       写入 gameDir
  → RestoreReport
```

`_creeper/` 条目不会被恢复，防止快照携带启动器自己的索引。

## 3. 版本解析细节

### 3.1 继承

链按父 → 子合并。子级覆盖 `mainClass`、`assets`、`assetIndex`、`javaVersion`、`logging`；`libraries` 追加并按完整 Maven name 去重；`arguments` 先父后子追加。`downloads.client` 使用链中最近一个有 client 的版本，从而支持 Fabric 的 `jar_id`。

### 3.2 规则

`rules_allow`：

- 空规则 → 允许；
- 有规则 → 从 false 开始，对每一条匹配规则应用其 action；
- OS name / arch 匹配；features 必须全部匹配传入 feature map；
- 有 `versionRange` 的规则按“现代 Windows 满足”处理（当前不支持按具体 Windows build 判定）；
- 检查 `os.version` 的逻辑故意不启用，以避免错误拒绝 Win 10/11。

### 3.3 native 过滤

现代 Mojang JSON 会列出同一 library 的多个 native classifier（windows / windows-arm64 / windows-x86）。官方启动器只把当前架构的 native jar 放入 classpath。`native_artifact_allowed()` 执行同样的筛选，防止 x64 机器加载 arm64 / x86 DLL。

旧版格式仍通过 `natives` + `downloads.classifiers` 解压到 `versions/<id>/natives`。

### 3.4 Java 选择

`select_best` 选择**满足最低版本的最小 major**。这不是为了怀旧，而是因为 Forge/Fabric 与部分图形模组对过高 JDK 更敏感。

探测候选：

- 配置中显式路径；
- `JAVA_HOME`；
- `gameRoot/runtime/*` 与官方 `.minecraft/runtime/*`；
- 常见 JDK 厂商安装目录；
- `PATH`。

## 4. TNT Cache（下载器）

### 4.1 状态

每个文件：

```text
dest exists → size matches? SHA-1 matches? → reuse
             ↓ no
download to <dest>.part → length check → SHA-1 check
             ↓
remove existing dest, rename(part, dest)
```

- 每个任务有 `CancelFlag`；每个 chunk 检查一次。
- 3 次重试，每次重新建立 `.part`。
- 连接超时 20s；请求总超时 120s。Fabric CDN 在部分网络会“连上但停滞”，已单独路由到 BMCLAPI Maven。
- 并发数来自配置，默认 16，clamp 到 1–64。

### 4.2 镜像

- `official`：Mojang 官方 + Fabric Maven 官方；Fabric Meta 先试官方（10s 超时）再回退 BMCLAPI。
- `bmclapi`：所有可识别 URL 映射到 BMCLAPI。
- Fabric Maven 在官方模式下也固定走 BMCLAPI Maven（该 CDN 在测试网络无法完成下载）。

### 4.3 assets

- 索引写入 `assets/indexes/<id>.json`。
- 对象路径 `assets/objects/<hash[0..2]>/<hash>`。
- 只有缺失或大小不匹配的对象才进入下载队列。
- 旧版 `virtual` / `map_to_resources` 会尝试 hardlink，失败则复制。

## 5. 进程守护

- 每次启动写入 `.creeper/logs/<version>/<timestamp>.log`，原始 stdout/stderr 全量落盘。
- UI 的日志显示会剥离 log4j XML 事件外层，保留线程 / level / message；磁盘日志保持原始内容，供取证。
- 退出后优先读取游戏自己的 `logs/latest.log` 末尾 600 行用于崩溃签名，因为它更干净。
- 手动终止通过 PID / taskkill process tree；关闭主窗口时 Rust 层也会兜底用 `taskkill /T` 结束子进程。
- 崩溃签名是**启发式**，不做过度承诺：OOM、Java class version、GLFW/GL context、模组依赖、缺文件、权限、会话。未知错误给通用日志建议。

## 6. Creeper Sense

诊断是纯本地、只读探测（修复动作是显式命令）。评分从 100 开始，按影响扣除：

| 类别 | 例子 |
| --- | --- |
| 核心文件 | 缺 client jar / libraries / asset index / assets |
| Java | 未找到 / 低于版本要求 / 32 位限制 |
| 内存 | 低于下限 / 超过物理内存 75% |
| 模组 | 重复 mod id / 缺 mandatory 依赖 / loader 不匹配 |
| 档案 | 没有可用档案 |
| 路径 | 游戏目录不可写 |

`has_missing_files` 为 true 时，UI 的运行检查面板会提供修复入口，对应 `repair_game`：按 resolved 清单重新下载缺失文件，不修改已有世界。

## 7. 模组舱

- 从 JAR 内部读取：`fabric.mod.json`、`quilt.mod.json`、`META-INF/neoforge.mods.toml`、`META-INF/mods.toml`、`mcmod.info`。
- 依赖收集是静态的；Fabric 的 `depends`、Forge 的 mandatory 依赖会被检查。不会猜测可选依赖。
- 启用/禁用 = 重命名 `.jar` ↔ `.jar.disabled`。
- 删除 = 直接删除模组文件，永久生效；不创建启动器回收站。
- 导入 = 验证 ZIP → 拒绝同名覆盖 → 复制到当前 gameDir/mods。

## 8. 时光机

- 快照写入 `<launcherRoot>/.creeper/backups/<version>/<id>.zip`。
- 两种范围：
  - `saves`：saves / options.txt / servers.dat / optionsof.txt；
  - `full`：额外 config / defaultconfigs / mods / resourcepacks / shaderpacks / kubejs。
- 每个 ZIP 内写 `_creeper/backup.json`；目录索引 `index.json` 用于快速列表。
- 恢复前自动安全快照；恢复只覆盖归档中存在的文件，不删除未包含的文件。

## 9. Tauri 命令白名单

主要命令：

```text
get_bootstrap
select_version / get_version_detail
scan_java
save_settings
create_account / select_account / delete_account
fetch_remote_versions
install_game / cancel_install / repair_game
diagnose_game
list_mods / toggle_mod / delete_mod / import_mods
list_backups / create_backup / restore_backup / delete_backup
launch_game / stop_game / get_game_status
get_game_logs / read_latest_log
delete_version
open_path / reveal_latest_log
get_app_info
```

事件：

```text
install-start / install-progress / install-finish / install-error
game-start / game-log / game-exit / game-stopping
```

## 10. 安全与本地数据

- 无沙箱以外的网络监听；不下载未验证的可执行文件。
- Tauri capability 只授予窗口控制、dialog 与核心默认权限。
- ZIP 恢复有路径穿越检查；临时文件名固定在目标目录，最终 rename 原子替换。
- 配置损坏时自动备份并回退默认，不阻止启动。
- 不写入官方启动器目录以外的注册表；不绕过正版验证。
- 离线 UUID 采用 Java `UUID.nameUUIDFromBytes(("OfflinePlayer:" + name))` 语义。

## 11. 已知技术债

- 下载器没有 Range 级 HTTP 续传，只有文件级缓存复用。
- 下载与解压的 SHA-1 校验阻塞在 async worker 中（文件不大，但大 jar 会有一次 IO 峰值）。
- 部分错误在 UI 直接显示 Rust 字符串；尚未统一错误码。
- 无下载任务队列；一次只能有一个安装或修复任务（保护共享目录）。
- Forge / NeoForge 通过官方 installer 的 `--installClient` 安装，引擎负责准备 `launcher_profiles.json`、选择 Java、运行安装器并发现新版本；没有手写 processor 流程。
- 配置策略暂无 schema version migration，仅有字段级 normalize。

这些债务是路线图的一部分，而不是隐藏的“已完成”。
