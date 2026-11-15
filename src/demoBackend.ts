/**
 * Browser-only demo backend.
 *
 * The real application talks to the Rust core through Tauri commands. When the
 * UI is opened in a plain browser (`npm run dev` without Tauri) we fall back to
 * this in-memory implementation so the interface can be designed and reviewed
 * without a macOS host. It intentionally mirrors the Rust mock backend used on
 * Windows builds.
 */
import type {
  ContainerConfig,
  ContainerSummary,
  CreateContainerRequest,
  EngineCatalogItem,
  EngineInfo,
  EngineProgress,
  EngineState,
  Game,
  LaunchRequest,
  LogLine,
  PlatformInfo,
  RunKind,
  UpdateContainerRequest,
  WinetricksVerb,
  WindowsVersion,
  GraphicsBackend,
} from "./types";

const now = Date.now();
const iso = (offsetDays = 0) => new Date(now - offsetDays * 86_400_000).toISOString();

const VERBS: WinetricksVerb[] = [
  { verb: "vcrun2022", label: "Visual C++ 2015-2022", description: "Most modern games and launchers require this runtime.", category: "Runtimes" },
  { verb: "vcrun2019", label: "Visual C++ 2015-2019", description: "Legacy runtime used by many DX11 titles.", category: "Runtimes" },
  { verb: "dotnet48", label: ".NET Framework 4.8", description: "Required by many launchers and tools.", category: "Runtimes" },
  { verb: "d3dx9", label: "DirectX 9 (d3dx9)", description: "DirectX 9 redistributable libraries.", category: "DirectX" },
  { verb: "d3dx11_43", label: "DirectX 11 (d3dx11_43)", description: "D3DX11 helper libraries for older engines.", category: "DirectX" },
  { verb: "xact", label: "XACT audio", description: "Audio engine used by many Unreal Engine 3 games.", category: "Audio" },
  { verb: "corefonts", label: "Core fonts", description: "Arial, Times, Courier and friends.", category: "Fonts" },
  { verb: "faudio", label: "FAudio / XAudio2", description: "Modern XAudio2 implementation with better audio latency.", category: "Audio" },
  { verb: "mfinst", label: "Media Foundation", description: "Video playback for in-game cinematics.", category: "Media" },
  { verb: "vulkan-rt", label: "Vulkan runtime", description: "Vulkan loader used by the DXVK path.", category: "DirectX" },
];

const CATALOG: EngineCatalogItem[] = [
  {
    id: "apple-gptk-2",
    name: "Apple Game Porting Toolkit 2",
    version: "2.0",
    kind: "gptk",
    description: "Apple's D3DMetal translation layer. Best choice for DirectX 11/12 games on Apple Silicon.",
    url: "https://github.com/Whisky-App/wine-builds/releases",
    approxSizeMb: 420,
    recommended: true,
  },
  {
    id: "wine-staging-9",
    name: "Wine Staging 9.x",
    version: "9.0",
    kind: "wine",
    description: "Upstream Wine with staging patches. Pair with DXVK for DirectX 9/10/11 titles.",
    url: "https://github.com/Whisky-App/wine-builds/releases",
    approxSizeMb: 180,
    recommended: false,
  },
  {
    id: "wine-cx-24",
    name: "CrossOver 24 runtime",
    version: "24.0",
    kind: "crossover",
    description: "CodeWeavers runtime with proprietary patches. Requires an existing CrossOver installation.",
    url: "",
    approxSizeMb: 0,
    recommended: false,
  },
];

interface DemoContainer {
  config: ContainerConfig;
  games: Game[];
  sizeBytes: number;
  running: boolean;
}

function makeContainer(
  id: string,
  name: string,
  windows: WindowsVersion,
  backendKind: GraphicsBackend,
  games: Array<[string, string, number, string?]>,
  createdOffset: number,
): DemoContainer {
  const gamesMapped: Game[] = games.map(([gameName, exe, size, args], index) => ({
    id: `drive_c/${exe}`,
    name: gameName,
    exePath: `C:\\${exe.replace(/\//g, "\\")}`,
    args: args ?? "",
    source: exe.toLowerCase().startsWith("games/") ? "games" : "program-files",
    sizeBytes: size,
    lastPlayed: index === 0 ? iso(1) : index === 1 ? iso(3) : null,
    running: false,
  }));
  return {
    config: {
      id,
      name,
      path: `/Users/demo/Games/WinContainers/${name.replace(/\s+/g, "")}`,
      windowsVersion: windows,
      graphicsBackend: backendKind,
      esync: true,
      msync: backendKind !== "wined3d",
      metalHud: false,
      retina: true,
      engineId: backendKind === "d3dmetal" ? "apple-gptk-2" : "wine-staging-9",
      extraEnv: {},
      launchArgs: "",
      gameOverrides: {},
      lastPlayed: {},
      createdAt: iso(createdOffset),
      updatedAt: iso(0),
    },
    games: gamesMapped,
    sizeBytes: games.reduce((total, g) => total + g[2], 0) * 2.4,
    running: false,
  };
}

const containers: DemoContainer[] = [
  makeContainer("c-steam", "Steam Library", "win10", "d3dmetal", [
    ["Cyberpunk 2077", "Program Files/CD Projekt Red/Cyberpunk 2077/bin/x64/Cyberpunk2077.exe", 74_530_000_000],
    ["Elden Ring", "Games/ELDEN RING/Game/eldenring.exe", 51_200_000_000],
    ["Baldur's Gate 3", "Games/Baldurs Gate 3/bin/bg3.exe", 122_400_000_000],
    ["Hades II", "Games/Hades II/Hades2.exe", 9_800_000_000],
  ], 42),
  makeContainer("c-gog", "GOG Classics", "win10", "dxvk", [
    ["The Witcher 3: Wild Hunt", "Games/The Witcher 3/bin/x64/witcher3.exe", 48_600_000_000],
    ["Heroes of Might & Magic III", "Games/Heroes3/Heroes3.exe", 1_100_000_000],
  ], 21),
  makeContainer("c-retro", "Retro DX9", "win10", "wined3d", [
    ["Need for Speed: Most Wanted", "Games/NFS Most Wanted/nfsmw.exe", 3_400_000_000],
    ["GTA: San Andreas", "Games/GTA San Andreas/gta_sa.exe", 4_700_000_000],
  ], 8),
];

const engines: EngineInfo[] = [
  {
    id: "apple-gptk-2",
    name: "Apple Game Porting Toolkit 2",
    kind: "gptk",
    winePath: "/Applications/Game Porting Toolkit.app/Contents/Resources/wine/bin/wine64",
    winetricksPath: "/opt/homebrew/bin/winetricks",
    version: "wine-9.0 (gptk 2.0)",
    sourcePath: "/Applications/Game Porting Toolkit.app",
    managed: false,
    selected: true,
    available: true,
    supports: ["D3DMetal", "ESync", "MSync"],
    notes: "Detected in /Applications",
  },
  {
    id: "wine-staging-9",
    name: "Wine Staging 9.0",
    kind: "wine",
    winePath: "/opt/homebrew/bin/wine64",
    winetricksPath: "/opt/homebrew/bin/winetricks",
    version: "wine-9.0 (Staging)",
    sourcePath: "/opt/homebrew/bin",
    managed: false,
    selected: false,
    available: true,
    supports: ["DXVK", "ESync", "MSync"],
    notes: "Homebrew installation",
  },
  {
    id: "wine-cx-24",
    name: "CrossOver 24 runtime",
    kind: "crossover",
    winePath: "/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine",
    version: "wine-9.0 (CrossOver 24.0.4)",
    sourcePath: "/Applications/CrossOver.app",
    managed: false,
    selected: false,
    available: false,
    supports: ["DXVK", "ESync", "MSync"],
    notes: "CrossOver is not installed",
  },
];

type Listener = (payload: unknown) => void;
const listeners = new Map<string, Set<Listener>>();
const logStore = new Map<string, LogLine[]>();
const mockRuns = new Map<string, { containerId: string; cancelled: boolean; timer: number }>();

function emit(event: string, payload: unknown) {
  listeners.get(event)?.forEach((fn) => fn(payload));
}

function pushLog(containerId: string, runId: string, line: Omit<LogLine, "ts" | "containerId" | "runId">) {
  const entry: LogLine = { ...line, containerId, runId, ts: Date.now() };
  const list = logStore.get(containerId) ?? [];
  list.push(entry);
  if (list.length > 1500) list.splice(0, list.length - 1500);
  logStore.set(containerId, list);
  emit("wine-log", entry);
}

function summary(c: DemoContainer): ContainerSummary {
  const engine = engines.find((e) => e.id === c.config.engineId) ?? null;
  return {
    config: c.config,
    gameCount: c.games.length,
    sizeBytes: c.sizeBytes,
    engine: engine
      ? { id: engine.id, name: engine.name, version: engine.version, kind: engine.kind }
      : null,
    engineMissing: !engine?.available,
    running: c.running,
    exists: true,
  };
}

function findContainer(id: string): DemoContainer {
  const found = containers.find((c) => c.config.id === id);
  if (!found) throw new Error(`Container not found: ${id}`);
  return found;
}

function slug(name: string): string {
  return name.replace(/[^a-zA-Z0-9-_ ]/g, "").replace(/\s+/g, "") || "Container";
}

function setRunning(containerId: string, running: boolean) {
  const c = containers.find((x) => x.config.id === containerId);
  if (!c) return;
  c.running = running;
  c.games.forEach((g) => (g.running = running));
  emit("containers-changed", null);
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

function simulateRun(containerId: string, exe: string, kind: RunKind, runId: string) {
  const c = findContainer(containerId);
  const engine = engines.find((e) => e.id === c.config.engineId);
  const prefix = c.config.path;
  const env: Array<[string, string]> = [
    ["WINEPREFIX", prefix],
    ["WINEDEBUG", "-all"],
    ["WINEESYNC", c.config.esync ? "1" : "0"],
    ["WINEMSYNC", c.config.msync ? "1" : "0"],
  ];
  if (c.config.metalHud) env.push(["MTL_HUD_ENABLED", "1"]);
  if (c.config.graphicsBackend === "dxvk") env.push(["DXVK_FILTER_DEVICE_NAME", "Apple M3 Pro (MoltenVK)"]);
  if (c.config.graphicsBackend === "d3dmetal") env.push(["D3DM_ENABLE_HUD", c.config.metalHud ? "1" : "0"]);

  const script: Array<[number, Omit<LogLine, "ts" | "containerId" | "runId">]> = [
    [40, { stream: "system", level: "system", text: `$ wine64 "${exe}"` }],
    [80, { stream: "system", level: "system", text: env.map(([k, v]) => `${k}=${v}`).join("  ") }],
    [140, { stream: "stdout", level: "info", text: "wine: using engine " + (engine?.name ?? "unknown") + " (" + (engine?.version ?? "n/a") + ")" }],
    [200, { stream: "stderr", level: "warn", text: "0024:fixme:ntdll:NtQuerySystemInformation info_class SYSTEM_PROCESS_INFORMATION" }],
    [260, { stream: "stdout", level: "info", text: c.config.graphicsBackend === "d3dmetal"
      ? "D3DMetal: created Metal device Apple M3 Pro (40 cores), feature set Apple9"
      : c.config.graphicsBackend === "dxvk" ? "info:  DXVK: v2.3.1 (MoltenVK 1.2.8)" : "wine: using WineD3D on top of OpenGL/Metal" }],
    [330, { stream: "stdout", level: "info", text: "wine: initialized prefix C:\\ drive_c, 64-bit" }],
    [400, { stream: "stderr", level: "warn", text: "0024:warn:win:SetWindowTheme unsupported theme 'Explorer' (ignored)" }],
    [480, { stream: "stdout", level: "info", text: "Resolver: loading game configuration and shader cache…" }],
    [580, { stream: "stdout", level: "success", text: "Renderer ready — swapchain 2560x1440 @ 120 Hz (Metal HUD " + (c.config.metalHud ? "on" : "off") + ")" }],
    [700, { stream: "stdout", level: "info", text: "Game thread started. First frame presented in 412 ms." }],
  ];

  const run = { containerId, cancelled: false, timer: 0 };
  mockRuns.set(runId, run);
  setRunning(containerId, true);
  emit("run-started", { runId, containerId, kind, simulated: true });

  void (async () => {
    for (const [delay, line] of script) {
      if (run.cancelled) break;
      await sleep(delay);
      if (run.cancelled) break;
      pushLog(containerId, runId, line);
    }
    if (!run.cancelled) {
      pushLog(containerId, runId, {
        stream: "system",
        level: "success",
        text: "Process still running — UI demo mode (no real Wine process on this platform).",
      });
      // Keep the simulated session alive for a short while, then report exit.
      await sleep(4200);
    }
    if (run.cancelled) {
      pushLog(containerId, runId, { stream: "system", level: "warn", text: "Process terminated by user." });
    }
    mockRuns.delete(runId);
    setRunning(containerId, false);
    emit("run-exit", { runId, containerId, code: run.cancelled ? 130 : 0, simulated: true, kind });
  })();
}

export async function demoInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const a = (args ?? {}) as Record<string, any>;
  switch (cmd) {
    case "get_platform_info":
      return {
        os: "macos",
        arch: "aarch64",
        isMac: true,
        isMock: true,
        mockReason: "Browser preview — backend simulated in JavaScript",
        appVersion: "0.1.0",
        dataDir: "~/Library/Application Support/WinMacLauncher",
        containersDir: "~/Games/WinContainers",
        engineDir: "~/Library/Application Support/WinMacLauncher/engine",
      } satisfies PlatformInfo as unknown as T;

    case "list_containers":
      return containers.map(summary) as unknown as T;

    case "get_container":
      return summary(findContainer(a.id as string)) as unknown as T;

    case "create_container": {
      const req = a.request as CreateContainerRequest;
      const id = `c-${Math.random().toString(36).slice(2, 9)}`;
      const created: DemoContainer = {
        config: {
          id,
          name: req.name,
          path: `${req.location ?? "/Users/demo/Games/WinContainers"}/${slug(req.name)}`,
          windowsVersion: req.windowsVersion,
          graphicsBackend: req.graphicsBackend,
          esync: req.esync,
          msync: req.msync,
          metalHud: req.metalHud,
          retina: req.retina,
          engineId: req.engineId ?? null,
          extraEnv: {},
          launchArgs: "",
          gameOverrides: {},
          lastPlayed: {},
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
        },
        games: [],
        sizeBytes: 0,
        running: false,
      };
      containers.push(created);
      emit("containers-changed", null);
      return summary(created) as unknown as T;
    }

    case "update_container": {
      const req = a.request as UpdateContainerRequest;
      const c = findContainer(req.id);
      const cfg = c.config;
      if (req.name != null) cfg.name = req.name;
      if (req.windowsVersion != null) cfg.windowsVersion = req.windowsVersion;
      if (req.graphicsBackend != null) cfg.graphicsBackend = req.graphicsBackend;
      if (req.engineId !== undefined) cfg.engineId = req.engineId;
      if (req.esync != null) cfg.esync = req.esync;
      if (req.msync != null) cfg.msync = req.msync;
      if (req.metalHud != null) cfg.metalHud = req.metalHud;
      if (req.retina != null) cfg.retina = req.retina;
      if (req.launchArgs != null) cfg.launchArgs = req.launchArgs;
      if (req.extraEnv != null) cfg.extraEnv = req.extraEnv;
      cfg.updatedAt = new Date().toISOString();
      emit("containers-changed", null);
      return summary(c) as unknown as T;
    }

    case "delete_container": {
      const idx = containers.findIndex((c) => c.config.id === a.id);
      if (idx >= 0) containers.splice(idx, 1);
      emit("containers-changed", null);
      return null as unknown as T;
    }

    case "clone_container": {
      const src = findContainer(a.id as string);
      const id = `c-${Math.random().toString(36).slice(2, 9)}`;
      const name = a.name as string;
      const clone: DemoContainer = JSON.parse(JSON.stringify(src));
      clone.config.id = id;
      clone.config.name = name;
      clone.config.path = `${a.location ?? "/Users/demo/Games/WinContainers"}/${slug(name)}`;
      clone.config.createdAt = new Date().toISOString();
      clone.games = clone.games.map((g) => ({ ...g, running: false }));
      clone.running = false;
      containers.push(clone);
      emit("containers-changed", null);
      return summary(clone) as unknown as T;
    }

    case "import_container": {
      const path = a.path as string;
      const name = path.split(/[\\/]/).filter(Boolean).pop() ?? "Imported";
      return {
        config: {
          id: `c-${Math.random().toString(36).slice(2, 9)}`,
          name,
          path,
          windowsVersion: "win10",
          graphicsBackend: "d3dmetal",
          esync: true,
          msync: true,
          metalHud: false,
          retina: true,
          engineId: null,
          extraEnv: {},
          launchArgs: "",
          gameOverrides: {},
          lastPlayed: {},
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
        },
        gameCount: 0,
        sizeBytes: 0,
        engine: null,
        engineMissing: false,
        running: false,
        exists: true,
      } satisfies ContainerSummary as unknown as T;
    }

    case "scan_games":
      return (findContainer(a.containerId as string).games.slice()) as unknown as T;

    case "launch": {
      const req = a.request as LaunchRequest;
      const c = findContainer(req.containerId);
      const game = req.gameId ? c.games.find((g) => g.id === req.gameId) : undefined;
      const exe = req.exePath ?? game?.exePath ?? "C:\\setup.exe";
      const runId = `run-${Math.random().toString(36).slice(2, 8)}`;
      simulateRun(req.containerId, exe, req.kind, runId);
      return { runId, simulated: true, message: "Simulated launch" } as unknown as T;
    }

    case "stop_run": {
      const run = mockRuns.get(a.runId as string);
      if (run) run.cancelled = true;
      return null as unknown as T;
    }

    case "get_logs":
      return (logStore.get(a.containerId as string) ?? []).slice() as unknown as T;

    case "clear_logs":
      logStore.delete(a.containerId as string);
      return null as unknown as T;

    case "get_engines":
      return {
        engines,
        catalog: CATALOG,
        winetricksInstalled: true,
        engineDir: "~/Library/Application Support/WinMacLauncher/engine",
      } satisfies EngineState as unknown as T;

    case "add_engine_manual": {
      const path = a.path as string;
      const id = `custom-${Math.random().toString(36).slice(2, 7)}`;
      engines.push({
        id,
        name: path.split(/[\\/]/).filter(Boolean).pop() ?? "Custom engine",
        kind: "custom",
        winePath: path,
        version: "wine-9.0",
        sourcePath: path,
        managed: false,
        selected: true,
        available: true,
        supports: ["DXVK", "ESync", "MSync"],
      });
      emit("containers-changed", null);
      return engines as unknown as T;
    }

    case "download_engine": {
      const id = a.id as string;
      let downloaded = 0;
      const total = 240 * 1024 * 1024;
      for (let i = 0; i < 12; i++) {
        await sleep(180);
        downloaded = Math.round((total * (i + 1)) / 12);
        emit("engine-progress", {
          id,
          phase: i === 11 ? "extracting" : "downloading",
          downloaded,
          total,
          message: i === 11 ? "Extracting archive…" : "Downloading engine archive",
        } satisfies EngineProgress);
      }
      emit("engine-progress", { id, phase: "done", downloaded: total, total, message: "Ready" } satisfies EngineProgress);
      emit("containers-changed", null);
      return null as unknown as T;
    }

    case "cancel_engine_download":
      return null as unknown as T;

    case "install_winetricks":
      return null as unknown as T;

    case "list_winetricks_verbs":
      return VERBS as unknown as T;

    case "run_winetricks": {
      const containerId = a.containerId as string;
      const verbs = (a.verbs as string[]) ?? [];
      const runId = `run-${Math.random().toString(36).slice(2, 8)}`;
      setRunning(containerId, true);
      emit("run-started", { runId, containerId, kind: "winetricks", simulated: true });
      void (async () => {
        pushLog(containerId, runId, { stream: "system", level: "system", text: `$ winetricks ${verbs.join(" ")}` });
        for (const verb of verbs) {
          await sleep(320);
          pushLog(containerId, runId, { stream: "stdout", level: "info", text: `Executing ${verb}…` });
          await sleep(420);
          pushLog(containerId, runId, { stream: "stdout", level: "success", text: `${verb}: installation complete` });
        }
        setRunning(containerId, false);
        emit("run-exit", { runId, containerId, code: 0, simulated: true, kind: "winetricks" });
      })();
      return { runId, simulated: true, message: "Simulated winetricks run" } as unknown as T;
    }

    case "set_game_override": {
      const c = findContainer(a.containerId as string);
      const key = a.gameId as string;
      const override = (a.gameOverride ?? {}) as Record<string, unknown>;
      c.config.gameOverrides[key] = { ...(c.config.gameOverrides[key] ?? {}), ...override };
      if (override.hidden === true) {
        c.games = c.games.filter((g) => g.id !== key);
      }
      emit("containers-changed", null);
      return null as unknown as T;
    }

    case "reveal_in_file_manager":
    case "open_container_folder":
      return null as unknown as T;

    default:
      throw new Error(`Demo backend: command "${cmd}" is not implemented`);
  }
}

export function demoListen(event: string, handler: (payload: unknown) => void): () => void {
  const set = listeners.get(event) ?? new Set<Listener>();
  set.add(handler);
  listeners.set(event, set);
  return () => set.delete(handler);
}

export { VERBS as demoVerbs };
