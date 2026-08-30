/** Completed and failed run-control receipts from Conductor. */

import type { MobileCommand, MobileSnapshot } from "./types.js";

type Send = (command: MobileCommand) => boolean;
type Notice = (text: string, error?: boolean) => void;

export function receiveControl(
  snapshot: MobileSnapshot,
  send: Send,
  notice: Notice
): string {
  const active = snapshot.active;
  const moved = active?.controls?.find((item) => item.state === "done" && item.result);
  if (moved && active) {
    send({ type: "control-ack", session: active.session, id: moved.id });
    notice("Model opened in a new chat on your Mac");
    return moved.result;
  }
  const failed = active?.controls?.find((item) => item.state === "failed");
  if (failed && active) {
    send({ type: "control-ack", session: active.session, id: failed.id });
    notice(failed.error || "Conductor could not apply that run setting", true);
  }
  return "";
}
