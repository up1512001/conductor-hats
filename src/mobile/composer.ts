/**
 * Conductor's own composer row: the run settings sit under the message, each as
 * its mark and its current value, and open in place.
 *
 * They belong here rather than on a chat row or behind a sheet, because they are
 * what the next message is sent with. This is the arrangement Conductor uses on
 * the Mac, so the phone reads the same way round, and the marks are Conductor's
 * own: the provider's logo for the model, and the effort meter that fills as the
 * level rises.
 */

import { esc } from "./markup.js";
import { agentMark, icon } from "./icons.js";
import { effortAxis, effortLabel, effortLevels } from "./effort.js";
import type { Account, ActiveChat, Chat } from "./types.js";

const BAR = 2;
const GAP = 2;
const TALL = 15;
const SHORT = 5;

/** Conductor's meter: one bar per level, filled up to the one in force. */
function meter(levels: string[], value: string): string {
  const bars = levels.filter((level) => level !== "none");
  const width = bars.length * BAR + (bars.length - 1) * GAP;
  const filled = value === "none" ? 0 : bars.indexOf(value) + 1;
  const rects = bars.map((level, at) => {
    const height = bars.length === 1 ? TALL : SHORT + ((TALL - SHORT) * at) / (bars.length - 1);
    return `<rect x="${at * (BAR + GAP)}" y="${(TALL - height).toFixed(2)}" width="${BAR}"
      height="${height.toFixed(2)}" rx="1" opacity="${at < filled ? "1" : ".3"}"/>`;
  }).join("");
  return `<svg class="meter" viewBox="0 0 ${width} ${TALL}" aria-hidden="true">${rects}</svg>`;
}

function title(value: string): string {
  return value.slice(0, 1).toUpperCase() + value.slice(1);
}

export function modelLabel(value: string): string {
  const aliases: Record<string, string> = {
    opus: "Opus 4.8",
    "opus-1m": "Opus 4.8 1M",
    sonnet: "Sonnet 4.6",
    haiku: "Haiku 4.5",
    "opus-5-1m": "Opus 5",
  };
  const model = value.replace(/^claude-/, "").replace(/^gpt-/, "");
  if (aliases[model]) return aliases[model];
  const million = model.match(/^([a-z]+)-(\d+)-1m$/i);
  if (million) return `${title(million[1] || "")} ${million[2]} 1M`;
  const version = model.match(/^([a-z]+)-(\d+)-(\d+)(-1m)?$/i);
  if (version) {
    return `${title(version[1] || "")} ${version[2]}.${version[3]}${version[4] ? " 1M" : ""}`;
  }
  return model.split("-").filter(Boolean).map((part) => title(part)).join(" ");
}

export function composerControls(chat: Chat | null, active: ActiveChat | null): string {
  if (!chat) return "";
  const pending = active?.session === chat.session && !!active.controls?.length;
  const held = (setting: string, fallback: string): string =>
    (active?.session === chat.session ? active.controls || [] : [])
      .find((item) => item.setting === setting)?.value || fallback;
  const button = (setting: string, label: string, glyph: string): string =>
    `<button type="button" class="tool-btn ${pending ? "busy" : ""}" data-control="${esc(setting)}"
      aria-label="${esc(setting)}">${glyph}${label ? `<span>${esc(label)}</span>` : ""}</button>`;
  const model = held("model", chat.model);
  const effort = held("effort", chat.effort);
  const levels = effortLevels(chat.agent, model);
  const permission = held("permission", chat.permission || "default");
  return button("account", chat.next || chat.on || "Account", icon("user"))
    + button("model", model ? modelLabel(model) : "Model", agentMark(chat.agent))
    + (levels.length
      ? button(
        "effort",
        effort ? effortLabel(chat.agent, model, effort) : effortAxis(chat.agent),
        meter(levels, effort)
      )
      : "")
    + button("fast", held("fast", chat.fast ? "on" : "off") === "on" ? "Fast" : "", icon("zap"))
    + button("permission", permission === "default" ? "" : permission, icon("map"));
}

function menuItem(setting: string, value: string, current: string, label: string): string {
  return `<button type="button" class="menu-item ${value === current ? "on" : ""}"
    data-setting="${esc(setting)}" data-value="${esc(value)}"
    data-current="${esc(current)}">${esc(label)}</button>`;
}

/** The values one control offers, as a menu anchored to its button. */
export function controlMenu(
  chat: Chat,
  setting: string,
  accounts: Record<string, Account[]>,
  models: Record<string, string[]>
): string {
  const profiles = (accounts?.[chat.agent] || []).filter((item) => item.signed_in);
  const lists: Record<string, string[]> = {
    account: Array.from(new Set([chat.next, ...profiles.map((item) => item.name)].filter(Boolean))),
    effort: effortLevels(chat.agent, chat.model),
    fast: ["off", "on"],
    permission: ["default", "plan", "acceptEdits", "bypassPermissions"],
  };
  const now: Record<string, string> = {
    account: chat.next || chat.on,
    model: chat.model,
    effort: chat.effort,
    fast: chat.fast ? "on" : "off",
    permission: chat.permission || "default",
  };
  const current = now[setting] || "";
  const naming = (value: string): string =>
    setting === "effort" ? effortLabel(chat.agent, chat.model, value) : value;
  if (setting === "model") {
    const known = ["claude", "codex"].flatMap((agent) => models?.[agent] || []);
    return `<div class="menu model-menu">${["claude", "codex"].map((agent) => {
      const values = [...(models?.[agent] || [])];
      if (agent === chat.agent && current && !known.includes(current)) values.push(current);
      const label = agent === "claude" ? "Claude Code" : "Codex";
      return `<section class="menu-section"><div class="menu-label">${label}</div>
        ${values.map((value) => menuItem(setting, value, current, modelLabel(value))).join("")}</section>`;
    }).join("")}</div>`;
  }
  const values = Array.from(new Set([...(lists[setting] || []), current].filter(Boolean)));
  return `<div class="menu">${values
    .map((value) => menuItem(setting, value, current, naming(value)))
    .join("")}</div>`;
}
