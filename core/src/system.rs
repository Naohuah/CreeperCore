use crate::model::SystemInfo;
use sysinfo::System;

pub fn system_info() -> SystemInfo {
    let mut system = System::new();
    system.refresh_memory();
    let to_mb = |bytes: u64| bytes / 1024 / 1024;
    SystemInfo {
        os: os_name(),
        arch: std::env::consts::ARCH.to_string(),
        cpu_threads: std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1),
        total_memory_mb: to_mb(system.total_memory()),
        used_memory_mb: to_mb(system.used_memory()),
        launcher_version: crate::model::LAUNCHER_VERSION.to_string(),
    }
}

pub fn total_memory_mb() -> u64 {
    let mut system = System::new();
    system.refresh_memory();
    system.total_memory() / 1024 / 1024
}

fn os_name() -> String {
    match std::env::consts::OS {
        "windows" => "Windows".to_string(),
        "macos" => "macOS".to_string(),
        "linux" => "Linux".to_string(),
        other => other.to_string(),
    }
}
