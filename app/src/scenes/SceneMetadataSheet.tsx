import { useCallback, useEffect, useState } from "react";
import { Sheet } from "../sheets/Sheet";
import { SceneAutosuggestChips } from "./SceneAutosuggestChips";
import {
  ipc,
  type CharacterIndexEntry,
  type SceneMetadata,
} from "../ipc/commands";

interface Props {
  sceneId: string;
  open: boolean;
  onClose: () => void;
}

/**
 * Right-edge sheet for editing scene-level character metadata
 * (`characters_present` + `pov_character_id`). Reads both the per-scene
 * meta and the full character index on open so the checkbox list can
 * show every character — linked or not — and the POV select can offer
 * only the linked subset (spec § 20: POV must be in `characters_present`).
 *
 * All mutations route through `ipc.characterLinkToScene` /
 * `characterUnlinkFromScene` / `characterSetPov`. Each mutation is
 * followed by a `reload()` so the local view stays consistent with
 * disk; the per-scene write lock on the Rust side prevents tearing if
 * the writer fires several toggles quickly.
 */
export function SceneMetadataSheet({ sceneId, open, onClose }: Props) {
  const [allChars, setAllChars] = useState<CharacterIndexEntry[]>([]);
  const [meta, setMeta] = useState<SceneMetadata | null>(null);

  const reload = useCallback(async () => {
    try {
      const [chars, m] = await Promise.all([
        ipc.characterList(),
        ipc.sceneReadMetadata(sceneId),
      ]);
      setAllChars(chars);
      setMeta(m);
    } catch {
      /* swallow — sheet shows last-known state */
    }
  }, [sceneId]);

  // Initial + scene-switch load. Cancellation guard mirrors the
  // CharacterIntakeSheet pattern (M3 T16): if `sceneId` changes mid-load,
  // drop the stale results rather than letting them clobber the new
  // scene's state.
  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    void (async () => {
      try {
        const [chars, m] = await Promise.all([
          ipc.characterList(),
          ipc.sceneReadMetadata(sceneId),
        ]);
        if (cancelled) return;
        setAllChars(chars);
        setMeta(m);
      } catch {
        /* swallow */
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [open, sceneId]);

  if (!meta) {
    return (
      <Sheet open={open} onClose={onClose} title="Scene details">
        <div role="status">Loading…</div>
      </Sheet>
    );
  }

  const linkedIds = new Set(meta.characters_present);

  const toggleLink = async (charId: string) => {
    try {
      if (linkedIds.has(charId)) {
        await ipc.characterUnlinkFromScene(sceneId, charId);
      } else {
        await ipc.characterLinkToScene(sceneId, charId);
      }
      await reload();
    } catch {
      /* swallow */
    }
  };

  const setPov = async (charId: string | null) => {
    try {
      await ipc.characterSetPov(sceneId, charId);
      await reload();
    } catch {
      /* swallow */
    }
  };

  // POV select only offers characters present in this scene. Spec § 20
  // requires the POV to be in `characters_present`; the Rust command
  // rejects out-of-set POVs at the boundary, but filtering the option
  // list keeps the UI from offering invalid choices in the first place.
  const povOptions = allChars.filter((c) => linkedIds.has(c.id));

  return (
    <Sheet open={open} onClose={onClose} title="Scene details">
      <SceneAutosuggestChips
        sceneId={sceneId}
        alreadyLinkedIds={linkedIds}
        onLinked={() => void reload()}
      />
      <section style={{ marginBottom: 16 }}>
        <h3
          style={{
            margin: "0 0 8px 0",
            fontFamily: "var(--water-font-sans)",
            fontSize: "var(--water-fs-ui)",
            fontWeight: 500,
            color: "var(--water-fg-muted)",
          }}
        >
          Characters present
        </h3>
        <ul style={{ listStyle: "none", padding: 0, margin: 0 }}>
          {allChars.map((c) => (
            <li key={c.id} style={{ padding: "4px 0" }}>
              <label
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 8,
                  cursor: "pointer",
                  fontSize: "var(--water-fs-ui)",
                }}
              >
                <input
                  type="checkbox"
                  checked={linkedIds.has(c.id)}
                  onChange={() => void toggleLink(c.id)}
                />
                {c.full_name ? (
                  c.full_name
                ) : (
                  <em style={{ color: "var(--water-fg-faint)" }}>(unnamed)</em>
                )}
              </label>
            </li>
          ))}
        </ul>
      </section>
      <section>
        <h3
          style={{
            margin: "0 0 8px 0",
            fontFamily: "var(--water-font-sans)",
            fontSize: "var(--water-fs-ui)",
            fontWeight: 500,
            color: "var(--water-fg-muted)",
          }}
        >
          POV character
        </h3>
        <select
          aria-label="POV character"
          value={meta.pov_character_id ?? ""}
          onChange={(e) =>
            void setPov(e.target.value === "" ? null : e.target.value)
          }
          style={{
            fontFamily: "var(--water-font-sans)",
            fontSize: "var(--water-fs-ui)",
            padding: "4px 8px",
            borderRadius: "var(--water-r-8)",
            border:
              "1px solid color-mix(in srgb, var(--water-fg-faint) 30%, transparent)",
            background: "var(--water-bg-paper)",
            color: "var(--water-fg-default)",
          }}
        >
          <option value="">— none —</option>
          {povOptions.map((c) => (
            <option key={c.id} value={c.id}>
              {c.full_name || "(unnamed)"}
            </option>
          ))}
        </select>
      </section>
    </Sheet>
  );
}
