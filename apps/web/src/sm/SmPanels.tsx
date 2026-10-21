import { useEffect, useMemo, useState } from "react";
import { act, query } from "../actions";
import { Icon } from "../components/Icon";
import { useSceneInfo } from "../doc";
import { ACTION_TYPES, CONDITIONS, INTERACTION_TYPES, actionSummary, toGuardArgs, type Action, type Guard } from "../lib/sm";
import { useEditor } from "../store";
import { NumberField, Section, Select, Toggle } from "../ui/controls";
import { useMachineDoc, type SmDoc } from "./SmGraph";
import { smSetInput } from "./useSmPreview";

// ------------------------------------------------------------------ left

export function MachinesPanel() {
  const revision = useEditor((s) => s.revision);
  const { machine, setMachine } = useEditor();
  const scene = useSceneInfo();
  const list = useMemo(() => query<{ id: string; name: string; states: number }[]>("list_state_machines", {}, []), [revision]);
  useEffect(() => {
    if (!machine && list.length) setMachine(list[0].id);
  }, [machine, list, setMachine]);
  return (
    <div className="leftbar-body sm-left">
      <div className="panel-head">
        <span>State machines</span>
        <button
          className="icon-btn"
          data-tip="New state machine"
          onClick={() => {
            const r = act<{ machine: string }>("add_state_machine", {});
            if (r) setMachine(r.machine);
          }}
        >
          <Icon name="plus" size={14} />
        </button>
      </div>
      <div className="panel-pad">
        {list.length === 0 && <div className="muted tiny">No state machines yet. Create one, then drag segments into the graph to make states.</div>}
        {list.map((m) => (
          <div key={m.id} className={`asset-row ${machine === m.id ? "on" : ""}`} onClick={() => setMachine(m.id)}>
            <Icon name="graph" size={13} />
            <input className="text-input inline" key={m.name} defaultValue={m.name} onClick={(e) => e.stopPropagation()} onBlur={(e) => e.target.value !== m.name && act("rename_state_machine", { machine: m.id, name: e.target.value })} />
            <span className="muted tiny">{m.states}</span>
            <button
              className="icon-btn mini"
              title="Delete"
              onClick={(e) => {
                e.stopPropagation();
                act("remove_state_machine", { machine: m.id });
                setMachine(null);
              }}
            >
              <Icon name="trash" size={13} />
            </button>
          </div>
        ))}
        <div className="subhead">Segments — drag into the graph</div>
        <div className="seg-chips">
          <span className="seg-chip all" draggable onDragStart={(e) => e.dataTransfer.setData("text/segment", "*")}>
            Whole animation
          </span>
          {scene.segments.map((s) => (
            <span key={s.name} className="seg-chip" draggable onDragStart={(e) => e.dataTransfer.setData("text/segment", s.name)} title={`${s.start}–${s.end}`}>
              {s.name}
            </span>
          ))}
        </div>
        {scene.segments.length === 0 && <div className="muted tiny">Create segments in Animate mode (Segments panel) to give states their own frame ranges.</div>}
      </div>
    </div>
  );
}

// ------------------------------------------------------------------ editors

function valueField(type: string | undefined, value: unknown, onChange: (v: unknown) => void, inputs: SmDoc["inputs"]) {
  const refs = inputs.filter((i) => (type === "Numeric" ? i.type === "Numeric" : type === "String" ? i.type === "String" : i.type === "Boolean")).map((i) => `$${i.name}`);
  if (typeof value === "string" && value.startsWith("$"))
    return <Select value={value} options={[...refs, "value"].map((r) => ({ value: r, label: r === "value" ? "fixed value…" : r }))} onChange={(v) => onChange(v === "value" ? (type === "Numeric" ? 0 : type === "Boolean" ? true : "") : v)} />;
  const refSel = refs.length > 0 && (
    <select className="mini-select" value="" onChange={(e) => e.target.value && onChange(e.target.value)} title="Compare with another input">
      <option value="">$</option>
      {refs.map((r) => (
        <option key={r} value={r}>
          {r}
        </option>
      ))}
    </select>
  );
  if (type === "Boolean")
    return (
      <>
        <Select value={String(value ?? true)} options={[{ value: "true", label: "true" }, { value: "false", label: "false" }]} onChange={(v) => onChange(v === "true")} />
        {refSel}
      </>
    );
  if (type === "Numeric")
    return (
      <>
        <NumberField value={Number(value) || 0} onChange={onChange} precision={2} />
        {refSel}
      </>
    );
  return (
    <>
      <input className="text-input" defaultValue={String(value ?? "")} key={String(value)} onBlur={(e) => onChange(e.target.value)} />
      {refSel}
    </>
  );
}

function GuardsEditor({ guards, inputs, onChange }: { guards: Guard[]; inputs: SmDoc["inputs"]; onChange: (g: ReturnType<typeof toGuardArgs>) => void }) {
  const args = toGuardArgs(guards);
  const set = (i: number, patch: object) => onChange(args.map((g, j) => (j === i ? { ...g, ...patch } : g)));
  return (
    <div className="stack">
      {args.length === 0 && <div className="muted tiny">No conditions: fires when the state finishes playing.</div>}
      {args.map((g, i) => {
        const def = inputs.find((x) => x.name === g.input);
        return (
          <div key={i} className="item-card">
            <div className="row">
              <Select value={g.input} options={inputs.map((x) => ({ value: x.name, label: `${x.name} · ${x.type}` }))} onChange={(input) => set(i, { input, condition: "equal", value: inputs.find((x) => x.name === input)?.type === "Boolean" ? true : 0 })} />
              <button className="icon-btn mini" onClick={() => onChange(args.filter((_, j) => j !== i))}>
                <Icon name="trash" size={13} />
              </button>
            </div>
            {def && def.type !== "Event" && (
              <div className="row">
                <Select
                  value={(g.condition ?? "equal") as string}
                  options={(def.type === "Numeric" ? CONDITIONS : ["Equal", "NotEqual"]).map((c) => ({ value: c.replace(/([a-z])([A-Z])/g, "$1_$2").toLowerCase(), label: c.replace(/([a-z])([A-Z])/g, "$1 $2").toLowerCase() }))}
                  onChange={(condition) => set(i, { condition })}
                />
                {valueField(def.type, g.value, (value) => set(i, { value }), inputs)}
              </div>
            )}
            {def?.type === "Event" && <div className="muted tiny">Passes when “{g.input}” is fired.</div>}
          </div>
        );
      })}
      <button className="chip-btn" disabled={!inputs.length} onClick={() => onChange([...args, inputs[0].type === "Event" ? { input: inputs[0].name } : { input: inputs[0].name, condition: "equal", value: inputs[0].type === "Boolean" ? true : inputs[0].type === "Numeric" ? 0 : "" }])}>
        + Condition
      </button>
      {!inputs.length && <div className="muted tiny">Add inputs first (Inputs tab).</div>}
    </div>
  );
}

export function ActionsEditor({ actions, onChange, inputs }: { actions: Action[]; onChange: (a: Action[]) => void; inputs: SmDoc["inputs"] }) {
  const themes = useMemo(() => query<{ id: string; name: string }[]>("list_themes", {}, []), []);
  const set = (i: number, a: Action) => onChange(actions.map((x, j) => (j === i ? a : x)));
  return (
    <div className="stack">
      {actions.map((a, i) => {
        const needsInput = ["Toggle", "SetBoolean", "SetNumeric", "SetString", "Increment", "Decrement", "Fire", "Reset"].includes(a.type);
        const inputType = a.type === "Toggle" || a.type === "SetBoolean" ? "Boolean" : a.type === "SetString" ? "String" : a.type === "Fire" ? "Event" : a.type === "Reset" ? undefined : "Numeric";
        const candidates = inputs.filter((x) => !inputType || x.type === inputType);
        return (
          <div key={i} className="item-card">
            <div className="row">
              <Select value={a.type} options={ACTION_TYPES.map((t) => ({ value: t, label: t.replace(/([a-z])([A-Z])/g, "$1 $2") }))} onChange={(type) => set(i, { type })} />
              <button className="icon-btn mini" onClick={() => onChange(actions.filter((_, j) => j !== i))}>
                <Icon name="trash" size={13} />
              </button>
            </div>
            {needsInput && <Select value={a.inputName ?? ""} options={[{ value: "", label: "— input —" }, ...candidates.map((x) => ({ value: x.name, label: x.name }))]} onChange={(inputName) => set(i, { ...a, inputName })} />}
            {["SetBoolean", "SetNumeric", "SetString", "Increment", "Decrement"].includes(a.type) && <div className="row">{valueField(a.type === "SetBoolean" ? "Boolean" : a.type === "SetString" ? "String" : "Numeric", a.value ?? (a.type === "SetBoolean" ? true : a.type.startsWith("Set") ? 0 : 1), (value) => set(i, { ...a, value }), inputs)}</div>}
            {a.type === "SetFrame" && <NumberField label="Frame" value={Number(a.value) || 0} precision={0} onChange={(value) => set(i, { ...a, value })} />}
            {a.type === "SetProgress" && <NumberField label="%" value={(Number(a.value) || 0) * 100} min={0} max={100} precision={0} onChange={(v) => set(i, { ...a, value: v / 100 })} />}
            {a.type === "SetTheme" && <Select value={String(a.value ?? "")} options={[{ value: "", label: "— theme —" }, ...themes.map((t) => ({ value: t.id, label: t.name }))]} onChange={(value) => set(i, { ...a, value })} />}
            {a.type === "OpenUrl" && (
              <div className="row">
                <input className="text-input" placeholder="https://…" defaultValue={a.url ?? ""} onBlur={(e) => set(i, { ...a, url: e.target.value })} />
                <Select value={a.target ?? "_blank"} options={["_blank", "_self", "_parent", "_top"].map((t) => ({ value: t, label: t }))} onChange={(target) => set(i, { ...a, target })} />
              </div>
            )}
            {a.type === "FireCustomEvent" && <input className="text-input" placeholder="event name" defaultValue={String(a.value ?? "")} onBlur={(e) => set(i, { ...a, value: e.target.value })} />}
            <div className="muted tiny">{actionSummary(a)}</div>
          </div>
        );
      })}
      <button className="chip-btn" onClick={() => onChange([...actions, inputs.find((x) => x.type === "Boolean") ? { type: "Toggle", inputName: inputs.find((x) => x.type === "Boolean")!.name } : { type: "FireCustomEvent", value: "event" }])}>
        + Action
      </button>
    </div>
  );
}

/** Actions must be valid before the engine accepts them; keep incomplete ones local. */
function validActions(a: Action[]): boolean {
  return a.every((x) => !["Toggle", "SetBoolean", "SetNumeric", "SetString", "Increment", "Decrement", "Fire", "Reset"].includes(x.type) || !!x.inputName) && a.every((x) => x.type !== "OpenUrl" || !!x.url);
}

function useDraftActions(initial: Action[], save: (a: Action[]) => void): [Action[], (a: Action[]) => void] {
  const [draft, setDraft] = useState<Action[] | null>(null);
  const cur = draft ?? initial;
  return [
    cur,
    (a) => {
      if (validActions(a)) {
        setDraft(null);
        save(a);
      } else setDraft(a);
    },
  ];
}

// ------------------------------------------------------------------ right

function InputsTab({ doc, machine }: { doc: SmDoc; machine: string }) {
  const [name, setName] = useState("");
  const [type, setType] = useState("boolean");
  return (
    <div className="panel-pad">
      {doc.inputs.length === 0 && <div className="muted tiny">Inputs are the variables that drive transitions. Interactions and actions change them; your app can set them too.</div>}
      {doc.inputs.map((i) => (
        <div key={i.name} className="item-card">
          <div className="item-card-head">
            <input className="text-input inline" defaultValue={i.name} key={i.name} onBlur={(e) => e.target.value !== i.name && act("update_input", { machine, name: i.name, new_name: e.target.value })} />
            <span className="type-badge">{i.type}</span>
            <button className="icon-btn mini" onClick={() => act("remove_input", { machine, name: i.name })}>
              <Icon name="trash" size={13} />
            </button>
          </div>
          {i.type !== "Event" && (
            <div className="row">
              <span className="muted tiny">Default</span>
              {i.type === "Boolean" ? (
                <Toggle label="" value={!!i.value} onChange={(value) => act("update_input", { machine, name: i.name, value })} />
              ) : i.type === "Numeric" ? (
                <NumberField value={Number(i.value) || 0} precision={2} onChange={(value) => act("update_input", { machine, name: i.name, value })} />
              ) : (
                <input className="text-input" defaultValue={String(i.value ?? "")} key={String(i.value)} onBlur={(e) => act("update_input", { machine, name: i.name, value: e.target.value })} />
              )}
            </div>
          )}
        </div>
      ))}
      <div className="row">
        <input className="text-input" placeholder="input name" value={name} onChange={(e) => setName(e.target.value)} />
        <Select value={type} options={["boolean", "numeric", "string", "event"].map((t) => ({ value: t, label: t }))} onChange={setType} />
      </div>
      <button className="btn" disabled={!name} onClick={() => act("add_input", { machine, name, type }) !== undefined && setName("")}>
        Add input
      </button>
    </div>
  );
}

function InteractionCard({ it, index, doc, machine }: { it: any; index: number; doc: SmDoc; machine: string }) {
  const scene = useSceneInfo();
  const layerNames = Array.from(new Set(scene.layers.map((l) => l.name).filter(Boolean)));
  const [actions, setActions] = useDraftActions(it.actions ?? [], (a) => act("update_interaction", { machine, index, actions: a }));
  const isPlayback = it.type === "OnComplete" || it.type === "OnLoopComplete";
  return (
    <div className="item-card">
      <div className="row">
        <Select value={it.type} options={INTERACTION_TYPES.map((t) => ({ value: t, label: t.replace(/([a-z])([A-Z])/g, "$1 $2") }))} onChange={(type) => act("update_interaction", { machine, index, type, ...(type === "OnComplete" || type === "OnLoopComplete" ? { state: it.stateName ?? doc.states.find((s) => s.type !== "GlobalState")?.name } : {}) })} />
        <button className="icon-btn mini" onClick={() => act("remove_interaction", { machine, index })}>
          <Icon name="trash" size={13} />
        </button>
      </div>
      {isPlayback ? (
        <Select value={it.stateName ?? ""} options={doc.states.filter((s) => s.type !== "GlobalState").map((s) => ({ value: s.name, label: `when “${s.name}” ${it.type === "OnComplete" ? "completes" : "loops"}` }))} onChange={(state) => act("update_interaction", { machine, index, state })} />
      ) : (
        <Select value={it.layerName ?? ""} options={[{ value: "", label: "anywhere on the animation" }, ...layerNames.map((n) => ({ value: n, label: `on layer “${n}”` }))]} onChange={(layer) => act("update_interaction", { machine, index, layer: layer || null })} />
      )}
      <ActionsEditor actions={actions} onChange={setActions} inputs={doc.inputs} />
    </div>
  );
}

function InteractionsTab({ doc, machine }: { doc: SmDoc; machine: string }) {
  return (
    <div className="panel-pad">
      {doc.interactions.length === 0 && <div className="muted tiny">Interactions turn clicks, hovers and playback events into actions on inputs.</div>}
      {doc.interactions.map((it, i) => (
        <InteractionCard key={i + JSON.stringify(it)} it={it} index={i} doc={doc} machine={machine} />
      ))}
      <button className="btn" onClick={() => act("add_interaction", { machine, type: "Click", actions: [] })}>
        Add interaction
      </button>
    </div>
  );
}

function StateEditor({ doc, machine, name }: { doc: SmDoc; machine: string; name: string }) {
  const st = doc.states.find((s) => s.name === name);
  const scene = useSceneInfo();
  const [entry, setEntry] = useDraftActions(st?.entryActions ?? [], (a) => act("update_state", { machine, name, entry_actions: a }));
  const [exit, setExit] = useDraftActions(st?.exitActions ?? [], (a) => act("update_state", { machine, name, exit_actions: a }));
  if (!st) return null;
  const upd = (args: object) => act("update_state", { machine, name, ...args });
  const global = st.type === "GlobalState";
  return (
    <div className="panel-pad">
      <div className="row">
        <input className="text-input" key={st.name} defaultValue={st.name} onBlur={(e) => e.target.value !== st.name && upd({ new_name: e.target.value }) !== undefined && useEditor.getState().setSmSel({ kind: "state", name: e.target.value })} />
        {!global && (
          <button className="chip-btn" disabled={doc.initial === st.name} onClick={() => act("set_initial_state", { machine, name })}>
            {doc.initial === st.name ? "▶ Initial" : "Make initial"}
          </button>
        )}
      </div>
      {!global && (
        <>
          <div className="prow">
            <span className="prow-label">Segment</span>
            <Select value={st.segment ?? ""} options={[{ value: "", label: "Whole animation" }, ...scene.segments.map((s) => ({ value: s.name, label: `${s.name} (${s.start}–${s.end})` }))]} onChange={(v) => upd({ segment: v || null })} />
            <span />
          </div>
          <div className="prow">
            <span className="prow-label">Direction</span>
            <Select value={(st.mode ?? "Forward").replace(/([a-z])([A-Z])/g, "$1_$2").toLowerCase()} options={["forward", "reverse", "bounce", "reverse_bounce"].map((m) => ({ value: m, label: m.replace("_", " ") }))} onChange={(mode) => upd({ mode })} />
            <span />
          </div>
          <div className="prow">
            <span className="prow-label">Speed</span>
            <NumberField value={st.speed ?? 1} step={0.05} precision={2} min={0.05} suffix="×" onChange={(speed) => upd({ speed })} />
            <span />
          </div>
          <div className="row wrap">
            <Toggle label="Autoplay" value={!!st.autoplay} onChange={(autoplay) => upd({ autoplay })} />
            <Toggle label="Loop" value={!!st.loop} onChange={(loop) => upd({ loop })} />
            <Toggle label="Final" value={!!st.final} onChange={(f) => upd({ final: f })} />
          </div>
          {st.loop && (
            <div className="prow">
              <span className="prow-label">Loop count</span>
              <NumberField value={(st as any).loopCount ?? 0} min={0} precision={0} onChange={(v) => upd({ loop_count: v > 0 ? v : null })} />
              <span className="muted tiny">0=∞</span>
            </div>
          )}
        </>
      )}
      <div className="subhead">On enter</div>
      <ActionsEditor actions={entry} onChange={setEntry} inputs={doc.inputs} />
      <div className="subhead">On exit</div>
      <ActionsEditor actions={exit} onChange={setExit} inputs={doc.inputs} />
      <div className="subhead">Transitions out</div>
      {st.transitions.map((t, i) => (
        <button key={i} className="asset-row" onClick={() => useEditor.getState().setSmSel({ kind: "transition", from: name, index: i })}>
          → {t.toState} <span className="muted tiny">{t.type === "Tweened" ? "tweened" : "instant"}</span>
        </button>
      ))}
    </div>
  );
}

function TransitionEditor({ doc, machine, from, index }: { doc: SmDoc; machine: string; from: string; index: number }) {
  const t = doc.states.find((s) => s.name === from)?.transitions[index];
  if (!t) return null;
  const upd = (args: object) => act("update_transition", { machine, from, index, ...args });
  const tweened = t.type === "Tweened";
  return (
    <div className="panel-pad">
      <div className="item-card-head">
        <span>
          {from} → {t.toState}
        </span>
        <button className="icon-btn mini" onClick={() => (act("remove_transition", { machine, from, index }), useEditor.getState().setSmSel(null))}>
          <Icon name="trash" size={13} />
        </button>
      </div>
      <div className="prow">
        <span className="prow-label">To</span>
        <Select value={t.toState} options={doc.states.filter((s) => s.type !== "GlobalState").map((s) => ({ value: s.name, label: s.name }))} onChange={(to) => upd({ to })} />
        <span />
      </div>
      <div className="prow">
        <span className="prow-label">Type</span>
        <Select value={tweened ? "tweened" : "instant"} options={[{ value: "instant", label: "Instant" }, { value: "tweened", label: "Tweened" }]} onChange={(type) => upd({ type })} />
        <span />
      </div>
      {tweened && (
        <>
          <div className="prow">
            <span className="prow-label">Duration</span>
            <NumberField value={t.duration ?? 0.5} min={0.01} step={0.05} precision={2} suffix="s" onChange={(duration) => upd({ duration })} />
            <span />
          </div>
          <div className="prow">
            <span className="prow-label">Easing</span>
            <Select value="" options={[{ value: "", label: (t.easing ?? []).map((x) => Number(x).toFixed(2)).join(", ") }, ...["linear", "ease_in", "ease_out", "ease_in_out", "smooth", "ease_out_back"].map((e) => ({ value: e, label: e.replace(/_/g, " ") }))]} onChange={(easing) => easing && upd({ easing })} />
            <span />
          </div>
        </>
      )}
      <div className="subhead">Conditions (all must pass)</div>
      <GuardsEditor guards={t.guards ?? []} inputs={doc.inputs} onChange={(guards) => upd({ guards })} />
    </div>
  );
}

export function SmInspector() {
  const doc = useMachineDoc();
  const machine = useEditor((s) => s.machine);
  const smSel = useEditor((s) => s.smSel);
  const [tab, setTab] = useState<"inputs" | "interactions">("inputs");
  if (!doc || !machine) return <div className="panel-empty">Select a state machine.</div>;
  return (
    <div className="panel-scroll">
      {smSel && (
        <Section title={smSel.kind === "state" ? "State" : "Transition"}>
          {smSel.kind === "state" ? <StateEditor doc={doc} machine={machine} name={smSel.name} /> : <TransitionEditor doc={doc} machine={machine} from={smSel.from} index={smSel.index} />}
        </Section>
      )}
      <div className="tabs">
        <button className={tab === "inputs" ? "on" : ""} onClick={() => setTab("inputs")}>
          Inputs · {doc.inputs.length}
        </button>
        <button className={tab === "interactions" ? "on" : ""} onClick={() => setTab("interactions")}>
          Interactions · {doc.interactions.length}
        </button>
      </div>
      {tab === "inputs" ? <InputsTab doc={doc} machine={machine} /> : <InteractionsTab doc={doc} machine={machine} />}
    </div>
  );
}

// ------------------------------------------------------------------ preview

export function PreviewPanel() {
  const doc = useMachineDoc();
  const snap = useEditor((s) => s.smSnap);
  const machine = useEditor((s) => s.machine);
  const setMachine = useEditor((s) => s.setMachine);
  const revision = useEditor((s) => s.revision);
  const list = useMemo(() => query<{ id: string; name: string }[]>("list_state_machines", {}, []), [revision]);
  const restart = () => {
    setMachine(null);
    setTimeout(() => setMachine(machine));
  };
  return (
    <div className="panel-scroll">
      <Section title="Preview">
        <div className="row">
          <Select value={machine ?? ""} options={[{ value: "", label: "— none —" }, ...list.map((m) => ({ value: m.id, label: m.name }))]} onChange={(v) => setMachine(v || null)} />
          <button className="btn" onClick={restart} disabled={!machine}>
            Restart
          </button>
        </div>
        {snap ? (
          <div className="sm-status">
            <span className="sm-state-badge">{snap.state}</span>
            <span className="muted tiny num">
              frame {snap.frame.toFixed(1)} · {snap.tweening ? "tweening" : snap.playing ? "playing" : "paused"}
              {snap.theme ? ` · theme ${snap.theme}` : ""}
            </span>
          </div>
        ) : (
          <div className="muted tiny">Pick a state machine to interact with the animation: click, hover and press on the canvas.</div>
        )}
      </Section>
      {snap && doc && (
        <Section title="Inputs">
          {doc.inputs.length === 0 && <div className="muted tiny">This machine has no inputs.</div>}
          {doc.inputs.map((i) => {
            const v = snap.inputs[i.name];
            return (
              <div key={i.name} className="prow">
                <span className="prow-label">{i.name}</span>
                <div className="prow-fields">
                  {i.type === "Event" ? (
                    <button className="btn" onClick={() => smSetInput(i.name, null)}>
                      Fire
                    </button>
                  ) : i.type === "Boolean" ? (
                    <Toggle label={String(v)} value={!!v} onChange={(b) => smSetInput(i.name, b)} />
                  ) : i.type === "Numeric" ? (
                    <NumberField value={Number(v) || 0} precision={2} onChange={(n) => smSetInput(i.name, n)} />
                  ) : (
                    <input className="text-input" defaultValue={String(v ?? "")} key={String(v)} onBlur={(e) => smSetInput(i.name, e.target.value)} />
                  )}
                </div>
                <span />
              </div>
            );
          })}
        </Section>
      )}
    </div>
  );
}

export function EventLog() {
  const log = useEditor((s) => s.smLog);
  const clear = useEditor((s) => s.clearSmLog);
  return (
    <div className="event-log">
      <div className="tl-toolbar">
        <span className="muted tiny">Event log</span>
        <div className="spacer" />
        <button className="chip-btn" onClick={clear}>
          Clear
        </button>
      </div>
      <div className="event-log-body">
        {log.length === 0 && <div className="panel-empty">Interact with the canvas to see state changes, input updates and events.</div>}
        {[...log].reverse().map((l, i) => (
          <div key={i} className={`log-row k-${l.kind}`}>
            <span className="log-kind">{l.kind}</span>
            <span>{l.text}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
