/**
 * The open panel and dialog, plus the hooks views use to request a redraw.
 * Keeping the mutable state here breaks the views/controller import cycle.
 */

import { invalidate, loadState } from "./state.js";
import type { PanelState } from "./state.js";

export interface View {
  level: "root" | "provider";
  agent?: string;
  /** Which layer a selection writes to. Defaults to the workspace, which is what
   * the control in the toolbar names. */
  scope?: "workspace" | "chat";
}

export interface OpenPanel {
  el: HTMLElement;
  anchor: HTMLElement;
  state: PanelState | null;
  error: unknown;
  view: View;
  pos?: { top: number; left: number };
}

export let panel: OpenPanel | null = null;

export function setPanel(next: OpenPanel | null): void {
  panel = next;
}

let dialog: HTMLElement | null = null;

export function openDialog(): HTMLElement | null {
  return dialog;
}

export function setDialog(next: HTMLElement | null): void {
  dialog = next;
}

export function closeDialog(): void {
  if (dialog && dialog.parentNode) dialog.parentNode.removeChild(dialog);
  dialog = null;
}

type Hook = (state: PanelState) => void;

let rerender: () => void = () => {};
let refreshTriggers: Hook = () => {};

export function onRerender(fn: () => void): void {
  rerender = fn;
}

export function onRefreshTriggers(fn: Hook): void {
  refreshTriggers = fn;
}

export function redraw(): void {
  rerender();
}

export function updateTriggers(state: PanelState): void {
  refreshTriggers(state);
}

/** Called after every write: reads past the cache and redraws from one read. */
export function reload(): Promise<void> {
  invalidate();
  return loadState(true).then((st) => {
    if (!panel) return;
    panel.state = st;
    rerender();
    refreshTriggers(st);
  });
}
