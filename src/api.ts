import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { demoInvoke, demoListen } from "./demoBackend";
import type {
  ContainerSummary,
  CreateContainerRequest,
  EngineState,
  Game,
  GameOverride,
  LaunchRequest,
  LaunchResult,
  LogLine,
  PlatformInfo,
  UpdateContainerRequest,
  WinetricksVerb,
} from "./types";

export const isTauri =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in (window as unknown as Record<string, unknown>);

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri) return invoke<T>(cmd, args);
  return demoInvoke<T>(cmd, args);
}

export function onEvent<T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  if (isTauri) return listen<T>(event, (e) => handler(e.payload));
  const off = demoListen(event, (payload) => handler(payload as T));
  return Promise.resolve(off);
}

export const api = {
  platformInfo: () => call<PlatformInfo>("get_platform_info"),

  listContainers: () => call<ContainerSummary[]>("list_containers"),
  getContainer: (id: string) => call<ContainerSummary>("get_container", { id }),
  createContainer: (request: CreateContainerRequest) =>
    call<ContainerSummary>("create_container", { request }),
  updateContainer: (request: UpdateContainerRequest) =>
    call<ContainerSummary>("update_container", { request }),
  deleteContainer: (id: string, deleteFiles: boolean) =>
    call<void>("delete_container", { id, deleteFiles }),
  cloneContainer: (id: string, name: string, location: string | null) =>
    call<ContainerSummary>("clone_container", { id, name, location }),
  importContainer: (path: string) => call<ContainerSummary>("import_container", { path }),

  scanGames: (containerId: string) => call<Game[]>("scan_games", { containerId }),
  setGameOverride: (containerId: string, gameId: string, override: GameOverride) =>
    call<void>("set_game_override", { containerId, gameId, gameOverride: override }),

  launch: (request: LaunchRequest) => call<LaunchResult>("launch", { request }),
  stopRun: (runId: string) => call<void>("stop_run", { runId }),
  getLogs: (containerId: string) => call<LogLine[]>("get_logs", { containerId }),
  clearLogs: (containerId: string) => call<void>("clear_logs", { containerId }),

  engines: () => call<EngineState>("get_engines"),
  addEngineManual: (path: string) => call<EngineState>("add_engine_manual", { path }),
  downloadEngine: (id: string, url: string | null, name: string | null, kind: string) =>
    call<void>("download_engine", { id, url, name, kind }),
  cancelEngineDownload: (id: string) => call<void>("cancel_engine_download", { id }),
  installWinetricks: () => call<void>("install_winetricks"),
  setSelectedEngine: (id: string) => call<EngineState>("set_selected_engine", { id }),
  removeEngine: (id: string) => call<EngineState>("remove_engine", { id }),

  winetricksVerbs: () => call<WinetricksVerb[]>("list_winetricks_verbs"),
  runWinetricks: (containerId: string, verbs: string[]) =>
    call<LaunchResult>("run_winetricks", { containerId, verbs }),

  reveal: (path: string) => call<void>("reveal_in_file_manager", { path }),
  openFolder: (path: string) => call<void>("open_folder", { path }),
};

export async function pickExecutable(): Promise<string | null> {
  if (!isTauri) return "C:\\Games\\Demo\\Setup.exe";
  const selected = await openDialog({
    multiple: false,
    directory: false,
    filters: [{ name: "Windows programs", extensions: ["exe", "msi", "bat", "cmd"] }],
  });
  return typeof selected === "string" ? selected : null;
}

export async function pickFolder(title?: string): Promise<string | null> {
  if (!isTauri) return "/Users/demo/Games/WinContainers";
  const selected = await openDialog({ multiple: false, directory: true, title });
  return typeof selected === "string" ? selected : null;
}

export async function pickEngineBinary(): Promise<string | null> {
  if (!isTauri) return "/opt/homebrew/bin/wine64";
  const selected = await openDialog({
    multiple: false,
    directory: false,
    title: "Select a wine binary or engine folder",
    filters: [{ name: "Wine binaries", extensions: ["*"] }],
  });
  return typeof selected === "string" ? selected : null;
}
