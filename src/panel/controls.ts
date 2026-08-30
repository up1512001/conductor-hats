/**
 * Applies a phone run setting through Conductor's own visible controls.
 *
 * Nothing writes Conductor's database. Models go through the mounted picker's
 * own handler; other settings, and compatibility fallback, use the real control
 * beside the composer. Hats then checks whether the change actually landed. A
 * control that cannot be found, or a value that does not take, is released
 * rather than reported as applied.
 */

import { acct, log, q } from "./cli.js";
import { NODES, rootFiber, type Fiber } from "./fiber.js";

export interface Control {
  id: string;
  session: string;
  setting: "model" | "effort" | "permission" | "fast";
  value: string;
  before: string;
  lease: string;
}


export function controlCommand(action: "complete" | "release", item: Control): Promise<string> {
  return acct("remote control-" + action + " " + q(JSON.stringify(item))).catch(() => "");
}

async function applied(item: Control): Promise<boolean> {
  const raw = await acct("remote control-check " + q(JSON.stringify(item)));
  const result = JSON.parse(raw) as { applied?: boolean };
  return !!result.applied;
}

function visible(node: HTMLElement): boolean {
  return node.getClientRects().length > 0 && !node.hasAttribute("disabled");
}

function words(value: string): string {
  return value.toLowerCase().replace(/[^a-z0-9]+/g, " ").trim();
}

function record(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object" ? value as Record<string, unknown> : null;
}

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

function modelHandler(item: Control): ((...args: unknown[]) => unknown) | null {
  const start = rootFiber();
  if (!start) return null;
  const stack: Fiber[] = [start];
  let seen = 0;
  let current: ((...args: unknown[]) => unknown) | null = null;
  let ambiguous = false;
  while (stack.length && seen < NODES) {
    const node = stack.pop() as Fiber;
    seen += 1;
    const props = node.memoizedProps;
    const visible = record(props?.visibleBuiltInModelIds);
    const includes = visible && Object.values(visible).some((models) =>
      Array.isArray(models) && models.includes(item.value)
    );
    if (includes && typeof props?.onSelect === "function") {
      const handler = props.onSelect as (...args: unknown[]) => unknown;
      if (hasSession(node, item.session)) return handler;
      if (props.selectedModel === item.before) {
        ambiguous ||= current !== null && current !== handler;
        current = handler;
      }
    }
    if (node.child) stack.push(node.child);
    if (node.sibling) stack.push(node.sibling);
  }
  return ambiguous ? null : current;
}

async function selectModel(item: Control): Promise<boolean> {
  const handler = modelHandler(item);
  if (!handler) return false;
  try {
    await handler(item.value, { focusComposer: false });
    return true;
  } catch (error) {
    log("Conductor model handler failed", error);
    return false;
  }
}

function modelWords(value: string): string {
  const aliases: Record<string, string> = {
    opus: "opus 4 8",
    "opus-1m": "opus 4 8 1m",
    sonnet: "sonnet 4 6",
    haiku: "haiku 4 5",
    "opus-5-1m": "opus 5",
  };
  const clean = value.replace(/^claude-/, "").replace(/^gpt-/, "");
  return aliases[clean] || words(clean);
}

function expectedChoice(item: Control): string {
  if (item.setting === "model") return modelWords(item.value);
  const labels: Record<string, string> = {
    xhigh: "extra high",
    acceptEdits: "accept edits",
    bypassPermissions: "bypass permissions",
  };
  return labels[item.value] || words(item.value);
}

function exactChoice(label: string, item: Control): boolean {
  const normalized = words(label).replace(/ new$/, "");
  const have = item.setting === "model" ? normalized.replace(/^gpt /, "") : normalized;
  const want = expectedChoice(item);
  return have === want || new RegExp("^" + want.replace(/ /g, "\\s+") + " [1-9]$").test(have);
}

function controlScope(): HTMLElement | null {
  const composer = document.querySelector<HTMLElement>('[data-testid="composer-input"]');
  let scope = composer;
  for (let hops = 0; scope && hops < 7; hops += 1, scope = scope.parentElement) {
    const send = scope.querySelector('[data-testid="composer-send-button"]');
    if (send && scope.querySelectorAll("button,[role=button]").length > 2) return scope;
  }
  return null;
}

function opener(scope: HTMLElement, item: Control): HTMLElement | null {
  const names: Record<Control["setting"], RegExp> = {
    model: /model/i,
    effort: /effort|reasoning|thinking/i,
    permission: /permission|plan mode|auto mode/i,
    fast: /fast mode|fast/i,
  };
  const nodes = Array.from(scope.querySelectorAll<HTMLElement>("button,[role=button]"))
    .filter((node) => visible(node) && !node.matches('[data-testid="composer-send-button"]'));
  const currentModel = modelWords(item.before);
  const hint = (node: HTMLElement): string => [
    node.getAttribute("aria-label"),
    node.getAttribute("title"),
    node.getAttribute("data-tooltip"),
    node.textContent,
  ].filter(Boolean).join(" ");
  if (item.setting === "model" && currentModel) {
    const agent = nodes.find((node) =>
      (node.getAttribute("aria-label") || "").startsWith("Change agent (")
    );
    if (agent) return agent;
    const current = nodes.find((node) => words(hint(node)).includes(currentModel));
    if (current) return current;
  }
  return nodes.find((node) => names[item.setting].test(hint(node))) || null;
}

function waitChoice(item: Control, timeout: number): Promise<HTMLElement | null> {
  const started = Date.now();
  return new Promise((resolve) => {
    const check = (): void => {
      const direct = Array.from(document.querySelectorAll<HTMLElement>(
        '[role="option"],[role="menuitem"],[role="menuitemradio"]'
      ));
      const overlays = Array.from(document.querySelectorAll<HTMLElement>(
        '[role="listbox"],[role="menu"],[data-radix-popper-content-wrapper],\
         [data-slot="dropdown-menu-content"],[data-slot="popover-content"]'
      )).filter(visible);
      const nested = overlays.flatMap((root) =>
        Array.from(root.querySelectorAll<HTMLElement>("button,[role=option],[role=menuitem]"))
      );
      const nodes = direct.concat(nested);
      const choice = nodes.find((node) => visible(node) && exactChoice(node.textContent || "", item));
      if (choice) resolve(choice);
      else if (Date.now() - started >= timeout) resolve(null);
      else setTimeout(check, 40);
    };
    check();
  });
}

export async function applyControl(item: Control): Promise<boolean> {
  if (await applied(item)) {
    await controlCommand("complete", item);
    return true;
  }
  let invoked = item.setting === "model" && await selectModel(item);
  if (!invoked) {
    const scope = controlScope();
    const button = scope && opener(scope, item);
    if (!button) return false;
    button.click();
    invoked = true;
    if (item.setting !== "fast") {
      const choice = await waitChoice(item, 1800);
      if (!choice) {
        document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
        return false;
      }
      choice.click();
    }
  }
  for (let attempt = 0; invoked && attempt < 20; attempt += 1) {
    if (await applied(item)) {
      await controlCommand("complete", item);
      log("remote run setting applied", item.setting, item.session);
      return true;
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  return false;
}
