/**
 * Letting the copy sign in to Conductor exactly as the real app does.
 *
 * The browser returns the sign-in code through a `conductor://auth` link, and
 * macOS gives that link to the real Conductor, which has no matching request
 * and drops it. While this copy is signing in, `hats signin --begin` makes it
 * the app that opens `conductor://`; once the code is exchanged, `--end` gives
 * the scheme back. hats restores it on its own after ten minutes regardless.
 *
 * Nothing is blocked: if hats cannot be reached the sign-in proceeds as before.
 */

import { acct } from "./cli.js";

const OPEN_URL = "plugin:opener|open_url";
const HTTP_FETCH = "plugin:http|fetch";
const SIGN_IN = "desktop-auth";
const EXCHANGE = "/api/desktop-auth/token";

function command(url: string): string {
  return decodeURIComponent(url.slice(url.lastIndexOf("/") + 1));
}

function opensSignIn(body: string): boolean {
  try {
    const args = JSON.parse(body) as { url?: unknown };
    return typeof args?.url === "string" && decodeURIComponent(args.url).includes(SIGN_IN);
  } catch {
    return false;
  }
}

function quietly(args: string): Promise<void> {
  return acct(args).then(
    () => undefined,
    (e: unknown) => console.warn("[hats] signin " + args + " failed:", e)
  );
}

export function installSignInHandover(): void {
  const real = window.fetch.bind(window);

  window.fetch = (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
    try {
      const url = String((input as Request)?.url ?? input);
      const body = init?.body;
      if (url.startsWith("ipc://") && typeof body === "string") {
        const name = command(url);
        if (name === OPEN_URL && opensSignIn(body)) {
          return quietly("signin --begin").then(() => real(input, init));
        }
        if (name === HTTP_FETCH && body.includes(EXCHANGE)) {
          const sent = real(input, init);
          sent.finally(() => quietly("signin --end")).catch(() => undefined);
          return sent;
        }
      }
    } catch {
      return real(input, init);
    }
    return real(input, init);
  };
}
