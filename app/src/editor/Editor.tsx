// React wrapper around a ProseMirror EditorView.
//
// The editor is a controlled-ish component: it mounts once with the
// initial `value`, emits `onChange(markdown)` for every doc-changing
// transaction, and only re-syncs from `value` when the parent's value
// drifts from the current serialized doc (e.g. on scene-switch).
//
// `onTransaction` is exposed for callers that want to observe raw
// transactions (e.g. for analytics or pill insertion); production
// EditorCanvas doesn't currently subscribe.

import { useEffect, useRef } from "react";
import { EditorState, type Transaction } from "prosemirror-state";
import { EditorView } from "prosemirror-view";
import { keymap } from "prosemirror-keymap";
import { history, redo, undo } from "prosemirror-history";
import { baseKeymap, splitBlock } from "prosemirror-commands";
import { splitListItem } from "prosemirror-schema-list";
import { schema } from "./schema";
import { blockIdPlugin } from "./blockIdPlugin";
import { docFromMarkdown, markdownFromDoc } from "./serialize";

interface Props {
  value: string;
  onChange: (markdown: string) => void;
  onTransaction?: (tr: Transaction) => void;
  placeholder?: string;
}

export function Editor({ value, onChange, onTransaction, placeholder }: Props) {
  const hostRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  // Track the latest props in refs so the persistent view (mounted once)
  // always reads fresh handlers without remounting on every render.
  const onChangeRef = useRef(onChange);
  const onTransactionRef = useRef(onTransaction);
  // True while we're applying a programmatic value-prop sync; suppresses
  // the change handler so the parent doesn't see its own value bounced
  // back as a "user edit".
  const syncingRef = useRef(false);
  useEffect(() => {
    onChangeRef.current = onChange;
    onTransactionRef.current = onTransaction;
  });

  // Mount once: create the PM view with the initial value.
  // We intentionally exclude `value` from deps; subsequent value changes
  // are handled by the sync effect below.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;

    const initial = EditorState.create({
      doc: docFromMarkdown(schema, value),
      schema,
      plugins: [
        history(),
        keymap({
          "Mod-z": undo,
          "Mod-y": redo,
          "Mod-Shift-z": redo,
          Enter: (s, dispatch) =>
            splitListItem(schema.nodes.list_item!)(s, dispatch) ||
            splitBlock(s, dispatch),
        }),
        keymap(baseKeymap),
        blockIdPlugin(),
      ],
    });
    // Run the block-id plugin's appendTransaction against the initial doc
    // synchronously so the view mounts with ids already assigned. This
    // avoids firing an `onChange` on mount (which would dirty the buffer
    // for a freshly-loaded scene).
    const state = initial.apply(initial.tr);

    const view = new EditorView(host, {
      state,
      dispatchTransaction(tr) {
        const next = view.state.apply(tr);
        view.updateState(next);
        if (tr.docChanged && !syncingRef.current) {
          onChangeRef.current(markdownFromDoc(next.doc));
        }
        onTransactionRef.current?.(tr);
      },
    });
    viewRef.current = view;

    return () => {
      view.destroy();
      viewRef.current = null;
    };
  }, []);

  // Reconcile external `value` changes (scene-switch). We compare against
  // the serialized current doc to avoid clobbering local edits with a
  // round-tripped version of the same text.
  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    const current = markdownFromDoc(view.state.doc);
    if (current === value) return;
    const newDoc = docFromMarkdown(schema, value);
    const tr = view.state.tr.replaceWith(
      0,
      view.state.doc.content.size,
      newDoc.content,
    );
    syncingRef.current = true;
    try {
      view.dispatch(tr);
    } finally {
      syncingRef.current = false;
    }
  }, [value]);

  return (
    <div
      ref={hostRef}
      className="water-editor"
      data-placeholder={placeholder}
      style={{
        outline: "none",
        minHeight: 480,
        color: "var(--water-fg-default)",
        fontFamily: "var(--water-font-sans)",
        fontSize: "var(--water-fs-body)",
        lineHeight: "var(--water-lh-body)",
      }}
    />
  );
}
