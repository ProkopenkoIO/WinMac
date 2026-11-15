use crate::container;
use crate::engine::{self, EngineRuntime};
use crate::error::{AppError, AppResult};
use crate::models::*;
use crate::paths;
use crate::runner::{self, RunnerState};
use crate::winetricks;
use tauri::{AppHandle, Manager, State};

async fn blocking<T, F>(task: F) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> AppResult<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|error| AppError::Message(error.to_string()))?
}

fn parse_engine_kind(value: &str) -> EngineKind {
    match value {
        "gptk" => EngineKind::Gptk,
        "crossover" => EngineKind::Crossover,
        "custom" => EngineKind::Custom,
        _ => EngineKind::Wine,
    }
}

/* -------------------------------------------------------------------------- */
/* Platform                                                                   */
/* -------------------------------------------------------------------------- */

#[tauri::command]
pub fn get_platform_info(app: AppHandle) -> AppResult<PlatformInfo> {
    let mut info = PlatformInfo::default();
    info.data_dir = paths::app_support_dir(&app)
        .map(|path| paths::path_string(&path))
        .unwrap_or_default();
    info.containers_dir = paths::path_string(&paths::default_containers_dir());
    info.engine_dir = paths::engine_dir(&app)
        .map(|path| paths::path_string(&path))
        .unwrap_or_default();

    #[cfg(not(target_os = "macos"))]
    {
        info.mock_reason = Some(
            "Demo mode: engine detection, launches and downloads are simulated on this platform. Containers, configs and EXE scanning run for real."
                .to_string(),
        );
    }

    Ok(info)
}

/* -------------------------------------------------------------------------- */
/* Containers                                                                 */
/* -------------------------------------------------------------------------- */

#[tauri::command]
pub async fn list_containers(app: AppHandle) -> AppResult<Vec<ContainerSummary>> {
    blocking(move || container::list(&app)).await
}

#[tauri::command]
pub async fn get_container(app: AppHandle, id: String) -> AppResult<ContainerSummary> {
    blocking(move || {
        let config = container::find(&app, &id)?;
        Ok(container::summary(&app, config))
    })
    .await
}

#[tauri::command]
pub async fn create_container(
    app: AppHandle,
    request: CreateContainerRequest,
) -> AppResult<ContainerSummary> {
    blocking(move || {
        let mut config = ContainerConfig::default();
        config.windows_version = request.windows_version;
        config.graphics_backend = request.graphics_backend;
        config.esync = request.esync;
        config.msync = request.msync;
        config.metal_hud = request.metal_hud;
        config.retina = request.retina;
        config.engine_id = request.engine_id.filter(|id| !id.is_empty());

        container::create(&app, &request.name, request.location.as_deref(), config)
    })
    .await
}

#[tauri::command]
pub fn update_container(
    app: AppHandle,
    request: UpdateContainerRequest,
) -> AppResult<ContainerSummary> {
    container::update(&app, request)
}

#[tauri::command]
pub fn delete_container(app: AppHandle, id: String, delete_files: bool) -> AppResult<()> {
    container::delete(&app, &id, delete_files)
}

#[tauri::command]
pub fn clone_container(
    app: AppHandle,
    id: String,
    name: String,
    location: Option<String>,
) -> AppResult<ContainerSummary> {
    container::clone(&app, &id, &name, location.as_deref())
}

#[tauri::command]
pub fn import_container(app: AppHandle, path: String) -> AppResult<ContainerSummary> {
    container::import(&app, &path)
}

#[tauri::command]
pub async fn scan_games(app: AppHandle, container_id: String) -> AppResult<Vec<Game>> {
    blocking(move || {
        let config = container::find(&app, &container_id)?;
        container::scan_games(&app, &config)
    })
    .await
}

#[tauri::command]
pub fn set_game_override(
    app: AppHandle,
    container_id: String,
    game_id: String,
    game_override: Option<GameOverride>,
) -> AppResult<()> {
    container::set_game_override(
        &app,
        &container_id,
        &game_id,
        game_override.unwrap_or_default(),
    )
}

/* -------------------------------------------------------------------------- */
/* Launching                                                                  */
/* -------------------------------------------------------------------------- */

#[tauri::command]
pub async fn launch(app: AppHandle, request: LaunchRequest) -> AppResult<LaunchResult> {
    blocking(move || runner::launch(&app, request)).await
}

#[tauri::command]
pub fn stop_run(app: AppHandle, run_id: String) -> AppResult<()> {
    runner::stop(&app, &run_id)
}

#[tauri::command]
pub fn get_logs(app: AppHandle, container_id: String) -> AppResult<Vec<LogLine>> {
    Ok(runner::logs(&app, &container_id))
}

#[tauri::command]
pub fn clear_logs(app: AppHandle, container_id: String) -> AppResult<()> {
    runner::clear_logs(&app, &container_id);
    Ok(())
}

/* -------------------------------------------------------------------------- */
/* Engines                                                                    */
/* -------------------------------------------------------------------------- */

#[tauri::command]
pub fn get_engines(app: AppHandle) -> AppResult<EngineStateDto> {
    engine::state(&app)
}

#[tauri::command]
pub fn add_engine_manual(app: AppHandle, path: String) -> AppResult<EngineStateDto> {
    engine::add_manual(&app, &path)
}

#[tauri::command]
pub fn set_selected_engine(app: AppHandle, id: String) -> AppResult<EngineStateDto> {
    engine::set_selected(&app, &id)
}

#[tauri::command]
pub fn remove_engine(app: AppHandle, id: String) -> AppResult<EngineStateDto> {
    engine::remove(&app, &id)
}

#[tauri::command]
pub fn download_engine(
    app: AppHandle,
    runtime: State<'_, EngineRuntime>,
    id: String,
    url: Option<String>,
    name: Option<String>,
    kind: Option<String>,
) -> AppResult<()> {
    let catalog = engine::catalog();
    let item = catalog.iter().find(|item| item.id == id);

    let resolved_url = url
        .filter(|value| !value.trim().is_empty())
        .or_else(|| item.map(|item| item.url.clone()))
        .unwrap_or_default();
    if resolved_url.trim().is_empty() {
        return Err(AppError::Message(
            "This engine has no direct download URL — use “Locate wine binary” instead."
                .to_string(),
        ));
    }

    let resolved_name = name
        .filter(|value| !value.trim().is_empty())
        .or_else(|| item.map(|item| item.name.clone()))
        .unwrap_or_else(|| id.clone());
    let resolved_kind = kind
        .map(|value| parse_engine_kind(&value))
        .or_else(|| item.map(|item| item.kind))
        .unwrap_or(EngineKind::Wine);

    engine::download(
        &app,
        &runtime,
        id,
        resolved_url,
        resolved_name,
        resolved_kind,
    )
}

#[tauri::command]
pub fn cancel_engine_download(runtime: State<'_, EngineRuntime>, id: String) -> AppResult<()> {
    runtime.cancel(&id);
    Ok(())
}

#[tauri::command]
pub async fn install_winetricks(app: AppHandle) -> AppResult<()> {
    engine::install_winetricks(&app).await
}

/* -------------------------------------------------------------------------- */
/* winetricks                                                                 */
/* -------------------------------------------------------------------------- */

#[tauri::command]
pub fn list_winetricks_verbs() -> AppResult<Vec<WinetricksVerb>> {
    Ok(winetricks::verbs())
}

#[tauri::command]
pub async fn run_winetricks(
    app: AppHandle,
    container_id: String,
    verbs: Vec<String>,
) -> AppResult<LaunchResult> {
    if verbs.iter().any(|verb| !winetricks::is_known(verb)) {
        return Err(AppError::Message(
            "Unknown winetricks verb requested".to_string(),
        ));
    }
    blocking(move || runner::run_winetricks(&app, &container_id, &verbs)).await
}

/* -------------------------------------------------------------------------- */
/* File manager                                                               */
/* -------------------------------------------------------------------------- */

#[tauri::command]
pub fn reveal_in_file_manager(path: String) -> AppResult<()> {
    let path = paths::expand_tilde(&path);
    container::reveal_in_file_manager(&path)
}

#[tauri::command]
pub fn open_folder(path: String) -> AppResult<()> {
    let path = paths::expand_tilde(&path);
    container::reveal_in_file_manager(&path)
}

/* -------------------------------------------------------------------------- */
/* Misc                                                                       */
/* -------------------------------------------------------------------------- */

#[tauri::command]
pub fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
pub fn get_running_run(app: AppHandle, container_id: String) -> Option<String> {
    let state = app.state::<RunnerState>();
    #[cfg(target_os = "macos")]
    {
        let procs = state.procs.lock().unwrap();
        for (run_id, handle) in procs.iter() {
            if handle.container_id == container_id {
                return Some(run_id.clone());
            }
        }
    }
    let sim = state.sim.lock().unwrap();
    for (run_id, process) in sim.iter() {
        if process.container_id == container_id {
            return Some(run_id.clone());
        }
    }
    None
}
