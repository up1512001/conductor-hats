/** Reading React's fiber tree, which is an internal, so every failure is a null.
 *
 * Conductor's webview routes in memory and its class names are hashed per build,
 * so the components themselves are the only stable place the open workspace and
 * chat are written down.
 */

/** How much of the tree to read before giving up. */
export const NODES = 20000;

export type Fiber = {
  child?: Fiber | null;
  sibling?: Fiber | null;
  return?: Fiber | null;
  stateNode?: unknown;
  memoizedProps?: Record<string, unknown> | null;
};


export function rootFiber(): Fiber | null {
  const root = document.getElementById("root") as unknown as Record<string, unknown>;
  if (!root) return null;
  const key = Object.keys(root).find((k) => k.indexOf("__reactContainer") === 0);
  return key ? (root[key] as Fiber) : null;
}

/** React hangs its fiber off the DOM node under a versioned key. */
export function fiberOf(node: Element): Fiber | null {
  const bag = node as unknown as Record<string, unknown>;
  const key = Object.keys(bag).find((k) => k.indexOf("__reactFiber$") === 0);
  return key ? (bag[key] as Fiber) : null;
}

/**
 * The deepest fiber whose element contains the button.
 *
 * Reading `__reactFiber$` off the button's ancestors is the obvious way and it
 * does not work here: measured on a real window, no ancestor of the toolbar
 * carries one. Walking down from the root does work, so the tree is searched for
 * the elements that enclose the button and the innermost is kept. From there
 * `return` climbs the components that own it.
 */
export function containing(button: Element): Fiber | null {
  const start = rootFiber();
  if (!start) return null;

  let best: Fiber | null = null;
  let bestDepth = -1;
  const stack: { node: Fiber; depth: number }[] = [{ node: start, depth: 0 }];
  let seen = 0;
  while (stack.length && seen < NODES) {
    const here = stack.pop() as { node: Fiber; depth: number };
    seen += 1;
    const el = here.node.stateNode as Element | null;
    if (el && typeof (el as Element).contains === "function" && el.contains(button)) {
      if (here.depth > bestDepth) {
        best = here.node;
        bestDepth = here.depth;
      }
    }
    if (here.node.child) stack.push({ node: here.node.child, depth: here.depth + 1 });
    if (here.node.sibling) stack.push({ node: here.node.sibling, depth: here.depth });
  }
  return best;
}

/* The search for the enclosing fiber walks the whole tree, which is too much to
 * repeat while watching for a change of chat. What is kept is the element it
 * found, never the fiber: React replaces a fiber on every render and leaves the
 * old one holding the props it had at the time. Keeping one showed the account of
 * whichever workspace was open when it was cached, for every workspace after it.
 *
 * The element is stable and React hangs the current fiber off it, so reading it
 * back costs nothing and is never out of date. */
let anchoredTo: Element | null = null;
let anchoredHost: Element | null = null;

/**
 * Whether the kept element is still the one wrapping the button.
 *
 * Detached, or no longer containing it, means the window moved on.
 */
function stillHolds(host: Element, button: Element): boolean {
  return host.isConnected && host.contains(button);
}

/**
 * The fiber to start climbing from, for the components around the button.
 *
 * The button is ours: we created it and put it in Conductor's toolbar, so React
 * does not own it and it carries no fiber of its own. Its DOM ancestors carry
 * none either, measured on a real window, so the tree is searched for the
 * innermost element that encloses it.
 */
export function anchorFiber(button: Element): Fiber | null {
  if (anchoredTo === button && anchoredHost && stillHolds(anchoredHost, button)) {
    const live = fiberOf(anchoredHost);
    if (live) return live;
  }

  let node: Fiber | null = null;
  let host: Element | null = button;
  while (host && !node) {
    node = fiberOf(host);
    host = host.parentElement;
  }
  if (!node) node = containing(button);

  anchoredTo = button;
  const found = node && (node.stateNode as Element | null);
  anchoredHost =
    found && typeof found.contains === "function" && found.contains(button) ? found : null;
  return node;
}
