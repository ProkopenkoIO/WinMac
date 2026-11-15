use crate::engine;
use crate::error::{message, AppError, AppResult};
use crate::models::{
    ContainerConfig, ContainerSummary, EngineBrief, Game, GameOverride, GameSource,
    UpdateContainerRequest,
};
use crate::paths::{self, ensure_dir, path_string, sanitize_folder_name};
use crate::runner;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::AppHandle;
use walkdir::WalkDir;

pub const CONFIG_FILE: &str = "container.json";
const INDEX_FILE: &str = "containers.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ContainerIndex {
    #[serde(default)]
    paths: Vec<String>,
}

fn index_file(app: &AppHandle) -> AppResult<PathBuf> {
    Ok(paths::app_support_dir(app)?.join(INDEX_FILE))
}

fn load_index(app: &AppHandle) -> AppResult<ContainerIndex> {
    let file = index_file(app)?;
    if !file.exists() {
        return Ok(ContainerIndex::default());
    }
    let raw = fs::read_to_string(&file)?;
    let mut index: ContainerIndex = serde_json::from_str(&raw).unwrap_or_default();
    index.paths.retain(|path| !path.trim().is_empty());
    index.paths.dedup();
    Ok(index)
}

fn save_index(app: &AppHandle, index: &ContainerIndex) -> AppResult<()> {
    let file = index_file(app)?;
    fs::write(&file, serde_json::to_string_pretty(index)?)?;
    Ok(())
}

pub fn register_path(app: &AppHandle, path: &Path) -> AppResult<()> {
    let mut index = load_index(app)?;
    let value = path_string(path);
    if !index.paths.iter().any(|existing| existing == &value) {
        index.paths.push(value);
        save_index(app, &index)?;
    }
    Ok(())
}

fn unregister_path(app: &AppHandle, path: &Path) -> AppResult<()> {
    let mut index = load_index(app)?;
    let value = path_string(path);
    index.paths.retain(|existing| existing != &value);
    save_index(app, &index)?;
    Ok(())
}

fn default_user_name() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "user".to_string())
}

/// Creates the minimal directory skeleton that makes a folder a valid WINEPREFIX.
/// Wine fills in the rest of `drive_c/windows` on first run.
pub fn init_prefix_layout(prefix: &Path) -> AppResult<()> {
    let drive_c = prefix.join("drive_c");
    let user_folder = format!("users/{}", default_user_name());
    for folder in [
        "Program Files",
        "Program Files (x86)",
        "Games",
        "users",
        user_folder.as_str(),
    ] {
        ensure_dir(&drive_c.join(folder))?;
    }

    #[cfg(unix)]
    {
        let dosdevices = prefix.join("dosdevices");
        ensure_dir(&dosdevices)?;
        let _ = std::os::unix::fs::symlink("../drive_c", dosdevices.join("c:"));
        let _ = std::os::unix::fs::symlink("/", dosdevices.join("z:"));
    }

    Ok(())
}

pub fn read_config(dir: &Path) -> AppResult<ContainerConfig> {
    let file = dir.join(CONFIG_FILE);
    let raw = fs::read_to_string(&file).map_err(|error| {
        AppError::Message(format!(
            "{} is not a WinMac container (missing {}): {error}",
            dir.display(),
            CONFIG_FILE
        ))
    })?;
    let mut config: ContainerConfig = serde_json::from_str(&raw)?;
    if config.path.trim().is_empty() {
        config.path = path_string(dir);
    }
    if config.id.trim().is_empty() {
        config.id = uuid::Uuid::new_v4().to_string();
        write_config(dir, &config)?;
    }
    if config.name.trim().is_empty() {
        config.name = dir
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "Container".to_string());
    }
    Ok(config)
}

pub fn write_config(dir: &Path, config: &ContainerConfig) -> AppResult<()> {
    ensure_dir(dir)?;
    fs::write(
        dir.join(CONFIG_FILE),
        serde_json::to_string_pretty(config)? + "\n",
    )?;
    Ok(())
}

pub fn create(
    app: &AppHandle,
    name: &str,
    location: Option<&str>,
    config: ContainerConfig,
) -> AppResult<ContainerSummary> {
    let name = name.trim();
    if name.is_empty() {
        return message("Container name cannot be empty");
    }

    let root = location
        .map(paths::expand_tilde)
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(paths::default_containers_dir);
    ensure_dir(&root)?;

    let folder = sanitize_folder_name(name);
    let mut dir = root.join(&folder);
    let mut counter = 2;
    while dir.join(CONFIG_FILE).exists() {
        dir = root.join(format!("{folder} ({counter})"));
        counter += 1;
    }

    fs::create_dir_all(&dir)?;
    init_prefix_layout(&dir)?;

    let mut config = config;
    config.id = uuid::Uuid::new_v4().to_string();
    config.name = name.to_string();
    config.path = path_string(&dir);
    let now = chrono::Utc::now().to_rfc3339();
    config.created_at = now.clone();
    config.updated_at = now;

    write_config(&dir, &config)?;
    register_path(app, &dir)?;

    Ok(summary(app, config))
}

pub fn list(app: &AppHandle) -> AppResult<Vec<ContainerSummary>> {
    let index = load_index(app)?;
    let mut summaries = Vec::new();
    let mut valid_paths = Vec::new();

    for raw_path in &index.paths {
        let dir = PathBuf::from(raw_path);
        match read_config(&dir) {
            Ok(config) => {
                valid_paths.push(raw_path.clone());
                summaries.push(summary(app, config));
            }
            Err(_) => {
                // Drop dangling registrations silently; folders can be re-imported.
            }
        }
    }

    if valid_paths.len() != index.paths.len() {
        let cleaned = ContainerIndex { paths: valid_paths };
        let _ = save_index(app, &cleaned);
    }

    summaries.sort_by(|a, b| {
        a.config
            .name
            .to_lowercase()
            .cmp(&b.config.name.to_lowercase())
    });
    Ok(summaries)
}

pub fn find(app: &AppHandle, id: &str) -> AppResult<ContainerConfig> {
    let index = load_index(app)?;
    for raw_path in index.paths {
        let dir = PathBuf::from(raw_path);
        if let Ok(config) = read_config(&dir) {
            if config.id == id {
                return Ok(config);
            }
        }
    }
    message(format!("Container {id} was not found"))
}

pub fn update(app: &AppHandle, request: UpdateContainerRequest) -> AppResult<ContainerSummary> {
    let mut config = find(app, &request.id)?;
    let dir = PathBuf::from(&config.path);

    if let Some(name) = request.name {
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            config.name = trimmed.to_string();
        }
    }
    if let Some(version) = request.windows_version {
        config.windows_version = version;
    }
    if let Some(backend) = request.graphics_backend {
        config.graphics_backend = backend;
    }
    config.engine_id = request.engine_id.filter(|id| !id.is_empty());
    if let Some(value) = request.esync {
        config.esync = value;
    }
    if let Some(value) = request.msync {
        config.msync = value;
    }
    if let Some(value) = request.metal_hud {
        config.metal_hud = value;
    }
    if let Some(value) = request.retina {
        config.retina = value;
    }
    if let Some(args) = request.launch_args {
        config.launch_args = args;
    }
    if let Some(env) = request.extra_env {
        config.extra_env = env
            .into_iter()
            .filter(|(key, _)| !key.trim().is_empty())
            .collect();
    }
    config.updated_at = chrono::Utc::now().to_rfc3339();

    write_config(&dir, &config)?;
    Ok(summary(app, config))
}

pub fn delete(app: &AppHandle, id: &str, delete_files: bool) -> AppResult<()> {
    let config = find(app, id)?;
    let dir = PathBuf::from(&config.path);
    unregister_path(app, &dir)?;

    if delete_files {
        // Safety: only remove folders that actually contain a container config.
        if dir.join(CONFIG_FILE).exists() {
            fs::remove_dir_all(&dir)?;
        } else if dir.exists() {
            return message(format!(
                "Refusing to delete {} because it is not a WinMac container folder",
                dir.display()
            ));
        }
    }
    Ok(())
}

pub fn clone(
    app: &AppHandle,
    id: &str,
    name: &str,
    location: Option<&str>,
) -> AppResult<ContainerSummary> {
    let source = find(app, id)?;
    let name = name.trim();
    if name.is_empty() {
        return message("Container name cannot be empty");
    }

    let source_dir = PathBuf::from(&source.path);
    if !source_dir.exists() {
        return message(format!("Container folder {} is missing", source.path));
    }

    let root = location
        .map(paths::expand_tilde)
        .or_else(|| source_dir.parent().map(Path::to_path_buf))
        .unwrap_or_else(paths::default_containers_dir);
    ensure_dir(&root)?;

    let folder = sanitize_folder_name(name);
    let mut target = root.join(&folder);
    let mut counter = 2;
    while target.exists() {
        target = root.join(format!("{folder} ({counter})"));
        counter += 1;
    }

    copy_dir_all(&source_dir, &target)?;

    let mut config = read_config(&target).unwrap_or_else(|_| source.clone());
    config.id = uuid::Uuid::new_v4().to_string();
    config.name = name.to_string();
    config.path = path_string(&target);
    config.last_played.clear();
    let now = chrono::Utc::now().to_rfc3339();
    config.created_at = now.clone();
    config.updated_at = now;
    write_config(&target, &config)?;
    register_path(app, &target)?;

    Ok(summary(app, config))
}

pub fn import(app: &AppHandle, path: &str) -> AppResult<ContainerSummary> {
    let dir = paths::expand_tilde(path);
    if !dir.is_dir() {
        return message(format!("{} is not a folder", dir.display()));
    }

    let mut config = read_config(&dir).unwrap_or_else(|_| {
        let mut config = ContainerConfig::default();
        config.name = dir
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "Imported container".to_string());
        config
    });

    config.id = if config.id.trim().is_empty() {
        uuid::Uuid::new_v4().to_string()
    } else {
        config.id.clone()
    };
    config.path = path_string(&dir);
    config.updated_at = chrono::Utc::now().to_rfc3339();
    write_config(&dir, &config)?;
    register_path(app, &dir)?;

    Ok(summary(app, config))
}

fn copy_dir_all(source: &Path, target: &Path) -> AppResult<()> {
    ensure_dir(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let to = target.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &to)?;
        } else if file_type.is_symlink() {
            #[cfg(unix)]
            {
                let link = fs::read_link(entry.path())?;
                let _ = std::os::unix::fs::symlink(link, &to);
            }
            #[cfg(not(unix))]
            {
                let _ = fs::copy(entry.path(), &to);
            }
        } else {
            fs::copy(entry.path(), &to)?;
        }
    }
    Ok(())
}

/* -------------------------------------------------------------------------- */
/* Summaries and scanning                                                     */
/* -------------------------------------------------------------------------- */

pub fn summary(app: &AppHandle, config: ContainerConfig) -> ContainerSummary {
    let dir = PathBuf::from(&config.path);
    let games = scan_games(app, &config).unwrap_or_default();

    #[cfg(target_os = "macos")]
    let size_bytes = dir_size(&dir);

    #[cfg(not(target_os = "macos"))]
    let size_bytes = crate::mock::container_size_hint(&dir).unwrap_or_else(|| dir_size(&dir));

    let resolved = engine::resolve(app, config.engine_id.as_deref());
    let engine = resolved.as_ref().map(|info| EngineBrief {
        id: info.id.clone(),
        name: info.name.clone(),
        version: info.version.clone(),
        kind: info.kind,
    });
    let engine_missing = resolved.is_none();

    ContainerSummary {
        config,
        game_count: games.len(),
        size_bytes,
        engine,
        engine_missing,
        running: runner::is_container_running(app, &dir),
        exists: dir.exists(),
    }
}

pub fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    for entry in WalkDir::new(path)
        .max_depth(12)
        .follow_links(false)
        .into_iter()
        .take(60_000)
        .flatten()
    {
        if entry.file_type().is_file() {
            if let Ok(metadata) = entry.metadata() {
                total = total.saturating_add(metadata.len());
            }
        }
    }
    total
}

fn is_junk_executable(name: &str, relative: &str) -> bool {
    let name = name.to_lowercase();
    let relative = relative.to_lowercase();
    const PREFIXES: &[&str] = &[
        "unins",
        "vcredist",
        "dxsetup",
        "dxwebsetup",
        "dotnetfx",
        "ndp4",
        "oalinst",
        "ue4prereq",
        "ue5prereq",
        "prereq",
        "python",
        "crashreport",
        "crashpad",
        "unitycrashhandler",
        "unrealcefsubprocess",
        "cefsharp",
        "notification_helper",
    ];
    const SKIP_DIRS: &[&str] = &[
        "redist",
        "_commonredist",
        "directx",
        "vcredist",
        "installers",
        "__installer",
        "support",
        "documentation",
        "manuals",
        "crashreport",
    ];

    PREFIXES.iter().any(|prefix| name.starts_with(prefix))
        || relative.split('/').any(|part| SKIP_DIRS.contains(&part))
}

fn prettify_name(stem: &str, parent: &str) -> String {
    const GENERIC: &[&str] = &[
        "game",
        "launcher",
        "start",
        "run",
        "play",
        "app",
        "bin",
        "client",
        "boot",
        "main",
        "application",
        "gameclient",
    ];

    let cleaned_stem = stem
        .replace(['_', '.', '+'], " ")
        .replace('-', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    let lowered = cleaned_stem.to_lowercase();
    let base = if GENERIC.contains(&lowered.as_str()) && !parent.is_empty() {
        parent
            .replace(['_', '.'], " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        cleaned_stem
    };

    if base.is_empty() {
        return stem.to_string();
    }

    // Title-case names that were shipped in all-lowercase or all-uppercase.
    let has_lower = base.chars().any(|character| character.is_lowercase());
    let has_upper = base.chars().any(|character| character.is_uppercase());
    if !has_upper || !has_lower {
        base.split(' ')
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    Some(first) => {
                        let mut result = first.to_uppercase().to_string();
                        result.push_str(&chars.as_str().to_lowercase());
                        result
                    }
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        base
    }
}

pub fn scan_games(_app: &AppHandle, config: &ContainerConfig) -> AppResult<Vec<Game>> {
    let prefix = PathBuf::from(&config.path);
    let drive_c = prefix.join("drive_c");
    if !drive_c.exists() {
        return Ok(Vec::new());
    }

    let roots = [
        (drive_c.join("Program Files"), GameSource::ProgramFiles),
        (
            drive_c.join("Program Files (x86)"),
            GameSource::ProgramFiles,
        ),
        (drive_c.join("Games"), GameSource::Games),
    ];

    let mut games: BTreeMap<String, Game> = BTreeMap::new();

    for (root, source) in roots {
        if !root.is_dir() {
            continue;
        }
        for entry in WalkDir::new(&root)
            .max_depth(7)
            .follow_links(false)
            .into_iter()
            .filter_entry(|entry| {
                if entry.depth() == 0 {
                    return true;
                }
                let name = entry.file_name().to_string_lossy().to_lowercase();
                !(entry.file_type().is_dir()
                    && matches!(
                        name.as_str(),
                        "redist" | "_commonredist" | "directx" | "vcredist" | "installers"
                    ))
            })
            .flatten()
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let is_exe = path
                .extension()
                .map(|ext| ext.eq_ignore_ascii_case("exe"))
                .unwrap_or(false);
            if !is_exe {
                continue;
            }

            let relative = match path.strip_prefix(&prefix) {
                Ok(value) => value.to_string_lossy().replace('\\', "/"),
                Err(_) => continue,
            };
            let file_name = path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default();
            if is_junk_executable(&file_name, &relative) {
                continue;
            }

            let metadata = match entry.metadata() {
                Ok(metadata) => metadata,
                Err(_) => continue,
            };
            let override_entry = config.game_overrides.get(&relative);
            if override_entry
                .and_then(|value| value.hidden)
                .unwrap_or(false)
            {
                continue;
            }

            let stem = path
                .file_stem()
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_else(|| file_name.clone());
            let parent = path
                .parent()
                .and_then(|value| value.file_name())
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_default();

            let name = override_entry
                .and_then(|value| value.name.clone())
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| prettify_name(&stem, &parent));
            let args = override_entry
                .and_then(|value| value.args.clone())
                .unwrap_or_default();

            #[cfg(target_os = "macos")]
            let size_bytes = metadata.len();

            #[cfg(not(target_os = "macos"))]
            let size_bytes = if metadata.len() > 1_048_576 {
                metadata.len()
            } else {
                crate::mock::game_size_hint(&file_name).unwrap_or(metadata.len())
            };

            games.insert(
                relative.clone(),
                Game {
                    id: relative.clone(),
                    name,
                    exe_path: windows_path_for(&relative),
                    args,
                    source,
                    size_bytes,
                    last_played: config.last_played.get(&relative).cloned(),
                    running: false,
                },
            );
        }
    }

    let mut result: Vec<Game> = games.into_values().collect();
    result.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(result)
}

fn windows_path_for(relative: &str) -> String {
    format!(
        "C:\\{}",
        relative.trim_start_matches("drive_c/").replace('/', "\\")
    )
}

pub fn set_game_override(
    app: &AppHandle,
    container_id: &str,
    game_id: &str,
    value: GameOverride,
) -> AppResult<()> {
    let mut config = find(app, container_id)?;
    let dir = PathBuf::from(&config.path);
    let entry = config
        .game_overrides
        .entry(game_id.to_string())
        .or_default();
    if value.name.is_some() {
        entry.name = value.name;
    }
    if value.args.is_some() {
        entry.args = value.args;
    }
    if value.hidden.is_some() {
        entry.hidden = value.hidden;
    }
    if entry.name.is_none() && entry.args.is_none() && entry.hidden.is_none() {
        config.game_overrides.remove(game_id);
    }
    config.updated_at = chrono::Utc::now().to_rfc3339();
    write_config(&dir, &config)?;
    Ok(())
}

pub fn mark_played(app: &AppHandle, container_id: &str, game_id: &str) -> AppResult<()> {
    let mut config = find(app, container_id)?;
    let dir = PathBuf::from(&config.path);
    config
        .last_played
        .insert(game_id.to_string(), chrono::Utc::now().to_rfc3339());
    config.updated_at = chrono::Utc::now().to_rfc3339();
    write_config(&dir, &config)?;
    Ok(())
}

/// Propagate prefix-level settings (Windows version, Retina) into the Wine registry.
/// Called before every launch; cheap once the prefix exists, and it also creates a
/// fresh prefix on the very first run.
#[cfg(target_os = "macos")]
pub fn apply_registry_settings(app: &AppHandle, config: &ContainerConfig) -> Vec<String> {
    use std::process::Command;

    let mut warnings = Vec::new();
    let engine = match engine::resolve(app, config.engine_id.as_deref()) {
        Some(engine) => engine,
        None => return vec!["No engine available — skipping prefix registry settings".to_string()],
    };
    let prefix = PathBuf::from(&config.path);

    let commands: Vec<(&str, Vec<&str>)> = vec![
        (
            "Windows version",
            vec![
                "reg",
                "add",
                "HKCU\\Software\\Wine",
                "/v",
                "Version",
                "/d",
                config.windows_version.registry_value(),
                "/f",
            ],
        ),
        (
            "Retina mode",
            vec![
                "reg",
                "add",
                "HKCU\\Software\\Wine\\Mac Driver",
                "/v",
                "RetinaMode",
                "/d",
                if config.retina { "y" } else { "n" },
                "/f",
            ],
        ),
    ];

    for (label, args) in commands {
        let output = Command::new(&engine.wine_path)
            .args(&args)
            .env("WINEPREFIX", &prefix)
            .env("WINEDEBUG", "-all")
            .output();
        match output {
            Ok(result) if !result.status.success() => warnings.push(format!(
                "{label} could not be applied: {}",
                String::from_utf8_lossy(&result.stderr).trim()
            )),
            Err(error) => warnings.push(format!("{label} failed: {error}")),
            _ => {}
        }
    }

    warnings
}

/* -------------------------------------------------------------------------- */
/* File manager helpers                                                       */
/* -------------------------------------------------------------------------- */

pub fn reveal_in_file_manager(path: &Path) -> AppResult<()> {
    if !path.exists() {
        return message(format!("{} does not exist", path.display()));
    }
    let is_dir = path.is_dir();

    #[cfg(target_os = "macos")]
    {
        let mut command = std::process::Command::new("open");
        if !is_dir {
            command.arg("-R");
        }
        command.arg(path);
        command.spawn()?;
    }

    #[cfg(target_os = "windows")]
    {
        let mut command = std::process::Command::new("explorer");
        if is_dir {
            command.arg(path);
        } else {
            command.arg(format!("/select,{}", path.display()));
        }
        command.spawn()?;
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open").arg(path).spawn()?;
    }

    Ok(())
}
