/* The account panel injected into Conductor's frontend.
 *
 * tools/patch-ui.py appends the built bundle to Conductor's main asset. It adds a
 * button in the workspace toolbar, next to "Open in", and a chip in the New
 * Workspace composer. Both open the same panel: providers first, then that
 * provider's accounts, each with a way to sign in or out, and one "Add new
 * account" at the foot.
 *
 * The panel never deletes anything. Signing out drops that account's credentials
 * and leaves its profile, routes, session pins and transcripts alone. Deleting a
 * profile is `conductor-acct remove` in a terminal, because it is the one
 * irreversible operation here and a popover you can open by accident is the wrong
 * place for it.
 *
 * Everything works against the DOM rather than by editing Conductor's React code.
 * Minified component names change on every release. "The element next to the one
 * whose tooltip says Open in" mostly does not.
 *
 * conductor-acct holds all state. This code shells out to it, so the CLI, the
 * /account command and this panel cannot disagree.
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

/* Wrapped because this runs inside a compiled bundle, where a thrown exception is
 * somebody's white screen. */
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

  /* Conductor re-renders constantly, so rather than fight React, re-attach on a
   * coalesced observer. Cheap: both attach paths return immediately when the
   * controls are already in place, and neither reads any state. */
  let pending: ReturnType<typeof setTimeout> | null = null;
  new MutationObserver(() => {
    if (pending) return;
    pending = setTimeout(() => {
      pending = null;
      tick();
    }, 250);
  }).observe(document.body, { childList: true, subtree: true });

  /* The labels used to be refreshed by the observer, which meant a process spawn
   * per render pass. A slow timer keeps them current at a fixed, small cost
   * instead. Switching from a terminal shows up within a few seconds, and
   * switching from the panel is immediate because the panel already knows. */
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

if (!window.__conductorMultiAccount) {
  window.__conductorMultiAccount = { version: VERSION };
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", boot);
  } else {
    setTimeout(boot, 800);
  }
}
