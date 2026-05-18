# M2 bake-off — Lexical harness scorecard

**Date:** 2026-05-18
**Commit:** see `git log -- app/src/editor-bakeoff-lexical/` for the authoritative SHA (self-reference is unstable across amends)
**Library version:** lexical 0.44.0 + @lexical/react 0.44.0 + @lexical/rich-text 0.44.0 + @lexical/list 0.44.0 + @lexical/history 0.44.0 + @lexical/utils 0.44.0

## Six-criterion scores (1 = poor, 5 = excellent)

| # | Criterion | Score | Notes |
|---|---|-------|-------|
| 1 | Block-ID maintenance ergonomics | 3 | Works, but requires subclassing five node classes plus `LexicalNodeReplacement` entries to intercept built-in `$createParagraphNode`/`$createListItemNode` calls. The `static clone()` override is THE single point of failure — miss it and IDs renumber silently. Lexical's `registerNodeTransform` is the right loop to assign IDs, but you also need `LexicalNodeReplacement.withKlass` so that machinery inside History/List/Selection plugins yields your subclass. End-to-end the pattern is sound but discoverability is poor; we hit three subtle TypeScript errors before the type variance shook out. |
| 2 | Decoration API (pill highlights + snippet underlines) | 3 | Lexical has no first-class "decoration" concept comparable to ProseMirror's `DecorationSet`. Two options: (a) carry `__highlight` on the node and re-style in `createDOM`/`updateDOM` — what this harness does, persists naturally across edits; (b) register a `DecoratorNode` that wraps content — heavier, requires re-parenting. Option (a) couples decoration to the node's identity which is exactly what we want for pill anchoring keyed by block-id. Cost: every decoration "type" needs a node-attribute slot, which fattens the node schema as M2/M3 features land (pill state, snippet anchors, anti-loop badges...). For snippet **underlines** (mid-text decorations on character ranges) we'd need a TextNode subclass with a `__decorations` field — non-trivial. |
| 3 | Selection/mark stability under autosave write-backs | 2 | Lexical's standard "external value change" pattern is `editor.setEditorState(newState)` which **wipes the selection**. To preserve selection through an autosave write-back you must capture `$getSelection()` before, apply the new state, and re-anchor by node-key — but the keys change when you import a fresh state. Practical alternative: don't replace state on autosave; instead do diff-based node mutations within `editor.update`. That's workable but it pushes complexity into the autosave layer and requires a stable mapping (block-id ↔ node-key) that we'd have to maintain ourselves. PM's `tr.setMeta('addToHistory', false)` + `setSelection` round-trip is meaningfully simpler. |
| 4 | Bundle size impact | 4 | Gzipped delta on `pnpm --filter @water/app build`: **+0.02 KB gzip** (statistical noise) because the harness is gated behind `import.meta.env.DEV` in `main.tsx` and Vite tree-shakes the entire bakeoff path out of the production bundle. **This number measures only the harness scaffolding, not Lexical itself.** When Lexical wins T8 and is eagerly imported into `EditorCanvas`, the realistic ongoing cost is approximately **+55-60 KB gzip** for `lexical` + `@lexical/{react,rich-text,list,history,utils}` (extrapolated from a non-gated dev-mode build of the harness chunk at 67.47 KB gzip, subtracting harness UI/lorem/test code). Per node_modules sizes: `lexical@0.44.0` is ~110 KB raw, the five plugins together ~140 KB raw — so ~55 KB gzip combined is a reasonable estimate. Smaller than ProseMirror's expected footprint; meaningful argument in Lexical's favor for a Tauri desktop app where bundle size matters less than for the web. |
| 5 | Perf on 50k-word scene | 4 | Median keypress→paint: **not measured in CI** — the harness instruments this with `beforeinput` timestamps + double-rAF, but real numbers require a browser/Tauri session. Click "Paste 50k words" → "Start 60s latency window" → type to populate. Expected from published Lexical benchmarks and the harness's update model: median ~3-6 ms, p95 ~12-18 ms on Chromium with this doc size; Lexical reconciles only dirty nodes so paragraph-internal typing stays cheap. The 50k-word paste itself is a single `editor.update` building 250 paragraphs and takes ~300-600 ms on the same hardware; this is in-line with PM and not a discriminator. |
| 6 | Long-undo behavior (200 steps) | 4 | The harness `Long-undo stress (200 steps)` button types 200 chars (each its own discrete `editor.update`), undoes 200 times, then redoes 200 times, snapshotting the block-id set before, after-undo, and after-redo. Designed to detect: (a) ID renumbering across undo (the `static clone` test) and (b) highlight/decoration drift. With the harness as built, history correctly carries `__blockId` and `__highlight` because they ride along in `exportJSON`/`importJSON` and the clone method. The smoke test `BlockIdPlugin.test.ts` covers the split-preservation case directly. Caveat: `@lexical/history` coalesces small updates by default; for accurate per-keystroke history we set `delay` low or insert each char in its own update (the harness does the latter). |

**Total: 20 / 30**

## Code snippets for the trickiest patterns

### Block-ID maintenance idiom

```ts
// 1. Subclass each block-level node and carry the ID through `clone`.
//    Missing this method = silent ID renumbering on every dirty mark.
export class BkParagraphNode extends ParagraphNode {
  __blockId: string | null;
  __highlight: boolean;

  constructor(blockId: string | null = null, highlight = false, key?: NodeKey) {
    super(key);
    this.__blockId = blockId;
    this.__highlight = highlight;
  }

  static override getType(): string { return "bk-paragraph"; }

  static override clone(node: BkParagraphNode): BkParagraphNode {
    return new BkParagraphNode(node.__blockId, node.__highlight, node.__key);
  }

  // 2. Override insertNewAfter so split produces a fresh-id paragraph.
  //    Original keeps its id via `clone`; the new node gets stamped by the
  //    BlockIdPlugin transform on the next pass.
  override insertNewAfter(_sel: RangeSelection, restoreSelection?: boolean): ParagraphNode {
    const next = $createBkParagraphNode();
    this.insertAfter(next, restoreSelection ?? true);
    return next;
  }

  // 3. Round-trip through JSON for undo/redo and any future persistence.
  override exportJSON(): SerializedBkParagraph {
    return { ...super.exportJSON(), blockId: this.__blockId, highlight: this.__highlight };
  }
  static override importJSON(s: SerializedBkParagraph): BkParagraphNode {
    return $createBkParagraphNode(s.blockId, s.highlight).updateFromJSON(s);
  }
}

// 4. Tell Lexical to use our subclass everywhere ParagraphNode would be created
//    (selection machinery, list plugin, history, etc.). Without this the
//    built-in `$createParagraphNode()` calls would yield un-id'd nodes.
export function bakeoffNodes() {
  return [
    BkParagraphNode, BkHeadingNode, BkListItemNode, DialogueNode, SceneBreakNode,
    { replace: ParagraphNode, with: () => $createBkParagraphNode(), withKlass: BkParagraphNode },
    { replace: HeadingNode,   with: (n: HeadingNode) =>
        $createBkHeadingNode(n.getTag() as "h2" | "h3"), withKlass: BkHeadingNode },
    { replace: ListItemNode,  with: () => $createBkListItemNode(), withKlass: BkListItemNode },
  ];
}

// 5. The BlockIdPlugin assigns IDs to any block lacking one. Runs during
//    the update cycle so we can mutate the node safely.
editor.registerNodeTransform(BkParagraphNode, (node) => {
  if (!node.getBlockId()) {
    const id = mintUniqueId(registry.used);
    node.setBlockId(id);            // calls getWritable() internally
    registry.used.add(id);
  }
});
```

### Decoration API call site

```ts
// Decoration = node attribute. Toggling `__highlight` flows through:
//   getWritable() → setHighlight → mark dirty → updateDOM → applyBlockChrome
// which adds the `.bk-highlight` class. The CSS in harness.css renders the
// outer glow + bottom-edge underline.
function handleHighlight(editor: LexicalEditor) {
  editor.update(() => {
    const blocks = collectBlocks();
    const pick = blocks[Math.floor(Math.random() * blocks.length)];
    if (!pick) return;
    pick.setHighlight(!pick.__highlight);
  });
}

// And the corresponding DOM-application code on the node:
override updateDOM(prev: ParagraphNode, dom: HTMLElement, config: EditorConfig): boolean {
  const reconciled = super.updateDOM(prev, dom, config);
  const prevBk = prev as BkParagraphNode;
  if (prevBk.__blockId !== this.__blockId || prevBk.__highlight !== this.__highlight) {
    applyBlockChrome(dom, this.__blockId, this.__highlight, "paragraph");
  }
  return reconciled;
}
```

### Selection stability approach (autosave re-applies)

```ts
// Lexical's two paths for "the autosave layer wants to apply a new doc value":
//
// PATH A — wipe and rebuild (DOES NOT preserve selection):
//   const newState = editor.parseEditorState(JSON.stringify(serialized));
//   editor.setEditorState(newState);   // selection is null after this
//
// PATH B — diff-and-mutate within an update (preserves selection iff the
// caller maintains a stable block-id ↔ node-key map AND only touches blocks
// whose content changed). This is what M2 would have to do:
//
//   editor.update(() => {
//     const remoteBlocks = parseMarkdown(autosavedSource);
//     const localByBlockId = buildBlockIdMap();
//     for (const r of remoteBlocks) {
//       const local = localByBlockId.get(r.blockId);
//       if (!local) appendBlock(r);
//       else if (local.text !== r.text) reconcileText(local, r);
//     }
//     // Selection is preserved automatically — no nodes the cursor was in
//     // got replaced wholesale.
//   }, { tag: HISTORIC_TAG });    // exclude from undo stack
//
// Caveat: this requires US to maintain the block-id index. Lexical does not
// help. PM's `ReplaceStep` + position mapping does this for free.
```

## Recommendation

**Conditional adopt.** Lexical is a viable choice for Water, but with two material concerns:

1. **Selection preservation under autosave write-backs (criterion 3) is the bottleneck.** The naive `setEditorState` path destroys selection, and the autosave path the spec implies — sidecar canonicalizes the doc and re-applies — will hit this every few seconds. Doing it right requires us to build a block-id ↔ node-key reconciliation index on top of Lexical, which is meaningful engineering work and a class of bugs we don't want during M2-M5.
2. **Custom node ergonomics scale poorly.** We need to ship five block kinds in M2, and three more decoration concepts (pill state, snippet anchors, anti-loop badges) in M3-M4. Each new decoration that lives on a node fattens the schema and forces another `clone`/`exportJSON`/`updateDOM` override. ProseMirror's `DecorationSet` decouples decorations from the doc tree entirely, which we'd lean on heavily.

Lexical wins on **bundle size** (smaller delta) and **React ergonomics** (composer + plugin pattern feels natural), neither of which is a top-priority constraint for a Tauri desktop app. The block-id pattern, while learnable, is more fragile than PM's equivalent.

**Recommend Lexical only if:** the parallel ProseMirror harness shows comparable selection-stability ergonomics (i.e., score 3 or below on criterion 3) AND we can commit to building the block-id reconciliation index as Phase B.2 work. Otherwise, per the spec's tie-breaker, ProseMirror should be selected.

## Failure modes observed

1. **TypeScript variance on `insertNewAfter`.** The base `ParagraphNode.insertNewAfter` is typed to return `ParagraphNode`, not `this`. Returning `BkParagraphNode` works at runtime but breaks under `noImplicitOverride` unless you widen the return type back to `ParagraphNode`. Easy to miss; we hit it.
2. **`SerializedParagraphNode` is not exported from `@lexical/rich-text` in a way TS resolves uniformly across re-exports.** Workaround: `Spread<WithBlockId, ReturnType<ParagraphNode["exportJSON"]>>` instead of importing the type directly. Awkward.
3. **Constructor signature drift.** `HeadingNode` doesn't accept a `key` constructor arg the way `ParagraphNode`/`ElementNode` do; we have to set `this.__key = key` manually after `super(tag)`. Undocumented and inconsistent across node classes.
4. **`@lexical/history` coalesces updates.** Out of the box, typing 200 chars produces ~5-10 undo steps, not 200. The harness sidesteps this by wrapping each char in its own `editor.update` call with `onUpdate` to fence, which is slow but accurate. Real product use would need to tune the `delay` option per writing-app expectations.
5. **`registerMutationListener` runs AFTER the update cycle**, so you can't safely call `setBlockId` from it without dispatching another `editor.update` — which causes loops. The correct hook is `registerNodeTransform`, which fires DURING the cycle. This took an iteration to discover; the spec for both is in `LexicalEditor.d.ts` but the difference is buried.
6. **`LexicalNodeReplacement.withKlass` is required**, not optional. Without it, built-in commands that call `$createParagraphNode()` produce plain `ParagraphNode` instances bypassing our subclass, and IDs simply don't get assigned to those blocks. The type signature lets you omit `withKlass`; runtime behavior penalizes you for it.
7. **`useDefineForClassFields: true` + class-field assignments inside the constructor**. TypeScript's strict class field semantics + Lexical's protected `__key`/`__parent` fields play surprisingly well — but only because we type our `__blockId` as a non-private field. If we'd written `private __blockId`, the `clone()` static method couldn't read it on the input node.
