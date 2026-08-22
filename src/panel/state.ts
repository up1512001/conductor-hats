/** Everything the panel knows, read from hats and cached briefly. */

import { acct, q } from "./cli.js";

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

function currentTarget(): Promise<Target> {
  return places().then((list) => {
    const hay = chromeText();
    for (const place of list) {
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

export function invalidate(): void {
  stateCache = null;
  stateAt = 0;
}

export function loadState(fresh?: boolean): Promise<PanelState> {
  if (!fresh && stateCache && Date.now() - stateAt < STATE_TTL) {
    return Promise.resolve(stateCache);
  }
  if (statePending) return statePending;
  statePending = currentTarget()
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
