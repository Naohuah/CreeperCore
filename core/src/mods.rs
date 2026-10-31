//! Mod discovery, metadata inspection, health checks and safe file operations.

use crate::model::{ModActionReport, ModInfo};
use crate::util;
use anyhow::{bail, Context, Result};
use serde_json::Value as JsonValue;
use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

const BUILTIN_DEPENDENCIES: &[&str] = &[
    "minecraft",
    "java",
    "fabricloader",
    "fabric-loader",
    "quilt_loader",
    "quilt-loader",
    "forge",
    "neoforge",
    "javafml",
    "lowcodefml",
    "mixinextras",
];

pub fn mods_dir(game_dir: &Path) -> PathBuf {
    game_dir.join("mods")
}

pub fn list_mods(game_dir: &Path) -> Result<Vec<ModInfo>> {
    let dir = mods_dir(game_dir);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut mods = Vec::new();
    let mut provided_ids = HashSet::new();
    for entry in
        std::fs::read_dir(&dir).with_context(|| format!("无法读取模组目录：{}", dir.display()))?
    {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().to_string();
        let enabled = file_name.to_ascii_lowercase().ends_with(".jar");
        let disabled = file_name.to_ascii_lowercase().ends_with(".jar.disabled");
        if !enabled && !disabled {
            continue;
        }
        let meta = entry.metadata().ok();
        let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
        let modified = meta
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let (metadata, read_error) = match inspect_jar(&path) {
            Ok(meta) => (meta, None),
            Err(err) => (ModMetadata::fallback(&file_name), Some(err.to_string())),
        };
        if let Some(id) = &metadata.id {
            provided_ids.insert(id.to_ascii_lowercase());
        }
        for id in &metadata.provides {
            provided_ids.insert(id.to_ascii_lowercase());
        }
        mods.push(ModInfo {
            file_name: file_name.clone(),
            path: path.to_string_lossy().to_string(),
            name: metadata
                .name
                .filter(|name| !name.trim().is_empty())
                .unwrap_or_else(|| fallback_name(&file_name)),
            mod_id: metadata.id,
            version: metadata.version,
            description: metadata.description,
            authors: metadata.authors,
            loader: metadata.loader,
            enabled,
            size,
            modified,
            missing_dependencies: metadata.dependencies,
            duplicate: false,
            read_error,
        });
    }

    let mut counts: HashMap<String, usize> = HashMap::new();
    for item in &mods {
        if let Some(id) = &item.mod_id {
            *counts.entry(id.to_ascii_lowercase()).or_default() += 1;
        }
    }
    let installed = provided_ids;
    for item in &mut mods {
        if let Some(id) = &item.mod_id {
            if counts.get(&id.to_ascii_lowercase()).copied().unwrap_or(0) > 1 {
                item.duplicate = true;
            }
        }
        let self_id = item.mod_id.as_ref().map(|id| id.to_ascii_lowercase());
        item.missing_dependencies.retain(|dep| {
            let lower = dep.to_ascii_lowercase();
            !BUILTIN_DEPENDENCIES.contains(&lower.as_str())
                && !installed.contains(&lower)
                && Some(&lower) != self_id.as_ref()
        });
        item.missing_dependencies.sort();
        item.missing_dependencies.dedup();
    }
    mods.sort_by(|a, b| {
        b.enabled
            .cmp(&a.enabled)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(mods)
}

pub fn set_mod_enabled(game_dir: &Path, file_name: &str, enabled: bool) -> Result<String> {
    let dir = mods_dir(game_dir);
    let safe_name = util::safe_component(file_name)?;
    let source = dir.join(&safe_name);
    if !source.is_file() {
        bail!("模组文件不存在：{safe_name}");
    }
    let lower = safe_name.to_ascii_lowercase();
    let target_name = if enabled {
        if lower.ends_with(".jar") {
            return Ok(safe_name);
        }
        if !lower.ends_with(".jar.disabled") {
            bail!("不是可识别的模组文件：{safe_name}");
        }
        safe_name
            .strip_suffix(".disabled")
            .unwrap_or(&safe_name)
            .to_string()
    } else {
        if lower.ends_with(".jar.disabled") {
            return Ok(safe_name);
        }
        if !lower.ends_with(".jar") {
            bail!("不是可识别的模组文件：{safe_name}");
        }
        format!("{safe_name}.disabled")
    };
    let target = dir.join(&target_name);
    if target.exists() {
        bail!("目标文件已存在：{target_name}");
    }
    std::fs::rename(&source, &target)
        .with_context(|| format!("无法重命名 {}", source.display()))?;
    Ok(target_name)
}

pub fn delete_mod(game_dir: &Path, file_name: &str) -> Result<String> {
    let dir = mods_dir(game_dir);
    let safe_name = util::safe_component(file_name)?;
    let source = dir.join(&safe_name);
    if !source.is_file() {
        bail!("模组文件不存在：{safe_name}");
    }
    std::fs::remove_file(&source)
        .with_context(|| format!("无法删除模组：{}", source.display()))?;
    Ok(source.to_string_lossy().to_string())
}

pub fn import_mods(game_dir: &Path, files: &[String]) -> ModActionReport {
    let dir = mods_dir(game_dir);
    let mut report = ModActionReport::default();
    let _ = std::fs::create_dir_all(&dir);
    for raw in files {
        let source = PathBuf::from(raw);
        let Some(file_name) = source.file_name().and_then(|n| n.to_str()) else {
            report.errors.push(format!("无效路径：{raw}"));
            continue;
        };
        if !file_name.to_ascii_lowercase().ends_with(".jar") {
            report
                .skipped
                .push(format!("{file_name}：仅支持 .jar 模组"));
            continue;
        }
        if !is_valid_zip(&source) {
            report
                .errors
                .push(format!("{file_name}：不是有效的 JAR 文件"));
            continue;
        }
        let safe_name = match util::sanitize_component(file_name) {
            Some(name) => name,
            None => {
                report.errors.push(format!("非法文件名：{file_name}"));
                continue;
            }
        };
        let target = dir.join(&safe_name);
        if target.exists() {
            report.skipped.push(format!("{safe_name}：已存在同名模组"));
            continue;
        }
        match std::fs::copy(&source, &target) {
            Ok(_) => report.changed.push(safe_name),
            Err(err) => report.errors.push(format!("{safe_name}：{err}")),
        }
    }
    report
}

fn is_valid_zip(path: &Path) -> bool {
    std::fs::File::open(path)
        .ok()
        .and_then(|file| zip::ZipArchive::new(file).ok())
        .is_some()
}

#[derive(Debug, Clone, Default)]
struct ModMetadata {
    id: Option<String>,
    name: Option<String>,
    version: Option<String>,
    description: Option<String>,
    authors: Vec<String>,
    loader: String,
    dependencies: Vec<String>,
    /// IDs exposed by this JAR and its nested JARs (Fabric API modules, JiJ libraries).
    provides: Vec<String>,
}

impl ModMetadata {
    fn fallback(file_name: &str) -> Self {
        Self {
            name: Some(fallback_name(file_name)),
            loader: "unknown".to_string(),
            ..Default::default()
        }
    }
}

fn fallback_name(file_name: &str) -> String {
    let mut name = file_name
        .trim_end_matches(".disabled")
        .trim_end_matches(".jar")
        .to_string();
    if let Some(index) = name.rfind(|c: char| c == '-' || c == '+') {
        let suffix = &name[index + 1..];
        if suffix.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            name.truncate(index);
        }
    }
    name.replace(['_', '-'], " ").trim().to_string()
}

fn read_zip_entry(path: &Path, names: &[&str]) -> Result<Option<(String, Vec<u8>)>> {
    let file =
        std::fs::File::open(path).with_context(|| format!("无法打开模组：{}", path.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .with_context(|| format!("模组不是有效的 ZIP：{}", path.display()))?;
    for name in names {
        if let Ok(mut entry) = archive.by_name(name) {
            if entry.size() > 4 * 1024 * 1024 {
                bail!("元数据文件过大：{name}");
            }
            let mut data = Vec::new();
            entry.read_to_end(&mut data)?;
            return Ok(Some(((*name).to_string(), data)));
        }
    }
    Ok(None)
}

fn inspect_jar(path: &Path) -> Result<ModMetadata> {
    let mut metadata = inspect_jar_base(path)?;
    metadata.provides.extend(nested_provided_ids(path));
    metadata.provides.sort();
    metadata.provides.dedup();
    Ok(metadata)
}

fn inspect_jar_base(path: &Path) -> Result<ModMetadata> {
    if let Some((_, raw)) = read_zip_entry(path, &["quilt.mod.json"])? {
        if let Ok(json) = serde_json::from_slice::<JsonValue>(&raw) {
            if let Some(meta) = parse_quilt(&json) {
                return Ok(meta);
            }
        }
    }
    if let Some((_, raw)) = read_zip_entry(path, &["fabric.mod.json"])? {
        if let Ok(json) = serde_json::from_slice::<JsonValue>(&raw) {
            return Ok(parse_fabric(&json));
        }
        bail!("fabric.mod.json 无法解析");
    }
    for candidate in ["META-INF/neoforge.mods.toml", "META-INF/mods.toml"] {
        if let Some((_, raw)) = read_zip_entry(path, &[candidate])? {
            let text = String::from_utf8_lossy(&raw);
            if let Ok(value) = text.parse::<toml::Value>() {
                return Ok(parse_forge(&value, candidate.contains("neoforge")));
            }
        }
    }
    if let Some((_, raw)) = read_zip_entry(path, &["mcmod.info"])? {
        if let Ok(json) = serde_json::from_slice::<JsonValue>(&raw) {
            return Ok(parse_mcmod(&json));
        }
    }
    Ok(ModMetadata::fallback(
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("mod.jar"),
    ))
}

fn nested_provided_ids(path: &Path) -> Vec<String> {
    let mut ids = Vec::new();
    let Ok(file) = std::fs::File::open(path) else {
        return ids;
    };
    let Ok(mut archive) = zip::ZipArchive::new(file) else {
        return ids;
    };
    let names: Vec<String> = archive
        .file_names()
        .filter(|name| name.starts_with("META-INF/jars/") && name.ends_with(".jar"))
        .map(str::to_string)
        .collect();
    for name in names {
        let Ok(mut entry) = archive.by_name(&name) else {
            continue;
        };
        if entry.size() > 64 * 1024 * 1024 {
            continue;
        }
        let mut bytes = Vec::new();
        if entry.read_to_end(&mut bytes).is_err() {
            continue;
        }
        let Ok(mut nested) = zip::ZipArchive::new(Cursor::new(bytes)) else {
            continue;
        };
        let Ok(mut metadata) = nested.by_name("fabric.mod.json") else {
            continue;
        };
        let mut raw = String::new();
        if metadata.read_to_string(&mut raw).is_err() {
            continue;
        }
        let Ok(json) = serde_json::from_str::<JsonValue>(&raw) else {
            continue;
        };
        if let Some(id) = json.get("id").and_then(JsonValue::as_str) {
            ids.push(id.to_string());
        }
        if let Some(provides) = json.get("provides").and_then(JsonValue::as_array) {
            ids.extend(
                provides
                    .iter()
                    .filter_map(JsonValue::as_str)
                    .map(str::to_string),
            );
        }
    }
    ids
}

fn parse_fabric(json: &JsonValue) -> ModMetadata {
    let id = json
        .get("id")
        .and_then(JsonValue::as_str)
        .map(str::to_string);
    let name = json
        .get("name")
        .and_then(JsonValue::as_str)
        .map(str::to_string);
    let version = json
        .get("version")
        .and_then(JsonValue::as_str)
        .map(str::to_string);
    let description = json
        .get("description")
        .and_then(JsonValue::as_str)
        .map(str::to_string);
    let authors = parse_authors(json.get("authors"));
    let mut dependencies = Vec::new();
    if let Some(depends) = json.get("depends").and_then(JsonValue::as_object) {
        for (dep, _) in depends {
            dependencies.push(dep.clone());
        }
    }
    let provides = json
        .get("provides")
        .and_then(JsonValue::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(JsonValue::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    ModMetadata {
        id,
        name,
        version,
        description,
        authors,
        loader: "fabric".to_string(),
        dependencies,
        provides,
    }
}

fn parse_quilt(json: &JsonValue) -> Option<ModMetadata> {
    let loader = json.get("quilt_loader")?;
    let id = loader
        .get("id")
        .and_then(JsonValue::as_str)
        .map(str::to_string);
    let version = loader
        .get("version")
        .and_then(JsonValue::as_str)
        .map(str::to_string);
    let metadata = loader.get("metadata");
    let name = metadata
        .and_then(|m| m.get("name"))
        .and_then(JsonValue::as_str)
        .map(str::to_string);
    let description = metadata
        .and_then(|m| m.get("description"))
        .and_then(JsonValue::as_str)
        .map(str::to_string);
    let authors = metadata
        .and_then(|m| m.get("contributors"))
        .and_then(JsonValue::as_object)
        .map(|object| object.keys().cloned().collect())
        .unwrap_or_default();
    let mut dependencies = Vec::new();
    if let Some(depends) = loader.get("depends") {
        match depends {
            JsonValue::Array(items) => {
                for item in items {
                    if item.get("optional").and_then(JsonValue::as_bool) == Some(true) {
                        continue;
                    }
                    if let Some(dep) = item.get("id").and_then(JsonValue::as_str) {
                        dependencies.push(dep.to_string());
                    }
                }
            }
            JsonValue::Object(map) => {
                dependencies.extend(map.keys().cloned());
            }
            _ => {}
        }
    }
    Some(ModMetadata {
        id,
        name,
        version,
        description,
        authors,
        loader: "quilt".to_string(),
        dependencies,
        provides: Vec::new(),
    })
}

fn parse_forge(value: &toml::Value, neo: bool) -> ModMetadata {
    let mods = value.get("mods").and_then(toml::Value::as_array);
    let first = mods.and_then(|items| items.first());
    let get_str = |key: &str| {
        first
            .and_then(|table| table.get(key))
            .and_then(toml::Value::as_str)
            .map(str::to_string)
    };
    let authors = first
        .and_then(|table| table.get("authors"))
        .and_then(toml::Value::as_str)
        .map(|text| {
            text.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let mut dependencies = Vec::new();
    if let Some(table) = value.get("dependencies").and_then(toml::Value::as_table) {
        for (_, entries) in table {
            if let Some(entries) = entries.as_array() {
                for entry in entries {
                    let mandatory = entry
                        .get("mandatory")
                        .and_then(toml::Value::as_bool)
                        .unwrap_or(true);
                    if !mandatory {
                        continue;
                    }
                    if let Some(dep) = entry.get("modId").and_then(toml::Value::as_str) {
                        dependencies.push(dep.to_string());
                    }
                }
            }
        }
    }
    ModMetadata {
        id: get_str("modId"),
        name: get_str("displayName"),
        version: get_str("version"),
        description: get_str("description"),
        authors,
        loader: if neo {
            "neoforge".to_string()
        } else {
            "forge".to_string()
        },
        dependencies,
        provides: Vec::new(),
    }
}

fn parse_mcmod(json: &JsonValue) -> ModMetadata {
    let item = match json {
        JsonValue::Array(items) => items.first().cloned().unwrap_or(JsonValue::Null),
        JsonValue::Object(map) => map
            .get("modList")
            .and_then(JsonValue::as_array)
            .and_then(|items| items.first())
            .cloned()
            .unwrap_or(JsonValue::Null),
        _ => JsonValue::Null,
    };
    let authors = parse_authors(item.get("authorList").or_else(|| item.get("authors")));
    let mut dependencies = Vec::new();
    if let Some(required) = item.get("requiredMods").and_then(JsonValue::as_array) {
        for dep in required {
            if let Some(text) = dep.as_str() {
                if let Some(id) = text.split('@').next() {
                    dependencies.push(id.to_string());
                }
            }
        }
    }
    ModMetadata {
        id: item
            .get("modid")
            .and_then(JsonValue::as_str)
            .map(str::to_string),
        name: item
            .get("name")
            .and_then(JsonValue::as_str)
            .map(str::to_string),
        version: item
            .get("version")
            .and_then(JsonValue::as_str)
            .map(str::to_string),
        description: item
            .get("description")
            .and_then(JsonValue::as_str)
            .map(str::to_string),
        authors,
        loader: "legacy".to_string(),
        dependencies,
        provides: Vec::new(),
    }
}

fn parse_authors(value: Option<&JsonValue>) -> Vec<String> {
    match value {
        Some(JsonValue::Array(items)) => items
            .iter()
            .filter_map(|item| match item {
                JsonValue::String(text) => Some(text.clone()),
                JsonValue::Object(obj) => obj
                    .get("name")
                    .and_then(JsonValue::as_str)
                    .map(str::to_string),
                _ => None,
            })
            .filter(|s| !s.trim().is_empty())
            .collect(),
        Some(JsonValue::String(text)) => vec![text.clone()],
        Some(JsonValue::Object(object)) => object.keys().cloned().collect(),
        _ => Vec::new(),
    }
}

pub fn mods_folder_size(game_dir: &Path) -> u64 {
    let dir = mods_dir(game_dir);
    std::fs::read_dir(dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}

pub fn quick_mod_summary(game_dir: &Path) -> (usize, usize) {
    let dir = mods_dir(game_dir);
    let Ok(entries) = std::fs::read_dir(dir) else {
        return (0, 0);
    };
    let mut enabled = 0;
    let mut disabled = 0;
    for entry in entries.filter_map(|e| e.ok()) {
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if name.ends_with(".jar.disabled") {
            disabled += 1;
        } else if name.ends_with(".jar") {
            enabled += 1;
        }
    }
    (enabled, disabled)
}

pub fn ensure_mods_dir(game_dir: &Path) -> Result<PathBuf> {
    let dir = mods_dir(game_dir);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn make_jar(dir: &Path, name: &str, metadata: &str) -> PathBuf {
        let path = dir.join(name);
        let file = std::fs::File::create(&path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file("fabric.mod.json", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(metadata.as_bytes()).unwrap();
        zip.finish().unwrap();
        path
    }

    #[test]
    fn reads_fabric_metadata_and_missing_dependencies() {
        let temp = tempfile::tempdir().unwrap();
        let game = temp.path();
        std::fs::create_dir_all(mods_dir(game)).unwrap();
        make_jar(
            &mods_dir(game),
            "example-mod-1.0.jar",
            r#"{"id":"example","name":"Example Mod","version":"1.0","description":"A test","authors":["Alex"],"depends":{"minecraft":">=1.20","fabric-api":"*"}}"#,
        );
        let mods = list_mods(game).unwrap();
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].mod_id.as_deref(), Some("example"));
        assert_eq!(mods[0].loader, "fabric");
        assert_eq!(mods[0].authors, vec!["Alex"]);
        assert!(mods[0]
            .missing_dependencies
            .iter()
            .any(|d| d == "fabric-api"));
    }

    #[test]
    fn treats_nested_jars_as_provided_dependencies() {
        let temp = tempfile::tempdir().unwrap();
        let game = temp.path();
        std::fs::create_dir_all(mods_dir(game)).unwrap();

        let mut nested_bytes = Vec::new();
        {
            let mut nested = zip::ZipWriter::new(std::io::Cursor::new(&mut nested_bytes));
            nested
                .start_file("fabric.mod.json", SimpleFileOptions::default())
                .unwrap();
            nested
                .write_all(br#"{"id":"xaerolib","provides":["nested-api"]}"#)
                .unwrap();
            nested.finish().unwrap();
        }

        let path = mods_dir(game).join("xaero.jar");
        let file = std::fs::File::create(&path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file("fabric.mod.json", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(br#"{"id":"xaero","name":"Xaero","depends":{"xaerolib":"*","nested-api":"*"}}"#)
            .unwrap();
        zip.start_file("META-INF/jars/xaerolib.jar", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(&nested_bytes).unwrap();
        zip.finish().unwrap();

        let mods = list_mods(game).unwrap();
        assert_eq!(mods.len(), 1);
        assert!(mods[0].missing_dependencies.is_empty(), "{:?}", mods[0].missing_dependencies);
    }

    #[test]
    fn toggles_and_deletes_mods() {
        let temp = tempfile::tempdir().unwrap();
        let game = temp.path();
        std::fs::create_dir_all(mods_dir(game)).unwrap();
        make_jar(
            &mods_dir(game),
            "a.jar",
            r#"{"id":"a","name":"A","version":"1"}"#,
        );
        let disabled = set_mod_enabled(game, "a.jar", false).unwrap();
        assert_eq!(disabled, "a.jar.disabled");
        assert!(!mods_dir(game).join("a.jar").exists());
        let enabled = set_mod_enabled(game, "a.jar.disabled", true).unwrap();
        assert_eq!(enabled, "a.jar");
        let deleted = delete_mod(game, "a.jar").unwrap();
        assert!(!Path::new(&deleted).exists());
    }
}
