import { api, onEvent, pickEngineBinary, pickFolder } from "./api";
import { icon } from "./icons";
import { toast } from "./toast";
import type {
  ContainerSummary,
  EngineInfo,
  EngineProgress,
  EngineState,
  Game,
  GraphicsBackend,
  WinetricksVerb,
  WindowsVersion,
} from "./types";
import { backendDescription, backendLabel, escapeHtml, formatBytes, shortPath } from "./ui";

/* -------------------------------------------------------------------------- */
/* Modal infrastructure                                                       */
/* -------------------------------------------------------------------------- */

interface ModalOptions {
  title: string;
  subtitle?: string;
  body: string;
  footer?: string;
  wide?: boolean;
  onClose?: () => void;
}

interface ModalHandle {
  root: HTMLDivElement;
  body: HTMLElement;
  footer: HTMLElement;
  close: () => void;
}

const modalStack: ModalHandle[] = [];
let escapeBound = false;

function bindEscape(): void {
  if (escapeBound) return;
  escapeBound = true;
  window.addEventListener("keydown", (event) => {
    if (event.key !== "Escape") return;
    const top = modalStack[modalStack.length - 1];
    if (top) {
      event.preventDefault();
      top.close();
    }
  });
}

function showModal(options: ModalOptions): ModalHandle {
  bindEscape();
  const root = document.createElement("div");
  root.className = "modal-backdrop";
  root.innerHTML = `
    <div class="modal ${options.wide ? "wide" : ""}" role="dialog" aria-modal="true">
      <div class="modal-head">
        <div>
          <h2>${escapeHtml(options.title)}</h2>
          ${options.subtitle ? `<p>${escapeHtml(options.subtitle)}</p>` : ""}
        </div>
        <button class="icon-btn close" data-modal-close aria-label="Close">${icon("x", 15)}</button>
      </div>
      <div class="modal-body"></div>
      <div class="modal-foot">${options.footer ?? ""}</div>
    </div>`;
  const body = root.querySelector<HTMLElement>(".modal-body")!;
  body.innerHTML = options.body;
  const footer = root.querySelector<HTMLElement>(".modal-foot")!;
  document.getElementById("modal-root")!.appendChild(root);

  const handle: ModalHandle = {
    root,
    body,
    footer,
    close: () => {
      root.remove();
      const index = modalStack.indexOf(handle);
      if (index >= 0) modalStack.splice(index, 1);
      options.onClose?.();
    },
  };
  modalStack.push(handle);
  root.querySelector("[data-modal-close]")?.addEventListener("click", () => handle.close());
  root.addEventListener("mousedown", (event) => {
    if (event.target === root) handle.close();
  });
  window.setTimeout(() => {
    const focusable = body.querySelector<HTMLElement>("input, select, textarea, button");
    focusable?.focus();
  }, 30);
  return handle;
}

function field(label: string, control: string, hint?: string): string {
  return `<div class="field">
    <label>${escapeHtml(label)}</label>
    ${control}
    ${hint ? `<div class="hint">${hint}</div>` : ""}
  </div>`;
}

function toggleRow(
  id: string,
  title: string,
  description: string,
  checked: boolean,
  disabled = false,
): string {
  return `<div class="toggle-row">
    <div class="toggle-text">
      <div class="toggle-title">${escapeHtml(title)}</div>
      <div class="toggle-desc">${description}</div>
    </div>
    <button type="button" class="switch ${checked ? "on" : ""}" data-toggle="${id}"
      ${disabled ? "disabled" : ""} aria-pressed="${checked}"></button>
  </div>`;
}

function bindToggles(root: HTMLElement): void {
  root.querySelectorAll<HTMLElement>("[data-toggle]").forEach((el) => {
    el.addEventListener("click", () => {
      if (el.hasAttribute("disabled")) return;
      el.classList.toggle("on");
      el.setAttribute("aria-pressed", String(el.classList.contains("on")));
    });
  });
}

function isOn(root: HTMLElement, id: string): boolean {
  return root.querySelector(`[data-toggle="${id}"]`)?.classList.contains("on") ?? false;
}

function bindSegmented(root: HTMLElement, group: string, onChange?: (value: string) => void): void {
  root.querySelectorAll<HTMLElement>(`[data-seg="${group}"] button`).forEach((button) => {
    button.addEventListener("click", () => {
      root.querySelectorAll(`[data-seg="${group}"] button`).forEach((b) => b.classList.remove("active"));
      button.classList.add("active");
      onChange?.(button.dataset.value ?? "");
    });
  });
}

function segValue(root: HTMLElement, group: string): string {
  const active = root.querySelector<HTMLElement>(`[data-seg="${group}"] button.active`);
  return active?.dataset.value ?? "";
}

function setSegmented(root: HTMLElement, group: string, value: string): void {
  root.querySelectorAll<HTMLElement>(`[data-seg="${group}"] button`).forEach((button) => {
    button.classList.toggle("active", button.dataset.value === value);
  });
}

/** Plain Wine builds have no D3DMetal shims; only GPTK/CrossOver do. */
function recommendedBackend(engine: EngineInfo | undefined): GraphicsBackend {
  return engine?.kind === "gptk" || engine?.kind === "crossover" ? "d3dmetal" : "wined3d";
}

function segControl(name: string, options: Array<[string, string]>, value: string): string {
  return `<div class="segmented" data-seg="${name}">
    ${options
      .map(
        ([val, label]) =>
          `<button type="button" data-value="${val}" class="${val === value ? "active" : ""}">${escapeHtml(label)}</button>`,
      )
      .join("")}
  </div>`;
}

/* -------------------------------------------------------------------------- */
/* Generic dialogs                                                            */
/* -------------------------------------------------------------------------- */

export function confirmDialog(options: {
  title: string;
  message: string;
  confirmLabel?: string;
  danger?: boolean;
}): Promise<boolean> {
  return new Promise((resolve) => {
    let settled = false;
    const finish = (value: boolean) => {
      if (settled) return;
      settled = true;
      resolve(value);
      handle.close();
    };
    const handle = showModal({
      title: options.title,
      body: `<p style="margin:0 0 4px;font-size:12.5px;line-height:1.55;color:var(--text-dim)">${escapeHtml(options.message)}</p>`,
      footer: `<span class="grow"></span>
        <button class="btn" data-cancel>Cancel</button>
        <button class="btn ${options.danger ? "danger" : "primary"}" data-confirm>${escapeHtml(options.confirmLabel ?? "Confirm")}</button>`,
      onClose: () => finish(false),
    });
    handle.footer.querySelector("[data-cancel]")?.addEventListener("click", () => finish(false));
    handle.footer.querySelector("[data-confirm]")?.addEventListener("click", () => finish(true));
  });
}

export function promptDialog(options: {
  title: string;
  subtitle?: string;
  label: string;
  value?: string;
  placeholder?: string;
  confirmLabel?: string;
}): Promise<string | null> {
  return new Promise((resolve) => {
    let settled = false;
    const finish = (value: string | null) => {
      if (settled) return;
      settled = true;
      resolve(value);
      handle.close();
    };
    const handle = showModal({
      title: options.title,
      subtitle: options.subtitle,
      body: field(
        options.label,
        `<input class="input" data-input value="${escapeHtml(options.value ?? "")}" placeholder="${escapeHtml(options.placeholder ?? "")}" />`,
      ),
      footer: `<span class="grow"></span>
        <button class="btn" data-cancel>Cancel</button>
        <button class="btn primary" data-confirm>${escapeHtml(options.confirmLabel ?? "Save")}</button>`,
      onClose: () => finish(null),
    });
    const input = handle.body.querySelector<HTMLInputElement>("[data-input]")!;
    const submit = () => {
      const value = input.value.trim();
      if (value) finish(value);
    };
    input.addEventListener("keydown", (event) => {
      if (event.key === "Enter") submit();
    });
    handle.footer.querySelector("[data-cancel]")?.addEventListener("click", () => finish(null));
    handle.footer.querySelector("[data-confirm]")?.addEventListener("click", submit);
  });
}

/* -------------------------------------------------------------------------- */
/* New container                                                              */
/* -------------------------------------------------------------------------- */

export async function openNewContainerModal(options: {
  engines: EngineState | null;
  defaultLocation: string;
  onCreated: (container: ContainerSummary) => void;
}): Promise<void> {
  const available = options.engines?.engines.filter((e) => e.available) ?? [];
  const defaultEngine = available.find((e) => e.selected) ?? available[0];
  const defaultBackend = recommendedBackend(defaultEngine);
  let recommended = defaultBackend;
  const handle = showModal({
    title: "New container",
    subtitle: "An isolated Windows prefix with its own C: drive, registry and game library.",
    body: `
      ${field(
        "Container name",
        `<input class="input" data-name placeholder="e.g. Steam Library" maxlength="48" />`,
        "Each container is a folder on disk. Keep one per launcher or per game for the best isolation.",
      )}
      ${field(
        "Location",
        `<div class="input-row">
          <input class="input" data-location value="${escapeHtml(options.defaultLocation)}" />
          <button class="btn" data-browse-location>Browse…</button>
        </div>`,
        "A sub-folder with the container name will be created inside this location.",
      )}
      ${field("Windows version", segControl("windows", [["win10", "Windows 10"], ["win11", "Windows 11"]], "win10"))}
      ${field(
        "Graphics backend",
        segControl(
          "backend",
          [
            ["d3dmetal", "D3DMetal"],
            ["dxvk", "DXVK"],
            ["wined3d", "WineD3D"],
          ],
          defaultBackend,
        ) + `<div class="hint" data-backend-hint>${backendDescription(defaultBackend)}</div>`,
      )}
      ${field(
        "Wine engine",
        `<select class="select" data-engine>
          <option value="">Auto — best available engine</option>
          ${available
            .map(
              (engine) =>
                `<option value="${escapeHtml(engine.id)}" ${engine.id === defaultEngine?.id ? "selected" : ""}>${escapeHtml(engine.name)}${engine.version ? ` · ${escapeHtml(engine.version)}` : ""}</option>`,
            )
            .join("")}
        </select>`,
        available.length === 0
          ? "No engine detected yet — you can create the container now and install an engine later."
          : undefined,
      )}
      <div style="margin-top:6px">
        ${toggleRow("esync", "ESync", "Eventfd-based synchronisation. Recommended for almost every game.", true)}
        ${toggleRow("msync", "MSync", "Mach semaphore sync — usually the biggest FPS win on Apple Silicon.", true)}
        ${toggleRow("hud", "Metal Performance HUD", "Overlay live FPS / frame time while the game is running.", false)}
        ${toggleRow("retina", "Retina mode", "Render at native panel resolution (crisper, heavier on the GPU).", true)}
      </div>`,
    footer: `<span class="grow"></span>
      <button class="btn" data-cancel>Cancel</button>
      <button class="btn primary" data-create>Create container</button>`,
    wide: true,
    onClose: () => undefined,
  });

  bindToggles(handle.body);
  bindSegmented(handle.body, "backend", (value) => {
    const hint = handle.body.querySelector("[data-backend-hint]");
    if (hint) hint.textContent = backendDescription(value);
  });

  const engineSelect = handle.body.querySelector<HTMLSelectElement>("[data-engine]");
  engineSelect?.addEventListener("change", () => {
    const engine = available.find((item) => item.id === engineSelect.value) ?? defaultEngine;
    const next = recommendedBackend(engine);
    // Only auto-switch while the user is still on the previous suggestion.
    if (segValue(handle.body, "backend") === recommended) {
      setSegmented(handle.body, "backend", next);
      const hint = handle.body.querySelector("[data-backend-hint]");
      if (hint) hint.textContent = backendDescription(next);
    }
    recommended = next;
  });

  handle.body.querySelector("[data-browse-location]")?.addEventListener("click", async () => {
    const folder = await pickFolder("Choose a location for containers");
    if (folder) {
      const input = handle.body.querySelector<HTMLInputElement>("[data-location]")!;
      input.value = folder;
    }
  });

  const create = async () => {
    const name = handle.body.querySelector<HTMLInputElement>("[data-name]")!.value.trim();
    if (!name) {
      toast("warn", "Name required", "Give the container a name to continue.");
      return;
    }
    const location = handle.body.querySelector<HTMLInputElement>("[data-location]")!.value.trim();
    const button = handle.footer.querySelector<HTMLButtonElement>("[data-create]")!;
    button.disabled = true;
    button.textContent = "Creating…";
    try {
      const created = await api.createContainer({
        name,
        location: location || null,
        windowsVersion: segValue(handle.body, "windows") as WindowsVersion,
        graphicsBackend: segValue(handle.body, "backend") as GraphicsBackend,
        engineId: handle.body.querySelector<HTMLSelectElement>("[data-engine]")!.value || null,
        esync: isOn(handle.body, "esync"),
        msync: isOn(handle.body, "msync"),
        metalHud: isOn(handle.body, "hud"),
        retina: isOn(handle.body, "retina"),
      });
      handle.close();
      options.onCreated(created);
      toast("ok", "Container created", created.config.path);
    } catch (error) {
      button.disabled = false;
      button.textContent = "Create container";
      toast("err", "Could not create container", String(error));
    }
  };

  handle.footer.querySelector("[data-cancel]")?.addEventListener("click", () => handle.close());
  handle.footer.querySelector("[data-create]")?.addEventListener("click", create);
  handle.body.querySelector("[data-name]")?.addEventListener("keydown", (event) => {
    if ((event as KeyboardEvent).key === "Enter") void create();
  });
}

/* -------------------------------------------------------------------------- */
/* Container settings                                                         */
/* -------------------------------------------------------------------------- */

export async function openSettingsModal(options: {
  container: ContainerSummary;
  engines: EngineState | null;
  onSaved: (container: ContainerSummary) => void;
  onDeleted: (id: string) => void;
}): Promise<void> {
  const config = options.container.config;
  const envText = Object.entries(config.extraEnv ?? {})
    .map(([key, value]) => `${key}=${value}`)
    .join("\n");
  const available = options.engines?.engines ?? [];

  const handle = showModal({
    title: "Container settings",
    subtitle: config.path,
    wide: true,
    body: `
      <div class="settings-tabs">
        <button class="active" data-tab="general">General</button>
        <button data-tab="performance">Performance</button>
        <button data-tab="advanced">Advanced</button>
      </div>
      <div data-pane="general">
        ${field("Name", `<input class="input" data-name value="${escapeHtml(config.name)}" maxlength="48" />`)}
        ${field(
          "Windows version",
          segControl("windows", [["win10", "Windows 10"], ["win11", "Windows 11"]], config.windowsVersion),
        )}
        ${field(
          "Graphics backend",
          segControl(
            "backend",
            [
              ["d3dmetal", "D3DMetal"],
              ["dxvk", "DXVK"],
              ["wined3d", "WineD3D"],
            ],
            config.graphicsBackend,
          ) + `<div class="hint" data-backend-hint>${backendDescription(config.graphicsBackend)}</div>`,
        )}
        ${field(
          "Wine engine",
          `<select class="select" data-engine>
            <option value="">Auto — best available engine</option>
            ${available
              .map(
                (engine) =>
                  `<option value="${escapeHtml(engine.id)}" ${engine.id === config.engineId ? "selected" : ""}>
                    ${escapeHtml(engine.name)}${engine.version ? ` · ${escapeHtml(engine.version)}` : ""}${engine.available ? "" : " (missing)"}
                  </option>`,
              )
              .join("")}
          </select>`,
        )}
        <div class="field">
          <label>Container folder</label>
          <div class="input-row">
            <input class="input" value="${escapeHtml(config.path)}" readonly />
            <button class="btn" data-reveal>Reveal</button>
          </div>
          <div class="hint">${formatBytes(options.container.sizeBytes)} on disk · ${options.container.gameCount} apps detected</div>
        </div>
      </div>
      <div data-pane="performance" style="display:none">
        ${toggleRow("esync", "ESync", "Eventfd-based synchronisation. Leave on unless a game misbehaves.", config.esync)}
        ${toggleRow("msync", "MSync", "Mach semaphore sync. The single biggest FPS improvement on Apple Silicon.", config.msync)}
        ${toggleRow("hud", "Metal Performance HUD", "Show Apple's Metal FPS overlay on top of the game window.", config.metalHud)}
        ${toggleRow("retina", "Retina mode", "Render at native resolution. Disable for better performance on 4K/5K displays.", config.retina)}
        ${field(
          "Extra launch arguments",
          `<input class="input" data-args value="${escapeHtml(config.launchArgs ?? "")}" placeholder="-windowed -novid" />`,
          "Appended to every launch inside this container.",
        )}
      </div>
      <div data-pane="advanced" style="display:none">
        ${field(
          "Environment variables",
          `<textarea class="input" data-env rows="5" style="height:auto;padding:9px 11px;font-family:var(--mono);font-size:12px;resize:vertical" placeholder="DXVK_HUD=fps&#10;WINE_LARGE_ADDRESS_AWARE=1">${escapeHtml(envText)}</textarea>`,
          "One KEY=VALUE pair per line. Applied on top of the managed compatibility variables.",
        )}
        <div class="divider">Danger zone</div>
        <div style="display:flex;gap:8px;flex-wrap:wrap">
          <button class="btn" data-clone>Clone container…</button>
          <button class="btn danger" data-delete>Delete container…</button>
        </div>
        <div class="hint" style="margin-top:8px">
          Deleting removes the container registration. You can keep the folder on disk to re-import it later.
        </div>
      </div>`,
    footer: `<span class="grow"></span>
      <button class="btn" data-cancel>Cancel</button>
      <button class="btn primary" data-save>Save changes</button>`,
  });

  bindToggles(handle.body);
  bindSegmented(handle.body, "backend", (value) => {
    const hint = handle.body.querySelector("[data-backend-hint]");
    if (hint) hint.textContent = backendDescription(value);
  });

  handle.body.querySelectorAll<HTMLElement>("[data-tab]").forEach((tab) => {
    tab.addEventListener("click", () => {
      handle.body.querySelectorAll("[data-tab]").forEach((t) => t.classList.remove("active"));
      tab.classList.add("active");
      handle.body.querySelectorAll<HTMLElement>("[data-pane]").forEach((pane) => {
        pane.style.display = pane.dataset.pane === tab.dataset.tab ? "" : "none";
      });
    });
  });

  handle.body.querySelector("[data-reveal]")?.addEventListener("click", () => {
    void api.reveal(config.path).catch((error) => toast("err", "Could not open folder", String(error)));
  });

  const save = async () => {
    const extraEnv: Record<string, string> = {};
    const rawEnv = handle.body.querySelector<HTMLTextAreaElement>("[data-env]")?.value ?? "";
    rawEnv.split(/\r?\n/).forEach((line) => {
      const trimmed = line.trim();
      if (!trimmed || trimmed.startsWith("#")) return;
      const eq = trimmed.indexOf("=");
      if (eq <= 0) return;
      extraEnv[trimmed.slice(0, eq).trim()] = trimmed.slice(eq + 1).trim();
    });
    const name = handle.body.querySelector<HTMLInputElement>("[data-name]")!.value.trim() || config.name;
    const button = handle.footer.querySelector<HTMLButtonElement>("[data-save]")!;
    button.disabled = true;
    button.textContent = "Saving…";
    try {
      const updated = await api.updateContainer({
        id: config.id,
        name,
        windowsVersion: segValue(handle.body, "windows") as WindowsVersion,
        graphicsBackend: segValue(handle.body, "backend") as GraphicsBackend,
        esync: isOn(handle.body, "esync"),
        msync: isOn(handle.body, "msync"),
        metalHud: isOn(handle.body, "hud"),
        retina: isOn(handle.body, "retina"),
        engineId: handle.body.querySelector<HTMLSelectElement>("[data-engine]")!.value || null,
        launchArgs: handle.body.querySelector<HTMLInputElement>("[data-args]")?.value ?? "",
        extraEnv,
      });
      handle.close();
      options.onSaved(updated);
      toast("ok", "Settings saved", `${updated.config.name} · ${backendLabel(updated.config.graphicsBackend)}`);
    } catch (error) {
      button.disabled = false;
      button.textContent = "Save changes";
      toast("err", "Could not save settings", String(error));
    }
  };

  handle.footer.querySelector("[data-cancel]")?.addEventListener("click", () => handle.close());
  handle.footer.querySelector("[data-save]")?.addEventListener("click", save);

  handle.body.querySelector("[data-delete]")?.addEventListener("click", async () => {
    const shouldDeleteFiles = await confirmDialog({
      title: "Delete container?",
      message: `“${config.name}” will be removed from the launcher. Choose OK to also delete the folder and all installed games, or Cancel to keep the files.`,
      confirmLabel: "Delete container",
      danger: true,
    });
    if (!shouldDeleteFiles) return;
    try {
      await api.deleteContainer(config.id, true);
      handle.close();
      options.onDeleted(config.id);
      toast("ok", "Container deleted", config.path);
    } catch (error) {
      toast("err", "Could not delete container", String(error));
    }
  });

  handle.body.querySelector("[data-clone]")?.addEventListener("click", async () => {
    const name = await promptDialog({
      title: "Clone container",
      subtitle: "Copies drive_c, registry and configuration.",
      label: "New container name",
      value: `${config.name} Copy`,
      confirmLabel: "Clone",
    });
    if (!name) return;
    try {
      const clone = await api.cloneContainer(config.id, name, null);
      handle.close();
      options.onSaved(clone);
      toast("ok", "Container cloned", clone.config.path);
    } catch (error) {
      toast("err", "Could not clone container", String(error));
    }
  });
}

/* -------------------------------------------------------------------------- */
/* Engine manager                                                             */
/* -------------------------------------------------------------------------- */

export async function openEngineManagerModal(options: {
  engines: EngineState | null;
  onChanged: () => Promise<void> | void;
}): Promise<void> {
  let state = options.engines ?? (await api.engines().catch(() => null));
  const activeDownloads = new Map<string, EngineProgress>();

  const handle = showModal({
    title: "Compatibility engines",
    subtitle: "Wine builds, Apple Game Porting Toolkit and CrossOver runtimes available on this Mac.",
    wide: true,
    body: `<div data-engine-content></div>`,
    footer: `<button class="btn" data-locate>${icon("folder", 14)} Locate wine binary…</button>
      <span class="grow"></span>
      <button class="btn" data-done>Done</button>`,
  });

  const content = handle.body.querySelector<HTMLElement>("[data-engine-content]")!;
  let unlisten: (() => void) | null = null;

  const render = () => {
    if (!state) {
      content.innerHTML = `<div class="empty-state"><h3>Engine list unavailable</h3><p>${escapeHtml("The engine manager could not be initialised.")}</p></div>`;
      return;
    }
    content.innerHTML = `
      <div class="engine-list">
        ${
          state.engines.length === 0
            ? `<div class="empty-state" style="padding:26px">
                <div class="empty-icon">${icon("wine", 20)}</div>
                <h3>No engine detected</h3>
                <p>Install Wine or Apple GPTK, or download a pre-built engine below.</p>
              </div>`
            : state.engines
                .map((engine) => {
                  const progress = activeDownloads.get(engine.id);
                  return `<div class="engine-card ${engine.selected ? "selected" : ""}">
                    <div class="e-icon">${icon(engine.kind === "gptk" ? "sparkles" : engine.kind === "crossover" ? "shield" : "wine", 17)}</div>
                    <div class="e-body">
                      <div class="e-name">
                        ${escapeHtml(engine.name)}
                        <span class="status-dot ${engine.available ? "ok" : "err"}"></span>
                        ${engine.managed ? '<span class="chip">managed</span>' : ""}
                        ${engine.selected ? '<span class="chip accent">default</span>' : ""}
                      </div>
                      <div class="e-sub" title="${escapeHtml(engine.winePath)}">${escapeHtml(engine.version ?? "unknown version")} · ${escapeHtml(shortPath(engine.winePath, 52))}</div>
                      ${
                        progress
                          ? `<div class="download-progress">
                              <div class="progress-track"><div class="progress-fill" style="width:${
                                progress.total > 0 ? Math.min(100, Math.round((progress.downloaded / progress.total) * 100)) : 8
                              }%"></div></div>
                              <div class="progress-label"><span>${escapeHtml(progress.phase)}</span><span>${
                                progress.total > 0 ? `${formatBytes(progress.downloaded)} / ${formatBytes(progress.total)}` : "…"
                              }</span></div>
                            </div>`
                          : ""
                      }
                    </div>
                    <div class="e-actions">
                      ${
                        engine.available
                          ? `<button class="btn small" data-select-engine="${escapeHtml(engine.id)}" ${engine.selected ? "disabled" : ""}>Use as default</button>
                             <button class="btn small" data-reveal-engine="${escapeHtml(engine.sourcePath)}" title="Reveal in Finder">${icon("folder", 13)}</button>
                             ${engine.managed ? `<button class="btn small danger" data-remove-engine="${escapeHtml(engine.id)}" title="Remove">${icon("trash", 13)}</button>` : ""}`
                          : `<span class="chip warn">not installed</span>`
                      }
                    </div>
                  </div>`;
                })
                .join("")
        }
      </div>
      <div class="divider">Downloadable engines</div>
      <div class="engine-list">
        ${state.catalog
          .map((item) => {
            const progress = activeDownloads.get(item.id);
            return `<div class="engine-card">
              <div class="e-icon">${icon("package", 17)}</div>
              <div class="e-body">
                <div class="e-name">${escapeHtml(item.name)}${item.recommended ? '<span class="chip accent">recommended</span>' : ""}</div>
                <div class="e-sub">${escapeHtml(item.description)}</div>
                ${
                  progress
                    ? `<div class="download-progress">
                        <div class="progress-track"><div class="progress-fill" style="width:${
                          progress.total > 0 ? Math.min(100, Math.round((progress.downloaded / progress.total) * 100)) : 8
                        }%"></div></div>
                        <div class="progress-label"><span>${escapeHtml(progress.phase)}</span><span>${
                          progress.total > 0 ? `${formatBytes(progress.downloaded)} / ${formatBytes(progress.total)}` : "…"
                        }</span></div>
                      </div>`
                    : `<div class="e-sub" style="margin-top:4px;font-family:var(--font);color:#687184">${item.approxSizeMb > 0 ? `~${item.approxSizeMb} MB` : "Requires an existing installation"}${item.url ? ` · ${escapeHtml(item.url.replace(/^https?:\/\//, "").slice(0, 46))}` : ""}</div>`
                }
              </div>
              <div class="e-actions">
                ${
                  progress && progress.phase !== "done" && progress.phase !== "error"
                    ? `<button class="btn small" data-cancel-engine="${escapeHtml(item.id)}">Cancel</button>`
                    : item.url
                      ? `<button class="btn small primary" data-download-engine="${escapeHtml(item.id)}">Download</button>`
                      : `<span class="chip">manual</span>`
                }
              </div>
            </div>`;
          })
          .join("")}
      </div>
      <div class="divider">Manual install</div>
      ${field(
        "Download from URL",
        `<div class="input-row">
          <input class="input" data-url placeholder="https://…/wine-build.tar.gz" />
          <button class="btn" data-download-url>Download</button>
        </div>`,
        "Supports .tar.gz and .zip archives containing bin/wine64 or bin/wine.",
      )}
      <div class="toggle-row" style="border-top:1px solid rgba(255,255,255,.055)">
        <div class="toggle-text">
          <div class="toggle-title">winetricks</div>
          <div class="toggle-desc">${
            state.winetricksInstalled
              ? "Installed — required for game dependencies (VC++ runtimes, .NET, DX9)."
              : "Not installed yet. Install it to run dependency scripts inside containers."
          }</div>
        </div>
        ${
          state.winetricksInstalled
            ? '<span class="chip ok">ready</span>'
            : `<button class="btn small primary" data-install-winetricks>Install</button>`
        }
      </div>
      <div class="hint" style="margin-top:10px;font-family:var(--mono);font-size:10px;word-break:break-all">Engines folder: ${escapeHtml(state.engineDir)}</div>`;
    bindEngineActions();
  };

  const bindEngineActions = () => {
    content.querySelectorAll<HTMLElement>("[data-select-engine]").forEach((el) => {
      el.addEventListener("click", async () => {
        try {
          state = await api.setSelectedEngine(el.dataset.selectEngine!);
          render();
          await options.onChanged();
        } catch (error) {
          toast("err", "Could not select engine", String(error));
        }
      });
    });
    content.querySelectorAll<HTMLElement>("[data-reveal-engine]").forEach((el) => {
      el.addEventListener("click", () => {
        void api.reveal(el.dataset.revealEngine!).catch(() => undefined);
      });
    });
    content.querySelectorAll<HTMLElement>("[data-remove-engine]").forEach((el) => {
      el.addEventListener("click", async () => {
        const ok = await confirmDialog({
          title: "Remove managed engine?",
          message: "The engine files will be deleted from the launcher engine folder.",
          confirmLabel: "Remove",
          danger: true,
        });
        if (!ok) return;
        try {
          state = await api.removeEngine(el.dataset.removeEngine!);
          render();
          await options.onChanged();
        } catch (error) {
          toast("err", "Could not remove engine", String(error));
        }
      });
    });
    content.querySelectorAll<HTMLElement>("[data-download-engine]").forEach((el) => {
      el.addEventListener("click", async () => {
        const id = el.dataset.downloadEngine!;
        const item = state?.catalog.find((c) => c.id === id);
        if (!item) return;
        await startDownload(id, item.url, item.name, item.kind);
      });
    });
    content.querySelectorAll<HTMLElement>("[data-cancel-engine]").forEach((el) => {
      el.addEventListener("click", async () => {
        await api.cancelEngineDownload(el.dataset.cancelEngine!).catch(() => undefined);
      });
    });
    content.querySelector("[data-download-url]")?.addEventListener("click", async () => {
      const input = content.querySelector<HTMLInputElement>("[data-url]");
      const url = input?.value.trim();
      if (!url) {
        toast("warn", "URL required", "Paste a direct link to a .tar.gz or .zip archive.");
        return;
      }
      const name = url.split("/").pop()?.replace(/\.(tar\.gz|tgz|zip)$/i, "") ?? "Custom engine";
      await startDownload(`custom-${Date.now().toString(36)}`, url, name, "custom");
    });
    content.querySelector("[data-install-winetricks]")?.addEventListener("click", async () => {
      try {
        await api.installWinetricks();
        state = await api.engines();
        render();
        toast("ok", "winetricks installed");
        await options.onChanged();
      } catch (error) {
        toast("err", "Could not install winetricks", String(error));
      }
    });
  };

  const startDownload = async (id: string, url: string, name: string, kind: string) => {
    activeDownloads.set(id, { id, phase: "downloading", downloaded: 0, total: 0, message: "Starting…" });
    render();
    try {
      await api.downloadEngine(id, url, name, kind);
      state = await api.engines();
      activeDownloads.delete(id);
      render();
      toast("ok", "Engine installed", name);
      await options.onChanged();
    } catch (error) {
      activeDownloads.delete(id);
      render();
      toast("err", "Engine download failed", String(error));
    }
  };

  render();
  unlisten = await onEvent<EngineProgress>("engine-progress", (progress) => {
    if (progress.phase === "done" || progress.phase === "error" || progress.phase === "cancelled") {
      activeDownloads.delete(progress.id);
    } else {
      activeDownloads.set(progress.id, progress);
    }
    if (!progress.id.startsWith("custom-")) render();
  });

  handle.footer.querySelector("[data-done]")?.addEventListener("click", () => handle.close());
  handle.footer.querySelector("[data-locate]")?.addEventListener("click", async () => {
    const path = await pickEngineBinary();
    if (!path) return;
    try {
      state = await api.addEngineManual(path);
      render();
      toast("ok", "Engine added", path);
      await options.onChanged();
    } catch (error) {
      toast("err", "Not a valid wine binary", String(error));
    }
  });

  const originalClose = handle.close;
  handle.close = () => {
    unlisten?.();
    originalClose();
  };
}

/* -------------------------------------------------------------------------- */
/* Winetricks                                                                 */
/* -------------------------------------------------------------------------- */

export async function openWinetricksModal(options: {
  container: ContainerSummary;
  verbs: WinetricksVerb[];
  onRun: (verbs: string[]) => Promise<void>;
}): Promise<void> {
  const selected = new Set<string>(["vcrun2022"]);
  const categories = [...new Set(options.verbs.map((v) => v.category))];
  const handle = showModal({
    title: "Install dependencies",
    subtitle: `Runs winetricks inside “${options.container.config.name}”. The game must be closed while dependencies install.`,
    wide: true,
    body: categories
      .map(
        (category) => `<div class="field">
          <label>${escapeHtml(category)}</label>
          <div class="verb-list">
            ${options.verbs
              .filter((verb) => verb.category === category)
              .map(
                (verb) => `<button class="verb-item ${selected.has(verb.verb) ? "checked" : ""}" data-verb="${verb.verb}">
                  <span class="checkbox">${icon("check", 10)}</span>
                  <span>
                    <span class="v-title">${escapeHtml(verb.label)}</span>
                    <span class="v-desc">${escapeHtml(verb.description)}</span>
                  </span>
                </button>`,
              )
              .join("")}
          </div>
        </div>`,
      )
      .join(""),
    footer: `<span class="grow"></span>
      <button class="btn" data-cancel>Cancel</button>
      <button class="btn primary" data-run>Install selected</button>`,
  });

  handle.body.querySelectorAll<HTMLElement>("[data-verb]").forEach((item) => {
    item.addEventListener("click", () => {
      const verb = item.dataset.verb!;
      if (selected.has(verb)) selected.delete(verb);
      else selected.add(verb);
      item.classList.toggle("checked", selected.has(verb));
      updateLabel();
    });
  });

  const updateLabel = () => {
    const button = handle.footer.querySelector<HTMLElement>("[data-run]")!;
    button.textContent = selected.size > 0 ? `Install ${selected.size} selected` : "Install selected";
  };

  handle.footer.querySelector("[data-cancel]")?.addEventListener("click", () => handle.close());
  handle.footer.querySelector("[data-run]")?.addEventListener("click", async () => {
    if (selected.size === 0) {
      toast("warn", "Nothing selected", "Pick at least one dependency.");
      return;
    }
    const button = handle.footer.querySelector<HTMLButtonElement>("[data-run]")!;
    button.disabled = true;
    button.textContent = "Starting…";
    try {
      handle.close();
      await options.onRun([...selected]);
    } catch (error) {
      handle.close();
      toast("err", "winetricks failed", String(error));
    }
  });
  updateLabel();
}

/* -------------------------------------------------------------------------- */
/* Context menus                                                              */
/* -------------------------------------------------------------------------- */

interface MenuItem {
  label: string;
  icon: string;
  action: () => void;
  danger?: boolean;
}

export type MenuEntry = MenuItem | "separator";

function openContextMenu(anchor: { x: number; y: number }, items: MenuEntry[]): void {
  document.querySelectorAll(".context-menu").forEach((menu) => menu.remove());
  const menu = document.createElement("div");
  menu.className = "context-menu";
  menu.innerHTML = items
    .map((item) =>
      item === "separator"
        ? '<div class="menu-sep"></div>'
        : `<button class="${item.danger ? "danger" : ""}" data-menu-item>
             ${icon(item.icon, 14)}<span>${escapeHtml(item.label)}</span>
           </button>`,
    )
    .join("");
  document.body.appendChild(menu);

  const width = menu.offsetWidth;
  const height = menu.offsetHeight;
  const x = Math.min(anchor.x, window.innerWidth - width - 10);
  const y = Math.min(anchor.y, window.innerHeight - height - 10);
  menu.style.left = `${Math.max(8, x)}px`;
  menu.style.top = `${Math.max(8, y)}px`;

  const clickable = items.filter((item): item is MenuItem => item !== "separator");
  const buttons = [...menu.querySelectorAll<HTMLButtonElement>("[data-menu-item]")];
  buttons.forEach((button, index) => {
    button.addEventListener("click", () => {
      menu.remove();
      clickable[index]?.action();
    });
  });

  const dismiss = (event: Event) => {
    if (!menu.contains(event.target as Node)) {
      menu.remove();
      window.removeEventListener("mousedown", dismiss);
      window.removeEventListener("keydown", onKey, true);
    }
  };
  const onKey = (event: KeyboardEvent) => {
    if (event.key === "Escape") {
      menu.remove();
      window.removeEventListener("mousedown", dismiss);
      window.removeEventListener("keydown", onKey, true);
    }
  };
  window.setTimeout(() => {
    window.addEventListener("mousedown", dismiss);
    window.addEventListener("keydown", onKey, true);
  }, 0);
}

export function openGameMenu(
  anchor: { x: number; y: number },
  game: Game,
  actions: {
    play: () => void;
    reveal: () => void;
    rename: () => void;
    hide: () => void;
  },
): void {
  openContextMenu(anchor, [
    { label: "Play", icon: "play", action: actions.play },
    { label: "Reveal in Finder", icon: "folder", action: actions.reveal },
    "separator",
    { label: "Rename in launcher…", icon: "pencil", action: actions.rename },
    { label: `Hide “${game.name}”`, icon: "eyeOff", action: actions.hide },
  ]);
}

export function openContainerMenu(
  anchor: { x: number; y: number },
  container: ContainerSummary,
  actions: {
    settings: () => void;
    reveal: () => void;
    clone: () => void;
    rescan: () => void;
    delete: () => void;
  },
): void {
  openContextMenu(anchor, [
    { label: "Container settings", icon: "sliders", action: actions.settings },
    { label: "Reveal in Finder", icon: "folder", action: actions.reveal },
    { label: "Clone container…", icon: "copy", action: actions.clone },
    { label: "Rescan for apps", icon: "refresh", action: actions.rescan },
    "separator",
    { label: `Delete “${container.config.name}”…`, icon: "trash", action: actions.delete, danger: true },
  ]);
}
