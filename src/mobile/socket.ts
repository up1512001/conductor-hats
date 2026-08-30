/** Reconnecting same-origin WebSocket transport with no refresh polling. */

import type { MobileCommand, MobileSnapshot } from "./types.js";

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
        if (value.type === "snapshot") handlers.snapshot(value as MobileSnapshot);
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
