import "./styles.css";
import { api, isTauri, onEvent, pickExecutable, pickFolder } from "./api";
import { ConsolePanel } from "./console";
import { icon, logoMark } from "./icons";
import {
  confirmDialog,
  openContainerMenu,
  openEngineManagerModal,
  openGameMenu,
  openNewContainerModal,
  openSettingsModal,
  openWinetricksModal,
  promptDialog,
} from "./modals";
import { toast } from "./toast";
import type {
  ContainerSummary,
  EngineProgress,
  Game,
  LogLine,
  PlatformInfo,
  RunExit,
  WinetricksVerb,
} from "./types";
import {
  avatarGradient,
  backendLabel,
  escapeHtml,
  formatBytes,
  gradientFor,
  monogram,
  shortPath,
  windowsLabel,
} from "./ui";

/* -------------------------------------------------------------------------- */
/* State                                                                      */
/* -------------------------------------------------------------------------- */

interface AppState {
  platform: PlatformInfo | null;
  containers: ContainerSummary[];
  activeId: string | null;
  games: Game[];
  engines: WinetricksVerb[];
  engineState: Awaited<ReturnType<typeof api.engines>> | null;
  runs: Map<string, string>;
  loading: boolean;
}

const state: AppState = {
  platform: null,
  containers: [],
  activeId: null,
  games: [],
  engines: [],
  engineState: null,
  runs: new Map(),
  loading: true,
};

const app = document.getElementById("app")!;
let panel: ConsolePanel;
let refreshTimer: number | undefined;

const platformName = () => {
  if (!state.platform) return "macOS";
  return state.platform.os === "macos" ? "macOS" : state.platform.os === "windows" ? "Windows" : "Linux";
};

const activeContainer = (): ContainerSummary | undefined =>
  state.containers.find((container) => container.config.id === state.activeId);

const engineForActive = () => {
  const container = activeContainer();
  if (!container || !state.engineState) return null;
  const engines = state.engineState.engines;
  return (
    engines.find((engine) => engine.id === container.config.engineId) ??
    engines.find((engine) => engine.selected && engine.available) ??
    engines.find((engine) => engine.available) ??
    null
  );
};

/* -------------------------------------------------------------------------- */
/* Shell                                                                      */
/* -------------------------------------------------------------------------- */

function renderShell(): void {
  const platform = state.platform!;
  document.body.classList.toggle("platform-macos", platform.os === "macos" && isTauri);
  app.innerHTML = `
    <div class="app-shell">
      <aside class="sidebar">
        <div class="brand" data-tauri-drag-region>
          <div class="brand-mark">${logoMark(20)}</div>
          <div class="brand-text">
            <span class="brand-title">WinMac</span>
            <span class="brand-sub">Game Launcher · ${escapeHtml(platformName())}</span>
          </div>
        </div>
        <div class="sidebar-heading">
          <span>Containers</span>
          <button class="icon-btn" data-action="new-container" title="New container">${icon("plus", 15)}</button>
        </div>
        <div class="container-list" data-container-list></div>
        <button class="new-container-btn" data-action="new-container"><span>+</span> New container</button>
        <div class="sidebar-footer">
          <button class="engine-pill" data-action="engine-manager" title="Manage compatibility engines">
            <span class="status-dot ${engineDot()}" data-engine-dot></span>
            <span class="caption" data-engine-caption>Checking engine…</span>
            <span class="chev">${icon("chevron", 13)}</span>
          </button>
          ${
            platform.isMock
              ? `<div class="sidebar-demo-note">${escapeHtml(platform.mockReason ?? "Demo mode — system calls are simulated on this platform.")}</div>`
              : ""
          }
        </div>
      </aside>
      <main class="main">
        <header class="topbar" data-tauri-drag-region data-topbar></header>
        <div class="content" data-content></div>
        <div class="statusbar" data-statusbar></div>
        <div class="drop-overlay" data-drop-overlay>
          <div>
            <div class="big">Drop a Windows program</div>
            <div class="small">Release a .exe or .msi to run it inside the selected container</div>
          </div>
        </div>
      </main>
    </div>`;

  panel = new ConsolePanel(document.querySelector(".main")!, {
    onClear: () => {
      if (state.activeId) void api.clearLogs(state.activeId).catch(() => undefined);
    },
    onStop: () => void stopActiveRun(),
  });
  const statusbar = document.querySelector("[data-statusbar]")!;
  statusbar.parentElement!.insertBefore(panel.element, statusbar);

  renderSidebar();
  renderActiveView();
}

function engineDot(): string {
  const engines = state.engineState?.engines ?? [];
  const available = engines.filter((engine) => engine.available);
  if (available.length === 0) return "err";
  if (available.some((engine) => engine.selected)) return "ok";
  return "warn";
}

function engineCaption(): string {
  const engines = state.engineState?.engines ?? [];
  const available = engines.filter((engine) => engine.available);
  if (available.length === 0) return "No engine detected";
  const selected = available.find((engine) => engine.selected) ?? available[0];
  const version = selected.version?.replace(/^wine-/, "").split(" ")[0] ?? "";
  return `${selected.name}${version ? ` ${version}` : ""}`;
}

function renderSidebar(): void {
  const list = document.querySelector<HTMLElement>("[data-container-list]");
  if (!list) return;
  list.innerHTML = state.containers
    .map((container) => {
      const config = container.config;
      const running = state.runs.has(config.id) || container.running;
      return `<button class="container-item ${config.id === state.activeId ? "active" : ""}"
          data-action="select-container" data-id="${escapeHtml(config.id)}">
        <span class="container-avatar" style="background:${avatarGradient(config.name)}">${escapeHtml(monogram(config.name))}</span>
        <span class="container-meta">
          <span class="container-name">${escapeHtml(config.name)}</span>
          <span class="container-caption">
            ${running ? '<span class="running-dot"></span>' : ""}
            <span>${container.gameCount} ${container.gameCount === 1 ? "app" : "apps"}</span>
            <span class="dot"></span>
            <span>${escapeHtml(backendLabel(config.graphicsBackend))}</span>
          </span>
        </span>
        ${container.engineMissing ? `<span class="status-dot err" title="Engine missing"></span>` : ""}
      </button>`;
    })
    .join("");

  if (state.containers.length === 0) {
    list.innerHTML = `<div style="padding:14px 10px;font-size:11.5px;color:var(--muted);line-height:1.5">
      No containers yet.<br />Create one to get started.
    </div>`;
  }

  const dot = document.querySelector<HTMLElement>("[data-engine-dot]");
  const caption = document.querySelector<HTMLElement>("[data-engine-caption]");
  if (dot) dot.className = `status-dot ${engineDot()}`;
  if (caption) caption.textContent = engineCaption();
}

function renderTopbar(): void {
  const topbar = document.querySelector<HTMLElement>("[data-topbar]");
  if (!topbar) return;
  const container = activeContainer();
  if (!container) {
    topbar.innerHTML = `
      <div class="container-head">
        <div class="container-head">
          <div class="avatar" style="background:${avatarGradient("WinMac")}">${logoMark(22)}</div>
          <div class="title-block">
            <h1>Welcome to WinMac</h1>
            <div class="subline">Run Windows games on ${escapeHtml(platformName())} with Wine + Metal</div>
          </div>
        </div>
      </div>
      <div class="topbar-actions">
        <button class="btn" data-action="engine-manager">${icon("package", 14)} Engines</button>
        <button class="btn primary" data-action="new-container">${icon("plus", 14)} New container</button>
      </div>`;
    return;
  }
  const config = container.config;
  const engine = engineForActive();
  const running = state.runs.has(config.id) || container.running;
  topbar.innerHTML = `
    <div class="container-head">
      <div class="avatar" style="background:${avatarGradient(config.name)}">${escapeHtml(monogram(config.name))}</div>
      <div class="title-block">
        <h1>${escapeHtml(config.name)}</h1>
        <div class="subline">
          <span class="chip">${escapeHtml(windowsLabel(config.windowsVersion))}</span>
          <span class="chip accent">${escapeHtml(backendLabel(config.graphicsBackend))}</span>
          ${config.msync ? '<span class="chip">MSync</span>' : config.esync ? '<span class="chip">ESync</span>' : ""}
          ${config.metalHud ? '<span class="chip warn">Metal HUD</span>' : ""}
          ${
            engine
              ? `<span class="chip ${engine.available ? "ok" : "warn"}">${escapeHtml(engine.name)}</span>`
              : '<span class="chip warn">No engine</span>'
          }
          <span>${container.gameCount} ${container.gameCount === 1 ? "app" : "apps"} · ${formatBytes(container.sizeBytes)}</span>
        </div>
      </div>
    </div>
    <div class="topbar-actions">
      ${
        running
          ? `<button class="btn danger" data-action="stop-run">${icon("stop", 12)} Stop</button>`
          : ""
      }
      <button class="btn" data-action="container-menu" title="Container actions">${icon("more", 15)}</button>
      <button class="btn" data-action="install-exe">${icon("download", 14)} Install .exe</button>
      <button class="btn primary" data-action="settings">${icon("sliders", 14)} Settings</button>
    </div>`;
}

function renderActiveView(): void {
  if (!panel) return;
  renderTopbar();
  const content = document.querySelector<HTMLElement>("[data-content]");
  const statusbar = document.querySelector<HTMLElement>("[data-statusbar]");
  const container = activeContainer();

  if (!container) {
    if (content) content.innerHTML = renderWelcome();
    if (statusbar) statusbar.innerHTML = renderStatusbar();
    panel.setLogs([]);
    panel.setRunning(false);
    return;
  }

  const config = container.config;
  const engine = engineForActive();
  const games = state.games
    .filter((game) => !(config.gameOverrides?.[game.id]?.hidden ?? false))
    .sort((a, b) => a.name.localeCompare(b.name));

  if (content) {
    content.innerHTML = `
      <div class="quickbar">
        <button class="quick-tile" data-action="install-exe">
          <span class="q-icon">${icon("bolt", 16)}</span>
          <span class="q-text"><span class="q-title">Run a .exe</span><span class="q-sub">Installer or portable program</span></span>
        </button>
        <button class="quick-tile" data-action="open-c-drive">
          <span class="q-icon violet">${icon("folder", 16)}</span>
          <span class="q-text"><span class="q-title">Open C: drive</span><span class="q-sub">drive_c in Finder</span></span>
        </button>
        <button class="quick-tile" data-action="settings">
          <span class="q-icon green">${icon("sliders", 16)}</span>
          <span class="q-text"><span class="q-title">Container settings</span><span class="q-sub">${escapeHtml(backendLabel(config.graphicsBackend))} · ${config.msync ? "MSync" : config.esync ? "ESync" : "No sync"}</span></span>
        </button>
        <button class="quick-tile" data-action="winetricks">
          <span class="q-icon amber">${icon("package", 16)}</span>
          <span class="q-text"><span class="q-title">Winetricks</span><span class="q-sub">VC++, .NET, DirectX deps</span></span>
        </button>
      </div>
      <div class="section-head">
        <h2>Installed apps <span class="count">${games.length}</span></h2>
        <div class="tools">
          <button class="btn small" data-action="rescan">${icon("refresh", 13)} Rescan</button>
          <button class="btn small" data-action="install-exe">${icon("plus", 13)} Add program</button>
        </div>
      </div>
      ${
        games.length > 0
          ? `<div class="game-grid">${games.map((game) => renderGameCard(game)).join("")}</div>`
          : `<div class="empty-state">
              <div class="empty-icon">${icon("gamepad", 20)}</div>
              <h3>No Windows programs found</h3>
              <p>Run an installer or copy a portable game into this container. WinMac scans Program Files and drive_c/Games automatically.</p>
              <div class="row">
                <button class="btn" data-action="install-exe">Run an installer</button>
                <button class="btn" data-action="open-c-drive">Open C: drive</button>
              </div>
            </div>`
      }
      ${
        engine && !engine.available
          ? `<div class="empty-state" style="margin-top:16px;border-color:rgba(239,181,83,.3)">
              <div class="empty-icon" style="background:rgba(239,181,83,.13);color:#f3cd8d">${icon("alert", 20)}</div>
              <h3>Compatibility engine not installed</h3>
              <p>Install Apple GPTK, Wine or CrossOver, or download a pre-built engine to start launching games.</p>
              <div class="row"><button class="btn primary" data-action="engine-manager">Open engine manager</button></div>
            </div>`
          : ""
      }`;
  }

  if (statusbar) statusbar.innerHTML = renderStatusbar();
  panel.element.style.display = "";
  panel.setRunning(state.runs.has(config.id) || container.running);
}

function renderWelcome(): string {
  return `
    <div class="empty-state" style="padding:64px 24px">
      <div class="empty-icon" style="width:52px;height:52px">${icon("wine", 24)}</div>
      <h3 style="font-size:17px">Your Windows game library, on ${escapeHtml(platformName())}</h3>
      <p>Create an isolated container (a Wine prefix with its own C: drive), install a game, and let D3DMetal or DXVK translate DirectX to Metal.</p>
      <div class="row" style="margin-top:12px">
        <button class="btn primary" data-action="new-container">${icon("plus", 14)} New container</button>
        <button class="btn" data-action="engine-manager">${icon("package", 14)} Manage engines</button>
      </div>
    </div>
    <div class="section-head" style="margin-top:26px">
      <h2>How it works</h2>
    </div>
    <div class="quickbar">
      <div class="quick-tile" style="cursor:default">
        <span class="q-icon">${icon("wine", 16)}</span>
        <span class="q-text"><span class="q-title">1 · Pick an engine</span><span class="q-sub">GPTK, Wine Staging or CrossOver</span></span>
      </div>
      <div class="quick-tile" style="cursor:default">
        <span class="q-icon violet">${icon("package", 16)}</span>
        <span class="q-text"><span class="q-title">2 · Create a container</span><span class="q-sub">Isolated WINEPREFIX per game</span></span>
      </div>
      <div class="quick-tile" style="cursor:default">
        <span class="q-icon green">${icon("bolt", 16)}</span>
        <span class="q-text"><span class="q-title">3 · Install &amp; play</span><span class="q-sub">D3DMetal / DXVK → Metal</span></span>
      </div>
      <div class="quick-tile" style="cursor:default">
        <span class="q-icon amber">${icon("terminal", 16)}</span>
        <span class="q-text"><span class="q-title">4 · Watch the log</span><span class="q-sub">Live Wine output &amp; errors</span></span>
      </div>
    </div>`;
}

function renderGameCard(game: Game): string {
  const config = activeContainer()?.config;
  const override = config?.gameOverrides?.[game.id];
  const displayName = override?.name || game.name;
  const running = state.runs.has(config?.id ?? "");
  const relative = game.id.replace(/^drive_c\//, "").replace(/\//g, " \\ ");
  return `<article class="game-card" data-game-id="${escapeHtml(game.id)}" tabindex="0" role="button" aria-label="Play ${escapeHtml(displayName)}">
    <div class="game-art" style="background:${gradientFor(displayName)}">
      <div class="art-grid"></div>
      <span class="art-badge ${running ? "ok" : ""}">${running ? "Running" : escapeHtml(game.source === "games" ? "Games" : "Program")}</span>
      <button class="card-menu-btn" data-action="game-menu" data-game-id="${escapeHtml(game.id)}" title="More actions">${icon("more", 14)}</button>
      <span class="monogram">${escapeHtml(monogram(displayName))}</span>
    </div>
    <div class="game-body">
      <div class="game-titles">
        <div class="game-name">${escapeHtml(displayName)}</div>
        <div class="game-path" title="${escapeHtml(game.exePath)}">${escapeHtml(shortPath(relative, 34))}</div>
      </div>
      <button class="play-fab" data-action="play-game" data-game-id="${escapeHtml(game.id)}" title="Play">${icon("play", 13)}</button>
    </div>
  </article>`;
}

function renderStatusbar(): string {
  const platform = state.platform;
  const container = activeContainer();
  const engine = engineForActive();
  const running = container ? state.runs.has(container.config.id) : false;
  return `
    <span class="status-dot ${engine ? (engine.available ? "ok" : "err") : "warn"}"></span>
    <span>${escapeHtml(engine ? engineCaption() : "No engine detected")}</span>
    <span class="sep"></span>
    <span>${container ? escapeHtml(windowsLabel(container.config.windowsVersion)) : "—"}</span>
    <span class="sep"></span>
    <span>${container ? escapeHtml(shortPath(container.config.path, 54)) : "No container selected"}</span>
    <span class="grow"></span>
    ${running ? `<span style="color:var(--ok)">process running</span>` : ""}
    ${platform?.isMock ? `<span class="mock-flag" title="${escapeHtml(platform.mockReason ?? "")}">demo mode</span>` : ""}
    <span class="sep"></span>
    <span>v${escapeHtml(platform?.appVersion ?? "0.1.0")}</span>`;
}

/* -------------------------------------------------------------------------- */
/* Data loading                                                               */
/* -------------------------------------------------------------------------- */

async function refreshContainers(preferredId?: string | null): Promise<void> {
  const previous = state.activeId;
  try {
    state.containers = await api.listContainers();
  } catch (error) {
    toast("err", "Could not load containers", String(error));
    state.containers = [];
  }
  const wanted = preferredId ?? previous;
  const nextActive =
    state.containers.find((container) => container.config.id === wanted)?.config.id ??
    state.containers[0]?.config.id ??
    null;

  const activeChanged = nextActive !== state.activeId;
  state.activeId = nextActive;

  if (state.activeId) {
    localStorage.setItem("winmac.activeContainer", state.activeId);
    if (activeChanged) {
      await loadContainerData(state.activeId);
    } else {
      try {
        state.games = await api.scanGames(state.activeId);
      } catch {
        state.games = [];
      }
    }
  } else {
    state.games = [];
  }

  renderSidebar();
  renderActiveView();
}

async function loadContainerData(containerId: string): Promise<void> {
  const [games, logs] = await Promise.all([
    api.scanGames(containerId).catch(() => [] as Game[]),
    api.getLogs(containerId).catch(() => [] as LogLine[]),
  ]);
  state.games = games;
  if (panel) {
    panel.setLogs(logs);
    const container = state.containers.find((c) => c.config.id === containerId);
    panel.setRunning(Boolean(container?.running || state.runs.has(containerId)));
  }
}

async function selectContainer(id: string): Promise<void> {
  if (state.activeId === id) return;
  state.activeId = id;
  localStorage.setItem("winmac.activeContainer", id);
  await loadContainerData(id);
  renderSidebar();
  renderActiveView();
}

function scheduleRefresh(): void {
  if (refreshTimer) window.clearTimeout(refreshTimer);
  refreshTimer = window.setTimeout(() => {
    void refreshContainers();
  }, 260);
}

async function refreshEngines(): Promise<void> {
  try {
    state.engineState = await api.engines();
  } catch {
    state.engineState = null;
  }
  renderSidebar();
  renderTopbar();
}

/* -------------------------------------------------------------------------- */
/* Actions                                                                    */
/* -------------------------------------------------------------------------- */

async function installProgram(): Promise<void> {
  const container = activeContainer();
  if (!container) {
    toast("warn", "No container selected", "Create a container first.");
    return;
  }
  const path = await pickExecutable();
  if (!path) return;
  await startInstaller(path);
}

async function startInstaller(path: string): Promise<void> {
  const container = activeContainer();
  if (!container) {
    toast("warn", "No container selected", "Create a container first.");
    return;
  }
  try {
    const result = await api.launch({
      containerId: container.config.id,
      exePath: path,
      kind: "installer",
    });
    state.runs.set(container.config.id, result.runId);
    panel.setCollapsed(false);
    panel.setRunning(true);
    renderSidebar();
    renderTopbar();
    toast("info", "Installer started", shortPath(path, 60));
  } catch (error) {
    toast("err", "Could not start installer", String(error));
    void promptEngineManager();
  }
}

async function promptEngineManager(): Promise<void> {
  const open = await confirmDialog({
    title: "Engine problem",
    message:
      "The launch failed. This usually means no compatible Wine engine is installed, or the selected engine is missing. Open the engine manager?",
    confirmLabel: "Open engine manager",
  });
  if (open) void openEngineManager();
}

async function launchGame(game: Game): Promise<void> {
  const container = activeContainer();
  if (!container) return;
  try {
    const result = await api.launch({
      containerId: container.config.id,
      gameId: game.id,
      kind: "game",
    });
    state.runs.set(container.config.id, result.runId);
    panel.setRunning(true);
    renderSidebar();
    renderTopbar();
    if (!result.simulated) toast("ok", `Launching ${game.name}`, "Wine output is streaming below.");
    else toast("info", `Launching ${game.name}`, result.message);
  } catch (error) {
    toast("err", "Launch failed", String(error));
    void promptEngineManager();
  }
}

async function stopActiveRun(): Promise<void> {
  const container = activeContainer();
  if (!container) return;
  const runId = state.runs.get(container.config.id);
  if (!runId) {
    toast("info", "Nothing to stop", "No process is running in this container.");
    return;
  }
  try {
    await api.stopRun(runId);
    state.runs.delete(container.config.id);
    panel.setRunning(false);
    renderSidebar();
    renderTopbar();
    toast("warn", "Process terminated");
  } catch (error) {
    toast("err", "Could not stop process", String(error));
  }
}

async function openEngineManager(): Promise<void> {
  await openEngineManagerModal({
    engines: state.engineState,
    onChanged: async () => {
      await refreshEngines();
    },
  });
}

async function openSettings(): Promise<void> {
  const container = activeContainer();
  if (!container) return;
  await openSettingsModal({
    container,
    engines: state.engineState,
    onSaved: async (updated) => {
      await refreshContainers(updated.config.id);
    },
    onDeleted: async (id) => {
      if (state.activeId === id) state.activeId = null;
      await refreshContainers();
    },
  });
}

async function openWinetricks(): Promise<void> {
  const container = activeContainer();
  if (!container) return;
  if (state.engines.length === 0) {
    state.engines = await api.winetricksVerbs().catch(() => []);
  }
  await openWinetricksModal({
    container,
    verbs: state.engines,
    onRun: async (verbs) => {
      try {
        const result = await api.runWinetricks(container.config.id, verbs);
        state.runs.set(container.config.id, result.runId);
        panel.setCollapsed(false);
        panel.setRunning(true);
        renderSidebar();
        renderTopbar();
        toast("info", "winetricks started", verbs.join(", "));
      } catch (error) {
        toast("err", "winetricks failed", String(error));
        throw error;
      }
    },
  });
}

async function openNewContainer(): Promise<void> {
  await openNewContainerModal({
    engines: state.engineState,
    defaultLocation: state.platform?.containersDir ?? "~/Games/WinContainers",
    onCreated: async (created) => {
      await refreshContainers(created.config.id);
    },
  });
}

async function cloneContainer(container: ContainerSummary): Promise<void> {
  const name = await promptDialog({
    title: "Clone container",
    subtitle: "Copies drive_c, registry and configuration into a new container.",
    label: "New container name",
    value: `${container.config.name} Copy`,
    confirmLabel: "Clone",
  });
  if (!name) return;
  try {
    const clone = await api.cloneContainer(container.config.id, name, null);
    await refreshContainers(clone.config.id);
    toast("ok", "Container cloned", clone.config.path);
  } catch (error) {
    toast("err", "Clone failed", String(error));
  }
}

async function deleteContainer(container: ContainerSummary): Promise<void> {
  const ok = await confirmDialog({
    title: "Delete container?",
    message: `“${container.config.name}” will be removed from the launcher and its folder (including installed games) will be deleted. This cannot be undone.`,
    confirmLabel: "Delete container",
    danger: true,
  });
  if (!ok) return;
  try {
    await api.deleteContainer(container.config.id, true);
    if (state.activeId === container.config.id) state.activeId = null;
    await refreshContainers();
    toast("ok", "Container deleted", container.config.name);
  } catch (error) {
    toast("err", "Could not delete container", String(error));
  }
}

async function importContainer(): Promise<void> {
  const folder = await pickFolder("Choose an existing container folder");
  if (!folder) return;
  try {
    const container = await api.importContainer(folder);
    await refreshContainers(container.config.id);
    toast("ok", "Container imported", container.config.name);
  } catch (error) {
    toast("err", "Could not import container", String(error));
  }
}

async function handleContainerMenu(anchor: { x: number; y: number }): Promise<void> {
  const container = activeContainer();
  if (!container) return;
  openContainerMenu(anchor, container, {
    settings: () => void openSettings(),
    reveal: () => void api.reveal(container.config.path).catch((error) => toast("err", "Reveal failed", String(error))),
    clone: () => void cloneContainer(container),
    rescan: () => void refreshContainers(container.config.id),
    delete: () => void deleteContainer(container),
  });
}

/* -------------------------------------------------------------------------- */
/* Events                                                                     */
/* -------------------------------------------------------------------------- */

function bindUiEvents(): void {
  document.addEventListener("click", (event) => {
    const target = (event.target as HTMLElement).closest<HTMLElement>("[data-action]");
    if (!target) return;
    const action = target.dataset.action;
    const containerId = target.dataset.id;
    switch (action) {
      case "select-container":
        if (containerId) void selectContainer(containerId);
        break;
      case "new-container":
        void openNewContainer();
        break;
      case "engine-manager":
        void openEngineManager();
        break;
      case "settings":
        void openSettings();
        break;
      case "install-exe":
        void installProgram();
        break;
      case "winetricks":
        void openWinetricks();
        break;
      case "open-c-drive": {
        const container = activeContainer();
        if (!container) return;
        void api
          .reveal(`${container.config.path}/drive_c`)
          .catch((error) => toast("err", "Could not open drive_c", String(error)));
        break;
      }
      case "rescan":
        void refreshContainers(state.activeId);
        toast("info", "Scanning for installed programs…");
        break;
      case "play-game": {
        const gameId = target.dataset.gameId;
        const game = state.games.find((item) => item.id === gameId);
        if (game) void launchGame(game);
        break;
      }
      case "game-menu": {
        event.stopPropagation();
        const gameId = target.dataset.gameId;
        const game = state.games.find((item) => item.id === gameId);
        if (!game) return;
        const rect = target.getBoundingClientRect();
        openGameMenu({ x: rect.right - 4, y: rect.bottom + 4 }, game, {
          play: () => void launchGame(game),
          reveal: () => {
            const container = activeContainer();
            if (!container) return;
            void api
              .reveal(`${container.config.path}/${game.id}`)
              .catch((error) => toast("err", "Reveal failed", String(error)));
          },
          rename: async () => {
            const name = await promptDialog({
              title: "Rename app",
              subtitle: "Only the launcher entry is renamed — files stay untouched.",
              label: "Display name",
              value: game.name,
            });
            if (!name) return;
            await api.setGameOverride(state.activeId!, game.id, { name }).catch((error) => {
              toast("err", "Rename failed", String(error));
            });
            await refreshContainers(state.activeId);
          },
          hide: async () => {
            await api.setGameOverride(state.activeId!, game.id, { hidden: true }).catch((error) => {
              toast("err", "Could not hide app", String(error));
            });
            await refreshContainers(state.activeId);
            toast("ok", "Hidden from library", game.name);
          },
        });
        break;
      }
      case "container-menu":
        void handleContainerMenu({ x: event.clientX + 12, y: event.clientY + 8 });
        break;
      case "stop-run":
        void stopActiveRun();
        break;
      case "import-container":
        void importContainer();
        break;
      default:
        break;
    }
  });

  document.addEventListener("dblclick", (event) => {
    const card = (event.target as HTMLElement).closest<HTMLElement>(".game-card");
    if (!card) return;
    const game = state.games.find((item) => item.id === card.dataset.gameId);
    if (game) void launchGame(game);
  });

  document.addEventListener("keydown", (event) => {
    const card = (event.target as HTMLElement).closest<HTMLElement>(".game-card");
    if (card && event.key === "Enter") {
      const game = state.games.find((item) => item.id === card.dataset.gameId);
      if (game) {
        event.preventDefault();
        void launchGame(game);
      }
      return;
    }
    const meta = event.metaKey || event.ctrlKey;
    if (meta && event.key.toLowerCase() === "n") {
      event.preventDefault();
      void openNewContainer();
    }
    if (meta && event.key.toLowerCase() === "r") {
      event.preventDefault();
      void refreshContainers(state.activeId);
    }
  });

  const overlay = document.querySelector<HTMLElement>("[data-drop-overlay]");
  let dragDepth = 0;
  window.addEventListener("dragenter", (event) => {
    event.preventDefault();
    dragDepth++;
    overlay?.classList.add("visible");
  });
  window.addEventListener("dragover", (event) => event.preventDefault());
  window.addEventListener("dragleave", () => {
    dragDepth = Math.max(0, dragDepth - 1);
    if (dragDepth === 0) overlay?.classList.remove("visible");
  });
  window.addEventListener("drop", (event) => {
    event.preventDefault();
    dragDepth = 0;
    overlay?.classList.remove("visible");
    if (isTauri) return;
    const file = event.dataTransfer?.files?.[0];
    if (file) void startInstaller(file.name);
  });
}

async function bindBackendEvents(): Promise<void> {
  await onEvent<LogLine>("wine-log", (line) => {
    if (line.containerId !== state.activeId) return;
    panel.append(line);
  });

  await onEvent<{ runId: string; containerId: string; kind: string }>("run-started", (payload) => {
    state.runs.set(payload.containerId, payload.runId);
    if (payload.containerId === state.activeId) panel.setRunning(true);
    renderSidebar();
    renderTopbar();
  });

  await onEvent<RunExit>("run-exit", (payload) => {
    state.runs.delete(payload.containerId);
    if (payload.containerId === state.activeId) {
      panel.setRunning(false);
      panel.append({
        runId: payload.runId,
        containerId: payload.containerId,
        ts: Date.now(),
        stream: "system",
        level: payload.code === 0 || payload.code === null ? "success" : "error",
        text: payload.simulated
          ? `Simulated process exited with code ${payload.code ?? 0}.`
          : `Process exited with code ${payload.code ?? 0}.`,
      });
    }
    const container = state.containers.find((c) => c.config.id === payload.containerId);
    if (payload.kind === "installer" && container) {
      toast(
        "ok",
        payload.code === 0 ? "Installer finished" : "Installer exited",
        payload.code === 0 ? "Rescanning the container for new programs…" : `Exit code ${payload.code ?? "?"}`,
      );
    }
    scheduleRefresh();
  });

  await onEvent<EngineProgress>("engine-progress", (progress) => {
    if (progress.phase === "done") {
      void refreshEngines();
    }
  });

  await onEvent("containers-changed", () => scheduleRefresh());

  if (isTauri) {
    try {
      const { getCurrentWebview } = await import("@tauri-apps/api/webview");
      const webview = getCurrentWebview();
      await webview.onDragDropEvent((event) => {
        const payload = event.payload as { type: string; paths?: string[] };
        if (payload.type === "over" || payload.type === "enter") {
          overlayShow(true);
        } else if (payload.type === "drop") {
          overlayShow(false);
          const file = payload.paths?.find((path) => /\.(exe|msi|bat|cmd)$/i.test(path));
          if (file) void startInstaller(file);
          else if (payload.paths?.length) toast("warn", "Unsupported file", "Drop a .exe or .msi file.");
        } else {
          overlayShow(false);
        }
      });
    } catch {
      /* drag & drop is optional */
    }
  }
}

function overlayShow(show: boolean): void {
  document.querySelector("[data-drop-overlay]")?.classList.toggle("visible", show);
}

/* -------------------------------------------------------------------------- */
/* Boot                                                                       */
/* -------------------------------------------------------------------------- */

async function boot(): Promise<void> {
  app.innerHTML = `<div class="boot-screen">
    <div class="boot-mark">${logoMark(32)}</div>
    <div class="boot-title">WinMac Launcher</div>
    <div class="boot-sub">Detecting compatibility engines…</div>
  </div>`;

  try {
    state.platform = await api.platformInfo();
    state.engineState = await api.engines().catch(() => null);
    await refreshContainers(localStorage.getItem("winmac.activeContainer"));
    state.engines = await api.winetricksVerbs().catch(() => []);

    renderShell();
    bindUiEvents();
    await bindBackendEvents();

    if (state.activeId) await loadContainerData(state.activeId);
    renderSidebar();
    renderActiveView();

    const saved = localStorage.getItem("winmac.activeContainer");
    const chosen = state.containers.find((c) => c.config.id === saved) ?? state.containers[0];
    if (chosen && chosen.config.id !== state.activeId) {
      await selectContainer(chosen.config.id);
    }

    const boot1 = document.querySelector(".boot-screen");
    if (boot1) {
      boot1.classList.add("hide");
      window.setTimeout(() => boot1.remove(), 320);
    }
    state.loading = false;
  } catch (error) {
    app.innerHTML = `<div class="boot-screen">
      <div class="boot-mark" style="background:linear-gradient(145deg,#ff8a9a,#c04a63)">${icon("alert", 30)}</div>
      <div class="boot-title">Could not start the launcher</div>
      <div class="boot-sub" style="max-width:520px;text-align:center;line-height:1.5">${escapeHtml(String(error))}</div>
      <button class="btn primary" onclick="location.reload()">Retry</button>
    </div>`;
    console.error(error);
  }
}

void boot();
