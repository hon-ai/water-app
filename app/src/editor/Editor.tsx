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
import { classifyCursor } from "./cursorClassifier";
import { useIdleDetector } from "./useIdleDetector";
import { emitTypingTelemetry } from "./typingTelemetry";

type StructuralInflection =
  | "new_scene"
  | "new_chapter"
  | "pov_change"
  | "location_change"
  | "none";

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
  // Telemetry state: rate-limit timestamps, rolling 10s word-count history,
  // and the most-recent detected structural inflection (consumed + cleared
  // on emit). The history is an append-only ring of `{ts, totalWords}`
  // snapshots, pruned to entries from the last 10s + at most one older
  // entry (the baseline for delta-from-10s-ago lookup).
  const lastEmitAtRef = useRef<number>(0);
  const wordHistoryRef = useRef<Array<{ ts: number; totalWords: number }>>([]);
  const pendingInflectionRef = useRef<StructuralInflection>("none");
  // Captured handle to the idle detector's `onActivity` so the persistent
  // dispatchTransaction closure can reset the idle timer on every edit.
  const onActivityRef = useRef<() => void>(() => {});
  useEffect(() => {
    onChangeRef.current = onChange;
    onTransactionRef.current = onTransaction;
  });

  // Build the telemetry payload from the current view state and emit it.
  // Caller passes the idle duration (0 for live-typing ticks, 3000 for the
  // 3 s idle pulse).
  const emitFromCurrentState = (idleMs: number) => {
    const view = viewRef.current;
    if (!view) return;
    if (syncingRef.current) return;
    const { state } = view;
    const $pos = state.doc.resolve(state.selection.from);
    const blockNode = $pos.parent;
    const blockOffset = $pos.parentOffset;
    // Append a trailing newline so the classifier's EOL branch can fire
    // when the cursor sits at the end of the block's text content.
    const blockText = `${blockNode.textContent}\n`;
    const cursorClassification = classifyCursor(blockText, blockOffset);
    const blockIdRaw: unknown = blockNode.attrs["blockId"];
    const blockId = typeof blockIdRaw === "string" ? blockIdRaw : "";
    // Spec § 5.3: `recent_word_delta` = words added in the last 10 s.
    // Walk the history backwards to find the most recent snapshot that is
    // >= 10 s old; that snapshot's totalWords is our baseline. If no such
    // snapshot exists (history is empty or all entries are within the
    // last 10 s window), the baseline is 0.
    const now = Date.now();
    const totalWords = markdownFromDoc(state.doc).split(/\s+/).filter(Boolean).length;
    const history = wordHistoryRef.current;
    const tenSecAgo = now - 10_000;
    let baseline = 0;
    for (let i = history.length - 1; i >= 0; i--) {
      const entry = history[i];
      if (entry && entry.ts <= tenSecAgo) {
        baseline = entry.totalWords;
        break;
      }
    }
    const recentWordDelta = totalWords - baseline;
    // Append current snapshot, then prune. Keep all entries within the
    // last 10 s window + at most one older entry (needed for baseline
    // lookup once the next emit happens).
    history.push({ ts: now, totalWords });
    while (history.length > 1) {
      const first = history[0];
      const second = history[1];
      if (first && second && first.ts < tenSecAgo && second.ts < tenSecAgo) {
        history.shift();
      } else {
        break;
      }
    }
    const structuralInflection = pendingInflectionRef.current;
    pendingInflectionRef.current = "none";
    void emitTypingTelemetry({
      idle_for_ms: idleMs,
      cursor_classification: cursorClassification,
      block_id: blockId,
      recent_word_delta: recentWordDelta,
      structural_inflection: structuralInflection,
    });
  };

  // 3 s idle pulse. We can't depend on `emitFromCurrentState` from inside
  // the persistent view, so we stash `onActivity` in a ref above.
  const idle = useIdleDetector(3000, () => emitFromCurrentState(3000));
  onActivityRef.current = idle.onActivity;

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
          // Structural-inflection scan: any scene_break -> "new_scene",
          // any h2 -> "new_chapter". Heuristic: this scans every block on
          // every transaction, OK at M2 scene sizes; revisit if it shows
          // up in profiling once scenes get long.
          let inflection: StructuralInflection = "none";
          next.doc.descendants((node) => {
            if (node.type.name === "scene_break") {
              inflection = "new_scene";
              return false;
            }
            if (node.type.name === "heading" && node.attrs["level"] === 2) {
              inflection = "new_chapter";
              return false;
            }
            return true;
          });
          if (inflection !== "none") pendingInflectionRef.current = inflection;
          // Reset idle timer; this is real user activity.
          onActivityRef.current();
          // 5 Hz cap on live-typing emits.
          const now = Date.now();
          if (now - lastEmitAtRef.current > 200) {
            lastEmitAtRef.current = now;
            emitFromCurrentState(0);
          }
        }
        onTransactionRef.current?.(tr);
      },
    });
    viewRef.current = view;
    // Seed the rolling-history baseline so the first emit's delta is 0
    // (no words added in the previous 10 s) instead of `totalWords`.
    wordHistoryRef.current = [
      {
        ts: Date.now(),
        totalWords: markdownFromDoc(state.doc).split(/\s+/).filter(Boolean)
          .length,
      },
    ];

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
    // Re-baseline the rolling history after a scene-switch so the next
    // live emit's `recent_word_delta` doesn't include the swap and we
    // don't inherit word-count history from the previous scene.
    wordHistoryRef.current = [
      {
        ts: Date.now(),
        totalWords: markdownFromDoc(view.state.doc)
          .split(/\s+/)
          .filter(Boolean).length,
      },
    ];
    pendingInflectionRef.current = "none";
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
