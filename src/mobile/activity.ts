/** Conductor-like thinking and tool activity for a compact transcript. */

import { esc } from "./markup.js";
import type { TranscriptLine } from "./types.js";

const PATHS: Record<string, string> = {
  brain: '<path d="M9.5 4.5A3 3 0 0 0 4 6a3 3 0 0 0 .6 1.8A3.5 3.5 0 0 0 5 14a3 3 0 0 0 4.5 3.1V4.5ZM14.5 4.5A3 3 0 0 1 20 6a3 3 0 0 1-.6 1.8A3.5 3.5 0 0 1 19 14a3 3 0 0 1-4.5 3.1V4.5Z"/><path d="M9.5 8H7.8M14.5 8h1.7M9.5 13H7.8M14.5 13h1.7"/>',
  terminal: '<path d="M4 5h16v14H4zM7 9l3 3-3 3M12 15h5"/>',
  file: '<path d="M6 3h8l4 4v14H6zM14 3v5h5M9 12h6M9 16h6"/>',
  edit: '<path d="M5 4h10l4 4v5M14 4v5h5M13 19H5V4M14 18l5-5 2 2-5 5-3 1z"/>',
  search: '<circle cx="10.5" cy="10.5" r="6.5"/><path d="m15.5 15.5 5 5M8 10.5h5"/>',
  globe: '<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18"/>',
  agent: '<rect x="5" y="7" width="14" height="12" rx="3"/><path d="M12 3v4M9 12h.01M15 12h.01M9 16h6"/>',
  wrench: '<path d="M14 6a4 4 0 0 0-5 5L3.5 16.5a2.1 2.1 0 0 0 3 3L12 14a4 4 0 0 0 5-5l-2 2-2-2z"/>',
  result: '<path d="m5 12 4 4L19 6"/>',
  error: '<circle cx="12" cy="12" r="9"/><path d="M12 7v6M12 17h.01"/>',
  chevron: '<path d="m9 6 6 6-6 6"/>',
};

function icon(name: string): string {
  return `<svg viewBox="0 0 24 24" aria-hidden="true">${PATHS[name] || PATHS.wrench}</svg>`;
}

function iconName(name: string): string {
  const value = name.toLowerCase().replace(/[^a-z]/g, "");
  if (/bash|shell|terminal|command/.test(value)) return "terminal";
  if (/grep|glob|search|find/.test(value)) return "search";
  if (/web|fetch|url|browser/.test(value)) return "globe";
  if (/edit|write|patch|notebook/.test(value)) return "edit";
  if (/read|file/.test(value)) return "file";
  if (/task|agent|todo/.test(value)) return "agent";
  return "wrench";
}

function firstLine(value: string): string {
  return value.split("\n").find((line) => line.trim())?.trim() || "";
}

export function activityLine(line: TranscriptLine, pairedTool = ""): string {
  const thinking = line.kind === "thinking";
  const result = line.kind === "tool_result";
  const body = line.detail || line.text;
  const label = thinking ? "Thinking" : result ? (line.failed ? "Failed" : "Result") : line.name;
  const preview = firstLine(line.text || line.detail);
  const symbol = line.failed ? "error" : thinking ? "brain" : result ? "result" : iconName(pairedTool || line.name);
  return `<details class="activity ${thinking ? "thinking" : ""} ${line.failed ? "failed" : ""}">
    <summary><span class="activity-icon">${icon(symbol)}</span>
      <span class="activity-copy"><span class="activity-name">${esc(label)}</span>
      ${preview ? `<span class="activity-preview">${esc(preview)}</span>` : ""}</span>
      <span class="activity-chevron">${icon("chevron")}</span></summary>
    ${body ? `<pre class="activity-body">${esc(body)}</pre>` : ""}</details>`;
}

export function activityCluster(lines: TranscriptLine[]): string {
  const names = new Map<string, string>();
  for (const line of lines) {
    if (line.kind === "tool" && line.id) names.set(line.id, line.name);
  }
  const calls = lines.filter((line) => line.kind === "tool").length;
  const results = lines.filter((line) => line.kind === "tool_result").length;
  const count = [
    calls ? `${calls} tool call${calls === 1 ? "" : "s"}` : "",
    results ? `${results} result${results === 1 ? "" : "s"}` : "",
  ].filter(Boolean).join(", ");
  const tools = Array.from(new Set(lines
    .filter((line) => line.kind === "tool")
    .map((line) => iconName(line.name)))).slice(0, 6);
  const failed = lines.some((line) => line.failed);
  const rows = lines.map((line) => activityLine(line, names.get(line.id) || "")).join("");
  return `<details class="activity-cluster ${failed ? "failed" : ""}">
    <summary><span class="activity-chevron">${icon("chevron")}</span>
      <span class="activity-name">${esc(count || "Tool activity")}</span>
      <span class="activity-icons">${tools.map((tool) => `<span>${icon(tool)}</span>`).join("")}</span></summary>
    <div class="activity-list">${rows}</div></details>`;
}
