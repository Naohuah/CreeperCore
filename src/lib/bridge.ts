import type {
  AppConfig,
  AppInfo,
  BackupInfo,
  BootstrapData,
  CancelInstallResult,
  CrashReport,
  DiagnosticReport,
  InstallOutcome,
  InstallProgress,
  InstallReport,
  InstanceEntry,
  JavaScanResult,
  LaunchGameResult,
  LogFilePayload,
  ModActionReport,
  ModInfo,
  ModpackInstallResult,
  ModrinthHit,
  MsaPollResponse,
  MsaProfileInfo,
  ModrinthInstallReport,
  ModrinthSearchPage,
  ModrinthProjectSummary,
  ModrinthVersionInfo,
  RemoteVersion,
  RestoreReport,
  SettingsUpdate,
  VersionDetail,
  VersionSettingsUpdate,
  VersionManifest,
  VersionSummary,
} from "./types";
import {
  mockEventBus,
  mockInvoke,
  mockIsTauriFilePickerAvailable,
} from "./mock";

export const isTauri = (): boolean =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

type UnlistenFn = () => void;

export async function invokeCommand<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (isTauri()) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<T>(command, args);
  }
  return mockInvoke<T>(command, args);
}

export async function listenEvent<T>(
  eventName: string,
  handler: (payload: T) => void,
): Promise<UnlistenFn> {
  if (isTauri()) {
    const { listen } = await import("@tauri-apps/api/event");
    return listen<T>(eventName, (event) => handler(event.payload));
  }
  return mockEventBus.listen(eventName, handler as (payload: unknown) => void);
}

export async function pickModFiles(): Promise<string[]> {
  if (!isTauri()) {
    return mockIsTauriFilePickerAvailable();
  }
  const { open } = await import("@tauri-apps/plugin-dialog");
  const selected = await open({
    multiple: true,
    directory: false,
    title: "选择要安装的模组",
    filters: [{ name: "Minecraft Mod", extensions: ["jar"] }],
  });
  if (!selected) return [];
  return Array.isArray(selected) ? selected : [selected];
}

export async function pickDirectory(
  title = "选择目录",
  defaultPath?: string,
): Promise<string | null> {
  if (!isTauri()) return null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const selected = await open({
    multiple: false,
    directory: true,
    title,
    defaultPath,
  });
  return typeof selected === "string" ? selected : null;
}

export async function pickJavaFile(defaultPath?: string): Promise<string | null> {
  if (!isTauri()) return null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const selected = await open({
    multiple: false,
    directory: false,
    title: "选择 java.exe",
    defaultPath,
    filters: [{ name: "Java runtime", extensions: ["exe", "bin"] }],
  });
  return typeof selected === "string" ? selected : null;
}

export async function minimizeWindow(): Promise<void> {
  if (!isTauri()) return;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow().minimize();
}

export async function toggleMaximizeWindow(): Promise<void> {
  if (!isTauri()) return;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow().toggleMaximize();
}

export async function closeWindow(): Promise<void> {
  if (!isTauri()) return;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow().close();
}

// Typed API surface used by the UI. Keeping command names in one place makes
// the Rust/TS boundary auditable.
export const api = {
  bootstrap: () => invokeCommand<BootstrapData>("get_bootstrap"),
  selectVersion: (versionId: string) =>
    invokeCommand<void>("select_version", { versionId }),
  getVersionDetail: (versionId: string) =>
    invokeCommand<VersionDetail>("get_version_detail", { versionId }),
  scanJava: () => invokeCommand<JavaScanResult>("scan_java"),
  saveSettings: (update: SettingsUpdate) =>
    invokeCommand<AppConfig>("save_settings", { update }),
  setVersionIsolation: (versionId: string, isolate: boolean | null) =>
    invokeCommand<AppConfig>("set_version_isolation", { versionId, isolate }),
  saveVersionSettings: (versionId: string, update: VersionSettingsUpdate) =>
    invokeCommand<AppConfig>("save_version_settings", { versionId, update }),
  listInstanceEntries: (versionId: string, kind: string) =>
    invokeCommand<InstanceEntry[]>("list_instance_entries", {
      versionId,
      kind,
    }),
  createAccount: (username: string) =>
    invokeCommand<AppConfig>("create_account", { username }),
  saveLocalSkin: (accountId: string, dataUrl: string) =>
    invokeCommand<void>("save_local_skin", { accountId, dataUrl }),
  deleteLocalSkin: (accountId: string) =>
    invokeCommand<void>("delete_local_skin", { accountId }),
  msaEmbeddedLogin: () => invokeCommand<MsaPollResponse>("msa_embedded_login"),
  msaProfile: (accountId: string) =>
    invokeCommand<MsaProfileInfo>("msa_profile", { accountId }),
  msaUploadSkin: (accountId: string, dataUrl: string, variant: string) =>
    invokeCommand<MsaProfileInfo>("msa_upload_skin", {
      accountId,
      dataUrl,
      variant,
    }),
  msaDeleteSkin: (accountId: string) =>
    invokeCommand<MsaProfileInfo>("msa_delete_skin", { accountId }),
  msaSetCape: (accountId: string, capeId: string | null) =>
    invokeCommand<MsaProfileInfo>("msa_set_cape", { accountId, capeId }),
  msaRefreshAccount: (accountId: string) =>
    invokeCommand<MsaPollResponse>("msa_refresh_account", { accountId }),
  selectAccount: (accountId: string) =>
    invokeCommand<AppConfig>("select_account", { accountId }),
  deleteAccount: (accountId: string) =>
    invokeCommand<AppConfig>("delete_account", { accountId }),
  fetchRemoteVersions: () =>
    invokeCommand<VersionManifest>("fetch_remote_versions"),
  installGame: (
    versionId: string,
    loader: "vanilla" | "fabric" | "forge" | "neoforge",
    includeAssets = true,
  ) =>
    invokeCommand<InstallOutcome>("install_game", {
      versionId,
      loader,
      includeAssets,
    }),
  cancelInstall: () => invokeCommand<CancelInstallResult>("cancel_install"),
  cancelModrinthInstall: () => invokeCommand<void>("cancel_modrinth_install"),
  repairGame: (versionId: string) =>
    invokeCommand<InstallReport>("repair_game", { versionId }),
  diagnoseGame: (versionId: string) =>
    invokeCommand<DiagnosticReport>("diagnose_game", { versionId }),
  searchModrinth: (versionId: string, query: string, offset = 0, limit = 20) =>
    invokeCommand<ModrinthSearchPage>("search_modrinth", {
      versionId,
      query,
      offset,
      limit,
    }),
  translateModrinthSummaries: (projectIds: string[]) =>
    invokeCommand<Record<string, string>>("translate_modrinth_summaries", { projectIds }),
  listModrinthVersions: (versionId: string, projectId: string) =>
    invokeCommand<ModrinthVersionInfo[]>("list_modrinth_versions", {
      versionId,
      projectId,
    }),
  listModrinthProjects: (projectIds: string[]) =>
    invokeCommand<ModrinthProjectSummary[]>("list_modrinth_projects", {
      projectIds,
    }),
  installModrinthVersion: (versionId: string, modVersionId: string) =>
    invokeCommand<ModrinthInstallReport>("install_modrinth_version", {
      versionId,
      modVersionId,
    }),
  installModrinth: (versionId: string, projectId: string) =>
    invokeCommand<ModrinthInstallReport>("install_modrinth", {
      versionId,
      projectId,
    }),
  searchModrinthContent: (
    versionId: string,
    projectType: "resourcepack" | "shader" | "datapack",
    query: string,
    offset = 0,
    limit = 20,
  ) =>
    invokeCommand<ModrinthSearchPage>("search_modrinth_content", {
      versionId,
      projectType,
      query,
      offset,
      limit,
    }),
  installModrinthContent: (
    versionId: string,
    projectId: string,
    projectType: "resourcepack" | "shader" | "datapack",
  ) =>
    invokeCommand<ModrinthInstallReport>("install_modrinth_content", {
      versionId,
      projectId,
      projectType,
    }),
  searchModrinthModpacks: (versionId: string, query: string, offset = 0, limit = 20) =>
    invokeCommand<ModrinthSearchPage>("search_modrinth_modpacks", {
      versionId,
      query,
      offset,
      limit,
    }),
  installModrinthModpack: (versionId: string, projectId: string) =>
    invokeCommand<ModpackInstallResult>("install_modrinth_modpack", {
      versionId,
      projectId,
    }),
  listMods: (versionId: string) =>
    invokeCommand<ModInfo[]>("list_mods", { versionId }),
  toggleMod: (versionId: string, fileName: string, enabled: boolean) =>
    invokeCommand<string>("toggle_mod", { versionId, fileName, enabled }),
  deleteMod: (versionId: string, fileName: string) =>
    invokeCommand<string>("delete_mod", { versionId, fileName }),
  importMods: (versionId: string, paths: string[]) =>
    invokeCommand<ModActionReport>("import_mods", { versionId, paths }),
  listBackups: (versionId: string) =>
    invokeCommand<BackupInfo[]>("list_backups", { versionId }),
  createBackup: (
    versionId: string,
    name: string,
    note: string,
    includeMods: boolean,
  ) =>
    invokeCommand<BackupInfo>("create_backup", {
      versionId,
      name,
      note,
      includeMods,
    }),
  restoreBackup: (versionId: string, backupId: string) =>
    invokeCommand<RestoreReport>("restore_backup", { versionId, backupId }),
  deleteBackup: (versionId: string, backupId: string) =>
    invokeCommand<void>("delete_backup", { versionId, backupId }),
  launchGame: (versionId: string) =>
    invokeCommand<LaunchGameResult>("launch_game", { versionId }),
  stopGame: () => invokeCommand<void>("stop_game"),
  getGameStatus: () =>
    invokeCommand<{ pid: number; versionId: string } | null>("get_game_status"),
  getGameLogs: (limit = 400) =>
    invokeCommand<string[]>("get_game_logs", { limit }),
  readLatestLog: (versionId: string) =>
    invokeCommand<LogFilePayload>("read_latest_log", { versionId }),
  deleteVersion: (versionId: string) =>
    invokeCommand<void>("delete_version", { versionId }),
  openExternal: (url: string) => invokeCommand<void>("open_external", { url }),
  openPath: (target: string, versionId?: string | null) =>
    invokeCommand<void>("open_path", { target, versionId: versionId ?? null }),
  revealLatestLog: () => invokeCommand<string>("reveal_latest_log"),
  appInfo: () => invokeCommand<AppInfo>("get_app_info"),
};

export type {
  BackupInfo,
  BootstrapData,
  CrashReport,
  InstallProgress,
  ModpackInstallResult,
  ModrinthHit,
  MsaPollResponse,
  MsaProfileInfo,
  ModrinthInstallReport,
  ModrinthSearchPage,
  ModrinthProjectSummary,
  ModrinthVersionInfo,
  RemoteVersion,
  VersionManifest,
  VersionSummary,
};
