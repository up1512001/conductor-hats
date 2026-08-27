/** Element helpers and the wording shared across views. */

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

/** Display only. Profile names are lower case on disk; never feed this back. */
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

/** Claude's account, or whichever provider has one. */
/**
 * What the chat on screen will actually run on, not what the workspace route
 * says. A chat that started before the route changed keeps the account it
 * started with, so reading the route here made the label claim one account
 * while messages went to another.
 */
export function effective(p: { current: string; chat?: string; session?: string }): string {
  return p.session && p.chat ? p.chat : p.current;
}

export function primary(state: PanelState): string {
  const providers = state.providers || [];
  const claude = providers.filter((p) => p.agent === "claude")[0];
  if (claude && effective(claude)) return effective(claude);
  const any = providers.filter((p) => effective(p))[0];
  return any ? effective(any) : "";
}

/**
 * Whether Conductor has a chat open here, which decides what a choice writes.
 * With one open the toolbar sets that chat; without one there is nothing to pin
 * and it sets the workspace instead.
 */
export function openChat(state: PanelState): boolean {
  if (state.chatId) return true;
  return (state.providers || []).filter((p) => !!p.session).length > 0;
}

export function scopeText(state: PanelState): string {
  if (state.scope === "repository") {
    return state.target.name ? "New workspace in " + state.target.name : "New workspace";
  }
  if (openChat(state)) {
    return state.target.name ? "This chat in " + state.target.name : "This chat";
  }
  if (state.target.kind === "workspace") return "Workspace: " + state.target.name;
  if (state.target.kind === "repository") return "The next workspace in " + state.target.name;
  return "No workspace in view";
}

export function footText(state: PanelState): string {
  if (state.scope === "repository") {
    return "Applies to the workspace you create next, and to that one alone.";
  }
  if (openChat(state)) {
    return "Applies to this chat alone. It comes up on the new account next time you open it; the conversation on screen keeps the one it started with.";
  }
  if (state.target.kind === "workspace") {
    return "No chat open, so this sets the workspace. Chats started here use it unless they are set on their own.";
  }
  if (state.target.kind === "repository") {
    return "No chat here yet, so this applies to the workspace you create next.";
  }
  return "Open a workspace to choose its account.";
}

/** A transient line, for a failure that does not deserve a dialog. */
export function note(host: Element, text: string): void {
  const n = el("div", "cma-note", text);
  host.appendChild(n);
  setTimeout(() => {
    if (n.parentNode) n.parentNode.removeChild(n);
  }, 4000);
}
