use creeper_core::config::AppConfig;
use creeper_core::download::CancelFlag;
use creeper_core::model::CrashReport;
use serde::Serialize;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct InstallControl {
    pub task_id: String,
    pub label: String,
    pub cancel: CancelFlag,
}

pub struct GameSession {
    pub pid: u32,
    pub version_id: String,
    pub started_at_iso: String,
    pub log_path: String,
    pub stop_requested: Arc<std::sync::atomic::AtomicBool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameStatus {
    pub pid: u32,
    pub version_id: String,
    pub started_at: String,
    pub log_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveInstallState {
    pub task_id: String,
    pub label: String,
}

pub struct AppState {
    pub config_path: PathBuf,
    config: std::sync::RwLock<AppConfig>,
    pub http: reqwest::Client,
    pub install: tokio::sync::Mutex<Option<InstallControl>>,
    pub game: Mutex<Option<GameSession>>,
    pub logs: Mutex<VecDeque<String>>,
    pub last_crash: Mutex<Option<CrashReport>>,
}

impl AppState {
    pub fn new() -> anyhow::Result<Self> {
        let config_path = creeper_core::config::default_config_path();
        let config = AppConfig::load(&config_path)?;
        creeper_core::ensure_game_layout(&config.root())?;
        let http = creeper_core::download::default_client()?;
        Ok(Self {
            config_path,
            config: std::sync::RwLock::new(config),
            http,
            install: tokio::sync::Mutex::new(None),
            game: Mutex::new(None),
            logs: Mutex::new(VecDeque::new()),
            last_crash: Mutex::new(None),
        })
    }

    pub fn config(&self) -> AppConfig {
        self.config
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn mutate_config<F, T>(&self, action: F) -> anyhow::Result<T>
    where
        F: FnOnce(&mut AppConfig) -> T,
    {
        let mut guard = self.config.write().unwrap_or_else(|e| e.into_inner());
        let result = action(&mut guard);
        guard.normalize();
        guard.save(&self.config_path)?;
        Ok(result)
    }

    pub fn save_config(&self, config: &AppConfig) -> anyhow::Result<()> {
        let mut guard = self.config.write().unwrap_or_else(|e| e.into_inner());
        *guard = config.clone();
        guard.normalize();
        guard.save(&self.config_path)
    }

    pub fn game_status(&self) -> Option<GameStatus> {
        let guard = self.game.lock().unwrap_or_else(|e| e.into_inner());
        guard.as_ref().map(|session| GameStatus {
            pid: session.pid,
            version_id: session.version_id.clone(),
            started_at: session.started_at_iso.clone(),
            log_path: session.log_path.clone(),
        })
    }

    pub async fn async_install_state(&self) -> Option<ActiveInstallState> {
        self.install
            .lock()
            .await
            .as_ref()
            .map(|item| ActiveInstallState {
                task_id: item.task_id.clone(),
                label: item.label.clone(),
            })
    }

    pub fn clear_logs(&self) {
        self.logs.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }

    pub fn push_log(&self, line: &str) {
        let mut logs = self.logs.lock().unwrap_or_else(|e| e.into_inner());
        logs.push_back(line.to_string());
        while logs.len() > 500 {
            logs.pop_front();
        }
    }

    pub fn log_tail(&self, limit: usize) -> Vec<String> {
        let logs = self.logs.lock().unwrap_or_else(|e| e.into_inner());
        let start = logs.len().saturating_sub(limit);
        logs.iter().skip(start).cloned().collect()
    }

    pub fn set_last_crash(&self, report: CrashReport) {
        *self.last_crash.lock().unwrap_or_else(|e| e.into_inner()) = Some(report);
    }

    pub fn last_crash(&self) -> Option<CrashReport> {
        self.last_crash
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}
