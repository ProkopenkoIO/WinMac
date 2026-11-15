# WinMac Launcher

A modern, dark, macOS-native looking launcher that runs Windows games and applications on Apple Silicon and Intel Macs through **Wine**, **Apple Game Porting Toolkit (D3DMetal)**, **DXVK** and **MoltenVK**.

Every game lives in its own isolated *container* (a Wine prefix stored as a plain folder), so DirectX versions, Windows builds, DLL overrides and dependency sets never collide.

```
┌───────────────────────────────────────────────────────────────┐
│  Containers          │  Steam Library                          │
│  ● Steam Library     │  ┌──────────┐ ┌──────────┐ ┌──────────┐ │
│  ● GOG Classics      │  │ Cyberpunk│ │ Elden    │ │ Baldur's │ │
│  + New container     │  │  2077    │ │  Ring    │ │  Gate 3  │ │
│                      │  └──────────┘ └──────────┘ └──────────┘ │
│  Quick actions       │  Live terminal  ▸ wine64 …             │
└───────────────────────────────────────────────────────────────┘
```

---

## Features

- **Containers (bottles)** — create, clone, import, delete or reveal a prefix. Stored as ordinary folders in `~/Games/WinContainers/<name>`; every setting lives in a human-readable `container.json`.
- **Per-container configuration** — Windows 10 / 11 registry version, graphics backend, ESync, MSync, Metal HUD, Retina (HiDPI) mode, extra environment variables and launch arguments.
- **Engine manager** — detects Wine builds already installed on the Mac (GPTK, Homebrew Wine, CrossOver, Whisky), downloads managed Wine archives from GitHub releases, or registers any `wine64` binary manually. Download progress and cancellation are streamed to the UI.
- **Games** — automatic EXE scanning under `drive_c/Program Files` and `drive_c/Games`, plus drag-and-drop of loose `.exe` / `.msi` files. Cards show size, backend and last-played date.
- **Installers** — `.msi` files are routed through `msiexec`, `.exe` installers launch normally inside the selected container.
- **Quick actions** — Run `.exe`, Open C:, Container settings, Winetricks.
- **Winetricks** — one-click installation of VC++ runtimes, DirectX 9/10/11, .NET, XNA, PhysX and other common dependencies.
- **Live terminal** — stdout/stderr of the running program is streamed line by line with severity colouring, and the launch can be stopped (Wine process group is killed and `wineserver -k` is issued).
- **Demo mode** — on Windows, Linux or inside a plain browser the UI runs against a faithful mock backend: containers, configs and EXE scanning are real, while engine detection and launches are simulated. Perfect for UI work without a Mac.

---

## Requirements

| | |
|---|---|
| **Runtime** | macOS 12+ (macOS 14+ and Apple Silicon strongly recommended for D3DMetal / GPTK 2) |
| **Toolchain (dev)** | Node.js 20+, Rust 1.77+, Xcode Command Line Tools |
| **Optional** | A free Apple Developer account to install the Game Porting Toolkit evaluation environment |
| **Windows (UI only)** | Node.js 20+, WebView2 (bundled with Windows 11). Rust + MSVC Build Tools are only needed for `npm run tauri:dev`. |

---

## Download & install (unsigned build)

Builds are produced automatically by GitHub Actions — see [Automation](#automation-github-actions).

1. Open **Actions → Build macOS bundle →** newest run → **Artifacts**.
2. Download `WinMac-Launcher-macos-universal-dmg` (or the zipped `.app` artifact).
3. Because the bundle is unsigned, macOS will quarantine it. Either right-click the app → **Open**, or run:

   ```bash
   xattr -dr com.apple.quarantine "/Applications/WinMac Launcher.app"
   ```

---

## Development

### UI-only development on Windows / Linux (recommended for design work)

```powershell
npm install
npm run dev        # http://localhost:1420 — demo backend, no Rust required
npm run build      # type-check + production bundle
```

The browser preview uses `src/demoBackend.ts`; on Windows the Tauri build uses `src-tauri/src/mock.rs` and seeds a demo library under `~/Games/WinContainers` on first run.

To run the real shell on Windows you additionally need Rust and the MSVC Build Tools:

```powershell
winget install Rustlang.Rustup
winget install Microsoft.VisualStudio.2022.BuildTools   # "Desktop development with C++"
npm run tauri:dev
```

### Running on macOS (from source)

Prerequisites — Xcode Command Line Tools (WebKit headers and `codesign`), Node.js 20+, Rust stable:

```bash
xcode-select --install
brew install node
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Clone, install and start the app:

```bash
git clone <your-repo-url> winmac-launcher
cd winmac-launcher
npm install
npm run tauri:dev
```

The first run compiles the Rust side (a few minutes) and opens the native window; Vite serves the UI on port 1420 with hot reload.

#### 1. Select a Wine engine

The launcher never bundles Wine. Open **Engine manager** (the engine pill in the bottom-left corner) and either:

- **download** a managed build (a GitHub release `.tar.gz` / `.zip`),
- **locate** an existing `wine64` binary, or
- use an auto-detected installation (Game Porting Toolkit, Homebrew Wine, CrossOver, Whisky).

D3DMetal needs a Game Porting Toolkit runtime. Without GPTK, pick **DXVK** if your Wine build ships the DXVK DLLs (Whisky and CrossOver builds do), otherwise **WineD3D** as the universal fallback.

#### 2. Create a container and install something

**New container** → choose Windows 10/11, backend, ESync/MSync → **Create**. Then:

- **Run a .exe** to launch a Steam / GOG / Epic installer or a portable game;
- **Open C: drive** to copy files into `drive_c` by hand;
- **Winetricks** to add `vcrun2019`, `dotnet48`, `d3dx9` and similar dependencies.

Programs under `drive_c/Program Files` and `drive_c/Games` are picked up automatically.

#### 3. Play

Press **Play**. Wine output streams into the Output panel; **Stop** kills the process group and runs `wineserver -k`.

#### Build a local `.app` / `.dmg`

```bash
npm run tauri:build                                      # host architecture
npm run tauri:build -- --target universal-apple-darwin   # Intel + Apple Silicon
```

Artifacts land in `src-tauri/target/release/bundle/` or, for the universal build, `src-tauri/target/universal-apple-darwin/release/bundle/` (`dmg/*.dmg` and `macos/*.app`). The bundles are unsigned; on another Mac clear the quarantine flag first:

```bash
xattr -dr com.apple.quarantine "/Applications/WinMac Launcher.app"
```

---

## Automation (GitHub Actions)

`.github/workflows/build-macos.yml` builds a **universal** macOS bundle on every push to `main` and on manual dispatch:

- runs on `macos-latest`, installs Node 20 and both macOS Rust targets;
- runs the Rust unit tests (`cargo test --lib`);
- `npx tauri build --target universal-apple-darwin` (unsigned, ad-hoc only);
- uploads two artifacts with `actions/upload-artifact@v4`: the `.dmg` and the `.app`.

No secrets or signing certificates are required. To notarize later, add `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`, `APPLE_TEAM_ID` / `APPLE_PASSWORD` and drop the `APPLE_SIGNING_IDENTITY: "-"` override.

---

## How it works

```text
src/                 Vanilla TypeScript UI (no framework)
  main.ts            app state, rendering, event wiring
  modals.ts          new container / settings / engine manager / winetricks
  api.ts             typed invoke() wrappers + event listeners
  demoBackend.ts     browser-only mock used by `npm run dev`
  styles.css         design system

src-tauri/src/
  lib.rs             builder, managed state, IPC registration
  commands.rs        the Tauri command surface
  container.rs       prefix layout, container.json, EXE scanning, registry tweaks
  engine.rs          wine64 detection, archive download/extract, winetricks
  runner.rs          environment, process spawn, log streaming, stop
  winetricks.rs      curated verb allowlist
  mock.rs            Windows/CI demo backend
  paths.rs           app-support dirs, tilde expansion, folder sanitising
  models.rs          serde DTOs shared with TypeScript
```

### Container layout

```text
~/Games/WinContainers/Steam Library/
├── container.json          # id, name, backend, esync/msync, engine, dates
├── drive_c/
│   ├── Program Files/…
│   └── Games/…             # scanned for *.exe
└── .winmac/
    └── settings.json       # last applied registry stamp
```

`container.json` is intentionally plain JSON:

```json
{
  "id": "steam-library",
  "name": "Steam Library",
  "path": "/Users/you/Games/WinContainers/Steam Library",
  "windowsVersion": "win10",
  "graphicsBackend": "d3dmetal",
  "esync": true,
  "msync": true,
  "metalHud": false,
  "retina": true,
  "engineId": "apple-gptk-2",
  "extraEnv": {},
  "launchArgs": "-windowed",
  "gameOverrides": {},
  "lastPlayed": {},
  "createdAt": "2026-01-01T00:00:00Z",
  "updatedAt": "2026-01-01T00:00:00Z"
}
```

### Launch environment

For every run the launcher composes:

| Variable | Source |
|---|---|
| `WINEPREFIX` | container folder |
| `WINEESYNC` / `WINEMSYNC` | container toggles |
| `WINEDLLOVERRIDES` | graphics backend (`d3d11,dxgi=n,b` for D3DMetal / DXVK) |
| `MTL_HUD_ENABLED` | Metal HUD toggle |
| `DXVK_HUD` | DXVK + HUD enabled |
| `WINEDEBUG=-all`, `WINE_LARGE_ADDRESS_AWARE=1` | always |
| user variables | *Container settings → Environment* |

On launch, Windows-version and Retina registry keys are re-applied only when the `.winmac/settings.json` stamp is stale, then the process is started in its own process group with `DYLD_FALLBACK_LIBRARY_PATH` pointing at the engine's `lib/` folder.

### Graphics backends

| Backend | Engine | Notes |
|---|---|---|
| **D3DMetal** | Game Porting Toolkit 2 | Apple's DX11/DX12 → Metal translator. Best performance and compatibility on Apple Silicon; requires the GPTK runtime. |
| **DXVK** | Wine / CrossOver | DX9–DX11 → Vulkan → MoltenVK. Great for older DirectX 9/10 titles. |
| **WineD3D** | any | Wine's built-in OpenGL path. Slowest, but the most forgiving fallback. |

### Engine detection

The launcher looks in the managed folder (`…/WinMacLauncher/engine/<id>/`) and at common system locations:

```text
/Applications/Game Porting Toolkit.app/Contents/Resources/wine/bin/wine64
/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine
/Applications/Whisky.app/Contents/Resources/Libraries/Wine/bin/wine64
/Applications/Wine Stable.app/Contents/Resources/wine/bin/wine64
/opt/homebrew/bin/wine64
/usr/local/bin/wine64
```

Anything else can be added with **Engine manager → Locate wine binary**. Managed engines are downloaded from GitHub releases (`gh:owner/repo`) as `.tar.gz`, `.tar.xz` or `.zip`; extraction flattens the archive and locates `wine64` automatically.

---

## Troubleshooting

| Symptom | Fix |
|---|---|
| “WinMac Launcher is damaged” | `xattr -dr com.apple.quarantine "/Applications/WinMac Launcher.app"` |
| “No compatible Wine engine is installed” | Open the engine manager, download a Wine build or locate an existing `wine64`. |
| Game launches then exits instantly | Open the live terminal — usually a missing VC++ runtime. Install `vcrun2019` via Winetricks. |
| Black screen with D3DMetal | Verify GPTK is installed and the container backend is D3DMetal; try DXVK instead. |
| Tiny or blurry UI in-game | Toggle **Retina mode** in container settings. |
| Controller not detected | Wine maps controllers through GameController.framework; enable the game's native controller support and restart the container. |
| `wineserver` keeps running | **Stop** in the terminal bar kills the process group and calls `wineserver -k`. |

---

## Limitations

- No anti-cheat support: EAC / BattlEye titles will not run.
- 32-bit Windows apps work only with engines that still ship 32-bit support.
- GPTK is Apple evaluation software; you must obtain it yourself with an Apple Developer account. The launcher never bundles or redistributes it.
- macOS builds from CI are unsigned and not notarized.

---

## License

Private project — no license granted. Wine, DXVK, MoltenVK, CrossOver and the Game Porting Toolkit remain the property of their respective authors.
