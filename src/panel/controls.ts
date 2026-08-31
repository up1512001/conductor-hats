/**
 * Applies a phone run setting through Conductor's own visible controls.
 *
 * Nothing writes Conductor's database. Models and effort go through the mounted
 * picker's own handlers; other settings, and compatibility fallback, use the
 * real control beside the composer. Hats then checks whether the change actually
 * landed. A control that cannot be found, or a value that does not take, is
 * released rather than reported as applied.
 */

import { acct, log, q } from "./cli.js";
import { applyEffort, modelHandler } from "./picker.js";

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

async function selectModel(item: Control): Promise<boolean> {
  const handler = modelHandler(item.session, item.value, item.before);
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

/**
 * Every label Conductor is willing to print for one value.
 *
 * Effort carries per-model overrides on the Mac, so the same level reads as
 * "Low" on one model and "Light" on another, and one of them has to match.
 */
function expectedChoices(item: Control): string[] {
  if (item.setting === "model") return [modelWords(item.value)];
  const labels: Record<string, string[]> = {
    none: ["off"],
    low: ["low", "light"],
    xhigh: ["extra high"],
    acceptEdits: ["accept edits"],
    bypassPermissions: ["bypass permissions"],
  };
  return labels[item.value] || [words(item.value)];
}

function exactChoice(label: string, item: Control): boolean {
  const normalized = words(label).replace(/ new$/, "");
  const have = item.setting === "model" ? normalized.replace(/^gpt /, "") : normalized;
  return expectedChoices(item).some((want) =>
    have === want || new RegExp("^" + want.replace(/ /g, "\\s+") + " [1-9]$").test(have)
  );
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

function buttons(scope: HTMLElement): HTMLElement[] {
  return Array.from(scope.querySelectorAll<HTMLElement>("button,[role=button]"))
    .filter((node) => visible(node) && !node.matches('[data-testid="composer-send-button"]'));
}

function hint(node: HTMLElement): string {
  return [
    node.getAttribute("aria-label"),
    node.getAttribute("title"),
    node.getAttribute("data-tooltip"),
    node.textContent,
  ].filter(Boolean).join(" ");
}

/**
 * The one control that opens Conductor's picker, found only by its own label.
 *
 * Nothing looser will do for effort. The button beside it is a bar meter whose
 * visible text is the level, so a search for "high" finds that meter, and a
 * press on it advances the level instead of opening anything.
 */
function pickerButton(scope: HTMLElement): HTMLElement | null {
  return buttons(scope).find((node) =>
    (node.getAttribute("aria-label") || "").startsWith("Change agent (")
  ) || null;
}

/**
 * Effort is deliberately absent. Conductor's composer control for it is a bar
 * meter wired to `chat.toggleThinking`: a click advances to the next level, so
 * pressing it applies whatever comes next rather than what was asked for.
 */
function opener(scope: HTMLElement, item: Control): HTMLElement | null {
  const names: Record<string, RegExp> = {
    permission: /permission|plan mode|auto mode/i,
    fast: /fast mode|fast/i,
  };
  const nodes = buttons(scope);
  if (item.setting === "model") {
    const current = modelWords(item.before);
    const named = current && nodes.find((node) => words(hint(node)).includes(current));
    return pickerButton(scope) || named || nodes.find((node) => /model/i.test(hint(node))) || null;
  }
  const want = names[item.setting];
  return want ? nodes.find((node) => want.test(hint(node))) || null : null;
}

/** Everything clickable inside whatever Conductor has open over the page. */
function overlayChoices(): HTMLElement[] {
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
  return direct.concat(nested).filter(visible);
}

function waitFor(match: (node: HTMLElement) => boolean, timeout: number): Promise<HTMLElement | null> {
  const started = Date.now();
  return new Promise((resolve) => {
    const check = (): void => {
      const choice = overlayChoices().find(match);
      if (choice) resolve(choice);
      else if (Date.now() - started >= timeout) resolve(null);
      else setTimeout(check, 40);
    };
    check();
  });
}

/** Conductor opens the Effort list on hover, so a click alone never reaches it. */
function hover(node: HTMLElement): void {
  const Kind = typeof PointerEvent === "function" ? PointerEvent : MouseEvent;
  for (const name of ["pointerover", "pointerenter", "mouseover", "mouseenter", "pointermove"]) {
    node.dispatchEvent(new Kind(name, { bubbles: !name.endsWith("enter"), cancelable: true }));
  }
}

function dismiss(): false {
  for (let press = 0; press < 2; press += 1) {
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
  }
  return false;
}

/** The picker's own Effort submenu, for a build whose handler cannot be read. */
async function effortThroughMenu(item: Control): Promise<boolean> {
  const scope = controlScope();
  const button = scope && pickerButton(scope);
  if (!button) return false;
  button.click();
  const submenu = await waitFor((node) => /^effort( |$)/.test(words(node.textContent || "")), 1800);
  if (!submenu) return dismiss();
  hover(submenu);
  submenu.click();
  const choice = await waitFor((node) => exactChoice(node.textContent || "", item), 1800);
  if (!choice) return dismiss();
  choice.click();
  return true;
}

async function throughControl(item: Control): Promise<boolean> {
  const scope = controlScope();
  const button = scope && opener(scope, item);
  if (!button) return false;
  button.click();
  if (item.setting === "fast") return true;
  const choice = await waitFor((node) => exactChoice(node.textContent || "", item), 1800);
  if (!choice) return dismiss();
  choice.click();
  return true;
}

async function invoke(item: Control): Promise<boolean> {
  if (item.setting === "model") {
    return (await selectModel(item)) || (await throughControl(item));
  }
  if (item.setting === "effort") {
    return (await applyEffort(item.session, item.value)) || (await effortThroughMenu(item));
  }
  return throughControl(item);
}

export async function applyControl(item: Control): Promise<boolean> {
  if (await applied(item)) {
    await controlCommand("complete", item);
    return true;
  }
  if (!(await invoke(item))) return false;
  for (let attempt = 0; attempt < 20; attempt += 1) {
    if (await applied(item)) {
      await controlCommand("complete", item);
      log("remote run setting applied", item.setting, item.session);
      return true;
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  return false;
}
