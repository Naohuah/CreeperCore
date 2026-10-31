//! Java discovery, validation and compatibility selection.

use crate::model::JavaInfo;
use crate::util;
use anyhow::{anyhow, Result};
use regex::Regex;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn java_exe_name() -> &'static str {
    if cfg!(windows) {
        "java.exe"
    } else {
        "java"
    }
}

fn command_for(path: &Path) -> Command {
    let mut command = Command::new(path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
        .arg("-XshowSettings:properties")
        .arg("-version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

/// Inspect a concrete java executable. Returns `None` when it cannot run.
pub fn inspect_java(path: &Path, source: &str) -> Option<JavaInfo> {
    if !path.exists() && path.components().count() > 1 {
        return None;
    }
    let output = command_for(path).output().ok()?;
    let mut text = String::from_utf8_lossy(&output.stderr).to_string();
    text.push('\n');
    text.push_str(&String::from_utf8_lossy(&output.stdout));
    if output.status.code().unwrap_or(1) != 0 && !text.contains("version") {
        return None;
    }
    parse_java_output(&text, path, source)
}

fn parse_java_output(text: &str, path: &Path, source: &str) -> Option<JavaInfo> {
    let mut properties: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some((key, value)) = trimmed.split_once('=') {
            properties.insert(key.trim(), value.trim());
        }
    }
    let version_line = text
        .lines()
        .find(|line| line.to_ascii_lowercase().contains("version \""))
        .unwrap_or_default();
    let quoted = Regex::new(r#"version "([^"]+)""#)
        .ok()
        .and_then(|re| re.captures(version_line))
        .and_then(|captures| captures.get(1))
        .map(|m| m.as_str().to_string());
    let version = properties
        .get("java.version")
        .map(|v| v.to_string())
        .or(quoted)?;
    let major = parse_major(&version)?;
    let arch_raw = properties.get("os.arch").copied().unwrap_or("unknown");
    let architecture = match arch_raw {
        "amd64" | "x86_64" => "x86_64".to_string(),
        "aarch64" | "arm64" => "arm64".to_string(),
        "x86" | "i386" => "x86".to_string(),
        other => other.to_string(),
    };
    let data_model = properties.get("sun.arch.data.model").copied().unwrap_or("");
    let is_64bit = data_model == "64"
        || matches!(architecture.as_str(), "x86_64" | "arm64")
        || std::env::consts::ARCH.contains("64");
    let vendor = properties
        .get("java.vendor")
        .map(|v| v.trim_matches('"').to_string())
        .unwrap_or_else(|| "Unknown".to_string());
    Some(JavaInfo {
        path: path.to_string_lossy().to_string(),
        version,
        major,
        vendor,
        architecture,
        is_64bit,
        source: source.to_string(),
        recommended: false,
    })
}

pub fn parse_major(version: &str) -> Option<u32> {
    let first = version.split(['.', '-', '+']).next().unwrap_or(version);
    if first == "1" {
        version.split('.').nth(1)?.parse().ok()
    } else {
        first.parse().ok()
    }
}

fn candidate_paths(game_root: &Path, configured: Option<&str>) -> Vec<(PathBuf, String)> {
    let mut candidates: Vec<(PathBuf, String)> = Vec::new();
    let mut push = |path: PathBuf, source: &str| {
        if path.exists() || !path.is_absolute() {
            candidates.push((path, source.to_string()));
        }
    };

    if let Some(configured) = configured.filter(|value| !value.trim().is_empty()) {
        push(PathBuf::from(configured), "手动选择");
    }
    if let Some(home) = std::env::var_os("JAVA_HOME") {
        push(
            PathBuf::from(home).join("bin").join(java_exe_name()),
            "JAVA_HOME",
        );
    }

    // Minecraft's own runtime is the most version-correct choice.
    let mut runtime_roots = vec![game_root.join("runtime")];
    if let Some(appdata) = std::env::var_os("APPDATA") {
        let official = PathBuf::from(appdata).join(".minecraft").join("runtime");
        if official != runtime_roots[0] {
            runtime_roots.push(official);
        }
    }
    for runtime_root in runtime_roots {
        if !runtime_root.exists() {
            continue;
        }
        for entry in walkdir::WalkDir::new(&runtime_root)
            .max_depth(5)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.file_name().and_then(|n| n.to_str()) == Some(java_exe_name()) {
                push(path.to_path_buf(), "Minecraft 运行时");
            }
        }
    }

    let program_files = std::env::var_os("ProgramFiles").map(PathBuf::from);
    let program_files_x86 = std::env::var_os("ProgramFiles(x86)").map(PathBuf::from);
    let local_app_data = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let mut vendor_roots = Vec::new();
    for base in [program_files, program_files_x86].into_iter().flatten() {
        for vendor in [
            "Java",
            "Eclipse Adoptium",
            "Eclipse Foundation",
            "Microsoft",
            "Zulu",
            "BellSoft",
            "Amazon Corretto",
            "Semeru",
            "GraalVM",
        ] {
            vendor_roots.push(base.join(vendor));
        }
    }
    if let Some(base) = local_app_data {
        for vendor in ["Programs\\Eclipse Adoptium", "Programs\\Microsoft\\jdk"] {
            vendor_roots.push(base.join(vendor));
        }
    }
    for root in vendor_roots {
        if !root.exists() {
            continue;
        }
        if root.join("bin").join(java_exe_name()).exists() {
            push(root.join("bin").join(java_exe_name()), "系统安装");
        }
        if let Ok(entries) = std::fs::read_dir(&root) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path().join("bin").join(java_exe_name());
                if path.exists() {
                    push(path, "系统安装");
                }
            }
        }
    }

    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let java = dir.join(java_exe_name());
            if java.exists() {
                push(java, "PATH");
            }
        }
    }
    // "java" itself might be a shim only discoverable by name.
    candidates.push((PathBuf::from(java_exe_name()), "PATH".to_string()));
    candidates
}

/// Discover every runnable Java on the machine, de-duplicated and sorted by
/// descending major version. This intentionally avoids spawning more than a few
/// dozen processes: each candidate is validated once.
pub fn discover_java(game_root: &Path, configured: Option<&str>) -> Vec<JavaInfo> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for (path, source) in candidate_paths(game_root, configured) {
        let key = util::normalize_path_key(&path);
        if !seen.insert(key) {
            continue;
        }
        if result.len() >= 40 {
            break;
        }
        if let Some(info) = inspect_java(&path, &source) {
            result.push(info);
        }
    }
    result.sort_by(|a, b| {
        b.major
            .cmp(&a.major)
            .then_with(|| a.path.to_lowercase().cmp(&b.path.to_lowercase()))
    });
    result
}

/// Prefer the lowest Java that satisfies the version requirement. Using the
/// newest JVM can break old mod loaders, so "newest" is deliberately not the
/// default.
pub fn select_best(javas: &[JavaInfo], required: Option<u32>) -> Option<JavaInfo> {
    let required = required.unwrap_or(8);
    let mut eligible: Vec<&JavaInfo> = javas.iter().filter(|j| j.major >= required).collect();
    eligible.sort_by_key(|j| j.major);
    if let Some(java) = eligible.first() {
        return Some((*java).clone());
    }
    javas.first().cloned()
}

pub fn validate_java_for_version(path: &Path, required: Option<u32>) -> Result<JavaInfo> {
    let info = inspect_java(path, "启动前检查")
        .ok_or_else(|| anyhow!("无法执行 Java：{}", path.display()))?;
    if let Some(required) = required {
        if info.major < required {
            return Err(anyhow!(
                "此版本需要 Java {required} 或更高，当前为 Java {}",
                info.major
            ));
        }
    }
    Ok(info)
}

pub fn java_path_from_root(root: &Path) -> Option<PathBuf> {
    for candidate in [
        root.join("bin").join(java_exe_name()),
        root.join("jre").join("bin").join(java_exe_name()),
    ] {
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_modern_and_legacy_versions() {
        assert_eq!(parse_major("25.0.1"), Some(25));
        assert_eq!(parse_major("1.8.0_402"), Some(8));
        assert_eq!(parse_major("17-ea"), Some(17));
        assert_eq!(parse_major("not-a-version"), None);
    }

    #[test]
    fn parses_java_properties() {
        let text = r#"openjdk version "21.0.2" 2024-01-16
Property settings:
    java.version = 21.0.2
    java.vendor = Eclipse Adoptium
    os.arch = amd64
    sun.arch.data.model = 64
"#;
        let info = parse_java_output(text, Path::new("java.exe"), "test").unwrap();
        assert_eq!(info.major, 21);
        assert_eq!(info.vendor, "Eclipse Adoptium");
        assert!(info.is_64bit);
    }

    #[test]
    fn picks_lowest_compatible_java() {
        let mk = |major| JavaInfo {
            major,
            path: format!("java{major}"),
            ..Default::default()
        };
        let javas = vec![mk(25), mk(17), mk(21)];
        assert_eq!(select_best(&javas, Some(17)).unwrap().major, 17);
        assert_eq!(select_best(&javas, Some(20)).unwrap().major, 21);
        assert_eq!(select_best(&javas, Some(30)).unwrap().major, 25);
    }
}
