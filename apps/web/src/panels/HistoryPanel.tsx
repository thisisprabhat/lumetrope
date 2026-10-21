import { useEffect, useMemo, useState } from "react";
import { engine } from "../engine";
import { changeSummary, relativeTime, type Diff } from "../lib/history";
import { saveNow } from "../persist";
import { useEditor } from "../store";
import { Icon } from "../components/Icon";
import { Select } from "../ui/controls";
import { openContextMenu, SEP } from "../contextMenu";

interface CommitInfo {
  id: string;
  message: string;
  author: string;
  ts: number;
  parents: string[];
}

export function commitNow(message: string): string | null {
  const st = useEditor.getState();
  try {
    const id = engine().vcs_commit(message || "Untitled change", "You", Date.now());
    st.bumpHistory();
    void saveNow();
    st.toast(`Saved version “${message || "Untitled change"}”`);
    return id;
  } catch (e) {
    st.toast(String(e), "error");
    return null;
  }
}

export function HistoryPanel() {
  const { revision, historyTick, bumpHistory, refresh, toast } = useEditor();
  const [message, setMessage] = useState("");
  const [compare, setCompare] = useState<{ id: string; diff: Diff } | null>(null);
  const [tick, setTick] = useState(0);
  useEffect(() => {
    const t = setInterval(() => setTick((n) => n + 1), 30_000);
    return () => clearInterval(t);
  }, []);
  const log = useMemo(() => JSON.parse(engine().vcs_log()) as { branch: string; commits: CommitInfo[] }, [historyTick, revision]);
  const branches = useMemo(() => JSON.parse(engine().vcs_branches()) as { current: string; branches: { name: string; head: string }[] }, [historyTick, revision]);
  const status = useMemo(() => JSON.parse(engine().vcs_status()) as Diff, [historyTick, revision]);
  const dirty = changeSummary(status) !== "no changes";
  void tick;

  const restore = (c: CommitInfo) => {
    try {
      engine().vcs_restore(c.id);
      useEditor.getState().select([]);
      refresh();
      bumpHistory();
      toast(`Restored “${c.message}”`);
    } catch (e) {
      toast(String(e), "error");
    }
  };

  const doCompare = (c: CommitInfo) => {
    const head = log.commits[0];
    if (!head || head.id === c.id) {
      // Compare with the working document.
      setCompare({ id: c.id, diff: JSON.parse(engine().vcs_status()) });
      return;
    }
    setCompare({ id: c.id, diff: JSON.parse(engine().vcs_diff(c.id, head.id)) });
  };

  return (
    <>
      <div className="panel-head">
        <span>Version history</span>
      </div>
      <div className="panel-pad">
        <div className="row">
          <Select
            value={branches.current}
            options={branches.branches.map((b) => ({ value: b.name, label: `⎇ ${b.name}` }))}
            onChange={(name) => {
              try {
                engine().vcs_switch(name);
                refresh();
                bumpHistory();
              } catch (e) {
                toast(String(e), "error");
              }
            }}
          />
          <button
            className="btn"
            onClick={() => {
              const name = `branch-${branches.branches.length}`;
              try {
                engine().vcs_create_branch(name);
                bumpHistory();
                toast(`Created branch ${name}`);
              } catch (e) {
                toast(String(e), "error");
              }
            }}
          >
            New branch
          </button>
        </div>
        <div className={`vcs-status ${dirty ? "dirty" : ""}`}>
          <span className="dot" />
          {dirty ? `Unsaved: ${changeSummary(status)}` : log.commits.length ? "All changes saved as versions" : "No versions yet"}
        </div>
        <textarea
          className="text-area"
          rows={2}
          placeholder="Describe this version…"
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && (e.metaKey || e.ctrlKey) && commitNow(message)) setMessage("");
          }}
        />
        <button className="btn primary" disabled={!dirty && log.commits.length > 0} onClick={() => commitNow(message) && setMessage("")}>
          <Icon name="save" size={14} /> Save version
        </button>
        <div className="subhead">{log.commits.length} versions</div>
        <ol className="commit-list">
          {log.commits.map((c, i) => (
            <li
              key={c.id}
              className={`commit ${i === 0 ? "head" : ""}`}
              onContextMenu={(e) =>
                openContextMenu(e, [
                  { label: "Restore this version", onSelect: () => restore(c) },
                  { label: i === 0 ? "Compare with current" : "Compare with latest", onSelect: () => doCompare(c) },
                  SEP,
                  { label: "Copy version id", onSelect: () => void navigator.clipboard?.writeText(c.id) },
                ])
              }
            >
              <span className="commit-dot" />
              <div className="commit-body">
                <div className="commit-msg">{c.message}</div>
                <div className="muted tiny">
                  {relativeTime(c.ts)} · <span className="num">{c.id.slice(0, 7)}</span>
                </div>
                <div className="row">
                  <button className="chip-btn" onClick={() => restore(c)}>
                    Restore
                  </button>
                  <button className="chip-btn" onClick={() => doCompare(c)}>
                    {i === 0 ? "Diff current" : "Diff latest"}
                  </button>
                </div>
                {compare?.id === c.id && (
                  <div className="diff-box">
                    <div>{changeSummary(compare.diff)}</div>
                    {compare.diff.layers_added.length > 0 && <div className="ok">+ {compare.diff.layers_added.join(", ")}</div>}
                    {compare.diff.layers_removed.length > 0 && <div className="bad">− {compare.diff.layers_removed.join(", ")}</div>}
                    {compare.diff.layers_changed.length > 0 && <div>~ {compare.diff.layers_changed.join(", ")}</div>}
                  </div>
                )}
              </div>
            </li>
          ))}
        </ol>
      </div>
    </>
  );
}
