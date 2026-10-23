// Home. Left rail: ways to start (new canvas with presets or a custom size,
// open / paste / import, samples, AI agents). Right: the library of every
// project created, opened, edited, previewed or exported on this device.
import { useEffect, useMemo, useRef, useState } from "react";
import loaderUrl from "../../../../samples/demo-loader.json?url";
import buttonUrl from "../../../../samples/demo-button.lottie?url";
import toggleUrl from "../../../../samples/demo-toggle.lottie?url";
import { openContextMenu, SEP } from "../contextMenu";
import { pickFile } from "../files";
import { filterProjects, lastActivity, sortProjects, type ActivityKind, type ProjectFilter, type ProjectSort } from "../library/activity";
import { linkLabel } from "../library/files";
import { formatBytes } from "../lib/export";
import { relativeTime } from "../lib/history";
import { keyLabel } from "../lib/keys";
import {
  backfillThumbnails,
  createFromSvg,
  createProject,
  duplicateProject,
  library,
  openFromClipboard,
  openFromDisk,
  openPicked,
  openProject,
  openSample,
  removeProject,
  renameProject,
  storageUsage,
  type ProjectMeta,
} from "../persist";
import { localMcpSupported } from "../platform";
import { useEditor } from "../store";
import { Icon } from "./Icon";
import { McpBadge } from "./McpDialog";

const PRESETS = [
  { name: "Square", w: 512, h: 512 },
  { name: "Icon", w: 128, h: 128 },
  { name: "Social post", w: 1080, h: 1080 },
  { name: "Landscape HD", w: 1920, h: 1080 },
  { name: "Story / Reel", w: 1080, h: 1920 },
  { name: "Web banner", w: 1200, h: 400 },
  { name: "Mobile screen", w: 390, h: 844 },
];

const SAMPLES = [
  { name: "Loader", file: "demo-loader.json", url: loaderUrl, note: "Trim paths + rotation" },
  { name: "Interactive button", file: "demo-button.lottie", url: buttonUrl, note: "Click → spin", interactive: true },
  { name: "Toggle switch", file: "demo-toggle.lottie", url: toggleUrl, note: "Boolean input", interactive: true },
];

const FILTERS: { id: ProjectFilter; label: string }[] = [
  { id: "all", label: "All" },
  { id: "created", label: "Created here" },
  { id: "files", label: "Local files" },
  { id: "previewed", label: "Previewed" },
  { id: "exported", label: "Exported" },
];

const VERB: Record<ActivityKind, string> = { created: "Created", opened: "Opened", edited: "Edited", previewed: "Previewed", exported: "Exported" };

function activityText(m: ProjectMeta): string {
  const a = lastActivity(m);
  const verb = a.kind === "exported" && m.exported ? `Exported ${m.exported.format.toUpperCase()}` : VERB[a.kind];
  return `${verb} ${relativeTime(a.at)}`;
}

function FrameGlyph({ w, h, size = 22 }: { w: number; h: number; size?: number }) {
  const s = size / Math.max(w, h);
  return (
    <span className="glyph-box" style={{ width: size, height: size }}>
      <span className="preset-glyph" style={{ width: Math.max(4, Math.round(w * s)), height: Math.max(4, Math.round(h * s)) }} />
    </span>
  );
}

async function fromDrop(e: React.DragEvent): Promise<{ file: File; handle?: FileSystemFileHandle } | null> {
  const item = [...e.dataTransfer.items].find((i) => i.kind === "file");
  const file = e.dataTransfer.files[0];
  if (!file) return null;
  const get = (item as any)?.getAsFileSystemHandle;
  const handle = get ? await get.call(item).catch(() => null) : null;
  return { file, handle: handle?.kind === "file" ? handle : undefined };
}

function CustomSize() {
  const [w, setW] = useState(800);
  const [h, setH] = useState(600);
  const [fps, setFps] = useState(30);
  const [secs, setSecs] = useState(3);
  const num = (v: string, min: number, max: number) => Math.min(max, Math.max(min, Math.round(Number(v) || min)));
  return (
    <form
      className="custom-size"
      onSubmit={(e) => {
        e.preventDefault();
        void createProject({ w, h, fr: fps, frames: Math.max(1, Math.round(secs * fps)) });
      }}
    >
      <label>
        <span>W</span>
        <input type="number" min={16} max={8192} value={w} onChange={(e) => setW(num(e.target.value, 16, 8192))} />
      </label>
      <label>
        <span>H</span>
        <input type="number" min={16} max={8192} value={h} onChange={(e) => setH(num(e.target.value, 16, 8192))} />
      </label>
      <label>
        <span>fps</span>
        <input type="number" min={1} max={120} value={fps} onChange={(e) => setFps(num(e.target.value, 1, 120))} />
      </label>
      <label>
        <span>sec</span>
        <input type="number" min={0.1} max={600} step={0.1} value={secs} onChange={(e) => setSecs(Math.min(600, Math.max(0.1, Number(e.target.value) || 0.1)))} />
      </label>
      <button className="btn small" type="submit">
        Create
      </button>
    </form>
  );
}

function StartRail({ count }: { count: number }) {
  const [custom, setCustom] = useState(false);
  const [usage, setUsage] = useState<{ used: number; quota: number } | null>(null);
  useEffect(() => {
    void storageUsage().then(setUsage);
  }, [count]);
  const importSvg = async () => {
    const f = await pickFile(".svg");
    if (f) await createFromSvg(f);
  };
  return (
    <aside className="home-rail">
      <div className="rail-block">
        <button className="btn primary big wide" onClick={() => void createProject()}>
          <Icon name="plus" size={15} /> New project <kbd>{keyLabel("⌘N")}</kbd>
        </button>
        <div className="rail-actions">
          <button className="rail-action" onClick={() => void openFromDisk()}>
            <Icon name="open" size={15} />
            <span>Open file…</span>
            <kbd>{keyLabel("⌘O")}</kbd>
          </button>
          <button className="rail-action" onClick={() => void openFromClipboard()}>
            <Icon name="copy" size={15} />
            <span>Paste Lottie JSON</span>
          </button>
          <button className="rail-action" onClick={() => void importSvg()}>
            <Icon name="image" size={15} />
            <span>New from SVG…</span>
          </button>
        </div>
      </div>

      <div className="rail-block">
        <h2>Canvas</h2>
        <div className="rail-list">
          {PRESETS.map((p) => (
            <button key={p.name} className="rail-row" onClick={() => void createProject({ w: p.w, h: p.h })}>
              <FrameGlyph w={p.w} h={p.h} />
              <span className="rail-row-name">{p.name}</span>
              <span className="rail-row-note num">
                {p.w}×{p.h}
              </span>
            </button>
          ))}
          <button className={`rail-row ${custom ? "on" : ""}`} onClick={() => setCustom(!custom)} aria-expanded={custom}>
            <span className="glyph-box">
              <Icon name="settings" size={14} />
            </span>
            <span className="rail-row-name">Custom size…</span>
            <Icon name={custom ? "chevronDown" : "chevronRight"} size={12} />
          </button>
          {custom && <CustomSize />}
        </div>
      </div>

      <div className="rail-block">
        <h2>Samples</h2>
        <div className="rail-list">
          {SAMPLES.map((s) => (
            <button key={s.name} className="rail-row sample" onClick={() => void openSample(s.url, s.file, s.name)}>
              <span className="glyph-box">
                <Icon name={s.interactive ? "bolt" : "play"} size={14} />
              </span>
              <span className="rail-row-name">{s.name}</span>
              <span className="rail-row-note">{s.note}</span>
            </button>
          ))}
        </div>
      </div>

      <div className="rail-foot">
        {localMcpSupported() && (
          <button className="rail-action" onClick={() => useEditor.setState({ mcpHelpOpen: true })}>
            <Icon name="mcp" size={15} />
            <span>Connect an AI agent</span>
          </button>
        )}
        <div className="rail-meta">
          <span>
            {count} project{count === 1 ? "" : "s"} on this device
          </span>
          {usage && <span className="num">{formatBytes(usage.used)} used</span>}
        </div>
        <div className="rail-meta faint">Drop a .lottie, .json or .svg anywhere to open it</div>
      </div>
    </aside>
  );
}

function ProjectCard({ m, onChanged }: { m: ProjectMeta; onChanged: () => void }) {
  const [renaming, setRenaming] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (renaming) input.current?.select();
  }, [renaming]);
  const commitName = async (name: string) => {
    setRenaming(false);
    if (name.trim() && name.trim() !== m.name) {
      await renameProject(m.id, name);
      onChanged();
    }
  };
  const path = m.file?.link?.kind === "path" ? m.file.link.path : null;
  const play = () => void openProject(m.id).then((ok) => ok && useEditor.setState({ fullscreen: true }));
  const menu = (e: React.MouseEvent) =>
    openContextMenu(e, [
      { label: "Open", onSelect: () => void openProject(m.id) },
      { label: "Play full screen", onSelect: play },
      { label: "Rename", onSelect: () => setRenaming(true) },
      { label: "Duplicate", onSelect: () => void duplicateProject(m.id).then(onChanged) },
      ...(path ? [{ label: "Copy file path", onSelect: () => void navigator.clipboard?.writeText(path).then(() => useEditor.getState().toast("Copied path")) }] : []),
      SEP,
      { label: "Remove from Lumetrope…", danger: true, onSelect: () => setConfirming(true) },
    ]);
  const info = m.info;
  return (
    <article className="pcard" onContextMenu={menu}>
      <div className="pcard-thumb-wrap">
        <button className="pcard-thumb" onClick={() => void openProject(m.id)} aria-label={`Open ${m.name}`}>
          {m.thumb ? <img src={m.thumb} alt="" draggable={false} /> : <span className="pcard-initial">{m.name.trim().charAt(0).toUpperCase() || "·"}</span>}
        </button>
        <span className="pcard-badges">
          {!!info?.machines && <span className="pbadge cyan">Interactive</span>}
          {m.file && <span className={`pbadge ${m.file.synced ? "" : "lume"}`}>{m.file.synced ? "Saved to file" : "Unsaved to file"}</span>}
        </span>
        <button className="pcard-play" onClick={play} aria-label={`Play ${m.name} full screen`} data-tip="Play full screen">
          <Icon name="play" size={14} />
        </button>
      </div>
      <div className="pcard-body">
        <div className="pcard-title">
          {renaming ? (
            <input
              ref={input}
              className="pcard-rename"
              defaultValue={m.name}
              onBlur={(e) => void commitName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") void commitName(e.currentTarget.value);
                if (e.key === "Escape") setRenaming(false);
              }}
            />
          ) : (
            <span className="pcard-name" onDoubleClick={() => setRenaming(true)} title={m.name}>
              {m.name}
            </span>
          )}
          <button className="icon-btn small" onClick={menu} aria-label="Project actions">
            <Icon name="more" size={14} />
          </button>
        </div>
        <div className="pcard-path" title={m.file ? linkLabel(m.file) : "Saved in Lumetrope on this device"}>
          {m.file ? `‎${linkLabel(m.file)}` : "Saved in Lumetrope"}
        </div>
        <div className="pcard-meta">
          <span>{activityText(m)}</span>
          {info && (
            <span className="num">
              {info.w}×{info.h} · {(info.frames / (info.fr || 30)).toFixed(1)}s
            </span>
          )}
        </div>
        {confirming && (
          <div className="pcard-confirm">
            <span>Remove from Lumetrope?{m.file ? " The file on disk is kept." : " This deletes its version history."}</span>
            <button className="btn danger small" onClick={() => void removeProject(m.id).then(onChanged)}>
              Remove
            </button>
            <button className="btn ghost small" onClick={() => setConfirming(false)}>
              Cancel
            </button>
          </div>
        )}
      </div>
    </article>
  );
}

export function Dashboard() {
  const [projects, setProjects] = useState<ProjectMeta[] | null>(null);
  const [filter, setFilter] = useState<ProjectFilter>("all");
  const [sort, setSort] = useState<ProjectSort>("recent");
  const [query, setQuery] = useState("");
  const [dragging, setDragging] = useState(false);
  const reload = () =>
    void library
      .list()
      .then(setProjects)
      .catch(() => setProjects([]));
  useEffect(reload, []);
  // Older projects get thumbnails in the background.
  const backfilled = useRef(false);
  useEffect(() => {
    if (!projects || backfilled.current || !projects.some((p) => !p.thumb)) return;
    backfilled.current = true;
    void backfillThumbnails(projects, reload);
  }, [projects]);
  const shown = useMemo(() => (projects ? sortProjects(filterProjects(projects, filter, query), sort) : []), [projects, filter, query, sort]);
  const counts = useMemo(() => Object.fromEntries(FILTERS.map((f) => [f.id, projects ? filterProjects(projects, f.id, "").length : 0])), [projects]);

  return (
    <div
      className={`home ${dragging ? "dragging" : ""}`}
      onDragOver={(e) => {
        e.preventDefault();
        setDragging(true);
      }}
      onDragLeave={(e) => e.currentTarget === e.target && setDragging(false)}
      onDrop={async (e) => {
        e.preventDefault();
        setDragging(false);
        const d = await fromDrop(e);
        if (!d) return;
        if (/\.svg$/i.test(d.file.name)) return void createFromSvg(d.file);
        if (!/\.(lottie|json)$/i.test(d.file.name)) return useEditor.getState().toast("Drop a .lottie, Lottie .json or .svg file", "error");
        await openPicked({ file: d.file, link: d.handle ? { kind: "handle", handle: d.handle } : undefined });
      }}
    >
      <header className="home-top">
        <div className="wordmark">
          <svg className="disc" viewBox="0 0 20 20" aria-hidden>
            <circle cx="10" cy="10" r="7" fill="none" stroke="currentColor" strokeWidth="3.2" strokeDasharray="5.5 5.5" />
          </svg>
          Lumetrope
        </div>
        <McpBadge />
      </header>

      <div className="home-body">
        <StartRail count={projects?.length ?? 0} />

        <main className="home-main">
          <div className="lib-head">
            <div>
              <h1>Your animations</h1>
              <p>Everything you've made, opened, previewed and exported, with full version history.</p>
            </div>
            <label className="home-search">
              <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden>
                <circle cx="7" cy="7" r="4.5" fill="none" stroke="currentColor" strokeWidth="1.5" />
                <path d="M10.5 10.5 14 14" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
              </svg>
              <input type="search" placeholder="Search projects and files" value={query} onChange={(e) => setQuery(e.target.value)} aria-label="Search projects" />
            </label>
          </div>
          <div className="lib-bar">
            <div className="lib-filters" role="tablist">
              {FILTERS.map((f) => (
                <button key={f.id} role="tab" aria-selected={filter === f.id} className={`lib-filter ${filter === f.id ? "on" : ""}`} onClick={() => setFilter(f.id)}>
                  {f.label} <span className="num">{counts[f.id]}</span>
                </button>
              ))}
            </div>
            <label className="lib-sort">
              Sort
              <select value={sort} onChange={(e) => setSort(e.target.value as ProjectSort)}>
                <option value="recent">Last activity</option>
                <option value="name">Name</option>
                <option value="created">Date created</option>
              </select>
            </label>
          </div>

          {projects === null ? null : shown.length ? (
            <div className="pgrid">
              {shown.map((m) => (
                <ProjectCard key={m.id} m={m} onChanged={reload} />
              ))}
            </div>
          ) : (
            <div className="lib-empty">
              {projects.length ? (
                <p>Nothing matches{query ? ` “${query}”` : ""} here.</p>
              ) : (
                <>
                  <p className="lib-empty-title">Nothing here yet</p>
                  <p>Start a canvas or open a file from the left. Everything you create, open, preview or export shows up here.</p>
                </>
              )}
            </div>
          )}
        </main>
      </div>
      {dragging && <div className="home-drop">Drop a .lottie, Lottie JSON or SVG file to open it</div>}
    </div>
  );
}
