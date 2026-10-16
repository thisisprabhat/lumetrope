# Lottie Creator / Lottie Editor — Teardown (clean-room reference)

Date: 2026-10-23. Targets: https://creator.lottiefiles.com (v1.117.0, "Lottie Creator 2.0") and
https://editor.lottiefiles.com. Method: passive observation of the live apps in the user's own
browser session, page resource inventory, public docs, the open dotLottie 2.0 spec, and static
reading of the MIT-licensed `@lottiefiles/creator-mcp@0.2.2` npm bundle (not executed).

**Clean-room rule:** this document records *behaviour and interoperable formats*. No proprietary
code is copied. The rebuild implements open specs (Lottie, dotLottie 2.0) independently.

Legend: **[O]** observed directly · **[D]** from public docs/spec · **[I]** inference.

---

## 1. Architecture of Lottie Creator

| Concern | Finding | Src |
|---|---|---|
| Canvas renderer | `thorvg_bg-*.wasm` — ThorVG compiled to WASM via wasm-bindgen (Rust wrapper naming) | O |
| Preview player | `@lottiefiles/dotlottie-web@0.80.0` `dotlottie-player.wasm` (dotlottie-rs + ThorVG) | O |
| Scripting sandbox | `quickjs-emscripten` — sandbox for plugins and/or expressions | O/I |
| Settings → Renderer | User-selectable renderer submenu (contents not captured) | O |
| Storage | Cloud files (`?fileId=uuid`), Drafts/workspace folders, "Show version history" | O |
| AI | "Motion Copilot" right-panel tab; "Animate with AI"; SM "AI assistant" | O |
| MCP | WebMCP toggle (badge in header) + "Local MCP" (npx bridge) | O |

## 2. Editor UI inventory (Animate mode) [O]

**Top toolbar:** main menu · Move (V) · Move-anchor · Shapes (Rectangle ⌥R, Ellipse ⌥O, Polygon, Star) ·
Pen · Pencil (expandable) · Text · Hand · Zoom −/%/+ · breadcrumb (Drafts / filename) · WebMCP badge ·
Feedback · **Animate | State Machines** mode switch · Share · Export · profile.

**Main menu:** Create new file · Make a copy · Move to · Save ⌘S · Show version history ·
Edit ▸ (Undo ⌘Z, Redo ⇧⌘Z, Cut, Copy, Paste, Paste as new scene ⌥⌘V, Duplicate ⌘D,
Advanced Duplicate ⇧⌘D, Delete, Select all ⌘A …) ·
View ▸ (Rulers ⇧R, Guides ⇧⌥;, Grid ⇧⌥', Snapping, Motion path ⇧⌥M, Zoom in/out/100%/fit,
Expand/Collapse timeline ⌘., Show/Hide UI `, Interface scale reset) ·
Settings ▸ (MCP Settings ▸ [Enable WebMCP, Enable Local MCP, guides], Interface Themes, Renderer ▸) ·
Help ▸ · Go to dashboard.

**Left sidebar:** Outliner / Assets tabs (scene tree, scene switcher) · Get started (starter assets,
learn, what's new) · Upload · Theme manager · Plugins (39 community plugins, e.g. SFX/audio,
Light Sweep, Procedural Nodes; per-plugin "Run") · Motion System (saved styles + motion presets) ·
Add nested scene · Segments.

**Canvas:** artboard with dot grid, transform controls (bbox handles, anchor marker), floating
player bar: time display, skip start/back/play/forward/end, loop, **theme selector**, snapshot
(take / hold-to-compare), preview-lottie, Motion Copilot.

**Right panel – Properties:**
- Scene: size presets (512×512…), W/H with lock, background (Transparent/colour), origin point
  (9-grid), Duration (mm:ss:ff), FPS, Start.
- Layer: align ×6 + distribute ×2; X/Y, Scale X/Y (aspect lock), Skew + Skew axis, Rotation —
  each with a **keyframe diamond** toggle; Anchor point 9-grid + visibility; Layer opacity,
  blend mode, visibility; Appearance: fill (solid/gradient, **slot-picker** to bind to a theme
  slot), fill opacity; stroke; **Matte**, **Trim path**, **Roundness**; **Effects**:
  Drop shadow, Gaussian blur, Fill, Tint, Tritone.

**Timeline:** scene tabs (Main Scene, Nested Scene N), layer list with expand, per-layer bar
(in/out), animation toggle, ruler in seconds, playhead, work-area range handles.

**Export dialog:** live preview with play/scrub/loop/speed/bg, file name, location
(workspace folder), Publish | Export (cloud). dotLottie export bundles SMs + themes + assets.

## 3. State Machines mode [O + D]

- Bottom panel = **node graph** per state machine (tabs StateMachine1 + "+"); left list of
  **Segments** (named time ranges authored in Animate mode) and **Animations**; drag onto graph to
  create states; always-present **GLOBAL STATE** node; zoom −/+/fit.
- Right panel tabs: Copilot · **Inputs** (name, type ∈ Boolean | Event | Numeric | String,
  default) · **Interactions** (type ∈ Click, PointerDown, PointerUp, PointerEnter, PointerExit,
  PointerMove [+ OnComplete/OnLoopComplete per spec], Layer Name target, Actions list).
- Canvas preview runs the live state machine.

### dotLottie 2.0 state machine schema [D] (https://dotlottie.io/spec/2.0/)
- File `s/<id>.json`: `{ initial, states[], interactions?[], inputs?[] }`.
- **States:** `PlaybackState {name, animation, transitions, loop?, loopCount?, autoplay?, final?,
  mode?: Forward|Reverse|Bounce|ReverseBounce, speed?, segment?(marker), backgroundColor?,
  entryActions?, exitActions?}`; `GlobalState {name, transitions, entryActions?, exitActions?}`
  (global transitions evaluated first).
- **Transitions:** `Transition {toState, guards?}`; `Tweened {toState, guards?, duration(s),
  easing:[x1,y1,x2,y2]}`. First passing transition in declaration order wins; guards ANDed;
  guardless = fallback; tweening blocks evaluation.
- **Guards:** Numeric (Equal, NotEqual, GreaterThan, GreaterThanOrEqual, LessThan,
  LessThanOrEqual), String/Boolean (Equal, NotEqual), Event; `compareTo` literal or `$input`.
- **Inputs:** Numeric, String, Boolean (with `value`), Event (edge-triggered, consumed per run).
- **Interactions:** PointerUp/Down/Enter/Exit, Click (`layerName?`), PointerMove, OnComplete
  (`stateName`), OnLoopComplete (`stateName`).
- **Actions:** OpenUrl, SetTheme, Increment, Decrement, Toggle, SetBoolean, SetString,
  SetNumeric, Fire, Reset, SetFrame, SetProgress, FireCustomEvent.

### dotLottie archive [D]
ZIP: `manifest.json` (`version:"2"`, generator, animations[{id, initialTheme, background,
themes}], themes[{id,name}], stateMachines[{id,name}], initial{stateMachine|animation}) ·
`a/` Lottie JSON · `i/` images · `s/` state machines · `t/` themes · `f/` fonts.
Themes: `rules[{id(slot), type: Color|Scalar|Position|Vector|Gradient|Image|Text, animations?,
value|keyframes, expression?}]`.

## 4. MCP surface [O static, MIT bundle]

`@lottiefiles/creator-mcp@0.2.2` — stdio MCP server (`@modelcontextprotocol/sdk` 1.27.1) that
starts a `ws` server on `127.0.0.1:3847`, accepts exactly one browser tab whose Origin is
creator.lottiefiles.com, and forwards `tool_request` → tab → `tool_response` (+ ack, queueing,
timeouts, ping). Tools are the same "native tools" Motion Copilot uses. 111 tools
(export_lottie excluded from MCP):

- **Scene/inspect:** read_scene, inspect_scene, list_scenes, create_scene, switch_scene,
  set_current_frame, set_scene_background, set_scene_settings, create_scene_layer,
  break_scene_layer, read_keyframes, ask_user
- **Shapes/text:** create_shape, bend_shape, create_shape_group, add_shape_to_layer,
  reorder_shape, set_group_transform, create_text, create_text_group, create_button,
  create_reference_board, set_text, set_text_style, measure_text_units, split_text, list_fonts
- **Layers:** duplicate_layer, tile_layer, reorder_layer, group_layers, delete_layer, rename_node,
  set_layer_visibility, set_layer_lock, flip_layer, align_layer, align_layers, layout,
  center_layer, set_anchor_point, set_layer_time
- **Paint/effects:** set_fill, set_stroke, animate_paint, set_blend_mode, add_drop_shadow,
  add_blur, add_color_overlay, add_tint, add_tritone, list_layer_effects, remove_layer_effect,
  update_layer_effect, create_mask, update_mask, remove_mask, set_matte, remove_matte
- **Animation:** apply_keyframes, animate_property, remove_keyframes, retime_keyframes,
  copy_animation, apply_motion_preset, apply_motion_path, stagger_layers, animate_entrance,
  apply_bounce, apply_squash, apply_coupled_shadow
- **Themes/slots/motion system:** list_themes, add_theme, select_theme, remove_theme, list_slots,
  create_slot, create_slot_from_value, update_slot_value, link_slot_to_layer, unlink_slot,
  list_motion_system_items
- **Segments/SM:** list_segments, add_segment, rename_segment, remove_segment, select_segment,
  list_state_machines, add_state_machine, set_active_state_machine, remove_state_machine,
  list_states, add_state, add_global_state, remove_state, update_state, set_state_actions,
  set_initial_state, add_transition, remove_transition, list_interactions,
  add_pointer_interaction, add_state_interaction, remove_interaction, list_inputs, add_input,
  update_input, remove_input
- **Assets:** import_asset (SVG/Lottie/image), vectorize_image

Design notes visible in tool descriptions: integer frames only (no sub-frame keys), layer ids
from read_scene, agents told to verify visually via frames.

## 5. Lottie Editor (editor.lottiefiles.com) [O]

Lightweight editor for existing Lotties: lottie-web 5.12.2 renderer; generator
`@lottiefiles/toolkit-js` 0.33.2. Layer tree with thumbnails, per-layer visibility/delete and
solo-highlight; **Unique colors** palette (global recolor) + All colors + Attributes per layer;
Settings (dimension W/H lock, duration, frame rate); Info (JSON size, Bodymovin version,
generator, lottie-web version); canvas toggles (bounds, transparency grid, …, colour picker),
zoom, speed, loop, frame scrubber; undo/redo; metered "10 edits available"; Save (cloud).

## 6. Gaps / unknowns
- Renderer submenu options not captured. Plugin API surface not inspected (QuickJS sandbox
  inferred). Expression support level unknown. Version history UI not opened (cloud).
- Exact WebMCP tool registration not observed (`navigator.modelContext` absent in this Chrome).
