use crate::container;
use crate::engine;
use crate::error::{message, AppError, AppResult};
use crate::models::{ContainerConfig, LaunchRequest, LaunchResult, LogLine, RunKind};
#[cfg(target_os = "macos")]
use crate::models::{EngineInfo, RunExit};
use crate::paths;
use std::collections::{BTreeMap, HashMap};
#[cfg(target_os = "macos")]
use std::fs;
#[cfg(target_os = "macos")]
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
#[cfg(target_os = "macos")]
use std::thread;
#[cfg(target_os = "macos")]
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

const MAX_LOG_LINES: usize = 2500;

/* -------------------------------------------------------------------------- */
/* State                                                                      */
/* -------------------------------------------------------------------------- */

#[cfg(target_os = "macos")]
struct ProcessHandle {
    container_id: String,
    container_path: String,
    wine_path: String,
    prefix: String,
    kind: RunKind,
    child: std::process::Child,
}

pub struct SimProcess {
    pub container_id: String,
    pub container_path: String,
    pub cancel: Arc<AtomicBool>,
}

#[derive(Default)]
pub struct RunnerState {
    pub logs: Mutex<HashMap<String, Vec<LogLine>>>,
    /// Real child processes (populated on macOS only).
    #[cfg(target_os = "macos")]
    pub procs: Mutex<HashMap<String, ProcessHandle>>,
    /// Simulated processes on Windows / Linux demo builds.
    pub sim: Mutex<HashMap<String, SimProcess>>,
}

/* -------------------------------------------------------------------------- */
/* Log plumbing                                                               */
/* -------------------------------------------------------------------------- */

pub fn push_log(app: &AppHandle, line: LogLine) {
    if let Some(state) = app.try_state::<RunnerState>() {
        let mut logs = state.logs.lock().unwrap();
        let entry = logs.entry(line.container_id.clone()).or_default();
        entry.push(line.clone());
        if entry.len() > MAX_LOG_LINES {
            let excess = entry.len() - MAX_LOG_LINES;
            entry.drain(0..excess);
        }
    }
    let _ = app.emit("wine-log", &line);
}

fn log(
    run_id: &str,
    container_id: &str,
    stream: &str,
    level: &str,
    text: impl Into<String>,
) -> LogLine {
    LogLine {
        run_id: run_id.to_string(),
        container_id: container_id.to_string(),
        ts: chrono::Utc::now().timestamp_millis(),
        stream: stream.to_string(),
        level: level.to_string(),
        text: text.into(),
    }
}

pub fn logs(app: &AppHandle, container_id: &str) -> Vec<LogLine> {
    app.try_state::<RunnerState>()
        .and_then(|state| state.logs.lock().unwrap().get(container_id).cloned())
        .unwrap_or_default()
}

pub fn clear_logs(app: &AppHandle, container_id: &str) {
    if let Some(state) = app.try_state::<RunnerState>() {
        state.logs.lock().unwrap().remove(container_id);
    }
}

/* -------------------------------------------------------------------------- */
/* Environment                                                                */
/* -------------------------------------------------------------------------- */

pub fn build_env(config: &ContainerConfig, prefix: &Path) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    env.insert("WINEPREFIX".to_string(), paths::path_string(prefix));
    env.insert("WINEDEBUG".to_string(), "-all".to_string());
    env.insert(
        "WINEESYNC".to_string(),
        if config.esync { "1" } else { "0" }.to_string(),
    );
    env.insert(
        "WINEMSYNC".to_string(),
        if config.msync { "1" } else { "0" }.to_string(),
    );
    env.insert("WINE_LARGE_ADDRESS_AWARE".to_string(), "1".to_string());

    if let Some(overrides) = config.graphics_backend.dll_overrides() {
        env.insert("WINEDLLOVERRIDES".to_string(), overrides.to_string());
    }

    if config.metal_hud {
        env.insert("MTL_HUD_ENABLED".to_string(), "1".to_string());
        if config.graphics_backend == crate::models::GraphicsBackend::Dxvk {
            env.insert("DXVK_HUD".to_string(), "fps,frametimes".to_string());
        }
    }

    for (key, value) in &config.extra_env {
        env.insert(key.clone(), value.clone());
    }

    env
}

pub fn split_args(input: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;

    for character in input.chars() {
        if character == '"' || character == '\'' {
            if quote == Some(character) {
                quote = None;
            } else if quote.is_none() {
                quote = Some(character);
            } else {
                current.push(character);
            }
            continue;
        }
        if character.is_whitespace() && quote.is_none() {
            if !current.is_empty() {
                result.push(std::mem::take(&mut current));
            }
            continue;
        }
        current.push(character);
    }
    if !current.is_empty() {
        result.push(current);
    }
    result
}

fn host_path_for(input: &str, prefix: &Path) -> PathBuf {
    let trimmed = input.trim().trim_matches('"');
    // A real file on the host always wins — this keeps drag & drop working on
    // Windows demo builds, where host paths look like `C:\Games\setup.exe`.
    let direct = paths::expand_tilde(trimmed);
    if direct.is_file() {
        return direct;
    }
    let bytes = trimmed.as_bytes();
    let looks_windows = bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/');
    if looks_windows {
        let rest = trimmed[2..]
            .trim_start_matches(['\\', '/'])
            .replace('\\', "/");
        return prefix.join("drive_c").join(rest);
    }
    direct
}

fn to_windows_path(path: &Path, prefix: &Path) -> String {
    if let Ok(relative) = path.strip_prefix(prefix.join("drive_c")) {
        format!("C:\\{}", relative.to_string_lossy().replace('/', "\\"))
    } else {
        format!("Z:{}", path.to_string_lossy().replace('/', "\\"))
    }
}

/* -------------------------------------------------------------------------- */
/* Target resolution                                                          */
/* -------------------------------------------------------------------------- */

struct LaunchTarget {
    display: String,
    wine_args: Vec<String>,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    cwd: PathBuf,
}

fn resolve_target(
    app: &AppHandle,
    config: &ContainerConfig,
    request: &LaunchRequest,
) -> AppResult<LaunchTarget> {
    let prefix = PathBuf::from(&config.path);

    if let Some(game_id) = &request.game_id {
        let games = container::scan_games(app, config)?;
        let game = games
            .into_iter()
            .find(|game| &game.id == game_id)
            .ok_or_else(|| {
                AppError::Message(format!("Program {game_id} is no longer installed"))
            })?;
        let host = prefix.join(&game.id);
        if !host.is_file() {
            return message(format!("{} is missing from the container", host.display()));
        }

        let mut extra = split_args(&config.launch_args);
        extra.extend(split_args(&game.args));
        if let Some(args) = &request.args {
            extra.extend(split_args(args));
        }

        let mut wine_args = vec![paths::path_string(&host)];
        wine_args.extend(extra);

        let _ = container::mark_played(app, &config.id, game_id);

        return Ok(LaunchTarget {
            display: game.name,
            wine_args,
            cwd: host.parent().unwrap_or(&prefix).to_path_buf(),
        });
    }

    let raw = request
        .exe_path
        .clone()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AppError::Message("No program was selected".to_string()))?;
    let host = host_path_for(&raw, &prefix);
    if !host.is_file() {
        return message(format!("{} does not exist", host.display()));
    }

    let display = host
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| raw.clone());
    let mut extra = split_args(&config.launch_args);
    if let Some(args) = &request.args {
        extra.extend(split_args(args));
    }

    let is_msi = host
        .extension()
        .map(|ext| ext.eq_ignore_ascii_case("msi"))
        .unwrap_or(false);

    let mut wine_args = if is_msi {
        vec![
            "msiexec".to_string(),
            "/i".to_string(),
            to_windows_path(&host, &prefix),
        ]
    } else {
        vec![paths::path_string(&host)]
    };
    wine_args.extend(extra);

    Ok(LaunchTarget {
        display,
        wine_args,
        cwd: host.parent().unwrap_or(&prefix).to_path_buf(),
    })
}

/* -------------------------------------------------------------------------- */
/* Launch                                                                     */
/* -------------------------------------------------------------------------- */

pub fn launch(app: &AppHandle, request: LaunchRequest) -> AppResult<LaunchResult> {
    let config = container::find(app, &request.container_id)?;
    let prefix = PathBuf::from(&config.path);
    if !prefix.is_dir() {
        return message(format!(
            "Container folder {} is missing. Re-import it or create a new container.",
            config.path
        ));
    }

    let target = resolve_target(app, &config, &request)?;
    let engine = engine::resolve(app, config.engine_id.as_deref()).ok_or_else(|| {
        AppError::Message(
            "No compatible Wine engine is installed. Open the engine manager to download one or point WinMac at an existing Wine / GPTK / CrossOver installation."
                .to_string(),
        )
    })?;

    if let Some(configured) = &config.engine_id {
        if configured != &engine.id {
            push_log(
                app,
                log(
                    "startup",
                    &config.id,
                    "system",
                    "warn",
                    format!(
                        "Configured engine {configured} is unavailable — falling back to {}",
                        engine.name
                    ),
                ),
            );
        }
    }

    let run_id = uuid::Uuid::new_v4().to_string();
    let env = build_env(&config, &prefix);

    push_log(
        app,
        log(
            &run_id,
            &config.id,
            "system",
            "system",
            format!("$ wine64 {}", target.wine_args.join(" ")),
        ),
    );
    push_log(
        app,
        log(
            &run_id,
            &config.id,
            "system",
            "system",
            format!(
                "WINEPREFIX={}  WINEESYNC={}  WINEMSYNC={}  HUD={}  backend={}",
                config.path,
                config.esync as u8,
                config.msync as u8,
                config.metal_hud as u8,
                config.graphics_backend.label()
            ),
        ),
    );

    #[cfg(target_os = "macos")]
    {
        apply_settings_if_needed(app, &config, &engine, &prefix, &run_id);
        spawn_streamed(
            app,
            &run_id,
            &config,
            &prefix,
            Path::new(&engine.wine_path),
            &target.wine_args,
            env,
            Some(target.cwd.clone()),
            request.kind,
            &engine,
        )?;
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = &env;
        register_sim(app, &run_id, &config);
        crate::mock::simulate_run(
            app,
            run_id.clone(),
            config.id.clone(),
            config.path.clone(),
            target.display.clone(),
            request.kind,
            sim_flag(app, &run_id),
        )?;
    }

    Ok(LaunchResult {
        run_id,
        simulated: !cfg!(target_os = "macos"),
        message: if cfg!(target_os = "macos") {
            "Wine process started".to_string()
        } else {
            "Simulated launch (demo mode on this platform)".to_string()
        },
    })
}

fn register_sim(app: &AppHandle, run_id: &str, config: &ContainerConfig) {
    let state = app.state::<RunnerState>();
    let mut sim = state.sim.lock().unwrap();
    sim.insert(
        run_id.to_string(),
        SimProcess {
            container_id: config.id.clone(),
            container_path: config.path.clone(),
            cancel: Arc::new(AtomicBool::new(false)),
        },
    );
}

fn sim_flag(app: &AppHandle, run_id: &str) -> Arc<AtomicBool> {
    app.state::<RunnerState>()
        .sim
        .lock()
        .unwrap()
        .get(run_id)
        .map(|process| process.cancel.clone())
        .unwrap_or_else(|| Arc::new(AtomicBool::new(false)))
}

#[cfg(target_os = "macos")]
fn apply_settings_if_needed(
    app: &AppHandle,
    config: &ContainerConfig,
    _engine: &EngineInfo,
    prefix: &Path,
    run_id: &str,
) {
    let stamp = prefix.join(".winmac").join("settings.json");
    let current = fs::read_to_string(&stamp)
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .map(|value| {
            value
                .get("windowsVersion")
                .and_then(|value| value.as_str())
                .map(|value| value == config.windows_version.registry_value())
                .unwrap_or(false)
                && value
                    .get("retina")
                    .and_then(|value| value.as_bool())
                    .map(|value| value == config.retina)
                    .unwrap_or(false)
        })
        .unwrap_or(false);
    if current {
        return;
    }

    for warning in container::apply_registry_settings(app, config) {
        push_log(app, log(run_id, &config.id, "system", "warn", warning));
    }

    let _ = fs::create_dir_all(prefix.join(".winmac"));
    let _ = fs::write(
        stamp,
        format!(
            "{{\"windowsVersion\":\"{}\",\"retina\":{}}}\n",
            config.windows_version.registry_value(),
            config.retina
        ),
    );
}

#[cfg(target_os = "macos")]
#[allow(clippy::too_many_arguments)]
fn spawn_streamed(
    app: &AppHandle,
    run_id: &str,
    config: &ContainerConfig,
    prefix: &Path,
    program: &Path,
    args: &[String],
    env: BTreeMap<String, String>,
    cwd: Option<PathBuf>,
    kind: RunKind,
    engine: &EngineInfo,
) -> AppResult<()> {
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    let mut command = Command::new(program);
    command.args(args);
    command.envs(env);
    if let Some(directory) = &cwd {
        if directory.is_dir() {
            command.current_dir(directory);
        }
    }

    // Make the engine's private libraries discoverable, like Whisky does.
    if let Some(root) = Path::new(&engine.wine_path).parent().and_then(Path::parent) {
        let library = root.join("lib");
        if library.is_dir() {
            let existing = std::env::var("DYLD_FALLBACK_LIBRARY_PATH").unwrap_or_default();
            let value = if existing.is_empty() {
                format!("{}:/usr/lib:/usr/local/lib", paths::path_string(&library))
            } else {
                format!("{}:{existing}", paths::path_string(&library))
            };
            command.env("DYLD_FALLBACK_LIBRARY_PATH", value);
        }
    }

    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command.process_group(0);

    let mut child = command.spawn().map_err(|error| {
        AppError::Message(format!(
            "Could not start {}: {error}",
            paths::path_string(program)
        ))
    })?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    {
        let state = app.state::<RunnerState>();
        state.procs.lock().unwrap().insert(
            run_id.to_string(),
            ProcessHandle {
                container_id: config.id.clone(),
                container_path: config.path.clone(),
                wine_path: engine.wine_path.clone(),
                prefix: paths::path_string(prefix),
                kind,
                child,
            },
        );
    }

    if let Some(pipe) = stdout {
        stream_pipe(
            app.clone(),
            run_id.to_string(),
            config.id.clone(),
            "stdout",
            pipe,
        );
    }
    if let Some(pipe) = stderr {
        stream_pipe(
            app.clone(),
            run_id.to_string(),
            config.id.clone(),
            "stderr",
            pipe,
        );
    }

    watch_process(app.clone(), run_id.to_string());
    Ok(())
}

#[cfg(target_os = "macos")]
fn stream_pipe<R: Read + Send + 'static>(
    app: AppHandle,
    run_id: String,
    container_id: String,
    stream: &'static str,
    reader: R,
) {
    thread::spawn(move || {
        let buffered = BufReader::new(reader);
        for line in buffered.lines() {
            let text = match line {
                Ok(text) => text,
                Err(_) => break,
            };
            if text.trim().is_empty() {
                continue;
            }
            push_log(
                &app,
                log(
                    &run_id,
                    &container_id,
                    stream,
                    level_for(stream, &text),
                    text,
                ),
            );
        }
    });
}

#[cfg(target_os = "macos")]
fn level_for(stream: &str, text: &str) -> &'static str {
    let lower = text.to_lowercase();
    let is_error = lower.contains("err:")
        || lower.contains("unhandled exception")
        || lower.contains("fatal")
        || (stream == "stdout" && lower.contains("error"));
    if is_error {
        return "error";
    }
    if stream == "stderr" || lower.contains("warn") || lower.contains("fixme") {
        return "warn";
    }
    if lower.contains("ready") || lower.contains("initialized") {
        return "success";
    }
    "info"
}

#[cfg(target_os = "macos")]
fn watch_process(app: AppHandle, run_id: String) {
    thread::spawn(move || loop {
        thread::sleep(Duration::from_millis(140));

        let state = match app.try_state::<RunnerState>() {
            Some(state) => state,
            None => return,
        };
        let mut procs = state.procs.lock().unwrap();
        let finished = match procs.get_mut(&run_id) {
            Some(handle) => match handle.child.try_wait() {
                Ok(Some(status)) => Some((handle.container_id.clone(), handle.kind, status.code())),
                Ok(None) => None,
                Err(_) => Some((handle.container_id.clone(), handle.kind, None)),
            },
            None => return,
        };

        if let Some((container_id, kind, code)) = finished {
            procs.remove(&run_id);
            drop(procs);

            let level = if code == Some(0) { "success" } else { "error" };
            let text = match code {
                Some(code) => format!("Process exited with code {code}."),
                None => "Process terminated by a signal.".to_string(),
            };
            push_log(&app, log(&run_id, &container_id, "system", level, text));

            let _ = app.emit(
                "run-exit",
                RunExit {
                    run_id: run_id.clone(),
                    container_id,
                    code,
                    simulated: false,
                    kind,
                },
            );
            return;
        }
    });
}

/* -------------------------------------------------------------------------- */
/* Stop                                                                       */
/* -------------------------------------------------------------------------- */

pub fn stop(app: &AppHandle, run_id: &str) -> AppResult<()> {
    let state = app.state::<RunnerState>();

    if let Some(process) = state.sim.lock().unwrap().get(run_id) {
        process.cancel.store(true, Ordering::SeqCst);
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        let mut procs = state.procs.lock().unwrap();
        let Some(handle) = procs.get_mut(run_id) else {
            return message("That process is not running any more");
        };

        // Wine spawns the game in a child process: kill the whole process group.
        let pid = handle.child.id();
        let _ = std::process::Command::new("kill")
            .args(["-9", &format!("-{pid}")])
            .status();
        let _ = handle.child.kill();

        let wine_path = handle.wine_path.clone();
        let prefix = handle.prefix.clone();
        drop(procs);
        kill_wineserver(&wine_path, &prefix);
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = state;
        message("That process is not running any more")
    }
}

#[cfg(target_os = "macos")]
fn kill_wineserver(wine_path: &str, prefix: &str) {
    if let Some(directory) = Path::new(wine_path).parent() {
        for name in ["wineserver", "wineserver64"] {
            let candidate = directory.join(name);
            if candidate.is_file() {
                let _ = std::process::Command::new(candidate)
                    .arg("-k")
                    .env("WINEPREFIX", prefix)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
                return;
            }
        }
    }
}

/* -------------------------------------------------------------------------- */
/* winetricks                                                                 */
/* -------------------------------------------------------------------------- */

pub fn run_winetricks(
    app: &AppHandle,
    container_id: &str,
    verbs: &[String],
) -> AppResult<LaunchResult> {
    if verbs.is_empty() {
        return message("Select at least one dependency to install");
    }
    let config = container::find(app, container_id)?;
    let prefix = PathBuf::from(&config.path);
    if !prefix.is_dir() {
        return message("Container folder is missing");
    }
    let engine = engine::resolve(app, config.engine_id.as_deref()).ok_or_else(|| {
        AppError::Message("No Wine engine is installed — open the engine manager first".to_string())
    })?;

    let run_id = uuid::Uuid::new_v4().to_string();
    let env = build_env(&config, &prefix);
    let display = verbs.join(", ");

    #[cfg(target_os = "macos")]
    {
        let winetricks = engine::winetricks_path(app)
            .or_else(|| engine.winetricks_path.as_ref().map(PathBuf::from))
            .ok_or_else(|| {
                AppError::Message(
                    "winetricks is not installed. Install it from the engine manager.".to_string(),
                )
            })?;

        push_log(
            app,
            log(
                &run_id,
                &config.id,
                "system",
                "system",
                format!("$ {} {display}", paths::path_string(&winetricks)),
            ),
        );

        spawn_streamed(
            app,
            &run_id,
            &config,
            &prefix,
            &winetricks,
            verbs,
            env,
            Some(prefix.clone()),
            RunKind::Winetricks,
            &engine,
        )?;
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = &env;
        let _ = &engine;
        register_sim(app, &run_id, &config);
        crate::mock::simulate_run(
            app,
            run_id.clone(),
            config.id.clone(),
            config.path.clone(),
            display.clone(),
            RunKind::Winetricks,
            sim_flag(app, &run_id),
        )?;
    }

    Ok(LaunchResult {
        run_id,
        simulated: !cfg!(target_os = "macos"),
        message: format!("winetricks: {display}"),
    })
}

/* -------------------------------------------------------------------------- */
/* Queries                                                                    */
/* -------------------------------------------------------------------------- */

pub fn is_container_running(app: &AppHandle, directory: &Path) -> bool {
    let state = match app.try_state::<RunnerState>() {
        Some(state) => state,
        None => return false,
    };
    let key = paths::path_string(directory);

    #[cfg(target_os = "macos")]
    {
        if state
            .procs
            .lock()
            .unwrap()
            .values()
            .any(|process| process.container_path == key)
        {
            return true;
        }
    }

    if state
        .sim
        .lock()
        .unwrap()
        .values()
        .any(|process| process.container_path == key)
    {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ContainerConfig, GraphicsBackend};

    #[test]
    fn splits_quoted_arguments() {
        assert_eq!(
            split_args("-windowed -novid \"C:\\Games\\My Game\\game.exe\""),
            vec!["-windowed", "-novid", "C:\\Games\\My Game\\game.exe"]
        );
    }

    #[test]
    fn builds_expected_environment() {
        let mut config = ContainerConfig::default();
        config.esync = true;
        config.msync = true;
        config.metal_hud = true;
        config.graphics_backend = GraphicsBackend::D3DMetal;
        config.extra_env.insert("FOO".into(), "bar".into());

        let env = build_env(&config, Path::new("/tmp/prefix"));
        assert_eq!(env.get("WINEESYNC").map(String::as_str), Some("1"));
        assert_eq!(env.get("WINEMSYNC").map(String::as_str), Some("1"));
        assert_eq!(env.get("MTL_HUD_ENABLED").map(String::as_str), Some("1"));
        assert_eq!(
            env.get("WINEDLLOVERRIDES").map(String::as_str),
            Some("d3d10core,d3d11,dxgi=n,b")
        );
        assert_eq!(env.get("FOO").map(String::as_str), Some("bar"));
    }

    #[test]
    fn maps_windows_paths_into_the_prefix() {
        let prefix = Path::new("/Users/demo/Games/Test");
        let mapped = host_path_for("C:\\Games\\ELDEN RING\\game.exe", prefix);
        assert_eq!(
            mapped,
            PathBuf::from("/Users/demo/Games/Test/drive_c/Games/ELDEN RING/game.exe")
        );
    }
}
