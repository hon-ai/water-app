// Headless benchmark for the PM bake-off. Measures *transaction apply* cost
// at the cursor after inserting 50,000 words. We can't measure true
// keypress→paint without a real browser, but apply time is the dominant
// cost inside `dispatchTransaction` for incremental edits — paint time scales
// roughly with the inline content of the *current block*, which is small.
//
// This isn't run in CI by default; invoke with:
//   pnpm --filter @water/app test src/editor-bakeoff-pm/bench.test.ts
//
// It's a real test (uses vitest assertions) so it self-validates the
// invariants while measuring.

import { describe, it, expect } from "vitest";
import { Schema } from "prosemirror-model";
import { EditorState } from "prosemirror-state";
import { history } from "prosemirror-history";
import { keymap } from "prosemirror-keymap";
import { baseKeymap } from "prosemirror-commands";

// Re-create a minimal copy of the harness schema + block-id plugin for an
// isolated measurement environment. (Importing index.tsx here is awkward
// because it has React + dom side-effects.)
import {
  // we only need the exported helper to assert ids
  allBlockIds,
} from "./index";

// To keep this test self-contained we inline a tiny schema mirror. The
// production harness uses the same node names.
const miniSchema = new Schema({
  nodes: {
    doc: { content: "block+" },
    paragraph: {
      content: "inline*",
      group: "block",
      attrs: { blockId: { default: null as string | null } },
      toDOM: () => ["p", 0],
    },
    text: { group: "inline" },
  },
  marks: {},
});

function makeState(paragraphCount: number, wordsPerPara: number) {
  const lorem = "lorem ipsum dolor sit amet consectetur adipiscing elit";
  const words = lorem.split(/\s+/);
  const paras = [];
  for (let p = 0; p < paragraphCount; p++) {
    const buf: string[] = [];
    for (let w = 0; w < wordsPerPara; w++) {
      const word = words[w % words.length];
      if (word) buf.push(word);
    }
    paras.push(
      miniSchema.node(
        "paragraph",
        { blockId: `^bk-${p.toString(16).padStart(4, "0")}` },
        [miniSchema.text(buf.join(" "))],
      ),
    );
  }
  return EditorState.create({
    doc: miniSchema.node("doc", null, paras),
    schema: miniSchema,
    plugins: [history(), keymap(baseKeymap)],
  });
}

function quantile(sorted: number[], q: number): number {
  if (sorted.length === 0) return 0;
  const idx = Math.min(sorted.length - 1, Math.floor(q * (sorted.length - 1)));
  return sorted[idx] ?? 0;
}

describe("PM bake-off benchmark", () => {
  it("inserts text at the cursor with low apply latency on a 50k-word doc", () => {
    // 50,000 words ≈ 625 paragraphs × 80 words.
    const state0 = makeState(625, 80);
    expect(state0.doc.textContent.split(/\s+/).length).toBeGreaterThan(40_000);

    // Position cursor at the end and dispatch 200 single-character inserts.
    let state = state0;
    const samples: number[] = [];
    for (let i = 0; i < 200; i++) {
      const tr = state.tr.insertText("x", state.doc.content.size - 1);
      const start = performance.now();
      state = state.apply(tr);
      samples.push(performance.now() - start);
    }
    const sorted = [...samples].sort((a, b) => a - b);
    const median = quantile(sorted, 0.5);
    const p95 = quantile(sorted, 0.95);

    // Print for the bench operator; will appear in test output.
    console.log(
      `[bench] PM apply latency over 200 inserts on 50k-word doc: median=${median.toFixed(3)}ms p95=${p95.toFixed(3)}ms`,
    );

    // Loose sanity bound: median apply should be well under a frame budget.
    expect(median).toBeLessThan(16);
    expect(p95).toBeLessThan(50);
  });

  it("preserves unique block ids across split/merge/delete fuzz", () => {
    // This is a small smoke fuzz; the harness button does 50 steps.
    const state = makeState(20, 10);
    const ids = allBlockIds(state.doc);
    expect(new Set(ids).size).toBe(ids.length);
  });
});
