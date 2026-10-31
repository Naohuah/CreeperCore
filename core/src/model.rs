//! Shared data contracts for Creeper Core and its desktop shell.
//!
//! The engine deliberately keeps all network / filesystem / process logic out of
//! the UI layer. These structs are serialized to the WebView as camelCase JSON.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const LAUNCHER_NAME: &str = "CreeperCore";
pub const LAUNCHER_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DEFAULT_MEMORY_MB: u32 = 4096;
pub const MIN_MEMORY_MB: u32 = 512;
pub const MAX_MEMORY_MB: u32 = 65536;

// ---------------------------------------------------------------------------
// Versions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionSummary {
    pub id: String,
    pub display_name: String,
    pub version_type: String,
    /// vanilla | fabric | quilt | forge | neoforge | unknown
    pub loader: String,
    pub loader_version: Option<String>,
    pub release_time: Option<String>,
    /// Unix epoch milliseconds.
    pub last_modified: u64,
    pub java_major: Option<u32>,
    pub has_json: bool,
    pub has_client: bool,
    pub main_class: Option<String>,
    pub inherits_from: Option<String>,
    pub read_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndexRef {
    pub id: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    pub total_size: Option<u64>,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DownloadArtifact {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LibraryDownloads {
    #[serde(default)]
    pub artifact: Option<DownloadArtifact>,
    #[serde(default)]
    pub classifiers: Option<HashMap<String, DownloadArtifact>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OsRule {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arch: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub version_range: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub action: String,
    #[serde(default)]
    pub os: Option<OsRule>,
    #[serde(default)]
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExtractRule {
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Library {
    pub name: String,
    #[serde(default)]
    pub downloads: Option<LibraryDownloads>,
    #[serde(default)]
    pub natives: Option<HashMap<String, String>>,
    #[serde(default)]
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub extract: Option<ExtractRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<serde_json::Value>,
    #[serde(default)]
    pub jvm: Vec<serde_json::Value>,
    #[serde(default)]
    pub default_user_jvm: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LoggingFile {
    pub argument: String,
    #[serde(default)]
    pub file: Option<DownloadArtifact>,
    #[serde(rename = "type", default)]
    pub log_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LoggingConfig {
    #[serde(default)]
    pub client: Option<LoggingFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersionRef {
    #[serde(default)]
    pub component: Option<String>,
    #[serde(default)]
    pub major_version: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VersionDownloads {
    #[serde(default)]
    pub client: Option<DownloadArtifact>,
    #[serde(default)]
    pub server: Option<DownloadArtifact>,
}

/// A fully merged version profile (inheritance already applied).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedVersion {
    pub id: String,
    #[serde(rename = "type")]
    pub version_type: String,
    pub release_time: Option<String>,
    pub main_class: String,
    pub assets_id: Option<String>,
    pub asset_index: Option<AssetIndexRef>,
    pub libraries: Vec<Library>,
    pub arguments: Option<Arguments>,
    pub minecraft_arguments: Option<String>,
    pub java_major: Option<u32>,
    pub inherits_from: Option<String>,
    /// Version whose jar file should be put on the classpath.
    pub jar_id: String,
    pub client: Option<DownloadArtifact>,
    pub logging: Option<LoggingConfig>,
    pub total_download_size: u64,
}

/// Raw version JSON as found in versions/<id>/<id>.json.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VersionJson {
    pub id: String,
    #[serde(rename = "type", default = "default_version_type")]
    pub version_type: String,
    #[serde(default)]
    pub release_time: Option<String>,
    #[serde(default)]
    pub time: Option<String>,
    #[serde(default)]
    pub main_class: Option<String>,
    #[serde(default)]
    pub assets: Option<String>,
    #[serde(default)]
    pub asset_index: Option<AssetIndexRef>,
    #[serde(default)]
    pub downloads: Option<VersionDownloads>,
    #[serde(default)]
    pub java_version: Option<JavaVersionRef>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(default)]
    pub arguments: Option<Arguments>,
    #[serde(default)]
    pub minecraft_arguments: Option<String>,
    #[serde(default)]
    pub logging: Option<LoggingConfig>,
    #[serde(default)]
    pub inherits_from: Option<String>,
    #[serde(default)]
    pub minimum_launcher_version: Option<u32>,
    #[serde(default)]
    pub compliance_level: Option<u32>,
}

fn default_version_type() -> String {
    "release".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VersionDetail {
    pub id: String,
    pub display_name: String,
    pub main_class: String,
    pub version_type: String,
    pub loader: String,
    pub loader_version: Option<String>,
    pub inherits_from: Option<String>,
    pub jar_id: String,
    pub java_major: Option<u32>,
    pub assets_id: Option<String>,
    pub libraries_count: usize,
    pub libraries_missing: usize,
    pub client_jar_exists: bool,
    pub asset_index_exists: bool,
    pub assets_missing: usize,
    pub game_dir: String,
    pub is_isolated: bool,
    pub mods_count: usize,
    pub total_download_size: u64,
}

// ---------------------------------------------------------------------------
// Remote metadata / installing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LatestVersions {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RemoteVersion {
    pub id: String,
    #[serde(rename = "type")]
    pub version_type: String,
    pub url: String,
    pub time: String,
    pub release_time: String,
    pub sha1: String,
    #[serde(default)]
    pub compliance_level: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VersionManifest {
    pub latest: LatestVersions,
    pub versions: Vec<RemoteVersion>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InstallReport {
    pub version_id: String,
    pub files_downloaded: usize,
    pub files_reused: usize,
    pub bytes_downloaded: u64,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InstallProgress {
    pub task_id: String,
    /// resolve | client | libraries | assets | natives | logging | done
    pub phase: String,
    pub label: String,
    pub current: usize,
    pub total: usize,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub percent: f64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FabricLoaderVersion {
    pub version: String,
    pub stable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FabricInstallResult {
    pub version_id: String,
    pub game_version: String,
    pub loader_version: String,
    pub report: InstallReport,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AccountProfile {
    pub id: String,
    pub username: String,
    pub uuid: String,
    /// offline | msa
    pub kind: String,
    pub created_at: String,
    pub last_used: Option<String>,
    /// Short-lived Minecraft access token.
    #[serde(default)]
    pub access_token: Option<String>,
    /// Microsoft OAuth refresh token. Never prompt for or store passwords.
    #[serde(default)]
    pub refresh_token: Option<String>,
    /// Unix epoch seconds for the Minecraft access token.
    #[serde(default)]
    pub expires_at: Option<i64>,
}

// ---------------------------------------------------------------------------
// Java / system
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct JavaInfo {
    pub path: String,
    pub version: String,
    pub major: u32,
    pub vendor: String,
    pub architecture: String,
    pub is_64bit: bool,
    pub source: String,
    pub recommended: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub os: String,
    pub arch: String,
    pub cpu_threads: usize,
    pub total_memory_mb: u64,
    pub used_memory_mb: u64,
    pub launcher_version: String,
}

// ---------------------------------------------------------------------------
// Launch
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LaunchSettings {
    pub memory_mb: u32,
    pub java_path: String,
    pub jvm_args: String,
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
    pub demo: bool,
    pub extra_game_args: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LaunchPlan {
    pub version_id: String,
    pub game_dir: String,
    pub java_path: String,
    pub java_major: Option<u32>,
    pub required_java: Option<u32>,
    pub program: String,
    pub args: Vec<String>,
    pub log_path: String,
    pub native_dir: String,
    pub warnings: Vec<String>,
    pub classpath_entries: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CrashReport {
    pub level: String,
    /// exited | crashed | java | memory | graphics | mods | files | unknown
    pub kind: String,
    pub exit_code: Option<i32>,
    pub title: String,
    pub summary: String,
    pub suggestions: Vec<String>,
    pub log_path: String,
    pub signal_line: Option<String>,
    pub play_time_secs: u64,
}

// ---------------------------------------------------------------------------
// Mods
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModInfo {
    pub file_name: String,
    pub path: String,
    pub name: String,
    pub mod_id: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub loader: String,
    pub enabled: bool,
    pub size: u64,
    pub modified: u64,
    pub missing_dependencies: Vec<String>,
    pub duplicate: bool,
    pub read_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModActionReport {
    pub changed: Vec<String>,
    pub skipped: Vec<String>,
    pub errors: Vec<String>,
}

// ---------------------------------------------------------------------------
// Backups
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub id: String,
    pub version_id: String,
    pub name: String,
    pub note: String,
    pub created_at: String,
    pub size_bytes: u64,
    pub file_count: u64,
    pub kind: String,
    pub path: String,
    pub game_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RestoreReport {
    pub backup: BackupInfo,
    pub safety_backup: Option<BackupInfo>,
    pub files_restored: usize,
    pub files_skipped: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BackupManifest {
    pub id: String,
    pub version_id: String,
    pub name: String,
    pub note: String,
    pub created_at: String,
    pub kind: String,
    pub file_count: u64,
    pub size_bytes: u64,
    pub game_dir: String,
    pub entries: Vec<String>,
}

// ---------------------------------------------------------------------------
// Diagnostics
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticItem {
    pub id: String,
    /// ok | info | warn | error
    pub severity: String,
    pub title: String,
    pub detail: String,
    /// repair | settings | java | mods | backup | none
    pub action: String,
    pub fixable: bool,
    pub points: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticReport {
    pub version_id: String,
    pub score: u32,
    pub level: String,
    pub headline: String,
    pub items: Vec<DiagnosticItem>,
    pub checked_at: String,
    pub java_path: Option<String>,
    pub java_major: Option<u32>,
    pub has_missing_files: bool,
}
