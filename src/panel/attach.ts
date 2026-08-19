/* Attaching to an app that does not know we exist: sealing pointer events,
 * choosing a mount point, and placing the panel.
 */

import { panel } from "./store.js";

/* Conductor's New Workspace dialog dismisses on pointer events it considers
 * outside itself, and a panel parked on document.body counts as outside. Choosing
 * an account used to dismiss the dialog and lose the typed prompt.
 *
 * The fix has two halves. Mounting inside the dialog puts the panel within
 * Conductor's containment check. Sealing pointer events at the panel's edge covers
 * listeners bound higher up the tree.
 *
 * The seal must run on the bubble phase. On capture it stopped the event before it
 * ever reached the row that was clicked, which is why nothing in the panel
 * responded and why the panel stopped opening at all. */
const SEALED = [
  "mousedown",
  "pointerdown",
  "mouseup",
  "pointerup",
  "click",
  "touchstart",
  "touchend",
];

export function seal(node: HTMLElement): void {
  for (const type of SEALED) {
    node.addEventListener(type, (e) => e.stopPropagation(), false);
  }
}

export function mountFor(anchor: HTMLElement): HTMLElement {
  let node: HTMLElement | null = anchor;
  while (node && node !== document.body) {
    const role = node.getAttribute("role");
    if (
      node.tagName === "DIALOG" ||
      role === "dialog" ||
      role === "alertdialog" ||
      node.getAttribute("aria-modal") === "true"
    ) {
      return node;
    }
    node = node.parentElement;
  }
  return document.body;
}

/* Placed once, when the panel opens, then left alone. Re-measuring on every
 * redraw is what made the panel jump: the provider view is a different height
 * from the root view, and clamping against the right edge moved it sideways too.
 *
 * position:fixed is relative to the nearest ancestor that establishes a
 * containing block, and Conductor animates its dialog with a transform, which
 * does exactly that. Rather than guess which ancestor wins, place the panel,
 * measure where it landed and correct by the difference. */
export function place(node: HTMLElement, anchor: HTMLElement): void {
  if (panel && panel.pos) {
    node.style.top = panel.pos.top + "px";
    node.style.left = panel.pos.left + "px";
    return;
  }
  const a = anchor.getBoundingClientRect();
  const h = node.offsetHeight;
  let wantTop = a.bottom + 6;
  if (wantTop + h > window.innerHeight - 12) {
    wantTop = Math.max(12, a.top - h - 6);
  }
  const wantLeft = Math.max(
    12,
    Math.min(a.left, window.innerWidth - node.offsetWidth - 12)
  );

  node.style.top = Math.round(wantTop) + "px";
  node.style.left = Math.round(wantLeft) + "px";
  const got = node.getBoundingClientRect();
  const dy = wantTop - got.top;
  const dx = wantLeft - got.left;
  const top = Math.round(wantTop + (Math.abs(dy) > 0.5 ? dy : 0));
  const left = Math.round(wantLeft + (Math.abs(dx) > 0.5 ? dx : 0));
  node.style.top = top + "px";
  node.style.left = left + "px";
  if (panel) panel.pos = { top, left };
}
