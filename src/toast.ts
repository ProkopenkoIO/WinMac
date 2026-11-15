import { icon } from "./icons";
import { escapeHtml } from "./ui";

export type ToastKind = "info" | "ok" | "err" | "warn";

export function toast(kind: ToastKind, title: string, message?: string): void {
  const root = document.getElementById("toast-root");
  if (!root) return;
  const el = document.createElement("div");
  el.className = `toast ${kind}`;
  const iconName = kind === "ok" ? "check" : kind === "err" ? "alert" : kind === "warn" ? "alert" : "info";
  el.innerHTML = `
    <span class="t-icon">${icon(iconName, 17)}</span>
    <div class="t-body">
      <div class="t-title">${escapeHtml(title)}</div>
      ${message ? `<div>${escapeHtml(message)}</div>` : ""}
    </div>`;
  root.appendChild(el);
  window.setTimeout(() => {
    el.classList.add("leaving");
    window.setTimeout(() => el.remove(), 220);
  }, kind === "err" ? 6500 : 3800);
}
