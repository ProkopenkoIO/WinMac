use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/* -------------------------------------------------------------------------- */
/* Enums                                                                      */
/* -------------------------------------------------------------------------- */

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WindowsVersion {
    Win10,
    Win11,
}

impl Default for WindowsVersion {
    fn default() -> Self {
        WindowsVersion::Win10
    }
}

impl WindowsVersion {
    #[cfg(target_os = "macos")]
    pub fn registry_value(self) -> &'static str {
        match self {
            WindowsVersion::Win10 => "win10",
            WindowsVersion::Win11 => "win11",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphicsBackend {
    #[serde(rename = "d3dmetal")]
    D3DMetal,
    #[serde(rename = "dxvk")]
    Dxvk,
    #[serde(rename = "wined3d")]
    Wined3d,
}

impl Default for GraphicsBackend {
    fn default() -> Self {
        GraphicsBackend::D3DMetal
    }
}

impl GraphicsBackend {
    pub fn label(self) -> &'static str {
        match self {
            GraphicsBackend::D3DMetal => "D3DMetal",
            GraphicsBackend::Dxvk => "DXVK",
            GraphicsBackend::Wined3d => "WineD3D",
        }
    }

    /// DLL overrides required to force the selected translation layer inside Wine.
    pub fn dll_overrides(self) -> Option<&'static str> {
        match self {
            // Apple GPTK ships its own d3d11 / dxgi / d3d10core Metal shims.
            GraphicsBackend::D3DMetal => Some("d3d10core,d3d11,dxgi=n,b"),
            // DXVK ships d3d9 / d3d10core / d3d11 / dxgi; keep wined3d out of the way.
            GraphicsBackend::Dxvk => Some("d3d9,d3d10core,d3d11,dxgi=n,b"),
            GraphicsBackend::Wined3d => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EngineKind {
    Wine,
    Gptk,
    Crossover,
    Custom,
}

impl EngineKind {
    pub fn supports(self) -> Vec<String> {
        let labels: &[&str] = match self {
            EngineKind::Gptk => &["D3DMetal", "ESync", "MSync", "MoltenVK"],
            EngineKind::Wine => &["DXVK", "ESync", "MSync", "MoltenVK"],
            EngineKind::Crossover => &["D3DMetal", "DXVK", "ESync", "MSync"],
            EngineKind::Custom => &["DXVK", "ESync", "MSync"],
        };
        labels.iter().map(|label| label.to_string()).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GameSource {
    ProgramFiles,
    Games,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunKind {
    Game,
    Installer,
    Winetricks,
    Settings,
}

/* -------------------------------------------------------------------------- */
/* Container                                                                  */
/* -------------------------------------------------------------------------- */

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GameOverride {
    pub name: Option<String>,
    pub args: Option<String>,
    pub hidden: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ContainerConfig {
    pub id: String,
    pub name: String,
    pub path: String,
    pub windows_version: WindowsVersion,
    pub graphics_backend: GraphicsBackend,
    pub esync: bool,
    pub msync: bool,
    pub metal_hud: bool,
    pub retina: bool,
    pub engine_id: Option<String>,
    pub extra_env: BTreeMap<String, String>,
    pub launch_args: String,
    pub game_overrides: BTreeMap<String, GameOverride>,
    pub last_played: BTreeMap<String, String>,
    pub created_at: String,
    pub updated_at: String,
}

impl Default for ContainerConfig {
    fn default() -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: String::new(),
            name: String::new(),
            path: String::new(),
            windows_version: WindowsVersion::default(),
            graphics_backend: GraphicsBackend::default(),
            esync: true,
            msync: true,
            metal_hud: false,
            retina: true,
            engine_id: None,
            extra_env: BTreeMap::new(),
            launch_args: String::new(),
            game_overrides: BTreeMap::new(),
            last_played: BTreeMap::new(),
            created_at: now.clone(),
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineBrief {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub kind: EngineKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerSummary {
    pub config: ContainerConfig,
    pub game_count: usize,
    pub size_bytes: u64,
    pub engine: Option<EngineBrief>,
    pub engine_missing: bool,
    pub running: bool,
    pub exists: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub id: String,
    pub name: String,
    pub exe_path: String,
    pub args: String,
    pub source: GameSource,
    pub size_bytes: u64,
    pub last_played: Option<String>,
    pub running: bool,
}

/* -------------------------------------------------------------------------- */
/* Engines                                                                    */
/* -------------------------------------------------------------------------- */

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineInfo {
    pub id: String,
    pub name: String,
    pub kind: EngineKind,
    pub wine_path: String,
    pub winetricks_path: Option<String>,
    pub version: Option<String>,
    pub source_path: String,
    pub managed: bool,
    pub selected: bool,
    pub available: bool,
    pub supports: Vec<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineCatalogItem {
    pub id: String,
    pub name: String,
    pub version: String,
    pub kind: EngineKind,
    pub description: String,
    pub url: String,
    pub approx_size_mb: u64,
    pub recommended: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStateDto {
    pub engines: Vec<EngineInfo>,
    pub catalog: Vec<EngineCatalogItem>,
    pub winetricks_installed: bool,
    pub engine_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineProgress {
    pub id: String,
    pub phase: String,
    pub downloaded: u64,
    pub total: u64,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WinetricksVerb {
    pub verb: String,
    pub label: String,
    pub description: String,
    pub category: String,
}

/* -------------------------------------------------------------------------- */
/* Requests                                                                   */
/* -------------------------------------------------------------------------- */

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateContainerRequest {
    pub name: String,
    pub location: Option<String>,
    #[serde(default)]
    pub windows_version: WindowsVersion,
    #[serde(default)]
    pub graphics_backend: GraphicsBackend,
    #[serde(default)]
    pub engine_id: Option<String>,
    #[serde(default = "default_true")]
    pub esync: bool,
    #[serde(default = "default_true")]
    pub msync: bool,
    #[serde(default)]
    pub metal_hud: bool,
    #[serde(default = "default_true")]
    pub retina: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateContainerRequest {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub windows_version: Option<WindowsVersion>,
    #[serde(default)]
    pub graphics_backend: Option<GraphicsBackend>,
    #[serde(default)]
    pub engine_id: Option<String>,
    #[serde(default)]
    pub esync: Option<bool>,
    #[serde(default)]
    pub msync: Option<bool>,
    #[serde(default)]
    pub metal_hud: Option<bool>,
    #[serde(default)]
    pub retina: Option<bool>,
    #[serde(default)]
    pub launch_args: Option<String>,
    #[serde(default)]
    pub extra_env: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchRequest {
    pub container_id: String,
    #[serde(default)]
    pub game_id: Option<String>,
    #[serde(default)]
    pub exe_path: Option<String>,
    #[serde(default)]
    pub args: Option<String>,
    #[serde(default = "default_run_kind")]
    pub kind: RunKind,
}

fn default_run_kind() -> RunKind {
    RunKind::Game
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchResult {
    pub run_id: String,
    pub simulated: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    pub run_id: String,
    pub container_id: String,
    pub ts: i64,
    pub stream: String,
    pub level: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunExit {
    pub run_id: String,
    pub container_id: String,
    pub code: Option<i32>,
    pub simulated: bool,
    pub kind: RunKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformInfo {
    pub os: String,
    pub arch: String,
    pub is_mac: bool,
    pub is_mock: bool,
    pub mock_reason: Option<String>,
    pub app_version: String,
    pub data_dir: String,
    pub containers_dir: String,
    pub engine_dir: String,
}

impl Default for PlatformInfo {
    fn default() -> Self {
        Self {
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            is_mac: cfg!(target_os = "macos"),
            is_mock: !cfg!(target_os = "macos"),
            mock_reason: None,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            data_dir: String::new(),
            containers_dir: String::new(),
            engine_dir: String::new(),
        }
    }
}
