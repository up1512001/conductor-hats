/** Level one: providers, the routing switch, and the placeholder states. */

import { acct, cliPath, log, message } from "../cli.js";
import {
  AGENT_ICON,
  AGENT_LABEL,
  effective,
  el,
  footText,
  label,
  note,
  scopeText,
} from "../dom.js";
import { icon } from "../icons.js";
import type { PanelState, Provider } from "../state.js";
import { panel, redraw, reload } from "../store.js";

function providerCard(provider: Provider): HTMLElement {
  const card = el("button", "cma-card");
  card.type = "button";
  card.appendChild(icon(AGENT_ICON[provider.agent] || "chevron", 13));

  const main = el("div", "cma-grow");
  main.appendChild(el("div", "cma-name", AGENT_LABEL[provider.agent] || provider.agent));
  const n = provider.accounts.length;
  main.appendChild(el("div", "cma-sub", n === 1 ? "1 Account" : n + " Accounts"));
  card.appendChild(main);

  /* What the chat on screen will run on, which is what the toolbar says. Reading
   * the workspace route here instead made level one and the toolbar disagree in
   * plain sight: the route said Personal while the chat, carrying a pin, said
   * Work. Both were true and neither was the same question. */
  const shown = effective(provider);
  const badge = shown
    ? shown.charAt(0).toUpperCase() + shown.slice(1)
    : provider.accounts.length
      ? "Not set"
      : "None";
  card.appendChild(el("span", "cma-badge", badge));
  card.appendChild(icon("chevron", 13));

  card.addEventListener("click", () => {
    if (!panel) return;
    panel.view = { level: "provider", agent: provider.agent };
    redraw();
  });
  return card;
}

export function rootView(state: PanelState, host: HTMLElement): void {
  host.appendChild(label(scopeText(state)));
  for (const p of state.providers) host.appendChild(providerCard(p));

  host.appendChild(el("div", "cma-sep"));

  const toggle = el("button", "cma-card");
  toggle.type = "button";
  const main = el("div", "cma-grow");
  main.appendChild(
    el("div", "cma-name", state.enabled ? "Turn routing off" : "Turn routing on")
  );
  main.appendChild(
    el(
      "div",
      "cma-sub",
      state.enabled ? "agents go back to one account" : "one account per workspace"
    )
  );
  toggle.appendChild(main);
  toggle.addEventListener("click", () => {
    acct(state.enabled ? "uninstall" : "install")
      .then(() => reload())
      .catch((e) => note(host, message(e)));
  });
  host.appendChild(toggle);

  host.appendChild(el("div", "cma-note", footText(state)));
}

/**
 * Shown while the first read is in flight. The height has to be about right: the
 * corner is pinned on this measurement, so a two-row panel would decide it fits
 * below the anchor and then grow off screen.
 */
export function loadingView(host: HTMLElement): void {
  host.appendChild(label("Loading accounts"));
  for (const agent of ["claude", "codex"]) {
    const card = el("div", "cma-card cma-ghost");
    card.appendChild(icon(AGENT_ICON[agent] || "chevron", 13));
    const main = el("div", "cma-grow");
    main.appendChild(el("div", "cma-name", AGENT_LABEL[agent] || agent));
    main.appendChild(el("div", "cma-sub", "reading accounts"));
    card.appendChild(main);
    host.appendChild(card);
  }
  host.appendChild(el("div", "cma-sep"));
  host.appendChild(
    el("div", "cma-note", "hats is answering. This is quick once warmed up.")
  );
}

/** A dead button teaches nobody anything: show the failure in the panel. */
export function errorView(host: HTMLElement, e: unknown): void {
  host.appendChild(label("Accounts unavailable"));
  const n = el("div", "cma-note", message(e));
  n.appendChild(el("code", "cma-code", cliPath() + " json"));
  host.appendChild(n);
  log("panel failed", e);
}
