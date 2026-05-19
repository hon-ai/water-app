import { useEffect, useState } from "react";
import { ipc, type AutosuggestResult } from "../ipc/commands";
import { subscribeAutosuggest } from "./sceneMetadataChannel";

interface Props {
  sceneId: string;
  /**
   * Characters already linked to this scene. Used to filter out chips
   * for characters the writer has already accepted — once present,
   * suggestion is redundant. The parent (SceneMetadataSheet) recomputes
   * this set after each link/unlink, so re-renders naturally hide
   * just-accepted chips.
   */
  alreadyLinkedIds: Set<string>;
  /** Fired after a chip's link succeeds so the parent can reload meta. */
  onLinked: () => void;
}

/**
 * Row of "Suggested present" chips fed by `subscribeAutosuggest`. Each
 * chip has two affordances: clicking the name links the character to
 * the scene; clicking the × dismisses the suggestion locally (no IPC
 * call, no persistence — the suggestion will re-appear if the autosave
 * loop publishes the same character on the next debounce).
 *
 * The dismiss set is keyed by character id and lives only in component
 * state. It's intentionally NOT cleared when the chip set changes:
 * dismissing "Talia" should stay dismissed even if the writer keeps
 * typing and the autosave loop re-publishes her — until the writer
 * either accepts the suggestion (chip disappears because she's now
 * linked) or remounts the sheet.
 */
export function SceneAutosuggestChips({
  sceneId,
  alreadyLinkedIds,
  onLinked,
}: Props) {
  const [results, setResults] = useState<AutosuggestResult[]>([]);
  const [dismissed, setDismissed] = useState<Set<string>>(new Set());

  useEffect(() => {
    return subscribeAutosuggest((sid, r) => {
      if (sid === sceneId) setResults(r);
    });
  }, [sceneId]);

  const visible = results.filter(
    (r) => !alreadyLinkedIds.has(r.character_id) && !dismissed.has(r.character_id),
  );

  if (visible.length === 0) return null;

  return (
    <div
      role="group"
      aria-label="Suggested characters"
      style={{ display: "flex", flexWrap: "wrap", gap: 6, marginBottom: 12 }}
    >
      <span
        style={{
          fontSize: "var(--water-fs-meta)",
          color: "var(--water-fg-muted)",
          alignSelf: "center",
        }}
      >
        Suggested present:
      </span>
      {visible.map((r) => (
        <span
          key={r.character_id}
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
                await ipc.characterLinkToScene(sceneId, r.character_id);
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
            {r.full_name} (×{r.mention_count})
          </button>
          <button
            type="button"
            aria-label={`Dismiss ${r.full_name}`}
            onClick={() =>
              setDismissed((d) => {
                const next = new Set(d);
                next.add(r.character_id);
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
