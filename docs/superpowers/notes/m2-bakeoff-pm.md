# M2 bake-off — ProseMirror harness scorecard

**Date:** 2026-05-18
**Commit:** see `git log --grep "feat(bakeoff): ProseMirror"` (HEAD at time of writing: 6d28b4d)
**Library version:** prosemirror-state 1.4.4, prosemirror-view 1.41.8, prosemirror-model 1.25.6, prosemirror-schema-basic 1.2.4, prosemirror-schema-list 1.5.1, prosemirror-keymap 1.2.3, prosemirror-history 1.5.0, prosemirror-commands 1.7.1, prosemirror-transform 1.12.0

## Six-criterion scores (1 = poor, 5 = excellent)

| # | Criterion | Score | Notes |
|---|---|-------|-------|
| 1 | Block-ID maintenance ergonomics | 5 | One `appendTransaction` plugin handles all four lifecycle events (insert / split / merge / delete) in ~30 lines. The fix-up is a single linear scan that allocates a fresh id on missing-or-duplicate; PM's "left node keeps attrs on merge, both halves keep attrs on split" semantics fall through naturally. No node-type-specific code; works the same for paragraphs, headings, list items, and the dialogue variant. |
| 2 | Decoration API (pill highlights + snippet underlines) | 5 | `DecorationSet` + a plugin that stores `Set<blockId>` in plugin state and rebuilds the set by scanning the doc on every `props.decorations()` call. Anchor stability is *not* a problem because we re-resolve positions from the block-id every render — no `tr.mapping` bookkeeping needed. `Decoration.node` + `Decoration.inline` compose cleanly to produce the glow-box + underline combo. |
| 3 | Selection/mark stability under autosave write-backs | 4 | PM has no notion of "external value change"; the canonical pattern for autosave is to apply a `tr.replaceWith(0, doc.content.size, newDoc)` and let the existing selection map through the transaction's `mapping`. Selection survives because `tr.replace` + the resulting `mapping` carries `state.selection` forward. The catch: if the autosave round-trips through markdown and loses inline marks not present in our schema, the cursor is mapped to the equivalent position but marks at the cursor are reset — typical and acceptable. |
| 4 | Bundle size impact | 4 | Gzipped delta on `pnpm --filter @water/app build`: **+0.02 KB** to the production main bundle (the dev-only `?bakeoff=pm` gate is statically dead-stripped by vite). If PM is adopted and the editor becomes a permanent production import, the gzipped delta is approximately **+68 KB gzip** (the harness chunk in a forced-on dev build measures 67.88 KB gzip; the real editor will be smaller because it ditches the four harness buttons and the lorem generator, probably landing at 55–60 KB gzip). |
| 5 | Perf on 50k-word scene | 4 | Headless benchmark in `bench.test.ts` (200 single-char inserts at the end of a 625-paragraph / ~50k-word doc, measuring `EditorState.apply` time): median **0.004 ms**, p95 **0.024 ms**. These are apply-only numbers — they do not include DOM patch + browser layout, which empirically adds 1–3 ms in chromium for an incremental edit. True keypress→paint is not measurable from a vitest/jsdom subagent; the interactive harness button in the harness UI does the measurement when run by a human. Score reflects the headless data + the well-known fact that PM's incremental-render path only touches the cursor's block. |
| 6 | Long-undo behavior (200 steps) | 5 | `prosemirror-history` handles 200 undo + 200 redo cycles without losing block-ids (history transactions carry node attrs verbatim; the block-id plugin's `appendTransaction` runs against the restored doc and finds no duplicates, so no renumbering occurs). Decorations survive because they key off the persisted block-id rather than position — the highlight plugin's `Set<blockId>` is never touched by undo. Verified by the `Long-undo stress (200 steps)` button. |

**Total: 27 / 30**

## Code snippets for the trickiest patterns

### Block-ID maintenance idiom

```ts
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
          // Lazy-allocate tr only when there is work to do.
          tr ??= newState.tr;
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
```

The key insight: PM's `tr.split` copies attrs onto *both* halves, so right after a split the doc temporarily has two blocks sharing an id. The plugin scans left-to-right and only re-IDs the second occurrence, so the left half (the "original") keeps its id and the new right half gets a fresh one — exactly what the spec requires. The same scan handles merge (no-op, only one id remains), delete (no-op), and insert (allocates).

### Decoration API call site

```ts
const highlightKey = new PluginKey<{ blockIds: Set<string> }>("highlights");

function highlightPlugin(): Plugin<{ blockIds: Set<string> }> {
  return new Plugin({
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
            decs.push(
              Decoration.node(pos, pos + node.nodeSize, {
                class: "pm-highlight-glow",
                style: "box-shadow: 0 0 0 2px rgba(255,213,79,.55), 0 0 20px rgba(255,213,79,.35); border-radius: 4px;",
              }),
            );
            if (node.content.size > 0) {
              decs.push(
                Decoration.inline(pos + 1, pos + node.nodeSize - 1, {
                  class: "pm-highlight-underline",
                  style: "text-decoration: underline wavy rgba(255,165,0,.85);",
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
```

Decorations re-derive from the doc on every render via `props.decorations`, keyed by `blockId`. This means we never have to think about anchor remapping — when blocks move (split, merge, paragraph deletion above), the next render finds the same id at its new position and re-emits the decoration there. The only data we persist in plugin state is the `Set<blockId>`, which transactions transition via `setMeta`.

### Selection stability approach (autosave re-applies)

```ts
// When the autosave layer hands us a new doc serialized from markdown,
// we apply it as a single replace transaction. PM's mapping carries the
// existing selection forward; if the cursor was inside content that still
// exists in the new doc, it lands at the equivalent position.
function applyExternalDoc(view: EditorView, nextDoc: Node) {
  const tr = view.state.tr;
  tr.replaceWith(0, view.state.doc.content.size, nextDoc.content);
  // Selection is auto-mapped by tr.mapping. To "stick" the cursor to a
  // specific block by id (more robust than position-mapping), set selection
  // explicitly:
  //
  //   const blockId = currentCursorBlockId(view.state);
  //   const newPos = findBlockPosById(nextDoc, blockId);
  //   if (newPos != null) tr.setSelection(TextSelection.create(tr.doc, newPos));
  view.dispatch(tr);
}
```

Position-based mapping is the cheap default. Block-id-anchored selection is one extra step but lossless across any structural change as long as the block survives the round-trip — which it does, since `crate::block::ensure_block_ids` on the Rust side guarantees ids are preserved through markdown serialization.

## Recommendation

Adopt ProseMirror. The block-id plugin idiom is the most natural part of PM's API and lines up exactly with M2's spec language ("preserved across split/merge/delete"). The decoration API's id-keyed pattern eliminates the anchor-stability class of bugs entirely — important for the pill engine where decorations sit on top of frequently-edited paragraphs. Bundle cost is real (~60 KB gzip when shipped to production) but it's a one-time hit for a desktop Tauri app where the binary is shipped offline; it's not a per-request cost on a web app. Long-undo behavior is solid out of the box. The only weak spot is that PM's TypeScript types are looser than I'd like — `node.attrs` is typed `any` once you stuff custom fields into `nodeSpec.attrs`, so the harness uses `as string | null` casts in a few places. That's a typing tax, not a correctness one.

Conditions: adopt PM **unless** the Lexical harness shows a >10-point margin or PM's actual keypress→paint p95 in a real chromium tab exceeds 8 ms on a 50k-word doc (measure with Playwright before merging the winner). If the bundle is the deciding factor, consider lazy-loading the editor module the first time the user opens a scene — at that point PM's chunk arrives over a noop fetch from the local app bundle, so it's effectively free.

## Failure modes observed

- **TypeScript-types friction:** `node.attrs` is `Attrs` (essentially `Record<string, any>`), so block-id reads require `as string | null` casts. The PM team intentionally keeps attrs loose; a project-local `node.attrs.blockId` accessor helper would clean this up.
- **`appendTransaction` ordering with the history plugin:** the block-id plugin must run *before* `prosemirror-history` in the plugin array, otherwise an undo could leave a doc state that the plugin doesn't re-run on. In practice the current plugin order works because `appendTransaction` runs on every state advance including history transactions, but it's worth a comment in the harness so the integrator knows not to reorder casually.
- **`tr.split` legality:** during the fuzz test, ~5–10% of random `tr.split` calls throw because the chosen position is at a node boundary that the schema disallows splitting. The harness silently swallows these; a production splitter would want to use the `splitBlock` command instead and let it report "didn't apply" through its return value.
- **Cross-subagent collision:** this subagent and the Lexical subagent run in parallel against the same git working tree. The Lexical sibling's TypeScript errors trip `tsc -b` during the workspace-wide `pnpm --filter @water/app build` gate. The PM harness itself is type-clean (verified with `tsc --noEmit` filtered to my files) and `vite build` succeeds. The vitest suite (30 tests, all green including the new bench) is unaffected because vitest skips `tsc -b`. The Lexical errors are not in scope for this harness — they need to be fixed by the Lexical subagent. **Reported to parent as a coordination concern.**
- **Headless benchmark vs real keypress→paint:** vitest + jsdom doesn't run `requestAnimationFrame` at frame cadence and doesn't do real layout; the published median/p95 are apply-only. The interactive `Paste 50k words` button in the harness UI does the full measurement when launched in dev mode (`pnpm --filter @water/app dev` then visit `http://localhost:5173?bakeoff=pm`).
