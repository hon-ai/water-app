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

// Node types that carry a `blockId` attr. Top-level block kinds plus
// `list_item` (M2 spec § 5.2: list items each carry their own block-id).
const ID_BEARING_TYPES = new Set([
  "paragraph",
  "heading",
  "scene_break",
  "dialogue",
  "list_item",
]);

// Container types whose children we should recurse into to find
// `list_item` nodes that need ids. We don't recurse into general blocks
// (like paragraphs); only list containers, which is the only nesting
// kind that owns id-bearing children inside another block.
const LIST_CONTAINER_TYPES = new Set(["ordered_list", "bullet_list"]);

/** Transaction filter that assigns + de-duplicates block IDs. */
export function blockIdPlugin(): Plugin {
  return new Plugin({
    appendTransaction(_transactions, _oldState, newState) {
      const seen = new Set<string>();
      const fixes: Array<{ pos: number; node: PMNode }> = [];

      // Recursive walker. `pos` is the position *before* `node` in the
      // outer doc. To iterate `node`'s children, the inner position
      // starts at `pos + 1` (skipping the opening token of `node`),
      // and each child sits at `pos + 1 + childOffset`. For the
      // top-level doc node we call `walk(doc, -1)` so that
      // `doc.forEach`-equivalent offsets line up: `(-1) + 1 + offset = offset`.
      function walk(node: PMNode, pos: number): void {
        if (ID_BEARING_TYPES.has(node.type.name)) {
          const id = (node.attrs?.blockId ?? "") as string;
          if (!id || seen.has(id)) {
            fixes.push({ pos, node });
          } else {
            seen.add(id);
          }
        }
        // Only recurse into list containers (or the doc itself). This
        // keeps the walk cheap and avoids stepping into paragraphs.
        if (node.type.name === "doc" || LIST_CONTAINER_TYPES.has(node.type.name)) {
          node.forEach((child, childOffset) => {
            walk(child, pos + 1 + childOffset);
          });
        }
      }
      walk(newState.doc, -1);

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
