/* Signing in from the panel, with no terminal.
 *
 * `claude auth login` prints a URL then blocks reading a code from stdin, so
 * `login-start` runs it with stdin on a FIFO and `login-code` feeds the answer
 * in. The browser step is the only part left, because that is what OAuth is.
 *
 * Two callers, one flow. "Add new account" needs a name typed. The sign-in
 * control on a signed-out row already knows which profile it is for, so it skips
 * straight to the button.
 */

import { acct, message, q, sh } from "../cli.js";
import { cap, el } from "../dom.js";
import { maskEmail } from "../mask.js";
import { reload } from "../store.js";
import type { PanelState } from "../state.js";

export interface SignInOptions {
  host: HTMLElement;
  profile?: string | null;
  replace?: HTMLElement | null;
  state?: PanelState | null;
}

/* One live token per account, so two profiles on one address take turns signing
 * each other out. Said here, where it just happened, rather than left for someone
 * to work out from an account that keeps logging out. */
function duplicateOf(
  state: PanelState | null | undefined,
  agent: string,
  profile: string,
  email: string
): string | null {
  if (!email || !state) return null;
  const mine = (state.providers || []).filter((p) => p.agent === agent)[0];
  if (!mine) return null;
  const clash = (mine.accounts || []).filter(
    (a) => a.name !== profile && a.email && a.email === email
  )[0];
  return clash ? clash.name : null;
}

export function signInForm(agent: string, opts: SignInOptions): void {
  if (opts.host.querySelector(".cma-form")) return;
  const fixed = opts.profile || null;
  const form = el("div", "cma-form");

  let name: HTMLInputElement | null = null;
  if (fixed) {
    form.appendChild(el("div", "cma-name", "Sign in to " + cap(fixed)));
  } else {
    name = document.createElement("input");
    name.className = "cma-input";
    name.placeholder = "name, for example work";
    name.spellcheck = false;
    form.appendChild(name);
  }

  const go = el("button", "cma-go", "Sign in");
  go.type = "button";
  const status = el("div", "cma-note", "Your browser opens for approval.");
  form.appendChild(go);
  form.appendChild(status);

  let codeField: HTMLInputElement | null = null;

  function fail(msg: string): void {
    status.textContent = msg;
    go.disabled = false;
  }

  function poll(profile: string, tries: number): void {
    acct(`login-status ${profile} ${agent}`)
      .then((out) => {
        if (/^ok\b/.test(out)) {
          const email = out.slice(2).trim();
          const clash = duplicateOf(opts.state, agent, profile, email);
          if (clash) {
            status.textContent =
              `${cap(clash)} is already signed in as ${maskEmail(email)}. ` +
              "One account cannot be two profiles, so they will sign each other " +
              `out. Remove one with conductor-acct remove ${clash}.`;
            go.remove();
            if (codeField) codeField.remove();
            setTimeout(() => void reload(), 4000);
            return;
          }
          status.textContent = email ? "Signed in as " + maskEmail(email) : "Signed in.";
          setTimeout(() => void reload(), 600);
          return;
        }
        if (/^error/.test(out)) {
          fail(out.replace(/^error\s*/, "") || "sign-in failed");
          return;
        }
        if (tries > 240) {
          fail("timed out waiting for the browser");
          return;
        }
        setTimeout(() => poll(profile, tries + 1), 1000);
      })
      .catch((e) => fail(message(e)));
  }

  function askForCode(profile: string): void {
    if (codeField) return;
    codeField = document.createElement("input");
    codeField.className = "cma-input";
    codeField.placeholder = "paste the code, then Enter";
    codeField.spellcheck = false;
    form.insertBefore(codeField, status);
    codeField.addEventListener("keydown", (e) => {
      if (e.key !== "Enter" || !codeField) return;
      const code = codeField.value.trim();
      if (!code) return;
      status.textContent = "Checking…";
      acct(`login-code ${profile} ${q(code)}`)
        .then(() => poll(profile, 0))
        .catch((err) => fail(message(err)));
    });
    setTimeout(() => codeField?.focus(), 0);
  }

  go.addEventListener("click", () => {
    const profile = fixed || (name ? name.value.trim() : "");
    if (!/^[A-Za-z0-9_-]+$/.test(profile)) {
      fail("Letters, digits, - and _ only.");
      return;
    }
    go.disabled = true;
    status.textContent = "Starting sign-in…";
    acct(`login-start ${profile} ${agent}`)
      .then((url) => {
        status.textContent = "Approve in your browser, then paste the code.";
        sh("open " + q(url)).catch(() => {});
        askForCode(profile);
        poll(profile, 0);
      })
      .catch((e) => fail(message(e)));
  });

  if (name) {
    name.addEventListener("keydown", (e) => {
      if (e.key === "Enter") go.click();
    });
    setTimeout(() => name?.focus(), 0);
  } else {
    setTimeout(() => go.focus(), 0);
  }

  if (opts.replace && opts.replace.parentNode) {
    opts.replace.parentNode.replaceChild(form, opts.replace);
  } else {
    opts.host.appendChild(form);
  }
}
