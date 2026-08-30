/** Conductor's own list, chat and composer surfaces, drawn for a phone. */

import type { Account, ActiveChat, Chat, MobileSnapshot, Project, TranscriptLine } from "./types.js";
import { chevron, markdown, esc, when } from "./markup.js";

function status(value: string): string {
  if (value === "working") return "Working";
  if (value === "error") return "Needs attention";
  return "Ready";
}

function initial(name: string): string {
  return esc(name.slice(0, 1).toUpperCase());
}

/**
 * A chat as one flat row: state, what it is, and how full it is.
 *
 * Model and thinking are settings the next message carries, so they live in the
 * composer. Repeating them here buried the title under small grey text.
 */
function chatRow(chat: Chat): string {
  const sub = [status(chat.status), chat.agent, chat.on || chat.next].filter(Boolean).join(" · ");
  return `<button class="row" data-session="${esc(chat.session)}">
    <span class="dot ${esc(chat.status)}"></span>
    <span class="copy"><span class="name">${esc(chat.title)}</span>
      <span class="sub">${esc(sub)}</span></span>
    <span class="trail">
      ${chat.pending ? `<span class="count pending">${chat.pending}</span>` : ""}
      ${chat.unread ? `<span class="count unread">${chat.unread}</span>` : ""}
      <span>${Math.round(chat.context || 0)}%</span>
    </span>
  </button>`;
}

export function projectsView(projects: Project[], source: string): string {
  if (!projects.length) return '<p class="empty">No open Conductor projects.</p>';
  const chats = projects.reduce((sum, project) => sum + project.chats.length, 0);
  const working = projects.reduce(
    (sum, project) => sum + project.chats.filter((chat) => chat.status === "working").length,
    0
  );
  const rows = projects.map((project) => {
    const spaces = new Set(project.chats.map((chat) => chat.workspace)).size;
    const busy = project.chats.filter((chat) => chat.status === "working").length;
    return `<button class="row" data-project="${esc(project.key)}">
      <span class="mark">${initial(project.name)}</span>
      <span class="copy"><span class="name">${esc(project.name)}</span>
        <span class="sub">${spaces} workspace${spaces === 1 ? "" : "s"} · ${project.chats.length} chat${project.chats.length === 1 ? "" : "s"}</span></span>
      <span class="trail">${busy ? `<span class="dot working"></span>` : ""}${chevron}</span>
    </button>`;
  }).join("");
  return `<div class="eyebrow">${esc(source || "Conductor")} · ${chats} chat${chats === 1 ? "" : "s"}${working ? ` · ${working} working` : ""}</div>
    ${rows}`;
}

function newChatButton(session: string, workspace: string, busy: boolean): string {
  const label = busy ? "Creating…" : "New chat";
  return `<button type="button" class="new-chat ${busy ? "busy" : ""}"
    data-new-chat="${esc(session)}" aria-label="${esc(label + " in " + workspace)}"
    ${busy ? "disabled" : ""}><span>+</span>${esc(label)}</button>`;
}

export function projectView(project: Project, creatingWorkspace = ""): string {
  const spaces = new Map<string, Chat[]>();
  for (const chat of project.chats) {
    if (!spaces.has(chat.workspace)) spaces.set(chat.workspace, []);
    spaces.get(chat.workspace)?.push(chat);
  }
  const groups = Array.from(spaces.entries()).map(([workspace, chats]) => {
    const first = chats[0];
    if (!first) return "";
    return `<div class="eyebrow workspace-head"><span>${esc(workspace)}</span>
      ${newChatButton(first.session, workspace, creatingWorkspace === first.workspace_id)}</div>
      ${chats.map(chatRow).join("")}`;
  }).join("");
  return `<section class="head"><div class="head-top"><span class="mark big">${initial(project.name)}</span>
    <div><h2>${esc(project.name)}</h2><p>${spaces.size} workspace${spaces.size === 1 ? "" : "s"} on your Mac</p></div></div></section>
    ${groups}`;
}

function tool(line: TranscriptLine): string {
  const body = line.detail || line.text;
  const summary = line.kind === "tool_result" ? (line.text.split("\n")[0] || "Result") : line.text;
  const glyph = line.failed ? "!" : line.kind === "thinking" ? "✦" : line.kind === "tool_result" ? "↳" : "›";
  return `<details class="tool ${line.kind === "thinking" ? "thinking" : ""} ${line.failed ? "failed" : ""}">
    <summary><span class="tool-icon">${glyph}</span><span class="tool-name">${esc(line.name)}</span><span class="tool-detail">${esc(summary)}</span></summary>
    ${body ? `<pre class="tool-body">${esc(body)}</pre>` : ""}</details>`;
}

function transcriptLine(line: TranscriptLine): string {
  if (line.kind === "say") {
    const me = line.role === "user";
    return `<article class="turn ${me ? "me" : "them"}">
      <div class="who"><time>${esc(when(line.at))}</time></div>
      <div class="md">${markdown(line.text)}</div></article>`;
  }
  if (["tool", "tool_result", "thinking"].includes(line.kind)) return tool(line);
  return `<div class="event ${line.failed ? "error" : ""}"><b>${esc(line.name)}</b><span>${esc(line.text)}</span></div>`;
}

/** What a queued reply is actually doing, in words rather than a spinner. */
function queueState(state: string): string {
  if (state === "sending") return "Sending in Conductor";
  if (state === "unavailable") return "Not open in this Conductor";
  return "Waiting for Conductor";
}

export function chatView(
  chat: Chat | null,
  active: ActiveChat | null,
  echo: string[] = [],
  creatingWorkspace = ""
): string {
  if (!chat) return '<p class="empty">This chat is no longer open on the Mac.</p>';
  const subscribed = active?.session === chat.session;
  const current = subscribed ? active : { transcript: [], outbox: [] };
  const band = Math.min(10, Math.max(0, Math.round((chat.context || 0) / 10)));
  const lines = (current.transcript || []).map(transcriptLine).join("");
  const queued = (current.outbox || []).map((item) => `<article class="turn me waiting">
    <div class="who"><time>${esc(queueState(item.state))}</time></div>
    <div class="md">${markdown(item.message)}</div></article>`).join("");
  const local = echo.map((text) => `<article class="turn me waiting">
    <div class="who"><time>Sending</time></div>
    <div class="md">${markdown(text)}</div></article>`).join("");
  const account = chat.on || chat.next;
  return `<section class="head"><div class="chat-title"><h2>${esc(chat.title || "Untitled")}</h2>
    ${newChatButton(chat.session, chat.workspace, creatingWorkspace === chat.workspace_id)}</div>
    <div class="head-path">${esc(chat.project)} / ${esc(chat.workspace)}</div>
    <div class="tags">${account ? `<span class="tag account">${esc(account)}</span>` : ""}
      <span class="tag">${esc(chat.agent)}</span>
      ${chat.personality && chat.personality !== "default" ? `<span class="tag">${esc(chat.personality)}</span>` : ""}</div>
    <div class="gauge"><span>Context</span><span class="gauge-track"><span class="g${band}"></span></span>
      <span>${Math.round(chat.context || 0)}%${chat.context_tokens ? ` · ${Math.round(chat.context_tokens / 1000)}k` : ""}</span></div></section>
    <section class="thread">${lines + queued + local || `<p class="empty">${subscribed ? "No messages yet." : "Loading conversation…"}</p>`}</section>`;
}

const GLYPH: Record<string, string> = {
  account: '<svg viewBox="0 0 24 24"><circle cx="12" cy="8" r="3.5"/><path d="M5 20c.7-4 3-6 7-6s6.3 2 7 6"/></svg>',
  model: '<svg viewBox="0 0 24 24"><path d="M12 3v18M3 12h18M6 6l12 12M18 6L6 18"/></svg>',
  effort: '<svg viewBox="0 0 24 24"><path d="M6 20V10M12 20V4M18 20v-6"/></svg>',
  fast: '<svg viewBox="0 0 24 24"><path d="M13 2L4 14h7l-1 8 9-12h-7z"/></svg>',
  permission: '<svg viewBox="0 0 24 24"><path d="M12 3l8 4v5c0 5-3.4 8.3-8 9-4.6-.7-8-4-8-9V7z"/></svg>',
};

/**
 * Conductor's own composer row: the run settings sit under the message, each as
 * an icon and its current value, and open in place.
 *
 * They belong here rather than on a chat row or behind a sheet, because they are
 * what the next message is sent with. This is the arrangement Conductor uses on
 * the Mac, so the phone reads the same way round.
 */
export function composerControls(chat: Chat | null, active: ActiveChat | null): string {
  if (!chat) return "";
  const pending = active?.session === chat.session && !!active.controls?.length;
  const held = (setting: string, fallback: string): string =>
    (active?.session === chat.session ? active.controls || [] : [])
      .find((item) => item.setting === setting)?.value || fallback;
  const button = (setting: string, label: string): string =>
    `<button type="button" class="tool-btn ${pending ? "busy" : ""}" data-control="${esc(setting)}"
      aria-label="${esc(setting)}">${GLYPH[setting] || ""}${label ? `<span>${esc(label)}</span>` : ""}</button>`;
  return button("account", chat.next || chat.on || "Account")
    + button("model", held("model", chat.model) || "Model")
    + button("effort", held("effort", chat.effort) || "Thinking")
    + button("fast", held("fast", chat.fast ? "on" : "off") === "on" ? "Fast" : "")
    + button("permission", held("permission", chat.permission || "default") === "default" ? "" : held("permission", chat.permission || "default"));
}

/** The values one control offers, as a menu anchored to its button. */
export function controlMenu(
  chat: Chat,
  setting: string,
  accounts: Record<string, Account[]>,
  models: Record<string, string[]>
): string {
  const profiles = (accounts?.[chat.agent] || []).filter((item) => item.signed_in);
  const lists: Record<string, string[]> = {
    account: Array.from(new Set([chat.next, ...profiles.map((item) => item.name)].filter(Boolean))),
    model: models?.[chat.agent] || [],
    effort: chat.agent === "codex"
      ? ["low", "medium", "high", "xhigh"]
      : ["low", "medium", "high", "max"],
    fast: ["off", "on"],
    permission: ["default", "plan", "acceptEdits", "bypassPermissions"],
  };
  const now: Record<string, string> = {
    account: chat.next || chat.on,
    model: chat.model,
    effort: chat.effort,
    fast: chat.fast ? "on" : "off",
    permission: chat.permission || "default",
  };
  const current = now[setting] || "";
  const values = Array.from(new Set([...(lists[setting] || []), current].filter(Boolean)));
  return `<div class="menu">${values.map((value) =>
    `<button type="button" class="menu-item ${value === current ? "on" : ""}"
      data-setting="${esc(setting)}" data-value="${esc(value)}"
      data-current="${esc(current)}">${esc(value)}</button>`
  ).join("")}</div>`;
}

export function settingsView(snapshot: MobileSnapshot): string {
  const working = (snapshot.chats || []).filter((chat) => chat.status === "working").length;
  const queued = (snapshot.chats || []).reduce((sum, chat) => sum + (chat.pending || 0), 0);
  return `<section class="info"><dl>
    <div><dt>Connected to</dt><dd>${esc(snapshot.source || "Conductor")}</dd></div>
    <div><dt>Open chats</dt><dd>${snapshot.chats?.length || 0}</dd></div>
    <div><dt>Agents working</dt><dd>${working}</dd></div>
    <div><dt>Replies waiting</dt><dd>${queued}</dd></div>
    <div><dt>Delivery</dt><dd>Conductor composer</dd></div>
    </dl></section>
    <p class="note">Everything is read from and written to your Mac. The tunnel opens no router port. Stop or revoke this connection from Mobile access in Conductor.</p>`;
}
