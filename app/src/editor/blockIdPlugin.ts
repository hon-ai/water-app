// Block-id allocation + de-duplication plugin.
//
// Every top-level block node carries a `blockId` attribute (`^bk-XXXX`).
// On every transaction we walk the doc:
//   - Empty / missing id → assign a fresh one.
//   - Duplicate id (e.g. ProseMirror copied parent attrs onto a new node
//     during `tr.split`) → keep the first occurrence (left half = "original"),
//     re-id the duplicate.
//
// Merge (Backspace at start) is handled implicitly: PM keeps the first
// node's attrs when joining, so the merged block inherits the left id and
// the right id simply disappears with the deleted node.

import { Plugin } from "prosemirror-state";
import type { Node as PMNode } from "prosemirror-model";

// Crockford-ish alphabet (no I/L/O/U) keeps ids visually unambiguous in
// the markdown serialization, where they appear as `^bk-XXXX`. The
// production allocator (water-core) will use a larger keyspace; this is
// a 32^4 = ~1M-id space, fine for an in-session editor.
const ALPHANUM = "0123456789abcdefghjkmnpqrstvwxyz";

function newBlockId(): string {
  let s = "^bk-";
  for (let i = 0; i < 4; i++) {
    const idx = Math.floor(Math.random() * ALPHANUM.length);
    s += ALPHANUM[idx] ?? "0";
  }
  return s;
}

/** Transaction filter that assigns + de-duplicates block IDs. */
export function blockIdPlugin(): Plugin {
  return new Plugin({
    appendTransaction(_transactions, _oldState, newState) {
      const seen = new Set<string>();
      const fixes: Array<{ pos: number; node: PMNode }> = [];

      newState.doc.forEach((node, offset) => {
        const id = (node.attrs?.blockId ?? "") as string;
        if (!id || seen.has(id)) {
          fixes.push({ pos: offset, node });
        } else {
          seen.add(id);
        }
      });

      if (fixes.length === 0) return null;

      const tr = newState.tr;
      for (const { pos, node } of fixes) {
        const fresh = newBlockId();
        // pos values were captured before any setNodeMarkup; setNodeMarkup
        // is an attrs-only change so positions remain valid across the loop.
        tr.setNodeMarkup(pos, undefined, { ...node.attrs, blockId: fresh });
        seen.add(fresh);
      }
      return tr;
    },
  });
}
