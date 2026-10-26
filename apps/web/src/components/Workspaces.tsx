// Workspace UI on the dashboard: the switcher in the header, the workspace
// block in the left rail (rename, new, export/import everything) and a banner
// when a folder workspace needs access again.
import { useEffect, useRef, useState } from "react";
import { create } from "zustand";
import { openContextMenu, SEP, type CtxItem } from "../contextMenu";
import { foldersSupported, listWorkspaces, workspaceWhere } from "../library/workspace";
import { useEditor } from "../store";
import { exportAllData, importBackupFile, newBrowserWorkspace, openFolderWorkspace, reconnectWorkspace, removeWorkspace, renameWorkspace, switchWorkspace } from "../workspaces";
import { Icon } from "./Icon";

type Editing = "rename" | "create" | "remove" | null;
const useWsUi = create<{ editing: Editing; set: (e: Editing) => void }>((set) => ({ editing: null, set: (editing) => set({ editing }) }));

function menuItems(): CtxItem[] {
  const cur = useEditor.getState().workspace.info;
  const set = useWsUi.getState().set;
  const folders = foldersSupported();
  return [
    ...listWorkspaces().map((w) => ({ label: w.name, checked: w.id === cur.id, onSelect: () => w.id !== cur.id && void switchWorkspace(w.id, true) })),
    SEP,
    { label: folders ? "Open folder as workspace…" : "Open folder as workspace (desktop app or Chrome/Edge)", disabled: !folders, onSelect: () => void openFolderWorkspace() },
    { label: "New workspace…", onSelect: () => set("create") },
    SEP,
    { label: "Export all data (.zip)…", onSelect: () => void exportAllData() },
    { label: "Import backup (.zip)…", onSelect: () => void importBackupFile() },
    SEP,
    { label: "Rename workspace…", disabled: cur.id === "default", onSelect: () => set("rename") },
    { label: cur.kind === "folder" ? "Remove from list…" : "Delete workspace…", danger: true, disabled: cur.id === "default", onSelect: () => set("remove") },
  ];
}

export function WorkspaceSwitch() {
  const ws = useEditor((s) => s.workspace);
  return (
    <button
      className="ws-switch"
      aria-label={`Workspace: ${ws.info.name}. Change workspace`}
      title={workspaceWhere(ws.info)}
      onClick={(e) => {
        const r = e.currentTarget.getBoundingClientRect();
        openContextMenu({ preventDefault: () => e.preventDefault(), stopPropagation: () => e.stopPropagation(), clientX: r.left, clientY: r.bottom + 4 }, menuItems());
      }}
    >
      <Icon name={ws.info.kind === "folder" ? "folder" : "layers"} size={14} />
      <span className="ws-switch-name">{ws.info.name}</span>
      <Icon name="chevronDown" size={12} />
    </button>
  );
}

function NameForm({ initial, submit, label, onCancel }: { initial: string; submit: (name: string) => void; label: string; onCancel: () => void }) {
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => input.current?.select(), []);
  return (
    <form
      className="ws-form"
      onSubmit={(e) => {
        e.preventDefault();
        submit(input.current?.value ?? "");
      }}
    >
      <input ref={input} className="text-input" defaultValue={initial} aria-label="Workspace name" onKeyDown={(e) => e.key === "Escape" && onCancel()} />
      <button className="btn small primary" type="submit">
        {label}
      </button>
      <button className="btn small ghost" type="button" onClick={onCancel}>
        Cancel
      </button>
    </form>
  );
}

export function WorkspaceBlock() {
  const ws = useEditor((s) => s.workspace);
  const { editing, set } = useWsUi();
  const [busy, setBusy] = useState(false);
  const card = useRef<HTMLDivElement>(null);
  // Forms opened from the header menu appear here; bring them into view.
  useEffect(() => {
    if (editing) card.current?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }, [editing]);
  const run = (f: () => Promise<void>) => async () => {
    setBusy(true);
    try {
      await f();
    } finally {
      setBusy(false);
    }
  };
  const w = ws.info;
  return (
    <div className="rail-block">
      <h2>Workspace</h2>
      <div className="ws-card" ref={card}>
        <div className="ws-card-head">
          <Icon name={w.kind === "folder" ? "folder" : "layers"} size={15} />
          <div className="ws-card-text">
            <span className="ws-card-name">{w.name}</span>
            <span className={`ws-card-where ${w.kind === "folder" ? "path" : ""}`} title={workspaceWhere(w)}>
              {"\u200e" + workspaceWhere(w)}
            </span>
          </div>
          <button className="icon-btn small" aria-label="Workspace actions" onClick={(e) => openContextMenu(e, menuItems())}>
            <Icon name="more" size={14} />
          </button>
        </div>
        {editing === "rename" && (
          <NameForm
            initial={w.name}
            label="Rename"
            onCancel={() => set(null)}
            submit={(n) => {
              renameWorkspace(w.id, n);
              set(null);
            }}
          />
        )}
        {editing === "create" && (
          <NameForm
            initial={`Workspace ${listWorkspaces().length + 1}`}
            label="Create"
            onCancel={() => set(null)}
            submit={(n) => {
              set(null);
              void newBrowserWorkspace(n);
            }}
          />
        )}
        {editing === "remove" && (
          <div className="pcard-confirm">
            <span>{w.kind === "folder" ? "Remove this folder from the list? Its files stay on disk." : "Delete this workspace and every project in it from this browser? Export it first to keep a copy."}</span>
            <button
              className="btn danger small"
              onClick={() => {
                set(null);
                void removeWorkspace(w);
              }}
            >
              {w.kind === "folder" ? "Remove" : "Delete"}
            </button>
            <button className="btn ghost small" onClick={() => set(null)}>
              Cancel
            </button>
          </div>
        )}
        <div className="ws-actions">
          <button className="rail-action" disabled={busy || ws.status !== "ready"} onClick={run(exportAllData)}>
            <Icon name="archive" size={15} />
            <span>Export all data (.zip)</span>
          </button>
          <button className="rail-action" disabled={busy || ws.status !== "ready"} onClick={run(() => importBackupFile())}>
            <Icon name="import" size={15} />
            <span>Import backup…</span>
          </button>
          <button className="rail-action" disabled={!foldersSupported()} title={foldersSupported() ? undefined : "Folders need the desktop app or Chrome / Edge"} onClick={() => void openFolderWorkspace()}>
            <Icon name="folder" size={15} />
            <span>Open folder as workspace…</span>
          </button>
        </div>
      </div>
    </div>
  );
}

/** Shown instead of the project grid when the workspace can't be read. */
export function WorkspaceProblem() {
  const ws = useEditor((s) => s.workspace);
  if (ws.status === "ready" || ws.status === "loading") return null;
  const text =
    ws.status === "needs-permission"
      ? "Your browser needs permission to open this folder again."
      : ws.status === "missing"
        ? "This workspace's folder can't be found. It may have been moved, renamed or deleted."
        : "Lumetrope couldn't open this workspace.";
  return (
    <div className="lib-empty ws-problem">
      <p className="lib-empty-title">{ws.info.name}</p>
      <p>{text}</p>
      <div className="row ws-problem-actions">
        {ws.status === "needs-permission" ? (
          <button className="btn primary" onClick={() => void reconnectWorkspace()}>
            Allow access
          </button>
        ) : (
          foldersSupported() && (
            <button className="btn primary" onClick={() => void openFolderWorkspace()}>
              Choose folder…
            </button>
          )
        )}
        <button className="btn" onClick={() => void switchWorkspace("default")}>
          Use “This device”
        </button>
      </div>
    </div>
  );
}
