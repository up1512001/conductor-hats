/**
 * The account panel injected into Conductor's frontend by `hats patch`.
 *
 * Adds a toolbar button and a composer chip, both opening a two-level panel:
 * providers, then that provider's accounts.
 *
 * Works against the DOM, not Conductor's React code, and finds anchors by product
 * copy because minified names change every release. hats holds all
 * state; this only reads and writes through it.
 */

import { log } from "./cli.js";
import { invalidate, loadState } from "./state.js";
import { fromToolbar } from "./route.js";
import { onRefreshTriggers } from "./store.js";
import { composerChip, refreshTriggers, toolbarButton } from "./triggers.js";
import styles from "./styles.scss";

const VERSION = "0.4.0";

function injectStyles(): void {
  if (document.getElementById("cma-style")) return;
  const style = document.createElement("style");
  style.id = "cma-style";
  style.textContent = styles;
  document.head.appendChild(style);
}

/**
 * Redraws the toolbar when the chat behind it changes.
 *
 * Each chat can be on its own account, so a label left over from the last one is
 * not stale decoration, it names the wrong account. The button survives the
 * switch, so nothing else prompts a re-read and the old label stayed until the
 * panel was opened by hand.
 *
 * The chat id is read from the components around the button, which is cheap
 * because the fiber it starts from is kept between calls.
 */
let shownFor: string | null = null;

function chatChanged(): void {
  if (!document.getElementById("cma-toolbar-btn")) return;
  const now = fromToolbar().session || "";
  if (now === shownFor) return;
  shownFor = now;
  invalidate();
  loadState(true).then(refreshTriggers).catch(() => {});
}

/** Wrapped: a throw inside a compiled bundle is somebody's white screen. */
function tick(): void {
  try {
    injectStyles();
    toolbarButton();
    composerChip();
    chatChanged();
  } catch (e) {
    log("tick failed", e);
  }
}

function boot(): void {
  onRefreshTriggers(refreshTriggers);
  tick();

  let pending: ReturnType<typeof setTimeout> | null = null;
  new MutationObserver(() => {
    if (pending) return;
    pending = setTimeout(() => {
      pending = null;
      tick();
    }, 250);
  }).observe(document.body, { childList: true, subtree: true });

  setInterval(() => {
    if (
      !document.getElementById("cma-toolbar-btn") &&
      !document.getElementById("cma-chip")
    ) {
      return;
    }
    loadState()
      .then(refreshTriggers)
      .catch(() => {});
  }, 8000);
  log("ready");
}

if (!window.__conductorHats) {
  window.__conductorHats = { version: VERSION };
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", boot);
  } else {
    setTimeout(boot, 800);
  }
}
