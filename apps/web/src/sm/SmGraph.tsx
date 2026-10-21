import "@xyflow/react/dist/style.css";
import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Background,
  BackgroundVariant,
  Controls,
  Handle,
  MarkerType,
  Position,
  ReactFlow,
  ReactFlowProvider,
  useReactFlow,
  type Edge,
  type Node,
  type NodeProps,
} from "@xyflow/react";
import { act } from "../actions";
import { projectValue } from "../engine";
import { openContextMenu, SEP } from "../contextMenu";
import { useSceneInfo } from "../doc";
import { autoLayout, guardSummary, type Guard } from "../lib/sm";
import { useEditor } from "../store";

export interface SmState {
  name: string;
  type: "PlaybackState" | "GlobalState";
  segment?: string;
  loop?: boolean;
  autoplay?: boolean;
  mode?: string;
  speed?: number;
  final?: boolean;
  transitions: { type: string; toState: string; guards?: Guard[]; duration?: number; easing?: number[] }[];
  entryActions?: any[];
  exitActions?: any[];
}

export interface SmDoc {
  initial: string;
  states: SmState[];
  inputs: { type: string; name: string; value?: unknown }[];
  interactions: any[];
  lmeta?: { name?: string; layout?: Record<string, [number, number]> };
}

/** Raw machine JSON (with editor layout) for the selected machine. */
export function useMachineDoc(): SmDoc | null {
  const revision = useEditor((s) => s.revision);
  const machine = useEditor((s) => s.machine);
  return useMemo(() => {
    if (!machine) return null;
    // The raw project copy keeps the editor-only node layout (lmeta).
    const raw = projectValue().stateMachines?.[machine];
    return raw ? ({ initial: "", states: [], inputs: [], interactions: [], ...raw, lmeta: raw.lmeta ?? { layout: {} } } as SmDoc) : null;
  }, [machine, revision]);
}

type StateNodeData = { st: SmState; initial: boolean; live: boolean };

function StateNode({ data, selected }: NodeProps<Node<StateNodeData>>) {
  const { st, initial, live } = data;
  const global = st.type === "GlobalState";
  return (
    <div className={`sm-node ${global ? "global" : ""} ${selected ? "sel" : ""} ${live ? "live" : ""}`}>
      {!global && <Handle type="target" position={Position.Left} />}
      <div className="sm-node-head">
        {initial && <span className="sm-initial" title="Initial state">▶</span>}
        <span className="sm-node-name">{st.name}</span>
      </div>
      {!global ? (
        <div className="sm-node-meta">
          <span>{st.segment ?? "whole animation"}</span>
          <span>
            {st.loop ? "loop" : "once"}
            {st.mode && st.mode !== "Forward" ? ` · ${st.mode}` : ""}
            {st.speed && st.speed !== 1 ? ` · ${st.speed}×` : ""}
            {st.final ? " · final" : ""}
          </span>
        </div>
      ) : (
        <div className="sm-node-meta">
          <span>Checked from every state</span>
        </div>
      )}
      <Handle type="source" position={Position.Right} />
    </div>
  );
}

const nodeTypes = { state: StateNode };

function Graph() {
  const doc = useMachineDoc();
  const machine = useEditor((s) => s.machine)!;
  const smSel = useEditor((s) => s.smSel);
  const setSmSel = useEditor((s) => s.setSmSel);
  const live = useEditor((s) => s.smSnap?.state);
  const scene = useSceneInfo();
  const rf = useReactFlow();
  const [dragOver, setDragOver] = useState(false);

  const globalName = doc?.states.find((s) => s.type === "GlobalState")?.name;
  const positions = useMemo(
    () => (doc ? autoLayout(doc.states.map((s) => s.name), doc.lmeta?.layout ?? {}, globalName, doc.initial, doc.states.flatMap((s) => s.transitions.map((t) => [s.name, t.toState] as [string, string]))) : {}),
    [doc, globalName],
  );

  const nodes: Node<StateNodeData>[] = useMemo(
    () =>
      (doc?.states ?? []).map((st) => ({
        id: st.name,
        type: "state",
        position: { x: positions[st.name]?.[0] ?? 0, y: positions[st.name]?.[1] ?? 0 },
        data: { st, initial: doc!.initial === st.name, live: live === st.name },
        selected: smSel?.kind === "state" && smSel.name === st.name,
      })),
    [doc, positions, live, smSel],
  );

  const edges: Edge[] = useMemo(
    () =>
      (doc?.states ?? []).flatMap((st) =>
        st.transitions.map((t, i) => ({
          id: `${st.name}::${i}`,
          source: st.name,
          target: t.toState,
          label: `${t.type === "Tweened" ? "⤳ " : ""}${guardSummary(t.guards)}`,
          animated: t.type === "Tweened",
          selected: smSel?.kind === "transition" && smSel.from === st.name && smSel.index === i,
          markerEnd: { type: MarkerType.ArrowClosed, width: 16, height: 16 },
          className: st.type === "GlobalState" ? "global-edge" : undefined,
          type: "smoothstep",
        })),
      ),
    [doc, smSel],
  );

  const [localNodes, setLocalNodes] = useState(nodes);
  useEffect(() => setLocalNodes(nodes), [nodes]);

  const addState = useCallback(
    (name: string, extra: object, at?: { x: number; y: number }) => {
      const taken = new Set(doc?.states.map((s) => s.name));
      let n = name;
      for (let i = 2; taken.has(n); i++) n = `${name} ${i}`;
      act("add_state", { machine, name: n, ...extra, ...(at ? { position: [Math.round(at.x), Math.round(at.y)] } : {}) });
      setSmSel({ kind: "state", name: n });
    },
    [doc, machine, setSmSel],
  );

  if (!doc) return null;
  return (
    <div
      className={`sm-graph ${dragOver ? "drag" : ""}`}
      onDragOver={(e) => {
        if (e.dataTransfer.types.includes("text/segment")) {
          e.preventDefault();
          setDragOver(true);
        }
      }}
      onDragLeave={() => setDragOver(false)}
      onDrop={(e) => {
        setDragOver(false);
        const seg = e.dataTransfer.getData("text/segment");
        if (seg === undefined || seg === "") return;
        const at = rf.screenToFlowPosition({ x: e.clientX, y: e.clientY });
        addState(seg === "*" ? "Playback" : seg, seg === "*" ? { loop: true } : { segment: seg, loop: true }, at);
      }}
    >
      <ReactFlow
        nodes={localNodes}
        edges={edges}
        nodeTypes={nodeTypes}
        fitView
        fitViewOptions={{ padding: 0.3, maxZoom: 1.2 }}
        proOptions={{ hideAttribution: true }}
        deleteKeyCode={null}
        onNodesChange={(changes) =>
          setLocalNodes((ns) =>
            ns.map((n) => {
              const c = changes.find((x) => "id" in x && x.id === n.id && x.type === "position");
              return c && c.type === "position" && c.position ? { ...n, position: c.position } : n;
            }),
          )
        }
        onNodeDragStop={(_, n) => act("update_state", { machine, name: n.id, position: [Math.round(n.position.x), Math.round(n.position.y)] })}
        onNodeClick={(_, n) => setSmSel({ kind: "state", name: n.id })}
        onEdgeClick={(_, e) => {
          const [from, idx] = e.id.split("::");
          setSmSel({ kind: "transition", from, index: Number(idx) });
        }}
        onPaneClick={() => setSmSel(null)}
        onConnect={(c) => {
          if (!c.source || !c.target || c.source === c.target) return;
          const r = act<{ index: number }>("add_transition", { machine, from: c.source, to: c.target });
          if (r) setSmSel({ kind: "transition", from: c.source, index: r.index });
        }}
        onNodeContextMenu={(e, n) => {
          setSmSel({ kind: "state", name: n.id });
          const st = doc.states.find((s) => s.name === n.id)!;
          openContextMenu(e, [
            { label: "Set as initial state", disabled: st.type === "GlobalState" || doc.initial === st.name, onSelect: () => act("set_initial_state", { machine, name: st.name }) },
            {
              label: "Add transition to",
              submenu: doc.states.filter((s) => s.name !== st.name && s.type !== "GlobalState").map((s) => ({ label: s.name, onSelect: () => act("add_transition", { machine, from: st.name, to: s.name }) })),
            },
            { label: st.loop ? "Play once" : "Loop", disabled: st.type === "GlobalState", onSelect: () => act("update_state", { machine, name: st.name, loop: !st.loop }) },
            SEP,
            { label: "Delete state", danger: true, onSelect: () => (act("remove_state", { machine, name: st.name }), setSmSel(null)) },
          ]);
        }}
        onEdgeContextMenu={(e, edge) => {
          const [from, idx] = edge.id.split("::");
          setSmSel({ kind: "transition", from, index: Number(idx) });
          openContextMenu(e, [
            { label: "Make instant", onSelect: () => act("update_transition", { machine, from, index: Number(idx), type: "instant" }) },
            { label: "Make tweened (0.3 s)", onSelect: () => act("update_transition", { machine, from, index: Number(idx), type: "tweened", duration: 0.3 }) },
            SEP,
            { label: "Delete transition", danger: true, onSelect: () => (act("remove_transition", { machine, from, index: Number(idx) }), setSmSel(null)) },
          ]);
        }}
        onPaneContextMenu={(e) => {
          const at = rf.screenToFlowPosition({ x: e.clientX, y: e.clientY });
          openContextMenu(e as React.MouseEvent, [
            {
              label: "Add state for segment",
              submenu: [
                { label: "Whole animation", onSelect: () => addState("Playback", { loop: true }, at) },
                ...scene.segments.map((s) => ({ label: s.name, onSelect: () => addState(s.name, { segment: s.name, loop: true }, at) })),
              ],
            },
            { label: "Add global state", disabled: !!globalName, onSelect: () => addState("Global", { type: "global" }, at) },
            SEP,
            { label: "Fit graph", onSelect: () => rf.fitView({ padding: 0.3 }) },
          ]);
        }}
      >
        <Background variant={BackgroundVariant.Dots} gap={18} size={1} color="#2c3138" />
        <Controls showInteractive={false} position="top-right" />
      </ReactFlow>
      {doc.states.length === 0 && <div className="sm-empty">Drag a segment from the left panel here, or right-click to add a state.</div>}
    </div>
  );
}

export function SmGraph() {
  const machine = useEditor((s) => s.machine);
  if (!machine) return <div className="panel-empty">Create or select a state machine on the left to start wiring interactivity.</div>;
  return (
    <ReactFlowProvider>
      <Graph />
    </ReactFlowProvider>
  );
}
