/** Everything the panel knows, read from hats and cached briefly. */

import { acct, q } from "./cli.js";
import { currentTarget, note } from "./place.js";
import type { PanelState, Prefer } from "./types.js";

export type { Account, PanelState, Prefer, Provider, Target } from "./types.js";





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
    .then((placed) =>
      acct(
        "json " +
          (placed.target.path ? q(placed.target.path) : "''") +
          (placed.chat ? " " + q(placed.chat) : "")
      ).then((out) => {
        const st = JSON.parse(out) as PanelState;
        st.target = placed.target;
        st.scope = prefer;
        st.chatId = placed.chat;
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

/**
 * The chat a choice applies to: what the window says first, what the CLI worked
 * out second. The window is exact where the CLI is inferring.
 */
function sessionOf(state: PanelState, agent: string): string {
  if (state.chatId) return state.chatId;
  const provider = state.providers.filter((p) => p.agent === agent)[0];
  return provider ? provider.session : "";
}

/**
 * The toolbar sets the chat that is open, and nothing else.
 *
 * A workspace holds several chats and each runs its own agent process, so one
 * can be Personal while the next is Work. Writing the workspace route here
 * instead would move every chat that is not pinned, which is the opposite of
 * what pressing a control inside one chat means.
 *
 * With no chat open there is nothing to pin, so the choice falls back to the
 * workspace and the wording says so.
 *
 * Neither can move the conversation already on screen: its agent process took a
 * config directory when it spawned and never reads one again. It decides the
 * next process Conductor starts for that chat.
 */
export function applyAccount(
  state: PanelState,
  agent: string,
  profile: string
): Promise<string> {
  const t = state.target;

  /* The composer chip is the only control that means the repository, and it says
   * so by asking for that scope. Everything else is the toolbar, which belongs to
   * the chat it was pressed in. Deciding this from `target.kind` instead let a
   * toolbar press bind the whole repository whenever the name matcher found the
   * repository's name on screen and the workspace's nowhere: one value shared by
   * every workspace in it, so the last account chosen won everywhere. */
  note(
    "choose " + profile + " agent=" + agent + " scope=" + state.scope +
      " target=" + t.kind + " session=" + (sessionOf(state, agent) || "none")
  );

  /* Pressed while a workspace is being created, before there is anything to
   * attach the choice to. A repository binding was the old answer and it is one
   * value for every workspace in the repository, so creating one on Work and the
   * next on Personal put both on Personal. This is spent by the next workspace
   * that starts without an account of its own, and by nothing else. */
  if (state.scope === "repository") {
    return acct(`next ${profile} ${agent}`);
  }

  const session = sessionOf(state, agent);
  if (session) return acct(`pin ${profile} ${agent} ${session}`);
  if (t.kind === "workspace") return acct(`use ${profile} ${agent} ${q(t.path)}`);

  /* No chat and no workspace, but a repository: this is the New Workspace view,
   * where the only workspace there is to mean is the one about to be made. It
   * still never binds the repository, which would move every workspace in it. */
  if (t.kind === "repository") return acct(`next ${profile} ${agent}`);

  return Promise.reject(new Error("open a workspace or a chat to choose an account"));
}

/**
 * Every chat in the workspace, which is the other thing someone might mean.
 *
 * The route alone would leave the open chat behind, because a pin beats a route
 * and the open chat is pinned the moment its agent starts. So the pin goes too,
 * and "every chat" means every chat.
 */
export function applyToWorkspace(
  state: PanelState,
  agent: string,
  profile: string
): Promise<string> {
  const t = state.target;
  if (t.kind !== "workspace") {
    return Promise.reject(new Error("no workspace in view"));
  }
  const session = sessionOf(state, agent);
  return acct(`use ${profile} ${agent} ${q(t.path)}`).then((out) =>
    session
      ? acct(`unpin ${agent} ${session}`)
          .then(() => out)
          .catch(() => out)
      : out
  );
}
