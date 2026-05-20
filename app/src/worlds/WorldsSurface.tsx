import { useEffect, useState } from "react";
import { WorldIndex } from "./WorldIndex";
import { WorldSegmentView } from "./WorldSegmentView";
import { WorldEntrySheet } from "./WorldEntrySheet";
import { WorldEntryIntakeSheet } from "./WorldEntryIntakeSheet";
import { SegmentTemplateEditor } from "./SegmentTemplateEditor";

/**
 * Worlds surface router (M4 T20, spec § 10).
 *
 * Five-view router covering the World Bible flow:
 *  - `index`         — the grid of WorldSegmentTiles (T20).
 *  - `segment`       — single-doc sheet OR collection-entry index for
 *                      a given segment (T21–T22, placeholder here).
 *  - `entry`         — collection-entry editor (T23, placeholder).
 *  - `entry-intake`  — overlaid Conversational Intake for a draft entry
 *                      (T24, placeholder).
 *  - `new-segment`   — segment-create modal (T26+, placeholder).
 *
 * **Scroll preservation:** the index captures `window.scrollY` when the
 * user opens a segment, and restores it via `queueMicrotask` on back
 * navigation. Same shape as M3 T20's `CharactersSurface`, just without
 * the under-display-none mount — the index re-renders on return but its
 * data effects guard against stale work via `cancelled` flags inside
 * `WorldIndex` / `WorldSegmentTile`, so the round-trip is cheap.
 *
 * The `projectId` prop is currently unused at this layer — IPC commands
 * implicitly scope to the open project — but is threaded so future
 * sub-views can derive per-project URLs or react to multi-project state
 * without re-plumbing.
 */
type View =
  | { kind: "index" }
  | { kind: "segment"; segmentId: string }
  | { kind: "entry"; segmentId: string; entryId: string }
  | { kind: "entry-intake"; segmentId: string; draftEntryId: string }
  | { kind: "new-segment" };

export function WorldsSurface({ projectId: _projectId }: { projectId: string }) {
  const [view, setView] = useState<View>({ kind: "index" });
  const [indexScrollY, setIndexScrollY] = useState(0);

  function goToSegment(segmentId: string) {
    if (view.kind === "index") setIndexScrollY(window.scrollY);
    setView({ kind: "segment", segmentId });
  }

  function goToIndex() {
    setView({ kind: "index" });
    queueMicrotask(() => window.scrollTo(0, indexScrollY));
  }

  function goToEntry(segmentId: string, entryId: string) {
    setView({ kind: "entry", segmentId, entryId });
  }

  // M4 T30: external nav from the Bouquet pin handler when a Chorus +
  // `no_universe_yet` pin seeds a new `locations` stub. The handler
  // dispatches `water:nav-world-entry` with `{ segmentId, entryId }`;
  // we route to the entry view so the writer sees the stub immediately.
  // Decoupled via the window event rather than a context to keep
  // `<Bouquet>` (which lives under the editor canvas, not this surface)
  // unaware of the worlds-surface routing primitive.
  useEffect(() => {
    function handler(e: Event) {
      const detail = (e as CustomEvent<{ segmentId?: string; entryId?: string }>).detail;
      if (!detail?.segmentId || !detail.entryId) return;
      goToEntry(detail.segmentId, detail.entryId);
    }
    window.addEventListener("water:nav-world-entry", handler);
    return () => {
      window.removeEventListener("water:nav-world-entry", handler);
    };
  }, []);

  return (
    <div className="worlds-surface">
      {view.kind === "index" && (
        <WorldIndex
          onSelectSegment={goToSegment}
          onNewSegment={() => setView({ kind: "new-segment" })}
        />
      )}
      {view.kind === "segment" && (
        <div>
          <button onClick={goToIndex}>← Back</button>
          <WorldSegmentView
            segmentId={view.segmentId}
            onOpenEntry={(entryId) => goToEntry(view.segmentId, entryId)}
            onOpenIntake={(segId, draftId) =>
              setView({
                kind: "entry-intake",
                segmentId: segId,
                draftEntryId: draftId,
              })
            }
          />
        </div>
      )}
      {view.kind === "entry" && (
        <div>
          <button onClick={() => goToSegment(view.segmentId)}>← Back</button>
          <WorldEntrySheet
            segmentId={view.segmentId}
            entryId={view.entryId}
          />
        </div>
      )}
      {view.kind === "entry-intake" && (
        <WorldEntryIntakeSheet
          segmentId={view.segmentId}
          draftEntryId={view.draftEntryId}
          onComplete={(entryId) =>
            setView({ kind: "entry", segmentId: view.segmentId, entryId })
          }
          onClose={() =>
            setView({ kind: "segment", segmentId: view.segmentId })
          }
        />
      )}
      {view.kind === "new-segment" && (
        <SegmentTemplateEditor
          mode="create"
          onSave={(newId) => setView({ kind: "segment", segmentId: newId })}
          onClose={() => setView({ kind: "index" })}
        />
      )}
    </div>
  );
}
