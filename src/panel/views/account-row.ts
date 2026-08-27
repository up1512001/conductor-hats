/* One account: pick it, or sign it out. */

import { acct, message } from "../cli.js";
import { AGENT_ICON, AGENT_LABEL, cap, el, note, effective } from "../dom.js";
import { dialog } from "../dialog.js";
import { icon } from "../icons.js";
import { maskEmail } from "../mask.js";
import { applyAccount } from "../state.js";
import type { Account, PanelState, Provider } from "../state.js";
import { panel, reload } from "../store.js";
import { signInForm } from "./sign-in.js";

export function accountSlot(
  state: PanelState,
  provider: Provider,
  account: Account
): HTMLElement {
  /* The tick follows what the chat on screen will actually run on. Reading the
   * workspace route instead would tick one account while messages went to
   * another, which is the misreporting this exists to end. */
  const active = account.name === effective(provider);
  const slot = el("div", "cma-slot");
  const row = el("div", "cma-row2");

  const card = el("button", "cma-card");
  card.type = "button";
  card.setAttribute("role", "menuitemradio");
  card.setAttribute("aria-checked", active ? "true" : "false");

  /* The provider's mark, so a row says whose account it is without reading the
   * heading above it. The same profile name can exist under both. */
  const mark = el("span", "cma-mark");
  mark.appendChild(icon(AGENT_ICON[provider.agent] || "chevron", 14));
  mark.title = AGENT_LABEL[provider.agent] || provider.agent;
  card.appendChild(mark);

  const main = el("div", "cma-grow");
  const shown = account.email ? maskEmail(account.email) : cap(account.name);
  const line = el("div", "cma-name" + (account.email ? " cma-mask" : ""), shown);
  if (account.email) line.setAttribute("aria-label", "email hidden");
  main.appendChild(line);
  main.appendChild(
    el(
      "div",
      "cma-sub",
      account.email ? cap(account.name) : account.signedIn ? "Signed in" : "Not signed in"
    )
  );
  card.appendChild(main);

  const tickslot = el("div", "cma-tickslot");
  if (active) tickslot.appendChild(icon("tick", 13));
  card.appendChild(tickslot);

  /* With no place identified there is no chat either: `hats json` was asked
   * without a path, so anything it reported came from the shell's directory
   * rather than from the window. Pinning on that would pin some other chat. */
  if (state.target.kind === "none") {
    row.setAttribute("aria-disabled", "true");
    card.setAttribute("aria-disabled", "true");
    card.title = "Open a workspace, or the New Workspace dialog, to pick an account";
  } else {
    card.addEventListener("click", () => {
      applyAccount(state, provider.agent, account.name)
        .then(() => reload())
        .catch((e) => note(panel ? panel.el : row, message(e)));
    });
  }
  row.appendChild(card);

  if (account.signedIn) {
    const out = el("button", "cma-signout");
    out.type = "button";
    out.title = "Sign out of " + cap(account.name);
    out.setAttribute("aria-label", "Sign out of " + cap(account.name));
    out.appendChild(icon("signout", 14));
    out.addEventListener("click", () => confirmSignOut(provider, account));
    row.appendChild(out);
  } else {
    const back = el("button", "cma-signout cma-signin");
    back.type = "button";
    back.title = "Sign in to " + cap(account.name);
    back.setAttribute("aria-label", "Sign in to " + cap(account.name));
    back.appendChild(icon("signin", 14));
    back.addEventListener("click", () => {
      signInForm(provider.agent, { host: slot, profile: account.name, state });
    });
    row.appendChild(back);
  }

  slot.appendChild(row);
  return slot;
}

/**
 * Signs out and nothing else: the profile, its routes, its session pins and its
 * transcripts all stay. Deleting a profile is `hats remove` in a
 * terminal, deliberately, since it is the one irreversible operation here.
 */
export function confirmSignOut(provider: Provider, account: Account): void {
  dialog({
    title: "Sign out of " + cap(account.name) + "?",
    body:
      "Signs " +
      (account.email ? maskEmail(account.email) : cap(account.name)) +
      " out of " +
      (AGENT_LABEL[provider.agent] || provider.agent) +
      ". Nothing else changes: the account stays in this list, and its routes, " +
      "sessions and transcripts are untouched. Sign back in from here whenever " +
      "you like.",
    confirm: "Sign out",
    danger: true,
    onConfirm: (done, fail) => {
      acct(`logout ${account.name} ${provider.agent}`)
        .then(() => {
          done();
          void reload();
        })
        .catch((e) => fail(message(e)));
    },
  });
}
