/** Conductor's own list, chat and composer surfaces, drawn for a phone. */

import type { ActiveChat, Chat, MobileSnapshot, Project, TranscriptLine } from "./types.js";
import { chevron, markdown, esc, when } from "./markup.js";
import { activityCluster, activityLine } from "./activity.js";

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

function transcriptLine(line: TranscriptLine): string {
  if (line.kind === "say") {
    const me = line.role === "user";
    return `<article class="turn ${me ? "me" : "them"}">
      <div class="who"><time>${esc(when(line.at))}</time></div>
      <div class="md">${markdown(line.text)}</div></article>`;
  }
  if (line.kind === "thinking") return activityLine(line);
  return `<div class="event ${line.failed ? "error" : ""}"><b>${esc(line.name)}</b><span>${esc(line.text)}</span></div>`;
}

function transcript(lines: TranscriptLine[]): string {
  const out: string[] = [];
  for (let at = 0; at < lines.length;) {
    const line = lines[at] as TranscriptLine;
    if (line.kind !== "tool" && line.kind !== "tool_result") {
      out.push(transcriptLine(line));
      at += 1;
      continue;
    }
    const group: TranscriptLine[] = [];
    while (at < lines.length && ["tool", "tool_result"].includes(lines[at]?.kind || "")) {
      group.push(lines[at] as TranscriptLine);
      at += 1;
    }
    out.push(activityCluster(group));
  }
  return out.join("");
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
  const lines = transcript(current.transcript || []);
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
