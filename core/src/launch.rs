//! Launch-plan construction and crash triage.
//!
//! The launcher never shells out through `cmd`: the Java executable receives an
//! argument vector directly, which avoids Windows quoting bugs and classpath
//! injection through world or mod names.

use crate::model::*;
use crate::util;
use crate::version::{self, LibraryKind, Platform};
use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub fn build_launch_plan(
    root: &Path,
    version_id: &str,
    game_dir: &Path,
    account: &AccountProfile,
    settings: &LaunchSettings,
) -> Result<LaunchPlan> {
    let resolved = version::resolve_version(root, version_id)?;
    let platform = Platform::current();
    let mut warnings = Vec::new();

    // Java compatibility.
    let java_info = crate::java::validate_java_for_version(
        Path::new(&settings.java_path),
        resolved.java_major,
    )?;
    if let Some(required) = resolved.java_major {
        if java_info.major > required + 5 {
            warnings.push(format!(
                "Java {} 明显高于此版本所需的 Java {required}，模组可能出现兼容问题",
                java_info.major
            ));
        }
    }

    let classpath_entries = version::classpath_entries(root, &resolved, &platform);
    let mut missing = Vec::new();
    for entry in &classpath_entries {
        if !entry.exists() {
            missing.push(entry.clone());
        }
    }
    if !missing.is_empty() {
        let names = missing
            .iter()
            .take(4)
            .map(|path| {
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("、");
        bail!(
            "缺少 {} 个运行文件（{}）。请使用「感知修复」补全核心。",
            missing.len(),
            names
        );
    }
    if classpath_entries.is_empty() {
        bail!("Classpath 为空，版本文件可能损坏");
    }

    let natives_dir = version::natives_directory(root, &resolved);
    prepare_natives(root, &resolved, &platform, &natives_dir)?;

    std::fs::create_dir_all(game_dir)
        .with_context(|| format!("无法创建游戏目录：{}", game_dir.display()))?;
    for folder in ["mods", "saves", "logs", "config", "resourcepacks"] {
        let _ = std::fs::create_dir_all(game_dir.join(folder));
    }

    let classpath_separator = if cfg!(windows) { ";" } else { ":" };
    let classpath = classpath_entries
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(classpath_separator);
    let assets_root = version::assets_dir(root);
    let assets_index_name = resolved
        .asset_index
        .as_ref()
        .map(|a| a.id.clone())
        .or_else(|| resolved.assets_id.clone())
        .unwrap_or_else(|| "legacy".to_string());
    let game_assets = assets_root.join("virtual").join(&assets_index_name);
    let logging_path = resolved
        .logging
        .as_ref()
        .and_then(|l| l.client.as_ref())
        .and_then(|c| c.file.as_ref())
        .map(|file| {
            let id = file
                .id
                .clone()
                .or_else(|| file.sha1.clone())
                .unwrap_or_else(|| "client-log4j.xml".to_string());
            assets_root.join("log_configs").join(id)
        });
    if let Some(path) = &logging_path {
        if !path.exists() {
            warnings.push("日志配置缺失，游戏将使用默认日志设置".to_string());
        }
    }

    let mut features = HashMap::new();
    features.insert("is_demo_user".to_string(), settings.demo);
    features.insert(
        "has_custom_resolution".to_string(),
        settings.width > 0 && settings.height > 0,
    );
    features.insert("has_quick_plays_support".to_string(), false);
    features.insert("is_quick_play_singleplayer".to_string(), false);
    features.insert("is_quick_play_multiplayer".to_string(), false);
    features.insert("is_quick_play_realms".to_string(), false);

    let microsoft_account = account.kind.eq_ignore_ascii_case("msa");
    let access_token = if microsoft_account {
        account
            .access_token
            .clone()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow::anyhow!("Microsoft 登录已失效，请在玩家档案中重新登录"))?
    } else {
        "0".to_string()
    };

    let mut placeholders: HashMap<String, String> = HashMap::new();
    placeholders.insert("auth_player_name".to_string(), account.username.clone());
    placeholders.insert(
        "auth_uuid".to_string(),
        crate::auth::compact_uuid(&account.uuid),
    );
    placeholders.insert("auth_access_token".to_string(), access_token);
    placeholders.insert("auth_session".to_string(), "0:0:0:0".to_string());
    placeholders.insert(
        "user_type".to_string(),
        if microsoft_account { "msa" } else { "legacy" }.to_string(),
    );
    placeholders.insert("user_properties".to_string(), "{}".to_string());
    placeholders.insert("clientid".to_string(), String::new());
    placeholders.insert("auth_xuid".to_string(), String::new());
    placeholders.insert("version_name".to_string(), resolved.id.clone());
    placeholders.insert("version_type".to_string(), resolved.version_type.clone());
    placeholders.insert(
        "game_directory".to_string(),
        game_dir.to_string_lossy().to_string(),
    );
    placeholders.insert(
        "assets_root".to_string(),
        assets_root.to_string_lossy().to_string(),
    );
    placeholders.insert(
        "game_assets".to_string(),
        game_assets.to_string_lossy().to_string(),
    );
    placeholders.insert("assets_index_name".to_string(), assets_index_name);
    placeholders.insert(
        "natives_directory".to_string(),
        natives_dir.to_string_lossy().to_string(),
    );
    placeholders.insert("classpath".to_string(), classpath.clone());
    placeholders.insert(
        "classpath_separator".to_string(),
        classpath_separator.to_string(),
    );
    placeholders.insert(
        "library_directory".to_string(),
        version::libraries_dir(root).to_string_lossy().to_string(),
    );
    placeholders.insert("launcher_name".to_string(), LAUNCHER_NAME.to_string());
    placeholders.insert("launcher_version".to_string(), LAUNCHER_VERSION.to_string());
    placeholders.insert("resolution_width".to_string(), settings.width.to_string());
    placeholders.insert("resolution_height".to_string(), settings.height.to_string());
    placeholders.insert("quickPlayPath".to_string(), String::new());

    let mut game_arguments = if let Some(arguments) = &resolved.arguments {
        version::argument_values(&arguments.game, &platform, &features)
    } else {
        resolved
            .minecraft_arguments
            .as_deref()
            .map(version::split_game_args)
            .unwrap_or_default()
    };
    game_arguments = substitute_all(game_arguments, &placeholders);
    // Vanilla 26.3's version JSON omits --userType, but authlib needs it to
    // use the Microsoft profile service instead of the legacy session server.
    if !game_arguments.iter().any(|arg| arg == "--userType") {
        game_arguments.push("--userType".to_string());
        game_arguments.push(if microsoft_account { "msa" } else { "legacy" }.to_string());
    }
    if settings.width > 0
        && settings.height > 0
        && !game_arguments.iter().any(|arg| arg == "--width")
    {
        game_arguments.push("--width".to_string());
        game_arguments.push(settings.width.to_string());
        game_arguments.push("--height".to_string());
        game_arguments.push(settings.height.to_string());
    }
    if settings.fullscreen && !game_arguments.iter().any(|arg| arg == "--fullscreen") {
        game_arguments.push("--fullscreen".to_string());
    }
    game_arguments.extend(util::parse_user_args(&settings.extra_game_args));

    let mut jvm_arguments: Vec<String> = Vec::new();
    let memory = settings.memory_mb.clamp(MIN_MEMORY_MB, MAX_MEMORY_MB);
    jvm_arguments.push(format!("-Xms{memory}M"));
    jvm_arguments.push(format!("-Xmx{memory}M"));
    jvm_arguments.push("-Dfile.encoding=UTF-8".to_string());
    jvm_arguments.push("-XX:+UseG1GC".to_string());
    jvm_arguments.push("-XX:+ParallelRefProcEnabled".to_string());
    jvm_arguments.push("-XX:MaxGCPauseMillis=200".to_string());
    jvm_arguments.push(format!("-Dminecraft.launcher.brand={}", LAUNCHER_NAME));
    jvm_arguments.push(format!("-Dminecraft.launcher.version={}", LAUNCHER_VERSION));
    jvm_arguments.extend(util::parse_user_args(&settings.jvm_args));

    if let Some(logging) = &resolved.logging {
        if let Some(log) = &logging.client {
            if let (Some(path), Some(file)) = (&logging_path, &log.file) {
                if path.exists() {
                    jvm_arguments.push(log.argument.replace("${path}", &path.to_string_lossy()));
                } else if !file.url.is_empty() {
                    // It will be downloaded by the repair flow; do not block launch.
                }
            }
        }
    }

    let has_modern_jvm = resolved
        .arguments
        .as_ref()
        .map(|a| !a.jvm.is_empty())
        .unwrap_or(false);
    if has_modern_jvm {
        let jvm = version::argument_values(
            &resolved.arguments.as_ref().unwrap().jvm,
            &platform,
            &features,
        );
        jvm_arguments.extend(substitute_all(jvm, &placeholders));
    } else {
        jvm_arguments.push(format!(
            "-Djava.library.path={}",
            natives_dir.to_string_lossy()
        ));
        jvm_arguments.push("-cp".to_string());
        jvm_arguments.push(classpath);
    }
    if !jvm_arguments
        .iter()
        .any(|arg| arg.starts_with("-Djava.library.path="))
    {
        jvm_arguments.push(format!(
            "-Djava.library.path={}",
            natives_dir.to_string_lossy()
        ));
    }

    let memory_total = crate::system::total_memory_mb();
    if memory_total > 0 && memory as u64 > memory_total * 3 / 4 {
        warnings.push(format!(
            "已分配 {memory} MB 内存，超过物理内存的 75%（{memory_total} MB）"
        ));
    }

    let log_path = log_path_for(root, &resolved.id);
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut args = jvm_arguments;
    args.push(resolved.main_class.clone());
    args.extend(game_arguments);

    Ok(LaunchPlan {
        version_id: resolved.id.clone(),
        game_dir: game_dir.to_string_lossy().to_string(),
        java_path: java_info.path.clone(),
        java_major: Some(java_info.major),
        required_java: resolved.java_major,
        program: java_info.path,
        args,
        log_path: log_path.to_string_lossy().to_string(),
        native_dir: natives_dir.to_string_lossy().to_string(),
        warnings,
        classpath_entries: classpath_entries.len(),
    })
}

pub fn log_path_for(root: &Path, version_id: &str) -> PathBuf {
    let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S%.3f");
    root.join(".creeper")
        .join("logs")
        .join(util::sanitize_component(version_id).unwrap_or_else(|| "version".to_string()))
        .join(format!("{stamp}.log"))
}

fn substitute_all(args: Vec<String>, placeholders: &HashMap<String, String>) -> Vec<String> {
    args.into_iter()
        .map(|arg| substitute(&arg, placeholders))
        .collect()
}

fn substitute(value: &str, placeholders: &HashMap<String, String>) -> String {
    let mut result = value.to_string();
    for (key, replacement) in placeholders {
        result = result.replace(&format!("${{{key}}}"), replacement);
    }
    result
}

fn prepare_natives(
    root: &Path,
    resolved: &ResolvedVersion,
    platform: &Platform,
    natives_dir: &Path,
) -> Result<()> {
    std::fs::create_dir_all(natives_dir)
        .with_context(|| format!("无法创建 natives 目录：{}", natives_dir.display()))?;
    for library in &resolved.libraries {
        for file in version::library_files(library, platform) {
            if file.kind != LibraryKind::Native {
                continue;
            }
            let jar = version::libraries_dir(root).join(&file.path);
            if !jar.exists() {
                bail!("缺少本地运行库：{}", jar.display());
            }
            extract_native_jar(&jar, natives_dir, &file.exclude)
                .with_context(|| format!("无法解压 native：{}", jar.display()))?;
        }
    }
    Ok(())
}

fn extract_native_jar(jar: &Path, target: &Path, excludes: &[String]) -> Result<()> {
    let file = std::fs::File::open(jar)?;
    let mut archive = zip::ZipArchive::new(file)?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let name = entry.name().to_string();
        if entry.is_dir() || name.starts_with("META-INF/") {
            continue;
        }
        if excludes
            .iter()
            .any(|exclude| !exclude.is_empty() && name.starts_with(exclude))
        {
            continue;
        }
        let relative = Path::new(&name);
        if !util::is_safe_relative(relative) {
            continue;
        }
        let destination = target.join(relative);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut output = std::fs::File::create(&destination)?;
        std::io::copy(&mut entry, &mut output)?;
    }
    Ok(())
}

pub fn analyze_crash(
    log: &str,
    exit_code: Option<i32>,
    log_path: &str,
    play_time_secs: u64,
) -> CrashReport {
    let lower = log.to_ascii_lowercase();
    let signal_line = log
        .lines()
        .find(|line| {
            let l = line.to_ascii_lowercase();
            l.contains("exception") || l.contains("error:") || l.contains("caused by")
        })
        .map(|line| line.trim().chars().take(320).collect::<String>());

    let mut report = CrashReport {
        level: if exit_code.unwrap_or(0) == 0 {
            "info".to_string()
        } else {
            "error".to_string()
        },
        kind: "unknown".to_string(),
        exit_code,
        title: "游戏已退出".to_string(),
        summary: "进程正常结束。".to_string(),
        suggestions: Vec::new(),
        log_path: log_path.to_string(),
        signal_line,
        play_time_secs,
    };

    if exit_code.unwrap_or(0) == 0 {
        return report;
    }

    if lower.contains("outofmemoryerror") || lower.contains("java heap space") {
        report.kind = "memory".to_string();
        report.title = "内存不足，苦力怕被挤爆了".to_string();
        report.summary = "JVM 无法申请更多堆内存，游戏在加载时崩溃。".to_string();
        report.suggestions = vec![
            "降低内存分配或关闭占用较高的光影/模组".to_string(),
            "把内存调整到 4096–8192 MB 后重试".to_string(),
            "如果使用 32 位 Java，请更换为 64 位 Java".to_string(),
        ];
        return report;
    }
    if lower.contains("unsupportedclassversionerror")
        || lower.contains("only recognizes class file versions")
    {
        report.kind = "java".to_string();
        report.title = "Java 版本过旧".to_string();
        report.summary = "游戏字节码需要更高版本的 Java 才能运行。".to_string();
        report.suggestions = vec![
            "在设置中重新扫描 Java，并选择版本要求以上的 JDK".to_string(),
            "查看版本卡片上的 Java 标记（1.21+ 通常需要 Java 21）".to_string(),
        ];
        return report;
    }
    // Require a real graphics failure signature. A normal startup logs
    // "Using graphics backend OpenGL", so matching the bare word "OpenGL" would
    // misdiagnose every unrelated exit as a graphics crash.
    if lower.contains("glfw error")
        || lower.contains("failed to create window")
        || lower.contains("failed to initialize glfw")
        || lower.contains("could not initialize glfw")
        || lower.contains("pixel format not accelerated")
        || lower.contains("openglexception")
        || lower.contains("no opengl context")
        || lower.contains("could not create opengl context")
        || lower.contains("failed to create opengl context")
    {
        report.kind = "graphics".to_string();
        report.title = "图形上下文初始化失败".to_string();
        report.summary = "OpenGL / 显卡驱动没有成功创建游戏窗口。".to_string();
        report.suggestions = vec![
            "更新显卡驱动，或在笔记本上切换到独立显卡".to_string(),
            "尝试关闭光影与 OptiFine 的快速渲染设置".to_string(),
            "远程桌面环境可能不支持 OpenGL，请在本机运行".to_string(),
        ];
        return report;
    }
    if lower.contains("missing or unsupported mandatory dependencies")
        || lower.contains("incompatible mod set")
        || lower.contains("modresolutionexception")
        || lower.contains("duplicate mods")
        || lower.contains("formoderror")
    {
        report.kind = "mods".to_string();
        report.title = "模组依赖发生冲突".to_string();
        report.summary = "模组加载器在游戏启动前终止了进程。".to_string();
        report.suggestions = vec![
            "在模组页查看缺失依赖和重复模组".to_string(),
            "暂时禁用最近添加的模组，二分定位冲突".to_string(),
            "确认所有模组都兼容当前游戏与加载器版本".to_string(),
        ];
        return report;
    }
    if lower.contains("could not find or load main class")
        || lower.contains("classnotfoundexception")
        || lower.contains("no such file")
    {
        report.kind = "files".to_string();
        report.title = "核心文件不完整".to_string();
        report.summary = "启动器找不到主类或关键运行库。".to_string();
        report.suggestions = vec![
            "回到首页运行「感知修复」重新校验核心文件".to_string(),
            "确认版本没有被杀毒软件隔离".to_string(),
        ];
        return report;
    }
    if lower.contains("accessdeniedexception") || lower.contains("permission denied") {
        report.kind = "files".to_string();
        report.title = "文件访问被拒绝".to_string();
        report.summary = "游戏无法写入存档、日志或配置目录。".to_string();
        report.suggestions = vec![
            "把游戏目录移到不受保护的路径，或以管理员身份运行".to_string(),
            "检查杀毒软件的“受控文件夹访问”是否拦截了 Java".to_string(),
        ];
        return report;
    }
    if lower.contains("failure to verify access token") || lower.contains("invalid session") {
        report.kind = "auth".to_string();
        report.title = "登录会话失效".to_string();
        report.summary = "离线或用旧令牌启动时，服务器拒绝了身份验证。".to_string();
        report.suggestions = vec![
            "单人游戏不受影响，多人游戏请等待在线登录功能".to_string(),
            "删除版本级会话缓存后重试".to_string(),
        ];
        return report;
    }
    report.title = "游戏异常退出".to_string();
    report.summary = format!(
        "游戏以退出码 {} 结束。已保留完整日志，方便继续定位。",
        exit_code.unwrap_or(-1)
    );
    report.suggestions = vec![
        "打开日志，搜索第一条 Exception 或 Caused by".to_string(),
        "关闭最近安装的模组或光影后重试".to_string(),
        "使用「感知」执行一次文件完整性检查".to_string(),
    ];
    report
}

pub fn read_log_tail(path: &Path, max_lines: usize) -> Result<String> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("无法读取日志：{}", path.display()))?;
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    Ok(lines[start..].join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_memory_crash() {
        let report = analyze_crash(
            "java.lang.OutOfMemoryError: Java heap space\nat net.minecraft...",
            Some(1),
            "latest.log",
            42,
        );
        assert_eq!(report.kind, "memory");
        assert!(!report.suggestions.is_empty());
    }

    #[test]
    fn detects_java_version_crash() {
        let report = analyze_crash(
            "java.lang.UnsupportedClassVersionError: net/minecraft/client/main/Main has been compiled by a more recent version",
            Some(1),
            "latest.log",
            0,
        );
        assert_eq!(report.kind, "java");
    }

    #[test]
    fn normal_exit_is_not_a_crash() {
        let report = analyze_crash("Stopping!", Some(0), "latest.log", 600);
        assert_eq!(report.level, "info");
        assert_eq!(report.kind, "unknown");
    }

    #[test]
    fn normal_opengl_startup_is_not_a_graphics_crash() {
        let log = "\
[Render thread/INFO]: Backend library: LWJGL version 3.4.1+2
[Render thread/INFO]: Using graphics backend OpenGL, using drivers: 3.3.0 NVIDIA 621.01
[Render thread/INFO]: Using graphics device: NVIDIA GeForce RTX 3050 Ti
[Render thread/INFO]: Sound engine started";
        let report = analyze_crash(log, Some(-1), "latest.log", 36);
        assert_ne!(report.kind, "graphics");
        assert_eq!(report.kind, "unknown");
    }

    #[test]
    fn substitutes_multiple_placeholders() {
        let mut map = HashMap::new();
        map.insert("name".to_string(), "Steve".to_string());
        map.insert("dir".to_string(), "C:/game".to_string());
        assert_eq!(
            substitute("--username=${name} --dir ${dir}", &map),
            "--username=Steve --dir C:/game"
        );
    }
}
