use crate::error::AppResult;
#[cfg(target_os = "macos")]
use crate::error::{message, AppError};
#[cfg(target_os = "macos")]
use crate::models::EngineProgress;
use crate::models::{EngineCatalogItem, EngineInfo, EngineKind, EngineStateDto};
use crate::paths;
#[cfg(target_os = "macos")]
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[cfg(target_os = "macos")]
use std::fs;
#[cfg(target_os = "macos")]
use std::hash::{Hash, Hasher};
#[cfg(target_os = "macos")]
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_os = "macos")]
use std::sync::OnceLock;
use std::sync::{Arc, Mutex};
#[cfg(target_os = "macos")]
use std::time::{Duration, Instant};
use tauri::AppHandle;
#[cfg(target_os = "macos")]
use tauri::Emitter;
#[cfg(target_os = "macos")]
use tauri::Manager;

/* -------------------------------------------------------------------------- */
/* Runtime state                                                              */
/* -------------------------------------------------------------------------- */

/// Managed by Tauri. Holds cancellation flags for engine downloads.
#[derive(Default)]
pub struct EngineRuntime {
    pub downloads: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl EngineRuntime {
    pub fn cancel(&self, id: &str) {
        if let Some(flag) = self.downloads.lock().unwrap().get(id) {
            flag.store(true, Ordering::SeqCst);
        }
    }
}

/* -------------------------------------------------------------------------- */
/* Catalog                                                                    */
/* -------------------------------------------------------------------------- */

pub fn catalog() -> Vec<EngineCatalogItem> {
    vec![
        EngineCatalogItem {
            id: "apple-gptk-2".into(),
            name: "Apple Game Porting Toolkit 2".into(),
            version: "2.0".into(),
            kind: EngineKind::Gptk,
            description:
                "Apple's D3DMetal translation layer. Best choice for DirectX 11/12 games on Apple Silicon. Requires a free Apple Developer account and macOS 14+."
                    .into(),
            url: String::new(),
            approx_size_mb: 0,
            recommended: true,
        },
        EngineCatalogItem {
            id: "gcenx-wine-latest".into(),
            name: "Wine (Gcenx build)".into(),
            version: "latest".into(),
            kind: EngineKind::Wine,
            description:
                "Community Wine build for macOS with MoltenVK, Mono, Gecko and GStreamer bundled. Downloads the newest release archive from GitHub (about 215 MB)."
                    .into(),
            url: "gh:Gcenx/macOS_Wine_builds".into(),
            approx_size_mb: 215,
            recommended: true,
        },
        EngineCatalogItem {
            id: "crossover-24".into(),
            name: "CrossOver 24 runtime".into(),
            version: "24.0".into(),
            kind: EngineKind::Crossover,
            description:
                "CodeWeavers runtime with proprietary patches. Install CrossOver normally, then use “Locate wine binary” to register its runtime."
                    .into(),
            url: String::new(),
            approx_size_mb: 0,
            recommended: false,
        },
    ]
}

/* -------------------------------------------------------------------------- */
/* Detection API                                                              */
/* -------------------------------------------------------------------------- */

#[cfg(target_os = "macos")]
static DETECT_CACHE: OnceLock<Mutex<Option<(Instant, Vec<EngineInfo>)>>> = OnceLock::new();

#[cfg(target_os = "macos")]
fn cache() -> &'static Mutex<Option<(Instant, Vec<EngineInfo>)>> {
    DETECT_CACHE.get_or_init(|| Mutex::new(None))
}

#[cfg(target_os = "macos")]
pub fn invalidate_cache() {
    *cache().lock().unwrap() = None;
}

#[cfg(target_os = "macos")]
fn cached_detect() -> Option<Vec<EngineInfo>> {
    let guard = cache().lock().unwrap();
    guard.as_ref().and_then(|(at, engines)| {
        if at.elapsed() < Duration::from_secs(20) {
            Some(engines.clone())
        } else {
            None
        }
    })
}

#[cfg(target_os = "macos")]
fn store_cache(engines: &[EngineInfo]) {
    *cache().lock().unwrap() = Some((Instant::now(), engines.to_vec()));
}

/// Resolve the engine for a container. Falls back to the default or the first
/// available engine when the configured one disappeared.
pub fn resolve(app: &AppHandle, engine_id: Option<&str>) -> Option<EngineInfo> {
    let engines = detect(app).unwrap_or_default();
    if let Some(id) = engine_id {
        if let Some(found) = engines
            .iter()
            .find(|engine| engine.id == id && engine.available)
        {
            return Some(found.clone());
        }
    }
    engines
        .iter()
        .find(|engine| engine.selected && engine.available)
        .or_else(|| engines.iter().find(|engine| engine.available))
        .cloned()
}

/* -------------------------------------------------------------------------- */
/* Registry (manual engines + default selection)                              */
/* -------------------------------------------------------------------------- */

#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct EngineRegistry {
    #[serde(default)]
    manual: Vec<ManualEngine>,
    #[serde(default)]
    default_engine: Option<String>,
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ManualEngine {
    path: String,
    #[serde(default)]
    name: Option<String>,
}

#[cfg(target_os = "macos")]
fn registry_file(app: &AppHandle) -> AppResult<PathBuf> {
    Ok(paths::app_support_dir(app)?.join("engines.json"))
}

#[cfg(target_os = "macos")]
fn load_registry(app: &AppHandle) -> EngineRegistry {
    let file = match registry_file(app) {
        Ok(file) => file,
        Err(_) => return EngineRegistry::default(),
    };
    fs::read_to_string(file)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

#[cfg(target_os = "macos")]
fn save_registry(app: &AppHandle, registry: &EngineRegistry) -> AppResult<()> {
    fs::write(
        registry_file(app)?,
        serde_json::to_string_pretty(registry)? + "\n",
    )?;
    Ok(())
}

/* -------------------------------------------------------------------------- */
/* Shared state / catalog                                                     */
/* -------------------------------------------------------------------------- */

pub fn state(app: &AppHandle) -> AppResult<EngineStateDto> {
    let engines = detect(app)?;
    let winetricks = winetricks_path(app);
    Ok(EngineStateDto {
        engines,
        catalog: catalog(),
        winetricks_installed: winetricks.is_some(),
        engine_dir: paths::engine_dir(app)
            .map(|path| paths::path_string(&path))
            .unwrap_or_default(),
    })
}

/* -------------------------------------------------------------------------- */
/* macOS implementation                                                       */
/* -------------------------------------------------------------------------- */

#[cfg(target_os = "macos")]
mod macos {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct EngineMeta {
        id: String,
        name: String,
        #[serde(default)]
        kind: Option<EngineKind>,
        #[serde(default)]
        version: Option<String>,
        #[serde(default)]
        wine_path: String,
        #[serde(default)]
        installed_at: String,
    }

    #[derive(Debug, Clone)]
    struct Candidate {
        binary: PathBuf,
        root: PathBuf,
        managed: bool,
        manual_name: Option<String>,
    }

    const SYSTEM_PATHS: &[(&str, &str)] = &[
        (
            "/Applications/Game Porting Toolkit.app/Contents/Resources/wine/bin/wine64",
            "Apple Game Porting Toolkit",
        ),
        ("/opt/homebrew/bin/wine64", "Wine (Homebrew)"),
        ("/opt/homebrew/bin/wine", "Wine (Homebrew)"),
        ("/usr/local/bin/wine64", "Wine"),
        ("/usr/local/bin/wine", "Wine"),
        (
            "/Applications/Wine Stable.app/Contents/Resources/wine/bin/wine64",
            "Wine Stable (Homebrew)",
        ),
        (
            "/Applications/Wine Staging.app/Contents/Resources/wine/bin/wine64",
            "Wine Staging (Homebrew)",
        ),
        (
            "/Applications/Wine Devel.app/Contents/Resources/wine/bin/wine64",
            "Wine Devel (Homebrew)",
        ),
        (
            "/Applications/Wine Crossover.app/Contents/Resources/wine/bin/wine64",
            "Wine Crossover (Homebrew)",
        ),
        (
            "/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine",
            "CrossOver",
        ),
        (
            "/Applications/Whisky.app/Contents/Resources/Libraries/Wine/bin/wine64",
            "Whisky Wine",
        ),
    ];

    fn stable_hash(value: &str) -> String {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        value.hash(&mut hasher);
        format!("{:08x}", hasher.finish() as u32)
    }

    pub fn detect(app: &AppHandle) -> AppResult<Vec<EngineInfo>> {
        if let Some(engines) = cached_detect() {
            return Ok(engines);
        }

        let engine_root = paths::engine_dir(app).ok();
        let registry = load_registry(app);
        let mut candidates: Vec<Candidate> = Vec::new();

        // 1. Engines downloaded into the launcher directory.
        if let Some(root) = &engine_root {
            if let Ok(entries) = fs::read_dir(root) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !path.is_dir() || entry.file_name().to_string_lossy() == ".downloads" {
                        continue;
                    }
                    if let Some(binary) = find_wine_binary(&path) {
                        candidates.push(Candidate {
                            binary,
                            root: path,
                            managed: true,
                            manual_name: None,
                        });
                    }
                }
            }
        }

        // 2. Engines registered manually through the UI.
        for manual in &registry.manual {
            let path = PathBuf::from(&manual.path);
            if let Some(binary) = resolve_binary(&path) {
                candidates.push(Candidate {
                    root: if path.is_dir() {
                        path
                    } else {
                        binary.parent().unwrap_or(Path::new("/")).to_path_buf()
                    },
                    binary,
                    managed: false,
                    manual_name: manual.name.clone(),
                });
            }
        }

        // 3. Well-known system installations.
        for (path, label) in SYSTEM_PATHS {
            let path = PathBuf::from(path);
            if path.exists() {
                if let Some(binary) = resolve_binary(&path) {
                    candidates.push(Candidate {
                        root: system_root(&path),
                        binary,
                        managed: false,
                        manual_name: Some((*label).to_string()),
                    });
                }
            }
        }

        let mut engines: Vec<EngineInfo> = Vec::new();
        let mut seen: Vec<PathBuf> = Vec::new();

        for candidate in candidates {
            let canonical = fs::canonicalize(&candidate.binary).unwrap_or(candidate.binary.clone());
            if seen.iter().any(|path| path == &canonical) {
                continue;
            }
            seen.push(canonical.clone());

            let meta = if candidate.managed {
                read_meta(&candidate.root)
            } else {
                None
            };

            let kind = meta
                .as_ref()
                .and_then(|meta| meta.kind)
                .unwrap_or_else(|| infer_kind(&candidate.root, &candidate.binary));
            let version = meta
                .as_ref()
                .and_then(|meta| meta.version.clone())
                .or_else(|| read_version(&candidate.binary));

            let id = meta
                .as_ref()
                .map(|meta| meta.id.clone())
                .filter(|id| !id.is_empty())
                .unwrap_or_else(|| {
                    if candidate.managed {
                        candidate
                            .root
                            .file_name()
                            .map(|name| name.to_string_lossy().to_string())
                            .unwrap_or_else(|| {
                                format!("wine-{}", stable_hash(&canonical.to_string_lossy()))
                            })
                    } else {
                        format!("external-{}", stable_hash(&canonical.to_string_lossy()))
                    }
                });

            let name = candidate
                .manual_name
                .clone()
                .or_else(|| meta.as_ref().map(|meta| meta.name.clone()))
                .filter(|name| !name.trim().is_empty())
                .unwrap_or_else(|| infer_name(&candidate.root, kind));

            engines.push(EngineInfo {
                id,
                name,
                kind,
                wine_path: paths::path_string(&canonical),
                winetricks_path: find_winetricks_near(&candidate.root)
                    .map(|path| paths::path_string(&path)),
                version,
                source_path: paths::path_string(&candidate.root),
                managed: candidate.managed,
                selected: false,
                available: true,
                supports: kind.supports(),
                notes: Some(if candidate.managed {
                    "Installed by WinMac Launcher".to_string()
                } else {
                    "Detected on this Mac".to_string()
                }),
            });
        }

        if let Some(default_id) = &registry.default_engine {
            if let Some(engine) = engines.iter_mut().find(|engine| &engine.id == default_id) {
                engine.selected = true;
            }
        }
        if !engines.iter().any(|engine| engine.selected) {
            if let Some(engine) = engines.iter_mut().find(|engine| engine.available) {
                engine.selected = true;
            }
        }

        engines.sort_by(|a, b| {
            b.managed
                .cmp(&a.managed)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });

        store_cache(&engines);
        Ok(engines)
    }

    fn system_root(path: &Path) -> PathBuf {
        let text = paths::path_string(path);
        if let Some(index) = text.find(".app") {
            return PathBuf::from(&text[..index + 4]);
        }
        path.parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| path.to_path_buf())
    }

    fn read_meta(root: &Path) -> Option<EngineMeta> {
        let raw = fs::read_to_string(root.join("engine.json")).ok()?;
        serde_json::from_str(&raw).ok()
    }

    pub fn write_meta(
        root: &Path,
        id: &str,
        name: &str,
        kind: EngineKind,
        wine_path: &Path,
        version: Option<String>,
    ) -> AppResult<()> {
        let meta = EngineMeta {
            id: id.to_string(),
            name: name.to_string(),
            kind: Some(kind),
            version,
            wine_path: paths::path_string(wine_path),
            installed_at: chrono::Utc::now().to_rfc3339(),
        };
        fs::write(
            root.join("engine.json"),
            serde_json::to_string_pretty(&meta)? + "\n",
        )?;
        Ok(())
    }

    fn infer_kind(root: &Path, binary: &Path) -> EngineKind {
        let text = format!(
            "{} {}",
            paths::path_string(root).to_lowercase(),
            paths::path_string(binary).to_lowercase()
        );
        if text.contains("game porting toolkit") || text.contains("gptk") {
            EngineKind::Gptk
        } else if text.contains("crossover") {
            EngineKind::Crossover
        } else if text.contains("whisky") {
            EngineKind::Wine
        } else {
            EngineKind::Wine
        }
    }

    fn infer_name(root: &Path, kind: EngineKind) -> String {
        let text = paths::path_string(root).to_lowercase();
        if text.contains("game porting toolkit") {
            return "Apple Game Porting Toolkit".to_string();
        }
        if text.contains("crossover") {
            return "CrossOver runtime".to_string();
        }
        if text.contains("whisky") {
            return "Whisky Wine".to_string();
        }
        match kind {
            EngineKind::Gptk => "Game Porting Toolkit".to_string(),
            EngineKind::Crossover => "CrossOver".to_string(),
            _ => root
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| "Wine".to_string()),
        }
    }

    pub fn resolve_binary(path: &Path) -> Option<PathBuf> {
        if path.is_file() {
            return if is_wine_like(path) && !is_app_launcher(path) {
                Some(path.to_path_buf())
            } else {
                None
            };
        }
        find_wine_binary(path)
    }

    fn is_wine_like(path: &Path) -> bool {
        path.file_name()
            .and_then(|name| name.to_str())
            .map(|name| name == "wine" || name == "wine64")
            .unwrap_or(false)
    }

    /// `Something.app/Contents/MacOS/wine` is the tiny GUI launcher that ships
    /// inside macOS Wine bundles. Running it (even with `--version`) opens a
    /// Wine window instead of printing anything, so it must never be picked as
    /// the engine binary — the command line loader lives in `.../wine/bin/wine`.
    pub fn is_app_launcher(path: &Path) -> bool {
        let text = paths::path_string(path).to_lowercase();
        text.contains("/contents/macos/")
    }

    pub fn find_wine_binary(root: &Path) -> Option<PathBuf> {
        const RELATIVE: &[&str] = &[
            "Contents/Resources/wine/bin/wine64",
            "Contents/Resources/wine/bin/wine",
            "bin/wine64",
            "bin/wine",
            "Contents/SharedSupport/CrossOver/bin/wine",
            "Libraries/Wine/bin/wine64",
            "wine/bin/wine64",
            "wine/bin/wine",
            "wine64",
            "wine",
        ];
        for relative in RELATIVE {
            let candidate = root.join(relative);
            if candidate.is_file() && !is_app_launcher(&candidate) {
                return Some(candidate);
            }
        }
        if root.is_file() && is_wine_like(root) && !is_app_launcher(root) {
            return Some(root.to_path_buf());
        }

        // Archives (Gcenx, Whisky, CrossOver) nest the payload a few folders
        // deep, so rank every candidate instead of trusting traversal order.
        let mut best: Option<(u32, usize, PathBuf)> = None;
        for entry in walkdir::WalkDir::new(root)
            .max_depth(8)
            .follow_links(false)
            .into_iter()
            .flatten()
        {
            if !entry.file_type().is_file() || !is_wine_like(entry.path()) {
                continue;
            }
            let path = entry.path();
            if is_app_launcher(path) {
                continue;
            }
            let lower = paths::path_string(path).to_lowercase();
            let parent_name = path
                .parent()
                .and_then(|parent| parent.file_name())
                .map(|name| name.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            let mut score = 0u32;
            if lower.contains("/resources/wine/bin/") {
                score += 100;
            }
            if parent_name == "bin" {
                score += 50;
            }
            if path.file_name().and_then(|name| name.to_str()) == Some("wine64") {
                score += 10;
            }
            if lower.contains("/crossover/") {
                score += 5;
            }
            let depth = entry.depth();
            let better = match &best {
                None => true,
                Some((best_score, best_depth, _)) => {
                    score > *best_score || (score == *best_score && depth < *best_depth)
                }
            };
            if better {
                best = Some((score, depth, path.to_path_buf()));
            }
        }
        best.map(|(_, _, path)| path)
    }

    pub fn read_version(binary: &Path) -> Option<String> {
        use std::io::Read;
        use std::process::{Command, Stdio};
        use std::sync::mpsc;

        // Never execute the `.app` GUI stub: it opens windows and never exits.
        if is_app_launcher(binary) {
            return None;
        }

        let mut child = Command::new(binary)
            .arg("--version")
            .env("WINEDEBUG", "-all")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;

        // Read on a separate thread and with a receive timeout: a stray child
        // holding the pipe open must not block engine detection either.
        let mut stdout = child.stdout.take()?;
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let mut text = String::new();
            let _ = stdout.read_to_string(&mut text);
            let _ = sender.send(text);
        });

        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(50));
                }
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                Err(_) => {
                    let _ = child.kill();
                    return None;
                }
            }
        }

        let text = receiver.recv_timeout(Duration::from_secs(5)).ok()?;
        let version = text.lines().next()?.trim().to_string();
        if version.is_empty() {
            None
        } else {
            Some(version)
        }
    }

    pub fn find_winetricks_near(root: &Path) -> Option<PathBuf> {
        for relative in [
            "winetricks",
            "bin/winetricks",
            "Contents/Resources/winetricks",
        ] {
            let candidate = root.join(relative);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        None
    }

    pub async fn install_winetricks(app: &AppHandle) -> AppResult<PathBuf> {
        let target = paths::engine_dir(app)?.join("winetricks");
        let client = reqwest::Client::builder()
            .user_agent("WinMac-Launcher/0.1")
            .build()?;
        let url = "https://raw.githubusercontent.com/Winetricks/winetricks/master/src/winetricks";
        let response = client.get(url).send().await?.error_for_status()?;
        let bytes = response.bytes().await?;
        if bytes.is_empty() {
            return message("winetricks download returned an empty file");
        }
        fs::write(&target, &bytes)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(&target)?.permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&target, permissions)?;
        }
        invalidate_cache();
        Ok(target)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::time::{SystemTime, UNIX_EPOCH};

        fn scratch(name: &str) -> PathBuf {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let dir = std::env::temp_dir().join(format!("winmac-engine-{name}-{stamp}"));
            fs::create_dir_all(&dir).unwrap();
            dir
        }

        fn touch(path: &Path) {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"#!/bin/sh\n").unwrap();
        }

        #[test]
        fn prefers_the_loader_over_the_app_stub() {
            // Mirror the Gcenx layout: the GUI stub is found first alphabetically.
            let root = scratch("gcenx");
            let app = root.join("Wine Staging.app");
            let loader = app.join("Contents/Resources/wine/bin/wine");
            let stub = app.join("Contents/MacOS/wine");
            touch(&loader);
            touch(&stub);
            touch(&app.join("Contents/Resources/wine/bin/wineserver"));

            assert!(is_app_launcher(&stub));
            assert!(!is_app_launcher(&loader));
            assert_eq!(find_wine_binary(&root).unwrap(), loader);
            assert!(resolve_binary(&stub).is_none());
            assert!(read_version(&stub).is_none());

            let _ = fs::remove_dir_all(&root);
        }

        #[test]
        fn finds_a_plain_bin_layout() {
            let root = scratch("plain");
            let binary = root.join("bin/wine64");
            touch(&binary);

            assert_eq!(find_wine_binary(&root).unwrap(), binary);

            let _ = fs::remove_dir_all(&root);
        }

        #[test]
        fn ignores_a_folder_without_wine() {
            let root = scratch("empty");
            touch(&root.join("bin/wineserver"));

            assert!(find_wine_binary(&root).is_none());

            let _ = fs::remove_dir_all(&root);
        }
    }
}

/* -------------------------------------------------------------------------- */
/* Public platform API                                                        */
/* -------------------------------------------------------------------------- */

#[cfg(target_os = "macos")]
pub fn detect(app: &AppHandle) -> AppResult<Vec<EngineInfo>> {
    macos::detect(app)
}

#[cfg(not(target_os = "macos"))]
pub fn detect(_app: &AppHandle) -> AppResult<Vec<EngineInfo>> {
    Ok(crate::mock::engines())
}

#[cfg(target_os = "macos")]
pub fn winetricks_path(app: &AppHandle) -> Option<PathBuf> {
    if let Ok(dir) = paths::engine_dir(app) {
        let bundled = dir.join("winetricks");
        if bundled.is_file() {
            return Some(bundled);
        }
    }
    for candidate in [
        "/opt/homebrew/bin/winetricks",
        "/usr/local/bin/winetricks",
        "/usr/bin/winetricks",
    ] {
        let path = PathBuf::from(candidate);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

#[cfg(not(target_os = "macos"))]
pub fn winetricks_path(_app: &AppHandle) -> Option<PathBuf> {
    Some(PathBuf::from("C:\\mock\\winetricks.exe"))
}

#[cfg(target_os = "macos")]
pub fn add_manual(app: &AppHandle, path: &str) -> AppResult<EngineStateDto> {
    let path = paths::expand_tilde(path);
    let binary = macos::resolve_binary(&path).ok_or_else(|| {
        if macos::is_app_launcher(&path) {
            AppError::Message(format!(
                "{} is the .app launcher, not the Wine loader. Choose the binary inside Contents/Resources/wine/bin instead.",
                path.display()
            ))
        } else {
            AppError::Message(format!(
                "{} is not a Wine binary or engine folder",
                path.display()
            ))
        }
    })?;
    let binary = fs::canonicalize(&binary).unwrap_or(binary);

    let mut registry = load_registry(app);
    let value = paths::path_string(&binary);
    if !registry.manual.iter().any(|manual| manual.path == value) {
        let name = binary
            .parent()
            .and_then(|parent| parent.parent())
            .and_then(|root| root.file_name())
            .map(|name| name.to_string_lossy().to_string())
            .or_else(|| {
                binary
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
            });
        registry.manual.push(ManualEngine { path: value, name });
        save_registry(app, &registry)?;
    }
    invalidate_cache();
    state(app)
}

#[cfg(not(target_os = "macos"))]
pub fn add_manual(app: &AppHandle, path: &str) -> AppResult<EngineStateDto> {
    crate::mock::add_engine(path)?;
    state(app)
}

#[cfg(target_os = "macos")]
pub fn set_selected(app: &AppHandle, id: &str) -> AppResult<EngineStateDto> {
    let mut registry = load_registry(app);
    registry.default_engine = Some(id.to_string());
    save_registry(app, &registry)?;
    invalidate_cache();
    state(app)
}

#[cfg(not(target_os = "macos"))]
pub fn set_selected(app: &AppHandle, id: &str) -> AppResult<EngineStateDto> {
    crate::mock::set_selected(id);
    state(app)
}

#[cfg(target_os = "macos")]
pub fn remove(app: &AppHandle, id: &str) -> AppResult<EngineStateDto> {
    let engines = detect(app)?;
    let engine = engines
        .iter()
        .find(|engine| engine.id == id)
        .ok_or_else(|| AppError::Message(format!("Engine {id} was not found")))?;

    if engine.managed {
        let dir = PathBuf::from(&engine.source_path);
        if dir.starts_with(paths::engine_dir(app)?) && dir.is_dir() {
            fs::remove_dir_all(dir)?;
        } else {
            return message("Refusing to remove an engine outside the launcher engine folder");
        }
    } else {
        let mut registry = load_registry(app);
        let before = registry.manual.len();
        let binary = engine.wine_path.clone();
        registry.manual.retain(|manual| manual.path != binary);
        if registry.manual.len() == before {
            return message(
                "System engines cannot be removed — uninstall them with their own installer",
            );
        }
        if registry.default_engine.as_deref() == Some(id) {
            registry.default_engine = None;
        }
        save_registry(app, &registry)?;
    }

    invalidate_cache();
    state(app)
}

#[cfg(not(target_os = "macos"))]
pub fn remove(app: &AppHandle, id: &str) -> AppResult<EngineStateDto> {
    crate::mock::remove(id)?;
    state(app)
}

/* -------------------------------------------------------------------------- */
/* Downloader                                                                 */
/* -------------------------------------------------------------------------- */

#[cfg(target_os = "macos")]
pub fn download(
    app: &AppHandle,
    runtime: &EngineRuntime,
    id: String,
    url: String,
    name: String,
    kind: EngineKind,
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
    let engine_dir = paths::engine_dir(&app)?;
    let downloads_dir = paths::downloads_dir(&app)?;

    tauri::async_runtime::spawn(async move {
        let result = download_task(
            &app,
            &id,
            &url,
            &name,
            kind,
            &cancel,
            &engine_dir,
            &downloads_dir,
        )
        .await;
        if let Err(error) = result {
            emit_progress(
                &app,
                &EngineProgress {
                    id: id.clone(),
                    phase: "error".into(),
                    downloaded: 0,
                    total: 0,
                    message: Some(error.to_string()),
                },
            );
        }
        app.state::<EngineRuntime>()
            .downloads
            .lock()
            .unwrap()
            .remove(&id);
        invalidate_cache();
    });

    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn download(
    app: &AppHandle,
    runtime: &EngineRuntime,
    id: String,
    url: String,
    name: String,
    kind: EngineKind,
) -> AppResult<()> {
    crate::mock::download_engine(app, runtime, id, url, name, kind)
}

#[cfg(target_os = "macos")]
fn emit_progress(app: &AppHandle, progress: &EngineProgress) {
    let _ = app.emit("engine-progress", progress);
}

#[cfg(target_os = "macos")]
async fn resolve_download_url(client: &reqwest::Client, spec: &str) -> AppResult<String> {
    let repo = if let Some(repo) = spec.strip_prefix("gh:") {
        repo
    } else if let Some(repo) = spec.strip_prefix("github:") {
        repo
    } else {
        return Ok(spec.to_string());
    };

    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let response = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?
        .error_for_status()?;
    let json: serde_json::Value = response.json().await?;
    let assets = json
        .get("assets")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();

    let archive = assets
        .iter()
        .filter_map(|asset| {
            let name = asset.get("name")?.as_str()?;
            let url = asset.get("browser_download_url")?.as_str()?;
            if name.ends_with(".tar.gz")
                || name.ends_with(".tgz")
                || name.ends_with(".tar.xz")
                || name.ends_with(".txz")
                || name.ends_with(".zip")
            {
                Some((name.to_string(), url.to_string()))
            } else {
                None
            }
        })
        .find(|(name, _)| name.contains("wine") || name.contains("Wine"))
        .or_else(|| {
            assets.iter().find_map(|asset| {
                let name = asset.get("name")?.as_str()?;
                let url = asset.get("browser_download_url")?.as_str()?;
                if name.ends_with(".tar.gz") || name.ends_with(".tar.xz") || name.ends_with(".zip")
                {
                    Some((name.to_string(), url.to_string()))
                } else {
                    None
                }
            })
        });

    match archive {
        Some((_, url)) => Ok(url),
        None => message(format!(
            "No downloadable archive found in the latest release of {repo}"
        )),
    }
}

#[cfg(target_os = "macos")]
async fn download_task(
    app: &AppHandle,
    id: &str,
    url: &str,
    name: &str,
    kind: EngineKind,
    cancel: &Arc<AtomicBool>,
    engine_dir: &Path,
    downloads_dir: &Path,
) -> AppResult<()> {
    use futures_util::StreamExt;
    use std::io::Write;

    let target_dir = engine_dir.join(id);
    if target_dir.join("engine.json").exists() {
        return message(format!("Engine “{name}” is already installed"));
    }

    let client = reqwest::Client::builder()
        .user_agent("WinMac-Launcher/0.1")
        .build()?;

    emit_progress(
        app,
        &EngineProgress {
            id: id.to_string(),
            phase: "downloading".into(),
            downloaded: 0,
            total: 0,
            message: Some("Resolving download…".into()),
        },
    );

    let resolved = resolve_download_url(&client, url).await?;
    let response = client.get(&resolved).send().await?.error_for_status()?;
    let total = response.content_length().unwrap_or(0);
    let lower = resolved
        .split('?')
        .next()
        .map(|value| value.to_lowercase())
        .unwrap_or_default();
    let extension = if lower.ends_with(".zip") {
        "zip"
    } else if lower.ends_with(".tar.xz") {
        "tar.xz"
    } else if lower.ends_with(".txz") {
        "txz"
    } else {
        "tar.gz"
    };
    let archive_path = downloads_dir.join(format!("{id}.{extension}"));

    let mut file = fs::File::create(&archive_path)?;
    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut last_emit = Instant::now();

    while let Some(chunk) = stream.next().await {
        if cancel.load(Ordering::SeqCst) {
            drop(file);
            let _ = fs::remove_file(&archive_path);
            emit_progress(
                app,
                &EngineProgress {
                    id: id.to_string(),
                    phase: "cancelled".into(),
                    downloaded,
                    total,
                    message: Some("Download cancelled".into()),
                },
            );
            return Ok(());
        }
        let chunk = chunk.map_err(|error| AppError::Http(error.to_string()))?;
        file.write_all(&chunk)?;
        downloaded += chunk.len() as u64;
        if last_emit.elapsed() > Duration::from_millis(280) {
            emit_progress(
                app,
                &EngineProgress {
                    id: id.to_string(),
                    phase: "downloading".into(),
                    downloaded,
                    total,
                    message: Some(format!("Downloading {}…", name)),
                },
            );
            last_emit = Instant::now();
        }
    }
    file.flush()?;
    drop(file);

    emit_progress(
        app,
        &EngineProgress {
            id: id.to_string(),
            phase: "extracting".into(),
            downloaded,
            total,
            message: Some("Extracting archive…".into()),
        },
    );

    let archive = archive_path.clone();
    let destination = target_dir.clone();
    let destination_for_task = destination.clone();
    tauri::async_runtime::spawn_blocking(move || extract_archive(&archive, &destination_for_task))
        .await
        .map_err(|error| AppError::Message(error.to_string()))??;

    let binary = match macos::find_wine_binary(&destination) {
        Some(binary) => binary,
        None => {
            let _ = fs::remove_dir_all(&destination);
            return message(
                "The archive did not contain a Wine command line binary (only an .app launcher?)",
            );
        }
    };
    make_executable(&destination);
    let version = match macos::read_version(&binary) {
        Some(version) => version,
        None => {
            let _ = fs::remove_dir_all(&destination);
            return message(format!(
                "The downloaded Wine build did not respond to `wine --version` ({}). The install was discarded.",
                paths::path_string(&binary)
            ));
        }
    };
    macos::write_meta(&destination, id, name, kind, &binary, Some(version))?;
    let _ = fs::remove_file(&archive_path);

    emit_progress(
        app,
        &EngineProgress {
            id: id.to_string(),
            phase: "done".into(),
            downloaded,
            total,
            message: Some(format!("{name} installed")),
        },
    );
    invalidate_cache();
    Ok(())
}

#[cfg(target_os = "macos")]
fn extract_archive(archive: &Path, destination: &Path) -> AppResult<()> {
    if destination.exists() {
        let _ = fs::remove_dir_all(destination);
    }
    paths::ensure_dir(destination)?;

    let file_name = archive
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    if file_name.ends_with(".zip") {
        let file = fs::File::open(archive)?;
        let mut zip =
            zip::ZipArchive::new(file).map_err(|error| AppError::Archive(error.to_string()))?;
        for index in 0..zip.len() {
            let mut entry = zip
                .by_index(index)
                .map_err(|error| AppError::Archive(error.to_string()))?;
            let relative = match entry.enclosed_name() {
                Some(path) => path.to_path_buf(),
                None => continue,
            };
            let target = destination.join(relative);
            if entry.is_dir() {
                paths::ensure_dir(&target)?;
                continue;
            }
            if let Some(parent) = target.parent() {
                paths::ensure_dir(parent)?;
            }
            let mut out = fs::File::create(&target)?;
            std::io::copy(&mut entry, &mut out)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = entry.unix_mode() {
                    let _ = fs::set_permissions(&target, fs::Permissions::from_mode(mode));
                }
            }
        }
        return Ok(());
    }

    if file_name.ends_with(".tar.gz") || file_name.ends_with(".tgz") {
        let file = fs::File::open(archive)?;
        let decoder = flate2::read::GzDecoder::new(file);
        let mut tar = tar::Archive::new(decoder);
        tar.unpack(destination)
            .map_err(|error| AppError::Archive(error.to_string()))?;
        return Ok(());
    }

    if file_name.ends_with(".tar.xz") || file_name.ends_with(".txz") {
        let file = fs::File::open(archive)?;
        let decoder = xz2::read::XzDecoder::new(file);
        let mut tar = tar::Archive::new(decoder);
        tar.unpack(destination)
            .map_err(|error| AppError::Archive(error.to_string()))?;
        return Ok(());
    }

    // Unknown container format: try tar.gz, then fall back to a clear error.
    let file = fs::File::open(archive)?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut tar = tar::Archive::new(decoder);
    tar.unpack(destination).map_err(|error| {
        AppError::Archive(format!(
            "Unsupported archive (expected .tar.gz, .tar.xz or .zip): {error}"
        ))
    })?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn make_executable(root: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let names = [
        "wine",
        "wine64",
        "wineserver",
        "winetricks",
        "wineboot",
        "winecfg",
    ];
    for entry in walkdir::WalkDir::new(root)
        .max_depth(8)
        .follow_links(false)
        .into_iter()
        .flatten()
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().to_string();
        let is_binary = names.contains(&file_name.as_str())
            || entry
                .path()
                .parent()
                .and_then(|parent| parent.file_name())
                .map(|name| name == "bin")
                .unwrap_or(false);
        if is_binary {
            if let Ok(metadata) = entry.metadata() {
                let mut permissions = metadata.permissions();
                permissions.set_mode(0o755);
                let _ = fs::set_permissions(entry.path(), permissions);
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub async fn install_winetricks(app: &AppHandle) -> AppResult<()> {
    macos::install_winetricks(app).await.map(|_| ())
}

#[cfg(not(target_os = "macos"))]
pub async fn install_winetricks(_app: &AppHandle) -> AppResult<()> {
    crate::mock::install_winetricks().await
}
