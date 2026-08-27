/** The shapes the panel passes between its parts. */

export interface Account {
  name: string;
  email: string;
  active: boolean;
  signedIn: boolean;
}

export interface Provider {
  agent: string;
  /** The workspace's account, which is what a route sets. */
  current: string;
  /** The chat being typed in, empty when none is live or two are equally fresh. */
  session: string;
  /** What the next process for that chat will use. */
  chat: string;
  /** What the process already running for it took when it spawned, which is what
   * the conversation on screen is on. Empty before it starts. */
  started: string;
  pinned: boolean;
  accounts: Account[];
}

export interface Target {
  kind: "workspace" | "repository" | "none";
  name: string;
  path: string;
}

/** Which kind of place a read should resolve to when both are on screen. */
export type Prefer = "workspace" | "repository";

export interface Placed {
  target: Target;
  chat: string;
}

export interface PanelState {
  workspace: string;
  repo: string;
  enabled: boolean;
  providers: Provider[];
  target: Target;
  /** Which control was pressed, which is what a choice applies to. Never the
   * name matcher's guess: the toolbar means this chat even when the only name it
   * could find on screen was the repository's. */
  scope: Prefer;
  /** Conductor's id for the chat the toolbar is mounted in, read from the window
   * rather than deduced. Empty from the composer, which has no chat. */
  chatId: string;
}
