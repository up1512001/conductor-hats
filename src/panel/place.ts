/** Which workspace, repository or chat the window is showing. */

import { acct, q } from "./cli.js";
import { fromToolbar, workspaceId } from "./route.js";
import type { Scan } from "./route.js";
import type { Placed, Prefer, Target } from "./types.js";

/**
 * Conductor's webview routes in memory, so `location` carries no workspace id.
 * Instead the on-screen chrome is matched against what Conductor knows, longest
 * name first so `rio-branch` is not beaten by a repo called `rio`.
 */
const PLACES_TTL = 30_000;
let placesCache: Target[] | null = null;
let placesAt = 0;

function places(): Promise<Target[]> {
  if (placesCache && Date.now() - placesAt < PLACES_TTL) {
    return Promise.resolve(placesCache);
  }
  return Promise.all([
    acct("workspaces").catch(() => ""),
    acct("repos").catch(() => ""),
  ]).then(([workspaces, repos]) => {
    const parse = (text: string, kind: Target["kind"]): Target[] =>
      text
        .split("\n")
        .map((l) => l.split("\t"))
        .filter((p) => p.length === 2 && p[0] && p[1])
        .map((p) => ({ kind, name: p[0] as string, path: p[1] as string }));
    const list = parse(workspaces, "workspace").concat(parse(repos, "repository"));
    list.sort((a, b) => b.name.length - a.name.length);
    placesCache = list;
    placesAt = Date.now();
    return list;
  });
}

/** Scoped to the chrome: the sidebar names every workspace, so a document-wide
 * search would match the wrong one. */
function chromeText(): string {
  const bits: string[] = [document.title || ""];
  const sel =
    "header,nav,[class*=titlebar],[class*=toolbar],[class*=breadcrumb],[class*=Breadcrumb],[data-tauri-drag-region]";
  const nodes = document.querySelectorAll(sel);
  for (let i = 0; i < nodes.length && i < 24; i++) {
    bits.push(nodes[i]?.textContent || "");
  }
  for (const id of ["cma-toolbar-btn", "cma-chip"]) {
    let node: Element | null = document.getElementById(id);
    while (node && node !== document.body) {
      bits.push(node.textContent || "");
      if ((node.textContent || "").length > 400) break;
      node = node.parentElement;
    }
  }
  return bits.join(" \n ");
}

/**
 * A workspace and the repository holding it are both named on screen, so which
 * one wins is a choice, not a lookup. It follows the control that was pressed:
 * the toolbar button belongs to the workspace you are looking at, the composer
 * chip to the repository the next workspace will come from. Length only breaks
 * ties within a kind, so `rio-branch` still beats a repo called `rio`.
 */
/**
 * Records what the panel decided, when `hats debug on` says to. Silent
 * otherwise, and it never sees anything typed: only which place was resolved and
 * how. Diagnosing the panel used to mean injecting a probe, and every injection
 * re-signs the copy, which signs it out of Conductor.
 */
export function note(line: string): void {
  acct("log " + q(line)).catch(() => {});
}

export function currentTarget(prefer: Prefer): Promise<Placed> {
  const anchored = prefer === "workspace" ? fromToolbar() : null;
  const chat = (anchored && anchored.session) || "";
  let id = (anchored && anchored.workspace) || null;
  let how = id ? "toolbar" : "none";

  /* Counting ids across the whole tree is the last resort, and a poor one: every
   * sidebar row carries the id of the workspace it links to, so a busy window
   * offers dozens and the commonest is not the open one. */
  let scan: Scan = { id: null, fibers: 0, distinct: 0, how: "none" };
  if (!id && prefer === "workspace") {
    scan = workspaceId();
    id = scan.id;
    how = scan.how;
  }

  note(
    "scope=" + prefer + " how=" + how + " workspace=" + (id || "none") +
      " chat=" + (chat || "none") +
      " attached=" + (anchored ? anchored.attached : false) +
      " hops=" + (anchored ? anchored.hops : 0) +
      " fibers=" + scan.fibers + " distinctIds=" + scan.distinct
  );
  const exact: Promise<Target | null> = id
    ? acct("resolve " + q(id))
        .then((path) =>
          path ? ({ kind: "workspace", name: base(path), path } as Target) : null
        )
        .catch(() => null)
    : Promise.resolve(null);

  return exact.then((found) => {
    if (found) {
      note("target by id: " + found.kind + " " + found.name);
      return { target: found, chat };
    }
    return byName(prefer).then((t) => {
      note(
        "target by name: " + t.kind + " " + (t.name || "nothing matched") +
          " chromeChars=" + chromeText().length
      );
      return { target: t, chat };
    });
  });
}

function base(path: string): string {
  const parts = path.split("/");
  return parts[parts.length - 1] || path;
}

/** The fallback: match what Conductor knows against what is on screen. */
function byName(prefer: Prefer): Promise<Target> {
  return places().then((list) => {
    const hay = chromeText();
    const ordered = list.slice().sort((a, b) => {
      const rank = (t: Target) => (t.kind === prefer ? 0 : 1);
      return rank(a) - rank(b) || b.name.length - a.name.length;
    });
    for (const place of ordered) {
      if (hay.indexOf(place.name) >= 0) return place;
    }
    return { kind: "none", name: "", path: "" } as Target;
  });
}

