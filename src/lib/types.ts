export type PageId =
  | "launch"
  | "versions"
  | "downloads"
  | "mods"
  | "backups"
  | "settings";

export interface AccountProfile {
  id: string;
  username: string;
  uuid: string;
  kind: "offline" | "msa" | string;
  createdAt: string;
  lastUsed?: string | null;
}

export interface MsaPollResponse {
  status: "pending" | "complete";
  config?: AppConfig | null;
  skinDataUrl?: string | null;
}

export interface ModrinthHit {
  projectId: string;
  slug: string;
  title: string;
  description: string;
  author: string;
  iconUrl?: string | null;
  downloads: number;
  follows: number;
  categories: string[];
  versions: string[];
  latestVersion?: string | null;
  dateModified?: string | null;
  clientSide?: string | null;
  serverSide?: string | null;
}

export interface ModrinthSearchPage {
  hits: ModrinthHit[];
  offset: number;
  limit: number;
  totalHits: number;
}

export interface ModrinthInstallReport {
  installed: string[];
  skipped: string[];
  warnings: string[];
}

export interface ModrinthVersionInfo {
  id: string;
  projectId: string;
  name: string;
  versionNumber: string;
  versionType: string;
  datePublished?: string | null;
  downloads: number;
  gameVersions: string[];
  loaders: string[];
  fileName?: string | null;
  fileSize: number;
  requiredDependencies: number;
  dependencyProjectIds?: string[];
}

export interface ModrinthProjectSummary {
  projectId: string;
  slug: string;
  title: string;
  iconUrl?: string | null;
}

export interface MsaCapeInfo {
  id: string;
  alias: string;
  active: boolean;
  dataUrl?: string | null;
}

export interface MsaProfileInfo {
  skinDataUrl?: string | null;
  skinVariant?: string | null;
  capes: MsaCapeInfo[];
  activeCapeId?: string | null;
}

export interface ModpackInstallResult {
  versionId: string;
  packName: string;
  minecraftVersion: string;
  loaderVersion: string;
  filesInstalled: number;
  filesSkipped: number;
  overrides: number;
  warnings: string[];
}

export interface VersionSettings {
  isolate?: boolean | null;
  memoryMb?: number | null;
  javaPath?: string | null;
  jvmArgs?: string | null;
}

export interface VersionSettingsUpdate {
  memoryMb: number | null;
  javaPath: string | null;
  jvmArgs: string | null;
  isolate: boolean | null;
}

export interface InstanceEntry {
  name: string;
  path: string;
  isDir: boolean;
  size: number;
  modified?: string | null;
}

export interface AppConfig {
  configVersion: number;
  gameDir: string;
  selectedVersion: string | null;
  accounts: AccountProfile[];
  activeAccountId: string | null;
  javaPath: string | null;
  memoryMb: number;
  jvmArgs: string;
  width: number;
  height: number;
  fullscreen: boolean;
  downloadSource: string;
  communitySource: string;
  downloadConcurrency: number;
  isolateVersions: boolean;
  versionSettings: Record<string, VersionSettings>;
  closeAfterLaunch: boolean;
  minimizeOnLaunch: boolean;
  backgroundAnimation: boolean;
  showLogOnCrash: boolean;
  autoBackupBeforeLaunch: boolean;
  lastPage: string;
}

export interface VersionSummary {
  id: string;
  displayName: string;
  versionType: string;
  loader: "vanilla" | "fabric" | "quilt" | "forge" | "neoforge" | "unknown" | string;
  loaderVersion: string | null;
  releaseTime: string | null;
  lastModified: number;
  javaMajor: number | null;
  hasJson: boolean;
  hasClient: boolean;
  mainClass: string | null;
  inheritsFrom: string | null;
  readError: string | null;
}

export interface JavaInfo {
  path: string;
  version: string;
  major: number;
  vendor: string;
  architecture: string;
  is64bit: boolean;
  source: string;
  recommended: boolean;
}

export interface SystemInfo {
  os: string;
  arch: string;
  cpuThreads: number;
  totalMemoryMb: number;
  usedMemoryMb: number;
  launcherVersion: string;
}

export interface DiagnosticItem {
  id: string;
  severity: "ok" | "info" | "warn" | "error" | string;
  title: string;
  detail: string;
  action: string;
  fixable: boolean;
  points: number;
}

export interface DiagnosticReport {
  versionId: string;
  score: number;
  level: "ready" | "attention" | "danger" | string;
  headline: string;
  items: DiagnosticItem[];
  checkedAt: string;
  javaPath: string | null;
  javaMajor: number | null;
  hasMissingFiles: boolean;
}

export interface ModInfo {
  fileName: string;
  path: string;
  name: string;
  modId: string | null;
  version: string | null;
  description: string | null;
  authors: string[];
  loader: string;
  enabled: boolean;
  size: number;
  modified: number;
  missingDependencies: string[];
  duplicate: boolean;
  readError: string | null;
}

export interface BackupInfo {
  id: string;
  versionId: string;
  name: string;
  note: string;
  createdAt: string;
  sizeBytes: number;
  fileCount: number;
  kind: "saves" | "full" | string;
  path: string;
  gameDir: string;
}

export interface RestoreReport {
  backup: BackupInfo;
  safetyBackup: BackupInfo | null;
  filesRestored: number;
  filesSkipped: number;
  message: string;
}

export interface GameStatus {
  pid: number;
  versionId: string;
  startedAt: string;
  logPath: string;
}

export interface ActiveInstallState {
  taskId: string;
  label: string;
}

export interface BootstrapData {
  config: AppConfig;
  versions: VersionSummary[];
  javas: JavaInfo[];
  selectedJava: JavaInfo | null;
  system: SystemInfo;
  diagnostics: DiagnosticReport | null;
  mods: ModInfo[];
  backups: BackupInfo[];
  gameStatus: GameStatus | null;
  activeInstall: ActiveInstallState | null;
  logs: string[];
  lastCrash: CrashReport | null;
  systemClock: string;
}

export interface RemoteVersion {
  id: string;
  type: string;
  url: string;
  time: string;
  releaseTime: string;
  sha1: string;
  complianceLevel: number;
}

export interface VersionManifest {
  latest: { release: string; snapshot: string };
  versions: RemoteVersion[];
}

export interface FabricLoaderVersion {
  version: string;
  stable: boolean;
}

export interface InstallProgress {
  taskId: string;
  phase: string;
  label: string;
  current: number;
  total: number;
  bytesDone: number;
  bytesTotal: number;
  percent: number;
  message: string;
}

export interface InstallReport {
  versionId: string;
  filesDownloaded: number;
  filesReused: number;
  bytesDownloaded: number;
  elapsedMs: number;
}

export interface InstallOutcome {
  versionId: string;
  loader: string;
  report: InstallReport;
  warnings: string[];
}

export interface LaunchGameResult {
  pid: number;
  versionId: string;
  startedAt: string;
  logPath: string;
  warnings: string[];
  javaPath: string;
  javaMajor: number | null;
  gameDir: string;
}

export interface CrashReport {
  level: "info" | "warn" | "error" | string;
  kind: string;
  exitCode: number | null;
  title: string;
  summary: string;
  suggestions: string[];
  logPath: string;
  signalLine: string | null;
  playTimeSecs: number;
}

export interface GameLogPayload {
  line: string;
  stream: string;
  timestamp: string;
}

export interface LogFilePayload {
  path: string;
  content: string;
}

export interface ModActionReport {
  changed: string[];
  skipped: string[];
  errors: string[];
}

export interface VersionDetail {
  id: string;
  displayName: string;
  mainClass: string;
  versionType: string;
  loader: string;
  loaderVersion: string | null;
  inheritsFrom: string | null;
  jarId: string;
  javaMajor: number | null;
  assetsId: string | null;
  librariesCount: number;
  librariesMissing: number;
  clientJarExists: boolean;
  assetIndexExists: boolean;
  assetsMissing: number;
  gameDir: string;
  isIsolated: boolean;
  modsCount: number;
  totalDownloadSize: number;
}

export interface JavaScanResult {
  javas: JavaInfo[];
  recommended: JavaInfo | null;
  requiredMajor: number | null;
}

export interface SettingsUpdate {
  gameDir: string;
  javaPath: string | null;
  memoryMb: number;
  jvmArgs: string;
  width: number;
  height: number;
  fullscreen: boolean;
  downloadSource: string;
  communitySource: string;
  downloadConcurrency: number;
  isolateVersions: boolean;
  backgroundAnimation: boolean;
  minimizeOnLaunch: boolean;
  closeAfterLaunch: boolean;
  autoBackupBeforeLaunch: boolean;
  showLogOnCrash: boolean;
}

export interface InstallStartPayload {
  taskId: string;
  versionId: string;
  loader: string;
}

export interface GameStartPayload {
  pid: number;
  versionId: string;
  startedAt: string;
  logPath: string;
}

export interface CancelInstallResult {
  cancelled: boolean;
}

export interface AppInfo {
  name: string;
  version: string;
  identifier: string;
  debugBuild: boolean;
}
