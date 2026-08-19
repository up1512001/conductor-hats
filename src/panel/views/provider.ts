/* Level two: one provider's accounts, with a way in and a way out of each. */

import { AGENT_ICON, AGENT_LABEL, el, footText } from "../dom.js";
import { icon } from "../icons.js";
import type { PanelState } from "../state.js";
import { panel, redraw } from "../store.js";
import { accountSlot } from "./account-row.js";
import { rootView } from "./root.js";
import { signInForm } from "./sign-in.js";

export function providerView(state: PanelState, host: HTMLElement, agent: string): void {
  const provider = state.providers.filter((p) => p.agent === agent)[0];
  if (!provider) {
    if (panel) panel.view = { level: "root" };
    rootView(state, host);
    return;
  }

  const back = el("button", "cma-back");
  back.type = "button";
  back.appendChild(icon("back", 13));
  back.appendChild(el("span", null, "Back"));
  back.addEventListener("click", () => {
    if (!panel) return;
    panel.view = { level: "root" };
    redraw();
  });
  host.appendChild(back);

  const title = el("div", "cma-title");
  title.appendChild(icon(AGENT_ICON[agent] || "chevron", 13));
  title.appendChild(el("span", null, AGENT_LABEL[agent] || agent));
  host.appendChild(title);

  if (!provider.accounts.length) {
    host.appendChild(
      el("div", "cma-note", "No accounts yet. Add one and it becomes selectable here.")
    );
  }
  for (const account of provider.accounts) {
    host.appendChild(accountSlot(state, provider, account));
  }

  const add = el("button", "cma-add");
  add.type = "button";
  add.appendChild(icon("plus", 12));
  add.appendChild(el("span", null, "Add new account"));
  add.addEventListener("click", () => {
    signInForm(agent, { host, replace: add, profile: null, state });
  });
  host.appendChild(add);

  host.appendChild(el("div", "cma-note", footText(state)));
}
