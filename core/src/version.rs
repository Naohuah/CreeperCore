//! Minecraft version discovery, inheritance resolution and rule evaluation.

use crate::model::*;
use crate::util;
use anyhow::{anyhow, bail, Context, Result};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

const MAX_INHERITANCE_DEPTH: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryKind {
    Classpath,
    Native,
}

#[derive(Debug, Clone)]
pub struct LibraryFile {
    pub path: String,
    pub url: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    pub kind: LibraryKind,
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Platform {
    pub os: &'static str,
    pub arch: &'static str,
    pub arch_bits: u32,
}

impl Platform {
    pub fn current() -> Self {
        let arch = match std::env::consts::ARCH {
            "x86_64" => "x86_64",
            "x86" => "x86",
            "aarch64" => "arm64",
            other => other,
        };
        let arch_bits = if cfg!(target_pointer_width = "64") {
            64
        } else {
            32
        };
        let os = match std::env::consts::OS {
            "windows" => "windows",
            "macos" => "osx",
            "linux" => "linux",
            other => other,
        };
        Self {
            os,
            arch,
            arch_bits,
        }
    }

    fn matches_os(&self, rule: &OsRule) -> bool {
        if let Some(name) = &rule.name {
            if name != self.os {
                return false;
            }
        }
        if let Some(arch) = &rule.arch {
            if arch != self.arch {
                return false;
            }
        }
        // version / version_range rules target very old Windows builds. Modern
        // Windows 10/11 always satisfy Mojang's current requirements.
        true
    }
}

pub fn versions_dir(root: &Path) -> PathBuf {
    root.join("versions")
}

pub fn libraries_dir(root: &Path) -> PathBuf {
    root.join("libraries")
}

pub fn assets_dir(root: &Path) -> PathBuf {
    root.join("assets")
}

pub fn version_json_path(root: &Path, id: &str) -> PathBuf {
    versions_dir(root).join(id).join(format!("{id}.json"))
}

pub fn read_version_json(root: &Path, id: &str) -> Result<VersionJson> {
    let safe_id = util::safe_component(id)?;
    let direct = version_json_path(root, &safe_id);
    let path = if direct.exists() {
        direct
    } else {
        let dir = versions_dir(root).join(&safe_id);
        std::fs::read_dir(&dir)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .find(|path| path.extension().and_then(|e| e.to_str()) == Some("json"))
            .ok_or_else(|| anyhow!("找不到版本文件：{}", direct.display()))?
    };
    util::read_json_sync(&path)
}

/// Scan `versions/*/*.json` without resolving every inheritance chain.
pub fn scan_versions(root: &Path) -> Result<Vec<VersionSummary>> {
    let dir = versions_dir(root);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    for entry in
        std::fs::read_dir(&dir).with_context(|| format!("无法读取版本目录：{}", dir.display()))?
    {
        let Ok(entry) = entry else { continue };
        if !entry.path().is_dir() {
            continue;
        }
        let dir_name = entry.file_name().to_string_lossy().to_string();
        let dir_path = entry.path();
        let preferred = dir_path.join(format!("{dir_name}.json"));
        let json_path = if preferred.exists() {
            Some(preferred)
        } else {
            std::fs::read_dir(&dir_path)
                .ok()
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok().map(|e| e.path()))
                .find(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
        };
        let Some(json_path) = json_path else {
            continue;
        };
        let modified = std::fs::metadata(&json_path)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        match util::read_json_sync::<VersionJson>(&json_path) {
            Ok(json) => {
                let id = if json.id.trim().is_empty() {
                    dir_name.clone()
                } else {
                    json.id.clone()
                };
                let (loader, loader_version) = detect_loader(
                    &id,
                    json.main_class.as_deref(),
                    json.inherits_from.as_deref(),
                );
                result.push(VersionSummary {
                    id: id.clone(),
                    display_name: id.clone(),
                    version_type: json.version_type.clone(),
                    loader,
                    loader_version,
                    release_time: json.release_time.clone(),
                    last_modified: modified,
                    java_major: json.java_version.as_ref().and_then(|j| j.major_version),
                    has_json: true,
                    has_client: find_existing_client_jar(root, &id).is_some(),
                    main_class: json.main_class.clone(),
                    inherits_from: json.inherits_from.clone(),
                    read_error: None,
                });
            }
            Err(err) => {
                result.push(VersionSummary {
                    id: dir_name.clone(),
                    display_name: dir_name.clone(),
                    version_type: "unknown".to_string(),
                    loader: "unknown".to_string(),
                    loader_version: None,
                    release_time: None,
                    last_modified: modified,
                    java_major: None,
                    has_json: true,
                    has_client: false,
                    main_class: None,
                    inherits_from: None,
                    read_error: Some(err.to_string()),
                });
            }
        }
    }
    result.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));
    Ok(result)
}

fn detect_loader(
    id: &str,
    main_class: Option<&str>,
    inherits: Option<&str>,
) -> (String, Option<String>) {
    let lower_id = id.to_ascii_lowercase();
    let lower_main = main_class.unwrap_or_default().to_ascii_lowercase();
    if lower_id.starts_with("fabric-loader-") || lower_main.contains("fabricmc") {
        let rest = lower_id
            .strip_prefix("fabric-loader-")
            .unwrap_or_default()
            .to_string();
        let version = inherits
            .map(|parent| {
                let suffix = format!("-{}", parent.to_ascii_lowercase());
                rest.strip_suffix(&suffix)
                    .map(str::to_string)
                    .unwrap_or_else(|| rest.clone())
            })
            .unwrap_or(rest);
        return ("fabric".to_string(), non_empty(version));
    }
    if lower_id.contains("quilt-loader") || lower_main.contains("quiltmc") {
        let rest = lower_id
            .strip_prefix("quilt-loader-")
            .unwrap_or(&lower_id)
            .to_string();
        return ("quilt".to_string(), non_empty(rest));
    }
    if lower_id.contains("neoforge") || lower_main.contains("neoforged") {
        return (
            "neoforge".to_string(),
            last_version_token(id).map(str::to_string),
        );
    }
    if lower_id.contains("forge") || lower_main.contains("minecraftforge") {
        return (
            "forge".to_string(),
            last_version_token(id).map(str::to_string),
        );
    }
    ("vanilla".to_string(), None)
}

fn non_empty(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

fn last_version_token(value: &str) -> Option<&str> {
    value
        .split(['-', '_'])
        .rev()
        .find(|part| part.chars().next().is_some_and(|c| c.is_ascii_digit()))
}

fn find_existing_client_jar(root: &Path, id: &str) -> Option<PathBuf> {
    let mut current = Some(id.to_string());
    let mut seen = HashSet::new();
    for _ in 0..MAX_INHERITANCE_DEPTH {
        let current_id = current?;
        if !seen.insert(current_id.clone()) {
            return None;
        }
        let jar = versions_dir(root)
            .join(&current_id)
            .join(format!("{current_id}.jar"));
        if jar.exists() {
            return Some(jar);
        }
        let json = read_version_json(root, &current_id).ok()?;
        current = json.inherits_from;
    }
    None
}

pub fn effective_client_jar(root: &Path, resolved: &ResolvedVersion) -> PathBuf {
    let own = versions_dir(root)
        .join(&resolved.jar_id)
        .join(format!("{}.jar", resolved.jar_id));
    if own.exists() {
        own
    } else {
        versions_dir(root)
            .join(&resolved.id)
            .join(format!("{}.jar", resolved.id))
    }
}

/// Load a version and all its parents, then merge from oldest to newest.
pub fn resolve_version(root: &Path, id: &str) -> Result<ResolvedVersion> {
    let safe_id = util::safe_component(id)?;
    let mut chain = Vec::new();
    let mut current = Some(safe_id);
    let mut seen = HashSet::new();
    while let Some(version_id) = current {
        if !seen.insert(version_id.clone()) {
            bail!("版本继承出现循环：{version_id}");
        }
        if chain.len() >= MAX_INHERITANCE_DEPTH {
            bail!("版本继承层级过深");
        }
        let json = read_version_json(root, &version_id)
            .with_context(|| format!("无法加载版本 {version_id}"))?;
        current = json.inherits_from.clone();
        chain.push(json);
    }
    chain.reverse();

    let mut main_class: Option<String> = None;
    let mut version_type = "release".to_string();
    let mut release_time: Option<String> = None;
    let mut assets: Option<String> = None;
    let mut asset_index: Option<AssetIndexRef> = None;
    let mut java_major: Option<u32> = None;
    let mut logging: Option<LoggingConfig> = None;
    let mut minecraft_arguments: Option<String> = None;
    let mut game_args: Vec<serde_json::Value> = Vec::new();
    let mut jvm_args: Vec<serde_json::Value> = Vec::new();
    let mut default_user_jvm: Vec<serde_json::Value> = Vec::new();
    let mut libraries: Vec<Library> = Vec::new();
    let mut client: Option<DownloadArtifact> = None;
    let mut jar_id = String::new();
    let mut total_download_size = 0u64;
    let top_id = chain
        .last()
        .map(|json| json.id.clone())
        .ok_or_else(|| anyhow!("空版本链"))?;
    let top_inherits = chain.last().and_then(|json| json.inherits_from.clone());

    for json in &chain {
        if json.version_type != "release" || version_type == "release" {
            version_type = json.version_type.clone();
        }
        if let Some(value) = &json.main_class {
            main_class = Some(value.clone());
        }
        if let Some(value) = &json.release_time {
            release_time = Some(value.clone());
        }
        if let Some(value) = &json.assets {
            assets = Some(value.clone());
        }
        if let Some(value) = &json.asset_index {
            asset_index = Some(value.clone());
        }
        if let Some(value) = json.java_version.as_ref().and_then(|j| j.major_version) {
            java_major = Some(value);
        }
        if let Some(value) = &json.logging {
            logging = Some(value.clone());
        }
        if let Some(value) = &json.minecraft_arguments {
            minecraft_arguments = Some(value.clone());
        }
        if let Some(arguments) = &json.arguments {
            game_args.extend(arguments.game.clone());
            jvm_args.extend(arguments.jvm.clone());
            default_user_jvm.extend(arguments.default_user_jvm.clone());
        }
        for library in &json.libraries {
            if !libraries
                .iter()
                .any(|existing| existing.name == library.name)
            {
                libraries.push(library.clone());
            }
        }
        if let Some(download) = json.downloads.as_ref().and_then(|d| d.client.as_ref()) {
            total_download_size += download.size.unwrap_or(0);
            client = Some(download.clone());
            jar_id = json.id.clone();
        }
    }
    if jar_id.is_empty() {
        jar_id = top_id.clone();
    }
    let main_class = main_class.ok_or_else(|| anyhow!("版本缺少 mainClass，无法启动"))?;
    if version_type.is_empty() {
        version_type = "release".to_string();
    }

    let platform = Platform::current();
    let mut seen_files = HashSet::new();
    for library in &libraries {
        for file in library_files(library, &platform) {
            if seen_files.insert(file.path.clone()) {
                total_download_size += file.size.unwrap_or(0);
            }
        }
    }
    if let Some(index) = &asset_index {
        total_download_size += index.total_size.unwrap_or(index.size.unwrap_or(0));
    }
    if let Some(log) = logging
        .as_ref()
        .and_then(|l| l.client.as_ref())
        .and_then(|c| c.file.as_ref())
    {
        total_download_size += log.size.unwrap_or(0);
    }

    Ok(ResolvedVersion {
        id: top_id,
        version_type,
        release_time,
        main_class,
        assets_id: assets.or_else(|| asset_index.as_ref().map(|a| a.id.clone())),
        asset_index,
        libraries,
        arguments: Some(Arguments {
            game: game_args,
            jvm: jvm_args,
            default_user_jvm,
        }),
        minecraft_arguments,
        java_major,
        inherits_from: top_inherits,
        jar_id,
        client,
        logging,
        total_download_size,
    })
}

pub fn rules_allow(rules: &[Rule], platform: &Platform, features: &HashMap<String, bool>) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for rule in rules {
        let mut applies = true;
        if let Some(os) = &rule.os {
            applies &= platform.matches_os(os);
        }
        if let Some(required) = &rule.features {
            for (name, expected) in required {
                if features.get(name).copied().unwrap_or(false) != *expected {
                    applies = false;
                    break;
                }
            }
        }
        if applies {
            allowed = rule.action == "allow";
        }
    }
    allowed
}

pub fn library_allowed(
    library: &Library,
    platform: &Platform,
    features: &HashMap<String, bool>,
) -> bool {
    if !rules_allow(&library.rules, platform, features) {
        return false;
    }
    native_artifact_allowed(&library.name, platform)
}

/// Modern Mojang profiles list the native JARs for every architecture as
/// separate libraries, all with the same OS-only rule. The official launcher
/// only puts the matching architecture on the classpath; we do the same.
fn native_artifact_allowed(name: &str, platform: &Platform) -> bool {
    let classifier = name.split(':').nth(3).unwrap_or_default();
    if !classifier.starts_with("natives-") {
        return true;
    }
    let expected = match platform.os {
        "windows" => match platform.arch {
            "arm64" => "natives-windows-arm64",
            "x86" => "natives-windows-x86",
            _ => "natives-windows",
        },
        "osx" => match platform.arch {
            "arm64" => "natives-macos-arm64",
            _ => "natives-macos",
        },
        "linux" => match platform.arch {
            "arm64" => "natives-linux-arm64",
            "x86" => "natives-linux-x86",
            _ => "natives-linux",
        },
        _ => return false,
    };
    classifier == expected
        || classifier == format!("{expected}-64")
        || (expected == "natives-windows" && classifier == "natives-windows-64")
}

pub fn native_classifier(library: &Library, platform: &Platform) -> Option<String> {
    let natives = library.natives.as_ref()?;
    let key = natives.get(platform.os)?;
    Some(key.replace("${arch}", &platform.arch_bits.to_string()))
}

pub fn maven_path_from_coords(coords: &str) -> Option<String> {
    let mut parts = coords.split(':');
    let group = parts.next()?;
    let artifact = parts.next()?;
    let version = parts.next()?;
    let classifier = parts.next();
    let (version, extension) = if let Some((v, ext)) = version.split_once('@') {
        (v, ext)
    } else {
        (version, "jar")
    };
    if group.is_empty() || artifact.is_empty() || version.is_empty() {
        return None;
    }
    let file = match classifier {
        Some(classifier) if !classifier.is_empty() => {
            format!("{artifact}-{version}-{classifier}.{extension}")
        }
        _ => format!("{artifact}-{version}.{extension}"),
    };
    Some(format!(
        "{}/{}/{}/{}",
        group.replace('.', "/"),
        artifact,
        version,
        file
    ))
}

fn artifact_path(library: &Library, artifact: &DownloadArtifact) -> Option<String> {
    artifact
        .path
        .clone()
        .or_else(|| maven_path_from_coords(&library.name))
}

fn fallback_url(library: &Library, path: &str) -> String {
    let base = library
        .url
        .as_deref()
        .filter(|url| !url.trim().is_empty())
        .unwrap_or("https://libraries.minecraft.net/");
    format!("{}/{}", base.trim_end_matches('/'), path)
}

pub fn library_files(library: &Library, platform: &Platform) -> Vec<LibraryFile> {
    if !library_allowed(library, platform, &HashMap::new()) {
        return Vec::new();
    }
    let mut result = Vec::new();
    if let Some(downloads) = &library.downloads {
        if let Some(artifact) = &downloads.artifact {
            if let Some(path) = artifact_path(library, artifact) {
                let url = if artifact.url.trim().is_empty() {
                    fallback_url(library, &path)
                } else {
                    artifact.url.clone()
                };
                result.push(LibraryFile {
                    path,
                    url,
                    sha1: artifact.sha1.clone(),
                    size: artifact.size,
                    kind: LibraryKind::Classpath,
                    exclude: library
                        .extract
                        .as_ref()
                        .map(|e| e.exclude.clone())
                        .unwrap_or_default(),
                });
            }
        }
    }
    if let Some(classifier) = native_classifier(library, platform) {
        let artifact = library
            .downloads
            .as_ref()
            .and_then(|d| d.classifiers.as_ref())
            .and_then(|c| c.get(&classifier));
        if let Some(artifact) = artifact {
            if let Some(path) = artifact_path(library, artifact) {
                let url = if artifact.url.trim().is_empty() {
                    fallback_url(library, &path)
                } else {
                    artifact.url.clone()
                };
                result.push(LibraryFile {
                    path,
                    url,
                    sha1: artifact.sha1.clone(),
                    size: artifact.size,
                    kind: LibraryKind::Native,
                    exclude: library
                        .extract
                        .as_ref()
                        .map(|e| e.exclude.clone())
                        .unwrap_or_default(),
                });
            }
        } else if library
            .downloads
            .as_ref()
            .and_then(|d| d.artifact.as_ref())
            .is_none()
        {
            // Legacy Forge metadata sometimes only provides the natives mapping.
            if let Some(path) = maven_path_from_coords(&format!("{}:{}", library.name, classifier))
            {
                result.push(LibraryFile {
                    url: fallback_url(library, &path),
                    path,
                    sha1: None,
                    size: None,
                    kind: LibraryKind::Native,
                    exclude: Vec::new(),
                });
            }
        }
    }
    if result.is_empty() {
        // Fabric's profile libraries usually contain just `name` + `url`.
        if let Some(path) = maven_path_from_coords(&library.name) {
            result.push(LibraryFile {
                url: fallback_url(library, &path),
                path,
                sha1: None,
                size: None,
                kind: LibraryKind::Classpath,
                exclude: Vec::new(),
            });
        }
    }
    result
}

pub fn classpath_entries(
    root: &Path,
    resolved: &ResolvedVersion,
    platform: &Platform,
) -> Vec<PathBuf> {
    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    for library in &resolved.libraries {
        for file in library_files(library, platform) {
            if file.kind == LibraryKind::Classpath {
                let path = libraries_dir(root).join(&file.path);
                if seen.insert(path.clone()) {
                    entries.push(path);
                }
            }
        }
    }
    let client = effective_client_jar(root, resolved);
    if seen.insert(client.clone()) {
        entries.push(client);
    }
    entries
}

pub fn argument_values(
    values: &[serde_json::Value],
    platform: &Platform,
    features: &HashMap<String, bool>,
) -> Vec<String> {
    let mut output = Vec::new();
    for value in values {
        match value {
            serde_json::Value::String(text) => output.push(text.clone()),
            serde_json::Value::Object(object) => {
                let rules: Vec<Rule> = object
                    .get("rules")
                    .cloned()
                    .and_then(|v| serde_json::from_value(v).ok())
                    .unwrap_or_default();
                if !rules_allow(&rules, platform, features) {
                    continue;
                }
                match object.get("value") {
                    Some(serde_json::Value::String(text)) => output.push(text.clone()),
                    Some(serde_json::Value::Array(items)) => {
                        output.extend(items.iter().filter_map(|v| v.as_str().map(str::to_string)));
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    output
}

pub fn split_game_args(raw: &str) -> Vec<String> {
    raw.split_whitespace().map(str::to_string).collect()
}

/// Pick a usable client jar path for downloads.
pub fn client_jar_destination(root: &Path, resolved: &ResolvedVersion) -> PathBuf {
    versions_dir(root)
        .join(&resolved.jar_id)
        .join(format!("{}.jar", resolved.jar_id))
}

pub fn natives_directory(root: &Path, resolved: &ResolvedVersion) -> PathBuf {
    versions_dir(root).join(&resolved.id).join("natives")
}

pub fn main_class(resolved: &ResolvedVersion) -> &str {
    &resolved.main_class
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_version(root: &Path, json: serde_json::Value) {
        let id = json["id"].as_str().unwrap();
        let dir = root.join("versions").join(id);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join(format!("{id}.json")),
            serde_json::to_vec_pretty(&json).unwrap(),
        )
        .unwrap();
    }

    fn platform() -> Platform {
        Platform {
            os: "windows",
            arch: "x86_64",
            arch_bits: 64,
        }
    }

    #[test]
    fn merges_inherited_profiles_and_libraries() {
        let temp = tempfile::tempdir().unwrap();
        write_version(
            temp.path(),
            serde_json::json!({
                "id": "1.20.1",
                "type": "release",
                "mainClass": "net.minecraft.client.main.Main",
                "releaseTime": "2023-06-12T00:00:00+00:00",
                "assets": "5",
                "assetIndex": {"id": "5", "url": "https://example.invalid/5.json", "size": 100, "totalSize": 200},
                "javaVersion": {"majorVersion": 17},
                "libraries": [{"name": "org.example:base:1.0", "downloads": {"artifact": {"path": "org/example/base/1.0/base-1.0.jar", "url": "https://example.invalid/base.jar", "size": 10}}}],
                "arguments": {"game": ["--username", "${auth_player_name}"], "jvm": ["-cp", "${classpath}"]},
                "downloads": {"client": {"url": "https://example.invalid/client.jar", "size": 50}}
            }),
        );
        write_version(
            temp.path(),
            serde_json::json!({
                "id": "fabric-loader-0.16.9-1.20.1",
                "inheritsFrom": "1.20.1",
                "type": "release",
                "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
                "libraries": [{"name": "net.fabricmc:fabric-loader:0.16.9", "url": "https://maven.fabricmc.net/"}]
            }),
        );
        let resolved = resolve_version(temp.path(), "fabric-loader-0.16.9-1.20.1").unwrap();
        assert_eq!(
            resolved.main_class,
            "net.fabricmc.loader.impl.launch.knot.KnotClient"
        );
        assert_eq!(resolved.jar_id, "1.20.1");
        assert_eq!(resolved.java_major, Some(17));
        assert_eq!(resolved.libraries.len(), 2);
        assert!(resolved.arguments.unwrap().game.iter().count() == 2);
        let summary = scan_versions(temp.path()).unwrap();
        assert_eq!(summary.len(), 2);
        let fabric = summary.iter().find(|v| v.loader == "fabric").unwrap();
        assert_eq!(fabric.loader_version.as_deref(), Some("0.16.9"));
    }

    #[test]
    fn evaluates_os_rules() {
        let p = platform();
        let features = HashMap::new();
        let win: Rule =
            serde_json::from_value(serde_json::json!({"action":"allow","os":{"name":"windows"}}))
                .unwrap();
        let mac: Rule =
            serde_json::from_value(serde_json::json!({"action":"allow","os":{"name":"osx"}}))
                .unwrap();
        assert!(rules_allow(&[win], &p, &features));
        assert!(!rules_allow(&[mac], &p, &features));
        assert!(rules_allow(&[], &p, &features));
    }

    #[test]
    fn computes_maven_coordinates() {
        assert_eq!(
            maven_path_from_coords("org.lwjgl:lwjgl:3.3.3:natives-windows").as_deref(),
            Some("org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar")
        );
        assert_eq!(
            maven_path_from_coords("net.fabricmc:fabric-loader:0.16.9").as_deref(),
            Some("net/fabricmc/fabric-loader/0.16.9/fabric-loader-0.16.9.jar")
        );
    }

    #[test]
    fn finds_modern_native_artifact_on_classpath() {
        let library: Library = serde_json::from_value(serde_json::json!({
            "name": "org.lwjgl:lwjgl:3.4.1:natives-windows",
            "downloads": {"artifact": {"path": "org/lwjgl/lwjgl/3.4.1/lwjgl-3.4.1-natives-windows.jar", "url": "https://example.invalid/native.jar", "size": 1}}
        }))
        .unwrap();
        let files = library_files(&library, &platform());
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].kind, LibraryKind::Classpath);
    }

    #[test]
    fn filters_native_artifacts_for_current_architecture() {
        let x64 = platform();
        let arm: Library = serde_json::from_value(serde_json::json!({
            "name": "org.lwjgl:lwjgl:3.4.1:natives-windows-arm64",
            "downloads": {"artifact": {"path": "org/lwjgl/lwjgl/3.4.1/lwjgl-3.4.1-natives-windows-arm64.jar", "url": "https://example.invalid/native.jar", "size": 1}}
        }))
        .unwrap();
        let x86: Library = serde_json::from_value(serde_json::json!({
            "name": "org.lwjgl:lwjgl:3.4.1:natives-windows-x86",
            "downloads": {"artifact": {"path": "org/lwjgl/lwjgl/3.4.1/lwjgl-3.4.1-natives-windows-x86.jar", "url": "https://example.invalid/native.jar", "size": 1}}
        }))
        .unwrap();
        let base: Library = serde_json::from_value(serde_json::json!({
            "name": "org.lwjgl:lwjgl:3.4.1:natives-windows",
            "downloads": {"artifact": {"path": "org/lwjgl/lwjgl/3.4.1/lwjgl-3.4.1-natives-windows.jar", "url": "https://example.invalid/native.jar", "size": 1}}
        }))
        .unwrap();
        assert!(library_files(&arm, &x64).is_empty());
        assert!(library_files(&x86, &x64).is_empty());
        assert_eq!(library_files(&base, &x64).len(), 1);
    }

    #[test]
    fn extracts_legacy_native_classifier() {
        let library: Library = serde_json::from_value(serde_json::json!({
            "name": "org.lwjgl:lwjgl:2.9.4",
            "natives": {"windows": "natives-windows-${arch}"},
            "downloads": {
                "artifact": {"path": "org/lwjgl/lwjgl/2.9.4/lwjgl-2.9.4.jar", "url": "https://example.invalid/lwjgl.jar", "size": 1},
                "classifiers": {"natives-windows-64": {"path": "org/lwjgl/lwjgl/2.9.4/lwjgl-2.9.4-natives-windows-64.jar", "url": "https://example.invalid/native.jar", "size": 1}}
            }
        }))
        .unwrap();
        let files = library_files(&library, &platform());
        assert!(files.iter().any(|f| f.kind == LibraryKind::Classpath));
        assert!(files.iter().any(|f| f.kind == LibraryKind::Native));
    }
}
