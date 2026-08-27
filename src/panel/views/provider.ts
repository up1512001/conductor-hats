/* Level two: one provider's accounts, with a way in and a way out of each. */

import { message } from "../cli.js";
import {
  AGENT_ICON,
  AGENT_LABEL,
  cap,
  effective,
  el,
  footText,
  note,
  pendingChange,
} from "../dom.js";
import { icon } from "../icons.js";
import { applyToWorkspace } from "../state.js";
import type { PanelState, Provider } from "../state.js";
import { panel, redraw, reload } from "../store.js";
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

  const whole = wholeWorkspace(state, provider);
  if (whole) host.appendChild(whole);

  const add = el("button", "cma-add");
  add.type = "button";
  add.appendChild(icon("plus", 12));
  add.appendChild(el("span", null, "Add new account"));
  add.addEventListener("click", () => {
    signInForm(agent, { host, replace: add, profile: null, state });
  });
  host.appendChild(add);

  const moving = pendingChange(provider);
  if (moving) {
    host.appendChild(
      el(
        "div",
        "cma-note",
        "This conversation is on " + cap(effective(provider)) + " and cannot move. " +
          "Reopen it, or start a new one, and it comes up on " + cap(moving) + "."
      )
    );
  }

  host.appendChild(el("div", "cma-note", footText(state)));
}

/**
 * The other thing a person might mean, offered only when it would do something.
 *
 * Choosing an account sets the chat on screen. When that leaves the chat on one
 * account and the workspace on another, this says so and offers to make the
 * chat's account the workspace's too. When they already agree there is nothing
 * to press, so nothing is drawn.
 */
function wholeWorkspace(state: PanelState, provider: Provider): HTMLElement | null {
  if (state.target.kind !== "workspace") return null;
  const chat = effective(provider);
  if (!chat || !provider.session || chat === provider.current) return null;

  const button = el("button", "cma-add cma-whole");
  button.type = "button";
  button.appendChild(el("span", null, "Use " + cap(chat) + " for every chat here"));
  button.addEventListener("click", () => {
    applyToWorkspace(state, provider.agent, chat)
      .then(() => reload())
      .catch((e) => note(panel ? panel.el : button, message(e)));
  });
  return button;
}
