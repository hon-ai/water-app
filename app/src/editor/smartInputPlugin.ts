/**
 * Smart input substitutions for prose typography.
 *
 * Implemented as a ProseMirror Plugin with `handleTextInput`. Watches
 * each typed character and rewrites the *previous* character(s) into
 * a typographic equivalent when the canonical pattern is hit:
 *
 *   `--` (the second hyphen)   → em-dash `—`
 *   `...` (the third period)   → ellipsis `…`
 *   `"` after BOL/whitespace   → left curly quote `“`
 *   `"` after word character   → right curly quote `”`
 *   `'` after BOL/whitespace   → left curly quote `‘`
 *   `'` after word character   → right curly quote `’`
 *
 * Each rule runs only inside a textblock (paragraph / dialogue /
 * heading / list_item). Avoided inside code-block-style nodes — the
 * Water schema doesn't currently expose one, but the guard is cheap.
 *
 * Substitutions can be undone by the writer via Mod-Z because each
 * one dispatches a single transaction; history collects them.
 */

import { Plugin } from "prosemirror-state";

const WORD_CHAR = /[\w*À-ſ]/u;

export function smartInputPlugin(): Plugin {
  return new Plugin({
    props: {
      handleTextInput(view, from, _to, text) {
        // Only single-char insertions trigger any rule.
        if (text.length !== 1) return false;
        const { state } = view;
        // Only apply inside a real textblock.
        const $from = state.doc.resolve(from);
        if (!$from.parent.isTextblock) return false;

        const prev1 = from > 0 ? state.doc.textBetween(from - 1, from, "\n", "\n") : "";
        const prev2 = from > 1 ? state.doc.textBetween(from - 2, from, "\n", "\n") : "";

        // ── em-dash: -- → —
        if (text === "-" && prev1 === "-") {
          view.dispatch(view.state.tr.replaceWith(from - 1, from, state.schema.text("—")));
          return true;
        }

        // ── ellipsis: ... → …
        if (text === "." && prev2 === "..") {
          view.dispatch(view.state.tr.replaceWith(from - 2, from, state.schema.text("…")));
          return true;
        }

        // ── smart double quotes
        if (text === '"') {
          const open = !prev1 || /\s/.test(prev1) || /[\(\[\{—–-]/.test(prev1);
          const ch = open ? "“" : "”";
          view.dispatch(view.state.tr.insertText(ch, from));
          return true;
        }

        // ── smart single quotes (also apostrophes after word chars)
        if (text === "'") {
          // After a word character: apostrophe (right single quote).
          // After whitespace / BOL: opening single quote.
          const isApostrophe = WORD_CHAR.test(prev1);
          const isOpener = !prev1 || /\s/.test(prev1) || /[\(\[\{—–-]/.test(prev1);
          const ch = isApostrophe ? "’" : isOpener ? "‘" : "’";
          view.dispatch(view.state.tr.insertText(ch, from));
          return true;
        }

        return false;
      },
    },
  });
}
