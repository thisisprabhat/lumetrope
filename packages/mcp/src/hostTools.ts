// Tools implemented by the app itself (editor state, rendering, history,
// state machine preview) in addition to the engine's document catalogue.
export const HOST_TOOLS = [
  {
    name: "get_editor_state",
    description: "Current editor state: file name, mode, playhead frame, selected layers, scene being edited, and whether there are unsaved versions.",
    inputSchema: { type: "object", properties: {}, additionalProperties: false },
  },
  {
    name: "get_frame_image",
    description: "Render a frame of the active animation to a PNG so you can SEE the result. Use it after edits to verify layout, colours and motion.",
    inputSchema: { type: "object", properties: { frame: { type: "number", description: "Frame (default: playhead)" }, width: { type: "number", description: "Pixel width (default 512, max 2048)" } }, additionalProperties: false },
  },
  {
    name: "set_current_frame",
    description: "Move the editor's playhead (also the frame new keyframes are added at by the UI).",
    inputSchema: { type: "object", properties: { frame: { type: "number" } }, required: ["frame"], additionalProperties: false },
  },
  { name: "undo", description: "Undo the last change.", inputSchema: { type: "object", properties: {}, additionalProperties: false } },
  { name: "redo", description: "Redo the last undone change.", inputSchema: { type: "object", properties: {}, additionalProperties: false } },
  {
    name: "save_version",
    description: "Save the current document as a named version in the local history.",
    inputSchema: { type: "object", properties: { message: { type: "string" } }, required: ["message"], additionalProperties: false },
  },
  { name: "list_versions", description: "List saved versions (newest first) on the current branch.", inputSchema: { type: "object", properties: {}, additionalProperties: false } },
  {
    name: "restore_version",
    description: "Restore a saved version by id (from list_versions). Unsaved work is first kept as an Auto-saved version.",
    inputSchema: { type: "object", properties: { id: { type: "string" } }, required: ["id"], additionalProperties: false },
  },
  {
    name: "preview_state_machine",
    description: "Start running a state machine in the preview so you can test it with state_machine_input / state_machine_pointer / state_machine_advance.",
    inputSchema: { type: "object", properties: { machine: { type: "string" } }, required: ["machine"], additionalProperties: false },
  },
  {
    name: "state_machine_input",
    description: "Set an input of the running state machine (boolean/number/string) or fire an event input (value omitted/null). Returns the new state.",
    inputSchema: { type: "object", properties: { name: { type: "string" }, value: {} }, required: ["name"], additionalProperties: false },
  },
  {
    name: "state_machine_pointer",
    description: "Simulate a pointer event (click, down, up, move, enter, exit) at scene coordinates on the running state machine.",
    inputSchema: { type: "object", properties: { kind: { type: "string", enum: ["click", "down", "up", "move", "enter", "exit"] }, x: { type: "number" }, y: { type: "number" } }, required: ["kind", "x", "y"], additionalProperties: false },
  },
  {
    name: "state_machine_advance",
    description: "Advance the running state machine by some seconds (lets segments finish and OnComplete fire). Returns the state.",
    inputSchema: { type: "object", properties: { seconds: { type: "number", exclusiveMinimum: 0, maximum: 60 } }, required: ["seconds"], additionalProperties: false },
  },
  { name: "stop_state_machine_preview", description: "Stop the running state machine preview.", inputSchema: { type: "object", properties: {}, additionalProperties: false } },
  {
    name: "iconify_search",
    description: "Search the public Iconify catalogue (api.iconify.design) for icons. Returns prefix:name ids. Use iconify_import to place one into the open animation.",
    inputSchema: {
      type: "object",
      properties: {
        query: { type: "string", description: "Search words, such as 'house' or 'arrow right'" },
        limit: { type: "number", description: "Page size, 1–64 (default 32)" },
        prefix: { type: "string", description: "Optional icon set, such as 'lucide' or 'mdi'" },
      },
      required: ["query"],
      additionalProperties: false,
    },
  },
  {
    name: "iconify_import",
    description: "Download one Iconify icon (prefix:name) as SVG and import it as an editable shape layer. The icon is embedded in the file. Optional color is a hex fill (default black). Optional position is [x, y] in the scene.",
    inputSchema: {
      type: "object",
      properties: {
        icon: { type: "string", description: "Icon id, such as 'lucide:house'" },
        color: { type: "string", description: "Hex fill baked into the SVG, default #000000" },
        name: { type: "string", description: "Layer name (default: the icon id)" },
        position: { type: "array", items: { type: "number" }, minItems: 2, maxItems: 2, description: "Scene position [x, y]. Omit to centre the icon." },
        scene: { type: "string", description: "Scene id (default: the scene being edited)" },
      },
      required: ["icon"],
      additionalProperties: false,
    },
  },
] as const;
