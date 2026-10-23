import "@xyflow/react/dist/style.css";
import "./graph.css";
import { memo, useCallback, useEffect, useMemo, useState } from "react";
import {
  applyNodeChanges,
  Background,
  BaseEdge,
  EdgeLabelRenderer,
  getBezierPath,
  type EdgeProps,
  BackgroundVariant,
  Controls,
  Handle,
  MarkerType,
  MiniMap,
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

type Kind = "initial" | "state" | "final" | "global";
type StateNodeData = { st: SmState; initial: boolean; live: boolean; kind: Kind };

/** Accent per kind of state, shared by nodes, edges and the minimap. */
const KIND_COLOR: Record<Kind, string> = { initial: "#ffb340", state: "#6fd3ff", final: "#7ee2a8", global: "#b69cff" };
const KIND_LABEL: Record<Kind, string> = { initial: "Start", state: "State", final: "Final", global: "Global" };

function kindOf(st: SmState, initial: string): Kind {
  if (st.type === "GlobalState") return "global";
  if (st.name === initial) return "initial";
  if (st.final) return "final";
  return "state";
}

/** Colour of a transition's guard chip: by the kind of input it checks. */
function guardColor(guards: Guard[] | undefined, inputs: SmDoc["inputs"]): string {
  const g = guards?.[0];
  if (!g) return "#8b939d";
  if (g.type === "Event") return "#ffb340";
  const t = inputs.find((i) => i.name === g.inputName)?.type ?? g.type;
  return t === "Boolean" ? "#7ee2a8" : t === "Numeric" ? "#6fa8ff" : t === "String" ? "#ff8fc7" : "#6fd3ff";
}

const StateNode = memo(function StateNode({ data, selected }: NodeProps<Node<StateNodeData>>) {
  const { st, live, kind } = data;
  const global = kind === "global";
  return (
    <div className={`smn kind-${kind} ${selected ? "sel" : ""} ${live ? "live" : ""}`} style={{ "--k": KIND_COLOR[kind] } as React.CSSProperties}>
      {!global && <Handle type="target" position={Position.Left} className="smn-handle" />}
      <div className="smn-head">
        <span className="smn-badge">{KIND_LABEL[kind]}</span>
        <span className="smn-name" title={st.name}>
          {st.name}
        </span>
        {live && <span className="smn-live">LIVE</span>}
      </div>
      <div className="smn-body">
        {global ? (
          <span className="smn-chip">Checked from every state</span>
        ) : (
          <>
            <span className="smn-chip smn-seg">{st.segment ?? "Whole animation"}</span>
            <span className="smn-chip">{st.loop ? "↻ loop" : "→ once"}</span>
            {st.mode && st.mode !== "Forward" && <span className="smn-chip">{st.mode}</span>}
            {!!st.speed && st.speed !== 1 && <span className="smn-chip">{st.speed}×</span>}
          </>
        )}
      </div>
      <Handle type="source" position={Position.Right} className="smn-handle" />
    </div>
  );
});

type SmEdgeData = { label: string; chip: string; color: string; offset: number; from: string; index: number };

/** Bezier transition with an HTML guard pill; two-way and parallel transitions get their pills pushed apart. */
const SmEdge = memo(function SmEdge(p: EdgeProps<Edge<SmEdgeData>>) {
  const [path, lx, ly] = getBezierPath(p);
  const d = p.data!;
  const dx = p.targetX - p.sourceX;
  const dy = p.targetY - p.sourceY;
  const len = Math.hypot(dx, dy) || 1;
  const x = lx + (-dy / len) * d.offset;
  const y = ly + (dx / len) * d.offset;
  return (
    <>
      <BaseEdge id={p.id} path={path} markerEnd={p.markerEnd} style={p.style} interactionWidth={18} />
      <EdgeLabelRenderer>
        <button
          className={`sm-edge-label nodrag nopan ${p.selected ? "sel" : ""}`}
          style={{ transform: `translate(-50%, -50%) translate(${x}px, ${y}px)`, "--c": d.chip, "--e": d.color } as React.CSSProperties}
          onClick={() => useEditor.getState().setSmSel({ kind: "transition", from: d.from, index: d.index })}
          onContextMenu={(e) => {
            e.preventDefault();
            useEditor.getState().setSmSel({ kind: "transition", from: d.from, index: d.index });
          }}
        >
          {d.label}
        </button>
      </EdgeLabelRenderer>
    </>
  );
});

const edgeTypes = { sm: SmEdge };

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

  // Nodes derived from the document. They are merged into the nodes xyflow already
  // measured (size, selection, drag state) so nothing is re-measured or hidden.
  const derived: Node<StateNodeData>[] = useMemo(
    () =>
      (doc?.states ?? []).map((st) => ({
        id: st.name,
        type: "state",
        position: { x: positions[st.name]?.[0] ?? 0, y: positions[st.name]?.[1] ?? 0 },
        data: { st, initial: doc!.initial === st.name, live: live === st.name, kind: kindOf(st, doc!.initial) },
      })),
    [doc, positions, live],
  );
  const [nodes, setNodes] = useState<Node<StateNodeData>[]>(derived);
  useEffect(() => {
    setNodes((prev) => {
      const byId = new Map(prev.map((n) => [n.id, n]));
      return derived.map((d) => {
        const p = byId.get(d.id);
        return p ? { ...p, data: d.data, position: p.dragging ? p.position : d.position } : d;
      });
    });
  }, [derived]);
  useEffect(() => {
    setNodes((prev) =>
      prev.map((n) => {
        const sel = smSel?.kind === "state" && smSel.name === n.id;
        return n.selected === sel ? n : { ...n, selected: sel };
      }),
    );
  }, [smSel]);

  const edges: Edge<SmEdgeData>[] = useMemo(() => {
    const inputs = doc?.inputs ?? [];
    const seen = new Map<string, number>();
    return (doc?.states ?? []).flatMap((st) =>
      st.transitions.map((t, i) => {
        const fromGlobal = st.type === "GlobalState";
        const tweened = t.type === "Tweened";
        const color = fromGlobal ? KIND_COLOR.global : tweened ? KIND_COLOR.initial : "#7f8fa0";
        const chip = guardColor(t.guards, inputs);
        // Spread the pills of parallel and two-way transitions between the same pair of states.
        const pair = [st.name, t.toState].sort().join("\u0000");
        const nth = (seen.get(pair) ?? 0) + 1;
        seen.set(pair, nth);
        const offset = (nth % 2 ? 1 : -1) * Math.ceil(nth / 2) * 16 * (st.name < t.toState ? 1 : -1);
        return {
          id: `${st.name}::${i}`,
          source: st.name,
          target: t.toState,
          type: "sm",
          animated: tweened,
          selected: smSel?.kind === "transition" && smSel.from === st.name && smSel.index === i,
          markerEnd: { type: MarkerType.ArrowClosed, width: 18, height: 18, color },
          style: { stroke: color, strokeWidth: 1.75 },
          className: fromGlobal ? "global-edge" : tweened ? "tween-edge" : undefined,
          data: { label: `${tweened ? "⤳ " : ""}${guardSummary(t.guards)}`, chip, color, offset, from: st.name, index: i },
        } satisfies Edge<SmEdgeData>;
      }),
    );
  }, [doc, smSel]);

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
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        edgeTypes={edgeTypes}
        fitView
        fitViewOptions={{ padding: 0.3, maxZoom: 1.2 }}
        proOptions={{ hideAttribution: true }}
        deleteKeyCode={null}
        minZoom={0.2}
        maxZoom={2.5}
        elevateEdgesOnSelect
        onlyRenderVisibleElements
        onNodesChange={(changes) => setNodes((ns) => applyNodeChanges(changes, ns))}
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
        <Background variant={BackgroundVariant.Dots} gap={20} size={1.4} color="#323a44" />
        <Controls showInteractive={false} position="top-right" />
        <MiniMap
          position="bottom-right"
          pannable
          zoomable
          nodeColor={(n) => KIND_COLOR[(n.data as StateNodeData).kind] ?? "#6fd3ff"}
          nodeStrokeWidth={0}
          nodeBorderRadius={6}
          maskColor="rgba(9, 10, 12, 0.72)"
          style={{ width: 150, height: 96 }}
        />
        <div className="sm-legend" aria-hidden>
          {(Object.keys(KIND_COLOR) as Kind[]).map((k) => (
            <span key={k} style={{ "--k": KIND_COLOR[k] } as React.CSSProperties}>
              {KIND_LABEL[k]}
            </span>
          ))}
          <span className="tween">Tweened</span>
        </div>
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
