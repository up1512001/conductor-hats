/**
 * The open workspace and chat, read from Conductor's router.
 *
 * Conductor 0.93 routes with TanStack Router and leaves the instance on
 * `self.__TSR_ROUTER__`. Its matched routes carry the ids of what is on screen,
 * exactly: `/repository/$repositoryId/workspace/$localWorkspaceId`. The same
 * release renamed the parameter from `workspaceId`, which is what left the panel
 * reading every component and finding no workspace at all.
 *
 * An internal, so anything unexpected answers "unknown" and the callers fall back
 * to the component tree and then to the database.
 */

const UUID = /^[0-9a-f-]{8,36}$/i;

interface Match {
  params?: Record<string, unknown>;
  search?: Record<string, unknown>;
}

interface Routed {
  workspace: string | null;
  session: string | null;
}

function pick(bag: Record<string, unknown> | undefined, keys: string[]): string | null {
  if (!bag) return null;
  for (const key of keys) {
    const value = bag[key];
    if (typeof value === "string" && UUID.test(value)) return value;
  }
  return null;
}

export const WORKSPACE_KEYS = ["localWorkspaceId", "workspaceId"];
const SESSION_KEYS = ["sessionId", "localSessionId", "session"];

export function fromRouter(): Routed {
  const out: Routed = { workspace: null, session: null };
  try {
    const router = (self as unknown as { __TSR_ROUTER__?: { state?: { matches?: Match[] } } })
      .__TSR_ROUTER__;
    const matches = router?.state?.matches;
    if (!Array.isArray(matches)) return out;
    for (const match of matches) {
      out.workspace = out.workspace || pick(match.params, WORKSPACE_KEYS);
      out.session =
        out.session || pick(match.params, SESSION_KEYS) || pick(match.search, SESSION_KEYS);
    }
  } catch {
    return out;
  }
  return out;
}
