/** Conductor's own visible model-picker state, shared with the phone. */

import { acct, log, q } from "./cli.js";
import { NODES, rootFiber, type Fiber } from "./fiber.js";
import { fromToolbar } from "./route.js";

const AGENTS = ["claude", "codex"] as const;
type Models = Record<string, string[]>;

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

/** Reads the provider API result already supplied to Conductor's model picker. */
export function conductorModels(): Models | null {
  const start = rootFiber();
  if (!start) return null;
  const stack: Fiber[] = [start];
  let seen = 0;
  let best: Models | null = null;
  while (stack.length && seen < NODES) {
    const node = stack.pop() as Fiber;
    seen += 1;
    const found = fromProps(node.memoizedProps);
    if (found) {
      best = found;
      if (AGENTS.every((agent) => found[agent]?.length)) return found;
    }
    if (node.child) stack.push(node.child);
    if (node.sibling) stack.push(node.sibling);
  }
  return best;
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
    const models = conductorModels();
    if (!models) return;
    const body = JSON.stringify({ models });
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
