/** Conductor's own visible model-picker state, shared with the phone. */

import { acct, log, q } from "./cli.js";
import { NODES, rootFiber, type Fiber } from "./fiber.js";
import { fromToolbar } from "./route.js";

const AGENTS = ["claude", "codex"] as const;
const UUID = /^[0-9a-f-]{8,36}$/i;
type Models = Record<string, string[]>;
type Titles = Record<string, string>;

interface Catalog {
  models: Models;
  titles: Titles;
}

function record(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object" ? value as Record<string, unknown> : null;
}

function strings(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return Array.from(new Set(value.filter((item): item is string =>
    typeof item === "string" && item.length > 0
  )));
}

function fromProps(props: Record<string, unknown> | null | undefined): Models | null {
  const visible = record(props?.visibleBuiltInModelIds);
  if (!visible) return null;
  const models: Models = {};
  for (const agent of AGENTS) {
    const values = strings(visible[agent]);
    if (values.length) models[agent] = values;
  }
  return Object.keys(models).length ? models : null;
}

function rememberTitle(titles: Titles, id: unknown, title: unknown, force = false): void {
  if (typeof id !== "string" || !UUID.test(id) || typeof title !== "string") return;
  const value = title.trim();
  if (!value || value.length > 240) return;
  const placeholder = /^(new chat|untitled)$/i.test(value);
  const existing = titles[id] || "";
  if (force || !existing || /^(new chat|untitled)$/i.test(existing) || !placeholder && value.length > existing.length) {
    titles[id] = value;
  }
}

function collectTitles(
  value: unknown,
  titles: Titles,
  visited: WeakSet<object>,
  depth = 0
): void {
  if (!value || typeof value !== "object" || depth > 3 || visited.has(value)) return;
  visited.add(value);
  if (Array.isArray(value)) {
    for (const item of value) collectTitles(item, titles, visited, depth + 1);
    return;
  }
  const item = value as Record<string, unknown>;
  rememberTitle(titles, item.sessionId || item.session_id || item.id, item.title);
  for (const [key, child] of Object.entries(item)) {
    if (!["children", "_owner", "return", "child", "sibling"].includes(key)) {
      collectTitles(child, titles, visited, depth + 1);
    }
  }
}

function visibleTitles(titles: Titles): void {
  for (const route of document.querySelectorAll<HTMLAnchorElement>('a[href*="sessionId="]')) {
    let session = "";
    try {
      session = new URL(route.href, location.href).searchParams.get("sessionId") || "";
    } catch {
      continue;
    }
    const candidates = [route, ...route.querySelectorAll<HTMLElement>("*")]
      .filter((node) => node.childElementCount === 0 && node.getClientRects().length > 0)
      .map((node) => (node.textContent || "").trim())
      .filter((text) => text.length > 1 && text.length <= 240)
      .filter((text) => !/^(\d+|thinking|working|ready|idle|completed|needs attention)$/i.test(text))
      .sort((a, b) => b.length - a.length);
    if (candidates[0]) rememberTitle(titles, session, candidates[0], true);
  }
}

/** Reads the live API state already supplied to Conductor's mounted components. */
export function conductorCatalog(): Catalog | null {
  const start = rootFiber();
  if (!start) return null;
  const stack: Fiber[] = [start];
  let seen = 0;
  const models: Models = {};
  const titles: Titles = {};
  const visited = new WeakSet<object>();
  while (stack.length && seen < NODES) {
    const node = stack.pop() as Fiber;
    seen += 1;
    const found = fromProps(node.memoizedProps);
    if (found) {
      for (const agent of AGENTS) {
        if ((found[agent]?.length || 0) > (models[agent]?.length || 0)) {
          models[agent] = found[agent] || [];
        }
      }
    }
    collectTitles(node.memoizedProps, titles, visited);
    if (node.child) stack.push(node.child);
    if (node.sibling) stack.push(node.sibling);
  }
  visibleTitles(titles);
  return AGENTS.every((agent) => models[agent]?.length) ? { models, titles } : null;
}

let published = "";
let publishing = false;
let publishAgain = false;

export function publishModelCatalog(force = false): void {
  try {
    if (publishing) {
      publishAgain ||= force;
      return;
    }
    const workspace = fromToolbar().workspace;
    if (!workspace) return;
    const catalog = conductorCatalog();
    if (!catalog) return;
    const body = JSON.stringify(catalog);
    const key = workspace + ":" + body;
    if (!force && key === published) return;
    publishing = true;
    acct("remote catalog " + q(workspace) + " " + q(body))
      .then(() => {
        published = key;
      })
      .catch((error) => log("model catalog sync failed", error))
      .then(() => {
        publishing = false;
        if (publishAgain) {
          publishAgain = false;
          publishModelCatalog(true);
        }
      });
  } catch (error) {
    publishing = false;
    log("model catalog read failed", error);
  }
}

export function startModelCatalogSync(): void {
  publishModelCatalog();
  setInterval(publishModelCatalog, 8000);
}
