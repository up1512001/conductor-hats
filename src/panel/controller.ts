/* Opening, drawing and closing the panel. */

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

/* A confirmation dialog is a sibling of the panel, not a descendant, so both of
 * these would otherwise treat interacting with it as clicking away and pull the
 * panel out from under it. */
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

/* Opens on the first event rather than after a round trip. Waiting for the state
 * read before showing anything is what made a press look ignored, and pressing
 * again then only toggled the panel that had not appeared yet. */
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

  loadState()
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

/* Opens on press rather than on click, for one specific reason. Conductor
 * re-renders its toolbar constantly, and when React replaces the container the
 * trigger is rebuilt with it. A rebuild landing between mousedown and mouseup
 * means the browser fires no click at all, so the press did nothing and you
 * pressed again. A single event cannot be split that way.
 *
 * This is also how native menus behave, so it feels faster besides. Click is
 * still handled for keyboard activation, guarded so a real pointer press does not
 * toggle twice. */
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
