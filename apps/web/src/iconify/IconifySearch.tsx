import { useEffect, useRef, useState } from "react";
import { act } from "../actions";
import { Icon } from "../components/Icon";
import { useEditor } from "../store";
import { collectionCredit, fetchIconSvg, searchIcons, type IconSearch } from "./api";
import { encodeIconDrag, ICON_MIME } from "./drop";
import { iconifyMessage, iconImportArgs, isOffline } from "./place";
import { iconPreview } from "./preview";

export function IconifySearch() {
  const [input, setInput] = useState("");
  const [query, setQuery] = useState("");
  const [result, setResult] = useState<IconSearch | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    const t = window.setTimeout(() => setQuery(input.trim()), 300);
    return () => window.clearTimeout(t);
  }, [input]);

  useEffect(() => {
    if (!query) {
      setResult(null);
      setError(null);
      setBusy(false);
      return;
    }
    let live = true;
    setBusy(true);
    setError(null);
    searchIcons(query)
      .then((r) => {
        if (live) setResult(r);
      })
      .catch((e) => {
        if (!live) return;
        setResult(null);
        setError(isOffline(e) ? "Search when you're online" : "Icon search failed");
      })
      .finally(() => {
        if (live) setBusy(false);
      });
    return () => {
      live = false;
    };
  }, [query]);

  const credit = result ? collectionCredit(result.collections) : "";

  return (
    <div className="iconify-search">
      <div className="subhead">Icons</div>
      <input className="text-input" value={input} placeholder="Search Iconify" aria-label="Search Iconify" onChange={(e) => setInput(e.target.value)} />
      {busy && <div className="iconify-status">Searching…</div>}
      {error && <div className="iconify-status">{error}</div>}
      {result && !busy && result.icons.length === 0 && <div className="iconify-status">No icons for “{query}”.</div>}
      {result && result.icons.length > 0 && (
        <div className="thumb-grid">
          {result.icons.map((id) => (
            <IconTile key={id} id={id} />
          ))}
        </div>
      )}
      {credit && <div className="iconify-status">{credit}</div>}
    </div>
  );
}

function IconTile({ id }: { id: string }) {
  const svgRef = useRef<string | null>(null);
  const [preview, setPreview] = useState<ReturnType<typeof iconPreview> | null>(null);
  const [previewError, setPreviewError] = useState(false);

  useEffect(() => {
    let live = true;
    setPreview(null);
    setPreviewError(false);
    svgRef.current = null;
    fetchIconSvg(id).then(
      (svg) => {
        if (!live) return;
        svgRef.current = svg;
        setPreview(iconPreview(svg));
      },
      () => {
        if (live) setPreviewError(true);
      },
    );
    return () => {
      live = false;
    };
  }, [id]);

  const place = async () => {
    try {
      const st = useEditor.getState();
      const args = await iconImportArgs(id, { scene: st.scene, canvas: st.info });
      const r = act<{ id: string }>("import_svg", args);
      if (r) st.select([r.id]);
    } catch (e) {
      useEditor.getState().toast(iconifyMessage(e, "Could not add icon"), "error");
    }
  };

  const copy = async (e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      const svg = (await iconImportArgs(id, { scene: "root" })).svg;
      svgRef.current = svg;
      await navigator.clipboard.writeText(svg);
      useEditor.getState().toast(`Copied ${id}`);
    } catch (e) {
      useEditor.getState().toast(iconifyMessage(e, "Could not copy the icon"), "error");
    }
  };

  return (
    <div
      className="thumb iconify-tile"
      role="button"
      tabIndex={0}
      title={id}
      aria-label={id}
      draggable
      onClick={() => void place()}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          void place();
        }
      }}
      onDragStart={(e) => {
        const svg = svgRef.current;
        if (!svg) {
          e.preventDefault();
          return;
        }
        e.dataTransfer.setData(ICON_MIME, encodeIconDrag(svg, id));
        e.dataTransfer.setData("text/plain", svg);
        e.dataTransfer.effectAllowed = "copy";
      }}
    >
      {preview && !previewError ? (
        <img src={preview.src} className={preview.monochrome ? "iconify-monochrome" : undefined} width={32} height={32} alt="" draggable={false} onError={() => setPreviewError(true)} />
      ) : (
        <span className="iconify-status">{previewError ? "Preview unavailable" : "Loading…"}</span>
      )}
      <button type="button" className="icon-btn mini iconify-copy" title="Copy SVG" aria-label={`Copy ${id}`} onClick={(e) => void copy(e)}>
        <Icon name="copy" size={11} />
      </button>
    </div>
  );
}
