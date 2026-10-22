# Building Lumetrope

This guide covers everything needed to build Lumetrope from source on **macOS**, **Windows** and
**Linux**: the Rust engine, its **WebAssembly** build, the web editor, the native desktop app and the
MCP server.

- [How the pieces fit](#how-the-pieces-fit)
- [Requirements](#requirements)
  - [Every platform](#every-platform)
  - [macOS](#macos)
  - [Windows](#windows)
  - [Linux](#linux)
- [Get the source](#get-the-source)
- [1. Build the engine to WebAssembly](#1-build-the-engine-to-webassembly)
- [2. Run or build the web editor](#2-run-or-build-the-web-editor)
- [3. Build the desktop app](#3-build-the-desktop-app)
- [4. Build the MCP server](#4-build-the-mcp-server)
- [Run the tests](#run-the-tests)
- [Continuous integration](#continuous-integration)
- [Troubleshooting](#troubleshooting)

## How the pieces fit

```
crates/core      Rust engine (model, renderer, ops, state machines, dotLottie, version control)
   │  wasm-pack
   ▼
crates/wasm  ──► apps/web/src/wasm     WebAssembly + JS bindings (generated, not committed)
                    │  Vite
                    ▼
                 apps/web/dist         the editor as static files (runs in any modern browser)
                    │  Tauri
                    ▼
                 apps/desktop          native app: .app/.dmg · .exe/.msi · .deb/.rpm/.AppImage

packages/mcp     MCP server (Node) that lets AI agents drive a running editor
```

Every build of the editor needs the WebAssembly step first. The root `pnpm build` and the desktop
build run it for you.

## Requirements

### Every platform

| Tool | Version | Used for |
|---|---|---|
| Git | any recent | getting the source |
| Rust (via [rustup](https://rustup.rs)) | current stable (Tauri 2 needs at least 1.77.2; tested with 1.99) | engine, desktop shell |
| Rust target `wasm32-unknown-unknown` | — | WebAssembly build |
| [wasm-pack](https://rustwasm.github.io/wasm-pack/) | **0.13 or newer** (tested with 0.15) | WebAssembly build + `wasm-opt` |
| Node.js | **20 or newer** (tested with 20.20) | web editor, MCP server, Tauri CLI |
| pnpm | 9 or newer (the repo pins 12.11.2 via `packageManager`) | workspace package manager |

Disk and memory: a full release build of the desktop app uses about **3.5 GB** in `target/` plus
about 130 MB of `node_modules/`. Plan for at least **6 GB free** and **8 GB RAM**.

Install the common tools once Rust and Node are on your machine:

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack            # or use the installer at rustwasm.github.io/wasm-pack/installer
corepack enable                    # gives you the pinned pnpm
```

`wasm-pack` downloads `wasm-opt` (Binaryen) the first time it runs a release build, so that first
build needs internet access.

### macOS

- **Build machine:** a recent macOS on Apple Silicon or Intel (tested on macOS 27, Apple Silicon).
- **Runs on:** macOS 11 Big Sur or newer.
- **Xcode Command Line Tools:** `xcode-select --install`. The full Xcode is not needed.
- Optional, for a universal (Apple Silicon + Intel) build: `rustup target add x86_64-apple-darwin aarch64-apple-darwin`.

Homebrew users can install the rest with:

```bash
brew install rustup node
rustup-init -y && source "$HOME/.cargo/env"
```

### Windows

- **Build machine and runtime:** Windows 10 or 11, x64 or ARM64.
- **Microsoft C++ Build Tools:** install
  [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with the
  **"Desktop development with C++"** workload, which includes MSVC, the **Windows 10/11 SDK** and
  the resource compiler `rc.exe` used to embed the app icon.
- **WebView2 runtime:** preinstalled on Windows 11 and current Windows 10. The installer Lumetrope
  produces downloads it automatically if it is missing.
- **Rust:** install with `rustup-init.exe` and keep the default **MSVC** toolchain
  (`stable-x86_64-pc-windows-msvc`). The GNU toolchain is not supported by Tauri's bundler.
- **Node.js:** the LTS installer from nodejs.org, or `winget install OpenJS.NodeJS.LTS`.

With winget (run in an elevated PowerShell):

```powershell
winget install Git.Git Rustlang.Rustup OpenJS.NodeJS.LTS
winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

Open a **new** terminal afterwards so `cargo`, `node` and `pnpm` are on `PATH`. The MSI bundler
(WiX) and the NSIS installer tools are downloaded automatically by Tauri on the first desktop build.

If you clone into a deep folder, enable long paths once: `git config --global core.longpaths true`.

### Linux

- **Runs on:** any x86_64 or arm64 desktop with **WebKitGTK 4.1, version 2.40 or newer**. That
  version is the first with the WebAssembly SIMD the engine uses; Ubuntu 22.04+, Debian 12+ and
  Fedora 38+ ship it.
- **Build packages:**

Debian / Ubuntu (22.04 or newer):

```bash
sudo apt update
sudo apt install -y libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

Fedora:

```bash
sudo dnf install -y webkit2gtk4.1-devel openssl-devel curl wget file \
  libappindicator-gtk3-devel librsvg2-devel libxdo-devel
sudo dnf group install -y "c-development"
```

Arch / Manjaro:

```bash
sudo pacman -Syu --needed webkit2gtk-4.1 base-devel curl wget file openssl \
  appmenu-gtk-module libappindicator-gtk3 librsvg xdotool
```

Then install Rust with rustup and Node 20+ from your distribution, nvm or nodejs.org.

## Get the source

```bash
git clone <repository-url> lumetrope
cd lumetrope
pnpm install
```

`pnpm install` installs every workspace package (`apps/web`, `apps/desktop`, `packages/mcp`). The
workspace already allows `esbuild`'s install script, so there is nothing to approve.

All commands below run from the repository root unless stated otherwise. They work the same in
bash, zsh and PowerShell.

## 1. Build the engine to WebAssembly

```bash
pnpm wasm
```

This runs `wasm-pack build crates/wasm --target web --release` and writes the module and its
JavaScript bindings to `apps/web/src/wasm/`. That folder is generated and git-ignored.

What the release build does:

- compiles with `opt-level = 3`, LTO and **WebAssembly SIMD** (`.cargo/config.toml` sets
  `+simd128` for the wasm target only, so native builds are unaffected);
- runs `wasm-opt -O3 --enable-simd` (configured in `crates/wasm/Cargo.toml`).

Re-run `pnpm wasm` whenever you change anything under `crates/`. The result works in Chrome/Edge
91+, Firefox 89+, Safari 16.4+ and every desktop webview listed above.

## 2. Run or build the web editor

Development server with hot reload:

```bash
pnpm dev            # http://localhost:5173
```

Production build (runs the WebAssembly step, type-checks, then bundles):

```bash
pnpm build          # → apps/web/dist
pnpm --filter @lumetrope/web preview   # serve the build locally to check it
```

`apps/web/dist` is a static site. Host it on any static file server; just make sure `.wasm` files
are served as `application/wasm`. Version history and autosave live in the browser's IndexedDB, so
no backend is needed.

## 3. Build the desktop app

The desktop shell is [Tauri 2](https://tauri.app). It loads the same editor in the system webview
(WKWebView on macOS, WebView2 on Windows, WebKitGTK on Linux) and adds native open/save dialogs.

Develop with live reload. This starts the Vite dev server for you:

```bash
pnpm --filter @lumetrope/desktop dev
```

Release builds run `pnpm build` (WebAssembly + web) automatically, then compile and package the
native app. **Build each platform on that platform**; cross-compiling desktop bundles isn't
supported, so use the CI workflow to get all three.

| Platform | Command | Output (under `target/release/bundle/`) |
|---|---|---|
| macOS | `pnpm --filter @lumetrope/desktop build:mac` | `macos/Lumetrope.app`, `dmg/Lumetrope_0.1.0_<arch>.dmg` |
| macOS universal | `pnpm --filter @lumetrope/desktop build:mac-universal` | as above, under `target/universal-apple-darwin/release/bundle/` |
| Windows | `pnpm --filter @lumetrope/desktop build:windows` | `nsis\Lumetrope_0.1.0_x64-setup.exe`, `msi\Lumetrope_0.1.0_x64_en-US.msi` |
| Linux | `pnpm --filter @lumetrope/desktop build:linux` | `deb/*.deb`, `rpm/*.rpm`, `appimage/*.AppImage` |
| Any | `pnpm --filter @lumetrope/desktop build` | every format the current OS supports |

For a quick local build without installers use the Tauri CLI directly:
`pnpm --filter @lumetrope/desktop tauri build --bundles app` (macOS `.app` only) or
`pnpm --filter @lumetrope/desktop tauri build --no-bundle` (any OS; the bare executable is
`target/release/lumetrope-desktop`, `lumetrope-desktop.exe` on Windows).

A first release build compiles a few hundred crates and takes 3–10 minutes; later builds are
incremental.

**Installing unsigned builds.** Builds from source aren't code-signed:

- macOS: the first time, right-click the app and choose **Open**, or run
  `xattr -dr com.apple.quarantine /Applications/Lumetrope.app`.
- Windows: SmartScreen may warn about an unknown publisher. Choose **More info → Run anyway**.
- Linux: `chmod +x Lumetrope_*.AppImage` before running it, or install the `.deb`/`.rpm`.

To sign and notarize, set the usual Tauri environment variables (`APPLE_SIGNING_IDENTITY`,
`APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` on macOS; a certificate thumbprint in
`bundle.windows.certificateThumbprint` on Windows). See Tauri's distribution guide.

**File access.** The app can only read or write files you pick in its native Open/Save dialogs;
no folder is opened up front (see `apps/desktop/src-tauri/capabilities/default.json`).

## 4. Build the MCP server

```bash
pnpm --filter lumetrope-mcp build     # → packages/mcp/dist/index.js
```

Register it with your agent. Claude Code, macOS/Linux:

```bash
claude mcp add lumetrope -- node "$(pwd)/packages/mcp/dist/index.js"
```

Claude Code, Windows PowerShell:

```powershell
claude mcp add lumetrope -- node "$PWD\packages\mcp\dist\index.js"
```

Any other MCP client takes the same command in its JSON config (`"command": "node"`,
`"args": ["<absolute path>/packages/mcp/dist/index.js"]`). Then open Lumetrope (browser or
desktop), click the **MCP** badge and enable **Local MCP**. The server listens on
`ws://127.0.0.1:47321`; set `LUMETROPE_MCP_PORT` to change it.

If you change engine tools, regenerate the tool catalogue with
`pnpm --filter lumetrope-mcp catalog`.

## Run the tests

```bash
cargo test -p lumetrope-core     # engine: renderer golden images, ops, state machines, dotLottie, VCS
pnpm -r test                     # web UI logic + MCP bridge and stdio server
# or all at once:
pnpm test
```

The 25 interactive dotLottie files used for conformance tests are optional: place them in
`samples/lottiefiles/` (git-ignored) and the engine tests pick them up.

To regenerate the demo files in `samples/`:

```bash
cargo test -p lumetrope-core --test demos -- --ignored write_demo_samples
```

## Continuous integration

`.github/workflows/build.yml` runs the full test suite on Ubuntu, then builds the desktop app on
`macos-latest`, `windows-latest` and `ubuntu-22.04` and uploads the `.dmg`, `.exe`/`.msi` and
`.deb`/`.rpm`/`.AppImage` as workflow artifacts. Push to GitHub, or run it by hand from the
Actions tab, to get installers for all three platforms.

## Troubleshooting

| Problem | Fix |
|---|---|
| `error: can't find crate for core` / `wasm32-unknown-unknown target may not be installed` | `rustup target add wasm32-unknown-unknown` |
| `wasm-pack: command not found` | `cargo install wasm-pack`, then open a new terminal |
| `wasm-opt` download fails (offline/firewalled) | Install Binaryen (`brew install binaryen`, `apt install binaryen`, `choco install binaryen`), or set `wasm-opt = false` under `[package.metadata.wasm-pack.profile.release]` in `crates/wasm/Cargo.toml` for a slower build |
| Web build fails with `Cannot find module './wasm/...'` | Run `pnpm wasm` first (or use `pnpm build`, which does) |
| Windows: `NotAttempted("llvm-rc")` or `rc.exe not found` | Install the Windows SDK through Visual Studio Build Tools ("Desktop development with C++"), then use a fresh terminal |
| Windows: `link.exe not found` | Same as above: the MSVC build tools are missing |
| Linux: `The system library 'webkit2gtk-4.1' required by crate ... was not found` | Install the `-dev`/`-devel` packages listed for your distribution |
| Linux: blank or white window (often NVIDIA) | Start with `WEBKIT_DISABLE_DMABUF_RENDERER=1 lumetrope-desktop` (or set it in the AppImage's environment) |
| Linux: "WebAssembly.instantiate(): ... simd" error | WebKitGTK is older than 2.40; update your distribution's `webkit2gtk-4.1` |
| Canvas effects (blur, luma mattes) look wrong on an older webview | Lumetrope already falls back to the CPU renderer when Canvas2D filters are missing; you can switch manually with **View ▸ Renderer** |
| Port 5173 already in use | Stop the other dev server, or run `pnpm --filter @lumetrope/web exec vite --port 5174` (and update `devUrl` in `apps/desktop/src-tauri/tauri.conf.json` for desktop dev) |
| MCP: "Port 47321 is already in use" | Another `lumetrope-mcp` is running; stop it or set `LUMETROPE_MCP_PORT` |
