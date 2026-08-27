/**
 * Which workspace the window is showing, taken from Conductor's own components.
 *
 * Matching workspace names against the visible chrome is a guess: a workspace and
 * the repository holding it are both named on screen, and a sidebar names every
 * workspace there is. Conductor's components are handed the id of the workspace
 * they belong to, so the window already knows exactly which one it is.
 *
 * Read from React's fiber tree, which is an internal, so every failure falls back
 * to matching by name rather than breaking the panel.
 */

import { anchorFiber, NODES, rootFiber } from "./fiber.js";
import type { Fiber } from "./fiber.js";

const UUID = /^[0-9a-f-]{8,36}$/i;


/**
 * A workspace id, and whether it came from the router's matched parameters.
 *
 * The distinction is the whole of it. Every sidebar row is handed the id of the
 * workspace it links to, so a window listing thirty workspaces holds thirty ids
 * and counting them cannot say which one is open. Only the route that matched
 * carries its id under `params`, and there is one of those.
 */
function idIn(props: Record<string, unknown> | null | undefined): Found | null {
  if (!props) return null;
  const params = props.params as Record<string, unknown> | undefined;
  const routed = params && params.workspaceId;
  if (typeof routed === "string" && UUID.test(routed)) {
    return { id: routed, routed: true };
  }
  const direct = props.workspaceId;
  if (typeof direct === "string" && UUID.test(direct)) {
    return { id: direct, routed: false };
  }
  return null;
}

interface Found {
  id: string;
  routed: boolean;
}

/**
 * The open workspace's id is handed to many components; each sidebar row carries
 * its own once. So the most frequent wins, and a tie means the window is not
 * showing one workspace clearly enough to act on.
 */
export interface Scan {
  id: string | null;
  /** How much of the tree was read, and how many ids it held: the two numbers
   * that separate "no id in the window" from "the tree was never walked". */
  fibers: number;
  distinct: number;
  /** Which reading answered: the tree above the button, the matched route, the
   * commonest id, or neither. */
  how: "anchor" | "routed" | "counted" | "none";
}



/**
 * The id held by the components the toolbar button sits inside.
 *
 * This is the reading that works. Counting ids across the whole tree cannot say
 * which workspace is open, because every sidebar row is handed the id of the one
 * it links to: measured on a real window, 3897 fibers held 35 distinct ids and
 * no winner. The button, though, is mounted in the workspace's own toolbar, so
 * every component enclosing it belongs to the workspace on screen and the
 * sidebar is nowhere among them.
 */
function uuid(value: unknown): string | null {
  return typeof value === "string" && UUID.test(value) ? value : null;
}

/** What the components enclosing the toolbar button know about where they are. */
export interface Anchored {
  workspace: string | null;
  /** Conductor's own id for the chat, which is what it passes the agent as
   * `--session-id` and therefore what a pin has to be filed under. */
  session: string | null;
  /** Whether a React fiber was reachable at all, which separates "the toolbar
   * knows nothing" from "we never found the tree". */
  attached: boolean;
  hops: number;
}




export function fromToolbar(): Anchored {
  /* The button is ours: we created it and put it in Conductor's toolbar, so React
   * does not own it and it carries no fiber of its own. */
  const button = document.getElementById("cma-toolbar-btn");
  const node0 = button ? anchorFiber(button) : null;
  let node: Fiber | null = node0;

  const out: Anchored = { workspace: null, session: null, attached: !!node, hops: 0 };
  let climbed: Fiber | null = node;
  while (climbed && out.hops < 600 && !(out.workspace && out.session)) {
    const props = climbed.memoizedProps;
    if (props) {
      const params = props.params as Record<string, unknown> | undefined;
      if (!out.workspace) {
        out.workspace = uuid(params && params.workspaceId) || uuid(props.workspaceId);
      }
      if (!out.session) {
        out.session = uuid(params && params.sessionId) || uuid(props.sessionId);
      }
    }
    if (!out.session) {
      out.session = sessionBeside(climbed);
    }
    climbed = climbed.return || null;
    out.hops += 1;
  }
  return out;
}

/**
 * One chat's id from the components sitting beside the button.
 *
 * The toolbar carries the chat's own status, the "Working…" and its spinner, and
 * those are handed the id of the chat they report on. They are siblings of the
 * button rather than its ancestors, so climbing alone never sees them. Each
 * enclosing level is swept on the way up and the first that holds exactly one
 * chat is the answer: one id means one chat, and the sweep is still inside the
 * toolbar rather than out in the sidebar where every row would offer its own.
 */
function sessionBeside(top: Fiber): string | null {
  const found = new Set<string>();
  const stack: Fiber[] = [top];
  let seen = 0;
  while (stack.length && seen < 4000) {
    const node = stack.pop() as Fiber;
    seen += 1;
    const props = node.memoizedProps;
    if (props) {
      const params = props.params as Record<string, unknown> | undefined;
      const id = uuid(params && params.sessionId) || uuid(props.sessionId);
      if (id) {
        found.add(id);
        if (found.size > 1) return null;
      }
    }
    if (node.child) stack.push(node.child);
    if (node !== top && node.sibling) stack.push(node.sibling);
  }
  return found.size === 1 ? (found.values().next().value as string) : null;
}

function fromAnchor(): string | null {
  return fromToolbar().workspace;
}

export function workspaceId(): Scan {
  const above = fromAnchor();
  if (above) {
    return { id: above, fibers: 0, distinct: 0, how: "anchor" };
  }

  const start = rootFiber();
  if (!start) return { id: null, fibers: 0, distinct: 0, how: "none" };

  const counts = new Map<string, number>();
  const routed = new Map<string, number>();
  const stack: Fiber[] = [start];
  let seen = 0;
  while (stack.length && seen < NODES) {
    const node = stack.pop() as Fiber;
    seen += 1;
    const found = idIn(node.memoizedProps);
    if (found) {
      counts.set(found.id, (counts.get(found.id) || 0) + 1);
      if (found.routed) routed.set(found.id, (routed.get(found.id) || 0) + 1);
    }
    if (node.child) stack.push(node.child);
    if (node.sibling) stack.push(node.sibling);
  }

  /* One matched route means one open workspace. Counting is only reached when
   * the router put nothing in reach, and it is a guess: a sidebar listing thirty
   * workspaces holds thirty ids, none of them the answer. */
  if (routed.size === 1) {
    const only = routed.keys().next().value as string;
    return { id: only, fibers: seen, distinct: counts.size, how: "routed" };
  }

  let best = "";
  let bestCount = 0;
  let runnerUp = 0;
  counts.forEach((count, id) => {
    if (count > bestCount) {
      runnerUp = bestCount;
      bestCount = count;
      best = id;
    } else if (count > runnerUp) {
      runnerUp = count;
    }
  });
  const won = best && bestCount > runnerUp ? best : null;
  return {
    id: won,
    fibers: seen,
    distinct: counts.size,
    how: won ? "counted" : "none",
  };
}
