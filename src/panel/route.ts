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

const UUID = /^[0-9a-f-]{8,36}$/i;
const NODES = 20000;

type Fiber = {
  child?: Fiber | null;
  sibling?: Fiber | null;
  memoizedProps?: Record<string, unknown> | null;
};

function rootFiber(): Fiber | null {
  const root = document.getElementById("root") as unknown as Record<string, unknown>;
  if (!root) return null;
  const key = Object.keys(root).find((k) => k.indexOf("__reactContainer") === 0);
  return key ? (root[key] as Fiber) : null;
}

function idIn(props: Record<string, unknown> | null | undefined): string | null {
  if (!props) return null;
  const direct = props.workspaceId;
  if (typeof direct === "string" && UUID.test(direct)) return direct;
  const params = props.params as Record<string, unknown> | undefined;
  const nested = params && params.workspaceId;
  return typeof nested === "string" && UUID.test(nested) ? nested : null;
}

/**
 * The open workspace's id is handed to many components; each sidebar row carries
 * its own once. So the most frequent wins, and a tie means the window is not
 * showing one workspace clearly enough to act on.
 */
export function workspaceId(): string | null {
  const start = rootFiber();
  if (!start) return null;

  const counts = new Map<string, number>();
  const stack: Fiber[] = [start];
  let seen = 0;
  while (stack.length && seen < NODES) {
    const node = stack.pop() as Fiber;
    seen += 1;
    const id = idIn(node.memoizedProps);
    if (id) counts.set(id, (counts.get(id) || 0) + 1);
    if (node.child) stack.push(node.child);
    if (node.sibling) stack.push(node.sibling);
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
  return best && bestCount > runnerUp ? best : null;
}
