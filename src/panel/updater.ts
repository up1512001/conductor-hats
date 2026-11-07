/**
 * Keeping the copy from updating itself into stock Conductor.
 *
 * Conductor checks for an update every minute while focused, downloads it on
 * its own, and installs it on quit or when idle. In a copy that replaces the
 * whole bundle with the vendor's: the panel is gone, and the identifier is
 * `com.conductor.app` again, so the "copy" opens the real app's database and
 * keychain. A copy is refreshed with `hats repatch` instead.
 *
 * The check is answered with no update, which Conductor already handles, and
 * the download and install commands are refused should anything reach them.
 */

const CHECK = "plugin:updater|check";
const REFUSED = new Set([
  "plugin:updater|download",
  "plugin:updater|install",
  "plugin:updater|download_and_install",
]);

/** A successful Tauri command whose value is `null`. */
function noUpdate(): Response {
  return new Response("null", {
    status: 200,
    headers: { "content-type": "application/json", "Tauri-Response": "ok" },
  });
}

/** Tauri reports a failed command as a 400 whose body is the error. */
function refusal(): Response {
  return new Response(JSON.stringify("hats: updates are off in a patched copy; run hats repatch"), {
    status: 400,
    headers: { "content-type": "application/json" },
  });
}

export function installUpdaterGuard(): void {
  const real = window.fetch.bind(window);

  window.fetch = (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
    try {
      const url = String((input as Request)?.url ?? input);
      if (url.startsWith("ipc://")) {
        const command = decodeURIComponent(url.slice(url.lastIndexOf("/") + 1));
        if (command === CHECK) {
          return Promise.resolve(noUpdate());
        }
        if (REFUSED.has(command)) {
          console.warn("[hats] refusing a Conductor self-update in a patched copy");
          return Promise.resolve(refusal());
        }
      }
    } catch {
      return real(input, init);
    }
    return real(input, init);
  };
}
