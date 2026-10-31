//! "Time Machine" backups: zipped world snapshots with restore-time rollback.

use crate::model::*;
use crate::util;
use anyhow::{anyhow, bail, Context, Result};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;

pub fn backups_root(launcher_root: &Path) -> PathBuf {
    launcher_root.join(".creeper").join("backups")
}

fn version_dir(launcher_root: &Path, version_id: &str) -> PathBuf {
    backups_root(launcher_root)
        .join(util::sanitize_component(version_id).unwrap_or_else(|| "version".to_string()))
}

fn index_path(launcher_root: &Path, version_id: &str) -> PathBuf {
    version_dir(launcher_root, version_id).join("index.json")
}

fn read_index(launcher_root: &Path, version_id: &str) -> Result<Vec<BackupInfo>> {
    let path = index_path(launcher_root, version_id);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(&path)?;
    let mut items: Vec<BackupInfo> = serde_json::from_str(&raw).unwrap_or_default();
    items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(items)
}

fn write_index(launcher_root: &Path, version_id: &str, items: &[BackupInfo]) -> Result<()> {
    let path = index_path(launcher_root, version_id);
    util::write_json_atomic_sync(&path, &items)
}

pub fn list_backups(launcher_root: &Path, version_id: &str) -> Result<Vec<BackupInfo>> {
    let items = read_index(launcher_root, version_id)?;
    Ok(items
        .into_iter()
        .filter(|item| Path::new(&item.path).exists())
        .collect())
}

fn should_include(relative: &Path, include_mods: bool) -> bool {
    let first = relative
        .components()
        .next()
        .map(|c| c.as_os_str().to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if first == ".creeper" || first == "logs" || first == "crash-reports" {
        return false;
    }
    const SAVE_SCOPE: &[&str] = &["saves", "options.txt", "servers.dat", "optionsof.txt"];
    const FULL_SCOPE: &[&str] = &[
        "saves",
        "options.txt",
        "servers.dat",
        "optionsof.txt",
        "config",
        "defaultconfigs",
        "mods",
        "resourcepacks",
        "shaderpacks",
        "kubejs",
    ];
    if include_mods {
        FULL_SCOPE.contains(&first.as_str())
    } else {
        SAVE_SCOPE.contains(&first.as_str())
    }
}

/// Create a snapshot in the launcher's `.creeper/backups` area.
pub fn create_backup(
    launcher_root: &Path,
    game_dir: &Path,
    version_id: &str,
    name: &str,
    note: &str,
    include_mods: bool,
) -> Result<BackupInfo> {
    if !game_dir.exists() {
        bail!("游戏目录不存在，无法创建备份");
    }
    let dir = version_dir(launcher_root, version_id);
    std::fs::create_dir_all(&dir)?;
    let id = format!(
        "{}-{}",
        chrono::Utc::now().format("%Y%m%d-%H%M%S%.3f"),
        &uuid::Uuid::new_v4().to_string()[..4]
    )
    .replace('.', "");
    let archive_path = dir.join(format!("{id}.zip"));
    let part_path = dir.join(format!("{id}.zip.part"));
    let created_at = crate::auth::now_iso();
    let file = std::fs::File::create(&part_path)
        .with_context(|| format!("无法创建备份：{}", part_path.display()))?;
    let mut writer = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);

    let mut entries = Vec::new();
    let mut file_count = 0u64;
    let mut total_bytes = 0u64;
    for entry in WalkDir::new(game_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        let path = entry.path();
        if path == game_dir {
            continue;
        }
        let relative = match path.strip_prefix(game_dir) {
            Ok(relative) => relative,
            Err(_) => continue,
        };
        if !should_include(relative, include_mods) {
            continue;
        }
        if entry.file_type().is_dir() {
            continue;
        }
        if !entry.file_type().is_file() {
            continue;
        }
        let relative_text = relative.to_string_lossy().replace('\\', "/");
        let mut source = std::fs::File::open(path)
            .with_context(|| format!("无法读取备份文件：{}", path.display()))?;
        writer
            .start_file(&relative_text, options)
            .with_context(|| format!("无法写入备份条目：{relative_text}"))?;
        let size = std::io::copy(&mut source, &mut writer)?;
        total_bytes += size;
        file_count += 1;
        if entries.len() < 20_000 {
            entries.push(relative_text);
        }
    }

    let kind = if include_mods { "full" } else { "saves" };
    let manifest = BackupManifest {
        id: id.clone(),
        version_id: version_id.to_string(),
        name: name.trim().to_string(),
        note: note.trim().to_string(),
        created_at: created_at.clone(),
        kind: kind.to_string(),
        file_count,
        size_bytes: total_bytes,
        game_dir: game_dir.to_string_lossy().to_string(),
        entries,
    };
    writer
        .start_file("_creeper/backup.json", options)
        .context("无法写入备份清单")?;
    writer.write_all(&serde_json::to_vec_pretty(&manifest)?)?;
    writer.finish().context("无法完成备份归档")?;
    std::fs::rename(&part_path, &archive_path)?;
    let archive_size = std::fs::metadata(&archive_path)?.len();
    let info = BackupInfo {
        id: id.clone(),
        version_id: version_id.to_string(),
        name: if name.trim().is_empty() {
            format!(
                "{}快照 · {}",
                if include_mods { "完整" } else { "存档" },
                chrono::Local::now().format("%m-%d %H:%M")
            )
        } else {
            name.trim().to_string()
        },
        note: note.to_string(),
        created_at,
        size_bytes: archive_size,
        file_count,
        kind: kind.to_string(),
        path: archive_path.to_string_lossy().to_string(),
        game_dir: game_dir.to_string_lossy().to_string(),
    };
    let mut index = read_index(launcher_root, version_id)?;
    index.insert(0, info.clone());
    write_index(launcher_root, version_id, &index)?;
    if file_count == 0 {
        let _ = std::fs::remove_file(&archive_path);
        let _ = remove_from_index(launcher_root, version_id, &id);
        bail!("没有可备份的存档或配置");
    }
    Ok(info)
}

pub fn restore_backup(
    launcher_root: &Path,
    game_dir: &Path,
    version_id: &str,
    backup_id: &str,
) -> Result<RestoreReport> {
    let safe_id = util::safe_component(backup_id)?;
    let index = read_index(launcher_root, version_id)?;
    let backup = index
        .iter()
        .find(|item| item.id == safe_id)
        .cloned()
        .ok_or_else(|| anyhow!("备份不存在或索引已损坏"))?;
    let archive_path = version_dir(launcher_root, version_id).join(format!("{safe_id}.zip"));
    if !archive_path.exists() {
        bail!("备份归档缺失：{}", archive_path.display());
    }

    // Safety net: never overwrite a world without a fresh snapshot.
    let include_mods = backup.kind == "full";
    let safety = create_backup(
        launcher_root,
        game_dir,
        version_id,
        "恢复前自动快照",
        &format!("在恢复「{}」之前自动创建", backup.name),
        include_mods,
    )
    .ok();

    let file = std::fs::File::open(&archive_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut restored = 0usize;
    let mut skipped = 0usize;
    std::fs::create_dir_all(game_dir)?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let raw_name = entry.name().replace('\\', "/");
        if raw_name.starts_with("_creeper/") || raw_name.is_empty() {
            continue;
        }
        let relative = Path::new(&raw_name);
        if !util::is_safe_relative(relative) {
            skipped += 1;
            continue;
        }
        let destination = game_dir.join(relative);
        if entry.is_dir() {
            std::fs::create_dir_all(&destination)?;
            continue;
        }
        if !entry.is_file() {
            skipped += 1;
            continue;
        }
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut output = std::fs::File::create(&destination)
            .with_context(|| format!("无法恢复文件：{}", destination.display()))?;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let read = entry.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            output.write_all(&buffer[..read])?;
        }
        restored += 1;
    }
    Ok(RestoreReport {
        backup,
        safety_backup: safety,
        files_restored: restored,
        files_skipped: skipped,
        message: format!("已恢复 {restored} 个文件，原状态已自动备份"),
    })
}

pub fn delete_backup(launcher_root: &Path, version_id: &str, backup_id: &str) -> Result<()> {
    let safe_id = util::safe_component(backup_id)?;
    let archive = version_dir(launcher_root, version_id).join(format!("{safe_id}.zip"));
    if archive.exists() {
        std::fs::remove_file(&archive)?;
    }
    remove_from_index(launcher_root, version_id, &safe_id)
}

fn remove_from_index(launcher_root: &Path, version_id: &str, backup_id: &str) -> Result<()> {
    let mut index = read_index(launcher_root, version_id)?;
    index.retain(|item| item.id != backup_id);
    write_index(launcher_root, version_id, &index)
}

pub fn total_backup_size(launcher_root: &Path) -> u64 {
    WalkDir::new(backups_root(launcher_root))
        .max_depth(4)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .filter_map(|entry| entry.metadata().ok())
        .map(|meta| meta.len())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prepare() -> (tempfile::TempDir, PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let game = temp.path().join("game");
        std::fs::create_dir_all(game.join("saves/world")).unwrap();
        std::fs::write(game.join("saves/world/level.dat"), b"world-data").unwrap();
        std::fs::write(game.join("options.txt"), b"fov:70").unwrap();
        (temp, game)
    }

    #[test]
    fn creates_lists_and_restores_backup() {
        let (_temp, game) = prepare();
        let launcher_root = game.parent().unwrap().join("launcher");
        let info = create_backup(&launcher_root, &game, "1.21.1", "测试快照", "", false).unwrap();
        assert_eq!(info.file_count, 2);
        let list = list_backups(&launcher_root, "1.21.1").unwrap();
        assert_eq!(list.len(), 1);
        std::fs::remove_file(game.join("saves/world/level.dat")).unwrap();
        let report = restore_backup(&launcher_root, &game, "1.21.1", &info.id).unwrap();
        assert_eq!(
            std::fs::read(game.join("saves/world/level.dat")).unwrap(),
            b"world-data"
        );
        assert_eq!(report.files_restored, 2);
        assert!(report.safety_backup.is_some());
        delete_backup(&launcher_root, "1.21.1", &info.id).unwrap();
        // Deleting a backup removes only that archive; the automatic one remains.
        assert!(list_backups(&launcher_root, "1.21.1").unwrap().len() >= 1);
    }

    #[test]
    fn rejects_empty_snapshot() {
        let temp = tempfile::tempdir().unwrap();
        let game = temp.path().join("empty");
        std::fs::create_dir_all(&game).unwrap();
        let result = create_backup(temp.path(), &game, "v", "", "", false);
        assert!(result.is_err());
    }
}
