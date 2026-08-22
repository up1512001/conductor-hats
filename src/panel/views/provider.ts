/* Level two: one provider's accounts, with a way in and a way out of each. */

import { AGENT_ICON, AGENT_LABEL, el, footText } from "../dom.js";
import { icon } from "../icons.js";
import type { PanelState, Scope } from "../state.js";
import { panel, redraw } from "../store.js";
import { accountSlot } from "./account-row.js";
import { rootView } from "./root.js";
import { signInForm } from "./sign-in.js";

/* Two layers can hold an account, and which one a click writes to has to be
 * visible rather than implied. Only offered when a chat is actually live: with
 * nothing to pin the control would be a dead option. */
function scopeSwitch(host: HTMLElement, scope: Scope): void {
  const bar = el("div", "cma-scope");
  const seg = (value: Scope, label: string, hint: string): void => {
    const b = el("button", "cma-seg" + (scope === value ? " cma-seg-on" : ""));
    b.type = "button";
    b.setAttribute("role", "tab");
    b.setAttribute("aria-selected", scope === value ? "true" : "false");
    b.title = hint;
    b.appendChild(el("span", null, label));
    b.addEventListener("click", () => {
      if (!panel || panel.view.scope === value) return;
      panel.view = { ...panel.view, scope: value };
      redraw();
    });
    bar.appendChild(b);
  };
  seg("workspace", "This workspace", "The account new chats here start on");
  seg("chat", "This chat", "The account this conversation restarts on");
  host.appendChild(bar);
}

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

  const live = Boolean(provider.session);
  const scope: Scope = live && panel?.view.scope === "chat" ? "chat" : "workspace";
  if (live) scopeSwitch(host, scope);

  if (!provider.accounts.length) {
    host.appendChild(
      el("div", "cma-note", "No accounts yet. Add one and it becomes selectable here.")
    );
  }
  for (const account of provider.accounts) {
    host.appendChild(accountSlot(state, provider, account, scope));
  }

  const add = el("button", "cma-add");
  add.type = "button";
  add.appendChild(icon("plus", 12));
  add.appendChild(el("span", null, "Add new account"));
  add.addEventListener("click", () => {
    signInForm(agent, { host, replace: add, profile: null, state });
  });
  host.appendChild(add);

  host.appendChild(el("div", "cma-note", footText(state, scope)));
}
