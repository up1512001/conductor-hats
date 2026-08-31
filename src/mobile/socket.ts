/** Pairing, then a reconnecting same-origin socket with no refresh polling. */

import type { MobileCommand, MobileSnapshot } from "./types.js";

const EMPTY: MobileSnapshot = {
  source: "",
  chats: [],
  accounts: {},
  models: {},
  active: null,
};

/**
 * Rebuilds a whole snapshot from the sections the Mac decided were stale.
 *
 * The Mac sends only what moved. Sending everything meant an agent streaming in
 * an unrelated workspace pushed the chat list and the open transcript down the
 * tunnel several times a second. A section that is absent is unchanged, which is
 * why `active` is tested for presence rather than truth: it is legitimately null
 * when no chat is open, and that is different from "you already have it".
 */
function merged(held: MobileSnapshot | null, update: Partial<MobileSnapshot>): MobileSnapshot {
  const base = held || EMPTY;
  return {
    source: update.source ?? base.source,
    chats: update.chats ?? base.chats,
    accounts: update.accounts ?? base.accounts,
    models: update.models ?? base.models,
    active: "active" in update ? update.active ?? null : base.active,
  };
}

interface Handlers {
  state(label: string, live: boolean): void;
  snapshot(value: MobileSnapshot): void;
  event(value: { type: string; value?: unknown; request?: string }): void;
}

export function createTransport(handlers: Handlers): {
  connect(): void;
  send(value: MobileCommand): boolean;
  close(): void;
} {
  let socket: WebSocket | null = null;
  let held: MobileSnapshot | null = null;
  let retry = 0;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let closed = false;

  function address(): string {
    const protocol = location.protocol === "https:" ? "wss:" : "ws:";
    return `${protocol}//${location.host}/ws`;
  }

  function schedule(): void {
    if (closed || timer) return;
    handlers.state("Reconnecting", false);
    const delay = Math.min(12000, 500 * (2 ** Math.min(retry, 5)));
    retry += 1;
    timer = setTimeout(() => {
      timer = null;
      connect();
    }, delay);
  }

  function connect(): void {
    if (socket && socket.readyState < WebSocket.CLOSING) return;
    closed = false;
    handlers.state(retry ? "Reconnecting" : "Connecting", false);
    socket = new WebSocket(address());
    socket.addEventListener("open", () => {
      /* The Mac tracks what it has sent per connection, so a reconnect starts
       * from nothing on both ends. Keeping the old sections would leave the
       * phone showing a chat list the new connection never confirmed. */
      held = null;
      retry = 0;
      handlers.state("Live", true);
    });
    socket.addEventListener("message", (event) => {
      try {
        const value = JSON.parse(String(event.data)) as {
          type?: string;
          value?: unknown;
          request?: string;
        };
        if (value.type === "snapshot") {
          held = merged(held, value as Partial<MobileSnapshot>);
          handlers.snapshot(held);
        }
        else handlers.event({
          type: value.type || "error",
          value: value.value,
          request: value.request,
        });
      } catch (_) {
        handlers.event({ type: "error", value: "The Mac sent an invalid update" });
      }
    });
    socket.addEventListener("close", schedule);
    socket.addEventListener("error", () => socket?.close());
  }

  function send(value: MobileCommand): boolean {
    if (!socket || socket.readyState !== WebSocket.OPEN) {
      handlers.event({ type: "error", value: "Waiting to reconnect to your Mac" });
      schedule();
      return false;
    }
    socket.send(JSON.stringify(value));
    return true;
  }

  document.addEventListener("visibilitychange", () => {
    if (!document.hidden && (!socket || socket.readyState >= WebSocket.CLOSING)) connect();
  });
  window.addEventListener("online", connect);

  return { connect, send, close() { closed = true; socket?.close(); } };
}

/**
 * Spends the one-use token a pairing link carries, before anything connects.
 *
 * The token is taken out of the address bar first so a screenshot, a shared tab
 * or the back button cannot replay it.
 */
export async function pairFromLink(): Promise<void> {
  const token = new URLSearchParams(location.hash.slice(1)).get("token");
  if (!token) return;
  history.replaceState(null, "", location.pathname + location.search);
  const response = await fetch("/api/pair", {
    method: "POST",
    headers: { "X-Hats-Token": token },
    cache: "no-store",
  });
  if (!response.ok) {
    throw new Error("This pairing link expired or was already used. Create a fresh code on the Mac.");
  }
}
