/**
 * Phase 3.5 — Pill anchor resolver.
 *
 * Given a pill's anchor payload (captured at emerge time) and the
 * editor's current set of blocks, resolve the *current* char range
 * to highlight. The four tiers below let the highlight survive
 * paragraph splits, merges, typos, and partial deletions; only
 * full deletion of the snippet causes a fallback to whole-block
 * highlight + "drifted" pip.
 *
 * Contract per UX_SPEC.md §C.6.b:
 *
 *   1. Block-id + snippet substring  (cheap, common case)
 *   2. Block-hash + snippet substring (block-id changed; same content)
 *   3. Snippet-only fuzzy (≤ 2 char edits; typo-tolerant)
 *   4. Fallback to original block + `fallback: true`
 *
 * The resolver is a pure function — no DOM access. The caller
 * captures DOM block snapshots and passes them in; tests use a
 * synthetic block list.
 */

export interface AnchorPayload {
  /** The block-id the pill anchored to at emerge time. */
  blockId: string;
  /** The 3–10 word phrase the pill is reacting to. May be empty
   *  when the backend hasn't surfaced a trigger phrase — in that
   *  case the resolver falls straight to tier 4. */
  snippet: string;
  /** First 80 chars of the block's text at emerge time, normalized
   *  (lowercase, whitespace collapsed). Lets us locate the block
   *  even if its block-id has changed (paragraph split/merge). */
  blockHash: string;
  /** Char offset of the snippet within the block at trigger time.
   *  Used only as a search starting-point; not authoritative. */
  offsetHint: number;
}

export interface BlockSnapshot {
  blockId: string;
  text: string;
}

export type ResolutionTier = "id" | "hash" | "fuzzy" | "fallback";

export interface ResolvedAnchor {
  /** The block-id the highlight should land in. May differ from
   *  the anchor's original blockId (tier 2/3 found a different
   *  block). */
  blockId: string;
  /** Char offset within the block where the highlight starts. */
  start: number;
  /** Char offset within the block where the highlight ends (exclusive). */
  end: number;
  /** Which tier produced this result. Useful for telemetry +
   *  the "drifted" pip decision (tier === "fallback"). */
  tier: ResolutionTier;
}

/**
 * Normalize text for hashing: lowercase + collapse whitespace to a
 * single space + trim. Mirrors the discipline the caller uses to
 * compute `blockHash` at capture time, so equality compares apples
 * to apples.
 */
export function normalizeForHash(text: string): string {
  return text.toLowerCase().replace(/\s+/g, " ").trim();
}

/**
 * Compute the canonical blockHash from a block's text — first 80
 * characters of the normalized form. Exposed so the renderer and the
 * resolver agree on the recipe; changing it requires bumping both.
 */
export function computeBlockHash(text: string): string {
  return normalizeForHash(text).slice(0, 80);
}

/**
 * Find the substring offset of `needle` in `haystack`, preferring a
 * match near `near` when several occurrences exist. Returns -1 when
 * not found. The "near" preference matters when the writer has
 * duplicated a phrase: pick the one closest to where the pill
 * originally pointed.
 */
function indexNear(haystack: string, needle: string, near: number): number {
  if (needle.length === 0) return -1;
  // Cheap path: only one occurrence ⇒ no decision to make.
  const first = haystack.indexOf(needle);
  if (first === -1) return -1;
  const next = haystack.indexOf(needle, first + 1);
  if (next === -1) return first;
  // Multiple matches — pick the one with the smallest |index - near|.
  let best = first;
  let bestDistance = Math.abs(first - near);
  let ix = next;
  while (ix !== -1) {
    const d = Math.abs(ix - near);
    if (d < bestDistance) {
      best = ix;
      bestDistance = d;
    }
    ix = haystack.indexOf(needle, ix + 1);
  }
  return best;
}

/**
 * Levenshtein distance with an early-exit cap. Returns `cap + 1` as
 * soon as the running cost exceeds `cap`, so callers wanting "≤ k
 * edits" can short-circuit. We only need this for short snippets
 * (~3–10 words, < 80 chars), so the O(n*m) cost is negligible.
 */
function editDistance(a: string, b: string, cap: number): number {
  if (a === b) return 0;
  if (a.length === 0) return b.length;
  if (b.length === 0) return a.length;
  // Quick lower-bound: length difference must be ≤ cap.
  if (Math.abs(a.length - b.length) > cap) return cap + 1;

  let prev = new Array(b.length + 1).fill(0) as number[];
  let curr = new Array(b.length + 1).fill(0) as number[];
  for (let j = 0; j <= b.length; j++) prev[j] = j;

  for (let i = 1; i <= a.length; i++) {
    curr[0] = i;
    let rowMin = curr[0]!;
    for (let j = 1; j <= b.length; j++) {
      const cost = a[i - 1] === b[j - 1] ? 0 : 1;
      const sub = prev[j - 1]! + cost;
      const ins = curr[j - 1]! + 1;
      const del = prev[j]! + 1;
      const v = Math.min(sub, ins, del);
      curr[j] = v;
      if (v < rowMin) rowMin = v;
    }
    if (rowMin > cap) return cap + 1;
    [prev, curr] = [curr, prev];
  }
  return prev[b.length]!;
}

/**
 * Slide a window the size of `snippet` across `text` and return the
 * window offset whose contents are ≤ `cap` edits away from `snippet`.
 * Returns -1 when no window matches. Used by tier 3 (fuzzy snippet
 * search) — the typo-tolerant fallback when block-id + hash both
 * fail to locate an exact substring.
 *
 * The window step is 1 char; on a typical paragraph (~300 chars)
 * with a 40-char snippet this is ~260 distance computations, each
 * capped to O(snippet * snippet). Cheap.
 */
function fuzzyIndex(text: string, snippet: string, cap: number): number {
  if (snippet.length === 0) return -1;
  if (text.length < snippet.length - cap) return -1;
  // Try widths from snippet.length - cap up to snippet.length + cap.
  // A pure insertion/deletion can shift the matched window's length
  // by up to `cap`. For each width, slide and check.
  let best = -1;
  let bestDist = cap + 1;
  for (
    let width = Math.max(1, snippet.length - cap);
    width <= snippet.length + cap;
    width++
  ) {
    if (width > text.length) break;
    for (let i = 0; i + width <= text.length; i++) {
      const window = text.slice(i, i + width);
      const d = editDistance(window, snippet, cap);
      if (d < bestDist) {
        best = i;
        bestDist = d;
        if (d === 0) return best;
      }
    }
  }
  return best;
}

/**
 * Resolve an anchor against the current block list.
 *
 * Returns `null` only when *no* block exists by the original id AND
 * no fuzzy match was found anywhere — i.e. the snippet has been
 * fully deleted and the block was renamed/removed. Callers treat
 * `null` as "drop the highlight, don't render anything."
 */
export function resolveAnchor(
  payload: AnchorPayload,
  blocks: BlockSnapshot[],
): ResolvedAnchor | null {
  if (blocks.length === 0) return null;
  const { blockId, snippet, blockHash, offsetHint } = payload;

  // ── Tier 1: original block-id still present + contains snippet
  const sameId = blocks.find((b) => b.blockId === blockId);
  if (sameId && snippet.length > 0) {
    const ix = indexNear(sameId.text, snippet, offsetHint);
    if (ix !== -1) {
      return {
        blockId: sameId.blockId,
        start: ix,
        end: ix + snippet.length,
        tier: "id",
      };
    }
  }

  // ── Tier 2: block-hash match + contains snippet
  if (snippet.length > 0 && blockHash.length > 0) {
    for (const b of blocks) {
      if (computeBlockHash(b.text) !== blockHash) continue;
      const ix = indexNear(b.text, snippet, offsetHint);
      if (ix !== -1) {
        return {
          blockId: b.blockId,
          start: ix,
          end: ix + snippet.length,
          tier: "hash",
        };
      }
    }
  }

  // ── Tier 3: fuzzy snippet search across every block
  if (snippet.length > 0) {
    let bestBlockId: string | null = null;
    let bestStart = -1;
    let bestWidth = 0;
    let bestDist = 3; // strictly less than this — we want ≤ 2 edits
    for (const b of blocks) {
      // editDistance is O(snippet * snippet). To get the width that
      // matched (needed to compute `end`), we redo a width scan once
      // we know a window is close enough.
      const ix = fuzzyIndex(b.text, snippet, 2);
      if (ix === -1) continue;
      // Recover the matched width: try the window of length snippet
      // first; if that wasn't the best match, widen/narrow. We
      // recompute the distance for each width and keep the best.
      for (
        let width = Math.max(1, snippet.length - 2);
        width <= snippet.length + 2 && ix + width <= b.text.length;
        width++
      ) {
        const d = editDistance(b.text.slice(ix, ix + width), snippet, 2);
        if (d < bestDist) {
          bestBlockId = b.blockId;
          bestStart = ix;
          bestWidth = width;
          bestDist = d;
        }
      }
    }
    if (bestBlockId !== null && bestStart !== -1) {
      return {
        blockId: bestBlockId,
        start: bestStart,
        end: bestStart + bestWidth,
        tier: "fuzzy",
      };
    }
  }

  // ── Tier 4: fallback to original block (whole-paragraph highlight)
  if (sameId) {
    return {
      blockId: sameId.blockId,
      start: 0,
      end: sameId.text.length,
      tier: "fallback",
    };
  }

  return null;
}
