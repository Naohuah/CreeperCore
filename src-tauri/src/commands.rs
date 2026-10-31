use crate::state::{ActiveInstallState, AppState, GameSession, GameStatus};
use anyhow::{Context, Result};
use creeper_core::model::*;
use creeper_core::{
    auth, backups, diagnose, download, java, launch, loader_install, modrinth, mods, msa,
    offline_skin, system, version,
};
use futures_util::{stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{
    AppHandle, Emitter, Manager, State, Url, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

type CmdResult<T> = std::result::Result<T, String>;

static MODRINTH_CANCEL: AtomicBool = AtomicBool::new(false);

async fn wait_modrinth_cancel() {
    loop {
        if MODRINTH_CANCEL.load(Ordering::SeqCst) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(120)).await;
    }
}

async fn run_modrinth_task<F, T>(future: F) -> CmdResult<T>
where
    F: std::future::Future<Output = Result<T>>,
{
    MODRINTH_CANCEL.store(false, Ordering::SeqCst);
    let result = tokio::select! {
        result = future => result.map_err(|err| err.to_string()),
        _ = wait_modrinth_cancel() => Err("下载已取消".to_string()),
    };
    MODRINTH_CANCEL.store(false, Ordering::SeqCst);
    result
}

#[tauri::command]
pub fn cancel_modrinth_install() {
    MODRINTH_CANCEL.store(true, Ordering::SeqCst);
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapData {
    pub config: creeper_core::config::AppConfig,
    pub versions: Vec<VersionSummary>,
    pub javas: Vec<JavaInfo>,
    pub selected_java: Option<JavaInfo>,
    pub system: SystemInfo,
    pub diagnostics: Option<DiagnosticReport>,
    pub mods: Vec<ModInfo>,
    pub backups: Vec<BackupInfo>,
    pub game_status: Option<GameStatus>,
    pub active_install: Option<ActiveInstallState>,
    pub logs: Vec<String>,
    pub last_crash: Option<CrashReport>,
    pub system_clock: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaScanResult {
    pub javas: Vec<JavaInfo>,
    pub recommended: Option<JavaInfo>,
    pub required_major: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallOutcome {
    pub version_id: String,
    pub loader: String,
    pub report: InstallReport,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchGameResult {
    pub pid: u32,
    pub version_id: String,
    pub started_at: String,
    pub log_path: String,
    pub warnings: Vec<String>,
    pub java_path: String,
    pub java_major: Option<u32>,
    pub game_dir: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameLogPayload {
    pub line: String,
    pub stream: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallStartPayload {
    pub task_id: String,
    pub version_id: String,
    pub loader: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameStartPayload {
    pub pid: u32,
    pub version_id: String,
    pub started_at: String,
    pub log_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameReadyPayload {
    pub pid: u32,
    pub version_id: String,
}

#[cfg(windows)]
fn game_window_visible(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{HWND, LPARAM, TRUE};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextLengthW, GetWindowThreadProcessId, IsWindowVisible,
    };

    struct Search {
        pid: u32,
        found: bool,
    }

    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> i32 {
        let search = &mut *(lparam as *mut Search);
        let mut owner = 0u32;
        GetWindowThreadProcessId(hwnd, &mut owner);
        if owner == search.pid
            && IsWindowVisible(hwnd) != 0
            && GetWindowTextLengthW(hwnd) > 0
        {
            search.found = true;
            return 0;
        }
        TRUE
    }

    let mut search = Search {
        pid,
        found: false,
    };
    unsafe {
        EnumWindows(Some(visit), &mut search as *mut Search as LPARAM);
    }
    search.found
}

#[cfg(not(windows))]
fn game_window_visible(_pid: u32) -> bool {
    false
}

async fn wait_for_game_window(app: AppHandle, pid: u32, version_id: String) {
    for _ in 0..480 {
        if game_window_visible(pid) {
            let _ = app.emit(
                "game-ready",
                &GameReadyPayload {
                    pid,
                    version_id,
                },
            );
            return;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogFilePayload {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsUpdate {
    pub game_dir: String,
    pub java_path: Option<String>,
    pub memory_mb: u32,
    pub jvm_args: String,
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
    pub download_source: String,
    pub community_source: String,
    pub download_concurrency: usize,
    pub isolate_versions: bool,
    pub background_animation: bool,
    pub minimize_on_launch: bool,
    pub close_after_launch: bool,
    pub auto_backup_before_launch: bool,
    pub show_log_on_crash: bool,
}

fn game_dir_for(config: &creeper_core::config::AppConfig, version_id: &str) -> PathBuf {
    config.game_dir_for(version_id)
}

fn selected_version(
    config: &creeper_core::config::AppConfig,
    versions: &[VersionSummary],
) -> Option<String> {
    if let Some(id) = &config.selected_version {
        if versions.iter().any(|item| &item.id == id) {
            return Some(id.clone());
        }
    }
    versions
        .iter()
        .find(|item| item.has_client)
        .or_else(|| versions.first())
        .map(|item| item.id.clone())
}

fn java_required(root: &Path, version_id: Option<&str>) -> Option<u32> {
    version_id.and_then(|id| {
        version::resolve_version(root, id)
            .ok()
            .and_then(|resolved| resolved.java_major)
    })
}

fn choose_java(
    config: &creeper_core::config::AppConfig,
    versions: &[JavaInfo],
    version_id: Option<&str>,
    required: Option<u32>,
) -> Option<JavaInfo> {
    if let Some(version_id) = version_id {
        if let Some(configured) = config.java_for(version_id) {
            let key = creeper_core::util::normalize_path_key(Path::new(&configured));
            if let Some(found) = versions
                .iter()
                .find(|item| creeper_core::util::normalize_path_key(Path::new(&item.path)) == key)
            {
                return Some(found.clone());
            }
        }
    } else if let Some(configured) = &config.java_path {
        let key = creeper_core::util::normalize_path_key(Path::new(configured));
        if let Some(found) = versions
            .iter()
            .find(|item| creeper_core::util::normalize_path_key(Path::new(&item.path)) == key)
        {
            return Some(found.clone());
        }
    }
    java::select_best(versions, required)
}

fn mark_recommended(javas: &mut [JavaInfo], selected: &Option<JavaInfo>) {
    if let Some(selected) = selected {
        for item in javas {
            item.recommended = item.path.eq_ignore_ascii_case(&selected.path);
        }
    }
}

#[tauri::command]
pub async fn get_bootstrap(app: AppHandle, state: State<'_, AppState>) -> CmdResult<BootstrapData> {
    let mut config = state.config();
    let root = config.root();
    if let Err(err) = creeper_core::ensure_game_layout(&root) {
        return Err(format!("无法初始化游戏目录：{err}"));
    }
    let root_for_versions = root.clone();
    let root_for_java = root.clone();
    let configured_java = config.java_path.clone();
    let versions_task =
        tauri::async_runtime::spawn_blocking(move || version::scan_versions(&root_for_versions));
    let java_task = tauri::async_runtime::spawn_blocking(move || {
        java::discover_java(&root_for_java, configured_java.as_deref())
    });
    let (versions_result, javas_result) = tokio::join!(versions_task, java_task);
    let versions = versions_result
        .map_err(|err| format!("版本扫描失败：{err}"))?
        .map_err(|err| err.to_string())?;
    let mut javas = javas_result.map_err(|err| format!("Java 扫描失败：{err}"))?;

    let selected = selected_version(&config, &versions);
    let required = java_required(&root, selected.as_deref());
    let mut selected_java = choose_java(&config, &javas, selected.as_deref(), required);
    mark_recommended(&mut javas, &selected_java);
    if let Some(java) = &selected_java {
        if config.java_path.is_none() {
            config.java_path = Some(java.path.clone());
        } else if config
            .java_path
            .as_deref()
            .map(|path| creeper_core::util::normalize_path_key(Path::new(path)))
            .map(|key| {
                javas.iter().all(|candidate| {
                    creeper_core::util::normalize_path_key(Path::new(&candidate.path)) != key
                })
            })
            .unwrap_or(false)
        {
            // The configured JVM disappeared; fall back to a freshly discovered one.
            config.java_path = Some(java.path.clone());
        }
        if let Some(selected) = &selected {
            if let Some(override_path) = config
                .version_settings
                .get(selected)
                .and_then(|settings| settings.java_path.clone())
            {
                let key = creeper_core::util::normalize_path_key(Path::new(&override_path));
                let valid = javas.iter().any(|candidate| {
                    creeper_core::util::normalize_path_key(Path::new(&candidate.path)) == key
                });
                if !valid {
                    if let Some(settings) = config.version_settings.get_mut(selected) {
                        settings.java_path = None;
                    }
                }
            }
        }
    }
    config.selected_version = selected.clone();
    let _ = state.save_config(&config);

    let account = config.active_account().cloned();
    let version_for_diag = selected.clone();
    let config_for_diag = config.clone();
    let selected_java_for_diag = selected_java.clone();
    let root_for_diag = root.clone();
    let (diagnostics, mods_list, backup_list) = if let Some(version_id) = version_for_diag {
        let game_dir = game_dir_for(&config_for_diag, &version_id);
        let diag_root = root_for_diag.clone();
        let diag_game = game_dir.clone();
        let java_for_diag = selected_java_for_diag.clone();
        let account_for_diag = account.clone();
        let memory = config_for_diag.memory_for(&version_id);
        let diag_task = tauri::async_runtime::spawn_blocking(move || {
            diagnose::diagnose(diagnose::DiagnoseInput {
                root: &diag_root,
                version_id: &version_id,
                game_dir: &diag_game,
                java: java_for_diag.as_ref(),
                memory_mb: memory,
                account: account_for_diag.as_ref(),
            })
        });
        let mods_game = game_dir.clone();
        let mods_task = tauri::async_runtime::spawn_blocking(move || mods::list_mods(&mods_game));
        let backups_root = root_for_diag.clone();
        let backups_version = config.selected_version.clone().unwrap_or_default();
        let backups_task = tauri::async_runtime::spawn_blocking(move || {
            backups::list_backups(&backups_root, &backups_version)
        });
        let (diag, mods_result, backup_result) = tokio::join!(diag_task, mods_task, backups_task);
        (
            diag.map_err(|err| err.to_string())
                .ok()
                .and_then(|r| r.ok()),
            mods_result
                .map_err(|err| err.to_string())
                .ok()
                .and_then(|r| r.ok())
                .unwrap_or_default(),
            backup_result
                .map_err(|err| err.to_string())
                .ok()
                .and_then(|r| r.ok())
                .unwrap_or_default(),
        )
    } else {
        (None, Vec::new(), Vec::new())
    };

    let active_install = state.async_install_state().await;
    let game_status = state.game_status();
    let logs = state.log_tail(400);
    let last_crash = state.last_crash();
    if let Some(java) = &mut selected_java {
        java.recommended = true;
    }

    let _ = app.emit("bootstrap-ready", true);
    Ok(BootstrapData {
        config,
        versions,
        javas,
        selected_java,
        system: system::system_info(),
        diagnostics,
        mods: mods_list,
        backups: backup_list,
        game_status,
        active_install,
        logs,
        last_crash,
        system_clock: chrono::Local::now().to_rfc3339(),
    })
}

#[tauri::command]
pub async fn select_version(state: State<'_, AppState>, version_id: String) -> CmdResult<()> {
    if !version::version_json_path(&state.config().root(), &version_id).exists() {
        return Err("版本不存在".to_string());
    }
    state
        .mutate_config(|config| {
            config.selected_version = Some(version_id);
        })
        .map_err(|err| err.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn get_version_detail(
    state: State<'_, AppState>,
    version_id: String,
) -> CmdResult<VersionDetail> {
    let config = state.config();
    let root = config.root();
    tauri::async_runtime::spawn_blocking(move || {
        version_detail_blocking(&config, &root, &version_id)
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(|err| err.to_string())
}

fn version_detail_blocking(
    config: &creeper_core::config::AppConfig,
    root: &Path,
    version_id: &str,
) -> Result<VersionDetail> {
    let resolved = version::resolve_version(root, version_id)?;
    let platform = version::Platform::current();
    let mut libraries_missing = 0usize;
    for library in &resolved.libraries {
        for file in version::library_files(library, &platform) {
            if !version::libraries_dir(root).join(&file.path).exists() {
                libraries_missing += 1;
            }
        }
    }
    let client_jar_exists = version::effective_client_jar(root, &resolved).exists();
    let (asset_index_exists, assets_missing) = asset_state(root, &resolved);
    let game_dir = config.game_dir_for(version_id);
    let (mods_count, _) = mods::quick_mod_summary(&game_dir);
    let summary = version::scan_versions(root)?
        .into_iter()
        .find(|item| item.id == version_id);
    Ok(VersionDetail {
        id: resolved.id.clone(),
        display_name: resolved.id.clone(),
        main_class: resolved.main_class.clone(),
        version_type: resolved.version_type.clone(),
        loader: summary
            .as_ref()
            .map(|item| item.loader.clone())
            .unwrap_or_else(|| "vanilla".to_string()),
        loader_version: summary.and_then(|item| item.loader_version),
        inherits_from: resolved.inherits_from.clone(),
        jar_id: resolved.jar_id.clone(),
        java_major: resolved.java_major,
        assets_id: resolved.assets_id.clone(),
        libraries_count: resolved.libraries.len(),
        libraries_missing,
        client_jar_exists,
        asset_index_exists,
        assets_missing,
        game_dir: game_dir.to_string_lossy().to_string(),
        is_isolated: config.isolation_for(version_id),
        mods_count,
        total_download_size: resolved.total_download_size,
    })
}

fn asset_state(root: &Path, resolved: &ResolvedVersion) -> (bool, usize) {
    #[derive(Deserialize, Default)]
    struct ProbeIndex {
        #[serde(default)]
        objects: std::collections::HashMap<String, ProbeObject>,
    }
    #[derive(Deserialize, Default)]
    struct ProbeObject {
        hash: String,
        size: u64,
    }
    let Some(index_ref) = &resolved.asset_index else {
        return (false, 0);
    };
    let path = download::assets_index_path(root, &index_ref.id);
    if !path.exists() {
        return (false, 0);
    }
    let Ok(raw) = std::fs::read(path) else {
        return (true, 0);
    };
    let Ok(index) = serde_json::from_slice::<ProbeIndex>(&raw) else {
        return (true, 0);
    };
    let mut missing = 0usize;
    for object in index.objects.values() {
        if object.hash.len() < 2 {
            continue;
        }
        let object_path = root
            .join("assets")
            .join("objects")
            .join(&object.hash[..2])
            .join(&object.hash);
        if !std::fs::metadata(&object_path)
            .map(|meta| meta.is_file() && meta.len() == object.size)
            .unwrap_or(false)
        {
            missing += 1;
        }
    }
    (true, missing)
}

#[tauri::command]
pub async fn scan_java(state: State<'_, AppState>) -> CmdResult<JavaScanResult> {
    let config = state.config();
    let root = config.root();
    let selected = config.selected_version.clone();
    let required = java_required(&root, selected.as_deref());
    let configured = config.java_for(selected.as_deref().unwrap_or_default());
    let mut javas = tauri::async_runtime::spawn_blocking(move || {
        java::discover_java(&root, configured.as_deref())
    })
    .await
    .map_err(|err| err.to_string())?;
    let recommended = java::select_best(&javas, required);
    mark_recommended(&mut javas, &recommended);
    Ok(JavaScanResult {
        javas,
        recommended,
        required_major: required,
    })
}

#[tauri::command]
pub async fn save_settings(
    state: State<'_, AppState>,
    update: SettingsUpdate,
) -> CmdResult<creeper_core::config::AppConfig> {
    let new_root = PathBuf::from(update.game_dir.trim());
    if update.game_dir.trim().is_empty() {
        return Err("游戏目录不能为空".to_string());
    }
    std::fs::create_dir_all(&new_root).map_err(|err| format!("无法创建游戏目录：{err}"))?;
    creeper_core::ensure_game_layout(&new_root).map_err(|err| err.to_string())?;
    let java_path = update
        .java_path
        .map(|path| path.trim().to_string())
        .filter(|path| !path.is_empty());
    if let Some(path) = &java_path {
        if !Path::new(path).is_file() {
            return Err(format!("Java 路径不存在：{path}"));
        }
    }
    if !matches!(update.download_source.as_str(), "official" | "bmclapi") {
        return Err("未知下载源".to_string());
    }
    if !matches!(update.community_source.as_str(), "official" | "mcim") {
        return Err("未知社区资源来源".to_string());
    }
    let new_config = state
        .mutate_config(|config| {
            let old_root = config.root();
            let root_changed = creeper_core::util::normalize_path_key(&old_root)
                != creeper_core::util::normalize_path_key(&new_root);
            config.game_dir = new_root.to_string_lossy().to_string();
            if root_changed {
                config.selected_version = None;
                config.version_settings.clear();
            }
            config.java_path = java_path;
            config.memory_mb = update.memory_mb.clamp(MIN_MEMORY_MB, MAX_MEMORY_MB);
            config.jvm_args = update.jvm_args;
            config.width = update.width.max(320);
            config.height = update.height.max(240);
            config.fullscreen = update.fullscreen;
            config.download_source = update.download_source;
            config.community_source = update.community_source;
            config.download_concurrency = update.download_concurrency.clamp(1, 64);
            config.isolate_versions = update.isolate_versions;
            config.background_animation = update.background_animation;
            config.minimize_on_launch = update.minimize_on_launch;
            config.close_after_launch = update.close_after_launch;
            config.auto_backup_before_launch = update.auto_backup_before_launch;
            config.show_log_on_crash = update.show_log_on_crash;
            config.clone()
        })
        .map_err(|err| err.to_string())?;
    Ok(new_config)
}

#[tauri::command]
pub async fn set_version_isolation(
    state: State<'_, AppState>,
    version_id: String,
    isolate: Option<bool>,
) -> CmdResult<creeper_core::config::AppConfig> {
    state
        .mutate_config(|config| {
            let entry = config.version_settings.entry(version_id).or_default();
            entry.isolate = isolate;
            config.clone()
        })
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn create_account(
    state: State<'_, AppState>,
    username: String,
) -> CmdResult<creeper_core::config::AppConfig> {
    let account = auth::make_offline_account(username.trim()).map_err(|err| err.to_string())?;
    let mut config = state.config();
    if config
        .accounts
        .iter()
        .any(|item| item.username.eq_ignore_ascii_case(&account.username))
    {
        return Err("已存在同名离线档案".to_string());
    }
    config.accounts.push(account.clone());
    config.active_account_id = Some(account.id.clone());
    state.save_config(&config).map_err(|err| err.to_string())?;
    Ok(config)
}

#[tauri::command]
pub async fn select_account(
    state: State<'_, AppState>,
    account_id: String,
) -> CmdResult<creeper_core::config::AppConfig> {
    let mut config = state.config();
    if !config.accounts.iter().any(|item| item.id == account_id) {
        return Err("档案不存在".to_string());
    }
    config.active_account_id = Some(account_id.clone());
    config.touch_account(&account_id);
    state.save_config(&config).map_err(|err| err.to_string())?;
    Ok(config)
}

#[tauri::command]
pub async fn delete_account(
    state: State<'_, AppState>,
    account_id: String,
) -> CmdResult<creeper_core::config::AppConfig> {
    let mut config = state.config();
    if config.accounts.len() <= 1 {
        return Err("至少保留一个玩家档案".to_string());
    }
    let before = config.accounts.len();
    config.accounts.retain(|item| item.id != account_id);
    if config.accounts.len() == before {
        return Err("档案不存在".to_string());
    }
    if config.active_account_id.as_deref() == Some(account_id.as_str()) {
        config.active_account_id = config.accounts.first().map(|item| item.id.clone());
    }
    state.save_config(&config).map_err(|err| err.to_string())?;
    Ok(config)
}

fn pick_installer_java(
    javas: &[creeper_core::model::JavaInfo],
) -> Option<std::path::PathBuf> {
    let modern = |major: u32| {
        javas
            .iter()
            .find(|java| java.major == major)
            .map(|java| std::path::PathBuf::from(&java.path))
    };
    modern(21)
        .or_else(|| modern(17))
        .or_else(|| {
            javas
                .iter()
                .filter(|java| java.major >= 17)
                .max_by_key(|java| java.major)
                .map(|java| std::path::PathBuf::from(&java.path))
        })
        .or_else(|| {
            javas
                .iter()
                .max_by_key(|java| java.major)
                .map(|java| std::path::PathBuf::from(&java.path))
        })
}

fn community_source(config: &creeper_core::config::AppConfig) -> modrinth::ContentSource {
    modrinth::ContentSource::parse(&config.community_source)
}

fn content_target(
    config: &creeper_core::config::AppConfig,
    version_id: &str,
) -> CmdResult<(std::path::PathBuf, String)> {
    let root = config.root();
    let summary = version::scan_versions(&root)
        .map_err(|err| format!("无法扫描版本：{err}"))?
        .into_iter()
        .find(|item| item.id == version_id)
        .ok_or_else(|| "目标版本不存在".to_string())?;
    let game_version = summary
        .inherits_from
        .clone()
        .unwrap_or_else(|| summary.id.clone());
    Ok((config.game_dir_for(version_id), game_version))
}

fn content_folder(game_dir: &std::path::Path, project_type: &str) -> CmdResult<std::path::PathBuf> {
    match project_type {
        "resourcepack" => Ok(game_dir.join("resourcepacks")),
        "shader" => Ok(game_dir.join("shaderpacks")),
        "datapack" => Ok(game_dir.join("datapacks")),
        _ => Err("不支持的内容类型".to_string()),
    }
}

fn modrinth_target(
    config: &creeper_core::config::AppConfig,
    version_id: &str,
) -> CmdResult<(std::path::PathBuf, String, String)> {
    let root = config.root();
    let summary = version::scan_versions(&root)
        .map_err(|err| format!("无法扫描版本：{err}"))?
        .into_iter()
        .find(|item| item.id == version_id)
        .ok_or_else(|| "目标版本不存在".to_string())?;
    modrinth::loader_category(&summary.loader).map_err(|err| err.to_string())?;
    let game_version = summary
        .inherits_from
        .clone()
        .unwrap_or_else(|| summary.id.clone());
    let mods_dir = modrinth::mods_directory(&config.game_dir_for(version_id));
    Ok((mods_dir, game_version, summary.loader))
}

#[tauri::command]
pub async fn search_modrinth(
    state: State<'_, AppState>,
    version_id: String,
    query: String,
    offset: Option<u64>,
    limit: Option<u64>,
) -> CmdResult<modrinth::ModrinthSearchPage> {
    let config = state.config();
    let (_, game_version, loader) = modrinth_target(&config, &version_id)?;
    modrinth::search(
        &state.http,
        community_source(&config),
        &query,
        &game_version,
        &loader,
        offset.unwrap_or(0),
        limit.unwrap_or(20).clamp(1, 30),
    )
    .await
    .map_err(|err| err.to_string())
}

static MODRINTH_TRANSLATIONS: std::sync::OnceLock<std::sync::Mutex<HashMap<String, String>>> =
    std::sync::OnceLock::new();

fn translation_cache() -> &'static std::sync::Mutex<HashMap<String, String>> {
    MODRINTH_TRANSLATIONS.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

#[derive(Debug, Deserialize)]
struct McimTranslation {
    #[serde(default)]
    translated: Option<String>,
}

#[tauri::command]
pub async fn translate_modrinth_summaries(
    state: State<'_, AppState>,
    project_ids: Vec<String>,
) -> CmdResult<HashMap<String, String>> {
    let ids: Vec<String> = project_ids
        .into_iter()
        .filter(|id| {
            !id.is_empty()
                && id.len() <= 128
                && id
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
        })
        .take(40)
        .collect();
    let client = state.http.clone();
    let results: Vec<(String, Option<String>)> = stream::iter(ids.into_iter().map(|project_id| {
        let client = client.clone();
        async move {
            if let Some(value) = translation_cache()
                .lock()
                .ok()
                .and_then(|cache| cache.get(&project_id).cloned())
            {
                return (project_id, Some(value));
            }
            let response = client
                .get("https://mod.mcimirror.top/translate/modrinth")
                .query(&[("project_id", project_id.as_str())])
                .timeout(std::time::Duration::from_secs(8))
                .send()
                .await;
            let translated = match response {
                Ok(response) if response.status().is_success() => {
                    match response.json::<McimTranslation>().await {
                        Ok(value) => value.translated.filter(|text| !text.trim().is_empty()),
                        Err(_) => None,
                    }
                }
                _ => None,
            };
            if let Some(text) = &translated {
                if let Ok(mut cache) = translation_cache().lock() {
                    cache.insert(project_id.clone(), text.clone());
                }
            }
            (project_id, translated)
        }
    }))
    .buffer_unordered(6)
    .collect()
    .await;
    Ok(results
        .into_iter()
        .filter_map(|(id, text)| text.map(|text| (id, text)))
        .collect())
}

#[tauri::command]
pub async fn search_modrinth_content(
    state: State<'_, AppState>,
    version_id: String,
    project_type: String,
    query: String,
    offset: Option<u64>,
    limit: Option<u64>,
) -> CmdResult<modrinth::ModrinthSearchPage> {
    if !matches!(project_type.as_str(), "resourcepack" | "shader" | "datapack") {
        return Err("不支持的内容类型".to_string());
    }
    let config = state.config();
    let (_, game_version) = content_target(&config, &version_id)?;
    modrinth::search_content(
        &state.http,
        community_source(&config),
        &project_type,
        &query,
        &game_version,
        offset.unwrap_or(0),
        limit.unwrap_or(20).clamp(1, 30),
    )
    .await
    .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn install_modrinth_content(
    state: State<'_, AppState>,
    version_id: String,
    project_id: String,
    project_type: String,
) -> CmdResult<modrinth::ModrinthInstallReport> {
    let config = state.config();
    let (game_dir, game_version) = content_target(&config, &version_id)?;
    let destination = content_folder(&game_dir, &project_type)?;
    if state.game_status().is_some() {
        return Err("游戏运行时不能修改资源文件".to_string());
    }
    run_modrinth_task(modrinth::install_content_latest(
        &state.http,
        community_source(&config),
        &project_id,
        &destination,
        &game_version,
    ))
    .await
}

#[tauri::command]
pub async fn list_modrinth_versions(
    state: State<'_, AppState>,
    version_id: String,
    project_id: String,
) -> CmdResult<Vec<modrinth::ModrinthVersionInfo>> {
    let config = state.config();
    let (_, game_version, loader) = modrinth_target(&config, &version_id)?;
    modrinth::list_versions(
        &state.http,
        community_source(&config),
        &project_id,
        &game_version,
        &loader,
    )
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn install_modrinth_version(
    state: State<'_, AppState>,
    version_id: String,
    mod_version_id: String,
) -> CmdResult<modrinth::ModrinthInstallReport> {
    let config = state.config();
    let (mods_dir, game_version, loader) = modrinth_target(&config, &version_id)?;
    if state.game_status().is_some() {
        return Err("游戏运行时不能修改模组".to_string());
    }
    run_modrinth_task(modrinth::install_selected(
        &state.http,
        community_source(&config),
        &mod_version_id,
        &mods_dir,
        &game_version,
        &loader,
    ))
    .await
}

#[tauri::command]
pub async fn install_modrinth(
    state: State<'_, AppState>,
    version_id: String,
    project_id: String,
) -> CmdResult<modrinth::ModrinthInstallReport> {
    let config = state.config();
    let (mods_dir, game_version, loader) = modrinth_target(&config, &version_id)?;
    if state.game_status().is_some() {
        return Err("游戏运行时不能修改模组".to_string());
    }
    run_modrinth_task(modrinth::install_latest(
        &state.http,
        community_source(&config),
        &project_id,
        &mods_dir,
        &game_version,
        &loader,
    ))
    .await
}

#[tauri::command]
pub async fn search_modrinth_modpacks(
    state: State<'_, AppState>,
    version_id: String,
    query: String,
    offset: Option<u64>,
    limit: Option<u64>,
) -> CmdResult<modrinth::ModrinthSearchPage> {
    let config = state.config();
    let (_, game_version, loader) = modrinth_target(&config, &version_id)?;
    if !loader.eq_ignore_ascii_case("fabric") {
        return Err("整合包目前只支持 Fabric 基础版本".to_string());
    }
    modrinth::search_packs(
        &state.http,
        community_source(&config),
        &query,
        &game_version,
        "fabric",
        offset.unwrap_or(0),
        limit.unwrap_or(20).clamp(1, 30),
    )
    .await
    .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn install_modrinth_modpack(
    state: State<'_, AppState>,
    version_id: String,
    project_id: String,
) -> CmdResult<modrinth::ModpackInstallResult> {
    let config = state.config();
    let (_, _, loader) = modrinth_target(&config, &version_id)?;
    if !loader.eq_ignore_ascii_case("fabric") {
        return Err("整合包目前只支持 Fabric 基础版本".to_string());
    }
    if state.game_status().is_some() {
        return Err("游戏运行时不能安装整合包".to_string());
    }
    let result = run_modrinth_task(modrinth::install_latest_modpack(
        &state.http,
        community_source(&config),
        &project_id,
        &version_id,
        &config.root(),
    ))
    .await?;
    state
        .mutate_config(|config| {
            let entry = config.version_settings.entry(result.version_id.clone()).or_default();
            entry.isolate = Some(true);
            config.selected_version = Some(result.version_id.clone());
        })
        .map_err(|err| err.to_string())?;
    Ok(result)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MsaPollResponse {
    pub status: String,
    pub config: Option<creeper_core::config::AppConfig>,
    pub skin_data_url: Option<String>,
}

fn save_msa_identity(
    state: &AppState,
    identity: msa::MsaIdentity,
) -> CmdResult<MsaPollResponse> {
    let mut config = state.config();
    let existing = config.accounts.iter().position(|account| {
        account.kind.eq_ignore_ascii_case("msa")
            && account.uuid.eq_ignore_ascii_case(&identity.uuid)
    });
    let account = if let Some(index) = existing {
        let account = &mut config.accounts[index];
        account.username = identity.username.clone();
        account.access_token = Some(identity.access_token.clone());
        account.refresh_token = Some(identity.refresh_token.clone());
        account.expires_at = Some(identity.expires_at);
        account.last_used = Some(auth::now_iso());
        account.clone()
    } else {
        let now = auth::now_iso();
        let account = AccountProfile {
            id: uuid::Uuid::new_v4().to_string(),
            username: identity.username.clone(),
            uuid: identity.uuid.clone(),
            kind: "msa".to_string(),
            created_at: now.clone(),
            last_used: Some(now),
            access_token: Some(identity.access_token.clone()),
            refresh_token: Some(identity.refresh_token.clone()),
            expires_at: Some(identity.expires_at),
        };
        config.accounts.push(account.clone());
        account
    };
    config.active_account_id = Some(account.id.clone());
    state.save_config(&config).map_err(|err| err.to_string())?;
    Ok(MsaPollResponse {
        status: "complete".to_string(),
        config: Some(config),
        skin_data_url: identity.skin_data_url,
    })
}

#[tauri::command]
pub async fn save_local_skin(
    state: State<'_, AppState>,
    account_id: String,
    data_url: String,
) -> CmdResult<()> {
    let config = state.config();
    if !config.accounts.iter().any(|account| account.id == account_id) {
        return Err("账户不存在".to_string());
    }
    let data_dir = state
        .config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        offline_skin::save_local_skin(&data_dir, &account_id, &data_url)
    })
    .await
    .map_err(|err| err.to_string())?
    .map(|_| ())
    .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn delete_local_skin(
    state: State<'_, AppState>,
    account_id: String,
) -> CmdResult<()> {
    let data_dir = state
        .config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        offline_skin::delete_local_skin(&data_dir, &account_id)
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn list_modrinth_projects(
    state: State<'_, AppState>,
    project_ids: Vec<String>,
) -> CmdResult<Vec<modrinth::ModrinthProjectSummary>> {
    let config = state.config();
    modrinth::list_projects(&state.http, community_source(&config), &project_ids)
        .await
        .map_err(|err| err.to_string())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MsaCapeInfo {
    pub id: String,
    pub alias: String,
    pub active: bool,
    pub data_url: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MsaProfileInfo {
    pub skin_data_url: Option<String>,
    pub skin_variant: Option<String>,
    pub capes: Vec<MsaCapeInfo>,
    pub active_cape_id: Option<String>,
}

async fn msa_access_token(state: &AppState, account_id: &str) -> CmdResult<String> {
    let mut config = state.config();
    let index = config
        .accounts
        .iter()
        .position(|account| account.id == account_id)
        .ok_or_else(|| "账户不存在".to_string())?;
    if !config.accounts[index].kind.eq_ignore_ascii_case("msa") {
        return Err("只有 Microsoft 账户支持在线皮肤和披风".to_string());
    }
    let expires = config.accounts[index].expires_at.unwrap_or(0);
    if expires <= chrono::Utc::now().timestamp() + 120
        || config.accounts[index]
            .access_token
            .as_deref()
            .unwrap_or_default()
            .is_empty()
    {
        let refresh = config.accounts[index]
            .refresh_token
            .clone()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "Microsoft 登录已失效，请重新登录".to_string())?;
        let identity = msa::refresh_session(&state.http, &refresh)
            .await
            .map_err(|err| format!("Microsoft 登录刷新失败：{err}"))?;
        {
            let account = &mut config.accounts[index];
            account.username = identity.username.clone();
            account.uuid = identity.uuid.clone();
            account.access_token = Some(identity.access_token.clone());
            account.refresh_token = Some(identity.refresh_token);
            account.expires_at = Some(identity.expires_at);
            account.last_used = Some(auth::now_iso());
        }
        state.save_config(&config).map_err(|err| err.to_string())?;
        Ok(identity.access_token)
    } else {
        Ok(config.accounts[index]
            .access_token
            .clone()
            .unwrap_or_default())
    }
}

async fn msa_profile_from_value(
    http: &reqwest::Client,
    value: serde_json::Value,
) -> CmdResult<MsaProfileInfo> {
    let skin = value
        .get("skins")
        .and_then(|v| v.as_array())
        .and_then(|items| {
            items
                .iter()
                .find(|item| {
                    item.get("state")
                        .and_then(|v| v.as_str())
                        .map(|state| state.eq_ignore_ascii_case("ACTIVE"))
                        .unwrap_or(true)
                })
                .or_else(|| items.first())
        });
    let skin_data_url = if let Some(url) = skin
        .and_then(|item| item.get("url"))
        .and_then(|v| v.as_str())
    {
        msa::download_texture(http, url).await
    } else {
        None
    };
    let skin_variant = skin
        .and_then(|item| item.get("variant"))
        .and_then(|v| v.as_str())
        .map(|value| value.to_ascii_lowercase());
    let mut capes = Vec::new();
    let mut active_cape_id = None;
    if let Some(items) = value.get("capes").and_then(|v| v.as_array()) {
        for item in items {
            let Some(id) = item.get("id").and_then(|v| v.as_str()) else {
                continue;
            };
            let active = item
                .get("state")
                .and_then(|v| v.as_str())
                .map(|state| state.eq_ignore_ascii_case("ACTIVE"))
                .unwrap_or(false);
            if active {
                active_cape_id = Some(id.to_string());
            }
            let data_url = if let Some(url) = item.get("url").and_then(|v| v.as_str()) {
                msa::download_texture(http, url).await
            } else {
                None
            };
            capes.push(MsaCapeInfo {
                id: id.to_string(),
                alias: item
                    .get("alias")
                    .and_then(|v| v.as_str())
                    .unwrap_or("披风")
                    .to_string(),
                active,
                data_url,
            });
        }
    }
    Ok(MsaProfileInfo {
        skin_data_url,
        skin_variant,
        capes,
        active_cape_id,
    })
}

#[tauri::command]
pub async fn msa_profile(
    state: State<'_, AppState>,
    account_id: String,
) -> CmdResult<MsaProfileInfo> {
    let token = msa_access_token(&state, &account_id).await?;
    let value = msa::fetch_profile(&state.http, &token)
        .await
        .map_err(|err| err.to_string())?;
    msa_profile_from_value(&state.http, value).await
}

fn decode_skin_data_url(data_url: &str) -> CmdResult<Vec<u8>> {
    use base64::Engine as _;
    let payload = data_url
        .strip_prefix("data:image/png;base64,")
        .ok_or_else(|| "皮肤必须是 PNG 文件".to_string())?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload)
        .map_err(|_| "皮肤文件解码失败".to_string())?;
    if bytes.len() > 1_500_000 {
        return Err("皮肤文件不能超过 1.5 MB".to_string());
    }
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("皮肤不是有效的 PNG 文件".to_string());
    }
    Ok(bytes)
}

#[tauri::command]
pub async fn msa_upload_skin(
    state: State<'_, AppState>,
    account_id: String,
    data_url: String,
    variant: String,
) -> CmdResult<MsaProfileInfo> {
    let token = msa_access_token(&state, &account_id).await?;
    let bytes = decode_skin_data_url(&data_url)?;
    let value = msa::upload_skin(&state.http, &token, bytes, &variant)
        .await
        .map_err(|err| err.to_string())?;
    msa_profile_from_value(&state.http, value).await
}

#[tauri::command]
pub async fn msa_delete_skin(
    state: State<'_, AppState>,
    account_id: String,
) -> CmdResult<MsaProfileInfo> {
    let token = msa_access_token(&state, &account_id).await?;
    let value = msa::delete_skin(&state.http, &token)
        .await
        .map_err(|err| err.to_string())?;
    msa_profile_from_value(&state.http, value).await
}

#[tauri::command]
pub async fn msa_set_cape(
    state: State<'_, AppState>,
    account_id: String,
    cape_id: Option<String>,
) -> CmdResult<MsaProfileInfo> {
    let token = msa_access_token(&state, &account_id).await?;
    let value = msa::set_cape(&state.http, &token, cape_id.as_deref())
        .await
        .map_err(|err| err.to_string())?;
    msa_profile_from_value(&state.http, value).await
}

#[tauri::command]
pub async fn msa_embedded_login(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<MsaPollResponse> {
    let request = msa::build_auth_request().map_err(|err| err.to_string())?;
    let auth_url = Url::parse(&request.url)
        .map_err(|_| "无法生成 Microsoft 登录地址".to_string())?;
    if let Some(existing) = app.get_webview_window("msa-login") {
        let _ = existing.close();
    }
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Result<String, String>>();
    let navigation_tx = tx.clone();
    let expected_state = request.state.clone();
    let window = WebviewWindowBuilder::new(
        &app,
        "msa-login",
        WebviewUrl::External(auth_url),
    )
    .title("Microsoft 登录 · CreeperCore")
    .inner_size(520.0, 700.0)
    .min_inner_size(420.0, 560.0)
    .center()
    .on_navigation(move |url| {
        if url.scheme() == "http" && url.host_str() == Some("localhost") {
            let result = match msa::authorization_code_from_url(
                url.as_str(),
                &expected_state,
            ) {
                Ok(Some(code)) => Ok(code),
                Ok(None) => Err("Microsoft 回调没有授权码".to_string()),
                Err(err) => Err(err.to_string()),
            };
            let _ = navigation_tx.send(result);
            return false;
        }
        true
    })
    .build()
    .map_err(|err| format!("无法打开 Microsoft 登录窗口：{err}"))?;
    let close_tx = tx.clone();
    window.on_window_event(move |event| {
        if matches!(
            event,
            WindowEvent::CloseRequested { .. } | WindowEvent::Destroyed
        ) {
            let _ = close_tx.send(Err("Microsoft 登录窗口已关闭".to_string()));
        }
    });
    let code = match tokio::time::timeout(Duration::from_secs(300), rx.recv()).await {
        Ok(Some(Ok(code))) => code,
        Ok(Some(Err(err))) => {
            let _ = window.close();
            return Err(err);
        }
        Ok(None) => {
            let _ = window.close();
            return Err("Microsoft 登录窗口已关闭".to_string());
        }
        Err(_) => {
            let _ = window.close();
            return Err("Microsoft 登录超时，请重试".to_string());
        }
    };
    let _ = window.close();
    let identity = msa::login_with_authorization_code(
        &state.http,
        &code,
        &request.redirect_uri,
        &request.code_verifier,
    )
    .await
    .map_err(|err| err.to_string())?;
    save_msa_identity(&state, identity)
}

#[tauri::command]
pub async fn msa_refresh_account(
    state: State<'_, AppState>,
    account_id: String,
) -> CmdResult<MsaPollResponse> {
    let mut config = state.config();
    let index = config
        .accounts
        .iter()
        .position(|account| account.id == account_id)
        .ok_or_else(|| "账户不存在".to_string())?;
    let refresh = config.accounts[index]
        .refresh_token
        .clone()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "这个 Microsoft 账户没有 refresh token，请重新登录".to_string())?;
    let identity = msa::refresh_session(&state.http, &refresh)
        .await
        .map_err(|err| format!("Microsoft 登录刷新失败：{err}"))?;
    {
        let account = &mut config.accounts[index];
        account.username = identity.username.clone();
        account.uuid = identity.uuid.clone();
        account.access_token = Some(identity.access_token);
        account.refresh_token = Some(identity.refresh_token);
        account.expires_at = Some(identity.expires_at);
        account.last_used = Some(auth::now_iso());
    }
    state.save_config(&config).map_err(|err| err.to_string())?;
    Ok(MsaPollResponse {
        status: "complete".to_string(),
        config: Some(config),
        skin_data_url: identity.skin_data_url,
    })
}

#[tauri::command]
pub async fn fetch_remote_versions(state: State<'_, AppState>) -> CmdResult<VersionManifest> {
    let config = state.config();
    let source = download::DownloadSource::parse(&config.download_source);
    download::read_remote_manifest_cached(&state.http, &config.root(), source)
        .await
        .map_err(|err| format!("无法获取版本清单：{err}"))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelInstallResult {
    pub cancelled: bool,
}

#[tauri::command]
pub async fn install_game(
    app: AppHandle,
    state: State<'_, AppState>,
    version_id: String,
    loader: Option<String>,
    include_assets: Option<bool>,
) -> CmdResult<InstallOutcome> {
    let loader = loader.unwrap_or_else(|| "vanilla".to_string());
    if !matches!(loader.as_str(), "vanilla" | "fabric" | "forge" | "neoforge") {
        return Err("暂只支持原版、Fabric、Forge 与 NeoForge".to_string());
    }
    let config = state.config();
    let options = download::DownloadOptions {
        source: download::DownloadSource::parse(&config.download_source),
        concurrency: config.download_concurrency,
        include_assets: include_assets.unwrap_or(true),
    };
    let root = config.root();
    let installer_java = if matches!(loader.as_str(), "forge" | "neoforge") {
        let configured = config.java_for(&version_id);
        let scan_root = root.clone();
        let javas = tauri::async_runtime::spawn_blocking(move || {
            java::discover_java(&scan_root, configured.as_deref())
        })
        .await
        .map_err(|err| format!("扫描 Java 失败：{err}"))?;
        Some(
            pick_installer_java(&javas)
                .ok_or_else(|| "安装 Forge/NeoForge 需要可用的 Java 17 或更高版本".to_string())?,
        )
    } else {
        None
    };
    let task_id = uuid::Uuid::new_v4().to_string();
    {
        let mut guard = state.install.lock().await;
        if guard.is_some() {
            return Err("已有安装任务正在进行".to_string());
        }
        *guard = Some(crate::state::InstallControl {
            task_id: task_id.clone(),
            label: format!(
                "安装 {} {}",
                version_id,
                match loader.as_str() {
                    "fabric" => "Fabric",
                    "forge" => "Forge",
                    "neoforge" => "NeoForge",
                    _ => "原版",
                }
            ),
            cancel: download::CancelFlag::new(),
        });
    }
    let cancel = state
        .install
        .lock()
        .await
        .as_ref()
        .map(|item| item.cancel.clone())
        .unwrap_or_default();
    let _ = app.emit(
        "install-start",
        InstallStartPayload {
            task_id: task_id.clone(),
            version_id: version_id.clone(),
            loader: loader.clone(),
        },
    );
    let progress_app = app.clone();
    let progress_task = task_id.clone();
    let progress: download::ProgressFn = Arc::new(move |mut progress| {
        progress.task_id = progress_task.clone();
        let _ = progress_app.emit("install-progress", progress);
    });
    let client = state.http.clone();
    let result = match loader.as_str() {
        "fabric" => download::install_fabric(
            &client,
            &root,
            &version_id,
            &options,
            &task_id,
            progress.clone(),
            cancel.clone(),
        )
        .await
        .map(|result| InstallOutcome {
            version_id: result.version_id.clone(),
            loader: "fabric".to_string(),
            report: InstallReport {
                version_id: result.version_id.clone(),
                ..result.report
            },
            warnings: Vec::new(),
        }),
        "forge" | "neoforge" => {
            async {
                let mut base_report = InstallReport {
                    version_id: version_id.clone(),
                    ..Default::default()
                };
                if !version::version_json_path(&root, &version_id).exists() {
                    base_report = download::install_version(
                        &client,
                        &root,
                        &version_id,
                        &options,
                        &task_id,
                        progress.clone(),
                        cancel.clone(),
                    )
                    .await?;
                }
                let java = installer_java
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("没有可用的加载器安装 Java"))?;
                let label = if loader == "forge" { "Forge" } else { "NeoForge" };
                (progress)(InstallProgress {
                    task_id: task_id.clone(),
                    phase: "loader".to_string(),
                    label: format!("正在运行 {label} 安装器"),
                    message: String::new(),
                    current: 0,
                    total: 1,
                    bytes_done: 0,
                    bytes_total: 0,
                    percent: 0.0,
                });
                let installed = if loader == "forge" {
                    loader_install::install_forge(&client, &root, &version_id, java).await?
                } else {
                    loader_install::install_neoforge(&client, &root, &version_id, java).await?
                };
                Ok::<_, anyhow::Error>(InstallOutcome {
                    version_id: installed.version_id.clone(),
                    loader: loader.clone(),
                    report: InstallReport {
                        version_id: installed.version_id,
                        ..base_report
                    },
                    warnings: Vec::new(),
                })
            }
            .await
        }
        _ => download::install_version(
            &client,
            &root,
            &version_id,
            &options,
            &task_id,
            progress.clone(),
            cancel.clone(),
        )
        .await
        .map(|report| InstallOutcome {
            version_id: report.version_id.clone(),
            loader: "vanilla".to_string(),
            report,
            warnings: Vec::new(),
        }),
    };
    {
        let mut guard = state.install.lock().await;
        if guard
            .as_ref()
            .map(|item| item.task_id == task_id)
            .unwrap_or(false)
        {
            *guard = None;
        }
    }
    match result {
        Ok(outcome) => {
            let installed_id = outcome.version_id.clone();
            let _ = state.mutate_config(|config| {
                config.selected_version = Some(installed_id);
            });
            let _ = app.emit("install-finish", &outcome);
            Ok(outcome)
        }
        Err(err) => {
            let message = if download::is_cancelled(&err) {
                "安装已取消".to_string()
            } else {
                err.to_string()
            };
            let _ = app.emit("install-error", message.clone());
            Err(message)
        }
    }
}

#[tauri::command]
pub async fn cancel_install(state: State<'_, AppState>) -> CmdResult<CancelInstallResult> {
    let guard = state.install.lock().await;
    if let Some(control) = guard.as_ref() {
        control.cancel.cancel();
        Ok(CancelInstallResult { cancelled: true })
    } else {
        Ok(CancelInstallResult { cancelled: false })
    }
}

#[tauri::command]
pub async fn repair_game(
    app: AppHandle,
    state: State<'_, AppState>,
    version_id: String,
) -> CmdResult<InstallReport> {
    let config = state.config();
    let options = download::DownloadOptions {
        source: download::DownloadSource::parse(&config.download_source),
        concurrency: config.download_concurrency,
        include_assets: true,
    };
    let task_id = uuid::Uuid::new_v4().to_string();
    {
        let mut guard = state.install.lock().await;
        if guard.is_some() {
            return Err("已有任务正在进行".to_string());
        }
        *guard = Some(crate::state::InstallControl {
            task_id: task_id.clone(),
            label: format!("修复 {version_id}"),
            cancel: download::CancelFlag::new(),
        });
    }
    let cancel = state
        .install
        .lock()
        .await
        .as_ref()
        .map(|item| item.cancel.clone())
        .unwrap_or_default();
    let _ = app.emit(
        "install-start",
        InstallStartPayload {
            task_id: task_id.clone(),
            version_id: version_id.clone(),
            loader: "repair".to_string(),
        },
    );
    let progress_app = app.clone();
    let progress_task = task_id.clone();
    let progress: download::ProgressFn = Arc::new(move |mut progress| {
        progress.task_id = progress_task.clone();
        let _ = progress_app.emit("install-progress", progress);
    });
    let result = download::repair_version(
        &state.http,
        &config.root(),
        &version_id,
        &options,
        &task_id,
        progress,
        cancel,
    )
    .await;
    {
        let mut guard = state.install.lock().await;
        if guard
            .as_ref()
            .map(|item| item.task_id == task_id)
            .unwrap_or(false)
        {
            *guard = None;
        }
    }
    match result {
        Ok(report) => {
            let _ = app.emit("install-finish", report.clone());
            Ok(report)
        }
        Err(err) => {
            let message = if download::is_cancelled(&err) {
                "修复已取消".to_string()
            } else {
                err.to_string()
            };
            let _ = app.emit("install-error", message.clone());
            Err(message)
        }
    }
}

#[tauri::command]
pub async fn diagnose_game(
    state: State<'_, AppState>,
    version_id: String,
) -> CmdResult<DiagnosticReport> {
    let config = state.config();
    let root = config.root();
    let selected = config.selected_version.clone();
    let java_path = config.java_for(&version_id);
    let javas = tauri::async_runtime::spawn_blocking({
        let root = root.clone();
        move || java::discover_java(&root, java_path.as_deref())
    })
    .await
    .map_err(|err| err.to_string())?;
    let required = java_required(&root, Some(&version_id));
    let selected_java = choose_java(&config, &javas, Some(&version_id), required);
    let _ = selected;
    let game_dir = config.game_dir_for(&version_id);
    let memory = config.memory_for(&version_id);
    let account = config.active_account().cloned();
    tauri::async_runtime::spawn_blocking(move || {
        diagnose::diagnose(diagnose::DiagnoseInput {
            root: &root,
            version_id: &version_id,
            game_dir: &game_dir,
            java: selected_java.as_ref(),
            memory_mb: memory,
            account: account.as_ref(),
        })
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn list_mods(state: State<'_, AppState>, version_id: String) -> CmdResult<Vec<ModInfo>> {
    let config = state.config();
    let game_dir = config.game_dir_for(&version_id);
    tauri::async_runtime::spawn_blocking(move || mods::list_mods(&game_dir))
        .await
        .map_err(|err| err.to_string())?
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn toggle_mod(
    state: State<'_, AppState>,
    version_id: String,
    file_name: String,
    enabled: bool,
) -> CmdResult<String> {
    let config = state.config();
    let game_dir = config.game_dir_for(&version_id);
    tauri::async_runtime::spawn_blocking(move || {
        mods::set_mod_enabled(&game_dir, &file_name, enabled)
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn delete_mod(
    state: State<'_, AppState>,
    version_id: String,
    file_name: String,
) -> CmdResult<String> {
    let config = state.config();
    let game_dir = config.game_dir_for(&version_id);
    tauri::async_runtime::spawn_blocking(move || mods::delete_mod(&game_dir, &file_name))
        .await
        .map_err(|err| err.to_string())?
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn import_mods(
    state: State<'_, AppState>,
    version_id: String,
    paths: Vec<String>,
) -> CmdResult<ModActionReport> {
    let config = state.config();
    let game_dir = config.game_dir_for(&version_id);
    tauri::async_runtime::spawn_blocking(move || mods::import_mods(&game_dir, &paths))
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn list_backups(
    state: State<'_, AppState>,
    version_id: String,
) -> CmdResult<Vec<BackupInfo>> {
    let config = state.config();
    let root = config.root();
    tauri::async_runtime::spawn_blocking(move || backups::list_backups(&root, &version_id))
        .await
        .map_err(|err| err.to_string())?
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn create_backup(
    state: State<'_, AppState>,
    version_id: String,
    name: String,
    note: Option<String>,
    include_mods: Option<bool>,
) -> CmdResult<BackupInfo> {
    let config = state.config();
    let root = config.root();
    let game_dir = config.game_dir_for(&version_id);
    tauri::async_runtime::spawn_blocking(move || {
        backups::create_backup(
            &root,
            &game_dir,
            &version_id,
            &name,
            note.as_deref().unwrap_or(""),
            include_mods.unwrap_or(false),
        )
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn restore_backup(
    state: State<'_, AppState>,
    version_id: String,
    backup_id: String,
) -> CmdResult<RestoreReport> {
    let config = state.config();
    let root = config.root();
    let game_dir = config.game_dir_for(&version_id);
    tauri::async_runtime::spawn_blocking(move || {
        backups::restore_backup(&root, &game_dir, &version_id, &backup_id)
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn delete_backup(
    state: State<'_, AppState>,
    version_id: String,
    backup_id: String,
) -> CmdResult<()> {
    let config = state.config();
    let root = config.root();
    tauri::async_runtime::spawn_blocking(move || {
        backups::delete_backup(&root, &version_id, &backup_id)
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn launch_game(
    app: AppHandle,
    state: State<'_, AppState>,
    version_id: String,
) -> CmdResult<LaunchGameResult> {
    if state
        .game
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .is_some()
    {
        return Err("游戏已经在运行".to_string());
    }
    let mut config = state.config();
    let mut account = config
        .active_account()
        .cloned()
        .ok_or_else(|| "请先创建并选择一个玩家档案".to_string())?;
    if account.kind.eq_ignore_ascii_case("msa") {
        let expires = account.expires_at.unwrap_or(0);
        if account.access_token.as_deref().unwrap_or_default().is_empty()
            || expires <= chrono::Utc::now().timestamp() + 120
        {
            let refresh = account
                .refresh_token
                .clone()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    "Microsoft 登录已失效，请在玩家档案中重新登录".to_string()
                })?;
            let identity = msa::refresh_session(&state.http, &refresh)
                .await
                .map_err(|err| format!("Microsoft 登录刷新失败：{err}"))?;
            account.username = identity.username.clone();
            account.uuid = identity.uuid.clone();
            account.access_token = Some(identity.access_token);
            account.refresh_token = Some(identity.refresh_token);
            account.expires_at = Some(identity.expires_at);
            account.last_used = Some(auth::now_iso());
            if let Some(saved) = config
                .accounts
                .iter_mut()
                .find(|item| item.id == account.id)
            {
                *saved = account.clone();
            }
            state.save_config(&config).map_err(|err| err.to_string())?;
        }
    }
    let required = java_required(&config.root(), Some(&version_id));
    let javas = tauri::async_runtime::spawn_blocking({
        let root = config.root();
        let configured = config.java_for(&version_id);
        move || java::discover_java(&root, configured.as_deref())
    })
    .await
    .map_err(|err| err.to_string())?;
    let selected_java = choose_java(&config, &javas, Some(&version_id), required)
        .ok_or_else(|| "没有找到可用的 Java，请先在设置中指定".to_string())?;
    if config.java_path.is_none() {
        config.java_path = Some(selected_java.path.clone());
        let _ = state.save_config(&config);
    }
    let memory = config.memory_for(&version_id);
    let settings = LaunchSettings {
        memory_mb: memory,
        java_path: selected_java.path.clone(),
        jvm_args: config.jvm_args_for(&version_id),
        width: config.width,
        height: config.height,
        fullscreen: config.fullscreen,
        demo: false,
        extra_game_args: String::new(),
    };
    let game_dir = config.game_dir_for(&version_id);
    let mut warnings = Vec::new();

    let skin_data_dir = state
        .config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let skin_root = config.root();
    let skin_version = version_id.clone();
    let skin_game_dir = game_dir.clone();
    let skin_account = account.clone();
    let skin_result = tauri::async_runtime::spawn_blocking(move || {
        if skin_account.kind.eq_ignore_ascii_case("offline") {
            offline_skin::sync_account(
                &skin_game_dir,
                &skin_root,
                &skin_version,
                &skin_data_dir,
                &skin_account.id,
            )
        } else {
            offline_skin::clear(&skin_game_dir)
        }
    })
    .await
    .map_err(|err| err.to_string())?;
    if let Err(err) = skin_result {
        warnings.push(format!("离线皮肤资源包应用失败：{err}"));
    }

    if config.auto_backup_before_launch {
        let root = config.root();
        let game_dir_for_backup = game_dir.clone();
        let version_for_backup = version_id.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            backups::create_backup(
                &root,
                &game_dir_for_backup,
                &version_for_backup,
                "启动前自动快照",
                "由「启动前自动快照」设置创建",
                false,
            )
        })
        .await
        .map_err(|err| err.to_string())?;
        if let Err(err) = result {
            warnings.push(format!("自动快照失败：{err}"));
        }
    }

    let mut plan =
        launch::build_launch_plan(&config.root(), &version_id, &game_dir, &account, &settings)
            .map_err(|err| err.to_string())?;
    warnings.append(&mut plan.warnings);
    if let Some(parent) = Path::new(&plan.log_path).parent() {
        std::fs::create_dir_all(parent).map_err(|err| format!("无法创建日志目录：{err}"))?;
    }
    let header = format!(
        "==== Creeper Core {} | {} | Java {} | {} MB ====\n游戏目录：{}\n启动时间：{}\n\n",
        LAUNCHER_VERSION,
        version_id,
        selected_java.major,
        memory,
        game_dir.display(),
        chrono::Local::now().to_rfc3339()
    );
    std::fs::write(&plan.log_path, header.as_bytes())
        .map_err(|err| format!("无法写入日志文件：{err}"))?;
    let file = tokio::fs::OpenOptions::new()
        .append(true)
        .open(&plan.log_path)
        .await
        .map_err(|err| format!("无法打开日志文件：{err}"))?;
    let mut command = Command::new(&plan.program);
    command
        .args(&plan.args)
        .current_dir(&plan.game_dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.as_std_mut().creation_flags(0x0800_0000);
    }
    let mut child = command
        .spawn()
        .map_err(|err| format!("无法启动 Java：{err}"))?;
    let pid = child
        .id()
        .ok_or_else(|| "无法获取游戏进程 PID".to_string())?;
    let started_at = chrono::Local::now().to_rfc3339();
    let started_instant = Instant::now();
    let stop_requested = Arc::new(AtomicBool::new(false));
    let inserted = {
        let mut guard = state.game.lock().unwrap_or_else(|err| err.into_inner());
        if guard.is_some() {
            false
        } else {
            *guard = Some(GameSession {
                pid,
                version_id: version_id.clone(),
                started_at_iso: started_at.clone(),
                log_path: plan.log_path.clone(),
                stop_requested: stop_requested.clone(),
            });
            true
        }
    };
    if !inserted {
        let _ = child.kill().await;
        return Err("游戏已经在运行".to_string());
    }
    state.clear_logs();
    state.push_log(&header);
    let startup = GameStartPayload {
        pid,
        version_id: version_id.clone(),
        started_at: started_at.clone(),
        log_path: plan.log_path.clone(),
    };
    let _ = app.emit("game-start", &startup);
    tauri::async_runtime::spawn(wait_for_game_window(
        app.clone(),
        pid,
        version_id.clone(),
    ));

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let log_file = Arc::new(tokio::sync::Mutex::new(file));
    let tail = Arc::new(tokio::sync::Mutex::new(VecDeque::<String>::new()));
    let app_for_stdout = app.clone();
    let app_for_stderr = app.clone();
    let log_for_stdout = log_file.clone();
    let log_for_stderr = log_file.clone();
    let tail_for_stdout = tail.clone();
    let tail_for_stderr = tail.clone();
    let stdout_handle = stdout.map(|stream| {
        tauri::async_runtime::spawn(pump_stream(
            app_for_stdout,
            stream,
            "stdout",
            log_for_stdout,
            tail_for_stdout,
        ))
    });
    let stderr_handle = stderr.map(|stream| {
        tauri::async_runtime::spawn(pump_stream(
            app_for_stderr,
            stream,
            "stderr",
            log_for_stderr,
            tail_for_stderr,
        ))
    });

    let log_path_for_monitor = plan.log_path.clone();
    let version_for_monitor = version_id.clone();
    let app_for_monitor = app.clone();
    let tail_for_monitor = tail;
    let stop_for_monitor = stop_requested;
    let game_dir_for_monitor = game_dir.clone();
    tauri::async_runtime::spawn(async move {
        let status = child.wait().await;
        if let Some(handle) = stdout_handle {
            let _ = handle.await;
        }
        if let Some(handle) = stderr_handle {
            let _ = handle.await;
        }
        let exit_code = status.as_ref().ok().and_then(|status| status.code());
        let log_text = {
            let tail = tail_for_monitor.lock().await;
            tail.iter().cloned().collect::<Vec<_>>().join("\n")
        };
        let play_time_secs = started_instant.elapsed().as_secs();
        // Minecraft writes a clean, non-XML log of its own. Prefer its tail for
        // crash triage once the process has released the file.
        let analysis_text =
            capture_tail(&game_dir_for_monitor.join("logs").join("latest.log"), 600)
                .ok()
                .filter(|text| !text.trim().is_empty())
                .unwrap_or_else(|| log_text.clone());
        let mut report = launch::analyze_crash(
            &analysis_text,
            exit_code,
            &log_path_for_monitor,
            play_time_secs,
        );
        if stop_for_monitor.load(std::sync::atomic::Ordering::SeqCst) {
            report.level = "info".to_string();
            report.kind = "stopped".to_string();
            report.title = "游戏已由启动器终止".to_string();
            report.summary = "进程已被手动结束，存档通常会保留最近一次自动保存。".to_string();
            report.suggestions = vec!["如果误触终止，请重新启动游戏".to_string()];
        }
        {
            let app_state = app_for_monitor.state::<AppState>();
            let mut guard = app_state.game.lock().unwrap_or_else(|err| err.into_inner());
            if guard.as_ref().map(|session| session.pid) == Some(pid) {
                *guard = None;
            }
            app_state.set_last_crash(report.clone());
        }
        let _ = version_for_monitor;
        let _ = app_for_monitor.emit("game-exit", report);
    });

    Ok(LaunchGameResult {
        pid,
        version_id,
        started_at,
        log_path: plan.log_path,
        warnings,
        java_path: selected_java.path,
        java_major: Some(selected_java.major),
        game_dir: game_dir.to_string_lossy().to_string(),
    })
}

#[derive(Default)]
struct LogXmlContext {
    thread: String,
    level: String,
}

impl LogXmlContext {
    fn attr(line: &str, name: &str) -> Option<String> {
        let marker = format!("{name}=\"");
        let start = line.find(&marker)? + marker.len();
        let rest = &line[start..];
        let end = rest.find('"')?;
        Some(rest[..end].to_string())
    }

    /// The vanilla log4j configuration prints its XML event layout to stdout.
    /// Show one clean, human-readable line in the launcher UI while preserving
    /// the raw stream in the on-disk forensic log.
    fn clean(&mut self, line: &str) -> Option<String> {
        let trimmed = line.trim();
        if trimmed.starts_with("<log4j:Event") {
            if let Some(thread) = Self::attr(trimmed, "thread") {
                self.thread = thread;
            }
            if let Some(level) = Self::attr(trimmed, "level") {
                self.level = level;
            }
            return None;
        }
        if trimmed.starts_with("</log4j:Event") || trimmed.starts_with("<?xml") {
            return None;
        }
        if let Some(start) = line.find("<log4j:Message><![CDATA[") {
            let start = start + "<log4j:Message><![CDATA[".len();
            let rest = &line[start..];
            let message = rest
                .split("]]></log4j:Message>")
                .next()
                .unwrap_or(rest)
                .trim();
            if message.is_empty() {
                return None;
            }
            let prefix = if self.thread.is_empty() {
                String::new()
            } else {
                format!("[{}/{}] ", self.thread, self.level)
            };
            return Some(format!("{prefix}{message}"));
        }
        if let Some(start) = line.find("<log4j:Throwable><![CDATA[") {
            let start = start + "<log4j:Throwable><![CDATA[".len();
            let rest = &line[start..];
            let message = rest
                .split("]]></log4j:Throwable>")
                .next()
                .unwrap_or(rest)
                .trim();
            if message.is_empty() {
                return None;
            }
            return Some(message.to_string());
        }
        if trimmed.contains("]]></log4j:Throwable>") {
            return None;
        }
        if trimmed.starts_with("<log4j:") {
            return None;
        }
        Some(line.to_string())
    }
}

async fn pump_stream<R>(
    app: AppHandle,
    stream: R,
    stream_name: &'static str,
    log_file: Arc<tokio::sync::Mutex<tokio::fs::File>>,
    tail: Arc<tokio::sync::Mutex<VecDeque<String>>>,
) where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    let mut lines = BufReader::new(stream).lines();
    let mut xml = LogXmlContext::default();
    loop {
        match lines.next_line().await {
            Ok(Some(line)) => {
                {
                    let mut file = log_file.lock().await;
                    let _ = file.write_all(format!("{line}\n").as_bytes()).await;
                    let _ = file.flush().await;
                }
                {
                    let mut tail = tail.lock().await;
                    tail.push_back(line.clone());
                    while tail.len() > 1200 {
                        tail.pop_front();
                    }
                }
                let Some(display_line) = xml.clean(&line) else {
                    continue;
                };
                if let Some(state) = app.try_state::<AppState>() {
                    state.push_log(&display_line);
                }
                let _ = app.emit(
                    "game-log",
                    GameLogPayload {
                        line: display_line,
                        stream: stream_name.to_string(),
                        timestamp: chrono::Local::now().to_rfc3339(),
                    },
                );
            }
            Ok(None) => break,
            Err(_) => break,
        }
    }
}

#[tauri::command]
pub async fn stop_game(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    let (pid, stop_requested, version_id) = {
        let guard = state.game.lock().unwrap_or_else(|err| err.into_inner());
        let session = guard.as_ref().ok_or_else(|| "游戏尚未运行".to_string())?;
        (
            session.pid,
            session.stop_requested.clone(),
            session.version_id.clone(),
        )
    };
    stop_requested.store(true, std::sync::atomic::Ordering::SeqCst);
    #[cfg(windows)]
    {
        let mut command = Command::new("taskkill");
        command.arg("/PID").arg(pid.to_string()).arg("/T").arg("/F");
        let _ = command.status().await;
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("kill")
            .arg("-TERM")
            .arg(pid.to_string())
            .status()
            .await;
    }
    let _ = app.emit(
        "game-stopping",
        serde_json::json!({ "pid": pid, "versionId": version_id }),
    );
    Ok(())
}

#[tauri::command]
pub async fn get_game_status(state: State<'_, AppState>) -> CmdResult<Option<GameStatus>> {
    Ok(state.game_status())
}

#[tauri::command]
pub async fn get_game_logs(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> CmdResult<Vec<String>> {
    Ok(state.log_tail(limit.unwrap_or(400).min(2000)))
}

#[tauri::command]
pub async fn read_latest_log(
    state: State<'_, AppState>,
    version_id: String,
) -> CmdResult<LogFilePayload> {
    let config = state.config();
    if let Some(status) = state.game_status() {
        if status.version_id == version_id {
            let path = PathBuf::from(&status.log_path);
            if path.exists() {
                let content = capture_tail(&path, 800).map_err(|err| err.to_string())?;
                return Ok(LogFilePayload {
                    path: status.log_path,
                    content,
                });
            }
        }
    }
    let log_dir = config.root().join(".creeper").join("logs").join(
        creeper_core::util::sanitize_component(&version_id)
            .unwrap_or_else(|| "version".to_string()),
    );
    let latest = std::fs::read_dir(&log_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("log"))
        .max_by_key(|path| std::fs::metadata(path).and_then(|m| m.modified()).ok());
    if let Some(path) = latest {
        let content = capture_tail(&path, 800).map_err(|err| err.to_string())?;
        return Ok(LogFilePayload {
            path: path.to_string_lossy().to_string(),
            content,
        });
    }
    Err("还没有这个版本的启动日志".to_string())
}

fn capture_tail(path: &Path, max_lines: usize) -> Result<String> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("无法读取日志：{}", path.display()))?;
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    Ok(lines[start..].join("\n"))
}

fn clear_readonly(path: &Path) {
    if let Ok(metadata) = std::fs::metadata(path) {
        let mut permissions = metadata.permissions();
        if permissions.readonly() {
            permissions.set_readonly(false);
            let _ = std::fs::set_permissions(path, permissions);
        }
        if path.is_dir() {
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    clear_readonly(&entry.path());
                }
            }
        }
    }
}

async fn remove_version_dir(source: &Path) -> CmdResult<()> {
    let mut last_error = None;
    for attempt in 0..4 {
        let path = source.to_path_buf();
        let result = tauri::async_runtime::spawn_blocking(move || -> std::io::Result<()> {
            if path.exists() {
                std::fs::remove_dir_all(&path)
            } else {
                Ok(())
            }
        })
        .await
        .map_err(|err| err.to_string())?;
        match result {
            Ok(()) => return Ok(()),
            Err(err) => {
                last_error = Some(err);
                if attempt < 3 {
                    if attempt == 0 {
                        let path = source.to_path_buf();
                        let _ = tauri::async_runtime::spawn_blocking(move || {
                            clear_readonly(&path);
                        })
                        .await;
                    }
                    tokio::time::sleep(Duration::from_millis(350)).await;
                }
            }
        }
    }
    Err(format!(
        "无法删除版本：{}。请先完全退出该版本的游戏；如果仍失败，关闭正在占用该目录的窗口或杀毒软件扫描后重试。",
        last_error
            .map(|err| err.to_string())
            .unwrap_or_else(|| "未知错误".to_string())
    ))
}

#[tauri::command]
pub async fn delete_version(state: State<'_, AppState>, version_id: String) -> CmdResult<()> {
    let config = state.config();
    if state.install.lock().await.is_some() {
        return Err("安装/修复进行中，暂时不能删除版本".to_string());
    }
    if let Some(status) = state.game_status() {
        if status.version_id == version_id {
            return Err("该版本正在运行".to_string());
        }
    }
    let root = config.root();
    let versions = version::scan_versions(&root).map_err(|err| err.to_string())?;
    if !versions.iter().any(|item| item.id == version_id) {
        return Err("版本不存在".to_string());
    }
    if let Some(child) = versions
        .iter()
        .find(|item| item.inherits_from.as_deref() == Some(version_id.as_str()))
    {
        return Err(format!("版本 {} 正在继承它，请先处理派生版本", child.id));
    }
    let safe_id = creeper_core::util::safe_component(&version_id).map_err(|err| err.to_string())?;
    let source = version::versions_dir(&root).join(&safe_id);
    if !source.exists() {
        return Err("版本目录不存在".to_string());
    }
    remove_version_dir(&source).await?;
    state
        .mutate_config(|config| {
            if config.selected_version.as_deref() == Some(version_id.as_str()) {
                config.selected_version = None;
            }
            config.version_settings.remove(&version_id);
        })
        .map_err(|err| err.to_string())?;
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<String>,
}

#[tauri::command]
pub async fn list_instance_entries(
    state: State<'_, AppState>,
    version_id: String,
    kind: String,
) -> CmdResult<Vec<InstanceEntry>> {
    let config = state.config();
    let game_dir = config.game_dir_for(&version_id);
    let dir = match kind.as_str() {
        "saves" => game_dir.join("saves"),
        "resourcepacks" => game_dir.join("resourcepacks"),
        "shaderpacks" => game_dir.join("shaderpacks"),
        "datapacks" => game_dir.join("datapacks"),
        "mods" => game_dir.join("mods"),
        _ => return Err("未知的实例目录".to_string()),
    };
    let entries = tauri::async_runtime::spawn_blocking(move || -> std::io::Result<Vec<InstanceEntry>> {
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut result = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let Ok(entry) = entry else { continue };
            let path = entry.path();
            let Ok(metadata) = entry.metadata() else { continue };
            let is_dir = metadata.is_dir();
            let modified = metadata
                .modified()
                .ok()
                .map(|time| {
                    let date: chrono::DateTime<chrono::Utc> = time.into();
                    date.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
                });
            result.push(InstanceEntry {
                name: entry.file_name().to_string_lossy().to_string(),
                path: path.to_string_lossy().to_string(),
                is_dir,
                size: if is_dir { 0 } else { metadata.len() },
                modified,
            });
        }
        result.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(result)
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(|err| format!("无法读取实例目录：{err}"))?;
    Ok(entries)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionSettingsUpdate {
    pub memory_mb: Option<u32>,
    pub java_path: Option<String>,
    pub jvm_args: Option<String>,
    pub isolate: Option<bool>,
}

#[tauri::command]
pub async fn save_version_settings(
    state: State<'_, AppState>,
    version_id: String,
    update: VersionSettingsUpdate,
) -> CmdResult<creeper_core::config::AppConfig> {
    let config = state
        .mutate_config(|config| {
            let entry = config
                .version_settings
                .entry(version_id.clone())
                .or_default();
            entry.memory_mb = update
                .memory_mb
                .filter(|value| *value >= 512)
                .map(|value| value.clamp(512, 32768));
            entry.java_path = update
                .java_path
                .filter(|value| !value.trim().is_empty());
            entry.jvm_args = update
                .jvm_args
                .filter(|value| !value.trim().is_empty());
            entry.isolate = update.isolate;
            if entry.memory_mb.is_none()
                && entry.java_path.is_none()
                && entry.jvm_args.is_none()
                && entry.isolate.is_none()
            {
                config.version_settings.remove(&version_id);
            }
            config.clone()
        })
        .map_err(|err| err.to_string())?;
    Ok(config)
}

#[tauri::command]
pub async fn open_path(
    state: State<'_, AppState>,
    target: String,
    version_id: Option<String>,
) -> CmdResult<()> {
    let config = state.config();
    let root = config.root();
    let version = version_id
        .clone()
        .or_else(|| config.selected_version.clone())
        .unwrap_or_default();
    let game_dir = if version.is_empty() {
        root.clone()
    } else {
        config.game_dir_for(&version)
    };
    let path = match target.as_str() {
        "root" => root.clone(),
        "game" => game_dir.clone(),
        "mods" => game_dir.join("mods"),
        "resourcepacks" => game_dir.join("resourcepacks"),
        "shaderpacks" => game_dir.join("shaderpacks"),
        "datapacks" => game_dir.join("datapacks"),
        "saves" => game_dir.join("saves"),
        "config" => game_dir.join("config"),
        "screenshots" => game_dir.join("screenshots"),
        "logs" => {
            if version.is_empty() {
                root.join(".creeper").join("logs")
            } else {
                root.join(".creeper").join("logs").join(
                    creeper_core::util::sanitize_component(&version)
                        .unwrap_or_else(|| "version".to_string()),
                )
            }
        }
        "backups" => backups::backups_root(&root),
        "version" => {
            if version.is_empty() {
                root.join("versions")
            } else {
                version::versions_dir(&root).join(
                    creeper_core::util::sanitize_component(&version)
                        .unwrap_or_else(|| "version".to_string()),
                )
            }
        }
        _ => return Err("未知目录".to_string()),
    };
    if !path.exists() {
        let _ = std::fs::create_dir_all(&path);
    }
    #[cfg(windows)]
    {
        Command::new("explorer.exe")
            .arg(&path)
            .spawn()
            .map_err(|err| format!("无法打开目录：{err}"))?;
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|err| format!("无法打开目录：{err}"))?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|err| format!("无法打开目录：{err}"))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn open_external(url: String) -> CmdResult<()> {
    let parsed = reqwest::Url::parse(&url).map_err(|_| "外部链接格式无效".to_string())?;
    if parsed.scheme() != "https" {
        return Err("只允许打开 HTTPS 链接".to_string());
    }
    let host = parsed.host_str().unwrap_or_default().to_ascii_lowercase();
    let allowed = host == "modrinth.com"
        || host == "www.modrinth.com"
        || host == "mcmod.cn"
        || host == "www.mcmod.cn"
        || host.ends_with(".mcmod.cn")
        || host == "microsoft.com"
        || host == "www.microsoft.com"
        || host.ends_with(".microsoft.com")
        || host == "aka.ms"
        || host.ends_with(".aka.ms");
    if !allowed {
        return Err("不支持的外部站点".to_string());
    }
    #[cfg(windows)]
    {
        std::process::Command::new("rundll32.exe")
            .args(["url.dll,FileProtocolHandler", &url])
            .spawn()
            .map_err(|err| format!("无法打开浏览器：{err}"))?;
    }
    #[cfg(not(windows))]
    {
        let program = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
        std::process::Command::new(program)
            .arg(&url)
            .spawn()
            .map_err(|err| format!("无法打开浏览器：{err}"))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn reveal_latest_log(state: State<'_, AppState>) -> CmdResult<String> {
    let config = state.config();
    let log_dir = config.root().join(".creeper").join("logs");
    if !log_dir.exists() {
        std::fs::create_dir_all(&log_dir).map_err(|err| err.to_string())?;
    }
    #[cfg(windows)]
    {
        let _ = Command::new("explorer.exe").arg(&log_dir).spawn();
    }
    Ok(log_dir.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn get_app_info() -> CmdResult<serde_json::Value> {
    Ok(serde_json::json!({
        "name": LAUNCHER_NAME,
        "version": LAUNCHER_VERSION,
        "identifier": "com.creepercore.launcher",
        "debugBuild": cfg!(debug_assertions),
    }))
}
