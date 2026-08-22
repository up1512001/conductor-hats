/**
 * Keeping Conductor 0.82 from painting nothing.
 *
 * 0.82 puts its whole UI behind one request, `GET /minimum-client-version`, and
 * renders null for as long as that query is neither settled nor failed. In a
 * re-signed copy that query never settles, so the window stays empty for ever
 * with no error anywhere.
 *
 * What was measured, in the copy, before writing this:
 *
 *   - React mounts; the tree stops at the component holding that query.
 *   - Its other two queries succeed. This one stays `pending`/`fetching`.
 *   - The request itself is fine: status 200, and its body streams to the
 *     frontend in full, two chunks and an end marker.
 *   - Issuing the identical request by hand in the same window also returns 200.
 *
 * So the request works and Conductor's wrapper around it never settles. What is
 * left is to make it fail, which Conductor already handles: a failed check logs
 * and the app renders. Only this one URL is answered this way, and only in a
 * patched copy, which is never the Conductor you installed.
 *
 * The cost is stated rather than hidden: the copy does not enforce Conductor's
 * minimum client version. Remove this the day a release settles that query.
 */

const GATE = "/minimum-client-version";
const FETCH = "plugin:http|fetch";

/** Tauri reports a failed command as a 400 whose body is the error. */
function refusal(): Response {
  return new Response(JSON.stringify("hats: minimum client version check skipped"), {
    status: 400,
    headers: { "content-type": "application/json" },
  });
}

export function installMinimumVersionGuard(): void {
  const real = window.fetch.bind(window);

  window.fetch = (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
    const url = String((input as Request)?.url ?? input);
    if (url.startsWith("ipc://") && typeof init?.body === "string") {
      const command = decodeURIComponent(url.slice(url.lastIndexOf("/") + 1));
      if (command === FETCH && init.body.includes(GATE)) {
        console.warn("[hats] failing Conductor's minimum-client-version check, which never settles in a patched copy");
        return Promise.resolve(refusal());
      }
    }
    return real(input, init);
  };
}
