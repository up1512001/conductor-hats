/**
 * Which effort levels a model actually offers, and what Conductor calls them.
 *
 * The levels are per model, not per agent: Sonnet 4.6 has no extra-high, Opus
 * has Ultracode above Max, Haiku has no effort control at all, and gpt-5.6 calls
 * its lowest level Light. Offering one list per agent meant the phone showed
 * levels Conductor would never accept, and the setting came back refused.
 *
 * Copied from Conductor's own registry rather than guessed. To read it again
 * after a release: `hats assets --app <app> --dump /assets/index-*.js`, then
 * search for `defaultLevel` and follow the level arrays and label maps.
 */

const NONE_TO_XHIGH = ["none", "low", "medium", "high", "xhigh"];
const NONE_TO_MAX = [...NONE_TO_XHIGH, "max"];
const NONE_TO_ULTRA = [...NONE_TO_MAX, "ultra"];
const TO_MAX = ["low", "medium", "high", "max"];
const TO_XHIGH_MAX = ["low", "medium", "high", "xhigh", "max"];
const TO_ULTRACODE = [...TO_XHIGH_MAX, "ultracode"];
const TO_XHIGH = ["low", "medium", "high", "xhigh"];
const TO_HIGH = ["low", "medium", "high"];

interface Scale {
  levels: string[];
  labels: Record<string, string>;
  axis: string;
  models: Record<string, string[]>;
  overrides?: Record<string, string>;
  overridden?: string[];
}

const CLAUDE_LABELS: Record<string, string> = {
  low: "Low", medium: "Medium", high: "High", xhigh: "Extra high",
  max: "Max", ultracode: "Ultracode",
};
const CODEX_LABELS: Record<string, string> = {
  none: "Off", low: "Low", medium: "Medium", high: "High", xhigh: "Extra high",
  max: "Max", ultra: "Ultra",
};

const SCALES: Record<string, Scale> = {
  claude: {
    levels: TO_ULTRACODE,
    labels: CLAUDE_LABELS,
    axis: "Effort",
    models: {
      "fable-5": TO_ULTRACODE,
      "opus-5-1m": TO_ULTRACODE,
      "opus-4-8-1m": TO_ULTRACODE,
      "opus-4-7-1m": TO_ULTRACODE,
      "opus-4-8": TO_ULTRACODE,
      "opus-4-7": TO_ULTRACODE,
      "opus-1m": TO_ULTRACODE,
      opus: TO_ULTRACODE,
      "opus-4-6-1m": TO_MAX,
      "sonnet-5-1m": TO_XHIGH_MAX,
      "sonnet-4-6-1m": TO_MAX,
      "sonnet-4-6": TO_MAX,
      sonnet: TO_MAX,
      "haiku-4-5": [],
      haiku: [],
    },
  },
  codex: {
    levels: NONE_TO_ULTRA,
    labels: CODEX_LABELS,
    axis: "Thinking",
    overrides: { low: "Light" },
    overridden: ["gpt-5.6-sol", "gpt-5.6-terra", "gpt-5.6-luna"],
    models: {
      "gpt-5.6-sol": NONE_TO_ULTRA,
      "gpt-5.6-terra": NONE_TO_ULTRA,
      "gpt-5.6-luna": NONE_TO_MAX,
      "gpt-5.5": NONE_TO_XHIGH,
      "gpt-5.4": NONE_TO_XHIGH,
      "gpt-5.3-codex-spark": NONE_TO_XHIGH,
      "gpt-5.3-codex": NONE_TO_XHIGH,
      "gpt-5.2-codex": NONE_TO_XHIGH,
    },
  },
  cursor: {
    levels: TO_XHIGH,
    labels: { low: "Low", medium: "Medium", high: "High", xhigh: "Extra high" },
    axis: "Effort",
    models: { auto: [], "composer-2.5": [], "grok-4.6": TO_XHIGH, "grok-4.5": TO_HIGH },
  },
};

function named(model: string): string {
  return model.replace(/^claude-/, "");
}

/** Conductor's own name for this axis: effort for Claude, thinking for Codex. */
export function effortAxis(agent: string): string {
  return SCALES[agent]?.axis || "Effort";
}

/**
 * A model Conductor does not list keeps the agent's whole scale, because a
 * shorter guess would hide a level the Mac is willing to accept. An agent hats
 * has no scale for offers nothing, which hides the control rather than inventing
 * levels Conductor would refuse.
 */
export function effortLevels(agent: string, model: string): string[] {
  const found = SCALES[agent];
  if (!found) return [];
  return found.models[named(model)] || found.levels;
}

export function effortLabel(agent: string, model: string, value: string): string {
  const found = SCALES[agent];
  if (!found) return value;
  const override = found.overridden?.includes(named(model)) ? found.overrides?.[value] : undefined;
  return override || found.labels[value] || value;
}
