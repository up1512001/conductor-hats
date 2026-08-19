/* Talking to conductor-acct.
 *
 * `execute_shell_command` is one of the Tauri commands Conductor's webview can
 * invoke, and it is reachable through window.__TAURI_INTERNALS__ without the
 * invoke key. So the panel shells out to the CLI rather than keeping any state
 * of its own, and the CLI, the /account command and this panel cannot disagree.
 */

const CLI = "$HOME/.conductor-accounts/bin/conductor-acct";

interface TauriInternals {
  invoke?: (
    cmd: string,
    args: Record<string, unknown>
  ) => Promise<{ code?: number; stdout?: string; stderr?: string }>;
}

declare global {
  interface Window {
    __TAURI_INTERNALS__?: TauriInternals;
    __conductorMultiAccount?: { version: string };
    __conductorMultiAccountDebug?: boolean;
  }
}

export function log(...parts: unknown[]): void {
  if (window.__conductorMultiAccountDebug) {
    console.log("[multi-account]", ...parts);
  }
}

export function sh(command: string): Promise<string> {
  const internals = window.__TAURI_INTERNALS__;
  if (!internals || !internals.invoke) {
    return Promise.reject(new Error("no Tauri bridge"));
  }
  return internals
    .invoke("execute_shell_command", { shell: "/bin/zsh", command, noRcs: true })
    .then((r) => {
      if (r && r.code !== 0) {
        throw new Error((r.stderr || "").trim() || "exit " + r.code);
      }
      return ((r && r.stdout) || "").trim();
    });
}

export function acct(args: string): Promise<string> {
  return sh(CLI + " " + args);
}

/* Single-quoted for the shell, with embedded quotes escaped the POSIX way.
 * Workspace paths come from Conductor's database and can contain anything. */
export function q(s: string): string {
  return "'" + String(s).replace(/'/g, "'\\''") + "'";
}

export function cliPath(): string {
  return CLI;
}

export function message(e: unknown): string {
  if (e instanceof Error) return e.message;
  return String(e);
}
