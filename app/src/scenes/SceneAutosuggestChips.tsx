import { useEffect, useState } from "react";
import { ipc } from "../ipc/commands";
import {
  subscribeChips,
  type ChipSuggestion,
} from "./sceneMetadataChannel";

interface Props {
  sceneId: string;
  /**
   * Characters already linked to this scene. Used to filter out
   * character chips for characters the writer has already accepted —
   * once present, the suggestion is redundant. The parent
   * (SceneMetadataSheet) recomputes this set after each link/unlink,
   * so re-renders naturally hide just-accepted chips.
   */
  alreadyLinkedIds: Set<string>;
  /**
   * The current `scene.location_id`, if any. Used to filter out the
   * world chip that matches what the writer has already chosen.
   */
  currentLocationId: string | null;
  /** Fired after a chip's action (link or set-location) succeeds so the parent can reload meta. */
  onLinked: () => void;
}

/**
 * Row of "Suggested" chips fed by `subscribeChips`. Two chip kinds:
 *
 * - `character` chips link the character to the scene on primary click;
 *   `×` dismisses locally (no IPC, no persistence).
 * - `world_entry` chips set `scene.location_id` to the entry on primary
 *   click; `×` dismisses locally.
 *
 * The dismiss set is keyed by a `kind:id` composite so a character and
 * a location with the same id don't shadow each other. It's intentionally
 * NOT cleared when the chip set changes: dismissing "Talia" stays
 * dismissed even if the writer keeps typing and the producer
 * re-publishes her — until the writer accepts the suggestion (chip
 * disappears because she's now linked) or remounts the sheet.
 */
export function SceneAutosuggestChips({
  sceneId,
  alreadyLinkedIds,
  currentLocationId,
  onLinked,
}: Props) {
  const [suggestions, setSuggestions] = useState<ChipSuggestion[]>([]);
  const [dismissed, setDismissed] = useState<Set<string>>(new Set());

  useEffect(() => {
    return subscribeChips((sid, r) => {
      if (sid === sceneId) setSuggestions(r);
    });
  }, [sceneId]);

  const visible = suggestions.filter((s) => {
    const key = chipKey(s);
    if (dismissed.has(key)) return false;
    if (s.kind === "character") return !alreadyLinkedIds.has(s.characterId);
    return currentLocationId !== s.entryId;
  });

  if (visible.length === 0) return null;

  return (
    <div
      role="group"
      aria-label="Suggested"
      style={{ display: "flex", flexWrap: "wrap", gap: 6, marginBottom: 12 }}
    >
      <span
        style={{
          fontSize: "var(--water-fs-meta)",
          color: "var(--water-fg-muted)",
          alignSelf: "center",
        }}
      >
        Suggested:
      </span>
      {visible.map((s) => (
        <span
          key={chipKey(s)}
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: 2,
          }}
        >
          <button
            type="button"
            onClick={async () => {
              try {
                if (s.kind === "character") {
                  await ipc.characterLinkToScene(sceneId, s.characterId);
                } else {
                  await ipc.sceneSetLocation({
                    sceneId,
                    locationId: s.entryId,
                  });
                }
                onLinked();
              } catch {
                /* swallow — sheet stays open, writer can retry */
              }
            }}
            style={{
              border: "none",
              background: "transparent",
              color: "var(--water-fg-default)",
              cursor: "pointer",
              padding: "2px 6px",
              borderRadius: "var(--water-r-8)",
              fontSize: "var(--water-fs-meta)",
            }}
          >
            {chipLabel(s)}
          </button>
          <button
            type="button"
            aria-label={`Dismiss ${chipName(s)}`}
            onClick={() =>
              setDismissed((d) => {
                const next = new Set(d);
                next.add(chipKey(s));
                return next;
              })
            }
            style={{
              border: "none",
              background: "transparent",
              color: "var(--water-fg-muted)",
              cursor: "pointer",
              padding: "0 4px",
              fontSize: "var(--water-fs-meta)",
            }}
          >
            ×
          </button>
        </span>
      ))}
    </div>
  );
}

function chipKey(s: ChipSuggestion): string {
  return s.kind === "character" ? `char:${s.characterId}` : `world:${s.entryId}`;
}

function chipName(s: ChipSuggestion): string {
  return s.kind === "character" ? s.characterName : s.entryName;
}

function chipLabel(s: ChipSuggestion): string {
  if (s.kind === "character") {
    return `${s.characterName} (×${s.mentionCount})`;
  }
  return `📍 ${s.entryName}`;
}
