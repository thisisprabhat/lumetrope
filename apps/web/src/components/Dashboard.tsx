// Home: every project in the local library (created, opened, edited, previewed,
// exported) plus ways to start — new canvas presets, samples, open a file.
import { useEffect, useMemo, useRef, useState } from "react";
import loaderUrl from "../../../../samples/demo-loader.json?url";
import buttonUrl from "../../../../samples/demo-button.lottie?url";
import toggleUrl from "../../../../samples/demo-toggle.lottie?url";
import { openContextMenu, SEP } from "../contextMenu";
import { filterProjects, lastActivity, sortProjects, type ActivityKind, type ProjectFilter, type ProjectSort } from "../library/activity";
import { linkLabel } from "../library/files";
import { relativeTime } from "../lib/history";
import { keyLabel } from "../lib/keys";
import { createProject, duplicateProject, library, openFromDisk, openPicked, openProject, openSample, removeProject, renameProject, type ProjectMeta } from "../persist";
import { useEditor } from "../store";
import { Icon } from "./Icon";
import { McpBadge } from "./McpDialog";

const PRESETS = [
  { name: "Square", w: 512, h: 512 },
  { name: "Social post", w: 1080, h: 1080 },
  { name: "Landscape HD", w: 1920, h: 1080 },
  { name: "Story", w: 1080, h: 1920 },
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

function frameGlyph(w: number, h: number) {
  const s = 28 / Math.max(w, h);
  return <span className="preset-glyph" style={{ width: Math.round(w * s), height: Math.round(h * s) }} />;
}

async function handleFromDrop(e: React.DragEvent): Promise<{ file: File; handle?: FileSystemFileHandle } | null> {
  const item = [...e.dataTransfer.items].find((i) => i.kind === "file");
  const file = e.dataTransfer.files[0];
  if (!file) return null;
  const getHandle = (item as any)?.getAsFileSystemHandle;
  const handle = getHandle ? await getHandle.call(item).catch(() => null) : null;
  return { file, handle: handle?.kind === "file" ? handle : undefined };
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
  const menu = (e: React.MouseEvent) =>
    openContextMenu(e, [
      { label: "Open", onSelect: () => void openProject(m.id) },
      { label: "Rename", onSelect: () => setRenaming(true) },
      { label: "Duplicate", onSelect: () => void duplicateProject(m.id).then(onChanged) },
      ...(path ? [{ label: "Copy file path", onSelect: () => void navigator.clipboard?.writeText(path).then(() => useEditor.getState().toast("Copied path")) }] : []),
      SEP,
      { label: "Remove from Lumetrope…", danger: true, onSelect: () => setConfirming(true) },
    ]);
  const info = m.info;
  return (
    <article className="pcard" onContextMenu={menu}>
      <button className="pcard-thumb" onClick={() => void openProject(m.id)} aria-label={`Open ${m.name}`}>
        {m.thumb ? <img src={m.thumb} alt="" draggable={false} /> : <Icon name="image" size={22} />}
        <span className="pcard-badges">
          {!!info?.machines && <span className="pbadge cyan">Interactive</span>}
          {m.file && <span className={`pbadge ${m.file.synced ? "" : "lume"}`}>{m.file.synced ? "Saved to file" : "Unsaved to file"}</span>}
        </span>
      </button>
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
        <div className="pcard-path num" title={m.file ? linkLabel(m.file) : "Saved in Lumetrope on this device"}>
          {m.file ? `\u200e${linkLabel(m.file)}` : "Saved in Lumetrope"}
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
        const d = await handleFromDrop(e);
        if (!d) return;
        if (!/\.(lottie|json)$/i.test(d.file.name)) return useEditor.getState().toast("Drop a .lottie or Lottie .json file to open it", "error");
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
        <div className="home-search">
          <input type="search" placeholder="Search projects and files" value={query} onChange={(e) => setQuery(e.target.value)} aria-label="Search projects" />
        </div>
        <McpBadge />
      </header>

      <main className="home-main">
        <section className="home-hero">
          <div>
            <h1>Your animations</h1>
            <p>Everything you've made, opened, previewed and exported, kept on this device with its full version history.</p>
          </div>
          <div className="home-actions">
            <button className="btn primary big" onClick={() => void createProject()}>
              <Icon name="plus" size={15} /> New project <kbd>{keyLabel("⌘N")}</kbd>
            </button>
            <button className="btn big" onClick={() => void openFromDisk()}>
              <Icon name="open" size={15} /> Open file… <kbd>{keyLabel("⌘O")}</kbd>
            </button>
          </div>
        </section>

        <section className="home-start">
          <h2>Start something</h2>
          <div className="start-row">
            {PRESETS.map((p) => (
              <button key={p.name} className="start-tile" onClick={() => void createProject({ w: p.w, h: p.h, name: "Untitled" })}>
                <span className="start-art">{frameGlyph(p.w, p.h)}</span>
                <span className="start-name">{p.name}</span>
                <span className="start-note num">
                  {p.w}×{p.h}
                </span>
              </button>
            ))}
            {SAMPLES.map((s) => (
              <button key={s.name} className="start-tile sample" onClick={() => void openSample(s.url, s.file, s.name)}>
                <span className="start-art">
                  <Icon name={s.interactive ? "bolt" : "play"} size={18} />
                </span>
                <span className="start-name">{s.name}</span>
                <span className="start-note">{s.note}</span>
              </button>
            ))}
          </div>
        </section>

        <section className="home-library">
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
                  <p className="lib-empty-title">No projects yet</p>
                  <p>Start a new canvas, open a .lottie or Lottie JSON file, or try a sample above. Files you open are remembered here.</p>
                </>
              )}
            </div>
          )}
        </section>
      </main>
      {dragging && <div className="home-drop">Drop a .lottie or Lottie JSON file to open it</div>}
    </div>
  );
}
