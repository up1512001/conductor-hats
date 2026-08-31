/**
 * Delivers mobile messages through the composer that Conductor owns.
 *
 * The queue is claimed only for the exact chat read from the toolbar fiber. A
 * non-empty composer is somebody's draft and is never overwritten. The item is
 * removed only when hats can read the delivered user row back from Conductor's
 * database, so a reload after the click does not turn into a lost message.
 */

import { acct, log, q } from "./cli.js";
import { applyControl, controlCommand, type Control } from "./controls.js";
import { fromToolbar } from "./route.js";

interface Claim {
  id: string;
  session: string;
  message: string;
  lease: string;
}

interface RemoteRoute {
  session: string;
  workspace_id: string;
  repository_id: string;
}

interface CreateClaim {
  id: string;
  session: string;
  workspace: string;
  lease: string;
}

let busy = false;
let blockedUntil = 0;
let navigatingUntil = 0;

function parts(): {
  editor: HTMLElement;
  send: HTMLButtonElement;
} | null {
  const host = document.querySelector<HTMLElement>('[data-testid="composer-input"]');
  const editor = host?.querySelector<HTMLElement>('[contenteditable="true"]');
  const send = document.querySelector<HTMLButtonElement>('[data-testid="composer-send-button"]');
  return host && editor && send ? { editor, send } : null;
}

function empty(editor: HTMLElement): boolean {
  return !(editor.innerText || editor.textContent || "").trim();
}

function insert(editor: HTMLElement, message: string): boolean {
  editor.focus();
  const selection = window.getSelection();
  if (!selection) return false;
  const range = document.createRange();
  range.selectNodeContents(editor);
  selection.removeAllRanges();
  selection.addRange(range);
  return document.execCommand("insertText", false, message);
}

function waitFor(test: () => boolean, timeout: number): Promise<boolean> {
  const started = Date.now();
  return new Promise((resolve) => {
    const check = (): void => {
      if (test()) resolve(true);
      else if (Date.now() - started >= timeout) resolve(false);
      else setTimeout(check, 40);
    };
    check();
  });
}

function release(item: Claim): Promise<string> {
  return acct(
    "remote release " + q(item.session) + " " + q(item.id) + " " + q(item.lease)
  ).catch(() => "");
}

function createCommand(action: "complete" | "release", item: CreateClaim): Promise<string> {
  return acct("remote create-" + action + " " + q(JSON.stringify(item))).catch(() => "");
}

function newChatButton(): HTMLElement | null {
  return Array.from(document.querySelectorAll<HTMLElement>("button,[role=button]"))
    .filter((node) => node.getClientRects().length > 0 && !node.hasAttribute("disabled"))
    .find((node) => {
      const hint = [
        node.getAttribute("aria-label"),
        node.getAttribute("title"),
        node.getAttribute("data-tooltip"),
        node.textContent,
      ].filter(Boolean).join(" ");
      return /new chat(?:, same files)?/i.test(hint);
    }) || null;
}

async function chatCreated(item: CreateClaim): Promise<boolean> {
  const raw = await acct("remote create-check " + q(JSON.stringify(item)));
  const result = JSON.parse(raw) as { applied?: boolean };
  return !!result.applied;
}

/** Uses Conductor's visible action, with its documented Command-T command as fallback. */
async function createChat(item: CreateClaim): Promise<boolean> {
  if (await chatCreated(item)) {
    await createCommand("complete", item);
    return true;
  }
  const button = newChatButton();
  if (button) {
    button.click();
  } else {
    const active = document.activeElement as HTMLElement | null;
    active?.blur();
    document.body.dispatchEvent(new KeyboardEvent("keydown", {
      key: "t",
      code: "KeyT",
      metaKey: true,
      bubbles: true,
      cancelable: true,
    }));
  }
  for (let attempt = 0; attempt < 40; attempt += 1) {
    if (await chatCreated(item)) {
      await createCommand("complete", item);
      log("remote new chat created", item.workspace);
      return true;
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  return false;
}


async function openQueuedChat(
  ui: ReturnType<typeof parts>,
  scopeWorkspace: string | null
): Promise<void> {
  if (!scopeWorkspace || Date.now() < navigatingUntil || (ui && !empty(ui.editor))) return;
  const route = JSON.parse(
    await acct("remote next " + q(scopeWorkspace))
  ) as RemoteRoute | null;
  const current = fromToolbar();
  if (!route || current.workspace !== scopeWorkspace || route.session === current.session) return;
  if (!route.repository_id || !route.workspace_id) {
    blockedUntil = Date.now() + 5000;
    return;
  }
  const url =
    "/repository/" + encodeURIComponent(route.repository_id) +
    "/workspace/" + encodeURIComponent(route.workspace_id) +
    "?activeTabType=session&sessionId=" + encodeURIComponent(route.session);
  navigatingUntil = Date.now() + 3000;
  history.pushState(history.state, "", url);
  window.dispatchEvent(new PopStateEvent("popstate", { state: history.state }));
  log("opening chat for remote message", route.session);
}

/**
 * Conductor usually records the user row within a few hundred milliseconds, so
 * the first attempts are close together and only a genuinely slow write falls
 * back to the slower cadence. A flat half-second poll spent most of its time
 * waiting on a row that had already landed, which the phone saw as the message
 * sitting on "Delivering" long after Conductor had it.
 */
async function confirm(item: Claim): Promise<void> {
  for (let attempt = 0; attempt < 70; attempt += 1) {
    const raw = await acct(
      "remote confirm " + q(item.session) + " " + q(item.id) + " " + q(item.lease)
    );
    const result = JSON.parse(raw) as { delivered?: boolean };
    if (result.delivered) return;
    await new Promise((resolve) => setTimeout(resolve, attempt < 12 ? 90 : 400));
  }
}

/** Resolves true when something was delivered, so the caller can drain the rest. */
async function deliver(): Promise<boolean> {
  if (busy || Date.now() < blockedUntil) return false;
  const where = fromToolbar();
  const ui = parts();
  if ((ui && !empty(ui.editor)) || Date.now() < navigatingUntil) return false;
  if (!where.session || !ui) {
    await openQueuedChat(ui, where.workspace);
    return false;
  }

  busy = true;
  let item: Claim | null = null;
  let submitted = false;
  try {
    const taken = JSON.parse(await acct("remote take " + q(where.session))) as {
      control: Control | null;
      create: CreateClaim | null;
      message: Claim | null;
    };
    if (taken.control) {
      if (await applyControl(taken.control)) return true;
      await controlCommand("release", taken.control);
      blockedUntil = Date.now() + 600;
      return false;
    }
    if (taken.create) {
      if (await createChat(taken.create)) return true;
      await createCommand("release", taken.create);
      blockedUntil = Date.now() + 5000;
      return false;
    }
    item = taken.message;
    if (!item) {
      await openQueuedChat(ui, where.workspace);
      return false;
    }
    const still = fromToolbar();
    const live = parts();
    if (still.session !== item.session || !live || !empty(live.editor)) {
      await release(item);
      return false;
    }

    const inserted = insert(live.editor, item.message);
    const ready = inserted && await waitFor(() => !live.send.disabled, 1500);
    if (!ready || fromToolbar().session !== item.session) {
      if (!empty(live.editor)) document.execCommand("undo");
      await release(item);
      blockedUntil = Date.now() + 5000;
      return false;
    }

    live.send.click();
    submitted = true;
    log("remote message submitted", item.id, item.session);
    await confirm(item);
    return true;
  } catch (error) {
    log("remote delivery failed", item?.id || "unclaimed", error);
    if (item && !submitted) await release(item);
    blockedUntil = Date.now() + 5000;
    return false;
  } finally {
    busy = false;
  }
}

/**
 * One claim per tick, and one shell call per idle tick.
 *
 * A phone message waited on this interval before anything looked at it, so the
 * interval was most of the delay a phone could see. Halving it twice would have
 * meant four shell round trips a second while idle, so the two claims were
 * merged into one `remote take` first. After a delivery the next tick runs
 * immediately, which drains a burst of messages at the speed Conductor accepts
 * them rather than one per interval.
 */
export function startRemoteDelivery(): void {
  const tick = (): void => {
    deliver()
      .then((did) => {
        if (did) setTimeout(tick, 0);
      })
      .catch((error) => log("remote delivery tick failed", error));
  };
  setInterval(tick, 250);
}
