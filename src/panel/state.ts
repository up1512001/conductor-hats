/* Everything the panel knows, read from conductor-acct and cached briefly. */

import { acct, q } from "./cli.js";

export interface Account {
  name: string;
  email: string;
  active: boolean;
  signedIn: boolean;
}

export interface Provider {
  agent: string;
  current: string;
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

/* Conductor's webview runs an in-memory router, so location never changes and
 * there is no id to read. The panel works out where it is by matching what is on
 * screen against the workspaces and repositories Conductor knows about, longest
 * name first so "rio-branch" is not beaten by a repo called "rio". Cached,
 * because it is two SQLite reads, but not for ever: a workspace created after the
 * app started would otherwise never be recognised. */
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

/* Scoped to the app chrome: the sidebar lists every workspace by name, so
 * searching the whole document would match the wrong one constantly. */
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

/* Every read costs a shell out to conductor-acct, and `json` runs the router
 * twice inside itself to answer. Conductor re-renders constantly, so an uncached
 * read per render pass meant several process spawns a second, and a press's own
 * read then queued behind that backlog: the panel appeared late enough to look
 * like the press had been ignored.
 *
 * So: one in-flight read shared by every caller, and a short cache after it.
 * Anything that writes calls invalidate(), so the cache can never be the reason a
 * change fails to show. */
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

/* Inside a workspace this routes that workspace; in the New Workspace dialog
 * there is no workspace yet, so it binds the repository and the workspace you are
 * about to create starts on the account you picked. */
export function applyAccount(
  state: PanelState,
  agent: string,
  profile: string
): Promise<string> {
  const t = state.target;
  if (t.kind === "workspace") return acct(`use ${profile} ${agent} ${q(t.path)}`);
  if (t.kind === "repository") return acct(`bind ${profile} ${agent} ${q(t.path)}`);
  return Promise.reject(new Error("no workspace or repository in view"));
}
