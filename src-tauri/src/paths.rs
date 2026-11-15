use crate::error::{AppError, AppResult};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

pub fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
}

pub fn app_data_dir(app: &AppHandle) -> AppResult<PathBuf> {
    let dir = app.path().app_data_dir().map_err(|error| {
        AppError::Message(format!(
            "Cannot resolve application data directory: {error}"
        ))
    })?;
    ensure_dir(&dir)?;
    Ok(dir)
}

pub fn app_support_dir(app: &AppHandle) -> AppResult<PathBuf> {
    let dir = app_data_dir(app)?.join("WinMacLauncher");
    ensure_dir(&dir)?;
    Ok(dir)
}

pub fn engine_dir(app: &AppHandle) -> AppResult<PathBuf> {
    let dir = app_support_dir(app)?.join("engine");
    ensure_dir(&dir)?;
    Ok(dir)
}

#[cfg(target_os = "macos")]
pub fn downloads_dir(app: &AppHandle) -> AppResult<PathBuf> {
    let dir = engine_dir(app)?.join(".downloads");
    ensure_dir(&dir)?;
    Ok(dir)
}

pub fn default_containers_dir() -> PathBuf {
    home_dir().join("Games").join("WinContainers")
}

pub fn ensure_dir(path: &Path) -> AppResult<()> {
    std::fs::create_dir_all(path)?;
    Ok(())
}

pub fn expand_tilde(input: &str) -> PathBuf {
    let trimmed = input.trim();
    if trimmed == "~" {
        return home_dir();
    }
    if let Some(rest) = trimmed.strip_prefix("~/") {
        return home_dir().join(rest);
    }
    PathBuf::from(trimmed)
}

pub fn path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

/// Keep container folder names readable across macOS / Windows / Linux file systems.
pub fn sanitize_folder_name(name: &str) -> String {
    let mut cleaned: String = name
        .chars()
        .map(|character| match character {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '-',
            character if character.is_control() => '-',
            character => character,
        })
        .collect();
    cleaned = cleaned.trim().trim_matches('.').to_string();
    if cleaned.is_empty() {
        cleaned = "Container".to_string();
    }
    if cleaned.len() > 64 {
        cleaned.truncate(64);
        cleaned = cleaned.trim_end().to_string();
    }
    cleaned
}
