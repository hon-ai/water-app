/**
 * Phase 5.6 — accept-suggestion splice helper.
 *
 * Finds the textblock with `blockId == anchor.blockId`, replaces the
 * block-relative range `[start, end)` with `replacement`, and
 * dispatches a single transaction. The editor's normal change flow
 * picks it up — autosave runs, the diagnostic engine re-fires, and
 * since the offending span is gone the editor pill won't re-surface.
 *
 * Returns `true` if the splice landed, `false` when the target
 * block (or a valid range within it) couldn't be located. Callers
 * use the return value to decide whether the row-dismiss IPC should
 * still fire — if the splice failed the row stays so the writer can
 * see it.
 */

import type { EditorView } from "prosemirror-view";

export interface AcceptAnchor {
  blockId: string;
  start: number;
  end: number;
  replacement: string;
}

export function acceptSuggestion(
  view: EditorView | null,
  anchor: AcceptAnchor,
): boolean {
  if (!view) return false;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  if ((view as any).docView === null) return false;
  const { state } = view;
  let absStart: number | null = null;
  let absEnd: number | null = null;
  state.doc.descendants((node, pos) => {
    if (absStart !== null) return false;
    if (!node.isTextblock) return true;
    const id = (node.attrs?.["blockId"] ?? "") as string;
    if (id !== anchor.blockId) return true;
    const textLen = node.content.size;
    const s = Math.max(0, Math.min(anchor.start, textLen));
    const e = Math.max(s, Math.min(anchor.end, textLen));
    if (e <= s) {
      // Empty range — caller's anchor is broken. Bail.
      return false;
    }
    absStart = pos + 1 + s;
    absEnd = pos + 1 + e;
    return false;
  });
  if (absStart === null || absEnd === null) return false;
  const replacementNode = view.state.schema.text(anchor.replacement);
  view.dispatch(view.state.tr.replaceWith(absStart, absEnd, replacementNode));
  return true;
}
