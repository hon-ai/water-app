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

import { useEffect, useRef, useState } from "react";
import { EditorState, type Transaction, type EditorState as PMEditorState } from "prosemirror-state";
import { EditorView } from "prosemirror-view";
import { keymap } from "prosemirror-keymap";
import { history, redo, undo } from "prosemirror-history";
import { baseKeymap, splitBlock, toggleMark } from "prosemirror-commands";
import { splitListItem } from "prosemirror-schema-list";
import { schema } from "./schema";
import { blockIdPlugin } from "./blockIdPlugin";
import {
  setTriggerHighlight,
  triggerHighlightPlugin,
  type TriggerHighlight,
} from "./triggerHighlightPlugin";
import {
  editorUnderlinePlugin,
  setEditorUnderlines,
  type UnderlineAnchor,
} from "./editorUnderlinePlugin";
import { acceptSuggestion, type AcceptAnchor } from "./acceptSuggestion";
import { smartInputPlugin } from "./smartInputPlugin";
import { docFromMarkdown, markdownFromDoc } from "./serialize";
import { classifyCursor } from "./cursorClassifier";
import { useIdleDetector } from "./useIdleDetector";
import { emitTypingTelemetry } from "./typingTelemetry";
import { SelectionToolbar } from "./SelectionToolbar";
import { LinkPopup } from "./LinkPopup";
import { ipc } from "../ipc/commands";

type StructuralInflection =
  | "new_scene"
  | "new_chapter"
  | "pov_change"
  | "location_change"
  | "none";

function countDocWords(doc: import("prosemirror-model").Node): number {
  let words = 0;
  doc.descendants((node) => {
    if (node.isText && node.text) {
      words += node.text.split(/\s+/).filter(Boolean).length;
    }
    return true;
  });
  return words;
}

interface Props {
  value: string;
  onChange: (markdown: string) => void;
  onTransaction?: (tr: Transaction) => void;
  placeholder?: string;
}

type LinkPopupRequest = {
  anchor: { left: number; top: number };
  initialUrl: string;
  editing: boolean;
};

export function Editor({ value, onChange, onTransaction, placeholder }: Props) {
  const hostRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  // Popup request from Mod-K keymap or toolbar's Link icon click. Null
  // closes the popup; setting an object opens it.
  const [linkPopupReq, setLinkPopupRequest] = useState<LinkPopupRequest | null>(
    null,
  );
  // Force a re-render after the view mounts so the conditional
  // <SelectionToolbar editorView={...}> below can render (it needs a
  // non-null view reference).
  const [viewReady, setViewReady] = useState(false);
  // Track the latest props in refs so the persistent view (mounted once)
  // always reads fresh handlers without remounting on every render.
  const onChangeRef = useRef(onChange);
  const onTransactionRef = useRef(onTransaction);
  // True while we're applying a programmatic value-prop sync; suppresses
  // the change handler so the parent doesn't see its own value bounced
  // back as a "user edit".
  const syncingRef = useRef(false);
  // Track the last serialized markdown string we emitted or synced from the parent
  // to avoid redundant serialization loops during keystrokes/re-renders.
  const lastSerializedRef = useRef<string | null>(null);
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
    const totalWords = countDocWords(state.doc);
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
    // `last_block_text` is only populated on idle pulses (>=3 s). During
    // typing bursts (5 Hz cap) we send `null` to keep the wire small;
    // `character_dissonance` only needs the text once the writer pauses.
    const lastBlockText = idleMs >= 3000 ? blockNode.textContent : null;
    void emitTypingTelemetry({
      idle_for_ms: idleMs,
      cursor_classification: cursorClassification,
      block_id: blockId,
      recent_word_delta: recentWordDelta,
      structural_inflection: structuralInflection,
      last_block_text: lastBlockText,
    });
  };

  // 3 s idle pulse. We can't depend on `emitFromCurrentState` from inside
  // the persistent view, so we stash `onActivity` in a ref above.
  const idle = useIdleDetector(3000, () => emitFromCurrentState(3000));
  onActivityRef.current = idle.onActivity;

  // ---- Link popup helpers ----------------------------------------------
  // These close over `viewRef` (stable) and `setLinkPopupRequest` (stable
  // React setter), so they're safe to reference from the persistent keymap
  // closure created at mount time.

  function computeAnchorForSelection(): { left: number; top: number } {
    const view = viewRef.current;
    if (!view) return { left: 0, top: 0 };
    try {
      const { from, to } = view.state.selection;
      const fromCoords = view.coordsAtPos(from);
      const toCoords = view.coordsAtPos(to);
      const left = (fromCoords.left + toCoords.left) / 2;
      const top = Math.max(fromCoords.bottom, toCoords.bottom) + 8;
      return { left, top };
    } catch {
      return { left: 0, top: 0 };
    }
  }

  function getExistingLinkHref(state: PMEditorState): string {
    const $pos = state.doc.resolve(state.selection.from);
    const link = $pos.marks().find((m) => m.type.name === "link");
    const href = link?.attrs["href"];
    return typeof href === "string" ? href : "";
  }

  function hasLinkUnderCursor(state: PMEditorState): boolean {
    const $pos = state.doc.resolve(state.selection.from);
    return $pos.marks().some((m) => m.type.name === "link");
  }

  function expandToLinkMark(
    state: PMEditorState,
  ): { from: number; to: number } | null {
    const $pos = state.doc.resolve(state.selection.from);
    const linkMark = $pos.marks().find((m) => m.type.name === "link");
    if (!linkMark) return null;
    let from = state.selection.from;
    let to = state.selection.from;
    const doc = state.doc;
    while (from > 0) {
      const $p = doc.resolve(from - 1);
      if (
        !$p
          .marks()
          .some(
            (m) =>
              m.type.name === "link" &&
              m.attrs["href"] === linkMark.attrs["href"],
          )
      )
        break;
      from -= 1;
    }
    while (to < doc.content.size) {
      const $p = doc.resolve(to + 1);
      if (
        !$p
          .marks()
          .some(
            (m) =>
              m.type.name === "link" &&
              m.attrs["href"] === linkMark.attrs["href"],
          )
      )
        break;
      to += 1;
    }
    return { from, to };
  }

  const handleLinkApply = (url: string) => {
    const view = viewRef.current;
    if (!view) return;
    const { from, to } = view.state.selection;
    const tr = view.state.tr.addMark(
      from,
      to,
      schema.marks.link!.create({ href: url }),
    );
    view.dispatch(tr);
  };

  const handleLinkRemove = () => {
    const view = viewRef.current;
    if (!view) return;
    const { from, to } = view.state.selection;
    const range = from === to ? expandToLinkMark(view.state) : { from, to };
    if (!range) return;
    const tr = view.state.tr.removeMark(
      range.from,
      range.to,
      schema.marks.link!,
    );
    view.dispatch(tr);
  };

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
          "Mod-b": toggleMark(schema.marks.strong!),
          "Mod-i": toggleMark(schema.marks.em!),
          "Mod-Shift-x": toggleMark(schema.marks.strike!),
          "Mod-k": (state) => {
            // Mod-K opens the LinkPopup. If selection is collapsed and
            // inside a link, open in edit mode; otherwise require a
            // non-empty selection to add a link.
            if (state.selection.empty) {
              if (!hasLinkUnderCursor(state)) return false;
            }
            setLinkPopupRequest({
              anchor: computeAnchorForSelection(),
              initialUrl: getExistingLinkHref(state),
              editing: !state.selection.empty || hasLinkUnderCursor(state),
            });
            return true;
          },
          Enter: (s, dispatch) =>
            splitListItem(schema.nodes.list_item!)(s, dispatch) ||
            splitBlock(s, dispatch),
          // Shift-Enter inserts a hard_break — a soft line break
          // useful for stanzas, poetry, address blocks. Falls back
          // to a literal newline when the schema's hard_break node
          // somehow isn't present.
          "Shift-Enter": (s, dispatch) => {
            const hb = schema.nodes.hard_break;
            if (!hb) return false;
            if (dispatch) {
              const tr = s.tr.replaceSelectionWith(hb.create()).scrollIntoView();
              dispatch(tr);
            }
            return true;
          },
        }),
        keymap(baseKeymap),
        blockIdPlugin(),
        triggerHighlightPlugin(),
        editorUnderlinePlugin(),
        smartInputPlugin(),
      ],
    });
    // Run the block-id plugin's appendTransaction against the initial doc
    // synchronously so the view mounts with ids already assigned. This
    // avoids firing an `onChange` on mount (which would dirty the buffer
    // for a freshly-loaded scene).
    const state = initial.apply(initial.tr);

    const view = new EditorView(host, {
      state,
      // Mod-click on a link mark opens it in the OS default browser via
      // tauri-plugin-shell (capability-scoped to http/https/mailto). PM
      // calls this BEFORE its own selection behavior; returning true
      // suppresses the cursor-placement fall-through. Plain (un-modified)
      // clicks fall through to normal selection.
      handleClickOn(view, _pos, _node, _nodePos, event) {
        const isModClick = event.metaKey || event.ctrlKey;
        if (!isModClick) return false;
        // posAtCoords returns null when the click is outside the editor's
        // doc area (e.g. on the gutter); safe to skip in that case.
        const coords = view.posAtCoords({
          left: event.clientX,
          top: event.clientY,
        });
        if (!coords) return false;
        const $pos = view.state.doc.resolve(coords.pos);
        const link = $pos.marks().find((m) => m.type.name === "link");
        const href = link?.attrs["href"];
        if (typeof href !== "string" || href.length === 0) return false;
        void ipc.openExternalLink(href);
        return true;
      },
      dispatchTransaction(tr) {
        const next = view.state.apply(tr);
        view.updateState(next);
        if (tr.docChanged && !syncingRef.current) {
          const markdown = markdownFromDoc(next.doc);
          lastSerializedRef.current = markdown;
          onChangeRef.current(markdown);
          // Structural-inflection scan: any scene_break -> "new_scene",
          // any h2 -> "new_chapter". Gated on top-level block-count
          // change so plain typing inside a paragraph is O(1). The rare
          // setBlockType-in-place case (promoting a paragraph to h2 via
          // shortcut) won't trip this — acceptable per spec; the engine
          // picks the heading up on the next insertion.
          if (tr.before.childCount !== next.doc.childCount) {
            let inflection: StructuralInflection = "none";
            next.doc.forEach((node) => {
              if (node.type.name === "scene_break") {
                inflection = "new_scene";
              } else if (
                node.type.name === "heading" &&
                node.attrs["level"] === 2
              ) {
                inflection = "new_chapter";
              }
            });
            if (inflection !== "none") pendingInflectionRef.current = inflection;
          }
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
    setViewReady(true);
    // Seed the rolling-history baseline so the first emit's delta is 0
    // (no words added in the previous 10 s) instead of `totalWords`.
    wordHistoryRef.current = [
      {
        ts: Date.now(),
        totalWords: countDocWords(state.doc),
      },
    ];
    lastSerializedRef.current = value;

    return () => {
      // Defer view.destroy() to a microtask so child components
      // (SelectionToolbar) can run their useEffect cleanups against a
      // still-alive view. React 18 runs passive-effect cleanups in a
      // deleted subtree parent-first; without this defer, the toolbar's
      // restoreDispatch cleanup would call setProps on a destroyed view
      // and crash. Tests previously caught this as a docView-null error.
      const dyingView = view;
      viewRef.current = null;
      setViewReady(false);
      queueMicrotask(() => {
        dyingView.destroy();
      });
    };
  }, []);

  // Reconcile external `value` changes (scene-switch). We compare against
  // the serialized current doc to avoid clobbering local edits with a
  // round-tripped version of the same text.
  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    if (value === lastSerializedRef.current) return;
    const current = markdownFromDoc(view.state.doc);
    if (current === value) {
      lastSerializedRef.current = value;
      return;
    }
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
        totalWords: countDocWords(view.state.doc),
      },
    ];
    pendingInflectionRef.current = "none";
    lastSerializedRef.current = value;
  }, [value]);

  // Pill trigger-highlight bridge. PillLayer dispatches window
  // CustomEvents instead of holding a direct EditorView reference;
  // the editor listens here and forwards into the PM plugin. Using
  // events keeps the pill margin decoupled from the editor's
  // internals (Phase 3.5 — UX_SPEC.md §C.6.d).
  useEffect(() => {
    const onSet = (e: Event) => {
      const detail = (e as CustomEvent<TriggerHighlight>).detail;
      setTriggerHighlight(viewRef.current, detail);
    };
    const onClear = () => {
      setTriggerHighlight(viewRef.current, null);
    };
    window.addEventListener("water:set-trigger-highlight", onSet);
    window.addEventListener("water:clear-trigger-highlight", onClear);
    return () => {
      window.removeEventListener("water:set-trigger-highlight", onSet);
      window.removeEventListener("water:clear-trigger-highlight", onClear);
    };
  }, []);

  // Phase 5 — editor-pill underline bridge. EditorCanvas dispatches
  // `water:set-editor-underlines` whenever the live editor-pill set
  // for this scene changes (post-save diagnostics run + post-dismiss
  // refetch). The editor listens here and forwards into the
  // `editorUnderlinePlugin`.
  useEffect(() => {
    const onSet = (e: Event) => {
      const detail = (e as CustomEvent<UnderlineAnchor[]>).detail;
      setEditorUnderlines(viewRef.current, detail);
    };
    const onClear = () => {
      setEditorUnderlines(viewRef.current, null);
    };
    const onAccept = (e: Event) => {
      const detail = (e as CustomEvent<AcceptAnchor>).detail;
      acceptSuggestion(viewRef.current, detail);
    };
    window.addEventListener("water:set-editor-underlines", onSet);
    window.addEventListener("water:clear-editor-underlines", onClear);
    window.addEventListener("water:accept-editor-pill", onAccept);
    return () => {
      window.removeEventListener("water:set-editor-underlines", onSet);
      window.removeEventListener("water:clear-editor-underlines", onClear);
      window.removeEventListener("water:accept-editor-pill", onAccept);
    };
  }, []);

  return (
    <>
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
      {viewReady && viewRef.current && (
        <SelectionToolbar
          editorView={viewRef.current}
          onLinkClick={(anchor) => {
            const view = viewRef.current;
            if (!view) return;
            setLinkPopupRequest({
              anchor,
              initialUrl: getExistingLinkHref(view.state),
              editing:
                hasLinkUnderCursor(view.state) || !view.state.selection.empty,
            });
          }}
        />
      )}
      {linkPopupReq && (
        <LinkPopup
          anchor={linkPopupReq.anchor}
          initialUrl={linkPopupReq.initialUrl}
          editing={linkPopupReq.editing}
          onApply={handleLinkApply}
          onRemove={handleLinkRemove}
          onClose={() => setLinkPopupRequest(null)}
        />
      )}
    </>
  );
}
