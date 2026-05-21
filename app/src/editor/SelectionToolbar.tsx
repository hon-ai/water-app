// Pastel-glow selection toolbar matching M2 pill design. Appears above
// any non-collapsed selection in the editor with B / I / Link buttons.
// Portal-mounts to <body> so it escapes the editor host's overflow clip.
//
// Visibility:
//   - Hidden when selection is collapsed.
//   - Hidden when Escape is pressed (until next selection change).
//   - Hidden during scroll events (until next selection change).
//
// Position is computed via view.coordsAtPos for the selection endpoints;
// jsdom can't measure layout so tests assert presence + active state via
// data-active attributes, not pixel positions.

import { useEffect, useState, type CSSProperties, type MouseEvent } from "react";
import { createPortal } from "react-dom";
import { Bold, Italic, Link2 } from "lucide-react";
import type { EditorView } from "prosemirror-view";
import { toggleMark } from "prosemirror-commands";
import { schema } from "./schema";

interface Props {
  editorView: EditorView;
  /** Called when the writer clicks the Link icon. Receives screen
   *  coordinates anchored just below the selection so the popup (T5)
   *  can position itself. */
  onLinkClick?: (anchor: { left: number; top: number }) => void;
}

type Rect = { left: number; top: number; width: number; height: number };

function rectFromSelection(view: EditorView): Rect | null {
  const { from, to } = view.state.selection;
  if (from === to) return null;
  try {
    const fromCoords = view.coordsAtPos(from);
    const toCoords = view.coordsAtPos(to);
    const left = Math.min(fromCoords.left, toCoords.left);
    const right = Math.max(fromCoords.right, toCoords.right);
    const top = Math.min(fromCoords.top, toCoords.top);
    const bottom = Math.max(fromCoords.bottom, toCoords.bottom);
    return { left, top, width: right - left, height: bottom - top };
  } catch {
    // coordsAtPos can throw in jsdom or before layout; null falls through
    // to a (0,0) default in the render path so tests can still assert
    // presence.
    return null;
  }
}

function selectionHasMark(
  view: EditorView,
  markName: "strong" | "em" | "link",
): boolean {
  const markType = schema.marks[markName];
  if (!markType) return false;
  const { from, to, empty } = view.state.selection;
  if (empty) return false;
  return view.state.doc.rangeHasMark(from, to, markType);
}

export function SelectionToolbar({ editorView, onLinkClick }: Props) {
  // Bumped on every transaction with `selectionSet` or `docChanged` so
  // React re-renders. We don't read `version` directly; just touching it
  // is enough to trigger re-render.
  const [version, setVersion] = useState(0);
  // Latched on Escape or scroll. Reset on the next selection change.
  const [forceHidden, setForceHidden] = useState(false);

  // Wrap the editor's dispatchTransaction so we can observe selection +
  // doc changes. Cleanup restores the original handler so T6's Editor
  // (which sets its own dispatchTransaction) keeps working.
  useEffect(() => {
    const originalDispatch = editorView.props.dispatchTransaction?.bind(
      editorView,
    );
    editorView.setProps({
      dispatchTransaction(tr) {
        if (originalDispatch) {
          originalDispatch(tr);
        } else {
          editorView.updateState(editorView.state.apply(tr));
        }
        if (tr.selectionSet || tr.docChanged) {
          setVersion((v) => v + 1);
          setForceHidden(false);
        }
      },
    });
    return () => {
      // T6 review: defense-in-depth against React 18 deleted-tree cleanup
      // order; Editor's microtask-deferred view.destroy() is the primary
      // fix, this guard catches future refactors that might forget the
      // defer (or any third-party HoC that wraps Editor and inverts the
      // unmount order).
      if (editorView.isDestroyed) return;
      editorView.setProps({ dispatchTransaction: originalDispatch });
    };
  }, [editorView]);

  // Hide on Escape.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setForceHidden(true);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // Hide on scroll (any ancestor scroll changes the selection rect).
  useEffect(() => {
    const onScroll = () => setForceHidden(true);
    window.addEventListener("scroll", onScroll, { capture: true });
    return () =>
      window.removeEventListener("scroll", onScroll, {
        capture: true,
      } as EventListenerOptions);
  }, []);

  // Spec § 8.1 (4): auto-hide on editor blur. We listen to `focusout` on
  // the editor's host so we catch clicks/taps outside the editor host
  // (the title input, scenes panel, a sheet, etc.). We intentionally do
  // NOT hide when focus is moving to the toolbar's own buttons or to a
  // LinkPopup, both of which are rendered in portals outside the editor
  // host and need the toolbar to stay visible while interacting.
  useEffect(() => {
    const handleFocusOut = (e: FocusEvent) => {
      const next = e.relatedTarget as Element | null;
      if (next && next instanceof Element) {
        if (next.closest('[role="toolbar"]')) return;
        if (next.closest('[role="dialog"]')) return;
      }
      setForceHidden(true);
    };
    editorView.dom.addEventListener("focusout", handleFocusOut);
    return () => {
      editorView.dom.removeEventListener("focusout", handleFocusOut);
    };
  }, [editorView]);

  // Read version so the component re-renders on dispatch.
  void version;

  if (forceHidden) return null;
  // Primary visibility check is selection emptiness — that way jsdom
  // tests (which can't measure rects) still render the toolbar when a
  // non-empty selection is set programmatically.
  if (editorView.state.selection.empty) return null;
  const rect = rectFromSelection(editorView);

  const boldActive = selectionHasMark(editorView, "strong");
  const italicActive = selectionHasMark(editorView, "em");
  const linkActive = selectionHasMark(editorView, "link");

  // Position the toolbar above the selection's top edge with a real
  // vertical gap (translate handles the toolbar's own height; the
  // extra -14px keeps a visible margin between the toolbar's bottom
  // and the highlighted text). Anchor: top edge of selection.
  const left = rect ? rect.left + rect.width / 2 : 0;
  const top = rect ? rect.top : 0;

  const toolbarStyle: CSSProperties = {
    position: "fixed",
    left,
    top,
    transform: "translate(-50%, calc(-100% - 14px))",
    display: "flex",
    flexDirection: "row",
    gap: 6,
    padding: "6px 10px",
    borderRadius: "var(--water-r-16)",
    background:
      "color-mix(in oklch, var(--water-hue-coherence) 30%, var(--water-bg-paper))",
    boxShadow:
      "0 0 18px color-mix(in oklch, var(--water-hue-coherence) 50%, transparent)",
    pointerEvents: "auto",
    zIndex: 40,
    animation:
      "water-pill-fade-in var(--water-dur-small) var(--water-ease-out-soft) both",
  };

  const iconButtonStyle = (active: boolean): CSSProperties => ({
    width: 24,
    height: 24,
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    border: "none",
    background: active
      ? "color-mix(in oklch, var(--water-hue-flow) 40%, transparent)"
      : "transparent",
    color: "var(--water-fg-default)",
    cursor: "pointer",
    borderRadius: "var(--water-r-8)",
  });

  // Don't steal focus from the editor; clicks shouldn't collapse selection.
  const onMouseDownPreventBlur = (e: MouseEvent) => {
    e.preventDefault();
  };

  const handleBold = (e: MouseEvent) => {
    onMouseDownPreventBlur(e);
    const cmd = toggleMark(schema.marks.strong!);
    cmd(editorView.state, editorView.dispatch);
  };
  const handleItalic = (e: MouseEvent) => {
    onMouseDownPreventBlur(e);
    const cmd = toggleMark(schema.marks.em!);
    cmd(editorView.state, editorView.dispatch);
  };
  const handleLink = (e: MouseEvent) => {
    onMouseDownPreventBlur(e);
    const r = rectFromSelection(editorView);
    if (!r) {
      onLinkClick?.({ left: 0, top: 0 });
      return;
    }
    onLinkClick?.({ left: r.left + r.width / 2, top: r.top + r.height + 4 });
  };

  return createPortal(
    <div role="toolbar" aria-label="Text formatting" style={toolbarStyle}>
      <button
        type="button"
        aria-label="Bold"
        data-active={boldActive ? "true" : "false"}
        style={iconButtonStyle(boldActive)}
        onMouseDown={handleBold}
      >
        <Bold size={16} />
      </button>
      <button
        type="button"
        aria-label="Italic"
        data-active={italicActive ? "true" : "false"}
        style={iconButtonStyle(italicActive)}
        onMouseDown={handleItalic}
      >
        <Italic size={16} />
      </button>
      <button
        type="button"
        aria-label="Link"
        data-active={linkActive ? "true" : "false"}
        style={iconButtonStyle(linkActive)}
        onMouseDown={handleLink}
      >
        <Link2 size={16} />
      </button>
    </div>,
    document.body,
  );
}
