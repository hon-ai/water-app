/**
 * Phase 3.5 — ProseMirror inline-decoration plugin for the pill
 * trigger-phrase highlight. The plugin holds a single optional
 * highlight specified in block-relative char offsets; on each
 * dispatched meta it walks the doc to find the matching block
 * (by `blockId` attr) and converts the block-relative range to
 * absolute doc positions for the `Decoration.inline` it returns.
 *
 * The plugin doesn't drive its own hover logic — that belongs to
 * `PillLayer`, which consults `anchorResolver` and dispatches via
 * `setTriggerHighlight`.
 *
 * On `tr.docChanged` without a meta, decorations are mapped through
 * the transaction's position mapping so concurrent edits don't
 * desync the highlight. Setting a fresh meta re-locates from the
 * current doc, which is the authoritative path after the resolver
 * has re-computed against the latest text.
 */

import { Plugin, PluginKey } from "prosemirror-state";
import type { EditorState } from "prosemirror-state";
import type { EditorView } from "prosemirror-view";
import { Decoration, DecorationSet } from "prosemirror-view";

export interface TriggerHighlight {
  /** Block-id (the `blockId` attr; rendered to DOM as `data-bid`). */
  blockId: string;
  /** Char offset within the block's textContent. Inclusive. */
  start: number;
  /** Char offset within the block's textContent. Exclusive. */
  end: number;
}

export const triggerHighlightKey = new PluginKey<DecorationSet>(
  "water-trigger-highlight",
);

const META = "water-trigger-highlight/set";

/**
 * Locate the textblock with the matching `blockId` and return an
 * inline Decoration covering [start, end) inside it. Returns the
 * empty set when no block matches or the requested range is empty.
 */
function decorate(
  state: EditorState,
  h: TriggerHighlight,
): DecorationSet {
  let found: DecorationSet = DecorationSet.empty;
  state.doc.descendants((node, pos) => {
    const id = (node.attrs?.["blockId"] ?? "") as string;
    if (!node.isTextblock) return true;
    if (id !== h.blockId) return true;
    const textLen = node.content.size;
    const s = Math.max(0, Math.min(h.start, textLen));
    const e = Math.max(s, Math.min(h.end, textLen));
    if (e <= s) {
      found = DecorationSet.empty;
      return false;
    }
    // `pos` is the position before the node; the textblock's content
    // starts at `pos + 1` (one past the opening token).
    const absStart = pos + 1 + s;
    const absEnd = pos + 1 + e;
    found = DecorationSet.create(state.doc, [
      Decoration.inline(absStart, absEnd, {
        class: "water-trigger-highlight",
      }),
    ]);
    return false;
  });
  return found;
}

/**
 * The plugin. Add to the editor's plugin list alongside
 * `blockIdPlugin()` and friends.
 */
export function triggerHighlightPlugin(): Plugin<DecorationSet> {
  return new Plugin<DecorationSet>({
    key: triggerHighlightKey,
    state: {
      init: () => DecorationSet.empty,
      apply(tr, old, _oldState, newState) {
        // `null` meta clears; an object meta sets to a new highlight;
        // no meta keeps the current set, mapped through any doc change.
        if (tr.getMeta(META) === null) return DecorationSet.empty;
        const next = tr.getMeta(META) as TriggerHighlight | undefined;
        if (next) return decorate(newState, next);
        if (tr.docChanged) return old.map(tr.mapping, tr.doc);
        return old;
      },
    },
    props: {
      decorations(state) {
        return this.getState(state);
      },
    },
  });
}

/**
 * Dispatch a meta-only transaction that updates the highlight. Pass
 * `null` to clear. Safe to call with a destroyed view (no-ops).
 */
export function setTriggerHighlight(
  view: EditorView | null,
  highlight: TriggerHighlight | null,
): void {
  if (!view) return;
  // Guard against post-destroy calls (the SelectionToolbar pattern).
  // ProseMirror views set `.docView` to null on destroy.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  if ((view as any).docView === null) return;
  view.dispatch(view.state.tr.setMeta(META, highlight));
}
