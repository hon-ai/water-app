import type { HeatMetricKind } from "../ipc/commands";

/**
 * Heatmap hover-tooltip phrase bank. The canonical source is
 * `prompts/heat/phrasebank.toml`; this TS file mirrors it so the
 * renderer doesn't have to round-trip TOML at runtime.
 *
 * Phrase selection: the renderer hashes (paragraph_ix, metric) into a
 * stable index so the same hover always yields the same phrase
 * (no churn on every mouse-enter).
 *
 * Style notes (carried from phrasebank.toml):
 * - Lowercase, no punctuation, two-three words.
 * - Read as a noticing, not a label.
 * - Don't comment on writing craft — name the shape, not the quality.
 */

type Bucket = "low" | "mid" | "high" | "cold" | "neutral" | "warm";

const PHRASES: Record<HeatMetricKind, Partial<Record<Bucket, string[]>>> = {
  pacing: {
    low: ["holding", "resting", "slow burn", "unhurried", "patient", "still", "settled", "lingering"],
    mid: ["even pacing", "measured", "sustained", "moving", "steady", "set forth", "in step"],
    high: ["running", "quick", "dense", "rushing", "racing", "tight pulse", "in spate"],
  },
  valence: {
    cold: ["cold", "withdrawn", "guarded", "wintering", "thinning out", "drained", "remote"],
    neutral: ["even", "neutral", "observational", "level", "muted", "noticing"],
    warm: ["warm", "tender", "alive", "brightening", "open", "soft", "hopeful"],
  },
  coherence: {
    low: ["topic break", "side step", "drifting", "sudden jump", "new beat", "cut"],
    mid: ["shifting", "in motion", "transitional", "turning", "reorienting"],
    high: ["tightly woven", "continuous", "threaded", "connected", "of a piece", "carried over"],
  },
  presence: {
    low: ["alone", "single voice", "solo", "uninhabited", "spare cast"],
    mid: ["small cast", "two or three", "few present", "company"],
    high: ["crowded", "many voices", "thick cast", "full room"],
  },
  world_refs: {
    low: ["spare setting", "world quiet", "unanchored"],
    mid: ["noting places", "world brushed", "named ground"],
    high: ["deeply rooted", "rich setting", "world dense", "many places named"],
  },
};

function bucketFor(kind: HeatMetricKind, value: number): Bucket {
  if (kind === "valence") {
    if (value < -0.33) return "cold";
    if (value > 0.33) return "warm";
    return "neutral";
  }
  if (value < 0.33) return "low";
  if (value > 0.66) return "high";
  return "mid";
}

/**
 * Return the tooltip phrase for `(kind, value, paragraph_ix)`.
 * Deterministic per (kind, bucket, ix) so hover doesn't churn through
 * phrases — the same paragraph always shows the same word.
 */
export function phraseFor(
  kind: HeatMetricKind,
  value: number,
  paragraph_ix: number,
): string {
  const bucket = bucketFor(kind, value);
  const list = PHRASES[kind][bucket] ?? [];
  if (list.length === 0) return "";
  // Small FNV-1a-style hash over (kind + ix + bucket) so the result is
  // stable across reloads and won't drift if the phrase list reorders.
  const seed = `${kind}:${bucket}:${paragraph_ix}`;
  let h = 0x811c9dc5;
  for (let i = 0; i < seed.length; i++) {
    h ^= seed.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  const idx = Math.abs(h) % list.length;
  return list[idx]!;
}
