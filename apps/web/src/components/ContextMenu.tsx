import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { commands } from "../commands";
import { openContextMenu, useContextMenu, SEP, type CtxItem } from "../contextMenu";
import { useEditor } from "../store";

function MenuList({ items, x, y, onDone, depth = 0, flipX }: { items: CtxItem[]; x: number; y: number; onDone: () => void; depth?: number; flipX?: number }) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ x, y });
  const [sub, setSub] = useState<{ i: number; x: number; y: number; flipX: number } | null>(null);
  const [active, setActive] = useState(-1);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    let nx = x;
    let ny = y;
    if (nx + r.width > window.innerWidth - 6) nx = flipX !== undefined ? flipX - r.width : window.innerWidth - r.width - 6;
    if (ny + r.height > window.innerHeight - 6) ny = Math.max(6, window.innerHeight - r.height - 6);
    setPos({ x: Math.max(6, nx), y: ny });
  }, [x, y, flipX]);
  useEffect(() => {
    if (depth) return;
    const key = (e: KeyboardEvent) => {
      const usable = items.map((it, i) => (!it.separator && !it.disabled ? i : -1)).filter((i) => i >= 0);
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault();
        const cur = usable.indexOf(active);
        const n = e.key === "ArrowDown" ? usable[(cur + 1) % usable.length] : usable[(cur - 1 + usable.length) % usable.length];
        setActive(n ?? -1);
      } else if (e.key === "Enter" && active >= 0) {
        e.preventDefault();
        const it = items[active];
        if (it.onSelect) {
          onDone();
          it.onSelect();
        }
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [items, active, depth, onDone]);
  return (
    <div className="ctx-menu" ref={ref} style={{ left: pos.x, top: pos.y }} onContextMenu={(e) => e.preventDefault()}>
      {items.map((it, i) =>
        it.separator ? (
          <div key={i} className="menu-sep" />
        ) : (
          <button
            key={i}
            className={`menu-item ${it.danger ? "danger" : ""} ${active === i ? "active" : ""}`}
            disabled={it.disabled}
            onMouseEnter={(e) => {
              setActive(i);
              if (it.submenu) {
                const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
                setSub({ i, x: r.right + 2, y: r.top - 5, flipX: r.left - 2 });
              } else setSub(null);
            }}
            onClick={() => {
              if (it.submenu) return;
              onDone();
              it.onSelect?.();
            }}
          >
            <span className="menu-check">{it.checked ? "✓" : ""}</span>
            <span className="menu-label">{it.label}</span>
            {it.shortcut && <span className="menu-shortcut num">{it.shortcut}</span>}
            {it.submenu && <span className="menu-shortcut">›</span>}
          </button>
        ),
      )}
      {sub && items[sub.i]?.submenu && <MenuList items={items[sub.i].submenu!} x={sub.x} y={sub.y} flipX={sub.flipX} onDone={onDone} depth={depth + 1} />}
    </div>
  );
}

function isText(t: EventTarget | null): t is HTMLInputElement | HTMLTextAreaElement {
  return (t instanceof HTMLInputElement && !["checkbox", "color", "range", "file"].includes(t.type)) || t instanceof HTMLTextAreaElement;
}

function textItems(el: HTMLInputElement | HTMLTextAreaElement): CtxItem[] {
  const sel = (el.selectionEnd ?? 0) > (el.selectionStart ?? 0);
  const replace = (text: string) => {
    el.focus();
    el.setRangeText(text, el.selectionStart ?? 0, el.selectionEnd ?? 0, "end");
    el.dispatchEvent(new Event("input", { bubbles: true }));
  };
  return [
    { label: "Cut", shortcut: "⌘X", disabled: !sel || el.readOnly, onSelect: () => void navigator.clipboard?.writeText(el.value.slice(el.selectionStart ?? 0, el.selectionEnd ?? 0)).then(() => replace("")) },
    { label: "Copy", shortcut: "⌘C", disabled: !sel, onSelect: () => void navigator.clipboard?.writeText(el.value.slice(el.selectionStart ?? 0, el.selectionEnd ?? 0)) },
    { label: "Paste", shortcut: "⌘V", disabled: el.readOnly, onSelect: () => void navigator.clipboard?.readText().then(replace) },
    SEP,
    { label: "Select all", shortcut: "⌘A", onSelect: () => (el.focus(), el.select()) },
  ];
}

/** Global host: suppresses the native menu and renders the active app menu. */
export function ContextMenuHost() {
  const menu = useContextMenu((s) => s.menu);
  const close = useContextMenu((s) => s.close);
  useEffect(() => {
    // Fallback for areas without their own menu.
    const onCtx = (e: MouseEvent) => {
      if (e.defaultPrevented) return;
      if (isText(e.target)) return openContextMenu(e, textItems(e.target));
      const st = useEditor.getState();
      openContextMenu(e, [
        { label: "Undo", shortcut: "⌘Z", onSelect: commands.undo, disabled: !st.canUndo },
        { label: "Redo", shortcut: "⇧⌘Z", onSelect: commands.redo, disabled: !st.canRedo },
        SEP,
        { label: st.playing ? "Pause" : "Play", shortcut: "Space", onSelect: () => st.setPlaying(!st.playing) },
        { label: "Zoom to fit", shortcut: "⇧1", onSelect: commands.fit },
        SEP,
        { label: "Import…", onSelect: () => void commands.import() },
        { label: "Export…", shortcut: "⌘E", onSelect: commands.openExport },
      ]);
    };
    window.addEventListener("contextmenu", onCtx);
    return () => window.removeEventListener("contextmenu", onCtx);
  }, []);
  useEffect(() => {
    if (!menu) return;
    const down = (e: PointerEvent) => !(e.target as HTMLElement).closest?.(".ctx-menu") && close();
    const key = (e: KeyboardEvent) => e.key === "Escape" && close();
    const blur = () => close();
    window.addEventListener("pointerdown", down, true);
    window.addEventListener("keydown", key);
    window.addEventListener("blur", blur);
    window.addEventListener("resize", blur);
    return () => {
      window.removeEventListener("pointerdown", down, true);
      window.removeEventListener("keydown", key);
      window.removeEventListener("blur", blur);
      window.removeEventListener("resize", blur);
    };
  }, [menu, close]);
  if (!menu) return null;
  return <MenuList key={`${menu.x},${menu.y}`} items={menu.items} x={menu.x} y={menu.y} onDone={close} />;
}
