import { useRef } from "react";
import { smPointer } from "./useSmPreview";

/** Transparent layer over the artboard that sends real pointer events to the state machine. */
export function SmPointerOverlay({ zoom }: { zoom: number }) {
  const ref = useRef<HTMLDivElement>(null);
  const down = useRef(false);
  const at = (e: React.PointerEvent): [number, number] => {
    const r = ref.current!.getBoundingClientRect();
    return [(e.clientX - r.left) / zoom, (e.clientY - r.top) / zoom];
  };
  return (
    <div
      ref={ref}
      className="sm-pointer"
      onPointerEnter={(e) => smPointer("enter", ...at(e))}
      onPointerLeave={(e) => smPointer("exit", ...at(e))}
      onPointerMove={(e) => smPointer("move", ...at(e))}
      onPointerDown={(e) => {
        if (e.button !== 0) return;
        down.current = true;
        smPointer("down", ...at(e));
      }}
      onPointerUp={(e) => {
        if (e.button !== 0) return;
        smPointer("up", ...at(e));
        if (down.current) smPointer("click", ...at(e));
        down.current = false;
      }}
    />
  );
}
