import { useState } from "react";
import { WorldIndex } from "./WorldIndex";

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
          <div data-testid="segment-view-placeholder">
            segment: {view.segmentId}
          </div>
        </div>
      )}
      {view.kind === "entry" && (
        <div>
          <button onClick={() => goToSegment(view.segmentId)}>← Back</button>
          <div data-testid="entry-placeholder">entry: {view.entryId}</div>
        </div>
      )}
      {view.kind === "entry-intake" && (
        <div data-testid="intake-placeholder">
          intake: {view.draftEntryId}
        </div>
      )}
      {view.kind === "new-segment" && (
        <div data-testid="new-segment-placeholder">new segment modal</div>
      )}
    </div>
  );
}
