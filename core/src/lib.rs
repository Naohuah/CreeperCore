//! Creeper Core — an independent Minecraft launcher engine.
//!
//! The crate contains no GUI code. The Tauri shell in `src-tauri` is a thin
//! adapter over these APIs, and `creeper-cli` is a headless harness used by the
//! test suite and for integration debugging.

pub mod auth;
pub mod backups;
pub mod config;
pub mod diagnose;
pub mod download;
pub mod java;
pub mod launch;
pub mod loader_install;
pub mod model;
pub mod modrinth;
pub mod msa;
pub mod mods;
pub mod offline_skin;
pub mod system;
pub mod util;
pub mod version;

pub use model::*;

/// Ensure the standard Minecraft folder layout exists under a game root.
pub fn ensure_game_layout(root: &std::path::Path) -> anyhow::Result<()> {
    for dir in [
        "versions",
        "libraries",
        "assets",
        "assets/indexes",
        "assets/objects",
        ".creeper",
        ".creeper/logs",
        ".creeper/backups",
    ] {
        std::fs::create_dir_all(root.join(dir))?;
    }
    Ok(())
}
