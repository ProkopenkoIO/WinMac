export function escapeHtml(value: unknown): string {
  return String(value ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

export function hashString(value: string): number {
  let hash = 2166136261;
  for (let i = 0; i < value.length; i++) {
    hash ^= value.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  return hash >>> 0;
}

export function monogram(name: string): string {
  const words = name
    .replace(/[^\p{L}\p{N} ]+/gu, " ")
    .split(/\s+/)
    .filter(Boolean);
  if (words.length === 0) return "?";
  if (words.length === 1) return words[0].slice(0, 2).toUpperCase();
  return (words[0][0] + words[1][0]).toUpperCase();
}

export function gradientFor(seed: string): string {
  const hash = hashString(seed);
  const hue = hash % 360;
  const hue2 = (hue + 38 + ((hash >> 9) % 40)) % 360;
  const hue3 = (hue + 320) % 360;
  return `linear-gradient(135deg, hsl(${hue} 62% 38%) 0%, hsl(${hue2} 72% 50%) 52%, hsl(${hue3} 55% 26%) 100%)`;
}

export function avatarGradient(seed: string): string {
  const hash = hashString(seed);
  const hue = hash % 360;
  return `linear-gradient(145deg, hsl(${hue} 70% 58%), hsl(${(hue + 42) % 360} 66% 42%))`;
}

export function formatBytes(bytes: number): string {
  if (!bytes || bytes <= 0) return "—";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  const digits = value >= 100 || unit <= 1 ? 0 : 1;
  return `${value.toFixed(digits)} ${units[unit]}`;
}

export function relativeTime(iso?: string | null): string {
  if (!iso) return "never";
  const ts = Date.parse(iso);
  if (Number.isNaN(ts)) return "never";
  const diff = Date.now() - ts;
  if (diff < 60_000) return "just now";
  const mins = Math.floor(diff / 60_000);
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days}d ago`;
  return new Date(ts).toLocaleDateString();
}

export function backendLabel(backend: string): string {
  switch (backend) {
    case "d3dmetal":
      return "D3DMetal";
    case "dxvk":
      return "DXVK";
    case "wined3d":
      return "WineD3D";
    default:
      return backend;
  }
}

export function backendDescription(backend: string): string {
  switch (backend) {
    case "d3dmetal":
      return "DirectX 11/12 → Metal via Apple GPTK. Best for modern AAA games on Apple Silicon.";
    case "dxvk":
      return "DirectX 9/10/11 → Vulkan → Metal. Great compatibility for slightly older titles.";
    case "wined3d":
      return "Wine's built-in OpenGL translation. Slowest, but useful for troubleshooting.";
    default:
      return "";
  }
}

export function windowsLabel(version: string): string {
  return version === "win11" ? "Windows 11" : "Windows 10";
}

export function shortPath(path: string, max = 46): string {
  if (!path || path.length <= max) return path ?? "";
  const parts = path.split(/[\\/]/);
  if (parts.length <= 2) return `…${path.slice(-max)}`;
  const head = parts[0];
  const tail = parts.slice(-2).join(path.includes("\\") ? "\\" : "/");
  const result = `${head}${path.includes("\\") ? "\\" : "/"}…\\${tail}`;
  return result.length > max ? `…${path.slice(-max)}` : result;
}

export function debounce<T extends (...args: never[]) => void>(fn: T, ms: number): T {
  let timer: number | undefined;
  return ((...args: never[]) => {
    if (timer) window.clearTimeout(timer);
    timer = window.setTimeout(() => fn(...args), ms);
  }) as T;
}
