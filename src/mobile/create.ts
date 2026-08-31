/** New-chat state that survives redraws and is acknowledged only after DB readback. */

import type { MobileCommand, MobileSnapshot } from "./types.js";

type Send = (command: MobileCommand) => boolean;
type Notice = (text: string, error?: boolean) => void;

export function createChatManager(send: Send, notice: Notice): {
  bind(root: HTMLElement, snapshot: MobileSnapshot): void;
  workspace(): string;
  resume(): string;
  receive(snapshot: MobileSnapshot): string;
  fail(message: string, request: string | undefined): boolean;
} {
  let pending: { session: string; workspace: string; request: string } | null = null;

  function bind(root: HTMLElement, snapshot: MobileSnapshot): void {
    for (const button of root.querySelectorAll<HTMLButtonElement>("[data-new-chat]")) {
      button.disabled = !!pending;
      button.addEventListener("click", () => {
        const chat = snapshot.chats.find((item) => item.session === button.dataset.newChat);
        if (!chat || pending) return;
        const request = crypto.randomUUID();
        pending = { session: chat.session, workspace: chat.workspace_id, request };
        send({ type: "subscribe", session: chat.session });
        if (!send({ type: "new-chat", session: chat.session, request })) {
          pending = null;
          notice("Reconnect to your Mac before creating a chat", true);
        }
      });
    }
  }

  function receive(snapshot: MobileSnapshot): string {
    const created = pending && snapshot.active?.session === pending.session
      ? snapshot.active.creates?.[0]
      : null;
    if (created?.state !== "done") return "";
    send({ type: "create-ack", id: created.id });
    pending = null;
    if (created.error) {
      notice(created.error, true);
      return "";
    }
    if (created.result) notice("New chat opened on your Mac");
    return created.result;
  }

  return {
    bind,
    workspace: () => pending?.workspace || "",
    resume: () => pending?.session || "",
    receive,
    fail(message: string, request: string | undefined): boolean {
      if (!pending || pending.request !== request) return false;
      pending = null;
      notice(message, true);
      return true;
    },
  };
}
