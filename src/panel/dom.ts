/* Element helpers and the wording shared across views. */

import type { PanelState } from "./state.js";

export function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  cls?: string | null,
  text?: string | null
): HTMLElementTagNameMap[K] {
  const n = document.createElement(tag);
  if (cls) n.className = cls;
  if (text != null) n.textContent = text;
  return n;
}

export function label(text: string): HTMLDivElement {
  return el("div", "cma-head", text);
}

/* Profile names are lower case on disk, because they are typed at a CLI and used
 * as directory names. They are capitalised for display only: never feed the
 * result back to conductor-acct. */
export function cap(s: string | null | undefined): string {
  const v = String(s || "");
  return v ? v.charAt(0).toUpperCase() + v.slice(1) : v;
}

export const AGENT_LABEL: Record<string, string> = {
  claude: "Claude Code",
  codex: "Codex",
};

export const AGENT_ICON: Record<string, string> = {
  claude: "claude",
  codex: "codex",
};

/* The trigger label shows Claude's account, falling back to whichever provider
 * has one, because that is the one people mean when they glance at it. */
export function primary(state: PanelState): string {
  const providers = state.providers || [];
  const claude = providers.filter((p) => p.agent === "claude")[0];
  if (claude && claude.current) return claude.current;
  const any = providers.filter((p) => p.current)[0];
  return any ? any.current : "";
}

export function scopeText(state: PanelState): string {
  if (state.target.kind === "workspace") return "Workspace: " + state.target.name;
  if (state.target.kind === "repository") return "New workspaces in " + state.target.name;
  return "No workspace in view";
}

export function footText(state: PanelState): string {
  if (state.target.kind === "workspace") {
    return "Applies to the next chat here. A chat already running keeps the account it started on.";
  }
  if (state.target.kind === "repository") {
    return "Applies to workspaces created from now on.";
  }
  return "Open a workspace to choose its account.";
}

/* A transient line, for a failure that does not deserve a dialog. */
export function note(host: Element, text: string): void {
  const n = el("div", "cma-note", text);
  host.appendChild(n);
  setTimeout(() => {
    if (n.parentNode) n.parentNode.removeChild(n);
  }, 4000);
}
