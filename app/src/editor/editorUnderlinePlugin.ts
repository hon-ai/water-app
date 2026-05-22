/**
 * Phase 5 — ProseMirror plugin that paints dotted underlines on
 * editor-pill anchor ranges (UX_SPEC §E.4).
 *
 * Shape mirrors `triggerHighlightPlugin` (Phase 3.5):
 *   - Plugin state is a `DecorationSet`.
 *   - A single transaction-meta key (`water-editor-underline/set`)
 *     accepts the full anchor list and replaces the set in one go.
 *   - Between sets, the plugin maps decorations through any
 *     `tr.docChanged` mapping so live edits don't desync the marks.
 *
 * Per-anchor severity ("observation" / "suggestion" / "warning")
 * lands on the decoration as a CSS class so the underline hue can
 * differ per rule kind. The class names match the CSS in
 * `app/src/styles/editor.css`.
 */

import { Plugin, PluginKey } from "prosemirror-state";
import type { EditorState } from "prosemirror-state";
import type { EditorView } from "prosemirror-view";
import { Decoration, DecorationSet } from "prosemirror-view";

export interface UnderlineAnchor {
  /** Pill id — written to `data-editor-pill-id` so click handlers
   *  on the rendered span can map back to the row. */
  pillId: string;
  /** Block-id the underline lives in (PM attr `blockId`). */
  blockId: string;
  /** Char offset of the underline start, inside the block. */
  start: number;
  /** Char offset of the underline end (exclusive), inside the block. */
  end: number;
  /** Severity bucket — drives the underline hue via CSS class. */
  severity: "observation" | "suggestion" | "warning";
}

export const editorUnderlineKey = new PluginKey<DecorationSet>(
  "water-editor-underline",
);

const META = "water-editor-underline/set";

/**
 * Walk the doc, find the textblock with `blockId === anchor.blockId`,
 * convert the block-relative [start, end) to absolute doc positions,
 * and emit one `Decoration.inline` per anchor. Anchors that point at
 * a missing block or zero-width range are silently dropped (the
 * resolver upstream is supposed to filter those out; this is a
 * defense-in-depth filter).
 */
function decorate(state: EditorState, anchors: UnderlineAnchor[]): DecorationSet {
  if (anchors.length === 0) return DecorationSet.empty;
  const byBlock = new Map<string, UnderlineAnchor[]>();
  for (const a of anchors) {
    const arr = byBlock.get(a.blockId) ?? [];
    arr.push(a);
    byBlock.set(a.blockId, arr);
  }
  const decorations: Decoration[] = [];
  state.doc.descendants((node, pos) => {
    if (!node.isTextblock) return true;
    const id = (node.attrs?.["blockId"] ?? "") as string;
    const matched = byBlock.get(id);
    if (!matched) return true;
    const textLen = node.content.size;
    for (const a of matched) {
      const s = Math.max(0, Math.min(a.start, textLen));
      const e = Math.max(s, Math.min(a.end, textLen));
      if (e <= s) continue;
      decorations.push(
        Decoration.inline(pos + 1 + s, pos + 1 + e, {
          class: `water-editor-underline water-editor-underline-${a.severity}`,
          "data-editor-pill-id": a.pillId,
        }),
      );
    }
    return false;
  });
  return DecorationSet.create(state.doc, decorations);
}

/**
 * The plugin. Mount alongside `triggerHighlightPlugin` in the
 * editor's plugin list.
 */
export function editorUnderlinePlugin(): Plugin<DecorationSet> {
  return new Plugin<DecorationSet>({
    key: editorUnderlineKey,
    state: {
      init: () => DecorationSet.empty,
      apply(tr, old, _oldState, newState) {
        if (tr.getMeta(META) === null) return DecorationSet.empty;
        const next = tr.getMeta(META) as UnderlineAnchor[] | undefined;
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
 * Dispatch a meta-only transaction that replaces the underline set.
 * Pass `null` to clear. Safe to call against a destroyed view.
 */
export function setEditorUnderlines(
  view: EditorView | null,
  anchors: UnderlineAnchor[] | null,
): void {
  if (!view) return;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  if ((view as any).docView === null) return;
  view.dispatch(view.state.tr.setMeta(META, anchors));
}
