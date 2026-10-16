# Lumetrope Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build Lumetrope — a single open-source Lottie creator + editor with a Rust/WASM engine (renderer, state machines, version control, op catalogue), a React UI, an MCP bridge, and a Tauri macOS shell.

**Architecture:** `crates/core` (pure Rust) owns the project document as one `serde_json::Value`, every mutation (`ops`), rendering (`tiny-skia`), the dotLottie state machine runtime, dotLottie I/O and VCS. `crates/wasm` exposes it as an `Engine` class. `apps/web` is the whole UI. `packages/mcp` is a stdio MCP server bridging to the app over WebSocket. `apps/desktop` wraps `apps/web` with Tauri v2.

**Tech Stack:** Rust 1.99 (edition 2021), serde_json, tiny-skia 0.11, ttf-parser, usvg, zip (deflate, pure Rust), sha2, wasm-bindgen + wasm-pack; TypeScript 5, React 18, Vite 5, Zustand, @xyflow/react, Vitest; Node 20, @modelcontextprotocol/sdk, ws, zod; Tauri v2.

**Spec:** `docs/superpowers/specs/2026-10-23-lumetrope-design.md` (research: `docs/research/2026-10-23-lottie-creator-teardown.md`)

## Global Constraints

- Product name "Lumetrope"; crate names `lumetrope-core`, `lumetrope-wasm`; npm `lumetrope-mcp`.
- Exported `.json`/`.lottie` must play in lottie-web / dotlottie-web; dotLottie manifest `version: "2"`.
- Round-trip: unknown Lottie fields survive import → export (source of truth is JSON `Value`).
- Editor-only metadata lives under the layer key `"lmeta"` and is stripped by every export.
- Clean-room: no LottieFiles code/assets/branding; MCP tool descriptions written fresh.
- MCP bridge: `ws://127.0.0.1:47321`, one connection, origins `http://localhost:*`, `http://127.0.0.1:*`, `tauri://localhost`.
- Disk is tight (~5 GB): single cargo target dir at repo root; no extra toolchains; Tauri last.
- UI frames are integers; core time is `f64` frames.
- State machine semantics per dotLottie 2.0 (research §3). Builder's call: guardless transitions fire only once the current PlaybackState has completed (non-looping) — prevents infinite chains.

## Review Focus

1. **Malformed / partial Lottie input** (missing `layers`, `k` as number vs array, legacy `e` keyframes) → loads without panic, renders what it can. Test: `model::tests::parses_legacy_and_scalar_forms`, `engine::tests::garbage_input_is_error_not_panic`.
2. **Zero-length or degenerate geometry** (0-size rect, single-vertex path, keyframes at the same frame, trim start==end) → no NaN, no panic. Test: `render::tests::degenerate_shapes_do_not_panic`, `anim::tests::coincident_keyframes`.
3. **Undo after multi-step ops** (create shape then keyframe then delete) → exact JSON restore. Test: `ops::tests::undo_restores_exact_json`.
4. **State machine runaway** (transitions that ping-pong A↔B on the same guard) → pipeline caps at 32 iterations and stays responsive. Test: `sm::tests::ping_pong_is_bounded`.
5. **Export of editor-only data** (locks, selection ids, themes not in manifest) → stripped/valid. Test: `dotlottie::tests::export_strips_lmeta_and_validates_manifest`.

---

## File Structure

```
Cargo.toml                         workspace (members crates/core, crates/wasm)
crates/core/Cargo.toml
crates/core/src/lib.rs             re-exports
crates/core/src/model.rs           typed lenient Lottie model parsed from Value
crates/core/src/anim.rs            Prop evaluation, easing, spatial bezier
crates/core/src/geom.rs            Bezier path type, primitives → paths, trim, round corners, lengths
crates/core/src/scene.rs           layer time/transform/parent resolution helpers
crates/core/src/render/mod.rs      Renderer entry: render(project, anim, frame, w, h) -> Pixmap
crates/core/src/render/shapes.rs   shape-tree walker (group/paint/modifier/repeater semantics)
crates/core/src/render/paint.rs    fills, strokes, dashes, gradients
crates/core/src/render/layers.rs   masks, mattes, blend, precomp, image, solid, effects
crates/core/src/render/effects.rs  drop shadow, gaussian blur, fill, tint, tritone
crates/core/src/text.rs            font registry, layout, glyph outlines
crates/core/src/project.rs         Project root Value helpers, ids, active animation
crates/core/src/ops/mod.rs         Tx (undoable JSON edits), dispatcher, History
crates/core/src/ops/catalog.rs     tool definitions (name, description, JSON schema)
crates/core/src/ops/{scene,layers,shapes,keys,paint,fx,text,themes,sm,assets,presets}.rs
crates/core/src/hit.rs             bounds + hit testing
crates/core/src/sm.rs              state machine runtime
crates/core/src/dotlottie.rs       .lottie read/write, theme application
crates/core/src/svg.rs             SVG → shape layer
crates/core/src/vcs.rs             repo: objects, commits, branches, diff
crates/core/assets/                bundled OFL font
crates/core/tests/golden/          sample lotties + expected PNGs
crates/wasm/src/lib.rs             Engine facade
apps/web/…                         Vite React app (see Phase 5 tasks)
packages/mcp/src/{index,bridge,catalog}.ts
apps/desktop/src-tauri/…           Tauri shell
```

---

## Phase 1 — Workspace, model, keyframe evaluation

### Task 1: Workspace + lenient model

**Files:** Create `Cargo.toml`, `crates/core/Cargo.toml`, `crates/core/src/{lib,model}.rs`.

**Interfaces — Produces:**
- `model::Animation { fr, ip, op, w, h, nm, layers: Vec<Layer>, assets: Vec<Asset>, markers: Vec<Marker>, slots: Map<String, Value>, fonts: Vec<FontDef> }`
- `model::Layer { ty: i64, nm, ind: Option<i64>, parent: Option<i64>, ip, op, st, sr, ks: Transform, hd, bm, tt, tp, td, masks: Vec<Mask>, ef: Vec<Effect>, ref_id, w, h, tm: Option<Prop>, sc, sw, sh, shapes: Vec<Shape>, text: Option<TextData>, json_index: usize }`
- `model::Prop { animated: bool, value: Vec<f64>, keyframes: Vec<Keyframe>, sid: Option<String>, scalar: bool }`; `Keyframe { t, s: Vec<f64>, e: Option<Vec<f64>>, i: Option<Ease>, o: Option<Ease>, h: bool, to, ti }`; `Ease { x: Vec<f64>, y: Vec<f64> }`
- `model::ShapeProp { keyframes / static Bezier }`, `Bezier { c: bool, v, i, o: Vec<[f64;2]> }`
- `model::Shape` enum: `Group{it, tr…}`, `Rect`, `Ellipse`, `Star`, `Path`, `Fill`, `Stroke`, `GFill`, `GStroke`, `Transform`, `Trim`, `Round`, `Repeater`, `Unknown` (+ `nm`, `hd` common)
- `model::parse(v: &Value) -> Result<Animation, ModelError>`

- [ ] Step 1: tests `parses_minimal_doc` (w=512,h=512,fr=30, 1 shape layer → 1 Rect + 1 Fill), `parses_legacy_and_scalar_forms` (`"r":{"a":0,"k":45}` → value `[45.0]`, scalar=true; keyframe with `e` keeps `e`), `missing_layers_is_empty_not_error`.
- [ ] Step 2: `cargo test -p lumetrope-core` → FAIL (not defined).
- [ ] Step 3: implement with hand-written `from_value` functions (lenient: missing → defaults, wrong type → default), no `serde` derives needed for Shape dispatch.
- [ ] Step 4: tests PASS. Step 5: commit `feat(core): lenient lottie model`.

### Task 2: Keyframe evaluation

**Files:** Create `crates/core/src/anim.rs`.
**Produces:** `anim::eval(p: &Prop, frame: f64) -> Vec<f64>`, `anim::eval_shape(p: &ShapeProp, frame) -> Bezier`, `anim::cubic_ease(x1,y1,x2,y2,t) -> f64` (Newton + bisection on x).

- [ ] Tests: `linear_midpoint` (0→100 over frames 0..10, linear ease → 50 at 5), `hold_keyframe` (h=1 → 0 until frame 10), `ease_in_out_is_symmetric` (`cubic_ease(.42,0,.58,1,0.5)` ≈ 0.5 ±1e-4), `before_first_after_last_clamp`, `legacy_e_end_value`, `spatial_bezier_position` (to/ti nonzero → midpoint lies off straight line), `coincident_keyframes` (two kfs same t → no NaN), `shape_vertex_interpolation`.
- [ ] Implement: per-dimension easing when `i.x` has >1 entries; spatial position uses arc-length table (32 samples).
- [ ] Commit `feat(core): keyframe evaluation`.

## Phase 2 — Renderer

### Task 3: Geometry
**Files:** `crates/core/src/geom.rs`.
**Produces:** `Path { contours: Vec<Contour> }`, `Contour { pts: Vec<Vtx{p,in,out}>, closed }`; `geom::rect(c,size,round)`, `ellipse(c,size)`, `star(kind,c,pts,or,ir,os,is,rot)`, `from_bezier(&Bezier)`, `reversed()`, `transform(&Affine)`, `length()`, `trim(&[Path], start,end,offset, individually) -> Vec<Path>`, `round_corners(r)`, `to_skia() -> Option<tiny_skia::Path>`, `Affine` (2×3) with `from_lottie(a,p,s,r,sk,sa)`.
- [ ] Tests: `rect_has_4_vertices_clockwise`, `rect_roundness_adds_tangents`, `ellipse_length_close_to_circumference` (r=50 → 314.16 ±0.5), `trim_half_of_line` (0..100 line, s=0 e=50 → length 50), `trim_offset_wraps`, `star_5_points_has_10_vertices`, `affine_lottie_order` (anchor 10,10, pos 100,100, scale 200 → (10,10)→(100,100), (11,10)→(102,100)).
- [ ] Commit `feat(core): geometry`.

### Task 4: Shape-tree rendering + paint
**Files:** `crates/core/src/render/{mod,shapes,paint}.rs`, `crates/core/src/scene.rs`.
**Produces:** `render::render(anim: &Animation, ctx: &RenderCtx, frame: f64, w: u32, h: u32) -> Pixmap` (scales doc w,h to target, transparent bg); `RenderCtx { images, fonts, slot_overrides }`.
Semantics: iterate items last→first; paint at index k paints all geometry (incl. nested groups, transformed) with index < k; nested group drawn at its index; modifiers apply to earlier geometry incl. nested groups; repeater copies earlier items. Layers drawn last→first; layer transform with parent chain; layer opacity multiplies paint alpha; ip/op visibility; `hd` skip.
- [ ] Tests (pixel probes on 100×100): `red_rect_center_pixel_red`, `fill_after_two_shapes_paints_both`, `stroke_width_covers_edge`, `group_transform_moves_shape`, `linear_gradient_left_dark_right_light`, `radial_gradient_center`, `layer_opacity_halves_alpha`, `parenting_follows_parent_position`, `layer_outside_ip_op_not_drawn`, `trim_path_draws_half_stroke`, `repeater_three_copies`, `degenerate_shapes_do_not_panic`.
- [ ] Commit `feat(core): shape renderer`.

### Task 5: Layers: precomp, solid, image, masks, mattes, blend, effects
**Files:** `crates/core/src/render/{layers,effects}.rs`.
- [ ] Tests: `precomp_layer_renders_asset_with_time_offset`, `time_remap_selects_frame`, `solid_layer_fills_size`, `image_layer_draws_png_data_uri`, `mask_add_clips`, `mask_subtract_cuts_hole`, `mask_inverted`, `alpha_matte_clips`, `luma_matte`, `blend_multiply`, `drop_shadow_offsets_dark_pixels`, `gaussian_blur_softens_edge`, `tint_maps_to_gray_range`, `fill_effect_recolors`.
- [ ] Commit `feat(core): layer compositing and effects`.

### Task 6: Text
**Files:** `crates/core/src/text.rs`, `crates/core/assets/` (OFL font).
**Produces:** `text::FontRegistry::{with_default(), add(name, bytes)}`, `text::layout(doc: &TextDoc, fonts) -> Vec<Path>`; render text layers (fill + stroke, justify L/R/C, tracking, line height, `\r` line breaks, box text wrap).
- [ ] Tests: `text_renders_ink_pixels`, `center_justify_centers_bbox`, `unknown_font_falls_back`.
- [ ] Commit `feat(core): text rendering`.

### Task 7: Golden render tests
**Files:** `crates/core/tests/golden.rs`, `crates/core/tests/golden/*.json|png`.
- [ ] Author 6 sample lotties (shapes, gradients, masks/mattes, precomp, text, trim/repeater); test renders frame 0/mid and compares to PNGs (max per-channel delta ≤ 8 on 99% pixels); `UPDATE_GOLDEN=1` rewrites. Commit.

## Phase 3 — WASM facade + previewer v0

### Task 8: Engine facade
**Files:** `crates/wasm/{Cargo.toml,src/lib.rs}`, `crates/core/src/project.rs`.
**Produces (JS):** `new Engine()`, `load_file(name: string, bytes: Uint8Array): string /*info json*/`, `new_project(w,h,fr,frames)`, `info(): string`, `render(frame, w, h): Uint8ClampedArray`, `project_json(): string`, `set_project_json(s)`, `export_lottie(): string`, `export_dotlottie(): Uint8Array`, `apply(tool, argsJson): string` (`{"ok":…}`/`{"error":…}`), `catalog(): string`, `undo()/redo(): bool`, `can_undo()/can_redo()`, `hit_test(x,y,frame): string`, `layer_geometry(id, frame): string`, `add_font(name, bytes)`, `revision(): number`.
Project root Value: `{ "version":1, "active":"main", "animations": {"main": <lottie>}, "themes": {}, "stateMachines": {}, "fonts": {} , "settings": {} }`.
- [ ] Tests (core): `project_from_lottie_wraps_main`, `garbage_input_is_error_not_panic`. Build: `wasm-pack build crates/wasm --target web --release --out-dir ../../apps/web/src/wasm` succeeds. Commit.

### Task 9: Web app shell + player
**Files:** `apps/web/{package.json,vite.config.ts,tsconfig.json,index.html}`, `src/{main.tsx,App.tsx,engine.ts,store.ts,styles.css}`, `src/components/{TopBar,Canvas,PlayerBar}.tsx`.
- [ ] `engine.ts`: async `initEngine()` singleton; typed wrappers. `store.ts`: `{ revision, frame, playing, mode, selection, tool, zoom, pan, loop, speed }`. Canvas renders at devicePixelRatio, rAF playback at doc fr. Drag-and-drop/open `.json/.lottie/.svg`.
- [ ] Verify in Chrome: open a sample lottie, plays, scrubs. Vitest: `store.test.ts` frame clamping. Commit.

## Phase 4 — Ops + undo + hit testing

### Task 10: Tx, History, dispatcher, catalogue
**Files:** `crates/core/src/ops/{mod,catalog}.rs`.
**Produces:** `Tx { edits: Vec<Edit> }` with `set(ptr, v)`, `insert(ptr_array, idx, v)`, `remove(ptr_array, idx)`; `History { undo: Vec<Tx>, redo: Vec<Tx>, cap: 200 }`; `ops::apply(project: &mut Value, hist: &mut History, tool: &str, args: &Value) -> Result<Value, OpError>`; `catalog::tools() -> Vec<ToolDef{name, description, schema: Value, mutates: bool}>`. Ids: layer id `"<animId>:<compId>:<ind>"` where compId `root` or precomp asset id; shape path `"0/2/1"`.
- [ ] Tests: `undo_restores_exact_json`, `redo_reapplies`, `new_op_clears_redo`, `unknown_tool_errors`, `catalog_names_unique_and_schemas_are_objects`.

### Task 11: Operation families (one commit each, tests per family)
Files `ops/{scene,layers,shapes,keys,paint,fx,text,themes,sm,assets,presets}.rs`. Tool names (final catalogue):
- scene: `read_scene, inspect_layer, list_scenes, create_scene, switch_scene, set_scene_settings, list_colors, replace_color`
- layers: `create_shape, create_solid, create_null, duplicate_layer, delete_layer, rename_layer, reorder_layer, set_parent, set_layer_visibility, set_layer_lock, set_layer_time, set_blend_mode, set_matte, remove_matte, precompose_layers, align_layers, distribute_layers, set_transform, set_anchor_point, flip_layer`
- shapes: `add_shape, remove_shape_item, set_path, add_trim_path, add_round_corners, add_repeater, update_shape_item, reorder_shape`
- paint: `set_fill, set_stroke, set_gradient, remove_paint`
- keys: `set_keyframe, remove_keyframe, set_keyframe_easing, read_keyframes, clear_animation, shift_keyframes, retime_keyframes, copy_animation`
- masks/fx: `add_mask, update_mask, remove_mask, add_effect, update_effect, remove_effect, list_effects`
- text: `create_text, set_text, set_text_style`
- themes/slots/segments: `list_slots, create_slot, unlink_slot, update_slot_value, list_themes, add_theme, update_theme_rule, remove_theme, select_theme, list_segments, add_segment, update_segment, remove_segment`
- state machines: `list_state_machines, add_state_machine, remove_state_machine, rename_state_machine, read_state_machine, add_state, update_state, remove_state, set_initial_state, add_transition, update_transition, remove_transition, add_input, update_input, remove_input, add_interaction, update_interaction, remove_interaction`
- assets: `import_svg, import_lottie, import_image`
- presets: `apply_motion_preset (fade_in, fade_out, scale_in, pop, slide_in_left/right/up/down, spin, bounce, pulse, shake, wiggle, float), stagger_layers`
Property addressing in args: `{ layer, shape?: "0/1", property: "position"|"scale"|"rotation"|"opacity"|"anchor"|"skew"|"skewAxis"|"color"|"width"|"size"|"roundness"|"start"|"end"|"offset"|"points"|"outerRadius"|"innerRadius"|… or raw key }`.
- [ ] Tests per family, e.g. `create_shape_adds_layer_on_top`, `set_keyframe_animates_static_prop`, `remove_last_keyframe_makes_static`, `easing_preset_ease_in_out_writes_bezier`, `replace_color_updates_all_fills`, `precompose_moves_layers_to_asset`, `add_transition_with_numeric_guard`, `import_svg_creates_shape_layer`, `motion_preset_fade_in_writes_opacity_keys`.

### Task 12: Hit testing + geometry queries
**Files:** `crates/core/src/hit.rs`. **Produces:** `hit::hit_test(anim, frame, x, y) -> Option<LayerId>` (topmost visible, path containment with stroke tolerance), `hit::layer_bounds(anim, id, frame) -> Rect + corners in comp space`, `hit::layer_named_at(anim, frame, x, y) -> Vec<String>` (for SM layerName).
- [ ] Tests: `hit_topmost_wins`, `miss_returns_none`, `bounds_follow_scale`. Commit.

## Phase 5 — Editor UI

### Task 13: Layout + panels
`apps/web/src/components/`: `TopBar` (menu: New, Open, Import, Save .lottie, Export JSON/.lottie, Undo/Redo, View toggles, Settings), tools (Select V, Rect R, Ellipse O, Polygon, Star, Pen P, Text T, Hand H), mode switch Animate | State Machine | Preview; `LeftSidebar` tabs Layers / Assets / Themes / Segments / History / Colors; `PropertiesPanel`; `Timeline`; dark theme CSS variables. Keyboard shortcuts module `src/shortcuts.ts`.

### Task 14: Canvas interaction
`CanvasOverlay.tsx` (SVG): selection bbox + 8 handles + rotate handle, drag move/scale/rotate (writes `set_transform`, or `set_keyframe` when the property is animated / auto-key on), shape draw tools (drag → `create_shape`), pen tool (click/drag vertices, close on first vertex → `create_shape {type:"path"}`), text tool (click → `create_text`), hand/zoom (wheel, space-drag), rulers/grid/guides toggles, snapping to scene center/edges/other layer bounds (6px).

### Task 15: Properties panel
Scene (size presets, w/h, fr, duration, bg); Transform rows with scrubber inputs + keyframe diamond (toggle key at current frame; filled when key exists); Appearance: fill/stroke solid/gradient with color picker (`ColorPicker.tsx`, hex + alpha), stroke width/cap/join/dash; Trim/Round/Repeater add buttons; Masks list; Matte select; Blend select; Effects add menu (Drop shadow, Gaussian blur, Fill, Tint, Tritone) with param editors; Text properties; Slot link button (create/link slot).

### Task 16: Timeline
`Timeline.tsx`: ruler (frames/seconds), playhead drag, work area, layer rows (visibility, lock, solo, rename inline, drag reorder → `reorder_layer`, parent pick), layer bar (drag ip/op), expand to animated properties showing keyframes; keyframe select/multi-select/drag (→ `shift_keyframes`), delete, copy/paste; easing popover `EasingEditor.tsx` (bezier curve with 2 handles, presets linear/ease/in/out/in-out/back/hold) → `set_keyframe_easing`; segments track (markers) drag edit.

### Task 17: Import/export + Lottie Editor parity
Open/Import (.json, .lottie, .svg, .png/.jpg), Save project as `.lottie`, Export dialog (JSON / dotLottie, preview, file name). Colors tab (unique colors palette → `replace_color`), Info section (size, layer count, generator, version). Vitest: `export.test.ts` (download blob mime). Verify end-to-end in Chrome. Commit each task.

## Phase 6 — dotLottie, themes, segments
### Task 18: dotLottie read/write + themes
**Files:** `crates/core/src/dotlottie.rs`. **Produces:** `read(bytes) -> Result<Value /*project*/>` (v1 `animations/`, v2 `a/ i/ s/ t/ f/`), `write(project) -> Result<Vec<u8>>` (manifest v2, images moved to `i/`, `lmeta` stripped), `apply_theme(anim: &mut Animation, theme: &Value, anim_id)`.
- [ ] Tests: `roundtrip_v2_project`, `reads_v1_manifest`, `export_strips_lmeta_and_validates_manifest`, `theme_color_rule_overrides_slot`, `theme_keyframes_rule_animates_slot`. Commit.

## Phase 7 — State machines
### Task 19: Runtime
**Files:** `crates/core/src/sm.rs`. **Produces:** `Machine::new(def: &Value, anim: &Animation) -> Result<Machine>`; `start()`, `tick(dt_sec) -> Vec<SmEvent>`, `pointer(kind: PointerKind, x, y, hit_layers: &[String])`, `set_input(name, InputValue)`, `fire(name)`, `snapshot() -> MachineSnapshot{state, frame, inputs, playing, theme}`; `SmEvent` = `StateEntered, StateExited, Transition, OpenUrl{url,target}, CustomEvent(String), SetTheme(String), Complete, LoopComplete`.
- [ ] Tests: `initial_state_plays_segment`, `numeric_guard_greater_than`, `input_reference_compare_to`, `event_input_consumed_after_run`, `global_state_transitions_first`, `entry_exit_order`, `tweened_transition_blocks_until_done`, `on_complete_interaction`, `click_on_named_layer_only`, `increment_decrement_toggle_actions`, `set_progress_moves_frame`, `bounce_mode_reverses`, `loop_count_then_complete`, `ping_pong_is_bounded`. Commit.

### Task 20: SM editor UI + interactive previewer
`StateMachineEditor.tsx` (@xyflow/react): nodes for states (+ Global), edges for transitions, drag segment/animation from left list to create state, connect handles → `add_transition`; inspector panels: State (animation, segment, loop, loopCount, mode, speed, autoplay, final, entry/exit actions), Transition (type, duration, easing, guards editor), Inputs tab, Interactions tab (type, layer, actions editor). `Previewer.tsx`: canvas + engine `sm_*` API, pointer routing, input controls, current state badge, event log. Wasm: `sm_start(id)`, `sm_stop()`, `sm_tick(dt): string`, `sm_pointer(kind,x,y): string`, `sm_set_input(name,json)`, `sm_fire(name)`. Verify in Chrome with a two-state toggle example. Commit.

## Phase 8 — Version control
### Task 21: VCS core
**Files:** `crates/core/src/vcs.rs`. **Produces:** `Repo::new()`, `commit(&mut, project_bytes, message, author, ts) -> Hash`, `log(&self) -> Vec<CommitInfo>`, `checkout(hash) -> Vec<u8>`, `branches()`, `create_branch(name, at)`, `switch(name) -> Vec<u8>`, `diff(a,b) -> DiffSummary{layers_added, layers_removed, layers_changed, settings_changed, sm_changed, themes_changed}`, `to_bytes()/from_bytes()` (deflate-compressed JSON of objects + refs). Hash = sha256 hex of blob.
- [ ] Tests: `commit_and_log_order`, `identical_content_dedups`, `checkout_restores_bytes`, `branch_switch`, `diff_detects_added_layer`, `serialize_roundtrip`.
### Task 22: History UI + persistence
`HistoryPanel.tsx` (commit message box, timeline of commits, branch picker, restore/compare), `src/persist.ts` (IndexedDB: projects list, autosave every 5 s of idle, repo bytes). Wasm `vcs_*`. Commit.

## Phase 9 — MCP
### Task 23: lumetrope-mcp package
**Files:** `packages/mcp/{package.json,tsconfig.json,src/index.ts,src/bridge.ts,catalog.json,test/bridge.test.ts}`; `crates/core/src/bin/catalog.rs` emits catalog JSON.
- stdio MCP server registers every catalog tool (+ `get_frame_png` returns base64 PNG of current frame, `undo`, `redo`, `commit_version`, `list_versions`); forwards to app via bridge; 30 s timeout; "app not connected" error text tells the user to open Lumetrope and enable Local MCP.
- [ ] Vitest: `rejects_disallowed_origin`, `forwards_request_and_resolves_response`, `times_out_without_app`. 
### Task 24: App-side bridge + WebMCP
`apps/web/src/mcp/bridge.ts` (WebSocket client, reconnect w/ backoff, executes via engine.apply, triggers re-render), Settings toggle, WebMCP registration when `navigator.modelContext` exists. Manual check: Claude Code with `.mcp.json` → `read_scene`, `create_shape`. Commit.

## Phase 10 — Desktop
### Task 25: Tauri shell (macOS)
`apps/desktop/src-tauri/{Cargo.toml,tauri.conf.json,src/main.rs,capabilities/default.json}`; frontend = `apps/web/dist`; native open/save via `@tauri-apps/plugin-dialog` + `plugin-fs` behind `src/platform.ts` (browser fallback = file input / download). `cargo tauri build --bundles app` → `Lumetrope.app`; launch, open sample, export. Commit.

## Phase 11 — Finish
### Task 26: README, samples, licence (MIT), final verification
README (features, build, MCP setup), `samples/` (3 demo files incl. a state-machine button), full test run (`cargo test`, `pnpm -r test`, wasm build, web build), whole-branch review.
