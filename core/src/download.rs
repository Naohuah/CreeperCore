//! Parallel, checksum-verified installer with content-addressed asset storage.
//!
//! This is the heart of Creeper Core's "TNT Cache" pipeline: every file is
//! staged to a `.part` file, SHA-1 verified, then atomically promoted. Assets
//! are deduplicated by Mojang's content hash, so re-installs and repairs only
//! fetch bytes that are actually missing.

use crate::model::*;
use crate::util;
use crate::version::{self, LibraryKind, Platform};
use anyhow::{anyhow, bail, Context, Result};
use futures_util::{stream, StreamExt};
use serde::Deserialize;
use sha1::{Digest, Sha1};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::io::AsyncWriteExt;

pub const CANCELLED: &str = "__CREEPER_CANCELLED__";
const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const FABRIC_META: &str = "https://meta.fabricmc.net/v2";
const FABRIC_META_MIRROR: &str = "https://bmclapi2.bangbang93.com/fabric-meta/v2";

pub type ProgressFn = Arc<dyn Fn(InstallProgress) + Send + Sync>;

pub fn is_cancelled(err: &anyhow::Error) -> bool {
    err.to_string().contains(CANCELLED)
}

pub fn noop_progress() -> ProgressFn {
    Arc::new(|_| {})
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadSource {
    Official,
    BmclApi,
}

impl DownloadSource {
    pub fn parse(value: &str) -> Self {
        if value.eq_ignore_ascii_case("bmclapi") {
            Self::BmclApi
        } else {
            Self::Official
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Official => "official",
            Self::BmclApi => "bmclapi",
        }
    }
}

#[derive(Debug, Clone)]
pub struct DownloadOptions {
    pub source: DownloadSource,
    pub concurrency: usize,
    pub include_assets: bool,
}

impl Default for DownloadOptions {
    fn default() -> Self {
        Self {
            source: DownloadSource::Official,
            concurrency: 16,
            include_assets: true,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CancelFlag(Arc<AtomicBool>);

impl CancelFlag {
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            bail!(CANCELLED);
        }
        Ok(())
    }
}

pub fn default_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(format!(
            "{}/{} (CreeperCore; +https://github.com/creeper-core)",
            LAUNCHER_NAME, LAUNCHER_VERSION
        ))
        .connect_timeout(std::time::Duration::from_secs(20))
        .timeout(std::time::Duration::from_secs(120))
        .pool_max_idle_per_host(32)
        .build()
        .context("无法初始化网络客户端")
}

/// Mirror translation. Official endpoints are always the default; BMCLAPI is
/// kept as a fallback for networks where Mojang's CDN is slow.
pub fn map_url(source: DownloadSource, url: &str) -> String {
    if source == DownloadSource::Official {
        // FabricMC's Maven CDN is effectively unusable on some mainland China
        // networks (it accepts the connection, then stalls). Route only those
        // artifacts through the verified BMCLAPI Maven mirror.
        if let Some(rest) = url.strip_prefix("https://maven.fabricmc.net/") {
            return format!("https://bmclapi2.bangbang93.com/maven/{rest}");
        }
        return url.to_string();
    }
    let mut mapped = url.to_string();
    mapped = mapped.replace(
        "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json",
        "https://bmclapi2.bangbang93.com/mc/game/version_manifest_v2.json",
    );
    mapped = mapped.replace(
        "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json",
        "https://bmclapi2.bangbang93.com/mc/game/version_manifest_v2.json",
    );
    if let Some(rest) = url.strip_prefix("https://piston-meta.mojang.com/v1/packages/") {
        mapped = format!("https://bmclapi2.bangbang93.com/v1/packages/{rest}");
    }
    if let Some(rest) = url.strip_prefix("https://piston-data.mojang.com/v1/objects/") {
        mapped = format!("https://bmclapi2.bangbang93.com/v1/objects/{rest}");
    }
    if let Some(rest) = url.strip_prefix("https://resources.download.minecraft.net/") {
        mapped = format!("https://bmclapi2.bangbang93.com/assets/{rest}");
    }
    if let Some(rest) = url.strip_prefix("https://libraries.minecraft.net/") {
        mapped = format!("https://bmclapi2.bangbang93.com/maven/{rest}");
    }
    if let Some(rest) = url.strip_prefix("https://maven.fabricmc.net/") {
        mapped = format!("https://bmclapi2.bangbang93.com/maven/{rest}");
    }
    mapped
}

fn fabric_bases(source: DownloadSource) -> [&'static str; 2] {
    if source == DownloadSource::BmclApi {
        [FABRIC_META_MIRROR, FABRIC_META]
    } else {
        // The Fabric meta host is unreachable on some networks (notably mainland
        // China). Try it briefly, then fall back to the BMCLAPI mirror.
        [FABRIC_META, FABRIC_META_MIRROR]
    }
}

async fn fabric_get_json<T: serde::de::DeserializeOwned>(
    client: &reqwest::Client,
    path: &str,
    source: DownloadSource,
) -> Result<T> {
    let mut last_error = None;
    for (index, base) in fabric_bases(source).into_iter().enumerate() {
        let url = format!("{base}{path}");
        let request = client
            .get(&url)
            .timeout(std::time::Duration::from_secs(if index == 0 {
                10
            } else {
                25
            }))
            .send()
            .await;
        match request {
            Ok(response) if response.status().is_success() => {
                return response.json::<T>().await.context("Fabric 元数据格式无效");
            }
            Ok(response) => {
                last_error = Some(anyhow!("HTTP {}：{url}", response.status()));
            }
            Err(err) => last_error = Some(anyhow!("{err}")),
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow!("Fabric 元数据不可用")))
}

async fn send_with_retry(
    client: &reqwest::Client,
    url: &str,
    attempts: usize,
    cancel: &CancelFlag,
) -> Result<reqwest::Response> {
    let mut last_error = None;
    for attempt in 0..attempts {
        cancel.check()?;
        match client.get(url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    return Ok(response);
                }
                last_error = Some(anyhow!("HTTP {}：{}", response.status(), url));
            }
            Err(err) => last_error = Some(anyhow!("{err}")),
        }
        if attempt + 1 < attempts {
            tokio::time::sleep(std::time::Duration::from_millis(350 * (attempt as u64 + 1))).await;
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow!("网络请求失败")))
}

pub async fn fetch_manifest(
    client: &reqwest::Client,
    source: DownloadSource,
) -> Result<VersionManifest> {
    let url = map_url(source, MANIFEST_URL);
    let response = send_with_retry(client, &url, 3, &CancelFlag::new()).await?;
    let manifest: VersionManifest = response.json().await.context("版本清单格式无效")?;
    if manifest.versions.is_empty() {
        bail!("官方版本清单为空");
    }
    Ok(manifest)
}

async fn fetch_bytes(
    client: &reqwest::Client,
    source: DownloadSource,
    url: &str,
    cancel: &CancelFlag,
) -> Result<Vec<u8>> {
    let mapped = map_url(source, url);
    let response = send_with_retry(client, &mapped, 3, cancel).await?;
    let bytes = response.bytes().await.context("读取响应失败")?;
    Ok(bytes.to_vec())
}

async fn fetch_remote_version_json(
    client: &reqwest::Client,
    source: DownloadSource,
    remote: &RemoteVersion,
    cancel: &CancelFlag,
) -> Result<VersionJson> {
    let bytes = fetch_bytes(client, source, &remote.url, cancel).await?;
    if remote.sha1.len() == 40 {
        let actual = hex::encode(Sha1::digest(&bytes));
        if !actual.eq_ignore_ascii_case(&remote.sha1) {
            bail!("版本 JSON 校验失败（{}）", remote.id);
        }
    }
    serde_json::from_slice(&bytes).context("版本 JSON 无法解析")
}

async fn fetch_fabric_profile(
    client: &reqwest::Client,
    game_version: &str,
    loader_version: &str,
    source: DownloadSource,
) -> Result<(String, VersionJson)> {
    let path = format!("/versions/loader/{game_version}/{loader_version}/profile/json");
    let json: VersionJson = fabric_get_json(client, &path, source).await?;
    let id = json.id.clone();
    Ok((id, json))
}

pub async fn fetch_fabric_loaders(
    client: &reqwest::Client,
    game_version: &str,
    source: DownloadSource,
) -> Result<Vec<FabricLoaderVersion>> {
    #[derive(Deserialize)]
    struct Entry {
        loader: Loader,
    }
    #[derive(Deserialize)]
    struct Loader {
        version: String,
        stable: bool,
    }
    let path = format!("/versions/loader/{game_version}");
    let entries: Vec<Entry> = fabric_get_json(client, &path, source).await?;
    Ok(entries
        .into_iter()
        .map(|entry| FabricLoaderVersion {
            version: entry.loader.version,
            stable: entry.loader.stable,
        })
        .collect())
}

#[derive(Debug, Clone)]
struct DownloadJob {
    url: String,
    dest: PathBuf,
    sha1: Option<String>,
    size: Option<u64>,
    verify_sha1: bool,
    label: String,
}

#[derive(Debug, Clone, Copy, Default)]
struct PhaseStats {
    downloaded: usize,
    reused: usize,
    bytes: u64,
}

#[derive(Clone)]
struct PhaseEmitter {
    task_id: String,
    phase: String,
    label: String,
    total: usize,
    bytes_total: u64,
    current: Arc<AtomicUsize>,
    downloaded: Arc<AtomicUsize>,
    reused: Arc<AtomicUsize>,
    bytes_done: Arc<AtomicU64>,
    bytes_downloaded: Arc<AtomicU64>,
    callback: ProgressFn,
    last_emit_ms: Arc<AtomicU64>,
}

impl PhaseEmitter {
    fn new(
        task_id: &str,
        phase: &str,
        label: &str,
        total: usize,
        bytes_total: u64,
        callback: ProgressFn,
        _cancel: CancelFlag,
    ) -> Self {
        Self {
            task_id: task_id.to_string(),
            phase: phase.to_string(),
            label: label.to_string(),
            total,
            bytes_total,
            current: Arc::new(AtomicUsize::new(0)),
            downloaded: Arc::new(AtomicUsize::new(0)),
            reused: Arc::new(AtomicUsize::new(0)),
            bytes_done: Arc::new(AtomicU64::new(0)),
            bytes_downloaded: Arc::new(AtomicU64::new(0)),
            callback,
            last_emit_ms: Arc::new(AtomicU64::new(0)),
        }
    }

    fn percent(&self) -> f64 {
        let by_bytes = if self.bytes_total > 0 {
            self.bytes_done.load(Ordering::Relaxed) as f64 / self.bytes_total as f64
        } else if self.total > 0 {
            self.current.load(Ordering::Relaxed) as f64 / self.total as f64
        } else {
            0.0
        };
        (by_bytes * 100.0).clamp(0.0, 100.0)
    }

    fn emit(&self) {
        let current = self.current.load(Ordering::Relaxed);
        let message = if self.total > 0 {
            format!("{}  {current}/{}", self.label, self.total)
        } else {
            self.label.clone()
        };
        (self.callback)(InstallProgress {
            task_id: self.task_id.clone(),
            phase: self.phase.clone(),
            label: self.label.clone(),
            current,
            total: self.total,
            bytes_done: self.bytes_done.load(Ordering::Relaxed),
            bytes_total: self.bytes_total,
            percent: self.percent(),
            message,
        });
    }

    fn add_bytes(&self, bytes: u64) {
        self.bytes_done.fetch_add(bytes, Ordering::Relaxed);
        let now = util::now_millis();
        let previous = self.last_emit_ms.load(Ordering::Relaxed);
        if now.saturating_sub(previous) >= 120
            && self
                .last_emit_ms
                .compare_exchange(previous, now, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
        {
            self.emit();
        }
    }

    fn complete_download(&self, size: u64, bytes: u64, reused: bool) {
        if reused {
            self.reused.fetch_add(1, Ordering::Relaxed);
            self.bytes_done.fetch_add(size, Ordering::Relaxed);
        } else {
            self.downloaded.fetch_add(1, Ordering::Relaxed);
            self.bytes_downloaded.fetch_add(bytes, Ordering::Relaxed);
        }
        self.current.fetch_add(1, Ordering::Relaxed);
        self.emit();
    }

    fn stats(&self) -> PhaseStats {
        PhaseStats {
            downloaded: self.downloaded.load(Ordering::Relaxed),
            reused: self.reused.load(Ordering::Relaxed),
            bytes: self.bytes_downloaded.load(Ordering::Relaxed),
        }
    }
}

async fn download_one(
    client: &reqwest::Client,
    job: &DownloadJob,
    source: DownloadSource,
    cancel: &CancelFlag,
    emitter: Option<&PhaseEmitter>,
) -> Result<(u64, bool)> {
    cancel.check()?;
    if let Some(parent) = job.dest.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("无法创建目录：{}", parent.display()))?;
    }
    let expected_sha1 = if job.verify_sha1 {
        job.sha1.as_deref()
    } else {
        None
    };
    if util::file_matches(&job.dest, job.size, expected_sha1).await {
        return Ok((job.size.unwrap_or(0), true));
    }

    let part = job.dest.with_extension("part");
    let mut last_error: Option<anyhow::Error> = None;
    for attempt in 0..3 {
        cancel.check()?;
        if part.exists() {
            let _ = tokio::fs::remove_file(&part).await;
        }
        let url = map_url(source, &job.url);
        let response = match send_with_retry(client, &url, 1, cancel).await {
            Ok(response) => response,
            Err(err) => {
                last_error = Some(err);
                tokio::time::sleep(std::time::Duration::from_millis(500 * (attempt + 1))).await;
                continue;
            }
        };
        let mut stream = response.bytes_stream();
        let mut file = match tokio::fs::File::create(&part).await {
            Ok(file) => file,
            Err(err) => {
                last_error = Some(anyhow!(err));
                break;
            }
        };
        let mut written = 0u64;
        let mut stream_error = None;
        while let Some(chunk) = stream.next().await {
            if let Err(_cancelled) = cancel.check() {
                drop(file);
                let _ = tokio::fs::remove_file(&part).await;
                return Err(anyhow!(CANCELLED));
            }
            match chunk {
                Ok(chunk) => {
                    if let Err(err) = file.write_all(&chunk).await {
                        stream_error = Some(anyhow!(err));
                        break;
                    }
                    written += chunk.len() as u64;
                    if let Some(emitter) = emitter {
                        emitter.add_bytes(chunk.len() as u64);
                    }
                }
                Err(err) => {
                    stream_error = Some(anyhow!(err));
                    break;
                }
            }
        }
        if let Err(err) = file.flush().await {
            stream_error = Some(anyhow!(err));
        }
        drop(file);
        if let Some(err) = stream_error {
            last_error = Some(err);
            let _ = tokio::fs::remove_file(&part).await;
            tokio::time::sleep(std::time::Duration::from_millis(400 * (attempt + 1))).await;
            continue;
        }
        if let Some(size) = job.size {
            if written != size {
                last_error = Some(anyhow!(
                    "{} 大小校验失败：期望 {size}，实际 {written}",
                    job.label
                ));
                let _ = tokio::fs::remove_file(&part).await;
                continue;
            }
        }
        if job.verify_sha1 {
            if let Some(expected) = job.sha1.as_deref().filter(|s| s.len() == 40) {
                let actual = util::sha1_file_async(&part).await?;
                if !actual.eq_ignore_ascii_case(expected) {
                    last_error = Some(anyhow!("{} SHA-1 校验失败", job.label));
                    let _ = tokio::fs::remove_file(&part).await;
                    continue;
                }
            }
        }
        if tokio::fs::try_exists(&job.dest).await.unwrap_or(false) {
            let _ = tokio::fs::remove_file(&job.dest).await;
        }
        tokio::fs::rename(&part, &job.dest)
            .await
            .with_context(|| format!("无法写入 {}", job.dest.display()))?;
        return Ok((written, false));
    }
    Err(last_error.unwrap_or_else(|| anyhow!("下载失败：{}", job.label)))
}

async fn download_jobs(
    client: &reqwest::Client,
    jobs: Vec<DownloadJob>,
    source: DownloadSource,
    concurrency: usize,
    task_id: &str,
    phase: &str,
    label: &str,
    callback: ProgressFn,
    cancel: CancelFlag,
) -> Result<PhaseStats> {
    if jobs.is_empty() {
        return Ok(PhaseStats::default());
    }
    let bytes_total = jobs.iter().filter_map(|job| job.size).sum::<u64>();
    let emitter = PhaseEmitter::new(
        task_id,
        phase,
        label,
        jobs.len(),
        bytes_total,
        callback,
        cancel.clone(),
    );
    emitter.emit();
    let concurrency = concurrency.clamp(1, 64);
    let results = stream::iter(jobs)
        .map(|job| {
            let client = client.clone();
            let emitter = emitter.clone();
            let cancel = cancel.clone();
            async move {
                let result = download_one(&client, &job, source, &cancel, Some(&emitter)).await;
                match result {
                    Ok((bytes, reused)) => {
                        emitter.complete_download(job.size.unwrap_or(bytes), bytes, reused);
                        Ok(())
                    }
                    Err(err) => Err(err),
                }
            }
        })
        .buffer_unordered(concurrency)
        .collect::<Vec<Result<()>>>()
        .await;
    for result in results {
        result?;
    }
    let stats = emitter.stats();
    emitter.emit();
    Ok(stats)
}

/// Download a remote Minecraft profile into `versions/<id>/<id>.json` and all of
/// its runtime dependencies.
pub async fn install_version(
    client: &reqwest::Client,
    root: &Path,
    version_id: &str,
    options: &DownloadOptions,
    task_id: &str,
    callback: ProgressFn,
    cancel: CancelFlag,
) -> Result<InstallReport> {
    let started = Instant::now();
    cancel.check()?;
    let emitter = PhaseEmitter::new(
        task_id,
        "resolve",
        "正在解析版本清单",
        0,
        0,
        callback.clone(),
        cancel.clone(),
    );
    emitter.emit();
    let manifest = fetch_manifest(client, options.source).await?;
    let remote = manifest
        .versions
        .iter()
        .find(|v| v.id == version_id)
        .cloned()
        .ok_or_else(|| anyhow!("版本清单中不存在 {version_id}"))?;
    let json = fetch_remote_version_json(client, options.source, &remote, &cancel).await?;
    let installed_id = util::safe_component(&json.id)?;
    if let Some(parent) = &json.inherits_from {
        if !version::version_json_path(root, parent).exists() {
            bail!("该版本依赖 {parent}，请先安装原版 {parent}");
        }
    }
    let json_path = version::version_json_path(root, &installed_id);
    util::write_json_atomic(&json_path, &json).await?;
    let resolved = version::resolve_version(root, &installed_id)?;
    let (downloaded, reused, bytes) = sync_resolved_files(
        client,
        root,
        &resolved,
        options,
        task_id,
        callback.clone(),
        cancel.clone(),
        started,
    )
    .await?;
    cancel.check()?;
    (callback)(InstallProgress {
        task_id: task_id.to_string(),
        phase: "done".to_string(),
        label: "安装完成".to_string(),
        current: 1,
        total: 1,
        bytes_done: bytes,
        bytes_total: bytes,
        percent: 100.0,
        message: format!("{installed_id} 已就绪"),
    });
    Ok(InstallReport {
        version_id: installed_id,
        files_downloaded: downloaded,
        files_reused: reused,
        bytes_downloaded: bytes,
        elapsed_ms: started.elapsed().as_millis() as u64,
    })
}

/// Repair an existing local profile: re-download missing/corrupt files without
/// touching worlds, mods or configuration.
pub async fn repair_version(
    client: &reqwest::Client,
    root: &Path,
    version_id: &str,
    options: &DownloadOptions,
    task_id: &str,
    callback: ProgressFn,
    cancel: CancelFlag,
) -> Result<InstallReport> {
    let started = Instant::now();
    cancel.check()?;
    let resolved = version::resolve_version(root, version_id)?;
    (callback)(InstallProgress {
        task_id: task_id.to_string(),
        phase: "resolve".to_string(),
        label: "正在检查核心文件".to_string(),
        current: 0,
        total: 1,
        bytes_done: 0,
        bytes_total: 0,
        percent: 0.0,
        message: format!("检查 {version_id}"),
    });
    let (downloaded, reused, bytes) = sync_resolved_files(
        client,
        root,
        &resolved,
        options,
        task_id,
        callback.clone(),
        cancel.clone(),
        started,
    )
    .await?;
    cancel.check()?;
    (callback)(InstallProgress {
        task_id: task_id.to_string(),
        phase: "done".to_string(),
        label: "修复完成".to_string(),
        current: 1,
        total: 1,
        bytes_done: bytes,
        bytes_total: bytes,
        percent: 100.0,
        message: "核心文件已校验".to_string(),
    });
    Ok(InstallReport {
        version_id: version_id.to_string(),
        files_downloaded: downloaded,
        files_reused: reused,
        bytes_downloaded: bytes,
        elapsed_ms: started.elapsed().as_millis() as u64,
    })
}

#[allow(clippy::too_many_arguments)]
async fn sync_resolved_files(
    client: &reqwest::Client,
    root: &Path,
    resolved: &ResolvedVersion,
    options: &DownloadOptions,
    task_id: &str,
    callback: ProgressFn,
    cancel: CancelFlag,
    _started: Instant,
) -> Result<(usize, usize, u64)> {
    let platform = Platform::current();
    let mut downloaded = 0usize;
    let mut reused = 0usize;
    let mut bytes = 0u64;

    // 1. Client jar.
    if let Some(artifact) = &resolved.client {
        let dest = version::client_jar_destination(root, resolved);
        let job = DownloadJob {
            url: artifact.url.clone(),
            dest: dest.clone(),
            sha1: artifact.sha1.clone(),
            size: artifact.size,
            verify_sha1: true,
            label: "客户端核心".to_string(),
        };
        let stats = download_jobs(
            client,
            vec![job],
            options.source,
            options.concurrency,
            task_id,
            "client",
            "正在下载客户端核心",
            callback.clone(),
            cancel.clone(),
        )
        .await?;
        downloaded += stats.downloaded;
        reused += stats.reused;
        bytes += stats.bytes;
    } else if !version::effective_client_jar(root, resolved).exists() {
        bail!("版本缺少客户端文件，且没有可下载的客户端核心");
    }

    // 2. Libraries.
    let mut jobs = Vec::new();
    let mut seen = HashSet::new();
    for library in &resolved.libraries {
        for file in version::library_files(library, &platform) {
            if file.kind == LibraryKind::Native {
                // Natives are extracted at launch time; still fetch them.
            }
            let dest = version::libraries_dir(root).join(&file.path);
            if !seen.insert(dest.clone()) {
                continue;
            }
            jobs.push(DownloadJob {
                url: file.url,
                dest,
                sha1: file.sha1,
                size: file.size,
                verify_sha1: true,
                label: library.name.clone(),
            });
        }
    }
    let stats = download_jobs(
        client,
        jobs,
        options.source,
        options.concurrency,
        task_id,
        "libraries",
        "正在同步运行库",
        callback.clone(),
        cancel.clone(),
    )
    .await?;
    downloaded += stats.downloaded;
    reused += stats.reused;
    bytes += stats.bytes;

    // 3. Asset index + objects.
    if options.include_assets {
        if let Some(index_ref) = &resolved.asset_index {
            let index_path = assets_index_path(root, &index_ref.id);
            let index = ensure_asset_index(
                client,
                root,
                index_ref,
                options,
                task_id,
                callback.clone(),
                cancel.clone(),
            )
            .await?;
            let mut asset_jobs = Vec::new();
            for (name, object) in &index.objects {
                if object.hash.len() < 2 {
                    continue;
                }
                let dest = asset_object_path(root, &object.hash);
                if asset_object_ok(&dest, object.size).await {
                    continue;
                }
                asset_jobs.push(DownloadJob {
                    url: format!(
                        "https://resources.download.minecraft.net/{}/{}",
                        &object.hash[..2],
                        object.hash
                    ),
                    dest,
                    sha1: Some(object.hash.clone()),
                    size: Some(object.size),
                    verify_sha1: false,
                    label: name.clone(),
                });
            }
            let total_assets = asset_jobs.len();
            if total_assets > 0 || !index_path.exists() {
                let stats = download_jobs(
                    client,
                    asset_jobs,
                    options.source,
                    options.concurrency,
                    task_id,
                    "assets",
                    "正在同步资源文件",
                    callback.clone(),
                    cancel.clone(),
                )
                .await?;
                downloaded += stats.downloaded;
                reused += stats.reused;
                bytes += stats.bytes;
            }
            if index.r#virtual || index.map_to_resources {
                materialize_legacy_assets(root, &index_ref.id, &index).await?;
            }
        }
    }

    // 4. Log4j configuration.
    if let Some(logging) = &resolved.logging {
        if let Some(log) = &logging.client {
            if let Some(file) = &log.file {
                let id = file
                    .id
                    .clone()
                    .or_else(|| file.sha1.clone())
                    .unwrap_or_else(|| "client-log4j.xml".to_string());
                let dest = root.join("assets").join("log_configs").join(id);
                let job = DownloadJob {
                    url: file.url.clone(),
                    dest,
                    sha1: file.sha1.clone(),
                    size: file.size,
                    verify_sha1: true,
                    label: "日志配置".to_string(),
                };
                let stats = download_jobs(
                    client,
                    vec![job],
                    options.source,
                    options.concurrency,
                    task_id,
                    "logging",
                    "正在同步日志配置",
                    callback.clone(),
                    cancel.clone(),
                )
                .await?;
                downloaded += stats.downloaded;
                reused += stats.reused;
                bytes += stats.bytes;
            }
        }
    }

    Ok((downloaded, reused, bytes))
}

#[derive(Debug, Clone, Deserialize)]
struct AssetObject {
    hash: String,
    size: u64,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct AssetIndex {
    #[serde(default)]
    objects: HashMap<String, AssetObject>,
    #[serde(default, rename = "virtual")]
    r#virtual: bool,
    #[serde(default)]
    map_to_resources: bool,
}

pub fn assets_index_path(root: &Path, id: &str) -> PathBuf {
    root.join("assets")
        .join("indexes")
        .join(format!("{id}.json"))
}

fn asset_object_path(root: &Path, hash: &str) -> PathBuf {
    root.join("assets")
        .join("objects")
        .join(&hash[..2])
        .join(hash)
}

async fn asset_object_ok(path: &Path, expected_size: u64) -> bool {
    tokio::fs::metadata(path)
        .await
        .map(|m| m.is_file() && m.len() == expected_size)
        .unwrap_or(false)
}

async fn ensure_asset_index(
    client: &reqwest::Client,
    root: &Path,
    index_ref: &AssetIndexRef,
    options: &DownloadOptions,
    task_id: &str,
    callback: ProgressFn,
    cancel: CancelFlag,
) -> Result<AssetIndex> {
    let path = assets_index_path(root, &index_ref.id);
    if path.exists() {
        if let Ok(raw) = tokio::fs::read(&path).await {
            if let Ok(index) = serde_json::from_slice::<AssetIndex>(&raw) {
                if index_ref
                    .sha1
                    .as_deref()
                    .map(|expected| {
                        expected.len() != 40
                            || hex::encode(Sha1::digest(&raw)).eq_ignore_ascii_case(expected)
                    })
                    .unwrap_or(true)
                {
                    return Ok(index);
                }
            }
        }
    }
    let bytes = fetch_bytes(client, options.source, &index_ref.url, &cancel).await?;
    if let Some(expected) = index_ref.sha1.as_deref().filter(|s| s.len() == 40) {
        if !hex::encode(Sha1::digest(&bytes)).eq_ignore_ascii_case(expected) {
            bail!("资源索引校验失败");
        }
    }
    let index: AssetIndex = serde_json::from_slice(&bytes).context("资源索引格式无效")?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&path, &bytes).await?;
    let _ = callback;
    let _ = task_id;
    Ok(index)
}

async fn materialize_legacy_assets(root: &Path, id: &str, index: &AssetIndex) -> Result<()> {
    let base = if index.map_to_resources {
        root.join("resources")
    } else {
        root.join("assets").join("virtual").join(id)
    };
    for (name, object) in &index.objects {
        let source = asset_object_path(root, &object.hash);
        if !source.exists() {
            continue;
        }
        let target = base.join(name);
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        if target.exists() {
            continue;
        }
        if tokio::fs::hard_link(&source, &target).await.is_err() {
            let _ = tokio::fs::copy(&source, &target).await;
        }
    }
    Ok(())
}

/// Install Fabric Loader for an already-installed game version. The Fabric
/// profile inherits from the vanilla profile, giving one shared asset/library
/// cache while keeping per-version mod folders isolated.
pub async fn install_fabric(
    client: &reqwest::Client,
    root: &Path,
    game_version: &str,
    options: &DownloadOptions,
    task_id: &str,
    callback: ProgressFn,
    cancel: CancelFlag,
) -> Result<FabricInstallResult> {
    let started = Instant::now();
    if !version::version_json_path(root, game_version).exists() {
        let report = install_version(
            client,
            root,
            game_version,
            options,
            task_id,
            callback.clone(),
            cancel.clone(),
        )
        .await?;
        let _ = report;
    }
    let loaders = fetch_fabric_loaders(client, game_version, options.source)
        .await
        .context("无法获取 Fabric 加载器版本")?;
    let loader = loaders
        .iter()
        .find(|l| l.stable)
        .or_else(|| loaders.first())
        .ok_or_else(|| anyhow!("没有可用的 Fabric 加载器"))?
        .version
        .clone();
    let (fabric_id, profile) =
        fetch_fabric_profile(client, game_version, &loader, options.source).await?;
    if !profile
        .inherits_from
        .as_deref()
        .map(|parent| version::version_json_path(root, parent).exists())
        .unwrap_or(false)
    {
        bail!("Fabric 配置的父版本尚未安装");
    }
    let path = version::version_json_path(root, &fabric_id);
    util::write_json_atomic(&path, &profile).await?;
    let resolved = version::resolve_version(root, &fabric_id)?;
    (callback)(InstallProgress {
        task_id: task_id.to_string(),
        phase: "libraries".to_string(),
        label: "正在安装 Fabric".to_string(),
        current: 0,
        total: 1,
        bytes_done: 0,
        bytes_total: 0,
        percent: 0.0,
        message: format!("加载器 {loader}"),
    });
    let (downloaded, reused, bytes) = sync_resolved_files(
        client,
        root,
        &resolved,
        options,
        task_id,
        callback.clone(),
        cancel.clone(),
        started,
    )
    .await?;
    (callback)(InstallProgress {
        task_id: task_id.to_string(),
        phase: "done".to_string(),
        label: "Fabric 安装完成".to_string(),
        current: 1,
        total: 1,
        bytes_done: bytes,
        bytes_total: bytes,
        percent: 100.0,
        message: format!("{fabric_id} 已就绪"),
    });
    Ok(FabricInstallResult {
        version_id: fabric_id,
        game_version: game_version.to_string(),
        loader_version: loader,
        report: InstallReport {
            version_id: game_version.to_string(),
            files_downloaded: downloaded,
            files_reused: reused,
            bytes_downloaded: bytes,
            elapsed_ms: started.elapsed().as_millis() as u64,
        },
    })
}

pub async fn read_remote_manifest_cached(
    client: &reqwest::Client,
    root: &Path,
    source: DownloadSource,
) -> Result<VersionManifest> {
    let cache = root
        .join(".creeper")
        .join("cache")
        .join("version_manifest.json");
    if let Ok(meta) = tokio::fs::metadata(&cache).await {
        if let Ok(modified) = meta.modified() {
            let age = modified.elapsed().unwrap_or_default();
            if age.as_secs() < 1800 {
                if let Ok(raw) = tokio::fs::read(&cache).await {
                    if let Ok(manifest) = serde_json::from_slice(&raw) {
                        return Ok(manifest);
                    }
                }
            }
        }
    }
    let manifest = fetch_manifest(client, source).await?;
    if let Some(parent) = cache.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    if let Ok(bytes) = serde_json::to_vec_pretty(&manifest) {
        let _ = tokio::fs::write(&cache, bytes).await;
    }
    Ok(manifest)
}

/// Install a loader variant while returning the resolved profile for the UI.
pub fn download_source_from_config(value: &str) -> DownloadSource {
    DownloadSource::parse(value)
}
