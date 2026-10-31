mod commands;
mod state;

use std::io::Write;
use tauri::Manager;

fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = format!("{info}");
        eprintln!("{message}");
        let directory = creeper_core::config::config_dir();
        let _ = std::fs::create_dir_all(&directory);
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(directory.join("panic.log"))
        {
            let _ = writeln!(
                file,
                "[{}] {message}",
                chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
            );
        }
        previous(info);
    }));
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    install_panic_hook();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let state = state::AppState::new()?;
            app.manage(state);

            // Re-apply the icon at runtime as well: on Windows, a previously
            // cached shell icon can otherwise hide an updated .exe icon until
            // Explorer's icon cache is rebuilt.
            if let Some(window) = app.get_webview_window("main") {
                if let Ok(icon) =
                    tauri::image::Image::from_bytes(include_bytes!("../icons/128x128.png"))
                {
                    let _ = window.set_icon(icon);
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_bootstrap,
            commands::select_version,
            commands::get_version_detail,
            commands::scan_java,
            commands::save_settings,
            commands::set_version_isolation,
            commands::create_account,
            commands::msa_embedded_login,
            commands::msa_refresh_account,
            commands::msa_profile,
            commands::msa_upload_skin,
            commands::msa_delete_skin,
            commands::msa_set_cape,
            commands::list_modrinth_projects,
            commands::save_local_skin,
            commands::delete_local_skin,
            commands::select_account,
            commands::delete_account,
            commands::fetch_remote_versions,
            commands::install_game,
            commands::cancel_install,
            commands::repair_game,
            commands::diagnose_game,
            commands::search_modrinth,
            commands::translate_modrinth_summaries,
            commands::search_modrinth_content,
            commands::install_modrinth_content,
            commands::list_modrinth_versions,
            commands::install_modrinth_version,
            commands::install_modrinth,
            commands::cancel_modrinth_install,
            commands::search_modrinth_modpacks,
            commands::install_modrinth_modpack,
            commands::list_mods,
            commands::toggle_mod,
            commands::delete_mod,
            commands::import_mods,
            commands::list_backups,
            commands::create_backup,
            commands::restore_backup,
            commands::delete_backup,
            commands::launch_game,
            commands::stop_game,
            commands::get_game_status,
            commands::get_game_logs,
            commands::read_latest_log,
            commands::delete_version,
            commands::list_instance_entries,
            commands::save_version_settings,
            commands::open_external,
            commands::open_path,
            commands::reveal_latest_log,
            commands::get_app_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Creeper Core");
}
