import { useEffect, useState } from "react";
import { ipc, type WorldSegment } from "../ipc/commands";
import { WorldSegmentTile } from "./WorldSegmentTile";

/**
 * The World Bible index grid (M4 T20).
 *
 * Loads all segments via `ipc.worldSegmentList()` and renders one
 * `WorldSegmentTile` per non-hidden segment, plus a trailing
 * "+ New segment" affordance.
 *
 * Hidden segments (via `world_segment.hidden = 1`) are filtered out
 * here on the client — the Rust side intentionally returns them too so
 * Settings can show a "Restore hidden segments" list later. Until that
 * surface lands we just drop them in the index.
 *
 * Loading state is rendered as a placeholder div rather than a skeleton
 * because the first paint is fast (six built-ins + any custom segments,
 * each is a single SQLite row).
 */
export function WorldIndex({
  onSelectSegment,
  onNewSegment,
}: {
  onSelectSegment: (segmentId: string) => void;
  onNewSegment: () => void;
}) {
  const [segments, setSegments] = useState<WorldSegment[]>([]);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    let cancelled = false;
    ipc.worldSegmentList().then((rows) => {
      if (cancelled) return;
      setSegments(rows);
      setLoaded(true);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  if (!loaded) return <div>Loading…</div>;

  return (
    <div className="world-index">
      <h2>World Bible</h2>
      <div className="world-index-grid">
        {segments
          .filter((s) => !s.hidden)
          .map((s) => (
            <WorldSegmentTile
              key={s.id}
              segment={s}
              onClick={() => onSelectSegment(s.id)}
            />
          ))}
        <button
          className="world-index-new-segment"
          onClick={onNewSegment}
          data-testid="new-segment-button"
        >
          + New segment
        </button>
      </div>
    </div>
  );
}
