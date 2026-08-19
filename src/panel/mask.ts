/* Masking addresses for anything that renders on screen.
 *
 * A recorded session or a shared screenshot should not hand out an address. Each
 * part keeps a few characters with ** between, and the domain keeps its suffix so
 * the string still reads as an email:
 *
 *   someone.long@example.com  ->  som**ong@ex**e.com
 *   joe@mail.example.com      ->  j**@m**.example.com
 *
 * Enough to tell two accounts apart at a glance, and the profile name sits right
 * underneath for when it is not. The full address is never put in a title
 * attribute either, since a tooltip is just as visible on video. Use
 * `conductor-acct list` in a terminal when you need to read it.
 *
 * This rule is duplicated in `mask_email` in bin/conductor-acct, which the chat
 * card uses and which cannot be called once per row from here. A test runs both
 * over the same cases and fails if they disagree.
 */

/* How much is revealed scales with length, so a short local part is not handed
 * over in full for want of characters to hide. Nothing shorter than three
 * characters reveals anything at all. */
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
