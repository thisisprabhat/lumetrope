# Lumetrope — Design

Date: 2026-10-23 · Status: approved to build (user said "go ahead", questions skipped)
Research: `docs/research/2026-10-23-lottie-creator-teardown.md`

## Intent

Lumetrope is an open-source alternative to LottieFiles' **Lottie Creator** and **Lottie Editor**
in one application: author Lottie animations from scratch, edit existing Lottie/dotLottie files,
build dotLottie **state machines**, and preview them interactively. It works offline, keeps
local version history, and lets AI agents drive it through **MCP**.

User-stated requirements (theirs): single app (creator + editor, not two apps), feature-full
(animation options, state machine options), MCP support, cross-platform desktop (macOS first),
local version control, own rendering engine, interactive previewer, Lottie Creator + dotLottie
state machine parity, **built in WASM so it can later become a desktop app**.

## Goals

1. Open/import `.json` (Lottie), `.lottie` (dotLottie v1/v2), SVG, PNG/JPG; export `.json` and
   `.lottie` v2 (animations, themes, state machines, images, fonts) that play in standard
   players (lottie-web, dotlottie-web, dotlottie-rs).
2. Creator parity for authoring: shapes (rect, ellipse, polygon, star), pen + pencil paths,
   text, groups, nested scenes (precomps), keyframes on every animatable property, easing
   (presets + bezier curve editor + hold), motion paths, anchor points, parenting, layer
   in/out, blend modes, masks, track mattes, trim paths, round corners, gradients, strokes
   (width/cap/join/dash), effects (drop shadow, gaussian blur, fill, tint, tritone), segments
   (markers), themes + slots, align/distribute, snapping, guides/grid/rulers, copy/paste,
   duplicate, undo/redo.
3. Editor parity (Lottie Editor): layer tree of any imported Lottie, show/hide/delete, global
   recolor via a "unique colours" palette, canvas/duration/fps changes, file info.
4. State machine editor: node graph (PlaybackState, GlobalState), transitions (instant and
   Tweened with easing), guards (Numeric/String/Boolean/Event with all operators and `$input`
   references), inputs (Boolean, Numeric, String, Event), interactions (Click, PointerDown/Up/
   Enter/Exit/Move with layer targeting, OnComplete, OnLoopComplete), every action (OpenUrl,
   SetTheme, Increment, Decrement, Toggle, SetBoolean, SetString, SetNumeric, Fire, Reset,
   SetFrame, SetProgress, FireCustomEvent), entry/exit actions, multiple machines per file.
5. Interactive previewer: plays the document with the live state machine, routes real pointer
   events, shows current state, editable inputs, and an event log.
6. Own rendering engine (Rust → WASM), deterministic, used by canvas, previewer, thumbnails.
7. Local version control: commit, history, diff summary, restore, branch — no server.
8. MCP: an external AI agent can inspect and edit the open document with a tool catalogue
   covering the same ground as Creator's (~100 tools).
9. Runs in a browser today; packaged as a macOS desktop app via Tauri with no UI rewrite.

## Non-goals (v1)

Cloud accounts, sharing, collaboration, plugin marketplace, in-app AI copilot (MCP covers
agents), After Effects import, audio, 3D layers, a full AE expression engine (expressions are
preserved round-trip, not evaluated), Windows/Linux packaging (should work, not tested).

## Anti-goals

- Exported files that don't play in official players.
- Copying LottieFiles proprietary code, assets, or branding (clean-room: open specs only).
- Silently dropping unknown Lottie data on import → export (must round-trip).

## Architecture

```
lumetrope/  (repo root)
  Cargo.toml                 cargo workspace
  crates/core/               lumetrope-core: pure Rust, no wasm deps, fully unit-tested
    model/      Lottie schema (serde, lenient; unknown fields preserved in flattened maps)
    anim/       keyframe evaluation: cubic-bezier easing, hold, spatial bezier, per-type lerp
    scene/      evaluate(doc, frame) -> render tree (transforms, parenting, precomps, time remap)
    shapes/     rect/ellipse/star/polygon → bezier paths, trim paths, round corners
    render/     tiny-skia rasteriser: fills, strokes, gradients, masks, mattes, blend, effects
    text/       glyph outlines via ttf-parser, bundled OFL font, layout
    ops/        editor command layer: typed ops (= MCP tool catalogue), undo/redo
    hit/        hit-testing and bounds for canvas interaction
    sm/         dotLottie 2.0 state machine runtime
    dotlottie/  .lottie zip read/write, manifest, themes/slots
    vcs/        content-addressed object store, commits, branches, diff
    svg/        SVG import via usvg → Lottie shape layers
  crates/wasm/               lumetrope-wasm: wasm-bindgen facade (Engine class)
  apps/web/                  Vite + React + TypeScript UI (the whole app)
  packages/mcp/              Node stdio MCP server + WebSocket bridge to the app
  apps/desktop/              Tauri v2 shell wrapping apps/web (macOS first)
```

**One source of truth for operations.** Every mutation (UI gesture, MCP tool, undo/redo) is an
`Op` applied by `core::ops`. Ops are JSON `{tool, args}`; the core exports the op catalogue with
JSON Schemas, which the MCP server serves verbatim, so UI and agents cannot diverge.

**Rendering path.** `Engine.render(frame, w, h)` rasterises into an RGBA buffer in WASM memory;
the UI wraps it as `ImageData` and blits to `<canvas>`. Selection handles, pen tool, guides and
motion paths are drawn in an SVG overlay using geometry queried from the core.

**Persistence.** Browser: IndexedDB (projects + VCS objects) behind a storage interface. Tauri:
same interface backed by the filesystem plus native dialogs. VCS logic lives in Rust; the host
stores `(hash → bytes)` and refs only.

**MCP bridge.** `lumetrope-mcp` is a stdio MCP server listening on `ws://127.0.0.1:47321`; it
accepts one app connection from allow-listed origins (`http://localhost:*`,
`http://127.0.0.1:*`, `tauri://localhost`) and forwards `tool_request` → app → `tool_response`
with timeouts. The app connects when "Enable Local MCP" is on. If `navigator.modelContext`
exists, the same tools are registered as WebMCP tools.

**State machine runtime** follows dotLottie 2.0: global transitions first, declaration order,
ANDed guards, guardless fallback, event inputs consumed per run, tween blocks evaluation, exit
actions before change, entry actions after (or after the tween).

## Key decisions (Claude's calls)

- Rasteriser: **tiny-skia** (pure Rust, wasm-friendly). GPU (vello) later behind the same API.
- UI: React 18 + TypeScript + Vite; Zustand; @xyflow/react for the SM graph; custom dark theme.
- Frames are integers in the UI; the core supports fractional time.
- Text: bundled OFL font + user fonts embedded into `.lottie` `f/`.
- Project format: `.lottie` v2; version history stored beside it.
- Disk budget is tight (~5 GB free): one shared cargo target dir, release-size wasm, Tauri last.

## Testing

- Core: `cargo test` — TDD unit tests; golden-image tests; SM conformance tests from spec
  rules; round-trip tests (import → export → re-import equal).
- WASM: smoke tests of the Engine facade.
- Web: Vitest for stores/utilities; end-to-end verification in Chrome.
- MCP: Vitest with a fake app socket; manual check with Claude Code.
- Desktop: `cargo tauri build` on macOS, launch, open/save round-trip.

## Delivery phases

1. Workspace + core model + keyframe evaluation
2. Renderer (shapes, paint, transforms, precomps, masks, mattes, trim, blend) + golden tests
3. WASM facade + web shell with canvas player (previewer v0)
4. Ops layer + undo/redo + hit-testing
5. Editor UI: tools, properties, timeline/keyframes/easing, outliner, import/export
6. Text, effects, gradients UI, snapping/guides, recolor palette
7. dotLottie v2 I/O, themes/slots, segments
8. State machine runtime + graph editor + interactive previewer
9. Version control core + history UI
10. MCP server/bridge + WebMCP
11. Tauri desktop shell, macOS build and test
