/** Everything the panel knows, read from hats and cached briefly. */

import { acct, q } from "./cli.js";
import { workspaceId } from "./route.js";

export interface Account {
  name: string;
  email: string;
  active: boolean;
  signedIn: boolean;
}

export interface Provider {
  agent: string;
  /** The workspace's account, which is what the toolbar control sets. */
  current: string;
  /** The chat being typed in, empty when none is live or two are equally fresh. */
  session: string;
  /** What that chat actually resolves to, which differs when it carries a pin. */
  chat: string;
  pinned: boolean;
  accounts: Account[];
}

export interface Target {
  kind: "workspace" | "repository" | "none";
  name: string;
  path: string;
}

export interface PanelState {
  workspace: string;
  repo: string;
  enabled: boolean;
  providers: Provider[];
  target: Target;
}

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
  const sel = "header,[class*=titlebar],[class*=toolbar],[data-tauri-drag-region]";
  const nodes = document.querySelectorAll(sel);
  for (let i = 0; i < nodes.length && i < 12; i++) {
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
function currentTarget(prefer: Prefer): Promise<Target> {
  const id = prefer === "workspace" ? workspaceId() : null;
  const exact: Promise<Target | null> = id
    ? acct("resolve " + q(id))
        .then((path) =>
          path ? ({ kind: "workspace", name: base(path), path } as Target) : null
        )
        .catch(() => null)
    : Promise.resolve(null);

  return exact.then((found) => (found ? found : byName(prefer)));
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

/**
 * One in-flight read shared by every caller, then a short cache. Uncached reads
 * per render pass cost several process spawns a second, and a press's own read
 * queued behind them. Writes call invalidate().
 */
const STATE_TTL = 4000;
let stateCache: PanelState | null = null;
let stateAt = 0;
let statePending: Promise<PanelState> | null = null;
let statePrefer: Prefer | null = null;

/** Which kind of place a read should resolve to when both are on screen. */
export type Prefer = "workspace" | "repository";

export function invalidate(): void {
  stateCache = null;
  stateAt = 0;
  statePrefer = null;
}

export function loadState(fresh?: boolean, prefer: Prefer = "workspace"): Promise<PanelState> {
  const same = statePrefer === prefer;
  if (!fresh && same && stateCache && Date.now() - stateAt < STATE_TTL) {
    return Promise.resolve(stateCache);
  }
  if (statePending && same) return statePending;
  statePrefer = prefer;
  statePending = currentTarget(prefer)
    .then((target) =>
      acct("json " + (target.path ? q(target.path) : "")).then((out) => {
        const st = JSON.parse(out) as PanelState;
        st.target = target;
        return st;
      })
    )
    .then(
      (st) => {
        stateCache = st;
        stateAt = Date.now();
        statePending = null;
        return st;
      },
      (e) => {
        statePending = null;
        throw e;
      }
    );
  return statePending;
}

export type Scope = "workspace" | "chat";

/**
 * Routes a workspace, binds the repository when there is no workspace yet, or
 * pins one chat.
 *
 * A pin cannot move the conversation on screen: its agent process took a config
 * directory when it spawned and never reads one again. It decides the next
 * process Conductor starts for that chat.
 */
export function applyAccount(
  state: PanelState,
  agent: string,
  profile: string,
  scope: Scope = "workspace"
): Promise<string> {
  const t = state.target;
  if (scope === "chat") {
    const provider = state.providers.filter((p) => p.agent === agent)[0];
    const session = provider ? provider.session : "";
    if (!session) return Promise.reject(new Error("no chat is active here"));
    return acct(`pin ${profile} ${agent} ${session}`);
  }
  if (t.kind === "workspace") return acct(`use ${profile} ${agent} ${q(t.path)}`);
  if (t.kind === "repository") return acct(`bind ${profile} ${agent} ${q(t.path)}`);
  return Promise.reject(new Error("no workspace or repository in view"));
}
