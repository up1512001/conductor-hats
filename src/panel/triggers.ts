/* The two controls that open the panel: a button in the workspace toolbar and a
 * chip in the New Workspace composer.
 *
 * Both are found by product copy rather than by class name. Class names are hashed
 * per build; "the control whose tooltip says Open in" and "the field whose
 * placeholder says What do you want to work on" survive releases that rename every
 * identifier.
 */

import { message } from "./cli.js";
import { openOnPress } from "./controller.js";
import { cap, el, primary } from "./dom.js";
import { loadState } from "./state.js";
import type { PanelState } from "./state.js";

function findOpenIn(): HTMLElement | null {
  const nodes = document.querySelectorAll<HTMLElement>(
    "button,[role=button],a,[data-slot=button]"
  );
  /* Accessible name first, then visible text, then the app icon. Conductor renders
   * this control as an icon, so which of the three carries the words varies. */
  for (const n of nodes) {
    const lbl = (
      n.getAttribute("aria-label") ||
      n.getAttribute("title") ||
      n.getAttribute("data-tooltip") ||
      ""
    ).trim();
    if (/open (in|remote)/i.test(lbl)) return n;
  }
  for (const n of nodes) {
    const t = (n.textContent || "").trim();
    if (t.length < 24 && /open in/i.test(t)) return n;
  }
  let icon: HTMLElement | null = document.querySelector(
    'img[src*="app-icons"],img[src*="finder.png"]'
  );
  while (icon && icon !== document.body) {
    if (icon.tagName === "BUTTON" || icon.getAttribute("role") === "button") return icon;
    icon = icon.parentElement;
  }
  return null;
}

/* When the toolbar cannot be found the button goes top right instead of silently
 * not existing. A control in slightly the wrong place is debuggable; nothing at
 * all looks identical to the script never having run. */
function floatingHost(): HTMLElement {
  const existing = document.getElementById("cma-float");
  if (existing) return existing;
  const host = el("div");
  host.id = "cma-float";
  host.style.cssText = "position:fixed;top:9px;right:14px;z-index:99998";
  document.body.appendChild(host);
  return host;
}

let missedToolbar = 0;

export function toolbarButton(): void {
  const existing = document.getElementById("cma-toolbar-btn");
  const anchor = findOpenIn();
  let host: HTMLElement;
  let before: HTMLElement | null = null;

  if (anchor && anchor.parentElement) {
    host = anchor.parentElement;
    before = anchor;
    missedToolbar = 0;
  } else {
    /* Give the real toolbar a few render passes before giving up on it. */
    if (++missedToolbar < 8) return;
    host = floatingHost();
  }

  /* Left alone while it is still in the document and still in the right place.
   * Re-reading the label here ran on every render pass Conductor made, which
   * during a streaming chat is several a second, each one a process spawn. */
  if (existing && existing.isConnected && existing.parentElement === host) return;
  if (existing && existing.parentNode) existing.parentNode.removeChild(existing);

  const btn = el("button", "cma-btn");
  btn.id = "cma-toolbar-btn";
  btn.type = "button";
  btn.setAttribute("aria-label", "Agent account");
  btn.hidden = true;
  btn.appendChild(el("span", "cma-label"));
  openOnPress(btn);

  if (before) host.insertBefore(btn, before);
  else host.appendChild(btn);
  refreshToolbarLabel(btn);
}

export function refreshToolbarLabel(btn: HTMLElement, state?: PanelState): void {
  const apply = (s: PanelState): void => {
    const cur = primary(s);
    const lbl = btn.querySelector(".cma-label");
    if (lbl) lbl.textContent = cap(cur) || (s.enabled ? "Default" : "Off");
    btn.title = cur ? "Agent account: " + cap(cur) : "No account chosen here";
    (btn as HTMLButtonElement).hidden = false;
  };
  if (state) {
    apply(state);
    return;
  }
  loadState(false, "workspace")
    .then(apply)
    .catch((e) => {
      const lbl = btn.querySelector(".cma-label");
      if (lbl) lbl.textContent = "Account?";
      btn.title = "hats did not answer: " + message(e);
      (btn as HTMLButtonElement).hidden = false;
    });
}

function findComposer(): HTMLElement | null {
  const els = document.querySelectorAll<HTMLElement>("[placeholder],[data-placeholder]");
  for (const node of els) {
    const p =
      node.getAttribute("placeholder") || node.getAttribute("data-placeholder") || "";
    if (/what do you want to work on/i.test(p)) return node;
  }
  return null;
}

/* Walk up to the composer card, then take its last row: the one holding the model
 * picker and the Create button. */
function composerFooter(node: HTMLElement): HTMLElement | null {
  let e: HTMLElement | null = node;
  for (let i = 0; i < 8 && e; i++, e = e.parentElement) {
    const rows = e.querySelectorAll<HTMLElement>(":scope > div");
    if (rows.length >= 2) {
      const last = rows[rows.length - 1];
      if (last && last.querySelector("button") && last.textContent!.length < 400) {
        return last;
      }
    }
  }
  return null;
}

export function composerChip(): void {
  const composer = findComposer();
  if (!composer) return;
  const foot = composerFooter(composer);
  if (!foot) return;
  if (foot.querySelector("#cma-chip")) return;

  const chip = el("button", "cma-chip");
  chip.id = "cma-chip";
  chip.type = "button";
  chip.hidden = true;
  chip.appendChild(el("span", "cma-label"));
  openOnPress(chip);
  foot.insertBefore(chip, foot.firstChild);
  refreshComposerChip();
}

export function refreshComposerChip(state?: PanelState): void {
  const chip = document.getElementById("cma-chip") as HTMLButtonElement | null;
  if (!chip) return;
  const apply = (s: PanelState): void => {
    const lbl = chip.querySelector(".cma-label");
    const name = cap(primary(s)) || "Default account";
    if (lbl) lbl.textContent = name;
    chip.title = "This workspace will run agents on: " + name;
    chip.hidden = false;
  };
  if (state) {
    apply(state);
    return;
  }
  loadState(false, "repository").then(apply).catch(() => {});
}

/* Both refreshed from state already in hand. They can each fetch their own, and do
 * on first attach, but a switch would then cost three reads of the same thing and
 * the labels would visibly lag the tick. */
export function refreshTriggers(state: PanelState): void {
  const btn = document.getElementById("cma-toolbar-btn");
  if (btn) refreshToolbarLabel(btn, state);
  refreshComposerChip(state);
}
