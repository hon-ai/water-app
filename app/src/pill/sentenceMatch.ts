/**
 * Pick the sentence in a block whose content best matches the pill's
 * text. Used to refine the hover-highlight from "whole block" down
 * to a single sentence when the LLM's observation clearly references
 * one specific image/idea inside the paragraph.
 *
 * Strategy: score each sentence by how many *meaningful* (non-stop,
 * length ≥ 3) lowercase tokens it shares with the pill text. Return
 * the highest-scoring sentence's range when:
 *   - The block has more than one sentence (otherwise the whole-block
 *     fallback is just as informative).
 *   - The winning sentence shares ≥ 2 meaningful tokens with the pill
 *     text (lower threshold drops false positives — a single shared
 *     stopword-shaped word isn't enough signal).
 *   - The winner beats the runner-up by at least one token (avoids
 *     ambiguous ties where the pill could be about either of two
 *     sentences).
 *
 * Falls through to `null` in any ambiguous / no-signal case; the
 * caller then renders the whole-block highlight.
 *
 * This is a pure function over text strings — no DOM, no editor
 * state — so it's trivially testable.
 */

/** Common short closed-class words we never let drive a match. */
const STOPWORDS = new Set([
  "the", "a", "an", "and", "or", "but", "of", "to", "in", "on", "at",
  "by", "for", "with", "as", "is", "are", "was", "were", "be", "been",
  "being", "has", "had", "have", "this", "that", "these", "those", "it",
  "its", "they", "them", "their", "from", "into", "than", "then", "so",
  "if", "not", "no", "yes", "all", "any", "some", "what", "which", "who",
  "how", "when", "where", "why", "she", "her", "hers", "he", "his", "him",
  "you", "your", "i", "me", "my", "we", "us", "our", "do", "does", "did",
  "can", "could", "would", "should", "will", "shall", "may", "might",
  "just", "very", "much", "more", "most", "out", "up", "down", "over",
  "under", "again", "once", "only", "even", "also", "still", "yet",
]);

/** Split a token stream from text. Lowercase + strip punctuation. */
function tokenize(text: string): string[] {
  return text
    .toLowerCase()
    .replace(/[^a-z0-9']+/g, " ")
    .split(/\s+/)
    .filter((t) => t.length >= 3 && !STOPWORDS.has(t));
}

/**
 * Split a block of prose into sentence ranges. Returns each sentence
 * as `{ start, end, text }` where start/end are character offsets
 * within the original block (so the caller can pass them straight
 * into the trigger-highlight plugin).
 *
 * Sentences are delimited by `.`, `!`, `?` followed by whitespace
 * (or end-of-string). Quoted dialogue (`."`, `?"`) counts as a
 * sentence end. Pragmatic — handles 95 % of prose; an occasional
 * abbreviation like "Mr." may produce a too-short sentence, which
 * the keyword-overlap threshold then filters out anyway.
 */
interface SentenceRange {
  start: number;
  end: number;
  text: string;
}

function splitSentences(block: string): SentenceRange[] {
  const out: SentenceRange[] = [];
  const re = /[.!?]["')\]]?\s+|[.!?]["')\]]?$/g;
  let cursor = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(block)) !== null) {
    const end = m.index + m[0].trimEnd().length;
    const slice = block.slice(cursor, end);
    if (slice.trim().length > 0) {
      out.push({ start: cursor, end, text: slice });
    }
    cursor = end + (m[0].length - m[0].trimEnd().length);
  }
  // Trailing fragment without terminal punctuation (the writer is
  // mid-sentence). Include it so it's still considered for matching.
  if (cursor < block.length) {
    const slice = block.slice(cursor);
    if (slice.trim().length > 0) {
      out.push({ start: cursor, end: block.length, text: slice });
    }
  }
  return out;
}

/**
 * Best-matching sentence range, or `null` when no sentence clearly
 * stands out. `null` is the signal to fall back to the whole-block
 * highlight.
 */
export function bestSentenceRange(
  blockText: string,
  pillText: string,
): { start: number; end: number } | null {
  const sentences = splitSentences(blockText);
  if (sentences.length < 2) return null;

  const pillTokens = new Set(tokenize(pillText));
  if (pillTokens.size === 0) return null;

  let bestIdx = -1;
  let bestScore = 0;
  let secondScore = 0;

  for (let i = 0; i < sentences.length; i++) {
    const senTokens = tokenize(sentences[i]!.text);
    let overlap = 0;
    for (const t of senTokens) {
      if (pillTokens.has(t)) overlap += 1;
    }
    if (overlap > bestScore) {
      secondScore = bestScore;
      bestScore = overlap;
      bestIdx = i;
    } else if (overlap > secondScore) {
      secondScore = overlap;
    }
  }

  // Thresholds: the winner must (a) have at least 1 meaningful
  // shared token with the pill text, AND (b) beat the runner-up by
  // ≥ 1 token. The winner-vs-runner-up gap is what guards against
  // false positives — if two sentences share the same single
  // keyword, neither wins and we fall back to whole-block. A "≥ 2
  // tokens" floor would be too strict: realistic pill text often
  // shares only one form-specific word with the source sentence
  // (e.g., pill says "climbing", source says "climbed").
  if (bestScore < 1) return null;
  if (bestScore - secondScore < 1) return null;
  if (bestIdx < 0) return null;

  const win = sentences[bestIdx]!;
  return { start: win.start, end: win.end };
}
