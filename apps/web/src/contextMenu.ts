// App-wide right-click menus. The browser menu is suppressed everywhere; each
// area registers its own items via openContextMenu().
import { create } from "zustand";

export interface CtxItem {
  label: string;
  shortcut?: string;
  onSelect?: () => void;
  disabled?: boolean;
  checked?: boolean;
  danger?: boolean;
  submenu?: CtxItem[];
  separator?: boolean;
}

interface CtxState {
  menu: { x: number; y: number; items: CtxItem[] } | null;
  open: (x: number, y: number, items: CtxItem[]) => void;
  close: () => void;
}

export const useContextMenu = create<CtxState>((set) => ({
  menu: null,
  open: (x, y, items) => set({ menu: { x, y, items } }),
  close: () => set({ menu: null }),
}));

export const SEP: CtxItem = { label: "", separator: true };

/** Show a menu for a React mouse event (prevents the native menu). */
export function openContextMenu(e: { preventDefault(): void; stopPropagation(): void; clientX: number; clientY: number }, items: CtxItem[]): void {
  e.preventDefault();
  e.stopPropagation();
  const clean = items.filter((it, i, arr) => !(it.separator && (i === 0 || i === arr.length - 1 || arr[i - 1].separator)));
  useContextMenu.getState().open(e.clientX, e.clientY, clean);
}
