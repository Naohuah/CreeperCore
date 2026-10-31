//! Modrinth search and one-click mod installation.
//!
//! The launcher never exposes the raw Modrinth API to the WebView. Search,
//! version filtering, dependency resolution and file validation all happen in
//! this module so the download source can be swapped or mirrored later.

use crate::{util, version};
use anyhow::{anyhow, bail, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha1::{Digest, Sha1};
use sha2::Sha512;
use std::collections::{HashMap, HashSet, VecDeque};
use std::future::Future;
use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use zip::ZipArchive;

const API_BASE: &str = "https://api.modrinth.com/v2";
const MCIM_API_BASE: &str = "https://mod.mcimirror.top/modrinth/v2";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentSource {
    Official,
    Mcim,
}

impl ContentSource {
    pub fn parse(value: &str) -> Self {
        if value.eq_ignore_ascii_case("mcim") {
            Self::Mcim
        } else {
            Self::Official
        }
    }

    fn api_base(self) -> &'static str {
        match self {
            Self::Official => API_BASE,
            Self::Mcim => MCIM_API_BASE,
        }
    }
}
const MAX_FILE_BYTES: u64 = 200 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthSearchPage {
    pub hits: Vec<ModrinthHit>,
    pub offset: u64,
    pub limit: u64,
    pub total_hits: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthHit {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub follows: u64,
    pub categories: Vec<String>,
    pub versions: Vec<String>,
    pub latest_version: Option<String>,
    pub date_modified: Option<String>,
    pub client_side: Option<String>,
    pub server_side: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthInstallReport {
    pub installed: Vec<String>,
    pub skipped: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    hits: Vec<SearchHit>,
    #[serde(default)]
    offset: u64,
    #[serde(default)]
    limit: u64,
    #[serde(default)]
    total_hits: u64,
}

#[derive(Debug, Deserialize)]
struct SearchHit {
    project_id: String,
    #[serde(default)]
    slug: String,
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    icon_url: Option<String>,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    follows: u64,
    #[serde(default)]
    categories: Vec<String>,
    #[serde(default)]
    versions: Vec<String>,
    #[serde(default)]
    latest_version: Option<String>,
    #[serde(default)]
    date_modified: Option<String>,
    #[serde(default)]
    client_side: Option<String>,
    #[serde(default)]
    server_side: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiVersion {
    id: String,
    project_id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    version_number: String,
    #[serde(default)]
    version_type: String,
    #[serde(default)]
    date_published: Option<String>,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    game_versions: Vec<String>,
    #[serde(default)]
    loaders: Vec<String>,
    #[serde(default)]
    files: Vec<ApiFile>,
    #[serde(default)]
    dependencies: Vec<ApiDependency>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthVersionInfo {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub version_number: String,
    pub version_type: String,
    pub date_published: Option<String>,
    pub downloads: u64,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub file_name: Option<String>,
    pub file_size: u64,
    pub required_dependencies: usize,
    /// Required project IDs, used by the detail dialog to show prerequisites.
    pub dependency_project_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthProjectSummary {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub icon_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiFile {
    url: String,
    filename: String,
    #[serde(default)]
    primary: bool,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    hashes: ApiHashes,
}

#[derive(Debug, Default, Deserialize)]
struct ApiHashes {
    #[serde(default)]
    sha1: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiDependency {
    #[serde(default)]
    version_id: Option<String>,
    #[serde(default)]
    project_id: Option<String>,
    #[serde(default)]
    dependency_type: String,
}

pub fn loader_category(loader: &str) -> Result<&'static str> {
    match loader.to_ascii_lowercase().as_str() {
        "fabric" => Ok("fabric"),
        "quilt" => Ok("quilt"),
        "forge" => Ok("forge"),
        "neoforge" => Ok("neoforge"),
        "vanilla" => Err(anyhow!("原版版本不能安装模组，请先安装 Fabric、Forge 或 NeoForge")),
        other => Err(anyhow!("暂不支持 {other} 加载器的模组下载")),
    }
}

async fn bounded_secs<T>(
    future: impl Future<Output = Result<T>>,
    seconds: u64,
    label: &str,
) -> Result<T> {
    match tokio::time::timeout(Duration::from_secs(seconds), future).await {
        Ok(result) => result,
        Err(_) => Err(anyhow!("{label}超时")),
    }
}

async fn bounded<T>(future: impl Future<Output = Result<T>>, label: &str) -> Result<T> {
    bounded_secs(future, 25, label).await
}

fn valid_project_ref(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
}

async fn response_json<T: for<'de> Deserialize<'de>>(
    response: reqwest::Response,
    context: &str,
) -> Result<T> {
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        let message = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|value| {
                value
                    .get("description")
                    .or_else(|| value.get("error"))
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| format!("HTTP {status}"));
        bail!("{context}：{message}");
    }
    serde_json::from_str(&text).with_context(|| format!("{context}：响应格式无效"))
}

async fn search_kind(
    client: &Client,
    source: ContentSource,
    project_type: &str,
    query: &str,
    game_version: &str,
    category: Option<&str>,
    offset: u64,
    limit: u64,
) -> Result<ModrinthSearchPage> {
    if game_version.trim().is_empty() {
        bail!("当前版本没有可用的 Minecraft 版本号");
    }
    let mut facets = vec![vec![format!("project_type:{project_type}")]];
    if project_type == "mod" && category != Some("datapack") {
        facets.push(vec![
            "client_side:required".to_string(),
            "client_side:optional".to_string(),
        ]);
    }
    facets.push(vec![format!("versions:{game_version}")]);
    if let Some(category) = category {
        facets.push(vec![format!("categories:{category}")]);
    }
    let facets = json!(facets).to_string();
    let response = client
        .get(format!("{}/search", source.api_base()))
        .query(&[
            ("query", query.trim()),
            ("facets", facets.as_str()),
            ("offset", &offset.to_string()),
            ("limit", &limit.to_string()),
            ("index", "relevance"),
        ])
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .context("无法连接 Modrinth 搜索服务")?;
    let page: SearchResponse =
        response_json(response, "Modrinth 搜索失败").await?;
    Ok(ModrinthSearchPage {
        hits: page
            .hits
            .into_iter()
            .map(|hit| ModrinthHit {
                project_id: hit.project_id,
                slug: hit.slug,
                title: hit.title,
                description: hit.description,
                author: hit.author,
                icon_url: hit.icon_url,
                downloads: hit.downloads,
                follows: hit.follows,
                categories: hit.categories,
                versions: hit.versions,
                latest_version: hit.latest_version,
                date_modified: hit.date_modified,
                client_side: hit.client_side,
                server_side: hit.server_side,
            })
            .collect(),
        offset: page.offset,
        limit: page.limit,
        total_hits: page.total_hits,
    })
}

pub async fn search(
    client: &Client,
    source: ContentSource,
    query: &str,
    game_version: &str,
    loader: &str,
    offset: u64,
    limit: u64,
) -> Result<ModrinthSearchPage> {
    search_kind(
        client,
        source,
        "mod",
        query,
        game_version,
        Some(loader_category(loader)?),
        offset,
        limit,
    )
    .await
}

pub async fn search_packs(
    client: &Client,
    source: ContentSource,
    query: &str,
    game_version: &str,
    loader: &str,
    offset: u64,
    limit: u64,
) -> Result<ModrinthSearchPage> {
    search_kind(
        client,
        source,
        "modpack",
        query,
        game_version,
        Some(loader_category(loader)?),
        offset,
        limit,
    )
    .await
}

pub async fn search_content(
    client: &Client,
    source: ContentSource,
    project_type: &str,
    query: &str,
    game_version: &str,
    offset: u64,
    limit: u64,
) -> Result<ModrinthSearchPage> {
    if !matches!(project_type, "resourcepack" | "shader" | "datapack") {
        bail!("不支持的内容类型");
    }
    // Modrinth represents data packs as `mod` projects with the `datapack`
    // category; resource packs and shaders have their own project types.
    let (kind, category) = if project_type == "datapack" {
        ("mod", Some("datapack"))
    } else {
        (project_type, None)
    };
    search_kind(
        client,
        source,
        kind,
        query,
        game_version,
        category,
        offset,
        limit,
    )
    .await
}

async fn fetch_versions_for(
    client: &Client,
    source: ContentSource,
    project_id: &str,
    game_version: &str,
    loader: Option<&str>,
) -> Result<Vec<ApiVersion>> {
    if !valid_project_ref(project_id) {
        bail!("Modrinth 项目 ID 无效");
    }
    let game = serde_json::to_string(&[game_version])?;
    let mut request = client
        .get(format!(
            "{}/project/{project_id}/version",
            source.api_base()
        ))
        .query(&[("game_versions", game)]);
    if let Some(loader) = loader {
        let loaders = serde_json::to_string(&[loader_category(loader)?])?;
        request = request.query(&[("loaders", loaders)]);
    }
    let response = request
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .context("无法读取 Modrinth 版本列表")?;
    response_json(response, "Modrinth 版本读取失败").await
}

async fn fetch_versions(
    client: &Client,
    source: ContentSource,
    project_id: &str,
    game_version: &str,
    loader: &str,
) -> Result<Vec<ApiVersion>> {
    fetch_versions_for(client, source, project_id, game_version, Some(loader)).await
}

pub async fn list_versions(
    client: &Client,
    source: ContentSource,
    project_id: &str,
    game_version: &str,
    loader: &str,
) -> Result<Vec<ModrinthVersionInfo>> {
    let versions = bounded(
        fetch_versions(client, source, project_id, game_version, loader),
        "Modrinth 版本列表",
    )
    .await?;
    Ok(versions
        .into_iter()
        .map(|version| {
            let file = version
                .files
                .iter()
                .find(|file| file.primary)
                .or_else(|| version.files.first());
            ModrinthVersionInfo {
                id: version.id,
                project_id: version.project_id,
                name: version.name,
                version_number: version.version_number,
                version_type: if version.version_type.is_empty() {
                    "release".to_string()
                } else {
                    version.version_type
                },
                date_published: version.date_published,
                downloads: version.downloads,
                game_versions: version.game_versions,
                loaders: version.loaders,
                file_name: file.map(|file| file.filename.clone()),
                file_size: file.map(|file| file.size).unwrap_or(0),
                required_dependencies: version
                    .dependencies
                    .iter()
                    .filter(|dependency| dependency.dependency_type == "required")
                    .count(),
                dependency_project_ids: version
                    .dependencies
                    .iter()
                    .filter(|dependency| dependency.dependency_type == "required")
                    .filter_map(|dependency| dependency.project_id.clone())
                    .collect(),
            }
        })
        .collect())
}

pub async fn list_projects(
    client: &Client,
    source: ContentSource,
    project_ids: &[String],
) -> Result<Vec<ModrinthProjectSummary>> {
    let mut unique = Vec::new();
    for project_id in project_ids {
        if !valid_project_ref(project_id) {
            continue;
        }
        if !unique.iter().any(|item| item == project_id) {
            unique.push(project_id.clone());
        }
        if unique.len() >= 50 {
            break;
        }
    }
    if unique.is_empty() {
        return Ok(Vec::new());
    }
    #[derive(Debug, Deserialize)]
    struct ApiProject {
        id: String,
        #[serde(default)]
        slug: String,
        #[serde(default)]
        title: String,
        #[serde(default)]
        icon_url: Option<String>,
    }
    let ids = serde_json::to_string(&unique)?;
    let response = client
        .get(format!("{}/projects", source.api_base()))
        .query(&[("ids", ids)])
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .context("无法读取前置项目信息")?;
    let projects: Vec<ApiProject> =
        response_json(response, "前置项目信息读取失败").await?;
    Ok(projects
        .into_iter()
        .map(|project| ModrinthProjectSummary {
            project_id: project.id,
            slug: project.slug,
            title: if project.title.is_empty() {
                "未知项目".to_string()
            } else {
                project.title
            },
            icon_url: project.icon_url,
        })
        .collect())
}

async fn fetch_version(
    client: &Client,
    source: ContentSource,
    version_id: &str,
) -> Result<ApiVersion> {
    if !valid_project_ref(version_id) {
        bail!("Modrinth 版本 ID 无效");
    }
    let response = client
        .get(format!("{}/version/{version_id}", source.api_base()))
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .context("无法读取 Modrinth 依赖版本")?;
    response_json(response, "Modrinth 依赖版本读取失败").await
}

fn version_compatible(version: &ApiVersion, game_version: &str, loader: &str) -> bool {
    let loader = loader.to_ascii_lowercase();
    version
        .game_versions
        .iter()
        .any(|value| value.eq_ignore_ascii_case(game_version))
        && version
            .loaders
            .iter()
            .any(|value| value.eq_ignore_ascii_case(&loader))
}

fn safe_file_name(raw: &str) -> Result<String> {
    let name = Path::new(raw)
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| anyhow!("Modrinth 返回了无效文件名"))?;
    if name.is_empty() || name == "." || name == ".." || name != raw {
        bail!("Modrinth 返回了不安全的文件名");
    }
    Ok(name.to_string())
}

async fn file_sha1(path: &Path) -> Option<String> {
    let bytes = tokio::fs::read(path).await.ok()?;
    Some(hex::encode(Sha1::digest(bytes)))
}

async fn install_file(
    client: &Client,
    file: &ApiFile,
    mods_dir: &Path,
    report: &mut ModrinthInstallReport,
) -> Result<()> {
    let name = safe_file_name(&file.filename)?;
    if file.size > MAX_FILE_BYTES {
        bail!("文件过大：{name}");
    }
    let dest = mods_dir.join(&name);
    if tokio::fs::try_exists(&dest).await.unwrap_or(false) {
        if let Some(expected) = file.hashes.sha1.as_deref() {
            if file_sha1(&dest).await.as_deref() == Some(expected) {
                report.skipped.push(name);
                return Ok(());
            }
        }
        report
            .warnings
            .push(format!("已存在同名文件 {name}，为保留现有文件已跳过"));
        return Ok(());
    }

    let response = client
        .get(&file.url)
        .timeout(Duration::from_secs(180))
        .send()
        .await
        .with_context(|| format!("下载 {name} 失败"))?;
    if !response.status().is_success() {
        bail!("下载 {name} 返回 HTTP {}", response.status());
    }
    if response.content_length().unwrap_or(0) > MAX_FILE_BYTES {
        bail!("文件过大：{name}");
    }
    let bytes = response.bytes().await.context("模组文件读取失败")?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        bail!("文件过大：{name}");
    }
    if file.size > 0 && bytes.len() as u64 != file.size {
        bail!("{name} 文件大小校验失败");
    }
    if let Some(expected) = file.hashes.sha1.as_deref() {
        let actual = hex::encode(Sha1::digest(&bytes));
        if !actual.eq_ignore_ascii_case(expected) {
            bail!("{name} SHA-1 校验失败");
        }
    }
    tokio::fs::create_dir_all(mods_dir)
        .await
        .with_context(|| format!("无法创建模组目录：{}", mods_dir.display()))?;
    let part = mods_dir.join(format!("{name}.part"));
    tokio::fs::write(&part, &bytes)
        .await
        .with_context(|| format!("无法写入 {}", part.display()))?;
    if tokio::fs::try_exists(&dest).await.unwrap_or(false) {
        let _ = tokio::fs::remove_file(&part).await;
        bail!("保存 {name} 时发现同名文件，未覆盖");
    }
    tokio::fs::rename(&part, &dest)
        .await
        .with_context(|| format!("无法保存 {}", dest.display()))?;
    report.installed.push(name);
    Ok(())
}

async fn install_version(
    client: &Client,
    version: &ApiVersion,
    mods_dir: &Path,
    report: &mut ModrinthInstallReport,
) -> Result<()> {
    let file = version
        .files
        .iter()
        .find(|file| file.primary)
        .or_else(|| version.files.first())
        .ok_or_else(|| anyhow!("该版本没有可下载文件"))?;
    install_file(client, file, mods_dir, report).await
}

async fn install_version_tree(
    client: &Client,
    source: ContentSource,
    root: ApiVersion,
    project_id: &str,
    mods_dir: &Path,
    game_version: &str,
    loader: &str,
) -> Result<ModrinthInstallReport> {
    let mut report = ModrinthInstallReport {
        installed: Vec::new(),
        skipped: Vec::new(),
        warnings: Vec::new(),
    };
    let mut queue = VecDeque::from([root]);
    let mut seen_versions = HashSet::new();
    let mut seen_projects = HashSet::from([project_id.to_string()]);

    while let Some(version) = queue.pop_front() {
        if !seen_versions.insert(version.id.clone()) {
            continue;
        }
        if let Err(err) = install_version(client, &version, mods_dir, &mut report).await {
            report.warnings.push(format!("{}：{err}", version.name));
        }

        for dependency in &version.dependencies {
            if dependency.dependency_type != "required" {
                continue;
            }
            let dependency_version = if let Some(version_id) = &dependency.version_id {
                match bounded(
                    fetch_version(client, source, version_id),
                    "Modrinth 依赖版本",
                )
                .await
                {
                    Ok(value) if version_compatible(&value, game_version, loader) => Some(value),
                    Ok(_) => None,
                    Err(err) => {
                        report
                            .warnings
                            .push(format!("无法读取依赖 {version_id}：{err}"));
                        None
                    }
                }
            } else if let Some(project) = &dependency.project_id {
                match bounded(
                    fetch_versions(client, source, project, game_version, loader),
                    "Modrinth 依赖项目",
                )
                .await
                {
                    Ok(values) => values.into_iter().next(),
                    Err(err) => {
                        report
                            .warnings
                            .push(format!("无法读取依赖项目 {project}：{err}"));
                        None
                    }
                }
            } else {
                None
            };
            let Some(dependency_version) = dependency_version else {
                report
                    .warnings
                    .push("有一个必需依赖没有兼容版本，已跳过".to_string());
                continue;
            };
            if seen_projects.insert(dependency_version.project_id.clone()) {
                queue.push_back(dependency_version);
            }
        }
    }
    Ok(report)
}

pub async fn install_latest(
    client: &Client,
    source: ContentSource,
    project_id: &str,
    mods_dir: &Path,
    game_version: &str,
    loader: &str,
) -> Result<ModrinthInstallReport> {
    let versions = bounded(
        fetch_versions(client, source, project_id, game_version, loader),
        "Modrinth 版本列表",
    )
    .await?;
    let root = versions
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("没有找到与当前版本兼容的 Modrinth 版本"))?;
    install_version_tree(
        client,
        source,
        root,
        project_id,
        mods_dir,
        game_version,
        loader,
    )
    .await
}

pub async fn install_selected(
    client: &Client,
    source: ContentSource,
    version_id: &str,
    mods_dir: &Path,
    game_version: &str,
    loader: &str,
) -> Result<ModrinthInstallReport> {
    let root = bounded(
        fetch_version(client, source, version_id),
        "Modrinth 选定版本",
    )
    .await?;
    if !version_compatible(&root, game_version, loader) {
        bail!("这个版本与当前游戏版本或加载器不兼容");
    }
    let project_id = root.project_id.clone();
    install_version_tree(
        client,
        source,
        root,
        &project_id,
        mods_dir,
        game_version,
        loader,
    )
    .await
}

pub async fn install_content_latest(
    client: &Client,
    source: ContentSource,
    project_id: &str,
    destination: &Path,
    game_version: &str,
) -> Result<ModrinthInstallReport> {
    let versions = bounded(
        fetch_versions_for(client, source, project_id, game_version, None),
        "Modrinth 内容版本列表",
    )
    .await?;
    let version = versions
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("没有找到适用于 Minecraft {game_version} 的版本"))?;
    let mut report = ModrinthInstallReport {
        installed: Vec::new(),
        skipped: Vec::new(),
        warnings: Vec::new(),
    };
    install_version(client, &version, destination, &mut report).await?;
    Ok(report)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MrpackIndex {
    #[serde(default)]
    format_version: u32,
    #[serde(default)]
    game: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    files: Vec<MrpackFile>,
    #[serde(default)]
    dependencies: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MrpackFile {
    path: String,
    #[serde(default)]
    hashes: HashMap<String, String>,
    #[serde(default)]
    env: HashMap<String, String>,
    #[serde(default)]
    downloads: Vec<String>,
    #[serde(default)]
    file_size: u64,
}

#[derive(Debug, Deserialize)]
struct ProjectMeta {
    #[serde(default)]
    title: String,
    #[serde(default)]
    slug: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModpackInstallResult {
    pub version_id: String,
    pub pack_name: String,
    pub minecraft_version: String,
    pub loader_version: String,
    pub files_installed: usize,
    pub files_skipped: usize,
    pub overrides: usize,
    pub warnings: Vec<String>,
}

fn safe_relative(raw: &str) -> Result<PathBuf> {
    let path = Path::new(raw);
    if path.as_os_str().is_empty() || path.is_absolute() {
        bail!("整合包包含无效文件路径");
    }
    let mut clean = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => {
                let text = part.to_string_lossy();
                if text.is_empty() || text.contains(':') {
                    bail!("整合包包含不安全的文件路径");
                }
                clean.push(part);
            }
            Component::CurDir => {}
            _ => bail!("整合包包含目录穿越路径"),
        }
    }
    if clean.as_os_str().is_empty() {
        bail!("整合包包含空文件路径");
    }
    Ok(clean)
}

fn verify_hashes(bytes: &[u8], hashes: &HashMap<String, String>, name: &str) -> Result<()> {
    let sha1 = hashes.get("sha1");
    let sha512 = hashes.get("sha512");
    if sha1.is_none() && sha512.is_none() {
        bail!("{name} 缺少哈希，拒绝安装");
    }
    if let Some(expected) = sha1 {
        let actual = hex::encode(Sha1::digest(bytes));
        if !actual.eq_ignore_ascii_case(expected) {
            bail!("{name} SHA-1 校验失败");
        }
    }
    if let Some(expected) = sha512 {
        let actual = hex::encode(Sha512::digest(bytes));
        if !actual.eq_ignore_ascii_case(expected) {
            bail!("{name} SHA-512 校验失败");
        }
    }
    Ok(())
}

async fn fetch_project(
    client: &Client,
    source: ContentSource,
    project_id: &str,
) -> Result<ProjectMeta> {
    if !valid_project_ref(project_id) {
        bail!("Modrinth 项目 ID 无效");
    }
    let response = client
        .get(format!(
            "{}/project/{project_id}",
            source.api_base()
        ))
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .context("无法读取整合包信息")?;
    response_json(response, "整合包信息读取失败").await
}

async fn download_bytes(
    client: &Client,
    url: &str,
    limit: u64,
    label: &str,
) -> Result<Vec<u8>> {
    let response = client
        .get(url)
        .timeout(Duration::from_secs(180))
        .send()
        .await
        .with_context(|| format!("下载{label}失败"))?;
    if !response.status().is_success() {
        bail!("下载{label}返回 HTTP {}", response.status());
    }
    if response.content_length().unwrap_or(0) > limit {
        bail!("{label}超过大小限制");
    }
    let bytes = response.bytes().await.context("文件读取失败")?;
    if bytes.len() as u64 > limit {
        bail!("{label}超过大小限制");
    }
    Ok(bytes.to_vec())
}

async fn download_pack_files(
    client: &Client,
    source: ContentSource,
    project_id: &str,
    game_version: &str,
) -> Result<(MrpackIndex, ProjectMeta, Vec<u8>)> {
    let meta = bounded(
        fetch_project(client, source, project_id),
        "整合包信息",
    )
    .await?;
    let versions = bounded(
        fetch_versions(client, source, project_id, game_version, "fabric"),
        "整合包版本列表",
    )
    .await?;
    let pack_version = versions
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("没有找到适用于 Fabric {game_version} 的整合包版本"))?;
    let file = pack_version
        .files
        .iter()
        .find(|file| file.primary)
        .or_else(|| pack_version.files.first())
        .ok_or_else(|| anyhow!("整合包版本没有可下载文件"))?;
    if file.size > 200 * 1024 * 1024 {
        bail!("整合包文件超过 200 MB");
    }
    let bytes = bounded_secs(
        download_bytes(client, &file.url, 200 * 1024 * 1024, "整合包"),
        180,
        "整合包下载",
    )
    .await?;
    if let Some(expected) = file.hashes.sha1.as_deref() {
        let actual = hex::encode(Sha1::digest(&bytes));
        if !actual.eq_ignore_ascii_case(expected) {
            bail!("整合包文件 SHA-1 校验失败");
        }
    }
    let mut archive = ZipArchive::new(Cursor::new(bytes.as_slice()))
        .context("整合包不是有效的 .mrpack ZIP")?;
    let mut index_text = String::new();
    archive
        .by_name("modrinth.index.json")
        .context("整合包缺少 modrinth.index.json")?
        .read_to_string(&mut index_text)
        .context("整合包索引读取失败")?;
    let index: MrpackIndex = serde_json::from_str(&index_text).context("整合包索引格式无效")?;
    if index.game != "minecraft" {
        bail!("整合包不是 Minecraft 整合包");
    }
    if index.format_version != 0 && index.format_version != 1 {
        bail!("不支持的整合包格式版本：{}", index.format_version);
    }
    if let Some(mc) = index.dependencies.get("minecraft") {
        if mc != game_version {
            bail!("整合包要求 Minecraft {mc}，当前基础版本是 {game_version}");
        }
    }
    for unsupported in ["forge", "neoforge", "quilt-loader"] {
        if index.dependencies.contains_key(unsupported) {
            bail!("当前只支持 Fabric 整合包，这个整合包需要 {unsupported}");
        }
    }
    Ok((index, meta, bytes))
}

async fn install_mrpack(
    client: &Client,
    source: ContentSource,
    root: &Path,
    base_fabric_id: &str,
    base_game_version: &str,
    base_loader_version: &str,
    project_id: &str,
) -> Result<ModpackInstallResult> {
    let (index, meta, zip_bytes) =
        download_pack_files(client, source, project_id, base_game_version).await?;
    let mut warnings = Vec::new();
    if let Some(loader) = index.dependencies.get("fabric-loader") {
        if loader != base_loader_version {
            warnings.push(format!(
                "整合包声明 Fabric Loader {loader}，当前基础版本使用 {base_loader_version}，已沿用当前加载器"
            ));
        }
    }

    let slug = util::sanitize_component(if meta.slug.trim().is_empty() {
        &meta.title
    } else {
        &meta.slug
    })
    .unwrap_or_else(|| "pack".to_string());
    let mut new_id = format!("pack-{slug}");
    let mut suffix = 2;
    while version::version_json_path(root, &new_id).exists() {
        new_id = format!("pack-{slug}-{suffix}");
        suffix += 1;
    }
    let source = version::version_json_path(root, base_fabric_id);
    let source_text = tokio::fs::read_to_string(&source)
        .await
        .with_context(|| format!("无法读取基础 Fabric 版本：{}", source.display()))?;
    let mut json: Value = serde_json::from_str(&source_text).context("基础 Fabric 配置格式无效")?;
    json["id"] = Value::String(new_id.clone());
    let version_dir = root.join("versions").join(&new_id);
    tokio::fs::create_dir_all(&version_dir).await?;
    util::write_json_atomic(&version_dir.join(format!("{new_id}.json")), &json).await?;
    let game_dir = version_dir.join("game");
    tokio::fs::create_dir_all(game_dir.join("mods")).await?;
    tokio::fs::create_dir_all(game_dir.join("config")).await?;

    let mut result = ModpackInstallResult {
        version_id: new_id.clone(),
        pack_name: if index.name.trim().is_empty() {
            meta.title.clone()
        } else {
            index.name.clone()
        },
        minecraft_version: base_game_version.to_string(),
        loader_version: base_loader_version.to_string(),
        files_installed: 0,
        files_skipped: 0,
        overrides: 0,
        warnings,
    };

    for file in &index.files {
        if file
            .env
            .get("client")
            .map(|value| value.eq_ignore_ascii_case("unsupported"))
            .unwrap_or(false)
        {
            result.files_skipped += 1;
            continue;
        }
        let relative = match safe_relative(&file.path) {
            Ok(path) => path,
            Err(err) => {
                result
                    .warnings
                    .push(format!("跳过不安全路径 {}：{err}", file.path));
                continue;
            }
        };
        let dest = game_dir.join(&relative);
        if tokio::fs::try_exists(&dest).await.unwrap_or(false) {
            if let Ok(existing) = tokio::fs::read(&dest).await {
                if verify_hashes(&existing, &file.hashes, &file.path).is_ok() {
                    result.files_skipped += 1;
                    continue;
                }
            }
            result
                .warnings
                .push(format!("{} 已存在但内容不同，已保留现有文件", file.path));
            continue;
        }
        if file.file_size > MAX_FILE_BYTES {
            result
                .warnings
                .push(format!("{} 超过大小限制，已跳过", file.path));
            continue;
        }
        let mut downloaded = None;
        for url in &file.downloads {
            match bounded_secs(
                download_bytes(client, url, MAX_FILE_BYTES, &file.path),
                180,
                "整合包文件下载",
            )
            .await
            {
                Ok(bytes) => {
                    downloaded = Some(bytes);
                    break;
                }
                Err(err) => result.warnings.push(format!("{} 下载失败：{err}", file.path)),
            }
        }
        let Some(bytes) = downloaded else {
            continue;
        };
        if file.file_size > 0 && bytes.len() as u64 != file.file_size {
            result
                .warnings
                .push(format!("{} 文件大小不匹配，已跳过", file.path));
            continue;
        }
        if let Err(err) = verify_hashes(&bytes, &file.hashes, &file.path) {
            result.warnings.push(format!("{err}，已跳过"));
            continue;
        }
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let part = dest.with_file_name(format!(
            "{}.part",
            dest.file_name().and_then(|value| value.to_str()).unwrap_or("file")
        ));
        tokio::fs::write(&part, &bytes).await?;
        tokio::fs::rename(&part, &dest).await?;
        result.files_installed += 1;
    }

    let mut archive =
        ZipArchive::new(Cursor::new(zip_bytes.as_slice())).context("整合包 ZIP 读取失败")?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let raw_name = entry.name().to_string();
        let relative = if let Some(rest) = raw_name.strip_prefix("overrides/") {
            rest
        } else if let Some(rest) = raw_name.strip_prefix("client-overrides/") {
            rest
        } else {
            continue;
        };
        if relative.is_empty() || entry.is_dir() {
            continue;
        }
        let path = match safe_relative(relative) {
            Ok(path) => path,
            Err(err) => {
                result
                    .warnings
                    .push(format!("跳过覆盖文件 {raw_name}：{err}"));
                continue;
            }
        };
        if entry.size() > 50 * 1024 * 1024 {
            result
                .warnings
                .push(format!("覆盖文件 {raw_name} 超过大小限制"));
            continue;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        let dest = game_dir.join(path);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&dest, bytes)?;
        result.overrides += 1;
    }

    if result.files_installed == 0 && index.files.iter().any(|file| {
        !file
            .env
            .get("client")
            .map(|value| value.eq_ignore_ascii_case("unsupported"))
            .unwrap_or(false)
    }) {
        result
            .warnings
            .push("没有成功安装任何整合包文件，请检查网络和 Modrinth 状态".to_string());
    }
    Ok(result)
}

pub async fn install_latest_modpack(
    client: &Client,
    source: ContentSource,
    project_id: &str,
    base_fabric_id: &str,
    root: &Path,
) -> Result<ModpackInstallResult> {
    let versions = version::scan_versions(root).context("无法扫描已安装版本")?;
    let base = versions
        .iter()
        .find(|version| version.id == base_fabric_id)
        .ok_or_else(|| anyhow!("目标 Fabric 版本不存在"))?;
    if !base.loader.eq_ignore_ascii_case("fabric") {
        bail!("整合包需要 Fabric 基础版本");
    }
    let game_version = base
        .inherits_from
        .clone()
        .unwrap_or_else(|| base.id.clone());
    let loader_version = base.loader_version.clone().unwrap_or_default();
    install_mrpack(
        client,
        source,
        root,
        base_fabric_id,
        &game_version,
        &loader_version,
        project_id,
    )
    .await
}

/// Reject absolute paths and path traversal for a mod file destination.
pub fn mods_directory(game_dir: &Path) -> PathBuf {
    game_dir.join("mods")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_supported_loaders() {
        assert_eq!(loader_category("fabric").unwrap(), "fabric");
        assert_eq!(loader_category("NeoForge").unwrap(), "neoforge");
        assert!(loader_category("vanilla").is_err());
    }

    #[test]
    fn rejects_unsafe_mod_file_names() {
        assert!(safe_file_name("sodium.jar").is_ok());
        assert!(safe_file_name("../evil.jar").is_err());
        assert!(safe_file_name("folder/evil.jar").is_err());
    }

    #[test]
    fn parses_mrpack_index_and_rejects_unsafe_paths() {
        let index: MrpackIndex = serde_json::from_value(json!({
            "formatVersion": 1,
            "game": "minecraft",
            "versionId": "1.0.0",
            "name": "Test Pack",
            "files": [{
                "path": "mods/test.jar",
                "hashes": { "sha1": "abc" },
                "downloads": ["https://example.invalid/test.jar"],
                "fileSize": 3
            }],
            "dependencies": { "minecraft": "1.21.1", "fabric-loader": "0.16.9" }
        }))
        .unwrap();
        assert_eq!(index.format_version, 1);
        assert_eq!(index.dependencies.get("minecraft").map(String::as_str), Some("1.21.1"));
        assert_eq!(safe_relative(&index.files[0].path).unwrap(), PathBuf::from("mods/test.jar"));
        assert!(safe_relative("../escape.jar").is_err());
        assert!(safe_relative("C:/escape.jar").is_err());
    }
}
