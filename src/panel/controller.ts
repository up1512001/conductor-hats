/** Opening, drawing and closing the panel. */

import { mountFor, place, seal } from "./attach.js";
import { el } from "./dom.js";
import { loadState } from "./state.js";
import {
  closeDialog,
  onRerender,
  openDialog,
  panel,
  setPanel,
  updateTriggers,
} from "./store.js";
import { errorView, loadingView, rootView } from "./views/root.js";
import { providerView } from "./views/provider.js";

export function closePanel(): void {
  closeDialog();
  if (panel?.el.parentNode) panel.el.parentNode.removeChild(panel.el);
  panel?.anchor.setAttribute("aria-expanded", "false");
  setPanel(null);
  document.removeEventListener("mousedown", onDocDown, true);
  document.removeEventListener("keydown", onDocKey, true);
}

/** The dialog is a sibling, not a descendant, so both handlers must ignore it. */

function onDocDown(e: MouseEvent): void {
  if (!panel || openDialog()) return;
  const target = e.target as Node;
  if (panel.el.contains(target)) return;
  if (panel.anchor.contains(target)) return;
  closePanel();
}

function onDocKey(e: KeyboardEvent): void {
  if (e.key !== "Escape" || !panel || openDialog()) return;
  if (panel.view.level === "provider") {
    panel.view = { level: "root" };
    render();
    return;
  }
  closePanel();
}

export function render(): void {
  if (!panel) return;
  const host = panel.el;
  host.replaceChildren();
  if (panel.error) errorView(host, panel.error);
  else if (!panel.state) loadingView(host);
  else if (panel.view.level === "provider" && panel.view.agent) {
    providerView(panel.state, host, panel.view.agent);
  } else rootView(panel.state, host);
  place(host, panel.anchor);
}

onRerender(render);

function listen(): void {
  setTimeout(() => {
    document.addEventListener("mousedown", onDocDown, true);
    document.addEventListener("keydown", onDocKey, true);
  }, 0);
}

/** Opens on the first event rather than after a round trip. */
export function togglePanel(anchor: HTMLElement): void {
  if (panel) {
    const same = panel.anchor === anchor;
    closePanel();
    if (same) return;
  }
  anchor.setAttribute("aria-expanded", "true");
  const node = el("div", "cma-panel");
  node.setAttribute("role", "menu");
  seal(node);
  mountFor(anchor).appendChild(node);
  setPanel({ el: node, anchor, state: null, error: null, view: { level: "root" } });
  render();
  listen();

  loadState(false, anchor.id === "cma-chip" ? "repository" : "workspace")
    .then((state) => {
      /* Closed, or reopened against another trigger, while this was in flight. */
      if (!panel || panel.el !== node) return;
      panel.state = state;
      render();
      updateTriggers(state);
    })
    .catch((e) => {
      if (!panel || panel.el !== node) return;
      panel.error = e;
      render();
    });
}

/**
 * Opens on pointerdown, not click. Conductor rebuilds its toolbar constantly, and
 * a rebuild between mousedown and mouseup means no click is fired at all. Click is
 * kept for keyboard activation, guarded against double-toggling.
 */
export function openOnPress(trigger: HTMLElement): void {
  let pressedAt = 0;
  trigger.addEventListener("pointerdown", (e) => {
    if (e.button !== undefined && e.button !== 0) return;
    e.preventDefault();
    pressedAt = Date.now();
    togglePanel(trigger);
  });
  trigger.addEventListener("click", (e) => {
    e.preventDefault();
    if (Date.now() - pressedAt < 700) return;
    togglePanel(trigger);
  });
}
