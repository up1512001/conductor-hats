/**
 * The account panel injected into Conductor's frontend by `hats patch`.
 *
 * Adds a toolbar button and a composer chip, both opening a two-level panel:
 * providers, then that provider's accounts.
 *
 * Works against the DOM, not Conductor's React code, and finds anchors by product
 * copy because minified names change every release. conductor-acct holds all
 * state; this only reads and writes through it.
 */

import { log } from "./cli.js";
import { loadState } from "./state.js";
import { onRefreshTriggers } from "./store.js";
import { composerChip, refreshTriggers, toolbarButton } from "./triggers.js";
import styles from "./styles.scss";

const VERSION = "0.2.0";

function injectStyles(): void {
  if (document.getElementById("cma-style")) return;
  const style = document.createElement("style");
  style.id = "cma-style";
  style.textContent = styles;
  document.head.appendChild(style);
}

/** Wrapped: a throw inside a compiled bundle is somebody's white screen. */
function tick(): void {
  try {
    injectStyles();
    toolbarButton();
    composerChip();
  } catch (e) {
    log("tick failed", e);
  }
}

function boot(): void {
  onRefreshTriggers(refreshTriggers);
  tick();

  // Re-attach on a coalesced observer rather than fighting React. Both attach
  // paths return immediately when the controls are in place.
  let pending: ReturnType<typeof setTimeout> | null = null;
  new MutationObserver(() => {
    if (pending) return;
    pending = setTimeout(() => {
      pending = null;
      tick();
    }, 250);
  }).observe(document.body, { childList: true, subtree: true });

  // A slow timer, not the observer: refreshing per render pass cost a process
  // spawn each time.
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
