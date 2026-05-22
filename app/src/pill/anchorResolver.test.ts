import { describe, expect, it } from "vitest";
import {
  computeBlockHash,
  normalizeForHash,
  resolveAnchor,
  type AnchorPayload,
  type BlockSnapshot,
} from "./anchorResolver";

/**
 * Anchor payload helper. The snippet is the trigger phrase; the
 * blockHash is computed from whatever the block looked like at
 * "trigger time" — tests pass the same text as the block they
 * intend to be the original.
 */
function anchor(
  blockId: string,
  blockText: string,
  snippet: string,
  offsetHint = blockText.indexOf(snippet),
): AnchorPayload {
  return {
    blockId,
    snippet,
    blockHash: computeBlockHash(blockText),
    offsetHint: offsetHint >= 0 ? offsetHint : 0,
  };
}

const ORIGINAL_TEXT =
  "She was still avoiding his eyes, something she did not want him to see.";
const ORIGINAL_BLOCK: BlockSnapshot = {
  blockId: "^bk-aaa1",
  text: ORIGINAL_TEXT,
};
const ORIGINAL_PAYLOAD = anchor(
  ORIGINAL_BLOCK.blockId,
  ORIGINAL_TEXT,
  "still avoiding his eyes",
);

describe("normalizeForHash", () => {
  it("collapses whitespace and lowercases", () => {
    expect(normalizeForHash("  HELLO   world\nthere  ")).toBe(
      "hello world there",
    );
  });
});

describe("computeBlockHash", () => {
  it("caps at 80 chars after normalization", () => {
    const s = "x".repeat(120);
    expect(computeBlockHash(s).length).toBe(80);
  });
});

describe("resolveAnchor", () => {
  it("identity hit — block-id present, snippet present (tier id)", () => {
    const result = resolveAnchor(ORIGINAL_PAYLOAD, [ORIGINAL_BLOCK]);
    expect(result).not.toBeNull();
    expect(result!.tier).toBe("id");
    expect(result!.blockId).toBe(ORIGINAL_BLOCK.blockId);
    expect(ORIGINAL_TEXT.slice(result!.start, result!.end)).toBe(
      "still avoiding his eyes",
    );
  });

  it("paragraph split — original block-id gone, hash matches a sibling (tier hash)", () => {
    // The writer split the paragraph in half. The right half kept
    // the snippet but got a new block-id. The left half kept the
    // original blockId but lost the snippet, so tier 1 fails on
    // "contains snippet"; tier 2 finds the right half by content
    // hash (computed from full text at trigger time → matches a
    // block that *still* starts with the same 80 chars).
    //
    // To test this cleanly, set up two blocks where the new
    // block-with-snippet is the one whose first 80 chars hash
    // equal the original. (In practice split usually preserves
    // the *left* half's first 80 chars and the *right* half loses
    // them, but the hash matches whichever side still leads with
    // the same prefix.)
    const blocks: BlockSnapshot[] = [
      { blockId: "^bk-leftA", text: "She was still avoiding his eyes." },
      {
        blockId: "^bk-rightA",
        text: ORIGINAL_TEXT, // unchanged content, new id
      },
    ];
    const result = resolveAnchor(
      anchor(
        "^bk-aaa1", // original id is gone from the doc
        ORIGINAL_TEXT,
        "something she did not want him to see",
      ),
      blocks,
    );
    expect(result).not.toBeNull();
    expect(result!.tier).toBe("hash");
    expect(result!.blockId).toBe("^bk-rightA");
    expect(blocks[1]!.text.slice(result!.start, result!.end)).toBe(
      "something she did not want him to see",
    );
  });

  it("paragraph merge — original block-id present but text now longer (tier id finds offset shifted)", () => {
    // Backspace at the start of paragraph 2 merged it into
    // paragraph 1. PM keeps paragraph 1's blockId. The snippet is
    // still in there, just at a different offset; tier 1 finds it.
    const merged: BlockSnapshot = {
      blockId: "^bk-aaa1",
      text:
        "Across the room, the lamp flickered once. " +
        ORIGINAL_TEXT,
    };
    const result = resolveAnchor(ORIGINAL_PAYLOAD, [merged]);
    expect(result).not.toBeNull();
    expect(result!.tier).toBe("id");
    expect(merged.text.slice(result!.start, result!.end)).toBe(
      "still avoiding his eyes",
    );
  });

  it("typo correction — snippet has a typo vs current text (tier fuzzy, ≤2 edits)", () => {
    // The writer fixed a typo: "stil avoidin" → "still avoiding".
    // Snippet at trigger time was the typo'd version; current text
    // has the correction. Tier 1 fails (substring miss); tier 3
    // succeeds with ≤ 2 edits.
    const correctedText =
      "She was still avoiding his eyes, something she did not want him to see.";
    const corrected: BlockSnapshot = {
      blockId: "^bk-aaa1",
      text: correctedText,
    };
    const result = resolveAnchor(
      {
        blockId: "^bk-aaa1",
        snippet: "stil avoidin his eyes",
        // Hash is computed from the *current* text in our helper;
        // for this test we set it from the typo'd version so tier 2
        // can't sneak in.
        blockHash: computeBlockHash("She was stil avoidin his eyes"),
        offsetHint: 8,
      },
      [corrected],
    );
    expect(result).not.toBeNull();
    expect(result!.tier).toBe("fuzzy");
    expect(result!.blockId).toBe("^bk-aaa1");
    // The recovered window should land at or near the corrected
    // phrase — not necessarily exact bounds, since fuzzy width may
    // pick a slightly longer or shorter match.
    const matched = correctedText.slice(result!.start, result!.end);
    expect(matched).toMatch(/still avoiding his eye/);
  });

  it("partial deletion — snippet still substring-present at a new offset (tier id)", () => {
    // The writer cut some surrounding words. Snippet itself intact;
    // tier 1 still finds it.
    const trimmed: BlockSnapshot = {
      blockId: "^bk-aaa1",
      text: "still avoiding his eyes.",
    };
    const result = resolveAnchor(ORIGINAL_PAYLOAD, [trimmed]);
    expect(result).not.toBeNull();
    expect(result!.tier).toBe("id");
    expect(trimmed.text.slice(result!.start, result!.end)).toBe(
      "still avoiding his eyes",
    );
  });

  it("partial deletion — snippet half-cut, fuzzy still finds the remaining fragment", () => {
    // The writer chopped a few characters off the end of the
    // snippet. The remaining fragment is within 2 edits of the
    // original snippet, so tier 3 catches it.
    const trimmed: BlockSnapshot = {
      blockId: "^bk-aaa1",
      text: "she was still avoiding his ey.",
    };
    const result = resolveAnchor(
      {
        blockId: "^bk-aaa1",
        snippet: "still avoiding his eyes",
        blockHash: computeBlockHash(ORIGINAL_TEXT),
        offsetHint: 8,
      },
      [trimmed],
    );
    expect(result).not.toBeNull();
    // Tier 1 already exhausted (no substring "still avoiding his eyes");
    // tier 3 finds a 2-edit match.
    expect(result!.tier).toBe("fuzzy");
  });

  it("full deletion — block still exists but snippet utterly gone (tier fallback)", () => {
    // The writer deleted the snippet entirely; the block remains
    // (different content). Tier 1–3 all fail; tier 4 falls back to
    // the whole-block highlight + drifted flag.
    const wiped: BlockSnapshot = {
      blockId: "^bk-aaa1",
      text: "completely unrelated prose now.",
    };
    const result = resolveAnchor(ORIGINAL_PAYLOAD, [wiped]);
    expect(result).not.toBeNull();
    expect(result!.tier).toBe("fallback");
    expect(result!.blockId).toBe(wiped.blockId);
    expect(result!.start).toBe(0);
    expect(result!.end).toBe(wiped.text.length);
  });

  it("full deletion + block removed — returns null (highlight dropped)", () => {
    const orphan: BlockSnapshot = {
      blockId: "^bk-other",
      text: "totally different text here that shares no fuzzy match.",
    };
    const result = resolveAnchor(ORIGINAL_PAYLOAD, [orphan]);
    expect(result).toBeNull();
  });

  it("multiple snippet occurrences — picks the one near offsetHint", () => {
    // The writer duplicated the trigger phrase. The hint says we
    // anchored to the second occurrence; the resolver should prefer
    // that one, not the first.
    const dup: BlockSnapshot = {
      blockId: "^bk-aaa1",
      text:
        "still avoiding his eyes earlier in the line; later, she was still avoiding his eyes again.",
    };
    const secondIx = dup.text.lastIndexOf("still avoiding his eyes");
    const result = resolveAnchor(
      {
        blockId: "^bk-aaa1",
        snippet: "still avoiding his eyes",
        blockHash: computeBlockHash(dup.text),
        offsetHint: secondIx,
      },
      [dup],
    );
    expect(result).not.toBeNull();
    expect(result!.start).toBe(secondIx);
  });

  it("empty snippet falls straight to fallback", () => {
    const result = resolveAnchor(
      {
        blockId: "^bk-aaa1",
        snippet: "",
        blockHash: "",
        offsetHint: 0,
      },
      [ORIGINAL_BLOCK],
    );
    expect(result).not.toBeNull();
    expect(result!.tier).toBe("fallback");
  });
});
