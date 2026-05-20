import { useEffect, useState } from "react";
import { ipc, type WorldSegment } from "../ipc/commands";
import { WorldSingleDocSheet } from "./WorldSingleDocSheet";
import { WorldCollectionGrid } from "./WorldCollectionGrid";

/**
 * Segment-view router (M4 T21–T22).
 *
 * Loads the `WorldSegment` row by id from `worldSegmentList` (cheap — the
 * list is small and cached by the OS file cache server-side), then
 * branches on `is_collection`:
 *  - `false` → `WorldSingleDocSheet` (single-doc segment, inline-edit
 *    against the segment's `WorldEntryFile`).
 *  - `true`  → `WorldCollectionGrid` (entry-card grid + new-entry button).
 *
 * The list-then-find pattern (rather than a dedicated `worldSegmentRead`
 * command) matches how `WorldIndex` already fetches segments, and avoids
 * adding an IPC surface for a one-row read.
 */
export function WorldSegmentView({
  segmentId,
  onOpenEntry,
  onOpenIntake,
}: {
  segmentId: string;
  onOpenEntry: (entryId: string) => void;
  onOpenIntake: (segmentId: string, draftEntryId: string) => void;
}) {
  const [segment, setSegment] = useState<WorldSegment | null>(null);

  useEffect(() => {
    let cancelled = false;
    ipc.worldSegmentList().then((rows) => {
      if (cancelled) return;
      setSegment(rows.find((s) => s.id === segmentId) ?? null);
    });
    return () => {
      cancelled = true;
    };
  }, [segmentId]);

  if (!segment) return <div>Loading…</div>;
  if (segment.is_collection) {
    return (
      <WorldCollectionGrid
        segment={segment}
        onOpenEntry={onOpenEntry}
        onOpenIntake={onOpenIntake}
      />
    );
  }
  return <WorldSingleDocSheet segment={segment} />;
}
