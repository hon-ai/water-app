import { describe, expect, it } from "vitest";
import { bestSentenceRange } from "./sentenceMatch";

describe("bestSentenceRange", () => {
  it("returns null for a single-sentence block (whole block is just as informative)", () => {
    const block = "The library was an old place.";
    const pill = "The library feels old, like a memory.";
    expect(bestSentenceRange(block, pill)).toBeNull();
  });

  it("picks the sentence that shares the most pill keywords", () => {
    const block =
      "The library was an old place. Jacob's ladder climbed forever. " +
      "Marcus turned away.";
    const pill = "The ladder feels like a spine — vertebrae climbing upward.";
    const r = bestSentenceRange(block, pill);
    expect(r).not.toBeNull();
    expect(block.slice(r!.start, r!.end)).toContain("ladder");
  });

  it("returns null when the winner doesn't beat the runner-up cleanly", () => {
    const block =
      "The ladder is old. The ladder is wooden. The ladder leans.";
    // Pill mentions "ladder" — every sentence ties on the only
    // meaningful token. Ambiguous → fall back to whole block.
    const pill = "The ladder. Old wooden ladder.";
    expect(bestSentenceRange(block, pill)).toBeNull();
  });

  it("returns null when no meaningful keywords overlap", () => {
    const block = "The library was an old place. She climbed the ladder.";
    // Pill uses entirely different vocabulary.
    const pill = "Pellet vendor crossroads abandoned tile.";
    expect(bestSentenceRange(block, pill)).toBeNull();
  });

  it("ignores stopwords + short tokens when scoring", () => {
    // The block has three sentences. Pill's only meaningful token
    // is "library"; stopwords like "the" + "is" shouldn't pull
    // every sentence into the tie.
    const block =
      "The library was an old place. The cat slept on the rug. " +
      "The lighthouse blinked in fog.";
    const pill = "Old library memory; the the the.";
    const r = bestSentenceRange(block, pill);
    // "library" only appears in the first sentence; pill has
    // "library" + "memory" + "old" = 3 meaningful tokens. First
    // sentence has "library" + "old" = 2 overlap. Others have 0.
    // Winner-vs-runner-up gap = 2, so the threshold (≥1) is met.
    expect(r).not.toBeNull();
    expect(block.slice(r!.start, r!.end)).toContain("library");
  });

  it("range falls within block bounds", () => {
    const block = "Sentence one here. Second one is longer and louder.";
    const pill = "Louder sentence longer.";
    const r = bestSentenceRange(block, pill);
    expect(r).not.toBeNull();
    expect(r!.start).toBeGreaterThanOrEqual(0);
    expect(r!.end).toBeLessThanOrEqual(block.length);
    expect(r!.end).toBeGreaterThan(r!.start);
  });
});
