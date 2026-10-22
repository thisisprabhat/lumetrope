import { useEffect, useRef, useState, type ReactNode } from "react";
import { keyLabel } from "../lib/keys";

export interface MenuItem {
  label: string;
  shortcut?: string;
  onSelect?: () => void;
  disabled?: boolean;
  checked?: boolean;
  separator?: boolean;
}

/** Click-to-open dropdown menu. */
export function Menu({ trigger, items, align = "left" }: { trigger: ReactNode; items: MenuItem[]; align?: "left" | "right" }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const close = (e: PointerEvent) => !ref.current?.contains(e.target as Node) && setOpen(false);
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    window.addEventListener("pointerdown", close);
    window.addEventListener("keydown", esc);
    return () => {
      window.removeEventListener("pointerdown", close);
      window.removeEventListener("keydown", esc);
    };
  }, [open]);
  return (
    <div className="menu" ref={ref}>
      <button className={`menu-trigger ${open ? "open" : ""}`} onClick={() => setOpen(!open)}>
        {trigger}
      </button>
      {open && (
        <div className={`menu-pop ${align}`}>
          {items.map((it, i) =>
            it.separator ? (
              <div key={i} className="menu-sep" />
            ) : (
              <button
                key={i}
                className="menu-item"
                disabled={it.disabled}
                onClick={() => {
                  setOpen(false);
                  it.onSelect?.();
                }}
              >
                <span className="menu-check">{it.checked ? "✓" : ""}</span>
                <span className="menu-label">{it.label}</span>
                {it.shortcut && <span className="menu-shortcut num">{keyLabel(it.shortcut)}</span>}
              </button>
            ),
          )}
        </div>
      )}
    </div>
  );
}
