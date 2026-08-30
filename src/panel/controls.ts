/**
 * Applies a phone run setting through Conductor's own visible controls.
 *
 * Nothing writes Conductor's database. The panel finds the real control beside
 * the composer, opens it, picks the value by its label, then asks hats whether
 * the change actually landed. A control that cannot be found, or a value that
 * does not take, is released rather than reported as applied.
 */

import { acct, log, q } from "./cli.js";

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

function matchesChoice(label: string, value: string): boolean {
  const have = words(label);
  const want = words(value);
  return have === want || (have.length > 3 && want.includes(have)) || (want.length > 3 && have.includes(want));
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
  return nodes.find((node) => {
    const hint = [
      node.getAttribute("aria-label"),
      node.getAttribute("title"),
      node.getAttribute("data-tooltip"),
      node.textContent,
    ].filter(Boolean).join(" ");
    return names[item.setting].test(hint) || (item.setting === "model" && matchesChoice(hint, item.before));
  }) || null;
}

function waitChoice(value: string, timeout: number): Promise<HTMLElement | null> {
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
      const choice = nodes.find((node) => visible(node) && matchesChoice(node.textContent || "", value));
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
  const scope = controlScope();
  const button = scope && opener(scope, item);
  if (!button) return false;
  button.click();
  if (item.setting !== "fast") {
    const choice = await waitChoice(item.value, 1800);
    if (!choice) {
      document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
      return false;
    }
    choice.click();
  }
  for (let attempt = 0; attempt < 24; attempt += 1) {
    if (await applied(item)) {
      await controlCommand("complete", item);
      log("remote run setting applied", item.setting, item.session);
      return true;
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  return false;
}
