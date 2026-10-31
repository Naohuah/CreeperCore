//! PCL2-style offline skin support.
//!
//! Minecraft has no API for a launcher to attach a skin to an offline profile.
//! The practical vanilla-compatible approach is a resource pack that replaces
//! the default player textures, plus updating `options.txt` so the pack is
//! enabled. PCL2 does the same thing with `PCL2 Skin.zip`.

use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;

pub const PACK_FILE: &str = "CreeperCore Skin.zip";
const PACK_ENTRY: &str = "file/CreeperCore Skin.zip";
const MAX_SKIN_BYTES: usize = 1_500_000;
const DEFAULT_NAMES: [&str; 9] = [
    "alex", "ari", "efe", "kai", "makena", "noor", "steve", "sunny", "zuri",
];

pub fn skins_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("skins")
}

pub fn local_skin_path(data_dir: &Path, account_id: &str) -> Result<PathBuf> {
    if account_id.is_empty()
        || !account_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    {
        bail!("账户 ID 无效");
    }
    Ok(skins_dir(data_dir).join(format!("{account_id}.png")))
}

pub fn save_local_skin(data_dir: &Path, account_id: &str, data_url: &str) -> Result<PathBuf> {
    let payload = data_url
        .strip_prefix("data:image/png;base64,")
        .ok_or_else(|| anyhow::anyhow!("皮肤必须是 PNG data URL"))?;
    let bytes = STANDARD
        .decode(payload)
        .context("皮肤 base64 解码失败")?;
    if bytes.len() > MAX_SKIN_BYTES {
        bail!("皮肤文件不能超过 1.5 MB");
    }
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        bail!("皮肤不是有效的 PNG 文件");
    }
    let path = local_skin_path(data_dir, account_id)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("png.tmp");
    fs::write(&temporary, &bytes)?;
    fs::rename(&temporary, &path)?;
    Ok(path)
}

pub fn delete_local_skin(data_dir: &Path, account_id: &str) -> Result<()> {
    let path = local_skin_path(data_dir, account_id)?;
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

/// Apply the account's local skin, or remove a previously applied pack when
/// the account has no local skin. Failing to apply must not block launch.
pub fn sync_account(
    game_dir: &Path,
    root: &Path,
    version_id: &str,
    data_dir: &Path,
    account_id: &str,
) -> Result<()> {
    let path = local_skin_path(data_dir, account_id)?;
    if path.is_file() {
        let bytes = fs::read(&path)?;
        apply(game_dir, root, version_id, &bytes)?;
    } else {
        clear(game_dir)?;
    }
    Ok(())
}

pub fn apply(game_dir: &Path, root: &Path, version_id: &str, skin_png: &[u8]) -> Result<u32> {
    if skin_png.len() > MAX_SKIN_BYTES {
        bail!("皮肤文件不能超过 1.5 MB");
    }
    if !skin_png.starts_with(b"\x89PNG\r\n\x1a\n") {
        bail!("皮肤不是有效的 PNG 文件");
    }
    let pack_format = resource_pack_format(root, version_id)?;
    let resourcepacks = game_dir.join("resourcepacks");
    fs::create_dir_all(&resourcepacks)?;
    let pack_path = resourcepacks.join(PACK_FILE);
    let temporary = pack_path.with_extension("zip.tmp");
    write_pack(&temporary, skin_png, pack_format)?;
    fs::rename(&temporary, &pack_path)?;
    update_options(&game_dir.join("options.txt"), true)?;
    Ok(pack_format)
}

pub fn clear(game_dir: &Path) -> Result<()> {
    let pack_path = game_dir.join("resourcepacks").join(PACK_FILE);
    if pack_path.exists() {
        fs::remove_file(pack_path)?;
    }
    update_options(&game_dir.join("options.txt"), false)?;
    Ok(())
}

fn write_pack(path: &Path, skin_png: &[u8], pack_format: u32) -> Result<()> {
    let file = fs::File::create(path).context("无法创建离线皮肤资源包")?;
    let mut writer = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);
    let meta = format!(
        "{{\"pack\":{{\"pack_format\":{pack_format},\"supported_formats\":{{\"min_inclusive\":1,\"max_inclusive\":999}},\"description\":\"CreeperCore 离线皮肤\"}}}}"
    );
    writer.start_file("pack.mcmeta", options)?;
    writer.write_all(meta.as_bytes())?;

    // New default-skin layout (1.19.3+) and the legacy two-texture layout.
    for model in ["wide", "slim"] {
        for name in DEFAULT_NAMES {
            writer.start_file(
                format!("assets/minecraft/textures/entity/player/{model}/{name}.png"),
                options,
            )?;
            writer.write_all(skin_png)?;
        }
    }
    for name in ["steve", "alex"] {
        writer.start_file(
            format!("assets/minecraft/textures/entity/{name}.png"),
            options,
        )?;
        writer.write_all(skin_png)?;
    }
    writer.finish()?;
    Ok(())
}

fn update_options(path: &Path, enabled: bool) -> Result<()> {
    let existing = fs::read_to_string(path).unwrap_or_default();
    let mut lines: Vec<String> = existing.lines().map(str::to_string).collect();
    let mut found = false;
    for line in &mut lines {
        if let Some(value) = line.strip_prefix("resourcePacks:") {
            found = true;
            let inner = value.trim().trim_start_matches('[').trim_end_matches(']');
            let mut entries: Vec<String> = inner
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .filter(|item| {
                    *item != format!("\"{PACK_ENTRY}\"")
                        && *item != format!("\"{PACK_FILE}\"")
                })
                .map(str::to_string)
                .collect();
            if enabled {
                if entries.is_empty() {
                    entries.push("\"vanilla\"".to_string());
                }
                entries.push(format!("\"{PACK_ENTRY}\""));
            }
            *line = format!("resourcePacks:[{}]", entries.join(","));
        }
    }
    if enabled && !found {
        lines.push(format!("resourcePacks:[\"vanilla\",\"{PACK_ENTRY}\"]"));
    }
    if !found && !enabled {
        return Ok(());
    }
    let mut text = lines.join("\n");
    text.push('\n');
    let temporary = path.with_extension("txt.tmp");
    fs::write(&temporary, text)?;
    fs::rename(&temporary, path)?;
    Ok(())
}

fn resource_pack_format(root: &Path, version_id: &str) -> Result<u32> {
    if let Some(value) = resource_pack_format_from_json(root, version_id, 0)? {
        return Ok(value);
    }
    Ok(fallback_pack_format(version_id))
}

fn resource_pack_format_from_json(
    root: &Path,
    version_id: &str,
    depth: usize,
) -> Result<Option<u32>> {
    if depth > 8 {
        return Ok(None);
    }
    let version_dir = root.join("versions").join(version_id);
    let json_path = version_dir.join(format!("{version_id}.json"));
    if let Ok(text) = fs::read_to_string(&json_path) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(format) = pack_version_value(&json) {
                return Ok(Some(format));
            }
            if let Some(parent) = json.get("inheritsFrom").and_then(|v| v.as_str()) {
                if let Some(format) = resource_pack_format_from_json(root, parent, depth + 1)? {
                    return Ok(Some(format));
                }
            }
        }
    }
    // Modern version JSON can omit pack_version; the client jar still has it.
    let jar_path = version_dir.join(format!("{version_id}.jar"));
    if let Ok(file) = fs::File::open(&jar_path) {
        if let Ok(mut zip) = zip::ZipArchive::new(file) {
            if let Ok(mut entry) = zip.by_name("version.json") {
                let mut text = String::new();
                if std::io::Read::read_to_string(&mut entry, &mut text).is_ok() {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(format) = pack_version_value(&json) {
                            return Ok(Some(format));
                        }
                    }
                }
            }
        }
    }
    Ok(None)
}

fn pack_version_value(json: &serde_json::Value) -> Option<u32> {
    let value = json.get("pack_version")?;
    if let Some(number) = value.as_u64() {
        return u32::try_from(number).ok().filter(|value| *value > 0);
    }
    for key in ["resource", "resource_major"] {
        if let Some(number) = value.get(key).and_then(|v| v.as_u64()) {
            if let Ok(format) = u32::try_from(number) {
                if format > 0 {
                    return Some(format);
                }
            }
        }
    }
    None
}

fn fallback_pack_format(version_id: &str) -> u32 {
    let major = version_id
        .split(['.', '-'])
        .next()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0);
    match major {
        0..=8 => 1,
        9..=10 => 2,
        11..=12 => 3,
        13..=14 => 4,
        15 => 5,
        16 => 6,
        17 => 7,
        18 => 8,
        19 => 9,
        20 => 15,
        21 => 34,
        22..=25 => 46,
        _ => 46,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "creeper-offline-skin-{name}-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn fake_png() -> Vec<u8> {
        b"\x89PNG\r\n\x1a\ncreeper".to_vec()
    }

    #[test]
    fn writes_pack_and_enables_options() {
        let root = temp_dir("root");
        let game = temp_dir("game");
        let version = root.join("versions").join("26.3");
        fs::create_dir_all(&version).unwrap();
        fs::write(
            version.join("26.3.json"),
            r#"{"id":"26.3","pack_version":{"resource_major":97}}"#,
        )
        .unwrap();
        fs::write(game.join("options.txt"), "resourcePacks:[\"vanilla\"]\n").unwrap();

        assert_eq!(apply(&game, &root, "26.3", &fake_png()).unwrap(), 97);
        let options = fs::read_to_string(game.join("options.txt")).unwrap();
        assert!(options.contains("\"file/CreeperCore Skin.zip\""));
        let file = fs::File::open(game.join("resourcepacks").join(PACK_FILE)).unwrap();
        let mut zip = zip::ZipArchive::new(file).unwrap();
        assert!(zip
            .by_name("assets/minecraft/textures/entity/player/wide/steve.png")
            .is_ok());
        assert!(zip
            .by_name("assets/minecraft/textures/entity/player/slim/alex.png")
            .is_ok());
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(game);
    }

    #[test]
    fn clears_pack_and_option() {
        let game = temp_dir("clear");
        fs::write(
            game.join("options.txt"),
            "resourcePacks:[\"vanilla\",\"file/CreeperCore Skin.zip\"]\n",
        )
        .unwrap();
        clear(&game).unwrap();
        let options = fs::read_to_string(game.join("options.txt")).unwrap();
        assert!(!options.contains("CreeperCore Skin.zip"));
        let _ = fs::remove_dir_all(game);
    }

    #[test]
    fn saves_and_deletes_account_skin() {
        let dir = temp_dir("save");
        let data_url = format!("data:image/png;base64,{}", STANDARD.encode(fake_png()));
        let path = save_local_skin(&dir, "account-1", &data_url).unwrap();
        assert!(path.is_file());
        delete_local_skin(&dir, "account-1").unwrap();
        assert!(!path.exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn reads_pack_format_from_client_jar() {
        let root = temp_dir("jar");
        let version = root.join("versions").join("26.3");
        fs::create_dir_all(&version).unwrap();
        fs::write(version.join("26.3.json"), r#"{"id":"26.3"}"#).unwrap();
        let file = fs::File::create(version.join("26.3.jar")).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file("version.json", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(br#"{"id":"26.3","pack_version":{"resource_major":97}}"#)
            .unwrap();
        zip.finish().unwrap();
        assert_eq!(resource_pack_format(&root, "26.3").unwrap(), 97);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn remembers_pack_format_from_modern_json() {
        let root = temp_dir("format");
        let version = root.join("versions").join("26.3");
        fs::create_dir_all(&version).unwrap();
        fs::write(
            version.join("26.3.json"),
            r#"{"id":"26.3","pack_version":{"resource_major":97,"data_major":121}}"#,
        )
        .unwrap();
        assert_eq!(resource_pack_format(&root, "26.3").unwrap(), 97);
        let _ = fs::remove_dir_all(root);
    }
}
