/** Local message echoes reconciled by occurrence count, including duplicates. */

import type { MobileSnapshot } from "./types.js";

interface Echo {
  request: string;
  text: string;
  before: number;
}

function occurrences(value: MobileSnapshot, text: string): number {
  if (!value.active) return 0;
  const queued = value.active.outbox.filter((item) => item.message === text).length;
  const sent = value.active.transcript.filter(
    (line) => line.role === "user" && line.text === text
  ).length;
  return queued + sent;
}

export function echoManager(): {
  add(snapshot: MobileSnapshot, request: string, text: string): void;
  clear(): void;
  receive(snapshot: MobileSnapshot): void;
  reject(request: string | undefined): string;
  texts(): string[];
} {
  let items: Echo[] = [];
  return {
    add(snapshot, request, text): void {
      const before = occurrences(snapshot, text) + items.filter((item) => item.text === text).length;
      items.push({ request, text, before });
    },
    clear(): void {
      items = [];
    },
    receive(snapshot): void {
      items = items.filter((item) => occurrences(snapshot, item.text) <= item.before);
    },
    reject(request): string {
      const failed = items.find((item) => item.request === request);
      if (!failed) return "";
      items = items.filter((item) => item !== failed);
      return failed.text;
    },
    texts: () => items.map((item) => item.text),
  };
}
