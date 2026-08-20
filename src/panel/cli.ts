/**
 * Talking to hats through Conductor's own `execute_shell_command`,
 * which the webview can invoke without the invoke key.
 */

const CLI = "$HOME/.conductor-accounts/bin/hats";

interface TauriInternals {
  invoke?: (
    cmd: string,
    args: Record<string, unknown>
  ) => Promise<{ code?: number; stdout?: string; stderr?: string }>;
}

declare global {
  interface Window {
    __TAURI_INTERNALS__?: TauriInternals;
    __conductorHats?: { version: string };
    __conductorHatsDebug?: boolean;
  }
}

export function log(...parts: unknown[]): void {
  if (window.__conductorHatsDebug) {
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

/** Shell-quote a value. Workspace paths come from a database and can be anything. */
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
