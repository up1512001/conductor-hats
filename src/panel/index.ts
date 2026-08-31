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
import { startModelCatalogSync } from "./model_catalog.js";
import { startRemoteDelivery } from "./remote.js";
import { onRefreshTriggers, panel, redraw } from "./store.js";
import {
  composerChip,
  mobileButton,
  refreshMobileTrigger,
  refreshTriggers,
  toolbarButton,
} from "./triggers.js";
import styles from "./styles.scss";

const VERSION = "0.5.0";

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

let reading = false;

function chatChanged(): void {
  if (reading || !document.getElementById("cma-toolbar-btn")) return;
  const at = fromToolbar();
  /* The workspace as well as the chat. Moving between workspaces is the case
   * that left "This chat in macau" on screen while amman was open. */
  const now = (at.workspace || "") + "/" + (at.session || "");
  if (now === shownFor) return;
  shownFor = now;
  reading = true;
  invalidate();
  loadState(true)
    .then((state) => {
      refreshTriggers(state);
      /* An open panel is describing the place that just changed underneath it. */
      if (panel) {
        panel.state = state;
        redraw();
      }
    })
    .catch(() => {})
    .then(() => {
      reading = false;
    });
}

/** Wrapped: a throw inside a compiled bundle is somebody's white screen. */
function tick(): void {
  try {
    injectStyles();
    toolbarButton();
    mobileButton();
    composerChip();
    chatChanged();
  } catch (e) {
    log("tick failed", e);
  }
}

function boot(): void {
  onRefreshTriggers(refreshTriggers);
  tick();
  startModelCatalogSync();
  startRemoteDelivery();

  let pending: ReturnType<typeof setTimeout> | null = null;
  new MutationObserver(() => {
    if (pending) return;
    pending = setTimeout(() => {
      pending = null;
      tick();
    }, 250);
  }).observe(document.body, { childList: true, subtree: true });

  /* Watched on its own short interval as well as on mutation. A switch between
   * chats need not touch the toolbar's own subtree, so waiting for the observer
   * to notice left the wrong account on screen for as long as the window was
   * still. Reading the chat id is a walk up a kept fiber and a string compare. */
  setInterval(chatChanged, 200);

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
    refreshMobileTrigger();
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
