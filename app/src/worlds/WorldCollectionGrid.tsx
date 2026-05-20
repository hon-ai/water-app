import { useEffect, useState } from "react";
import {
  ipc,
  type WorldEntryIndexEntry,
  type WorldSegment,
} from "../ipc/commands";
import { WorldEntryCard } from "./WorldEntryCard";

/**
 * Collection-segment index (M4 T22).
 *
 * Lists every entry under a collection segment as a `WorldEntryCard`
 * grid, plus a trailing `+ New entry` button that creates an empty-name
 * draft via `worldEntryCreate` and routes straight into the
 * Conversational Intake overlay. The empty draft is reaped by the
 * orphan-reaper (`worldEntryDeleteIfEmpty`) if the user abandons intake
 * before naming the entry — see `commands::world` for the contract.
 */
export function WorldCollectionGrid({
  segment,
  onOpenEntry,
  onOpenIntake,
}: {
  segment: WorldSegment;
  onOpenEntry: (entryId: string) => void;
  onOpenIntake: (segmentId: string, draftEntryId: string) => void;
}) {
  const [entries, setEntries] = useState<WorldEntryIndexEntry[]>([]);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    let cancelled = false;
    ipc.worldEntryList(segment.id).then((rows) => {
      if (cancelled) return;
      setEntries(rows);
      setLoaded(true);
    });
    return () => {
      cancelled = true;
    };
  }, [segment.id]);

  if (!loaded) return <div className="water-loading">Loading</div>;

  return (
    <div className="world-collection-grid">
      <h2>{segment.name}</h2>
      <div className="world-entry-grid">
        {entries.map((e) => (
          <WorldEntryCard
            key={e.id}
            entry={e}
            hueToken={segment.hue_token}
            onClick={() => onOpenEntry(e.id)}
          />
        ))}
        <button
          className="world-entry-card-new"
          onClick={async () => {
            const newId = await ipc.worldEntryCreate({
              segmentId: segment.id,
              name: "",
            });
            onOpenIntake(segment.id, newId);
          }}
          data-testid="new-entry-button"
        >
          + New entry
        </button>
      </div>
    </div>
  );
}
