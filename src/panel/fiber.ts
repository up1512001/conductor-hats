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
 * repeat while looking for a change of chat. The button is the same element from
 * one render to the next, so the answer is kept against it. */
let anchoredTo: Element | null = null;
let anchoredFiber: Fiber | null = null;

/**
 * Whether a kept fiber is still the live one.
 *
 * React replaces a fiber on re-render and leaves the old one holding the props
 * it had at the time. Reusing one of those would report whichever chat was open
 * when it was cached, for ever, which is the bug this cache exists to fix.
 */
function stillHolds(node: Fiber, button: Element): boolean {
  const el = node.stateNode as Element | null;
  return (
    !!el &&
    typeof (el as Element).contains === "function" &&
    el.isConnected &&
    el.contains(button)
  );
}

/**
 * The fiber to start climbing from, for the components around the button.
 *
 * The button is ours: we created it and put it in Conductor's toolbar, so React
 * does not own it and it carries no fiber of its own. Its DOM ancestors carry
 * none either, measured on a real window, so the tree is searched instead. That
 * search is too heavy to repeat while watching for a change of chat, so the
 * answer is kept against the button and checked before it is reused.
 */
export function anchorFiber(button: Element): Fiber | null {
  if (anchoredTo === button && anchoredFiber && stillHolds(anchoredFiber, button)) {
    return anchoredFiber;
  }
  let node: Fiber | null = null;
  let host: Element | null = button;
  while (host && !node) {
    node = fiberOf(host);
    host = host.parentElement;
  }
  if (!node) node = containing(button);
  anchoredTo = button;
  anchoredFiber = node;
  return node;
}
