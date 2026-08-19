/* The open panel, the open dialog, and the two hooks that let views ask for a
 * redraw without importing the controller. Keeping the mutable bits here is what
 * stops views and controller from importing each other in a circle.
 */

import { invalidate, loadState } from "./state.js";
import type { PanelState } from "./state.js";

export interface View {
  level: "root" | "provider";
  agent?: string;
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

/* Called after every write. Reads past the cache, redraws the panel in place and
 * updates both trigger labels from the same state, so a switch shows up
 * everywhere at once instead of three reads apart. */
export function reload(): Promise<void> {
  invalidate();
  return loadState(true).then((st) => {
    if (!panel) return;
    panel.state = st;
    rerender();
    refreshTriggers(st);
  });
}
