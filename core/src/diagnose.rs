//! Creeper Sense: a pre-flight health scan that turns launcher internals into
//! actionable, plain-language advice.

use crate::model::*;
use crate::mods;
use crate::system;
use crate::version::{self, LibraryKind, Platform};
use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize, Default)]
struct AssetIndexProbe {
    #[serde(default)]
    objects: HashMap<String, AssetObjectProbe>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct AssetObjectProbe {
    hash: String,
    size: u64,
}

pub struct DiagnoseInput<'a> {
    pub root: &'a Path,
    pub version_id: &'a str,
    pub game_dir: &'a Path,
    pub java: Option<&'a JavaInfo>,
    pub memory_mb: u32,
    pub account: Option<&'a AccountProfile>,
}

pub fn diagnose(input: DiagnoseInput<'_>) -> Result<DiagnosticReport> {
    let mut items = Vec::new();
    let mut score: i32 = 100;
    let mut has_missing_files = false;
    let mut java_major = None;
    let mut java_path = None;
    let mut resolved_ok = true;

    let resolved = match version::resolve_version(input.root, input.version_id) {
        Ok(resolved) => Some(resolved),
        Err(err) => {
            resolved_ok = false;
            score -= 55;
            items.push(DiagnosticItem {
                id: "core-json".to_string(),
                severity: "error".to_string(),
                title: "版本核心无法读取".to_string(),
                detail: err.to_string(),
                action: "repair".to_string(),
                fixable: false,
                points: -55,
            });
            None
        }
    };

    if let Some(resolved) = &resolved {
        let platform = Platform::current();
        let mut missing_libraries = 0usize;
        for library in &resolved.libraries {
            for file in version::library_files(library, &platform) {
                if file.kind == LibraryKind::Native || file.kind == LibraryKind::Classpath {
                    if !version::libraries_dir(input.root).join(&file.path).exists() {
                        missing_libraries += 1;
                    }
                }
            }
        }
        let client_exists = version::effective_client_jar(input.root, resolved).exists();
        if client_exists && missing_libraries == 0 {
            items.push(DiagnosticItem {
                id: "core-files".to_string(),
                severity: "ok".to_string(),
                title: "核心文件完整".to_string(),
                detail: format!(
                    "客户端核心与 {} 个运行库均已就位。",
                    resolved.libraries.len()
                ),
                action: "none".to_string(),
                fixable: false,
                points: 0,
            });
        } else {
            if !client_exists {
                has_missing_files = true;
                score -= 28;
                items.push(DiagnosticItem {
                    id: "client-jar".to_string(),
                    severity: "error".to_string(),
                    title: "缺少客户端核心".to_string(),
                    detail: "version.jar 不存在，游戏无法启动。使用「感知修复」可自动补全。"
                        .to_string(),
                    action: "repair".to_string(),
                    fixable: true,
                    points: -28,
                });
            }
            if missing_libraries > 0 {
                has_missing_files = true;
                score -= 24;
                items.push(DiagnosticItem {
                    id: "libraries".to_string(),
                    severity: "error".to_string(),
                    title: format!("缺少 {missing_libraries} 个运行库"),
                    detail: "运行库缺失通常由下载中断或杀毒软件清理造成。".to_string(),
                    action: "repair".to_string(),
                    fixable: true,
                    points: -24,
                });
            }
        }

        // Assets.
        if let Some(index_ref) = &resolved.asset_index {
            let path = crate::download::assets_index_path(input.root, &index_ref.id);
            if !path.exists() {
                has_missing_files = true;
                score -= 10;
                items.push(DiagnosticItem {
                    id: "asset-index".to_string(),
                    severity: "warn".to_string(),
                    title: "资源索引缺失".to_string(),
                    detail: "缺失资源不会阻止启动，但会丢失贴图、声音与语言。".to_string(),
                    action: "repair".to_string(),
                    fixable: true,
                    points: -10,
                });
            } else if let Ok(raw) = std::fs::read(&path) {
                if let Ok(index) = serde_json::from_slice::<AssetIndexProbe>(&raw) {
                    let mut missing = 0usize;
                    for object in index.objects.values() {
                        if object.hash.len() < 2 {
                            continue;
                        }
                        let object_path = input
                            .root
                            .join("assets")
                            .join("objects")
                            .join(&object.hash[..2])
                            .join(&object.hash);
                        let ok = std::fs::metadata(&object_path)
                            .map(|meta| meta.is_file() && meta.len() == object.size)
                            .unwrap_or(false);
                        if !ok {
                            missing += 1;
                        }
                    }
                    if missing > 0 {
                        has_missing_files = true;
                        let penalty = if missing > 20 { 12 } else { 5 };
                        score -= penalty;
                        items.push(DiagnosticItem {
                            id: "assets".to_string(),
                            severity: if missing > 20 { "warn" } else { "info" }.to_string(),
                            title: format!("{missing} 个资源对象缺失"),
                            detail: "可以启动，但部分贴图、音效或语言会显示异常。".to_string(),
                            action: "repair".to_string(),
                            fixable: true,
                            points: -penalty,
                        });
                    } else {
                        items.push(DiagnosticItem {
                            id: "assets".to_string(),
                            severity: "ok".to_string(),
                            title: "资源对象完整".to_string(),
                            detail: format!("已校验 {} 个资源对象。", index.objects.len()),
                            action: "none".to_string(),
                            fixable: false,
                            points: 0,
                        });
                    }
                }
            }
        }

        // Logging config.
        if let Some(log) = resolved
            .logging
            .as_ref()
            .and_then(|l| l.client.as_ref())
            .and_then(|c| c.file.as_ref())
        {
            let id = log
                .id
                .clone()
                .or_else(|| log.sha1.clone())
                .unwrap_or_else(|| "client-log4j.xml".to_string());
            if !input
                .root
                .join("assets")
                .join("log_configs")
                .join(id)
                .exists()
            {
                score -= 2;
                items.push(DiagnosticItem {
                    id: "logging".to_string(),
                    severity: "info".to_string(),
                    title: "日志配置缺失".to_string(),
                    detail: "不影响进入世界，崩溃分析可能缺少更详细的信息。".to_string(),
                    action: "repair".to_string(),
                    fixable: true,
                    points: -2,
                });
            }
        }
    }

    // Java.
    if let Some(java) = input.java {
        java_major = Some(java.major);
        java_path = Some(java.path.clone());
        if let Some(required) = resolved.as_ref().and_then(|v| v.java_major) {
            if java.major < required {
                score -= 25;
                items.push(DiagnosticItem {
                    id: "java".to_string(),
                    severity: "error".to_string(),
                    title: format!("Java 版本过低（{} < {}）", java.major, required),
                    detail: format!("此版本需要 Java {required} 或更高。当前：{}", java.path),
                    action: "java".to_string(),
                    fixable: false,
                    points: -25,
                });
            } else if java.major > required + 5 {
                score -= 2;
                items.push(DiagnosticItem {
                    id: "java".to_string(),
                    severity: "warn".to_string(),
                    title: format!("Java {} 高于建议版本 {}", java.major, required),
                    detail: "原版通常可运行，但部分模组会对过新的 JDK 敏感。".to_string(),
                    action: "java".to_string(),
                    fixable: false,
                    points: -2,
                });
            } else {
                items.push(DiagnosticItem {
                    id: "java".to_string(),
                    severity: "ok".to_string(),
                    title: format!("Java {} 就绪", java.major),
                    detail: format!("{} · {}", java.vendor, java.path),
                    action: "java".to_string(),
                    fixable: false,
                    points: 0,
                });
            }
        } else {
            items.push(DiagnosticItem {
                id: "java".to_string(),
                severity: "ok".to_string(),
                title: format!("Java {} 就绪", java.major),
                detail: java.path.clone(),
                action: "java".to_string(),
                fixable: false,
                points: 0,
            });
        }
        if !java.is_64bit && input.memory_mb > 1536 {
            score -= 3;
            items.push(DiagnosticItem {
                id: "java-arch".to_string(),
                severity: "warn".to_string(),
                title: "32 位 Java 无法充分利用内存".to_string(),
                detail: "建议安装 64 位 JDK，并把内存提升到 4096 MB 以上。".to_string(),
                action: "java".to_string(),
                fixable: false,
                points: -3,
            });
        }
    } else {
        score -= 25;
        items.push(DiagnosticItem {
            id: "java".to_string(),
            severity: "error".to_string(),
            title: "没有可用的 Java".to_string(),
            detail: "启动器会在常见位置自动寻找，也可以在设置中手动指定 java.exe。".to_string(),
            action: "java".to_string(),
            fixable: false,
            points: -25,
        });
    }

    // Memory.
    let total_memory = system::total_memory_mb();
    if input.memory_mb < MIN_MEMORY_MB {
        score -= 10;
        items.push(DiagnosticItem {
            id: "memory".to_string(),
            severity: "error".to_string(),
            title: "内存分配过低".to_string(),
            detail: format!("当前仅 {} MB，游戏可能需要至少 1024 MB。", input.memory_mb),
            action: "settings".to_string(),
            fixable: true,
            points: -10,
        });
    } else if total_memory > 0 && input.memory_mb as u64 > total_memory * 3 / 4 {
        score -= 6;
        items.push(DiagnosticItem {
            id: "memory".to_string(),
            severity: "warn".to_string(),
            title: "内存分配接近物理上限".to_string(),
            detail: format!(
                "已分配 {} MB，物理内存 {} MB。过高会拖慢系统并增加崩溃概率。",
                input.memory_mb, total_memory
            ),
            action: "settings".to_string(),
            fixable: true,
            points: -6,
        });
    } else {
        items.push(DiagnosticItem {
            id: "memory".to_string(),
            severity: "ok".to_string(),
            title: format!("内存 {} MB", input.memory_mb),
            detail: format!("物理内存 {} MB，分配处于安全区间。", total_memory),
            action: "settings".to_string(),
            fixable: false,
            points: 0,
        });
    }

    // Mods.
    let mods = mods::list_mods(input.game_dir).unwrap_or_default();
    if !mods.is_empty() {
        let enabled_mods: Vec<&ModInfo> = mods.iter().filter(|m| m.enabled).collect();
        let duplicates = enabled_mods.iter().filter(|m| m.duplicate).count();
        let missing_deps: Vec<&str> = enabled_mods
            .iter()
            .flat_map(|m| m.missing_dependencies.iter().map(String::as_str))
            .collect();
        if duplicates > 0 {
            score -= 9;
            items.push(DiagnosticItem {
                id: "mods".to_string(),
                severity: "error".to_string(),
                title: format!("发现 {duplicates} 个重复模组"),
                detail: "同一个 mod id 被安装了多次，加载器会拒绝启动。".to_string(),
                action: "mods".to_string(),
                fixable: false,
                points: -9,
            });
        } else if !missing_deps.is_empty() {
            score -= 7;
            let mut unique: Vec<&str> = missing_deps.clone();
            unique.sort();
            unique.dedup();
            items.push(DiagnosticItem {
                id: "mods".to_string(),
                severity: "warn".to_string(),
                title: format!("{} 个模组依赖未满足", unique.len()),
                detail: format!("缺少：{}", unique.join("、")),
                action: "mods".to_string(),
                fixable: false,
                points: -7,
            });
        } else {
            items.push(DiagnosticItem {
                id: "mods".to_string(),
                severity: "ok".to_string(),
                title: format!("{} 个模组已启用", enabled_mods.len()),
                detail: "未发现重复 ID 或缺失依赖。".to_string(),
                action: "mods".to_string(),
                fixable: false,
                points: 0,
            });
        }

        if resolved_ok {
            if let Some(resolved) = &resolved {
                let loader = crate::version::scan_versions(input.root)
                    .ok()
                    .and_then(|items| items.into_iter().find(|v| v.id == resolved.id))
                    .map(|v| v.loader)
                    .unwrap_or_else(|| "vanilla".to_string());
                let has_fabric_mod = enabled_mods.iter().any(|m| m.loader == "fabric");
                if has_fabric_mod && !matches!(loader.as_str(), "fabric" | "quilt") {
                    score -= 6;
                    items.push(DiagnosticItem {
                        id: "mod-loader".to_string(),
                        severity: "warn".to_string(),
                        title: "模组与加载器不匹配".to_string(),
                        detail: "检测到 Fabric 模组，但当前是原版或其它加载器版本。".to_string(),
                        action: "mods".to_string(),
                        fixable: false,
                        points: -6,
                    });
                }
            }
        }
    } else {
        items.push(DiagnosticItem {
            id: "mods".to_string(),
            severity: "info".to_string(),
            title: "未安装模组".to_string(),
            detail: "这是纯净的原版环境，适合稳定游玩。".to_string(),
            action: "mods".to_string(),
            fixable: false,
            points: 0,
        });
    }

    // Account.
    match input.account {
        Some(account) if !account.username.trim().is_empty() => {
            items.push(DiagnosticItem {
                id: "account".to_string(),
                severity: "info".to_string(),
                title: format!("离线档案 · {}", account.username),
                detail: "单人游戏可用；正版多人服务器需要在线登录，已在路线图中。".to_string(),
                action: "settings".to_string(),
                fixable: false,
                points: 0,
            });
        }
        _ => {
            score -= 12;
            items.push(DiagnosticItem {
                id: "account".to_string(),
                severity: "warn".to_string(),
                title: "没有选择玩家档案".to_string(),
                detail: "请先创建一个离线档案，或选择已有档案。".to_string(),
                action: "settings".to_string(),
                fixable: false,
                points: -12,
            });
        }
    }

    // Writable game directory.
    let probe = input.game_dir.join(".creeper_write_probe");
    match std::fs::create_dir_all(input.game_dir)
        .and_then(|_| std::fs::write(&probe, b"ok"))
        .and_then(|_| std::fs::remove_file(&probe))
    {
        Ok(_) => {}
        Err(err) => {
            score -= 12;
            items.push(DiagnosticItem {
                id: "writable".to_string(),
                severity: "error".to_string(),
                title: "游戏目录不可写".to_string(),
                detail: format!("{}：{err}", input.game_dir.display()),
                action: "settings".to_string(),
                fixable: false,
                points: -12,
            });
        }
    }

    score = score.clamp(0, 100);
    let (level, headline) = match score {
        90..=100 => ("ready", "引信就绪，可以点火"),
        70..=89 => ("attention", "有几处引信需要处理"),
        _ => ("danger", "先排除故障，再点燃 TNT"),
    };
    items.sort_by(|a, b| severity_rank(&b.severity).cmp(&severity_rank(&a.severity)));
    Ok(DiagnosticReport {
        version_id: input.version_id.to_string(),
        score: score as u32,
        level: level.to_string(),
        headline: headline.to_string(),
        items,
        checked_at: crate::auth::now_iso(),
        java_path,
        java_major,
        has_missing_files,
    })
}

fn severity_rank(value: &str) -> u8 {
    match value {
        "error" => 3,
        "warn" => 2,
        "info" => 1,
        _ => 0,
    }
}

pub fn quick_game_dir_state(game_dir: &Path) -> (usize, u64) {
    let mut count = 0;
    let mut bytes = 0;
    for dir in ["saves", "mods", "resourcepacks", "shaderpacks"] {
        let path = game_dir.join(dir);
        let Ok(entries) = std::fs::read_dir(path) else {
            continue;
        };
        for entry in entries.filter_map(|e| e.ok()) {
            count += 1;
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    bytes += meta.len();
                }
            }
        }
    }
    (count, bytes)
}

pub fn game_dir_label(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

pub fn path_display(path: &PathBuf) -> String {
    path.to_string_lossy().to_string()
}
