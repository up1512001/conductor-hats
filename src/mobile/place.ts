/**
 * Keeping a reader's place across a snapshot.
 *
 * Each update replaces the whole transcript, so anything the browser was
 * holding goes with it: reading something further up jumps to the newest line
 * the moment the Mac writes one, and an opened tool row shuts itself.
 */

export interface Held {
  atBottom: boolean;
  above: number;
  open: Set<string>;
}

export function hold(view: HTMLElement): Held {
  const open = new Set<string>();
  for (const node of view.querySelectorAll<HTMLDetailsElement>("details")) {
    if (node.open) open.add(node.querySelector("summary")?.textContent || "");
  }
  return {
    atBottom: view.scrollHeight - view.scrollTop - view.clientHeight < 140,
    above: view.scrollHeight - view.scrollTop,
    open,
  };
}

export function restore(view: HTMLElement, held: Held): void {
  for (const node of view.querySelectorAll<HTMLDetailsElement>("details")) {
    if (held.open.has(node.querySelector("summary")?.textContent || "")) node.open = true;
  }
  requestAnimationFrame(() => {
    view.scrollTop = held.atBottom ? view.scrollHeight : view.scrollHeight - held.above;
  });
}
