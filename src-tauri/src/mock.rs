//! Windows / browser-preview mock backend.
//!
//! On non-macOS hosts there is no Wine and no `wine64` binary, so every system
//! level action (engine detection, launching, downloads) is simulated while the
//! rest of the app — containers on disk, `container.json`, EXE scanning, config
//! migration — runs for real. This keeps the UI fully debuggable on Windows.

use crate::container;
use crate::engine::EngineRuntime;
use crate::error::{message, AppResult};
use crate::models::*;
use crate::paths;
use crate::runner::{self, RunnerState};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

const DEMO_GPTK: &str = "demo-apple-gptk-2";
const DEMO_WINE: &str = "demo-wine-staging-9";

/* -------------------------------------------------------------------------- */
/* Simulated engine registry                                                  */
/* -------------------------------------------------------------------------- */

fn installed_engines() -> &'static Mutex<Vec<EngineInfo>> {
    static INSTALLED: OnceLock<Mutex<Vec<EngineInfo>>> = OnceLock::new();
    INSTALLED.get_or_init(|| Mutex::new(Vec::new()))
}

fn selected_engine() -> &'static Mutex<Option<String>> {
    static SELECTED: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    SELECTED.get_or_init(|| Mutex::new(None))
}

fn base_engines() -> Vec<EngineInfo> {
    vec![
        EngineInfo {
            id: DEMO_GPTK.into(),
            name: "Apple Game Porting Toolkit 2".into(),
            kind: EngineKind::Gptk,
            wine_path: "/Applications/Game Porting Toolkit.app/Contents/Resources/wine/bin/wine64"
                .into(),
            winetricks_path: Some("/opt/homebrew/bin/winetricks".into()),
            version: Some("wine-9.0 (gptk 2.0)".into()),
            source_path: "/Applications/Game Porting Toolkit.app".into(),
            managed: false,
            selected: true,
            available: true,
            supports: EngineKind::Gptk.supports(),
            notes: Some("Demo engine · simulated on this platform".into()),
        },
        EngineInfo {
            id: DEMO_WINE.into(),
            name: "Wine Staging 9.0".into(),
            kind: EngineKind::Wine,
            wine_path: "/opt/homebrew/bin/wine64".into(),
            winetricks_path: Some("/opt/homebrew/bin/winetricks".into()),
            version: Some("wine-9.0 (Staging)".into()),
            source_path: "/opt/homebrew/bin".into(),
            managed: false,
            selected: false,
            available: true,
            supports: EngineKind::Wine.supports(),
            notes: Some("Homebrew installation · simulated".into()),
        },
        EngineInfo {
            id: "demo-crossover-24".into(),
            name: "CrossOver 24 runtime".into(),
            kind: EngineKind::Crossover,
            wine_path: "/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine"
                .into(),
            winetricks_path: None,
            version: None,
            source_path: "/Applications/CrossOver.app".into(),
            managed: false,
            selected: false,
            available: false,
            supports: EngineKind::Crossover.supports(),
            notes: Some("CrossOver is not installed".into()),
        },
    ]
}

pub fn engines() -> Vec<EngineInfo> {
    let mut list = base_engines();
    list.extend(installed_engines().lock().unwrap().clone());

    let selected = selected_engine().lock().unwrap().clone();
    if let Some(selected) = selected {
        for engine in &mut list {
            engine.selected = engine.id == selected;
        }
    }
    if !list
        .iter()
        .any(|engine| engine.selected && engine.available)
    {
        if let Some(engine) = list.iter_mut().find(|engine| engine.available) {
            engine.selected = true;
        }
    }
    list
}

pub fn add_engine(path: &str) -> AppResult<()> {
    if path.trim().is_empty() {
        return message("Engine path is empty");
    }
    let name = Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "Custom Wine engine".to_string());
    let id = format!(
        "mock-manual-{}",
        installed_engines().lock().unwrap().len() + 1
    );
    installed_engines().lock().unwrap().push(EngineInfo {
        id: id.clone(),
        name,
        kind: EngineKind::Custom,
        wine_path: path.to_string(),
        winetricks_path: Some("/opt/homebrew/bin/winetricks".into()),
        version: Some("wine-9.0".into()),
        source_path: path.to_string(),
        managed: false,
        selected: false,
        available: true,
        supports: EngineKind::Custom.supports(),
        notes: Some("Added manually · simulated".into()),
    });
    *selected_engine().lock().unwrap() = Some(id);
    Ok(())
}

pub fn set_selected(id: &str) {
    *selected_engine().lock().unwrap() = Some(id.to_string());
}

pub fn remove(id: &str) -> AppResult<()> {
    let mut installed = installed_engines().lock().unwrap();
    let before = installed.len();
    installed.retain(|engine| engine.id != id);
    if installed.len() == before {
        return message("System engines cannot be removed in demo mode");
    }
    drop(installed);
    let mut selected = selected_engine().lock().unwrap();
    if selected.as_deref() == Some(id) {
        *selected = None;
    }
    Ok(())
}

/* -------------------------------------------------------------------------- */
/* Demo containers                                                            */
/* -------------------------------------------------------------------------- */

const DEMO_SET: &[(&str, &str, GraphicsBackend, &[(&str, &str, u64)])] = &[
    (
        "Steam Library",
        DEMO_GPTK,
        GraphicsBackend::D3DMetal,
        &[
            (
                "Cyberpunk 2077",
                "Program Files/CD Projekt Red/Cyberpunk 2077/bin/x64/Cyberpunk2077.exe",
                74_530_000_000,
            ),
            (
                "Elden Ring",
                "Games/ELDEN RING/Game/eldenring.exe",
                51_200_000_000,
            ),
            (
                "Baldur's Gate 3",
                "Games/Baldurs Gate 3/bin/bg3.exe",
                122_400_000_000,
            ),
            ("Hades II", "Games/Hades II/Hades2.exe", 9_800_000_000),
        ],
    ),
    (
        "GOG Classics",
        DEMO_WINE,
        GraphicsBackend::Dxvk,
        &[
            (
                "The Witcher 3: Wild Hunt",
                "Games/The Witcher 3/bin/x64/witcher3.exe",
                48_600_000_000,
            ),
            (
                "Heroes of Might & Magic III",
                "Games/Heroes3/Heroes3.exe",
                1_100_000_000,
            ),
        ],
    ),
    (
        "Retro DX9",
        DEMO_WINE,
        GraphicsBackend::Wined3d,
        &[
            (
                "Need for Speed: Most Wanted",
                "Games/NFS Most Wanted/nfsmw.exe",
                3_400_000_000,
            ),
            (
                "GTA: San Andreas",
                "Games/GTA San Andreas/gta_sa.exe",
                4_700_000_000,
            ),
        ],
    ),
];

/// Seed a realistic library the first time the app runs on a non-macOS host.
pub fn seed_if_needed(app: &AppHandle) {
    if cfg!(target_os = "macos") {
        return;
    }
    let support = match paths::app_support_dir(app) {
        Ok(dir) => dir,
        Err(_) => return,
    };
    ensure_demo_names();

    let marker = support.join("mock-demo-seeded.json");
    if marker.exists() {
        return;
    }

    let already_has_containers = container::list(app)
        .map(|items| !items.is_empty())
        .unwrap_or(false);

    if !already_has_containers {
        for (name, engine_id, backend, games) in DEMO_SET {
            if let Err(error) = seed_container(app, name, engine_id, *backend, games) {
                eprintln!("WinMac demo seed failed for {name}: {error}");
            }
        }
    }

    let _ = fs::write(
        &marker,
        format!("{{\"seededAt\":\"{}\"}}\n", chrono::Utc::now().to_rfc3339()),
    );
}

/// Apply the full retail titles to already seeded demo containers. The fake
/// executables keep realistic file names (`Heroes3.exe`), so the friendlier
/// title comes from the same override mechanism users get for their own games.
fn ensure_demo_names() {
    for (container_name, _, _, games) in DEMO_SET {
        let root =
            paths::default_containers_dir().join(paths::sanitize_folder_name(container_name));
        if !root.join(container::CONFIG_FILE).exists() {
            continue;
        }
        let mut config = match container::read_config(&root) {
            Ok(config) => config,
            Err(_) => continue,
        };

        let mut changed = false;
        for (title, relative, _) in *games {
            // Game ids are prefix-relative (they include `drive_c/`), so the
            // override and play-history keys have to use the same shape.
            let key = format!("drive_c/{relative}");
            if config.game_overrides.remove(*relative).is_some() {
                // Drop the prefix-less key older demo seeds wrote.
                changed = true;
            }
            if config.game_overrides.contains_key(&key) {
                continue;
            }
            config.game_overrides.insert(
                key,
                GameOverride {
                    name: Some((*title).to_string()),
                    ..Default::default()
                },
            );
            changed = true;
        }

        if changed {
            if let Err(error) = container::write_config(&root, &config) {
                eprintln!("WinMac demo name update failed for {container_name}: {error}");
            }
        }
    }
}

fn seed_container(
    app: &AppHandle,
    name: &str,
    engine_id: &str,
    backend: GraphicsBackend,
    games: &[(&str, &str, u64)],
) -> AppResult<()> {
    let folder = paths::sanitize_folder_name(name);
    let root = paths::default_containers_dir().join(&folder);
    if root.join(container::CONFIG_FILE).exists() {
        return Ok(());
    }

    fs::create_dir_all(&root)?;
    container::init_prefix_layout(&root)?;

    for (_, relative, _) in games {
        let file = root.join("drive_c").join(relative);
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent)?;
        }
        if !file.exists() {
            fs::write(
                &file,
                b"MZ\x90\x00WinMac Launcher demo executable - not a real PE file.\n",
            )?;
        }
    }

    let total: f64 = games.iter().map(|(_, _, size)| *size as f64).sum::<f64>() * 1.35;
    fs::write(
        root.join("mock-meta.json"),
        format!("{{\"sizeBytes\":{}}}\n", total as u64),
    )?;

    let mut config = ContainerConfig::default();
    config.id = format!("demo-{}", folder.to_lowercase().replace(' ', "-"));
    config.name = name.to_string();
    config.path = paths::path_string(&root);
    config.windows_version = WindowsVersion::Win10;
    config.graphics_backend = backend;
    config.esync = true;
    config.msync = backend != GraphicsBackend::Wined3d;
    config.metal_hud = false;
    config.retina = true;
    config.engine_id = Some(engine_id.to_string());
    config.created_at = chrono::Utc::now()
        .checked_sub_signed(chrono::Duration::days(14))
        .unwrap_or_else(chrono::Utc::now)
        .to_rfc3339();
    let now = chrono::Utc::now().to_rfc3339();
    config.updated_at = now.clone();
    for (index, (_, relative, _)) in games.iter().enumerate() {
        if index < 2 {
            config
                .last_played
                .insert(format!("drive_c/{relative}"), now.clone());
        }
    }

    container::write_config(&root, &config)?;
    container::register_path(app, &root)?;
    Ok(())
}

pub fn container_size_hint(dir: &Path) -> Option<u64> {
    if !dir.join("mock-meta.json").exists() {
        return None;
    }
    let raw = fs::read_to_string(dir.join("mock-meta.json")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    value.get("sizeBytes")?.as_u64()
}

pub fn game_size_hint(file_name: &str) -> Option<u64> {
    let name = file_name.to_lowercase();
    let sizes: &[(&str, u64)] = &[
        ("cyberpunk2077.exe", 74_530_000_000),
        ("eldenring.exe", 51_200_000_000),
        ("bg3.exe", 122_400_000_000),
        ("hades2.exe", 9_800_000_000),
        ("witcher3.exe", 48_600_000_000),
        ("heroes3.exe", 1_100_000_000),
        ("nfsmw.exe", 3_400_000_000),
        ("gta_sa.exe", 4_700_000_000),
    ];
    sizes
        .iter()
        .find(|(candidate, _)| *candidate == name)
        .map(|(_, size)| *size)
}

/* -------------------------------------------------------------------------- */
/* Simulated runs                                                             */
/* -------------------------------------------------------------------------- */

pub fn simulate_run(
    app: &AppHandle,
    run_id: String,
    container_id: String,
    container_path: String,
    display: String,
    kind: RunKind,
    cancel: Arc<AtomicBool>,
) -> AppResult<()> {
    let app = app.clone();
    thread::spawn(move || {
        let config = container::find(&app, &container_id).ok();
        let engine = engine_brief(&app, config.as_ref());
        let backend = config
            .as_ref()
            .map(|config| config.graphics_backend)
            .unwrap_or_default();
        let prefix = container_path.clone();
        let env_summary = config
            .as_ref()
            .map(|config| {
                format!(
                    "WINEPREFIX={prefix}  WINEESYNC={}  WINEMSYNC={}  WINEDLLOVERRIDES={}  MTL_HUD_ENABLED={}",
                    if config.esync { 1 } else { 0 },
                    if config.msync { 1 } else { 0 },
                    backend.dll_overrides().unwrap_or("-"),
                    if config.metal_hud { 1 } else { 0 }
                )
            })
            .unwrap_or_default();

        let mut lines: Vec<(u64, String, String, String)> = vec![
            (
                30,
                "system".into(),
                "system".into(),
                if kind == RunKind::Winetricks {
                    format!("$ winetricks {display}")
                } else {
                    format!("$ wine64 \"{display}\"")
                },
            ),
            (70, "system".into(), "system".into(), env_summary),
            (
                140,
                "stdout".into(),
                "info".into(),
                format!(
                    "wine: using engine {} ({})",
                    engine.0,
                    engine.1.unwrap_or_else(|| "version unknown".into())
                ),
            ),
            (
                210,
                "stderr".into(),
                "warn".into(),
                "0024:fixme:ntdll:NtQuerySystemInformation info_class SYSTEM_PROCESS_INFORMATION"
                    .into(),
            ),
            (
                270,
                "stdout".into(),
                "info".into(),
                match backend {
                    GraphicsBackend::D3DMetal => {
                        "D3DMetal: created Metal device Apple M3 Pro (40 cores), feature set Apple9"
                            .into()
                    }
                    GraphicsBackend::Dxvk => "info:  DXVK: v2.3.1 (MoltenVK 1.2.8)".into(),
                    GraphicsBackend::Wined3d => {
                        "wine: using WineD3D on top of OpenGL / Metal".into()
                    }
                },
            ),
            (
                340,
                "stdout".into(),
                "info".into(),
                format!("wine: initialized prefix {prefix}/drive_c, 64-bit"),
            ),
            (
                410,
                "stderr".into(),
                "warn".into(),
                "0024:warn:win:SetWindowTheme unsupported theme 'Explorer' (ignored)".into(),
            ),
            (
                500,
                "stdout".into(),
                "info".into(),
                "Resolver: loading game configuration and shader cache…".into(),
            ),
            (
                610,
                "stdout".into(),
                "success".into(),
                format!(
                    "Renderer ready — swapchain 2560x1440 @ 120 Hz (Metal HUD {})",
                    if config.as_ref().map(|c| c.metal_hud).unwrap_or(false) {
                        "on"
                    } else {
                        "off"
                    }
                ),
            ),
            (
                740,
                "stdout".into(),
                "info".into(),
                "Game thread started. First frame presented in 412 ms.".into(),
            ),
        ];

        if kind == RunKind::Winetricks {
            lines = vec![
                (
                    30,
                    "system".into(),
                    "system".into(),
                    format!("$ winetricks {display}"),
                ),
                (
                    90,
                    "stdout".into(),
                    "info".into(),
                    "Executing winetricks inside the prefix…".into(),
                ),
                (
                    220,
                    "stdout".into(),
                    "info".into(),
                    "Downloading dependency packages…".into(),
                ),
                (
                    420,
                    "stdout".into(),
                    "success".into(),
                    format!("{display}: installation complete"),
                ),
            ];
        }

        for (delay, stream, level, text) in lines {
            if cancel.load(Ordering::SeqCst) {
                break;
            }
            thread::sleep(Duration::from_millis(delay));
            if cancel.load(Ordering::SeqCst) {
                break;
            }
            runner::push_log(
                &app,
                LogLine {
                    run_id: run_id.clone(),
                    container_id: container_id.clone(),
                    ts: chrono::Utc::now().timestamp_millis(),
                    stream,
                    level,
                    text,
                },
            );
        }

        if !cancel.load(Ordering::SeqCst) {
            runner::push_log(
                &app,
                LogLine {
                    run_id: run_id.clone(),
                    container_id: container_id.clone(),
                    ts: chrono::Utc::now().timestamp_millis(),
                    stream: "system".into(),
                    level: "success".into(),
                    text: "Process still running — simulation continues (demo mode).".into(),
                },
            );
            for _ in 0..28 {
                if cancel.load(Ordering::SeqCst) {
                    break;
                }
                thread::sleep(Duration::from_millis(150));
            }
        }

        let cancelled = cancel.load(Ordering::SeqCst);
        if cancelled {
            runner::push_log(
                &app,
                LogLine {
                    run_id: run_id.clone(),
                    container_id: container_id.clone(),
                    ts: chrono::Utc::now().timestamp_millis(),
                    stream: "system".into(),
                    level: "warn".into(),
                    text: "Process terminated by user.".into(),
                },
            );
        } else {
            runner::push_log(
                &app,
                LogLine {
                    run_id: run_id.clone(),
                    container_id: container_id.clone(),
                    ts: chrono::Utc::now().timestamp_millis(),
                    stream: "system".into(),
                    level: "success".into(),
                    text: "Simulated process exited with code 0.".into(),
                },
            );
        }

        if let Some(state) = app.try_state::<RunnerState>() {
            state.sim.lock().unwrap().remove(&run_id);
        }

        let _ = app.emit(
            "run-exit",
            RunExit {
                run_id,
                container_id,
                code: Some(if cancelled { 130 } else { 0 }),
                simulated: true,
                kind,
            },
        );
    });
    Ok(())
}

fn engine_brief(app: &AppHandle, config: Option<&ContainerConfig>) -> (String, Option<String>) {
    let resolved =
        crate::engine::resolve(app, config.and_then(|config| config.engine_id.as_deref()));
    match resolved {
        Some(engine) => (engine.name, engine.version),
        None => ("no engine".into(), None),
    }
}

/* -------------------------------------------------------------------------- */
/* Simulated downloads                                                        */
/* -------------------------------------------------------------------------- */

pub fn download_engine(
    app: &AppHandle,
    runtime: &EngineRuntime,
    id: String,
    _url: String,
    name: String,
    _kind: EngineKind,
) -> AppResult<()> {
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut downloads = runtime.downloads.lock().unwrap();
        if downloads.contains_key(&id) {
            return message("This engine is already being downloaded");
        }
        downloads.insert(id.clone(), cancel.clone());
    }

    let app = app.clone();
    thread::spawn(move || {
        let total: u64 = 240 * 1024 * 1024;
        let mut last_emitted = 0u64;
        for step in 1..=14 {
            if cancel.load(Ordering::SeqCst) {
                let _ = app.emit(
                    "engine-progress",
                    EngineProgress {
                        id: id.clone(),
                        phase: "cancelled".into(),
                        downloaded: last_emitted,
                        total,
                        message: Some("Download cancelled".into()),
                    },
                );
                if let Some(runtime) = app.try_state::<EngineRuntime>() {
                    runtime.downloads.lock().unwrap().remove(&id);
                }
                return;
            }
            thread::sleep(Duration::from_millis(170));
            last_emitted = total * step / 14;
            let _ = app.emit(
                "engine-progress",
                EngineProgress {
                    id: id.clone(),
                    phase: if step == 14 {
                        "extracting"
                    } else {
                        "downloading"
                    }
                    .into(),
                    downloaded: last_emitted,
                    total,
                    message: Some(if step == 14 {
                        "Extracting archive…".into()
                    } else {
                        format!("Downloading {name}…")
                    }),
                },
            );
        }

        {
            let mut engines = installed_engines().lock().unwrap();
            if !engines.iter().any(|engine| engine.id == id) {
                engines.push(EngineInfo {
                    id: id.clone(),
                    name: name.clone(),
                    kind: EngineKind::Wine,
                    wine_path: format!("/Users/demo/Library/Application Support/WinMacLauncher/engine/{id}/bin/wine64"),
                    winetricks_path: Some("/opt/homebrew/bin/winetricks".into()),
                    version: Some("wine-9.0".into()),
                    source_path: format!("/Users/demo/Library/Application Support/WinMacLauncher/engine/{id}"),
                    managed: true,
                    selected: false,
                    available: true,
                    supports: EngineKind::Wine.supports(),
                    notes: Some("Downloaded engine · simulated".into()),
                });
            }
        }

        let _ = app.emit(
            "engine-progress",
            EngineProgress {
                id: id.clone(),
                phase: "done".into(),
                downloaded: total,
                total,
                message: Some(format!("{name} installed")),
            },
        );
        if let Some(runtime) = app.try_state::<EngineRuntime>() {
            runtime.downloads.lock().unwrap().remove(&id);
        }
    });

    Ok(())
}

pub async fn install_winetricks() -> AppResult<()> {
    Ok(())
}
