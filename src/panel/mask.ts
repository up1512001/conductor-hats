/**
 * Masking addresses so a recorded session cannot hand one out.
 *
 *   someone.long@example.com  ->  som**ong@ex**e.com
 *   joe@mail.example.com      ->  j**@m**.example.com
 *
 * Duplicated as `mask_email` in lib/mask.sh, which the chat card uses. A test
 * runs both over the same cases and fails if they disagree.
 */

/** Reveals less of a short part, so nothing under three characters leaks. */
function maskPart(s: string): string {
  const n = s.length;
  if (n <= 2) return "**";
  if (n <= 5) return s.charAt(0) + "**";
  if (n <= 8) return s.slice(0, 2) + "**" + s.slice(-1);
  return s.slice(0, 3) + "**" + s.slice(-3);
}

export function maskEmail(raw: string | null | undefined): string {
  const s = String(raw || "");
  if (!s) return "";
  const at = s.lastIndexOf("@");
  if (at < 1) return maskPart(s);
  const local = s.slice(0, at);
  const domain = s.slice(at + 1);
  const dot = domain.indexOf(".");
  const host = dot > 0 ? domain.slice(0, dot) : domain;
  const suffix = dot > 0 ? domain.slice(dot) : "";
  return maskPart(local) + "@" + maskPart(host) + suffix;
}
