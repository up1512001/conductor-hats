/**
 * Conductor's own model picker, driven through the handlers it already mounted.
 *
 * The picker holds three things a phone setting needs: the models Conductor is
 * willing to show, the configuration the chat is running, and the function that
 * applies a new one. Calling that function does exactly what a click on the Mac
 * does, including the mid-chat rules Conductor attaches to a change.
 *
 * For effort it is the only honest route. Conductor's composer control for
 * effort is a bar meter bound to `chat.toggleThinking`, and a click advances to
 * the next level rather than opening a menu, so driving that control applies
 * whichever level happens to come next instead of the one that was asked for.
 * The levels are only listed in the picker's own Effort submenu.
 */

import { log } from "./cli.js";
import { NODES, rootFiber, type Fiber } from "./fiber.js";

export interface Configuration {
  model?: string;
  thinkingLevel?: string;
}

type Handler = (...args: unknown[]) => unknown;

interface Effort {
  apply: Handler;
  configuration: Configuration;
}

function record(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object" ? (value as Record<string, unknown>) : null;
}

/** Whether this node is rendered for the chat the setting belongs to. */
function hasSession(node: Fiber, session: string): boolean {
  for (let here: Fiber | null | undefined = node, hops = 0; here && hops < 60;
    here = here.return, hops += 1) {
    const props = here.memoizedProps;
    const params = record(props?.params);
    const current = record(props?.session);
    const candidates = [
      props?.sessionId,
      props?.selectedSessionId,
      props?.viewSessionId,
      props?.targetSessionId,
      params?.sessionId,
      current?.id,
    ];
    if (candidates.includes(session)) return true;
  }
  return false;
}

/**
 * The first match for the chat, or the only unambiguous one otherwise.
 *
 * Several Conductor windows can be open on one screen, and each mounts its own
 * picker. A node that names the session is exact; without one, a single
 * candidate is still safe and two different ones are not.
 */
function walk<T>(session: string, read: (node: Fiber) => T | null, same: (a: T, b: T) => boolean): T | null {
  const start = rootFiber();
  if (!start) return null;
  const stack: Fiber[] = [start];
  let seen = 0;
  let current: T | null = null;
  let ambiguous = false;
  while (stack.length && seen < NODES) {
    const node = stack.pop() as Fiber;
    seen += 1;
    const found = read(node);
    if (found) {
      if (hasSession(node, session)) return found;
      ambiguous ||= current !== null && !same(current, found);
      current ||= found;
    }
    if (node.child) stack.push(node.child);
    if (node.sibling) stack.push(node.sibling);
  }
  return ambiguous ? null : current;
}

/** The handler behind a row of the picker's model list. */
export function modelHandler(session: string, value: string, before: string): Handler | null {
  return walk<Handler>(
    session,
    (node) => {
      const props = node.memoizedProps;
      const visible = record(props?.visibleBuiltInModelIds);
      const includes = visible && Object.values(visible).some((models) =>
        Array.isArray(models) && models.includes(value)
      );
      if (!includes || typeof props?.onSelect !== "function") return null;
      if (hasSession(node, session)) return props.onSelect as Handler;
      return props.selectedModel === before ? (props.onSelect as Handler) : null;
    },
    (a, b) => a === b
  );
}

/**
 * The picker's apply function paired with the configuration it is holding.
 *
 * Conductor refuses a configuration whose model is not the one the chat is on,
 * so the current configuration is carried through rather than rebuilt: only the
 * level changes.
 */
function effortTarget(session: string): Effort | null {
  return walk<Effort>(
    session,
    (node) => {
      const props = node.memoizedProps;
      const apply = props?.onApplyConfiguration;
      if (typeof apply !== "function") return null;
      const held = record(props?.configuration);
      if (held && typeof held.model === "string") {
        return { apply: apply as Handler, configuration: held as Configuration };
      }
      const rows = props?.getRowConfiguration;
      const selected = props?.selectedModel;
      if (typeof rows !== "function" || typeof selected !== "string") return null;
      const built = record((rows as Handler)(selected));
      return built && typeof built.model === "string"
        ? { apply: apply as Handler, configuration: built as Configuration }
        : null;
    },
    (a, b) => a.apply === b.apply
  );
}

export async function applyEffort(session: string, value: string): Promise<boolean> {
  const target = effortTarget(session);
  if (!target || target.configuration.thinkingLevel === undefined) return false;
  try {
    await target.apply({ ...target.configuration, thinkingLevel: value });
    return true;
  } catch (error) {
    log("Conductor effort handler failed", error);
    return false;
  }
}
