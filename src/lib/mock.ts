/**
 * Browser-only preview backend.
 *
 * The real desktop app always calls the Rust engine. This mock exists so the
 * interface can be developed and screenshotted in a normal browser without
 * faking any production code path inside Tauri.
 */
import type {
  AccountProfile,
  BackupInfo,
  BootstrapData,
  CrashReport,
  InstallProgress,
  InstallReport,
  JavaInfo,
  LaunchGameResult,
  ModInfo,
  ModrinthHit,
  RemoteVersion,
  RestoreReport,
  VersionManifest,
  VersionSummary,
} from "./types";

type EventHandler = (payload: unknown) => void;

const eventListeners = new Map<string, Set<EventHandler>>();

export const mockEventBus = {
  listen(name: string, handler: EventHandler): () => void {
    const bucket = eventListeners.get(name) ?? new Set<EventHandler>();
    bucket.add(handler);
    eventListeners.set(name, bucket);
    return () => {
      bucket.delete(handler);
      if (bucket.size === 0) eventListeners.delete(name);
    };
  },
  emit(name: string, payload: unknown) {
    for (const handler of eventListeners.get(name) ?? []) {
      handler(payload);
    }
  },
};

export function mockIsTauriFilePickerAvailable(): string[] {
  return [];
}

const now = new Date();
const daysAgo = (days: number, hours = 0) =>
  new Date(now.getTime() - days * 86400000 - hours * 3600000).toISOString();
const epoch = (days: number) => now.getTime() - days * 86400000;

const java21: JavaInfo = {
  path: "C:\\Program Files\\Eclipse Adoptium\\jdk-21.0.5.11-hotspot\\bin\\java.exe",
  version: "21.0.5",
  major: 21,
  vendor: "Eclipse Adoptium",
  architecture: "x86_64",
  is64bit: true,
  source: "系统安装",
  recommended: true,
};

const java17: JavaInfo = {
  path: "C:\\Program Files\\Microsoft\\jdk-17.0.13.11-hotspot\\bin\\java.exe",
  version: "17.0.13",
  major: 17,
  vendor: "Microsoft",
  architecture: "x86_64",
  is64bit: true,
  source: "系统安装",
  recommended: false,
};

const java25: JavaInfo = {
  path: "C:\\Users\\54428\\AppData\\Roaming\\.minecraft\\runtime\\java-runtime-epsilon\\bin\\java.exe",
  version: "25.0.1",
  major: 25,
  vendor: "Microsoft",
  architecture: "x86_64",
  is64bit: true,
  source: "Minecraft 运行时",
  recommended: false,
};

const account: AccountProfile = {
  id: "profile-creeper-steve",
  username: "Creeper_Steve",
  uuid: "1d9f4b7e-ff2d-3b44-9b18-77b8c4e55291",
  kind: "offline",
  createdAt: daysAgo(48),
  lastUsed: daysAgo(0, 3),
};

const versions: VersionSummary[] = [
  {
    id: "fabric-loader-0.16.9-1.21.1",
    displayName: "1.21.1 · Fabric",
    versionType: "release",
    loader: "fabric",
    loaderVersion: "0.16.9",
    releaseTime: "2024-08-08T14:13:41+00:00",
    lastModified: epoch(1),
    javaMajor: 21,
    hasJson: true,
    hasClient: true,
    mainClass: "net.fabricmc.loader.impl.launch.knot.KnotClient",
    inheritsFrom: "1.21.1",
    readError: null,
  },
  {
    id: "26.2",
    displayName: "26.2",
    versionType: "release",
    loader: "vanilla",
    loaderVersion: null,
    releaseTime: "2026-06-16T12:03:33+00:00",
    lastModified: epoch(3),
    javaMajor: 25,
    hasJson: true,
    hasClient: true,
    mainClass: "net.minecraft.client.main.Main",
    inheritsFrom: null,
    readError: null,
  },
  {
    id: "1.21.11",
    displayName: "1.21.11",
    versionType: "release",
    loader: "vanilla",
    loaderVersion: null,
    releaseTime: "2026-02-27T10:20:00+00:00",
    lastModified: epoch(8),
    javaMajor: 21,
    hasJson: true,
    hasClient: true,
    mainClass: "net.minecraft.client.main.Main",
    inheritsFrom: null,
    readError: null,
  },
  {
    id: "1.20.1-forge-47.3.0",
    displayName: "1.20.1 · Forge",
    versionType: "release",
    loader: "forge",
    loaderVersion: "47.3.0",
    releaseTime: "2023-06-12T12:47:53+00:00",
    lastModified: epoch(15),
    javaMajor: 17,
    hasJson: true,
    hasClient: true,
    mainClass: "cpw.mods.bootstraplauncher.BootstrapLauncher",
    inheritsFrom: "1.20.1",
    readError: null,
  },
  {
    id: "1.21.4-pre3",
    displayName: "1.21.4-pre3",
    versionType: "snapshot",
    loader: "vanilla",
    loaderVersion: null,
    releaseTime: "2024-11-27T09:22:11+00:00",
    lastModified: epoch(31),
    javaMajor: 21,
    hasJson: true,
    hasClient: true,
    mainClass: "net.minecraft.client.main.Main",
    inheritsFrom: null,
    readError: null,
  },
];

const mods: ModInfo[] = [
  {
    fileName: "sodium-fabric-0.6.13+mc1.21.1.jar",
    path: "mods\\sodium-fabric-0.6.13+mc1.21.1.jar",
    name: "Sodium",
    modId: "sodium",
    version: "0.6.13+mc1.21.1",
    description: "现代化渲染引擎，大幅提高帧率并减少卡顿。",
    authors: ["jellysquid3", "CaffeineMC"],
    loader: "fabric",
    enabled: true,
    size: 1626344,
    modified: epoch(2),
    missingDependencies: [],
    duplicate: false,
    readError: null,
  },
  {
    fileName: "iris-fabric-1.8.8+mc1.21.1.jar",
    path: "mods\\iris-fabric-1.8.8+mc1.21.1.jar",
    name: "Iris Shaders",
    modId: "iris",
    version: "1.8.8+mc1.21.1",
    description: "兼容 OptiFine 光影包的高性能光影加载器。",
    authors: ["coderbot", "Iris Team"],
    loader: "fabric",
    enabled: true,
    size: 7842912,
    modified: epoch(2),
    missingDependencies: [],
    duplicate: false,
    readError: null,
  },
  {
    fileName: "fabric-api-0.115.6+1.21.1.jar",
    path: "mods\\fabric-api-0.115.6+1.21.1.jar",
    name: "Fabric API",
    modId: "fabric-api",
    version: "0.115.6+1.21.1",
    description: "Fabric 模组生态的基础 API。",
    authors: ["FabricMC"],
    loader: "fabric",
    enabled: true,
    size: 2293817,
    modified: epoch(2),
    missingDependencies: [],
    duplicate: false,
    readError: null,
  },
  {
    fileName: "just-enough-items-19.21.0.418.jar",
    path: "mods\\just-enough-items-19.21.0.418.jar",
    name: "Just Enough Items",
    modId: "jei",
    version: "19.21.0.418",
    description: "物品配方查询，按 R 查看用途，按 U 查看来源。",
    authors: ["mezz"],
    loader: "fabric",
    enabled: true,
    size: 1827344,
    modified: epoch(4),
    missingDependencies: ["architectury"],
    duplicate: false,
    readError: null,
  },
  {
    fileName: "lithium-fabric-0.15.3+mc1.21.1.jar",
    path: "mods\\lithium-fabric-0.15.3+mc1.21.1.jar",
    name: "Lithium",
    modId: "lithium",
    version: "0.15.3+mc1.21.1",
    description: "优化服务端与客户端逻辑，降低 CPU 占用。",
    authors: ["CaffeineMC"],
    loader: "fabric",
    enabled: true,
    size: 1293002,
    modified: epoch(2),
    missingDependencies: [],
    duplicate: false,
    readError: null,
  },
  {
    fileName: "ferritecore-7.0.2-fabric.jar",
    path: "mods\\ferritecore-7.0.2-fabric.jar",
    name: "FerriteCore",
    modId: "ferritecore",
    version: "7.0.2",
    description: "减少游戏内存占用。",
    authors: ["malte0811"],
    loader: "fabric",
    enabled: true,
    size: 182002,
    modified: epoch(3),
    missingDependencies: [],
    duplicate: false,
    readError: null,
  },
  {
    fileName: "krypton-0.2.8.jar",
    path: "mods\\krypton-0.2.8.jar",
    name: "Krypton",
    modId: "krypton",
    version: "0.2.8",
    description: "优化网络栈，降低多人游戏延迟。",
    authors: ["astei"],
    loader: "fabric",
    enabled: true,
    size: 346112,
    modified: epoch(3),
    missingDependencies: [],
    duplicate: false,
    readError: null,
  },
  {
    fileName: "modmenu-11.0.3.jar",
    path: "mods\\modmenu-11.0.3.jar",
    name: "Mod Menu",
    modId: "modmenu",
    version: "11.0.3",
    description: "在游戏内查看与管理已安装模组。",
    authors: ["Prospector"],
    loader: "fabric",
    enabled: true,
    size: 729810,
    modified: epoch(5),
    missingDependencies: [],
    duplicate: false,
    readError: null,
  },
  {
    fileName: "cloth-config-15.0.140-fabric.jar",
    path: "mods\\cloth-config-15.0.140-fabric.jar",
    name: "Cloth Config API",
    modId: "cloth-config",
    version: "15.0.140",
    description: "模组配置界面基础库。",
    authors: ["shedaniel"],
    loader: "fabric",
    enabled: true,
    size: 1092261,
    modified: epoch(5),
    missingDependencies: [],
    duplicate: false,
    readError: null,
  },
  {
    fileName: "xaeros-minimap-24.6.1.jar",
    path: "mods\\xaeros-minimap-24.6.1.jar",
    name: "Xaero's Minimap",
    modId: "xaerominimap",
    version: "24.6.1",
    description: "可定制的世界小地图。",
    authors: ["xaero96"],
    loader: "fabric",
    enabled: true,
    size: 2263821,
    modified: epoch(6),
    missingDependencies: [],
    duplicate: false,
    readError: null,
  },
  {
    fileName: "distanthorizons-2.3.0-b.jar.disabled",
    path: "mods\\distanthorizons-2.3.0-b.jar.disabled",
    name: "Distant Horizons",
    modId: "distanthorizons",
    version: "2.3.0-b",
    description: "以 LOD 技术渲染超远视距。",
    authors: ["jeseibel"],
    loader: "fabric",
    enabled: false,
    size: 31938211,
    modified: epoch(12),
    missingDependencies: [],
    duplicate: false,
    readError: null,
  },
];

const backups: BackupInfo[] = [
  {
    id: "20261028-214501",
    versionId: "fabric-loader-0.16.9-1.21.1",
    name: "下界探险之前",
    note: "拿到下界合金锭之前，存个档。",
    createdAt: daysAgo(2, 2),
    sizeBytes: 13107200,
    fileCount: 84,
    kind: "saves",
    path: ".creeper/backups/1.21.1/20261028-214501.zip",
    gameDir: "C:\\Users\\Creeper\\AppData\\Roaming\\.minecraft",
  },
  {
    id: "20261024-100311",
    versionId: "fabric-loader-0.16.9-1.21.1",
    name: "建筑工坊 · 完整快照",
    note: "包含模组与配置。",
    createdAt: daysAgo(6, 3),
    sizeBytes: 42231700,
    fileCount: 219,
    kind: "full",
    path: ".creeper/backups/1.21.1/20261024-100311.zip",
    gameDir: "C:\\Users\\Creeper\\AppData\\Roaming\\.minecraft",
  },
  {
    id: "20261018-082100",
    versionId: "fabric-loader-0.16.9-1.21.1",
    name: "自动快照 · 周常",
    note: "每周自动保存一次。",
    createdAt: daysAgo(12, 6),
    sizeBytes: 9183232,
    fileCount: 51,
    kind: "saves",
    path: ".creeper/backups/1.21.1/20261018-082100.zip",
    gameDir: "C:\\Users\\Creeper\\AppData\\Roaming\\.minecraft",
  },
];

const diagnosticReport = {
  versionId: "fabric-loader-0.16.9-1.21.1",
  score: 93,
  level: "ready",
  headline: "运行环境正常",
  checkedAt: daysAgo(0),
  javaPath: java21.path,
  javaMajor: 21,
  hasMissingFiles: false,
  items: [
    {
      id: "core-files",
      severity: "ok",
      title: "核心文件完整",
      detail: "客户端核心与 Fabric 运行库均已就位。",
      action: "none",
      fixable: false,
      points: 0,
    },
    {
      id: "assets",
      severity: "ok",
      title: "资源对象完整",
      detail: "已校验 3,824 个资源对象。",
      action: "none",
      fixable: false,
      points: 0,
    },
    {
      id: "mods",
      severity: "warn",
      title: "1 个模组依赖未满足",
      detail: "缺少：architectury（由 Just Enough Items 请求）",
      action: "mods",
      fixable: false,
      points: -7,
    },
    {
      id: "account",
      severity: "info",
      title: "离线档案 · Creeper_Steve",
      detail: "单人游戏可用；正版多人服务器需要在线登录。",
      action: "settings",
      fixable: false,
      points: 0,
    },
  ],
};

const config = {
  configVersion: 1,
  gameDir: "C:\\Users\\Creeper\\AppData\\Roaming\\.minecraft",
  selectedVersion: "fabric-loader-0.16.9-1.21.1",
  accounts: [account],
  activeAccountId: account.id,
  javaPath: java21.path,
  memoryMb: 6144,
  jvmArgs: "",
  width: 1280,
  height: 720,
  fullscreen: false,
  downloadSource: "official",
  communitySource: "official",
  downloadConcurrency: 16,
  isolateVersions: true,
  versionSettings: {},
  closeAfterLaunch: false,
  minimizeOnLaunch: true,
  backgroundAnimation: true,
  showLogOnCrash: true,
  autoBackupBeforeLaunch: false,
  lastPage: "home",
};

let mockState: BootstrapData = {
  config,
  versions,
  javas: [java21, java17, java25],
  selectedJava: java21,
  system: {
    os: "Windows",
    arch: "x86_64",
    cpuThreads: 24,
    totalMemoryMb: 32604,
    usedMemoryMb: 14022,
    launcherVersion: "0.1.0",
  },
  diagnostics: diagnosticReport,
  mods,
  backups,
  gameStatus: null,
  activeInstall: null,
  logs: [],
  lastCrash: null,
  systemClock: now.toISOString(),
};

let mockRemoteManifest: VersionManifest = {
  latest: { release: "26.2", snapshot: "26.3-snapshot-9" },
  versions: [
    remote("26.2", "release", "2026-06-16T12:03:33+00:00"),
    remote("26.1.2", "release", "2026-05-10T08:30:00+00:00"),
    remote("26.3-snapshot-9", "snapshot", "2026-08-23T03:27:08+00:00"),
    remote("1.21.11", "release", "2026-02-27T10:20:00+00:00"),
    remote("1.21.4", "release", "2024-12-03T10:12:57+00:00"),
    remote("1.21.1", "release", "2024-08-08T14:13:41+00:00"),
    remote("1.21", "release", "2024-06-13T08:24:03+00:00"),
    remote("1.20.6", "release", "2024-04-29T12:40:00+00:00"),
    remote("1.20.1", "release", "2023-06-12T12:47:53+00:00"),
    remote("1.19.4", "release", "2023-03-14T12:56:18+00:00"),
    remote("1.18.2", "release", "2022-02-28T10:42:45+00:00"),
    remote("1.16.5", "release", "2021-01-14T16:05:00+00:00"),
    remote("1.12.2", "release", "2017-09-18T08:39:46+00:00"),
    remote("1.7.10", "release", "2014-05-14T17:29:23+00:00"),
  ],
};

function remote(id: string, type: string, releaseTime: string): RemoteVersion {
  return {
    id,
    type,
    url: "https://piston-meta.mojang.com/version.json",
    time: releaseTime,
    releaseTime,
    sha1: "0000000000000000000000000000000000000000",
    complianceLevel: 1,
  };
}

const clone = <T>(value: T): T => structuredClone(value);

const delay = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

let mockInstallCancelled = false;
let mockGameLogTimer: ReturnType<typeof setInterval> | null = null;
let mockGameStatus: LaunchGameResult | null = null;

export async function mockInvoke<T>(
  command: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  switch (command) {
    case "get_bootstrap":
      mockState = { ...mockState, systemClock: new Date().toISOString() };
      return clone(mockState) as T;

    case "select_version": {
      mockState.config.selectedVersion = String(args.versionId);
      mockState.diagnostics = diagnosticReport;
      return undefined as T;
    }

    case "get_version_detail": {
      const id = String(args.versionId);
      const version =
        mockState.versions.find((item) => item.id === id) ?? mockState.versions[0];
      return {
        id: version.id,
        displayName: version.displayName,
        mainClass: version.mainClass ?? "net.minecraft.client.main.Main",
        versionType: version.versionType,
        loader: version.loader,
        loaderVersion: version.loaderVersion,
        inheritsFrom: version.inheritsFrom,
        jarId: version.inheritsFrom ?? version.id,
        javaMajor: version.javaMajor,
        assetsId: "32",
        librariesCount: version.loader === "fabric" ? 138 : 121,
        librariesMissing: 0,
        clientJarExists: version.hasClient,
        assetIndexExists: true,
        assetsMissing: 0,
        gameDir: mockState.config.gameDir,
        isIsolated: mockState.config.isolateVersions,
        modsCount: version.loader === "fabric" ? mockState.mods.length : 0,
        totalDownloadSize: version.loader === "fabric" ? 524288000 : 491192320,
      } as T;
    }

    case "scan_java":
      return {
        javas: clone(mockState.javas),
        recommended: clone(mockState.selectedJava),
        requiredMajor: 21,
      } as T;

    case "save_settings": {
      const update = args.update as Record<string, unknown>;
      mockState.config = {
        ...mockState.config,
        gameDir: String(update.gameDir),
        javaPath: update.javaPath ? String(update.javaPath) : null,
        memoryMb: Number(update.memoryMb),
        jvmArgs: String(update.jvmArgs ?? ""),
        width: Number(update.width),
        height: Number(update.height),
        fullscreen: Boolean(update.fullscreen),
        downloadSource: String(update.downloadSource),
        communitySource: String(update.communitySource),
        downloadConcurrency: Number(update.downloadConcurrency),
        isolateVersions: Boolean(update.isolateVersions),
        backgroundAnimation: Boolean(update.backgroundAnimation),
        minimizeOnLaunch: Boolean(update.minimizeOnLaunch),
        closeAfterLaunch: Boolean(update.closeAfterLaunch),
        autoBackupBeforeLaunch: Boolean(update.autoBackupBeforeLaunch),
        showLogOnCrash: Boolean(update.showLogOnCrash),
      };
      return clone(mockState.config) as T;
    }

    case "set_version_isolation":
      return { ...mockState.config } as T;

    case "create_account": {
      const username = String(args.username ?? "").trim();
      if (!/^[A-Za-z0-9_]{3,16}$/.test(username)) {
        throw new Error("玩家名仅支持 3–16 位英文字母、数字和下划线");
      }
      if (mockState.config.accounts.some((item) => item.username === username)) {
        throw new Error("已存在同名离线档案");
      }
      const profile: AccountProfile = {
        id: `mock-${Date.now()}`,
        username,
        uuid: crypto.randomUUID(),
        kind: "offline",
        createdAt: new Date().toISOString(),
      };
      mockState.config.accounts = [...mockState.config.accounts, profile];
      mockState.config.activeAccountId = profile.id;
      return clone(mockState.config) as T;
    }

    case "save_local_skin":
    case "delete_local_skin":
      return undefined as T;

    case "list_instance_entries":
      return [
        {
          name: "新的世界",
          path: "C:\\mock\\saves\\新的世界",
          isDir: true,
          size: 0,
          modified: new Date().toISOString(),
        },
      ] as T;

    case "save_version_settings": {
      const versionId = String(args.versionId);
      const update = args.update as {
        memoryMb: number | null;
        javaPath: string | null;
        jvmArgs: string | null;
        isolate: boolean | null;
      };
      const settings = mockState.config.versionSettings ?? {};
      settings[versionId] = {
        memoryMb: update.memoryMb,
        javaPath: update.javaPath,
        jvmArgs: update.jvmArgs,
        isolate: update.isolate,
      };
      mockState.config.versionSettings = settings;
      return clone(mockState.config) as T;
    }

    case "list_modrinth_projects":
      return ((args.projectIds as string[] | undefined) ?? []).map((id) => ({
        projectId: id,
        slug: id.toLowerCase(),
        title:
          id === "P7dR8mSH"
            ? "Fabric API"
            : id === "Ha28R6CL"
              ? "Fabric Language Kotlin"
              : id,
        iconUrl: null,
      })) as T;

    case "msa_profile":
      return {
        skinDataUrl: null,
        skinVariant: "classic",
        capes: [],
        activeCapeId: null,
      } as T;

    case "msa_upload_skin":
    case "msa_delete_skin":
    case "msa_set_cape":
      throw new Error("浏览器预览模式无法修改 Microsoft 皮肤或披风");

    case "msa_embedded_login":
      throw new Error("浏览器预览模式无法打开 Microsoft 登录窗口，请使用桌面版");

    case "msa_refresh_account":
      throw new Error("浏览器预览模式无法刷新 Microsoft 登录");

    case "select_account":
      mockState.config.activeAccountId = String(args.accountId);
      return clone(mockState.config) as T;

    case "delete_account":
      mockState.config.accounts = mockState.config.accounts.filter(
        (item) => item.id !== args.accountId,
      );
      mockState.config.activeAccountId = mockState.config.accounts[0]?.id ?? null;
      return clone(mockState.config) as T;

    case "fetch_remote_versions":
      return clone(mockRemoteManifest) as T;

    case "install_game": {
      const versionId = String(args.versionId);
      const loader = String(args.loader ?? "vanilla");
      const taskId = `mock-install-${Date.now()}`;
      mockInstallCancelled = false;
      mockState.activeInstall = { taskId, label: `安装 ${versionId}` };
      mockEventBus.emit("install-start", { taskId, versionId, loader });
      const phases = [
        ["resolve", "正在解析版本清单", 8],
        ["client", "正在下载客户端核心", 22],
        ["libraries", "正在同步运行库", 47],
        ["assets", "正在同步资源文件", 76],
        ["logging", "正在同步日志配置", 91],
        ["done", "安装完成", 100],
      ] as const;
      for (const [phase, label, percent] of phases) {
        if (mockInstallCancelled) {
          mockState.activeInstall = null;
          throw new Error("安装已取消");
        }
        await delay(420);
        const progress: InstallProgress = {
          taskId,
          phase,
          label,
          current: Math.round((percent / 100) * 96),
          total: 96,
          bytesDone: Math.round((percent / 100) * 482_000_000),
          bytesTotal: 482_000_000,
          percent,
          message: `${label}  ${Math.round((percent / 100) * 96)}/96`,
        };
        mockEventBus.emit("install-progress", progress);
      }
      mockState.activeInstall = null;
      const installed: VersionSummary = {
        id: versionId,
        displayName: versionId,
        versionType: "release",
        loader,
        loaderVersion: loader === "fabric" ? "0.16.9" : null,
        releaseTime: new Date().toISOString(),
        lastModified: Date.now(),
        javaMajor: java21.major,
        hasJson: true,
        hasClient: true,
        mainClass: "net.minecraft.client.main.Main",
        inheritsFrom: null,
        readError: null,
      };
      if (!mockState.versions.some((item) => item.id === versionId)) {
        mockState.versions = [installed, ...mockState.versions];
      }
      mockState.config.selectedVersion = versionId;
      mockEventBus.emit("install-finish", {
        versionId,
        loader,
        report: {
          versionId,
          filesDownloaded: 96,
          filesReused: 12,
          bytesDownloaded: 482_000_000,
          elapsedMs: 2600,
        },
        warnings: [],
      });
      return {
        versionId,
        loader,
        report: {
          versionId,
          filesDownloaded: 96,
          filesReused: 12,
          bytesDownloaded: 482_000_000,
          elapsedMs: 2600,
        },
        warnings: [],
      } as T;
    }

    case "cancel_install":
      mockInstallCancelled = true;
      return { cancelled: true } as T;

    case "cancel_modrinth_install":
      return undefined as T;

    case "repair_game": {
      const report: InstallReport = {
        versionId: String(args.versionId),
        filesDownloaded: 0,
        filesReused: 142,
        bytesDownloaded: 0,
        elapsedMs: 1800,
      };
      return report as T;
    }

    case "diagnose_game":
      return clone(diagnosticReport) as T;

    case "search_modrinth_content": {
      const query = String(args.query ?? "").trim().toLowerCase();
      const projectType = String(args.projectType);
      const hits: ModrinthHit[] = [
        {
          projectId: projectType === "shader" ? "shader-demo" : projectType === "datapack" ? "datapack-demo" : "resourcepack-demo",
          slug: `${projectType}-demo`,
          title:
            projectType === "shader"
              ? "Complementary Reimagined"
              : projectType === "datapack"
                ? "Terralith"
                : "Fresh Animations",
          description:
            projectType === "shader"
              ? "高质量光影包，保留原版风格并增强光照与水面。"
              : projectType === "datapack"
                ? "大规模地形生成数据包，增加新的生物群系与结构。"
                : "为原版生物添加更生动的动画。",
          author: "demo",
          iconUrl: null,
          downloads: 3200000,
          follows: 900,
          categories: [projectType === "shader" ? "shaders" : projectType === "datapack" ? "worldgen" : "decoration"],
          versions: ["1.21.1"],
          latestVersion: "demo-1.0",
          dateModified: daysAgo(4),
          clientSide: "required",
          serverSide: "optional",
        },
      ];
      const filtered = query
        ? hits.filter((hit) => `${hit.title} ${hit.description}`.toLowerCase().includes(query))
        : hits;
      return { hits: filtered, offset: 0, limit: 20, totalHits: filtered.length } as T;
    }

    case "install_modrinth_content":
      return {
        installed: ["demo-content.zip"],
        skipped: [],
        warnings: [],
      } as T;

    case "list_modrinth_versions":
      return [
        {
          id: "SMxNOGZ6",
          projectId: String(args.projectId),
          name: "Sodium 0.8.13 for Fabric 1.21.1",
          versionNumber: "mc1.21.1-0.8.13-fabric",
          versionType: "release",
          datePublished: daysAgo(2),
          downloads: 1250000,
          gameVersions: ["1.21.1"],
          loaders: ["fabric"],
          fileName: "sodium-fabric-0.8.13+mc1.21.1.jar",
          fileSize: 1574609,
          requiredDependencies: 1,
          dependencyProjectIds: ["P7dR8mSH"],
        },
        {
          id: "QV48eyCs",
          projectId: String(args.projectId),
          name: "Sodium 0.8.13-beta.2 for Fabric 1.21.1",
          versionNumber: "mc1.21.1-0.8.13-beta.2-fabric",
          versionType: "beta",
          datePublished: daysAgo(8),
          downloads: 82000,
          gameVersions: ["1.21.1"],
          loaders: ["fabric"],
          fileName: "sodium-fabric-0.8.13-beta.2+mc1.21.1.jar",
          fileSize: 1568000,
          requiredDependencies: 0,
        },
      ] as T;

    case "install_modrinth_version":
      return {
        installed: ["sodium-fabric-0.8.13+mc1.21.1.jar"],
        skipped: [],
        warnings: [],
      } as T;

    case "translate_modrinth_summaries":
      return {
        AANobbMI: "现代渲染引擎，大幅提高帧率并减少卡顿。",
        YL57xq9U: "兼容 OptiFine 光影包的高性能光影加载器。",
        P7dR8mSH: "大多数 Fabric 模组依赖的基础 API。",
        Ha28R6CL: "Kotlin 语言支持库。",
      } as T;

    case "search_modrinth": {
      const query = String(args.query ?? "").trim().toLowerCase();
      const hits: ModrinthHit[] = [
        {
          projectId: "AANobbMI",
          slug: "sodium",
          title: "Sodium",
          description: "现代渲染引擎，大幅提高帧率并减少卡顿。",
          author: "jellysquid3",
          iconUrl: null,
          downloads: 257195667,
          follows: 18421,
          categories: ["fabric", "optimization"],
          versions: ["1.21.1", "1.21"],
          latestVersion: "SMxNOGZ6",
          dateModified: daysAgo(2),
          clientSide: "required",
          serverSide: "unsupported",
        },
        {
          projectId: "YL57xq9U",
          slug: "iris",
          title: "Iris Shaders",
          description: "兼容 OptiFine 光影包的高性能光影加载器。",
          author: "coderbot",
          iconUrl: null,
          downloads: 95000000,
          follows: 9320,
          categories: ["fabric", "shaders"],
          versions: ["1.21.1", "1.21"],
          latestVersion: "iris-1.8.8",
          dateModified: daysAgo(2),
          clientSide: "required",
          serverSide: "unsupported",
        },
        {
          projectId: "P7dR8mSH",
          slug: "fabric-api",
          title: "Fabric API",
          description: "大多数 Fabric 模组依赖的基础 API。",
          author: "modmuss50",
          iconUrl: null,
          downloads: 210000000,
          follows: 15400,
          categories: ["fabric", "library"],
          versions: ["1.21.1", "1.21"],
          latestVersion: "fabric-api-0.115.6",
          dateModified: daysAgo(5),
          clientSide: "required",
          serverSide: "required",
        },
        {
          projectId: "Ha28R6CL",
          slug: "fabric-language-kotlin",
          title: "Fabric Language Kotlin",
          description: "Kotlin 语言支持库。",
          author: "modmuss50",
          iconUrl: null,
          downloads: 120000000,
          follows: 3400,
          categories: ["fabric", "library"],
          versions: ["1.21.1", "1.21"],
          latestVersion: "fabric-language-kotlin-1.13",
          dateModified: daysAgo(9),
          clientSide: "required",
          serverSide: "required",
        },
      ];
      const filtered = query
        ? hits.filter((hit) => `${hit.title} ${hit.description}`.toLowerCase().includes(query))
        : hits;
      return { hits: filtered, offset: 0, limit: 20, totalHits: filtered.length } as T;
    }

    case "install_modrinth":
      return {
        installed: ["sodium-fabric-0.8.13+mc1.21.1.jar"],
        skipped: [],
        warnings: [],
      } as T;

    case "search_modrinth_modpacks": {
      const query = String(args.query ?? "").trim().toLowerCase();
      const hits: ModrinthHit[] = [
        {
          projectId: "1KVo5zza",
          slug: "fabulously-optimized",
          title: "Fabulously Optimized",
          description: "以优化和稳定性为目标的 Fabric 整合包，开箱即用。",
          author: "robotkoer",
          iconUrl: null,
          downloads: 18400000,
          follows: 3200,
          categories: ["fabric", "optimization"],
          versions: ["1.21.1"],
          latestVersion: "FO-6.6.0",
          dateModified: daysAgo(3),
          clientSide: "required",
          serverSide: "optional",
        },
        {
          projectId: "4b0qLb1n",
          slug: "adrenaline",
          title: "Adrenaline",
          description: "轻量 Fabric 性能整合包，减少卡顿，保留原版体验。",
          author: "Draylar",
          iconUrl: null,
          downloads: 6100000,
          follows: 1200,
          categories: ["fabric", "optimization"],
          versions: ["1.21.1"],
          latestVersion: "adrenaline-1.4.0",
          dateModified: daysAgo(8),
          clientSide: "required",
          serverSide: "optional",
        },
        {
          projectId: "s4J1l4sZ",
          slug: "simply-optimized",
          title: "Simply Optimized",
          description: "只放必要优化模组的极简整合包。",
          author: "MinecraftForge",
          iconUrl: null,
          downloads: 2800000,
          follows: 770,
          categories: ["fabric", "optimization"],
          versions: ["1.21.1"],
          latestVersion: "simply-optimized-3.2",
          dateModified: daysAgo(12),
          clientSide: "required",
          serverSide: "optional",
        },
      ];
      const filtered = query
        ? hits.filter((hit) => `${hit.title} ${hit.description}`.toLowerCase().includes(query))
        : hits;
      return { hits: filtered, offset: 0, limit: 20, totalHits: filtered.length } as T;
    }

    case "install_modrinth_modpack":
      return {
        versionId: "pack-fabulously-optimized",
        packName: "Fabulously Optimized",
        minecraftVersion: "1.21.1",
        loaderVersion: "0.16.9",
        filesInstalled: 24,
        filesSkipped: 0,
        overrides: 12,
        warnings: [],
      } as T;

    case "list_mods":
      return clone(mockState.mods) as T;

    case "toggle_mod": {
      const fileName = String(args.fileName);
      const enabled = Boolean(args.enabled);
      const mod = mockState.mods.find((item) => item.fileName === fileName);
      if (mod) {
        mod.enabled = enabled;
        mod.fileName = enabled
          ? mod.fileName.replace(/\.disabled$/, "")
          : `${mod.fileName.replace(/\.disabled$/, "")}.disabled`;
      }
      return (mod?.fileName ?? fileName) as T;
    }

    case "delete_mod": {
      const fileName = String(args.fileName);
      mockState.mods = mockState.mods.filter((item) => item.fileName !== fileName);
      return `mods\\${fileName}` as T;
    }

    case "import_mods":
      return { changed: [], skipped: [], errors: [] } as T;

    case "list_backups":
      return clone(mockState.backups) as T;

    case "create_backup": {
      const backup: BackupInfo = {
        id: `${Date.now()}`,
        versionId: String(args.versionId),
        name: String(args.name || "未命名快照"),
        note: String(args.note ?? ""),
        createdAt: new Date().toISOString(),
        sizeBytes: 8_912_640 + Math.round(Math.random() * 4_000_000),
        fileCount: 42,
        kind: args.includeMods ? "full" : "saves",
        path: ".creeper/backups/mock.zip",
        gameDir: mockState.config.gameDir,
      };
      mockState.backups = [backup, ...mockState.backups];
      return clone(backup) as T;
    }

    case "restore_backup": {
      const backup = mockState.backups.find((item) => item.id === args.backupId) ?? mockState.backups[0];
      const report: RestoreReport = {
        backup,
        safetyBackup: null,
        filesRestored: backup.fileCount,
        filesSkipped: 0,
        message: `已恢复 ${backup.fileCount} 个文件，原状态已自动备份`,
      };
      return report as T;
    }

    case "delete_backup":
      mockState.backups = mockState.backups.filter(
        (item) => item.id !== args.backupId,
      );
      return undefined as T;

    case "launch_game": {
      await new Promise((resolve) => window.setTimeout(resolve, 1200));
      const versionId = String(args.versionId);
      const version = mockState.versions.find((item) => item.id === versionId);
      const result: LaunchGameResult = {
        pid: 24816,
        versionId,
        startedAt: new Date().toISOString(),
        logPath: "C:\\Users\\Creeper\\AppData\\Roaming\\.minecraft\\.creeper\\logs\\latest.log",
        warnings:
          version?.javaMajor && version.javaMajor > 21
            ? [`Java ${version.javaMajor} 高于建议版本，可能出现模组兼容问题`]
            : [],
        javaPath: mockState.selectedJava?.path ?? java21.path,
        javaMajor: mockState.selectedJava?.major ?? 21,
        gameDir: mockState.config.gameDir,
      };
      mockGameStatus = result;
      mockState.gameStatus = {
        pid: result.pid,
        versionId,
        startedAt: result.startedAt,
        logPath: result.logPath,
      };
      mockState.logs = [];
      mockEventBus.emit("game-start", {
        pid: result.pid,
        versionId,
        startedAt: result.startedAt,
        logPath: result.logPath,
      });
      const lines = [
        "[10:42:01] [main/INFO]: Loading Minecraft 1.21.1 with Fabric Loader 0.16.9",
        "[10:42:01] [main/INFO]: Loading 12 mods:",
        "\t- fabric-api 0.115.6+1.21.1",
        "\t- fabricloader 0.16.9",
        "\t- iris 1.8.8+mc1.21.1",
        "\t- sodium 0.6.13+mc1.21.1",
        "[10:42:02] [main/INFO]: SpongePowered MIXIN Subsystem initialized",
        "[10:42:03] [Render thread/INFO]: Backend library: LWJGL version 3.3.3+sf3",
        "[10:42:04] [Render thread/INFO]: OpenGL debug context created",
        "[10:42:05] [Render thread/INFO]: Narrator library for x64 successfully loaded",
        "[10:42:06] [Render thread/INFO]: Reloading ResourceManager: vanilla",
        "[10:42:08] [Render thread/INFO]: Sound engine started",
        "[10:42:10] [Render thread/INFO]: Created: 1024x512x4 minecraft:textures/atlas/blocks.png-atlas",
        "[10:42:12] [Render thread/INFO]: Loaded 12869 recipes",
        "[10:42:14] [Render thread/INFO]: Time elapsed: 10.461 seconds",
      ];
      if (mockGameLogTimer) clearInterval(mockGameLogTimer);
      let index = 0;
      mockGameLogTimer = setInterval(() => {
        if (index >= lines.length) {
          if (mockGameLogTimer) clearInterval(mockGameLogTimer);
          mockGameLogTimer = null;
          return;
        }
        const line = lines[index++];
        mockState.logs = [...mockState.logs, line];
        mockEventBus.emit("game-log", {
          line,
          stream: "stdout",
          timestamp: new Date().toISOString(),
        });
      }, 260);
      return result as T;
    }

    case "stop_game": {
      if (mockGameLogTimer) clearInterval(mockGameLogTimer);
      mockGameLogTimer = null;
      await delay(300);
      const report: CrashReport = {
        level: "info",
        kind: "stopped",
        exitCode: 0,
        title: "游戏已由启动器终止",
        summary: "进程已被手动结束，存档通常会保留最近一次自动保存。",
        suggestions: ["如果误触终止，请重新启动游戏"],
        logPath: mockGameStatus?.logPath ?? "",
        signalLine: null,
        playTimeSecs: 186,
      };
      mockState.gameStatus = null;
      mockGameStatus = null;
      mockState.lastCrash = report;
      mockEventBus.emit("game-exit", report);
      return undefined as T;
    }

    case "get_game_status":
      return mockState.gameStatus as T;

    case "get_game_logs":
      return [...mockState.logs].slice(-Number(args.limit ?? 400)) as T;

    case "read_latest_log":
      return {
        path: "C:\\Users\\Creeper\\AppData\\Roaming\\.minecraft\\.creeper\\logs\\latest.log",
        content:
          "[10:42:12] [Render thread/INFO]: Loaded 12869 recipes\n[10:42:14] [Render thread/INFO]: Time elapsed: 10.461 seconds\n[10:42:14] [Render thread/INFO]: Stopping!",
      } as T;

    case "delete_version":
      mockState.versions = mockState.versions.filter(
        (item) => item.id !== args.versionId,
      );
      return undefined as T;

    case "open_external":
      return undefined as T;

    case "open_path":
    case "reveal_latest_log":
      return "" as T;

    case "get_app_info":
      return {
        name: "CreeperCore",
        version: "0.1.0",
        identifier: "com.creepercore.launcher",
        debugBuild: true,
      } as T;

    default:
      throw new Error(`Mock backend has no handler for ${command}`);
  }
}
