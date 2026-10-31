//! Forge and NeoForge client installers.
//!
//! Both projects ship an official installer jar. There is no supported "write
//! a version JSON directly" API for client installs, so we download the
//! official installer, run its `--installClient` entry point against the
//! selected game root, then discover the profile it created. This keeps the
//! library/processor logic owned by the loader projects instead of
//! reimplementing it incorrectly in the launcher.

use crate::{util, version};
use anyhow::{anyhow, bail, Context, Result};
use reqwest::Client;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const FORGE_API: &str = "https://bmclapi2.bangbang93.com/forge";
const FORGE_DOWNLOAD: &str = "https://bmclapi2.bangbang93.com/forge/download";
const NEOFORGE_MAVEN: &str =
    "https://bmclapi2.bangbang93.com/maven/net/neoforged/neoforge";

#[derive(Debug, Clone)]
pub struct LoaderInstallResult {
    pub version_id: String,
    pub loader: String,
    pub installer_version: String,
    pub output_tail: String,
}

#[derive(Debug, Deserialize)]
struct ForgeEntry {
    version: String,
}

fn version_rank(value: &str) -> Vec<u64> {
    value
        .split(['.', '-', '+'])
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect()
}

fn is_prerelease(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("beta") || lower.contains("alpha") || lower.contains("-pre")
}

fn latest_version(values: impl IntoIterator<Item = String>) -> Option<String> {
    values
        .into_iter()
        .filter(|value| !is_prerelease(value))
        .max_by(|a, b| version_rank(a).cmp(&version_rank(b)))
}

pub fn neoforge_prefix(minecraft: &str) -> String {
    // NeoForge used the Forge 47.x line for 1.20.1, then switched to a
    // Minecraft-derived version prefix (1.21.1 -> 21.1, 26.2 -> 26.2).
    if minecraft == "1.20.1" {
        "47.1.".to_string()
    } else {
        format!("{}.", minecraft.strip_prefix("1.").unwrap_or(minecraft))
    }
}

fn parse_neoforge_versions(xml: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<version>") {
        rest = &rest[start + "<version>".len()..];
        let Some(end) = rest.find("</version>") else {
            break;
        };
        result.push(rest[..end].trim().to_string());
        rest = &rest[end + "</version>".len()..];
    }
    result
}

async fn download_installer(client: &Client, url: &str, cache_path: &Path) -> Result<PathBuf> {
    if tokio::fs::try_exists(cache_path).await.unwrap_or(false) {
        if tokio::fs::metadata(cache_path)
            .await
            .map(|meta| meta.len() > 1_000_000)
            .unwrap_or(false)
        {
            return Ok(cache_path.to_path_buf());
        }
    }
    let response = client
        .get(url)
        .timeout(Duration::from_secs(300))
        .send()
        .await
        .context("加载器安装器下载失败")?;
    if !response.status().is_success() {
        bail!("加载器安装器下载返回 HTTP {}", response.status());
    }
    let bytes = response.bytes().await.context("加载器安装器读取失败")?;
    if bytes.len() < 1_000_000 {
        bail!("加载器安装器文件不完整");
    }
    if let Some(parent) = cache_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let part = cache_path.with_extension("jar.part");
    tokio::fs::write(&part, &bytes).await?;
    tokio::fs::rename(&part, cache_path).await?;
    Ok(cache_path.to_path_buf())
}

async fn forge_versions(client: &Client, minecraft: &str) -> Result<Vec<String>> {
    let response = client
        .get(format!("{FORGE_API}/minecraft/{minecraft}"))
        .timeout(Duration::from_secs(25))
        .send()
        .await
        .context("无法获取 Forge 版本列表")?;
    if !response.status().is_success() {
        bail!("Forge 版本列表返回 HTTP {}", response.status());
    }
    let entries: Vec<ForgeEntry> = response.json().await.context("Forge 版本列表格式无效")?;
    Ok(entries.into_iter().map(|entry| entry.version).collect())
}

async fn neoforge_versions(client: &Client, minecraft: &str) -> Result<Vec<String>> {
    let response = client
        .get(format!("{NEOFORGE_MAVEN}/maven-metadata.xml"))
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .context("无法获取 NeoForge 版本列表")?;
    if !response.status().is_success() {
        bail!("NeoForge 版本列表返回 HTTP {}", response.status());
    }
    let xml = response.text().await.context("NeoForge 版本列表读取失败")?;
    let prefix = neoforge_prefix(minecraft);
    Ok(parse_neoforge_versions(&xml)
        .into_iter()
        .filter(|value| value.starts_with(&prefix))
        .collect())
}

fn ensure_launcher_profiles(root: &Path) -> Result<()> {
    let path = root.join("launcher_profiles.json");
    if path.exists() {
        return Ok(());
    }
    let profile = serde_json::json!({
        "profiles": {
            "creeper-core": {
                "name": "CreeperCore",
                "type": "custom",
                "created": "2026-10-30T00:00:00.000Z",
                "lastUsed": "2026-10-30T00:00:00.000Z"
            }
        },
        "settings": {},
        "version": 3
    });
    util::write_json_atomic_sync(&path, &profile).context("无法创建 launcher_profiles.json")
}

async fn run_installer(
    java: &Path,
    installer: &Path,
    root: &Path,
    label: &str,
) -> Result<String> {
    ensure_launcher_profiles(root)?;
    let java = java.to_path_buf();
    let installer = installer.to_path_buf();
    let root_buf = root.to_path_buf();
    let label = label.to_string();
    let task_label = label.clone();
    let result = tokio::time::timeout(
        Duration::from_secs(900),
        tokio::task::spawn_blocking(move || -> Result<String> {
            let output = Command::new(&java)
                .arg("-jar")
                .arg(&installer)
                .arg("--installClient")
                .arg(&root_buf)
                .current_dir(&root_buf)
                .output()
                .with_context(|| format!("无法启动 {task_label} 安装器"))?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let tail = format!("{stdout}\n{stderr}")
                .lines()
                .rev()
                .take(24)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join("\n");
            if !output.status.success() {
                bail!("{task_label} 安装器退出失败：{tail}");
            }
            Ok(tail)
        }),
    )
    .await;
    match result {
        Ok(Ok(Ok(tail))) => Ok(tail),
        Ok(Ok(Err(err))) => Err(err),
        Ok(Err(err)) => Err(err.into()),
        Err(_) => bail!("{label} 安装器运行超时（15 分钟）"),
    }
}

fn find_installed_version(root: &Path, loader: &str, minecraft: &str) -> Result<String> {
    let summaries = version::scan_versions(root).context("无法扫描安装后的版本")?;
    summaries
        .into_iter()
        .filter(|item| item.loader.eq_ignore_ascii_case(loader))
        .filter(|item| item.inherits_from.as_deref() == Some(minecraft) || item.id.ends_with(minecraft))
        .max_by_key(|item| item.last_modified)
        .map(|item| item.id)
        .ok_or_else(|| anyhow!("{loader} 安装器没有生成可识别的版本配置"))
}

pub async fn install_forge(
    client: &Client,
    root: &Path,
    minecraft: &str,
    java: &Path,
) -> Result<LoaderInstallResult> {
    let versions = forge_versions(client, minecraft).await?;
    let forge = latest_version(versions)
        .ok_or_else(|| anyhow!("Forge 没有适用于 Minecraft {minecraft} 的版本"))?;
    let cache = root
        .join(".creeper")
        .join("loaders")
        .join(format!("forge-{minecraft}-{forge}-installer.jar"));
    let installer = download_installer(
        client,
        &format!(
            "{FORGE_DOWNLOAD}?mcversion={minecraft}&version={forge}&category=installer&format=jar"
        ),
        &cache,
    )
    .await?;
    let output = run_installer(java, &installer, root, "Forge").await?;
    let version_id = find_installed_version(root, "forge", minecraft)?;
    Ok(LoaderInstallResult {
        version_id,
        loader: "forge".to_string(),
        installer_version: forge,
        output_tail: output,
    })
}

pub async fn install_neoforge(
    client: &Client,
    root: &Path,
    minecraft: &str,
    java: &Path,
) -> Result<LoaderInstallResult> {
    let versions = neoforge_versions(client, minecraft).await?;
    let neoforge = latest_version(versions)
        .ok_or_else(|| anyhow!("NeoForge 没有适用于 Minecraft {minecraft} 的版本"))?;
    let cache = root
        .join(".creeper")
        .join("loaders")
        .join(format!("neoforge-{neoforge}-installer.jar"));
    let installer = download_installer(
        client,
        &format!("{NEOFORGE_MAVEN}/{neoforge}/neoforge-{neoforge}-installer.jar"),
        &cache,
    )
    .await?;
    let output = run_installer(java, &installer, root, "NeoForge").await?;
    let version_id = find_installed_version(root, "neoforge", minecraft)?;
    Ok(LoaderInstallResult {
        version_id,
        loader: "neoforge".to_string(),
        installer_version: neoforge,
        output_tail: output,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_numeric_loader_versions() {
        assert!(version_rank("52.1.20") > version_rank("52.1.9"));
        assert!(version_rank("21.1.265") > version_rank("21.1.99"));
        assert!(is_prerelease("21.1.265-beta"));
        assert!(!is_prerelease("21.1.265"));
    }

    #[test]
    fn maps_neoforge_prefixes() {
        assert_eq!(neoforge_prefix("1.20.1"), "47.1.");
        assert_eq!(neoforge_prefix("1.21.1"), "21.1.");
        assert_eq!(neoforge_prefix("26.2"), "26.2.");
    }

    #[test]
    fn parses_neoforge_metadata() {
        let versions = parse_neoforge_versions(
            "<metadata><versioning><versions><version>21.1.1</version><version>21.1.2</version></versions></versioning></metadata>",
        );
        assert_eq!(versions, vec!["21.1.1", "21.1.2"]);
    }
}
