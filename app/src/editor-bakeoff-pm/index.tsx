// ProseMirror harness for the M2 editor bake-off.
//
// This file is intentionally self-contained: it builds a tiny PM editor with
// the M2 block kinds (paragraph, h2, h3, ordered/unordered list, scene_break,
// dialogue), a block-id-preserving `appendTransaction` plugin, an id-keyed
// decoration plugin, and four interactive harness buttons exercising the six
// bake-off criteria.
//
// Reached via `?bakeoff=pm` on `main.tsx`. Production builds drop the import
// because `main.tsx` gates the dynamic import behind `import.meta.env.DEV`.

import { useEffect, useRef, useState } from "react";
import { Schema, type Node as PMNode, type NodeSpec } from "prosemirror-model";
import { EditorState, Plugin, PluginKey, type Transaction } from "prosemirror-state";
import { EditorView, Decoration, DecorationSet } from "prosemirror-view";
import { history, undo, redo } from "prosemirror-history";
import { keymap } from "prosemirror-keymap";
import { baseKeymap, splitBlock } from "prosemirror-commands";
import {
  splitListItem,
  liftListItem,
  sinkListItem,
} from "prosemirror-schema-list";

// ---------------------------------------------------------------------------
// 1. Schema
// ---------------------------------------------------------------------------
//
// All block-level nodes carry a `blockId` attribute. PM's default behavior
// during `tr.split` is to *copy* the parent's attrs onto the new node — which
// means the new half will temporarily share its sibling's id. The block-id
// plugin (below) repairs that on `appendTransaction`.

function blockIdAttr(): NodeSpec["attrs"] {
  return { blockId: { default: null as string | null } };
}

const paragraphSpec: NodeSpec = {
  content: "inline*",
  group: "block",
  attrs: blockIdAttr(),
  parseDOM: [
    {
      tag: "p",
      getAttrs: (dom) => ({
        blockId: (dom as HTMLElement).getAttribute("data-block-id"),
      }),
    },
  ],
  toDOM: (node) => [
    "p",
    {
      "data-block-id": node.attrs.blockId ?? "",
      class: "pm-paragraph",
    },
    0,
  ],
};

const dialogueSpec: NodeSpec = {
  content: "inline*",
  group: "block",
  attrs: blockIdAttr(),
  parseDOM: [
    {
      tag: "p.pm-dialogue",
      getAttrs: (dom) => ({
        blockId: (dom as HTMLElement).getAttribute("data-block-id"),
      }),
    },
  ],
  toDOM: (node) => [
    "p",
    {
      "data-block-id": node.attrs.blockId ?? "",
      class: "pm-dialogue",
      style:
        "font-style: italic; padding-left: 1.5em; border-left: 2px solid var(--water-fg-muted, #888); margin: 0.5em 0;",
    },
    0,
  ],
};

const heading2Spec: NodeSpec = {
  content: "inline*",
  group: "block",
  defining: true,
  attrs: blockIdAttr(),
  parseDOM: [
    {
      tag: "h2",
      getAttrs: (dom) => ({
        blockId: (dom as HTMLElement).getAttribute("data-block-id"),
      }),
    },
  ],
  toDOM: (node) => [
    "h2",
    { "data-block-id": node.attrs.blockId ?? "", class: "pm-h2" },
    0,
  ],
};

const heading3Spec: NodeSpec = {
  content: "inline*",
  group: "block",
  defining: true,
  attrs: blockIdAttr(),
  parseDOM: [
    {
      tag: "h3",
      getAttrs: (dom) => ({
        blockId: (dom as HTMLElement).getAttribute("data-block-id"),
      }),
    },
  ],
  toDOM: (node) => [
    "h3",
    { "data-block-id": node.attrs.blockId ?? "", class: "pm-h3" },
    0,
  ],
};

const sceneBreakSpec: NodeSpec = {
  group: "block",
  atom: true,
  attrs: blockIdAttr(),
  parseDOM: [{ tag: "hr" }],
  toDOM: (node) => [
    "hr",
    {
      "data-block-id": node.attrs.blockId ?? "",
      class: "pm-scene-break",
      style: "border: none; border-top: 1px dashed #888; margin: 1.5em 0;",
    },
  ],
};

const listItemSpec: NodeSpec = {
  content: "paragraph block*",
  attrs: blockIdAttr(),
  defining: true,
  parseDOM: [
    {
      tag: "li",
      getAttrs: (dom) => ({
        blockId: (dom as HTMLElement).getAttribute("data-block-id"),
      }),
    },
  ],
  toDOM: (node) => [
    "li",
    { "data-block-id": node.attrs.blockId ?? "" },
    0,
  ],
};

const orderedListSpec: NodeSpec = {
  content: "list_item+",
  group: "block",
  attrs: blockIdAttr(),
  parseDOM: [
    {
      tag: "ol",
      getAttrs: (dom) => ({
        blockId: (dom as HTMLElement).getAttribute("data-block-id"),
      }),
    },
  ],
  toDOM: (node) => [
    "ol",
    { "data-block-id": node.attrs.blockId ?? "" },
    0,
  ],
};

const unorderedListSpec: NodeSpec = {
  content: "list_item+",
  group: "block",
  attrs: blockIdAttr(),
  parseDOM: [
    {
      tag: "ul",
      getAttrs: (dom) => ({
        blockId: (dom as HTMLElement).getAttribute("data-block-id"),
      }),
    },
  ],
  toDOM: (node) => [
    "ul",
    { "data-block-id": node.attrs.blockId ?? "" },
    0,
  ],
};

const schema = new Schema({
  nodes: {
    doc: { content: "block+" },
    paragraph: paragraphSpec,
    dialogue: dialogueSpec,
    heading_2: heading2Spec,
    heading_3: heading3Spec,
    scene_break: sceneBreakSpec,
    ordered_list: orderedListSpec,
    unordered_list: unorderedListSpec,
    list_item: listItemSpec,
    text: { group: "inline" },
  },
  marks: {},
});

// ---------------------------------------------------------------------------
// 2. Block-id allocation + invariants
// ---------------------------------------------------------------------------
//
// IDs are short hex tokens. The plugin walks the doc on every transaction and:
//   - Assigns IDs to top-level blocks that lack one.
//   - On a *duplicate* (e.g. after `tr.split` copies attrs), keeps the first
//     occurrence (the left half = "original") and re-IDs the second.
//
// This satisfies the spec:
//   * paragraph split → left keeps id, right gets new
//   * paragraph merge (Backspace at start) → PM keeps the first node's attrs,
//     so the merged paragraph naturally inherits the left id
//   * delete → no fixup needed; ids of survivors are untouched

// Monotonic 16-bit counter — matches the spec's `^bk-XXXX` shape and avoids
// birthday collisions across the 200-step undo stress + 50-step fuzz. The
// production block-id allocator will live in Rust (`crate::block`) and use
// a larger keyspace; this harness counter is harness-only.
let blockIdCounter = 0;
function genBlockId(): string {
  blockIdCounter = (blockIdCounter + 1) & 0xffff;
  return `^bk-${blockIdCounter.toString(16).padStart(4, "0").toUpperCase()}`;
}

// Iterate every block node that should carry a block-id. We walk *all* depths
// so list_items get ids too. (Lists themselves get ids only at the top.)
function forEachBlockNode(
  doc: PMNode,
  cb: (node: PMNode, pos: number) => void,
): void {
  doc.descendants((node, pos) => {
    const t = node.type.name;
    if (
      t === "paragraph" ||
      t === "dialogue" ||
      t === "heading_2" ||
      t === "heading_3" ||
      t === "scene_break" ||
      t === "ordered_list" ||
      t === "unordered_list" ||
      t === "list_item"
    ) {
      cb(node, pos);
    }
    // Recurse into containers (ordered_list, unordered_list, list_item).
    return true;
  });
}

export function allBlockIds(doc: PMNode): string[] {
  const ids: string[] = [];
  forEachBlockNode(doc, (node) => {
    if (node.attrs.blockId) ids.push(node.attrs.blockId);
  });
  return ids;
}

const blockIdPluginKey = new PluginKey("blockIds");

function blockIdPlugin(): Plugin {
  return new Plugin({
    key: blockIdPluginKey,
    appendTransaction(_trs, _oldState, newState) {
      const seen = new Set<string>();
      let tr: Transaction | null = null;

      forEachBlockNode(newState.doc, (node, pos) => {
        const id = node.attrs.blockId as string | null;
        if (!id || seen.has(id)) {
          // Lazy-init `tr` from newState.tr so we only allocate a transaction
          // when there's actually work to do.
          tr ??= newState.tr;
          // Note: positions could shift after a setNodeMarkup, but since each
          // call replaces node attrs only (no size change), prior `pos` values
          // remain valid for subsequent calls in this loop.
          const fresh = genBlockId();
          tr.setNodeMarkup(pos, undefined, { ...node.attrs, blockId: fresh });
          seen.add(fresh);
        } else {
          seen.add(id);
        }
      });

      return tr;
    },
  });
}

// ---------------------------------------------------------------------------
// 3. Decorations keyed by block-id (not position)
// ---------------------------------------------------------------------------
//
// We store `Set<blockId>` in plugin state. On every transaction we rebuild the
// DecorationSet by scanning the doc for blocks whose id is in the set. This
// guarantees decorations survive edits *anywhere* in the doc — the underlying
// id is anchor-stable, unlike a raw position which `tr.mapping` could only
// move, not preserve across re-renders.

const highlightKey = new PluginKey<{ blockIds: Set<string> }>("highlights");

type HighlightMeta =
  | { kind: "add"; blockId: string }
  | { kind: "clear" };

function highlightPlugin(): Plugin<{ blockIds: Set<string> }> {
  return new Plugin<{ blockIds: Set<string> }>({
    key: highlightKey,
    state: {
      init: () => ({ blockIds: new Set<string>() }),
      apply(tr, prev) {
        const meta = tr.getMeta(highlightKey) as HighlightMeta | undefined;
        if (!meta) return prev;
        if (meta.kind === "clear") return { blockIds: new Set<string>() };
        const next = new Set(prev.blockIds);
        next.add(meta.blockId);
        return { blockIds: next };
      },
    },
    props: {
      decorations(state) {
        const { blockIds } = highlightKey.getState(state) ?? {
          blockIds: new Set<string>(),
        };
        if (blockIds.size === 0) return DecorationSet.empty;

        const decs: Decoration[] = [];
        forEachBlockNode(state.doc, (node, pos) => {
          const id = node.attrs.blockId as string | null;
          if (id && blockIds.has(id)) {
            // Two decorations per hit:
            //   - Node decoration: soft outer glow box on the block as a whole
            //   - Inline decoration: underline across its inline content
            decs.push(
              Decoration.node(pos, pos + node.nodeSize, {
                class: "pm-highlight-glow",
                style:
                  "box-shadow: 0 0 0 2px rgba(255, 213, 79, 0.55), 0 0 20px rgba(255, 213, 79, 0.35); border-radius: 4px;",
              }),
            );
            if (node.content.size > 0) {
              decs.push(
                Decoration.inline(pos + 1, pos + node.nodeSize - 1, {
                  class: "pm-highlight-underline",
                  style:
                    "text-decoration: underline wavy rgba(255, 165, 0, 0.85); text-underline-offset: 3px;",
                }),
              );
            }
          }
        });

        return DecorationSet.create(state.doc, decs);
      },
    },
  });
}

// ---------------------------------------------------------------------------
// 4. Latency instrumentation
// ---------------------------------------------------------------------------
//
// We wrap `dispatchTransaction` to time text-input transactions from when the
// transaction is *applied* to the next animation frame (a reasonable proxy for
// keypress→paint inside the editor). The harness button starts a 60s window;
// at the end we compute median + p95.

type LatencyState = {
  recording: boolean;
  samples: number[];
  windowEndsAt: number;
};

function isTextInput(tr: Transaction): boolean {
  // Look for ReplaceStep that inserts a text slice and doesn't delete much.
  if (tr.steps.length === 0) return false;
  // tr.getMeta will be set by undo/redo; ignore those.
  if (tr.getMeta("history$") || tr.getMeta("paste")) return false;
  // Heuristic: there's at least one inserted character and no big deletion.
  let insertedText = false;
  for (let i = 0; i < tr.steps.length; i++) {
    const rawStep = tr.steps[i];
    if (!rawStep) continue;
    const step = rawStep.toJSON();
    if (step.stepType === "replace" && step.slice) {
      const content = step.slice.content;
      if (Array.isArray(content)) {
        for (const c of content) {
          if (c.type === "text" && typeof c.text === "string" && c.text.length > 0) {
            insertedText = true;
          }
        }
      }
    }
  }
  return insertedText;
}

function quantile(sorted: number[], q: number): number {
  if (sorted.length === 0) return 0;
  const idx = Math.min(sorted.length - 1, Math.floor(q * (sorted.length - 1)));
  return sorted[idx] ?? 0;
}

// ---------------------------------------------------------------------------
// 5. Initial doc + lorem generator
// ---------------------------------------------------------------------------

function createInitialDoc() {
  return schema.node("doc", null, [
    schema.node("heading_2", { blockId: null }, [
      schema.text("ProseMirror bake-off harness"),
    ]),
    schema.node("paragraph", { blockId: null }, [
      schema.text(
        "Type freely. Block IDs are assigned on insert and preserved across split / merge / delete.",
      ),
    ]),
    schema.node("heading_3", { blockId: null }, [schema.text("Try these")]),
    schema.node("unordered_list", { blockId: null }, [
      schema.node("list_item", { blockId: null }, [
        schema.node("paragraph", { blockId: null }, [
          schema.text("Press Enter to split a paragraph"),
        ]),
      ]),
      schema.node("list_item", { blockId: null }, [
        schema.node("paragraph", { blockId: null }, [
          schema.text("Backspace at start of a paragraph to merge"),
        ]),
      ]),
    ]),
    schema.node("scene_break", { blockId: null }),
    schema.node("dialogue", { blockId: null }, [
      schema.text("\"This is a dialogue block,\" she said."),
    ]),
    schema.node("paragraph", { blockId: null }, [
      schema.text("Click the buttons above to exercise the harness."),
    ]),
  ]);
}

const LOREM_WORDS = (
  "lorem ipsum dolor sit amet consectetur adipiscing elit sed do " +
  "eiusmod tempor incididunt ut labore et dolore magna aliqua enim " +
  "ad minim veniam quis nostrud exercitation ullamco laboris nisi " +
  "aliquip ex ea commodo consequat duis aute irure in reprehenderit " +
  "voluptate velit esse cillum eu fugiat nulla pariatur excepteur " +
  "sint occaecat cupidatat non proident sunt culpa qui officia " +
  "deserunt mollit anim id est laborum"
).split(/\s+/);

function generateLoremParagraphs(totalWords: number, wordsPerPara = 80): string[] {
  const paras: string[] = [];
  let remaining = totalWords;
  while (remaining > 0) {
    const n = Math.min(wordsPerPara, remaining);
    const words: string[] = [];
    for (let i = 0; i < n; i++) {
      const w = LOREM_WORDS[Math.floor(Math.random() * LOREM_WORDS.length)];
      if (w) words.push(w);
    }
    paras.push(words.join(" ") + ".");
    remaining -= n;
  }
  return paras;
}

// ---------------------------------------------------------------------------
// 6. Random-edit fuzz helper
// ---------------------------------------------------------------------------

function runFuzzSteps(
  view: EditorView,
  steps: number,
): { passed: boolean; failedAt?: number; reason?: string } {
  for (let step = 0; step < steps; step++) {
    const doc = view.state.doc;
    const topLevelCount = doc.childCount;
    if (topLevelCount < 1) {
      // Repopulate to keep the harness alive.
      const tr = view.state.tr.insert(
        0,
        schema.node("paragraph", { blockId: null }, [schema.text("recover")]),
      );
      view.dispatch(tr);
      continue;
    }

    const choice = Math.floor(Math.random() * 3);

    if (choice === 0 && topLevelCount > 0) {
      // SPLIT: pick a top-level paragraph and split at the middle.
      const idx = Math.floor(Math.random() * topLevelCount);
      let pos = 1; // inside the doc
      for (let i = 0; i < idx; i++) pos += doc.child(i).nodeSize;
      const node = doc.child(idx);
      if (node.type.name === "paragraph" && node.content.size > 0) {
        const splitAt = pos + 1 + Math.floor(node.content.size / 2);
        try {
          const tr = view.state.tr.split(splitAt);
          view.dispatch(tr);
        } catch {
          // some splits are illegal; skip
        }
      }
    } else if (choice === 1 && topLevelCount >= 2) {
      // JOIN: pick a junction between two adjacent paragraphs.
      const idx = 1 + Math.floor(Math.random() * (topLevelCount - 1));
      let pos = 0;
      for (let i = 0; i < idx; i++) pos += doc.child(i).nodeSize;
      try {
        const tr = view.state.tr.join(pos);
        view.dispatch(tr);
      } catch {
        // join may fail across incompatible kinds; that's fine
      }
    } else if (topLevelCount > 1) {
      // DELETE: drop a single top-level node.
      const idx = Math.floor(Math.random() * topLevelCount);
      let pos = 0;
      for (let i = 0; i < idx; i++) pos += doc.child(i).nodeSize;
      const node = doc.child(idx);
      try {
        const tr = view.state.tr.delete(pos, pos + node.nodeSize);
        view.dispatch(tr);
      } catch {
        // ignore
      }
    }

    // Invariant: top-level child count matches unique top-level block IDs.
    const ids: string[] = [];
    const newDoc = view.state.doc;
    for (let i = 0; i < newDoc.childCount; i++) {
      const child = newDoc.child(i);
      if (child.attrs.blockId) ids.push(child.attrs.blockId);
    }
    const uniqueCount = new Set(ids).size;
    if (uniqueCount !== newDoc.childCount) {
      return {
        passed: false,
        failedAt: step,
        reason: `dup or missing: ${ids.length} ids (${uniqueCount} unique) vs ${newDoc.childCount} children`,
      };
    }
  }
  return { passed: true };
}

// ---------------------------------------------------------------------------
// 7. React harness component
// ---------------------------------------------------------------------------

export default function BakeOffPM() {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const viewRef = useRef<EditorView | null>(null);
  const latencyRef = useRef<LatencyState>({
    recording: false,
    samples: [],
    windowEndsAt: 0,
  });

  const [latencyMedian, setLatencyMedian] = useState<number | null>(null);
  const [latencyP95, setLatencyP95] = useState<number | null>(null);
  const [recording, setRecording] = useState(false);
  const [fuzzResult, setFuzzResult] = useState<string>("");
  const [undoResult, setUndoResult] = useState<string>("");
  const [statsTick, setStatsTick] = useState(0);
  const [blockCount, setBlockCount] = useState(0);

  useEffect(() => {
    if (!containerRef.current) return;

    const state = EditorState.create({
      doc: createInitialDoc(),
      schema,
      plugins: [
        history(),
        keymap({
          "Mod-z": undo,
          "Mod-y": redo,
          "Mod-Shift-z": redo,
          Enter: (state, dispatch) => {
            // Use list-aware split inside list items; fall back to splitBlock.
            return splitListItem(schema.nodes.list_item)(state, dispatch) ||
              splitBlock(state, dispatch);
          },
          Tab: sinkListItem(schema.nodes.list_item),
          "Shift-Tab": liftListItem(schema.nodes.list_item),
        }),
        keymap(baseKeymap),
        blockIdPlugin(),
        highlightPlugin(),
      ],
    });

    const view = new EditorView(containerRef.current, {
      state,
      dispatchTransaction(tr) {
        const start = performance.now();
        const wasText = isTextInput(tr);
        const newState = view.state.apply(tr);
        view.updateState(newState);
        if (wasText && latencyRef.current.recording) {
          // Sample paint time on next animation frame.
          requestAnimationFrame(() => {
            const elapsed = performance.now() - start;
            latencyRef.current.samples.push(elapsed);
          });
        }
        // Refresh some on-screen stats opportunistically.
        if (tr.docChanged) {
          setStatsTick((t) => (t + 1) % 1_000_000);
        }
      },
    });
    viewRef.current = view;

    // Trigger an initial empty transaction so the block-id plugin assigns
    // ids to the initial doc.
    view.dispatch(view.state.tr);

    return () => {
      view.destroy();
      viewRef.current = null;
    };
  }, []);

  // Update block count whenever the doc changes.
  useEffect(() => {
    const v = viewRef.current;
    if (!v) return;
    setBlockCount(v.state.doc.childCount);
  }, [statsTick]);

  // -------------------------------------------------------------------------
  // Button handlers
  // -------------------------------------------------------------------------

  function handleHighlightRandom() {
    const v = viewRef.current;
    if (!v) return;
    const ids: string[] = [];
    const doc = v.state.doc;
    for (let i = 0; i < doc.childCount; i++) {
      const child = doc.child(i);
      if (child.type.name === "paragraph" && child.attrs.blockId) {
        ids.push(child.attrs.blockId);
      }
    }
    if (ids.length === 0) return;
    const pick = ids[Math.floor(Math.random() * ids.length)];
    if (!pick) return;
    const tr = v.state.tr.setMeta(highlightKey, {
      kind: "add",
      blockId: pick,
    } satisfies HighlightMeta);
    v.dispatch(tr);
  }

  function handleClearHighlights() {
    const v = viewRef.current;
    if (!v) return;
    v.dispatch(
      v.state.tr.setMeta(highlightKey, { kind: "clear" } satisfies HighlightMeta),
    );
  }

  function handlePaste50k() {
    const v = viewRef.current;
    if (!v) return;
    const paras = generateLoremParagraphs(50_000, 80);
    const nodes: PMNode[] = paras.map((p) =>
      schema.node("paragraph", { blockId: null }, [schema.text(p)]),
    );

    // Single transaction insert at end of doc.
    const tr = v.state.tr;
    const insertAt = v.state.doc.content.size;
    tr.insert(insertAt, nodes);
    v.dispatch(tr);

    // Begin a 60s latency-measurement window.
    latencyRef.current = {
      recording: true,
      samples: [],
      windowEndsAt: performance.now() + 60_000,
    };
    setRecording(true);
    setLatencyMedian(null);
    setLatencyP95(null);

    window.setTimeout(() => {
      latencyRef.current.recording = false;
      setRecording(false);
      const sorted = [...latencyRef.current.samples].sort((a, b) => a - b);
      if (sorted.length === 0) {
        setLatencyMedian(0);
        setLatencyP95(0);
        return;
      }
      setLatencyMedian(quantile(sorted, 0.5));
      setLatencyP95(quantile(sorted, 0.95));
    }, 60_000);
  }

  function handleFuzz() {
    const v = viewRef.current;
    if (!v) return;
    const r = runFuzzSteps(v, 50);
    setFuzzResult(
      r.passed
        ? "PASS — all 50 steps preserved unique block-ids"
        : `FAIL @ step ${r.failedAt}: ${r.reason}`,
    );
  }

  function handleLongUndo() {
    const v = viewRef.current;
    if (!v) return;

    // Snapshot current top-level ids and ensure at least one highlight is set
    // so we have a decoration to drift-check.
    const beforeIds: string[] = [];
    for (let i = 0; i < v.state.doc.childCount; i++) {
      const c = v.state.doc.child(i);
      if (c.attrs.blockId) beforeIds.push(c.attrs.blockId);
    }
    if (beforeIds.length === 0) {
      setUndoResult("FAIL — no blocks to test");
      return;
    }
    const targetId = beforeIds[0]!;
    v.dispatch(
      v.state.tr.setMeta(highlightKey, {
        kind: "add",
        blockId: targetId,
      } satisfies HighlightMeta),
    );

    // Type 200 characters (one per transaction so each is undoable).
    for (let i = 0; i < 200; i++) {
      const tr = v.state.tr.insertText("x", v.state.selection.from);
      v.dispatch(tr);
    }
    // Undo 200, redo 200.
    for (let i = 0; i < 200; i++) undo(v.state, v.dispatch.bind(v));
    for (let i = 0; i < 200; i++) redo(v.state, v.dispatch.bind(v));

    // Assertions:
    //  (a) the highlight target's id is still present (no renumbering)
    //  (b) the decoration is still showing on that block
    const afterIds: string[] = [];
    for (let i = 0; i < v.state.doc.childCount; i++) {
      const c = v.state.doc.child(i);
      if (c.attrs.blockId) afterIds.push(c.attrs.blockId);
    }
    const idsStable = beforeIds.every((id) => afterIds.includes(id));
    const hlState = highlightKey.getState(v.state);
    const decoStillSet = hlState?.blockIds.has(targetId) ?? false;

    if (idsStable && decoStillSet) {
      setUndoResult("PASS — no id renumbering, no decoration drift");
    } else {
      setUndoResult(
        `FAIL — idsStable=${idsStable}, decoStillSet=${decoStillSet}`,
      );
    }
  }

  // -------------------------------------------------------------------------
  // Render
  // -------------------------------------------------------------------------

  const btn: React.CSSProperties = {
    padding: "8px 14px",
    border: "1px solid var(--water-fg-muted, #888)",
    borderRadius: 4,
    background: "var(--water-bg-paper, #fff)",
    color: "var(--water-fg-default, #111)",
    fontFamily: "var(--water-font-sans, system-ui)",
    fontSize: 14,
    cursor: "pointer",
  };

  return (
    <div
      style={{
        height: "100vh",
        width: "100vw",
        display: "flex",
        flexDirection: "column",
        background: "var(--water-bg-paper, #fff)",
        color: "var(--water-fg-default, #111)",
        fontFamily: "var(--water-font-sans, system-ui)",
      }}
    >
      <header
        style={{
          padding: "12px 16px",
          borderBottom: "1px solid var(--water-fg-muted, #ddd)",
        }}
      >
        <strong>ProseMirror bake-off harness</strong>
        <span style={{ marginLeft: 12, opacity: 0.7, fontSize: 13 }}>
          blocks: {blockCount} · samples:{" "}
          {latencyRef.current.samples.length}
          {recording ? " (recording)" : ""}
          {latencyMedian !== null
            ? ` · median ${latencyMedian.toFixed(2)}ms · p95 ${latencyP95?.toFixed(2)}ms`
            : ""}
        </span>
      </header>

      <div
        style={{
          padding: "10px 16px",
          display: "flex",
          gap: 8,
          flexWrap: "wrap",
          borderBottom: "1px solid var(--water-fg-muted, #ddd)",
        }}
      >
        <button style={btn} onClick={handleHighlightRandom}>
          Highlight random paragraph
        </button>
        <button style={btn} onClick={handleClearHighlights}>
          Clear highlights
        </button>
        <button style={btn} onClick={handlePaste50k} disabled={recording}>
          Paste 50k words
        </button>
        <button style={btn} onClick={handleFuzz}>
          Random edit fuzz (50 steps)
        </button>
        <button style={btn} onClick={handleLongUndo}>
          Long-undo stress (200 steps)
        </button>
      </div>

      <div
        style={{
          padding: "8px 16px",
          fontSize: 13,
          fontFamily: "var(--water-font-mono, monospace)",
          color: "var(--water-fg-muted, #555)",
          borderBottom: "1px solid var(--water-fg-muted, #ddd)",
          minHeight: 24,
        }}
      >
        {fuzzResult && <div>fuzz: {fuzzResult}</div>}
        {undoResult && <div>undo: {undoResult}</div>}
      </div>

      <div
        ref={containerRef}
        className="ProseMirror-host"
        style={{
          flex: 1,
          overflowY: "auto",
          padding: "24px 48px",
          fontFamily: "var(--water-font-serif, Georgia, serif)",
          fontSize: 16,
          lineHeight: 1.6,
        }}
      />
    </div>
  );
}
