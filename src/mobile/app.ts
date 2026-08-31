/** Mobile navigation and actions over one live WebSocket state. */

import { createTransport, pairFromLink } from "./socket.js";
import { createChatManager } from "./create.js";
import { receiveControl } from "./control.js";
import { echoManager } from "./echo.js";
import { hold, restore, type Held } from "./place.js";
import { noticeFor } from "./notice.js";
import { groupProjects } from "./projects.js";
import { composerControls, controlMenu } from "./composer.js";
import { chatView, projectView, projectsView, settingsView } from "./render.js";
import type { Chat, MobileSnapshot } from "./types.js";

const view = document.getElementById("view") as HTMLElement;
const title = document.getElementById("title") as HTMLElement;
const back = document.getElementById("back") as HTMLButtonElement;
const settings = document.getElementById("settings") as HTMLButtonElement;
const connection = document.getElementById("connection") as HTMLElement;
const notice = document.getElementById("notice") as HTMLElement;
const composer = document.getElementById("composer") as HTMLFormElement;
const composerTools = document.getElementById("composer-tools") as HTMLElement;
const message = document.getElementById("message") as HTMLTextAreaElement;
const send = document.getElementById("send") as HTMLButtonElement;
const sendState = document.getElementById("send-state") as HTMLElement;

let snapshot: MobileSnapshot = {
  source: "",
  chats: [],
  accounts: {},
  models: {},
  active: null,
};
let connectionLabel = "Connecting";
let connectionLive = false;
type Screen =
  | { level: "projects" }
  | { level: "project"; project: string }
  | { level: "chat"; session: string; project: string }
  | { level: "settings" };
let screen: Screen = { level: "projects" };
/** The chat the transcript on screen belongs to, so a first draw can jump to its end. */
let drawn = "";
const echoes = echoManager();
const showNotice = noticeFor(notice);

function selectedChat(): Chat | null {
  if (screen.level !== "chat") return null;
  const session = screen.session;
  return snapshot.chats.find((chat) => chat.session === session) || null;
}

/**
 * Only an empty box stops a send.
 *
 * A chat lets you type the next thing while the last one is still on its way;
 * blocking the composer until the Mac acknowledged made every message a
 * round trip the sender had to wait out, which is not how a chat behaves.
 */
function updateSend(): void {
  send.disabled = !message.value.trim();
}

function openProject(key: string): void {
  screen = { level: "project", project: key };
  render();
}

function openChat(session: string): void {
  const chat = snapshot.chats.find((item) => item.session === session);
  screen = { level: "chat", session, project: chat?.repository_id || chat?.project_path || "projects" };
  echoes.clear();
  transport.send({ type: "subscribe", session });
  render();
}

function bind(selector: string, event: string, task: EventListener): void {
  for (const node of view.querySelectorAll<HTMLElement>(selector)) node.addEventListener(event, task);
}

function bindComposer(selector: string, event: string, task: EventListener): void {
  for (const node of composerTools.querySelectorAll<HTMLElement>(selector)) {
    node.addEventListener(event, task);
  }
}

function render(): void {
  const currentScreen = screen;
  const grouped = groupProjects(snapshot.chats || []);
  const root = currentScreen.level === "projects";
  back.classList.toggle("is-hidden", root);
  back.disabled = root;
  back.tabIndex = root ? -1 : 0;
  composer.hidden = currentScreen.level !== "chat";
  if (currentScreen.level !== "chat") drawn = "";
  settings.classList.toggle("selected", currentScreen.level === "settings");
  connection.textContent = snapshot.source
    ? snapshot.source + " · " + connectionLabel
    : connectionLabel;
  connection.className = connectionLive ? "live" : "";
  if (currentScreen.level === "projects") {
    title.textContent = "Projects";
    composerTools.replaceChildren();
    view.innerHTML = projectsView(grouped, snapshot.source);
    bind("[data-project]", "click", (event) => {
      const node = event.currentTarget as HTMLElement;
      if (node.dataset.project) openProject(node.dataset.project);
    });
    return;
  }
  if (currentScreen.level === "project") {
    composerTools.replaceChildren();
    const project = grouped.find((item) => item.key === currentScreen.project);
    if (!project) {
      screen = { level: "projects" };
      render();
      return;
    }
    title.textContent = project.name;
    view.innerHTML = projectView(project, creation.workspace());
    bind("[data-session]", "click", (event) => {
      const node = event.currentTarget as HTMLElement;
      if (node.dataset.session) openChat(node.dataset.session);
    });
    creation.bind(view, snapshot);
    return;
  }
  if (currentScreen.level === "settings") {
    title.textContent = "Mobile access";
    composerTools.replaceChildren();
    view.innerHTML = settingsView(snapshot);
    return;
  }
  const chat = selectedChat();
  composer.hidden = !chat;
  message.disabled = !chat;
  const opening = drawn !== currentScreen.session;
  drawn = currentScreen.session;
  const held: Held = opening
    ? { atBottom: true, above: 0, open: new Set<string>() }
    : hold(view);
  title.textContent = chat?.workspace || "Chat";
  view.innerHTML = chatView(chat, snapshot.active, echoes.texts(), creation.workspace());
  composerTools.innerHTML = composerControls(chat, snapshot.active);
  bindComposer("[data-control]", "click", openControl);
  creation.bind(view, snapshot);
  updateSend();
  restore(view, held);
}

/** Opens one run setting as a menu over its button, the way Conductor does. */
function openControl(event: Event): void {
  const button = event.currentTarget as HTMLElement;
  const chat = selectedChat();
  const setting = button.dataset.control;
  if (!chat || !setting) return;
  composerTools.querySelector(".menu")?.remove();
  if (button.classList.contains("open")) {
    button.classList.remove("open");
    return;
  }
  for (const other of composerTools.querySelectorAll(".open")) other.classList.remove("open");
  button.classList.add("open");
  button.insertAdjacentHTML("afterend", controlMenu(chat, setting, snapshot.accounts, snapshot.models));
  for (const item of composerTools.querySelectorAll<HTMLElement>(".menu-item")) {
    item.addEventListener("click", chooseControl);
  }
}

function chooseControl(event: Event): void {
  const item = event.currentTarget as HTMLElement;
  if (screen.level !== "chat" || !item.dataset.value) return;
  composerTools.querySelector(".menu")?.remove();
  for (const other of composerTools.querySelectorAll(".open")) other.classList.remove("open");
  if (item.dataset.value === item.dataset.current) return;
  sendState.textContent = "Applying on your Mac";
  transport.send({
    type: item.dataset.setting === "account" ? "account" : "control",
    session: screen.session,
    setting: item.dataset.setting || "",
    value: item.dataset.value,
    before: item.dataset.current || "",
  });
}

function goBack(): void {
  if (screen.level === "chat") {
    transport.send({ type: "subscribe", session: "" });
    screen = { level: "project", project: screen.project };
  } else {
    screen = { level: "projects" };
  }
  render();
}

const creation = createChatManager(
  (command) => transport.send(command),
  showNotice
);

const transport = createTransport({
  state(label: string, live: boolean): void {
    connectionLabel = label;
    connectionLive = live;
    if (live) {
      const session = creation.resume() || (screen.level === "chat" ? screen.session : "");
      if (session) transport.send({ type: "subscribe", session });
    }
    render();
  },
  snapshot(value: MobileSnapshot): void {
    snapshot = value;
    const created = creation.receive(value);
    if (created) {
      openChat(created);
      return;
    }
    const moved = receiveControl(value, (command) => transport.send(command), showNotice);
    if (moved) {
      openChat(moved);
      return;
    }
    echoes.receive(value);
    sendState.textContent = value.active?.outbox?.length
      ? "Delivering through Conductor…"
      : value.active?.controls?.length
        ? "Applying run settings in Conductor…"
        : "Live on your Mac";
    render();
  },
  event(value: { type: string; value?: unknown; request?: string }): void {
    if (value.type === "error") {
      sendState.textContent = String(value.value || "Could not apply that change");
      creation.fail(String(value.value || "Could not create the chat"), value.request);
      const failed = echoes.reject(value.request);
      if (failed) {
        if (!message.value) message.value = failed;
        showNotice(String(value.value || "Could not send that message"), true);
        render();
      }
      updateSend();
    }
    if (value.type === "accepted") sendState.textContent = "Delivering through Conductor";
    if (value.type === "applied") {
      sendState.textContent = "Setting applied on your Mac";
      updateSend();
    }
    if (value.type === "accepted-setting") sendState.textContent = "Applying in Conductor on your Mac…";
    if (value.type === "accepted-new-chat") showNotice("Creating a new chat on your Mac…");
  },
});

back.addEventListener("click", goBack);
settings.addEventListener("click", () => {
  screen = screen.level === "settings" ? { level: "projects" } : { level: "settings" };
  render();
});
/**
 * The message leaves the box the moment it is sent.
 *
 * A chat clears its input on send and shows what you wrote straight away; it
 * does not hold your words hostage until a server agrees. The reply appears in
 * the thread immediately from the local echo, is replaced by the Mac's own copy
 * when the next snapshot arrives, and is put back in the box only if the socket
 * refused to carry it.
 */
composer.addEventListener("submit", (event) => {
  event.preventDefault();
  const text = message.value.trim();
  if (screen.level !== "chat" || !text) return;
  const request = crypto.randomUUID();
  if (!transport.send({ type: "send", session: screen.session, message: text, request })) {
    sendState.textContent = "Waiting to reconnect to your Mac";
    return;
  }
  echoes.add(snapshot, request, text);
  message.value = "";
  updateSend();
  render();
});
message.addEventListener("input", updateSend);
message.addEventListener("keydown", (event) => {
  if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) composer.requestSubmit();
});

pairFromLink().then(() => transport.connect()).catch((error: unknown) => {
  connection.textContent = "Not paired";
  const note = document.createElement("p");
  note.className = "empty";
  note.textContent = error instanceof Error ? error.message : String(error);
  view.replaceChildren(note);
});
