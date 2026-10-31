//! Persistent launcher configuration.

use crate::auth;
use crate::model::{AccountProfile, DEFAULT_MEMORY_MB, MAX_MEMORY_MB, MIN_MEMORY_MB};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Release builds run next to their data. The launcher is distributed as a
/// portable exe, so config, skins, webview cache and Minecraft live beside it.
/// If that directory is not writable, fall back to the roaming data directory.
pub fn portable_root() -> Option<&'static PathBuf> {
    static ROOT: OnceLock<Option<PathBuf>> = OnceLock::new();
    ROOT.get_or_init(|| {
        if cfg!(test) {
            return None;
        }
        if std::env::var("CREEPER_PORTABLE").as_deref() == Ok("0") {
            return None;
        }
        let exe = std::env::current_exe().ok()?;
        let dir = exe.parent()?.to_path_buf();
        let forced = std::env::var("CREEPER_PORTABLE").as_deref() == Ok("1")
            || dir.join("CreeperCore.portable").exists();
        if !forced {
            let probe = dir.join(".creeper-portable-probe");
            if fs::write(&probe, b"").is_err() {
                return None;
            }
            let _ = fs::remove_file(&probe);
        }
        Some(dir)
    })
    .as_ref()
}

pub fn config_dir() -> PathBuf {
    portable_root()
        .cloned()
        .map(|root| root.join("CreeperCore"))
        .unwrap_or_else(|| {
            dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("CreeperCore")
        })
}

pub fn default_config_path() -> PathBuf {
    config_dir().join("config.json")
}

pub fn default_game_root() -> PathBuf {
    if let Ok(value) = std::env::var("CREEPER_GAME_DIR") {
        if !value.trim().is_empty() {
            return PathBuf::from(value);
        }
    }
    if let Some(root) = portable_root() {
        return root.join(".minecraft");
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        let vanilla = PathBuf::from(appdata).join(".minecraft");
        if vanilla.exists() {
            return vanilla;
        }
    }
    config_dir().join("minecraft")
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct VersionSettings {
    /// None = inherit the global isolation setting.
    pub isolate: Option<bool>,
    pub memory_mb: Option<u32>,
    pub java_path: Option<String>,
    pub jvm_args: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    pub config_version: u32,
    pub game_dir: String,
    pub selected_version: Option<String>,
    pub accounts: Vec<AccountProfile>,
    pub active_account_id: Option<String>,
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
    pub version_settings: HashMap<String, VersionSettings>,
    pub close_after_launch: bool,
    pub minimize_on_launch: bool,
    pub background_animation: bool,
    pub show_log_on_crash: bool,
    pub auto_backup_before_launch: bool,
    pub last_page: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        let root = default_game_root();
        Self {
            config_version: 4,
            game_dir: root.to_string_lossy().to_string(),
            selected_version: None,
            active_account_id: None,
            accounts: Vec::new(),
            java_path: None,
            memory_mb: DEFAULT_MEMORY_MB,
            jvm_args: String::new(),
            width: 854,
            height: 480,
            fullscreen: false,
            download_source: "official".to_string(),
            community_source: "official".to_string(),
            download_concurrency: 16,
            isolate_versions: false,
            version_settings: HashMap::new(),
            close_after_launch: false,
            minimize_on_launch: false,
            background_animation: true,
            show_log_on_crash: true,
            auto_backup_before_launch: false,
            last_page: "home".to_string(),
        }
    }
}

impl AppConfig {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            let mut config = Self::default();
            config.normalize();
            return Ok(config);
        }
        let raw = fs::read_to_string(path)
            .with_context(|| format!("无法读取配置：{}", path.display()))?;
        // PowerShell and some editors write a UTF-8 BOM; serde_json does not
        // accept it even though the JSON itself is valid.
        let raw = raw.trim_start_matches('\u{feff}');
        match serde_json::from_str::<Self>(raw) {
            Ok(mut config) => {
                config.normalize();
                Ok(config)
            }
            Err(error) => {
                // Never prevent the launcher from opening because of a damaged
                // config. Preserve the original for inspection, then fall back.
                let backup = path.with_extension(format!(
                    "corrupt-{}.json",
                    chrono::Utc::now().format("%Y%m%d-%H%M%S")
                ));
                let _ = fs::copy(path, &backup);
                let mut config = Self::default();
                config.normalize();
                let _ = config.save(path);
                eprintln!(
                    "config parse failed ({error}); backed up to {}",
                    backup.display()
                );
                Ok(config)
            }
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("无法创建配置目录：{}", parent.display()))?;
        }
        let temp = path.with_extension("json.tmp");
        let raw = serde_json::to_string_pretty(self)?;
        fs::write(&temp, raw).with_context(|| format!("无法写入配置：{}", temp.display()))?;
        if path.exists() {
            let _ = fs::remove_file(path);
        }
        fs::rename(&temp, path).with_context(|| format!("无法保存配置：{}", path.display()))?;
        Ok(())
    }

    pub fn normalize(&mut self) {
        if self.game_dir.trim().is_empty() {
            self.game_dir = default_game_root().to_string_lossy().to_string();
        }
        self.memory_mb = self.memory_mb.clamp(MIN_MEMORY_MB, MAX_MEMORY_MB);
        self.download_concurrency = self.download_concurrency.clamp(1, 64);
        if self.width < 640 || self.width > 7680 {
            self.width = 854;
        }
        if self.height < 480 || self.height > 4320 {
            self.height = 480;
        }
        match self.download_source.as_str() {
            "official" | "bmclapi" => {}
            _ => self.download_source = "official".to_string(),
        }
        if !matches!(self.community_source.as_str(), "official" | "mcim") {
            self.community_source = "official".to_string();
        }
        if self.config_version < 2 {
            // Older development builds seeded a "Miner" offline profile. Do
            // not carry that placeholder into a fresh player setup.
            if self.accounts.len() == 1
                && self.accounts[0].id == "local-miner"
                && self.accounts[0].username == "Miner"
                && self.accounts[0].kind.eq_ignore_ascii_case("offline")
            {
                self.accounts.clear();
                self.active_account_id = None;
            }
            self.config_version = 2;
        }
        if self.config_version < 3 {
            self.accounts
                .retain(|account| !account.kind.eq_ignore_ascii_case("authlib"));
            self.config_version = 3;
        }
        if self.config_version < 4 {
            // Earlier builds defaulted to 1280×720. Align untouched
            // defaults with Minecraft's own 854×480 window.
            if self.width == 1280 && self.height == 720 {
                self.width = 854;
                self.height = 480;
            }
            self.config_version = 4;
        }
        let active_exists = self
            .active_account_id
            .as_ref()
            .map(|id| self.accounts.iter().any(|account| &account.id == id))
            .unwrap_or(false);
        if !active_exists {
            self.active_account_id = self.accounts.first().map(|account| account.id.clone());
        }
        for account in &mut self.accounts {
            if account.kind.trim().is_empty() {
                account.kind = "offline".to_string();
            }
        }
    }

    pub fn root(&self) -> PathBuf {
        PathBuf::from(&self.game_dir)
    }

    pub fn active_account(&self) -> Option<&AccountProfile> {
        let id = self.active_account_id.as_ref()?;
        self.accounts.iter().find(|account| &account.id == id)
    }

    pub fn active_account_mut(&mut self) -> Option<&mut AccountProfile> {
        let id = self.active_account_id.clone()?;
        self.accounts.iter_mut().find(|account| account.id == id)
    }

    pub fn isolation_for(&self, version_id: &str) -> bool {
        self.version_settings
            .get(version_id)
            .and_then(|item| item.isolate)
            .unwrap_or(self.isolate_versions)
    }

    /// Game directory for a version. Version isolation gives every profile its
    /// own `versions/<id>/game` world, config and mods folder.
    pub fn game_dir_for(&self, version_id: &str) -> PathBuf {
        if self.isolation_for(version_id) {
            crate::util::sanitize_component(version_id)
                .map(|id| self.root().join("versions").join(id).join("game"))
                .unwrap_or_else(|| self.root().clone())
        } else {
            self.root()
        }
    }

    pub fn memory_for(&self, version_id: &str) -> u32 {
        self.version_settings
            .get(version_id)
            .and_then(|item| item.memory_mb)
            .unwrap_or(self.memory_mb)
            .clamp(MIN_MEMORY_MB, MAX_MEMORY_MB)
    }

    pub fn java_for(&self, version_id: &str) -> Option<String> {
        self.version_settings
            .get(version_id)
            .and_then(|item| item.java_path.clone())
            .filter(|value| !value.trim().is_empty())
            .or_else(|| self.java_path.clone())
    }

    pub fn jvm_args_for(&self, version_id: &str) -> String {
        self.version_settings
            .get(version_id)
            .and_then(|item| item.jvm_args.clone())
            .unwrap_or_else(|| self.jvm_args.clone())
    }

    pub fn set_game_dir(&mut self, path: &Path) {
        self.game_dir = path.to_string_lossy().to_string();
        self.selected_version = None;
    }

    pub fn touch_account(&mut self, account_id: &str) {
        if let Some(account) = self.accounts.iter_mut().find(|item| item.id == account_id) {
            account.last_used = Some(auth::now_iso());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_config_with_utf8_bom() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("config.json");
        let config = AppConfig::default();
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(serde_json::to_vec_pretty(&config).unwrap().as_slice());
        fs::write(&path, bytes).unwrap();
        let loaded = AppConfig::load(&path).unwrap();
        assert_eq!(loaded.memory_mb, config.memory_mb);
        assert!(loaded.accounts.is_empty());
        assert!(loaded.active_account_id.is_none());
    }

    #[test]
    fn corrupted_config_is_backed_up_and_replaced() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("config.json");
        fs::write(&path, b"{ definitely-not-json").unwrap();
        let loaded = AppConfig::load(&path).unwrap();
        assert!(loaded.accounts.is_empty());
        let backups = fs::read_dir(temp.path())
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().contains("corrupt"))
            .count();
        assert_eq!(backups, 1);
        assert!(serde_json::from_str::<AppConfig>(&fs::read_to_string(&path).unwrap()).is_ok());
    }

    #[test]
    fn removes_legacy_default_account_once() {
        let mut config = AppConfig::default();
        config.config_version = 1;
        config.accounts = vec![AccountProfile {
            id: "local-miner".to_string(),
            username: "Miner".to_string(),
            uuid: "00000000-0000-0000-0000-000000000000".to_string(),
            kind: "offline".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            last_used: None,
            access_token: None,
            refresh_token: None,
            expires_at: None,
        }];
        config.active_account_id = Some("local-miner".to_string());
        config.normalize();
        assert_eq!(config.config_version, 4);
        assert!(config.accounts.is_empty());
        assert!(config.active_account_id.is_none());
    }

    #[test]
    fn migrates_old_default_window_size() {
        let mut config = AppConfig::default();
        config.config_version = 3;
        config.width = 1280;
        config.height = 720;
        config.normalize();
        assert_eq!((config.width, config.height), (854, 480));
        assert_eq!(config.config_version, 4);

        let mut custom = AppConfig::default();
        custom.config_version = 3;
        custom.width = 1600;
        custom.height = 900;
        custom.normalize();
        assert_eq!((custom.width, custom.height), (1600, 900));
    }

    #[test]
    fn removes_legacy_authlib_accounts() {
        let mut config = AppConfig::default();
        config.config_version = 2;
        config.accounts = vec![AccountProfile {
            id: "old-authlib".to_string(),
            username: "ExtUser".to_string(),
            uuid: "00000000-0000-0000-0000-000000000001".to_string(),
            kind: "authlib".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            last_used: None,
            access_token: Some("token".to_string()),
            refresh_token: None,
            expires_at: None,
        }];
        config.active_account_id = Some("old-authlib".to_string());
        config.normalize();
        assert!(config.accounts.is_empty());
        assert!(config.active_account_id.is_none());
        assert_eq!(config.config_version, 4);
    }
}
