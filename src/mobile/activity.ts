/**
 * Tool work and thinking, drawn the way Conductor draws them.
 *
 * Conductor gives one row to a tool call and folds its output inside, so the
 * transcript reads as a list of things done rather than a call and an answer for
 * each. The row carries the tool's own mark, what it was pointed at, and for a
 * change to a file how much of it moved. A run of them collapses behind a count
 * and the marks of the tools it used.
 */

import { esc } from "./markup.js";
import { icon, toolIcon } from "./icons.js";
import type { TranscriptLine } from "./types.js";

const IMAGE = /\.(png|jpe?g|gif|webp|svg|bmp|heic|avif)$/i;

interface Call {
  call: TranscriptLine | null;
  result: TranscriptLine | null;
}

function firstLine(value: string): string {
  return value.split("\n").find((line) => line.trim())?.trim() || "";
}

/** A path is shown by its last segment, the way Conductor names a file chip. */
function shortened(value: string): string {
  const line = firstLine(value);
  if (/\s/.test(line) || !line.includes("/")) return line;
  return line.split("/").filter(Boolean).pop() || line;
}

function lines(value: string): number {
  return value ? value.split("\n").length : 0;
}

/**
 * What a change to a file added and removed.
 *
 * An edit carries the text it replaced and the text it wrote, so the count is
 * the change rather than an estimate of it. A write has no prior text and its
 * input arrives as the content itself, so every line of it is an addition.
 */
function counts(line: TranscriptLine): string {
  let added = 0;
  let removed = 0;
  try {
    const input = JSON.parse(line.detail) as Record<string, unknown>;
    const parts = Array.isArray(input.edits) ? input.edits : [input];
    for (const part of parts) {
      const item = (part || {}) as Record<string, unknown>;
      if (typeof item.old_string !== "string" && typeof item.new_string !== "string") continue;
      added += lines(String(item.new_string || ""));
      removed += lines(String(item.old_string || ""));
    }
  } catch {
    if (line.name !== "Write") return "";
    added = lines(line.detail);
  }
  if (!added && !removed) return "";
  return `<span class="activity-counts">${added ? `<span class="added">+${added}</span>` : ""}${
    removed ? `<span class="removed">−${removed}</span>` : ""
  }</span>`;
}

/**
 * Conductor names a read by what came back: an image, or the number of lines it
 * got, and "Reading" while it is still waiting. Every other tool keeps its own
 * name, which is what the Mac shows too.
 */
function label(pair: Call): string {
  const { call, result } = pair;
  if (!call) return result?.failed ? "Failed" : "Result";
  if (call.name !== "Read" && call.name !== "NotebookRead") return call.name;
  if (!result) return "Reading";
  if (IMAGE.test(firstLine(call.text))) return "Read image";
  const read = lines(result.text);
  return read ? `Read ${read} line${read === 1 ? "" : "s"}` : "Read";
}

function mark(pair: Call): string {
  if (pair.result?.failed) return "circle-alert";
  return pair.call ? toolIcon(pair.call.name) : "square-check-big";
}

function body(pair: Call): string {
  const parts = [pair.call?.detail || "", pair.result?.text || ""].filter(Boolean);
  return parts.join("\n\n");
}

function row(pair: Call): string {
  const failed = !!pair.result?.failed;
  const chip = shortened(pair.call?.text || pair.result?.text || "");
  const inside = body(pair);
  const trailing = pair.call ? counts(pair.call) : "";
  return `<details class="activity ${failed ? "failed" : ""}">
    <summary><span class="activity-icon">${icon(mark(pair))}</span>
      <span class="activity-copy"><span class="activity-name">${esc(label(pair))}</span>
      ${chip ? `<span class="activity-preview">${esc(chip)}</span>` : ""}</span>
      ${trailing}<span class="activity-chevron">${icon("chevron-right")}</span></summary>
    ${inside ? `<pre class="activity-body">${esc(inside)}</pre>` : ""}</details>`;
}

/** A thinking block, which Conductor also gives a row and a mark of its own. */
export function activityLine(line: TranscriptLine): string {
  const preview = firstLine(line.text || line.detail);
  return `<details class="activity thinking">
    <summary><span class="activity-icon">${icon("brain")}</span>
      <span class="activity-copy"><span class="activity-name">Thinking</span>
      ${preview ? `<span class="activity-preview">${esc(preview)}</span>` : ""}</span>
      <span class="activity-chevron">${icon("chevron-right")}</span></summary>
    ${line.text ? `<pre class="activity-body">${esc(line.text)}</pre>` : ""}</details>`;
}

/** Each call with the result that answered it, in the order they were made. */
function paired(items: TranscriptLine[]): Call[] {
  const results = new Map<string, TranscriptLine>();
  for (const item of items) {
    if (item.kind === "tool_result" && item.id) results.set(item.id, item);
  }
  const out: Call[] = [];
  const taken = new Set<string>();
  for (const item of items) {
    if (item.kind === "tool") {
      const result = results.get(item.id) || null;
      if (result) taken.add(item.id);
      out.push({ call: item, result });
    } else if (item.kind === "tool_result" && !taken.has(item.id)) {
      out.push({ call: null, result: item });
    }
  }
  return out;
}

export function activityCluster(items: TranscriptLine[]): string {
  const calls = paired(items);
  if (calls.length === 1) return row(calls[0] as Call);
  const failed = calls.filter((pair) => pair.result?.failed).length;
  const count = [
    `${calls.length} tool call${calls.length === 1 ? "" : "s"}`,
    failed ? `${failed} failed` : "",
  ].filter(Boolean).join(", ");
  const marks = Array.from(new Set(calls.map(mark))).slice(0, 6);
  return `<details class="activity-cluster ${failed ? "failed" : ""}">
    <summary><span class="activity-chevron">${icon("chevron-right")}</span>
      <span class="activity-name">${esc(count)}</span>
      <span class="activity-icons">${marks.map((item) => `<span>${icon(item)}</span>`).join("")}</span></summary>
    <div class="activity-list">${calls.map((pair) => row(pair)).join("")}</div></details>`;
}
