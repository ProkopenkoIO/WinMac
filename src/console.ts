import { icon } from "./icons";
import type { LogLine } from "./types";
import { escapeHtml } from "./ui";

interface ConsoleCallbacks {
  onClear: () => void;
  onStop: () => void;
}

/**
 * Live Wine output panel. Keeps every line in memory (bounded) and applies the
 * search filter without re-rendering the whole log, so it stays smooth while a
 * game dumps thousands of fixme lines.
 */
export class ConsolePanel {
  readonly element: HTMLElement;
  private logView: HTMLElement;
  private meta: HTMLElement;
  private filterInput: HTMLInputElement;
  private stopButton: HTMLButtonElement;
  private collapseButton: HTMLButtonElement;
  private lines: LogLine[] = [];
  private filter = "";
  private collapsed = false;
  private expanded = false;

  constructor(host: HTMLElement, private callbacks: ConsoleCallbacks) {
    const section = document.createElement("section");
    section.className = "console";
    section.innerHTML = `
      <div class="console-head">
        <div class="console-title">${icon("terminal", 13)} Output <span class="console-meta" data-meta></span></div>
        <div class="console-actions" style="margin-left:auto">
          <input class="console-filter" data-filter placeholder="Filter output…" spellcheck="false" />
          <button class="icon-btn" data-action="clear-logs" title="Clear log">${icon("trash", 14)}</button>
          <button class="icon-btn" data-expand title="Toggle height">${icon("chevronDown", 14)}</button>
          <button class="icon-btn" data-collapse title="Collapse">${icon("x", 14)}</button>
          <button class="btn small danger" data-stop style="display:none">${icon("stop", 11)} Stop</button>
        </div>
      </div>
      <div class="console-body">
        <div class="log-view" data-log-view></div>
      </div>`;
    host.appendChild(section);
    this.element = section;
    this.logView = section.querySelector<HTMLElement>("[data-log-view]")!;
    this.meta = section.querySelector<HTMLElement>("[data-meta]")!;
    this.filterInput = section.querySelector<HTMLInputElement>("[data-filter]")!;
    this.stopButton = section.querySelector<HTMLButtonElement>("[data-stop]")!;
    this.collapseButton = section.querySelector<HTMLButtonElement>("[data-collapse]")!;

    section.querySelector("[data-collapse]")?.addEventListener("click", () => this.setCollapsed(!this.collapsed));
    section.querySelector("[data-expand]")?.addEventListener("click", () => {
      this.expanded = !this.expanded;
      section.classList.toggle("expanded", this.expanded);
    });
    section.querySelector("[data-action='clear-logs']")?.addEventListener("click", () => {
      this.lines = [];
      this.renderAll();
      this.callbacks.onClear();
    });
    this.stopButton.addEventListener("click", () => this.callbacks.onStop());
    this.filterInput.addEventListener("input", () => {
      this.filter = this.filterInput.value.trim().toLowerCase();
      this.applyFilter();
    });
    this.renderEmpty();
    this.updateMeta();
    this.setCollapsed(true);
  }

  setCollapsed(collapsed: boolean): void {
    this.collapsed = collapsed;
    this.element.classList.toggle("collapsed", collapsed);
    this.collapseButton.innerHTML = icon(collapsed ? "chevronDown" : "x", 14);
    this.collapseButton.title = collapsed ? "Expand output" : "Collapse output";
    if (!collapsed) {
      this.scrollToEnd();
    }
    this.updateMeta();
  }

  setLogs(lines: LogLine[]): void {
    this.lines = lines.slice(-1500);
    this.renderAll();
    // Keep the panel compact until there is something worth showing.
    this.setCollapsed(this.lines.length === 0);
  }

  append(line: LogLine): void {
    if (this.collapsed) this.setCollapsed(false);
    this.lines.push(line);
    if (this.lines.length > 1500) this.lines.splice(0, this.lines.length - 1500);
    const empty = this.logView.querySelector(".log-empty");
    if (empty) empty.remove();
    const row = this.renderLine(line);
    this.logView.appendChild(row);
    while (this.logView.childElementCount > 1200) {
      this.logView.firstElementChild?.remove();
    }
    if (this.filter && !line.text.toLowerCase().includes(this.filter)) {
      row.style.display = "none";
    }
    this.trimScroll();
    this.updateMeta();
  }

  setRunning(running: boolean): void {
    this.stopButton.style.display = running ? "" : "none";
    if (running && this.collapsed) {
      // Surface activity when a process starts while the panel is collapsed.
      this.setCollapsed(false);
    }
    this.updateMeta();
  }

  private renderAll(): void {
    if (this.lines.length === 0) {
      this.renderEmpty();
    } else {
      const fragment = document.createDocumentFragment();
      for (const line of this.lines) {
        fragment.appendChild(this.renderLine(line));
      }
      this.logView.replaceChildren(fragment);
      this.applyFilter();
      this.scrollToEnd();
    }
    this.updateMeta();
  }

  private renderEmpty(): void {
    this.logView.innerHTML = `<div class="log-empty">${icon("terminal", 15)} Wine output will appear here when you launch a program.</div>`;
  }

  private renderLine(line: LogLine): HTMLElement {
    const row = document.createElement("div");
    const level = line.level === "error" ? "error" : line.level === "warn" ? "warn" : line.stream === "stderr" ? "stderr" : line.level;
    row.className = `log-line ${level}`;
    const time = new Date(line.ts).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
    const tag = line.level === "system" ? "wine" : line.stream === "stderr" ? "stderr" : "stdout";
    row.dataset.text = line.text.toLowerCase();
    row.innerHTML = `<span class="ts">${time}</span><span class="tag">${tag}</span><span class="msg">${escapeHtml(line.text)}</span>`;
    return row;
  }

  private applyFilter(): void {
    this.logView.querySelectorAll<HTMLElement>(".log-line").forEach((row) => {
      row.style.display = !this.filter || (row.dataset.text ?? "").includes(this.filter) ? "" : "none";
    });
  }

  private scrollToEnd(): void {
    this.logView.scrollTop = this.logView.scrollHeight;
  }

  private trimScroll(): void {
    const nearBottom = this.logView.scrollHeight - this.logView.scrollTop - this.logView.clientHeight < 90;
    if (nearBottom) this.scrollToEnd();
  }

  private updateMeta(): void {
    if (this.collapsed) {
      this.meta.textContent = this.lines.length > 0 ? `${this.lines.length} lines` : "no output";
      return;
    }
    const errors = this.lines.filter((line) => line.level === "error").length;
    this.meta.innerHTML = `${this.lines.length} lines${errors > 0 ? ` · <span style="color:var(--err)">${errors} errors</span>` : ""}`;
  }
}
