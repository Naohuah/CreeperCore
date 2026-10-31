//! Headless development / verification CLI.

use anyhow::{bail, Result};
use creeper_core::download::{self, CancelFlag, DownloadOptions, DownloadSource, ProgressFn};
use creeper_core::launch;
use creeper_core::model::*;
use creeper_core::{auth, java, mods, version};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

fn flag(args: &HashMap<String, String>, name: &str) -> Option<String> {
    args.get(name).cloned().filter(|value| !value.is_empty())
}

fn required(args: &HashMap<String, String>, name: &str) -> Result<String> {
    flag(args, name).ok_or_else(|| anyhow::anyhow!("缂哄皯鍙傛暟 --{name}"))
}

fn print_progress() -> ProgressFn {
    let last = Arc::new(Mutex::new((String::new(), 0u32)));
    Arc::new(move |progress: InstallProgress| {
        let percent = progress.percent.floor() as u32;
        let mut guard = last.lock().unwrap();
        if guard.0 != progress.phase || percent >= guard.1 + 5 || progress.percent >= 100.0 {
            eprintln!(
                "[{:>5.1}%] {:<10} {}",
                progress.percent, progress.phase, progress.message
            );
            *guard = (progress.phase.clone(), percent);
        }
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut raw = std::env::args().skip(1);
    let Some(command) = raw.next() else {
        println!(
            "creeper-cli <scan|install|repair|fabric|plan|run|backup|diagnose> --root PATH ..."
        );
        return Ok(());
    };
    let mut args = HashMap::new();
    let mut current_key: Option<String> = None;
    for arg in raw {
        if let Some(key) = current_key.take() {
            args.insert(key, arg);
        } else if let Some(key) = arg.strip_prefix("--") {
            if key == "full" {
                args.insert("full".to_string(), "true".to_string());
            } else {
                current_key = Some(key.to_string());
            }
        }
    }
    let root = PathBuf::from(flag(&args, "root").unwrap_or_else(|| {
        creeper_core::config::default_game_root()
            .to_string_lossy()
            .to_string()
    }));
    creeper_core::ensure_game_layout(&root)?;

    match command.as_str() {
        "scan" => {
            let versions = version::scan_versions(&root)?;
            println!("{}", serde_json::to_string_pretty(&versions)?);
        }
        "diagnose" => {
            let version_id = required(&args, "version")?;
            let selected_java = select_java(&root, &version_id, flag(&args, "java"))?;
            let game_dir = root.clone();
            let account = auth::make_offline_account("Miner")?;
            let report = creeper_core::diagnose::diagnose(creeper_core::diagnose::DiagnoseInput {
                root: &root,
                version_id: &version_id,
                game_dir: &game_dir,
                java: selected_java.as_ref(),
                memory_mb: 4096,
                account: Some(&account),
            })?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        "install" => {
            let version_id = required(&args, "version")?;
            let source = DownloadSource::parse(
                &flag(&args, "source").unwrap_or_else(|| "official".to_string()),
            );
            let options = DownloadOptions {
                source,
                concurrency: 16,
                include_assets: true,
            };
            let client = download::default_client()?;
            let started = Instant::now();
            let report = download::install_version(
                &client,
                &root,
                &version_id,
                &options,
                "cli-install",
                print_progress(),
                CancelFlag::new(),
            )
            .await?;
            println!(
                "installed {} 路 {} files 路 {} MB 路 {:.1}s",
                report.version_id,
                report.files_downloaded,
                report.bytes_downloaded / 1024 / 1024,
                started.elapsed().as_secs_f64()
            );
        }
        "repair" => {
            let version_id = required(&args, "version")?;
            let options = DownloadOptions {
                source: DownloadSource::parse(
                    &flag(&args, "source").unwrap_or_else(|| "official".to_string()),
                ),
                concurrency: 16,
                include_assets: true,
            };
            let client = download::default_client()?;
            let report = download::repair_version(
                &client,
                &root,
                &version_id,
                &options,
                "cli-repair",
                print_progress(),
                CancelFlag::new(),
            )
            .await?;
            println!(
                "repaired {} 路 downloaded {} 路 reused {}",
                report.version_id, report.files_downloaded, report.files_reused
            );
        }
        "fabric" => {
            let game_version = required(&args, "version")?;
            let client = download::default_client()?;
            let result = download::install_fabric(
                &client,
                &root,
                &game_version,
                &DownloadOptions::default(),
                "cli-fabric",
                print_progress(),
                CancelFlag::new(),
            )
            .await?;
            println!("installed {}", result.version_id);
        }
        "plan" => {
            let version_id = required(&args, "version")?;
            let java_info = select_java(
                &root,
                &version_id,
                flag(&args, "java").or_else(|| Some(java_path_string(&root, &version_id))),
            )?
            .ok_or_else(|| anyhow::anyhow!("鎵句笉鍒?Java"))?;
            let account = auth::make_offline_account(
                &flag(&args, "user").unwrap_or_else(|| "Miner".to_string()),
            )?;
            let settings = LaunchSettings {
                memory_mb: 4096,
                java_path: java_info.path,
                jvm_args: String::new(),
                width: 1280,
                height: 720,
                fullscreen: false,
                demo: false,
                extra_game_args: String::new(),
            };
            let plan = launch::build_launch_plan(&root, &version_id, &root, &account, &settings)?;
            println!("java: {}", plan.java_path);
            println!("gameDir: {}", plan.game_dir);
            println!("classpath entries: {}", plan.classpath_entries);
            println!("warnings: {:?}", plan.warnings);
            println!("argv[0]: {}", plan.program);
            println!("argv[1..]: {:#?}", &plan.args[..plan.args.len().min(30)]);
        }
        "run" => {
            let version_id = required(&args, "version")?;
            let java_info = select_java(&root, &version_id, flag(&args, "java"))?
                .ok_or_else(|| anyhow::anyhow!("鎵句笉鍒板彲鐢?Java"))?;
            let account = auth::make_offline_account(
                &flag(&args, "user").unwrap_or_else(|| "Miner".to_string()),
            )?;
            let memory = flag(&args, "memory")
                .and_then(|v| v.parse().ok())
                .unwrap_or(4096);
            let settings = LaunchSettings {
                memory_mb: memory,
                java_path: java_info.path,
                jvm_args: String::new(),
                width: 1280,
                height: 720,
                fullscreen: false,
                demo: false,
                extra_game_args: String::new(),
            };
            let plan = launch::build_launch_plan(&root, &version_id, &root, &account, &settings)?;
            println!("launching {} with {}", version_id, plan.java_path);
            println!("log: {}", plan.log_path);
            let mut child = tokio::process::Command::new(&plan.program)
                .args(&plan.args)
                .current_dir(&plan.game_dir)
                .stdout(std::process::Stdio::inherit())
                .stderr(std::process::Stdio::inherit())
                .spawn()?;
            let status = child.wait().await?;
            println!("exit: {:?}", status.code());
        }
        "backup" => {
            let version_id = required(&args, "version")?;
            let game_dir = flag(&args, "dir")
                .map(PathBuf::from)
                .unwrap_or_else(|| root.clone());
            let info = creeper_core::backups::create_backup(
                &root,
                &game_dir,
                &version_id,
                &flag(&args, "name").unwrap_or_else(|| "CLI 蹇収".to_string()),
                "",
                args.contains_key("full"),
            )?;
            println!("{}", serde_json::to_string_pretty(&info)?);
        }
        "mods" => {
            let game_dir = PathBuf::from(required(&args, "dir")?);
            let items = mods::list_mods(&game_dir)?;
            println!("{}", serde_json::to_string_pretty(&items)?);
        }
        other => bail!("unknown command: {other}"),
    }
    Ok(())
}

fn select_java(
    root: &PathBuf,
    version_id: &str,
    explicit: Option<String>,
) -> Result<Option<JavaInfo>> {
    let required = version::resolve_version(root, version_id)
        .ok()
        .and_then(|resolved| resolved.java_major);
    if explicit.is_none() {
        let found = java::discover_java(root, None);
        return Ok(java::select_best(&found, required));
    }
    let javas = java::discover_java(root, explicit.as_deref());
    Ok(java::select_best(&javas, required))
}

fn java_path_string(root: &PathBuf, version_id: &str) -> String {
    select_java(root, version_id, None)
        .ok()
        .flatten()
        .map(|j| j.path)
        .unwrap_or_default()
}
