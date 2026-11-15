export type WindowsVersion = "win10" | "win11";
export type GraphicsBackend = "d3dmetal" | "dxvk" | "wined3d";
export type EngineKind = "wine" | "gptk" | "crossover" | "custom";
export type GameSource = "program-files" | "games" | "manual";

export interface GameOverride {
  name?: string | null;
  args?: string | null;
  hidden?: boolean | null;
}

export interface ContainerConfig {
  id: string;
  name: string;
  path: string;
  windowsVersion: WindowsVersion;
  graphicsBackend: GraphicsBackend;
  esync: boolean;
  msync: boolean;
  metalHud: boolean;
  retina: boolean;
  engineId?: string | null;
  extraEnv: Record<string, string>;
  launchArgs: string;
  gameOverrides: Record<string, GameOverride>;
  lastPlayed: Record<string, string>;
  createdAt: string;
  updatedAt: string;
}

export interface EngineBrief {
  id: string;
  name: string;
  version?: string | null;
  kind: EngineKind;
}

export interface ContainerSummary {
  config: ContainerConfig;
  gameCount: number;
  sizeBytes: number;
  engine?: EngineBrief | null;
  engineMissing: boolean;
  running: boolean;
  exists: boolean;
}

export interface Game {
  id: string;
  name: string;
  exePath: string;
  args: string;
  source: GameSource;
  sizeBytes: number;
  lastPlayed?: string | null;
  running: boolean;
}

export interface EngineInfo {
  id: string;
  name: string;
  kind: EngineKind;
  winePath: string;
  winetricksPath?: string | null;
  version?: string | null;
  sourcePath: string;
  managed: boolean;
  selected: boolean;
  available: boolean;
  supports: string[];
  notes?: string | null;
}

export interface EngineCatalogItem {
  id: string;
  name: string;
  version: string;
  kind: EngineKind;
  description: string;
  url: string;
  approxSizeMb: number;
  recommended: boolean;
}

export interface EngineState {
  engines: EngineInfo[];
  catalog: EngineCatalogItem[];
  winetricksInstalled: boolean;
  engineDir: string;
}

export interface EngineProgress {
  id: string;
  phase: "downloading" | "extracting" | "done" | "error" | "cancelled";
  downloaded: number;
  total: number;
  message?: string | null;
}

export interface LogLine {
  runId: string;
  containerId: string;
  ts: number;
  stream: "stdout" | "stderr" | "system";
  level: "info" | "warn" | "error" | "success" | "system";
  text: string;
}

export interface RunExit {
  runId: string;
  containerId: string;
  code: number | null;
  simulated: boolean;
  kind: RunKind;
}

export type RunKind = "game" | "installer" | "winetricks" | "settings";

export interface PlatformInfo {
  os: "macos" | "windows" | "linux";
  arch: string;
  isMac: boolean;
  isMock: boolean;
  mockReason?: string | null;
  appVersion: string;
  dataDir: string;
  containersDir: string;
  engineDir: string;
}

export interface WinetricksVerb {
  verb: string;
  label: string;
  description: string;
  category: string;
}

export interface CreateContainerRequest {
  name: string;
  location?: string | null;
  windowsVersion: WindowsVersion;
  graphicsBackend: GraphicsBackend;
  engineId?: string | null;
  esync: boolean;
  msync: boolean;
  metalHud: boolean;
  retina: boolean;
}

export interface UpdateContainerRequest {
  id: string;
  name?: string | null;
  windowsVersion?: WindowsVersion | null;
  graphicsBackend?: GraphicsBackend | null;
  engineId?: string | null;
  esync?: boolean | null;
  msync?: boolean | null;
  metalHud?: boolean | null;
  retina?: boolean | null;
  launchArgs?: string | null;
  extraEnv?: Record<string, string> | null;
}

export interface LaunchRequest {
  containerId: string;
  gameId?: string | null;
  exePath?: string | null;
  args?: string | null;
  kind: RunKind;
}

export interface LaunchResult {
  runId: string;
  simulated: boolean;
  message: string;
}
