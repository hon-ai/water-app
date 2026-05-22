import { useCallback, useEffect, useState } from "react";
import { Sheet } from "../sheets/Sheet";
import { SceneAutosuggestChips } from "./SceneAutosuggestChips";
import {
  ipc,
  type CharacterIndexEntry,
  type SceneMetadata,
  type WorldEntryIndexEntry,
} from "../ipc/commands";
import { GlassSelect } from "../chrome/GlassSelect";

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
  // `null` here means "no `locations` segment exists in this project" —
  // the selector hides entirely. An empty array means "segment exists but
  // has no entries" — the selector still renders with just the
  // "— none —" option.
  const [locationOptions, setLocationOptions] =
    useState<WorldEntryIndexEntry[] | null>(null);

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

  // Load the `locations` segment + its entries once per open. The sheet
  // shows whatever entries exist at open time; a stub created via Chorus
  // pin (T29+) lands the next time the sheet is reopened.
  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    void (async () => {
      try {
        const segs = await ipc.worldSegmentList();
        const loc = segs.find((s) => s.slug === "locations");
        if (!loc) {
          if (!cancelled) setLocationOptions(null);
          return;
        }
        const entries = await ipc.worldEntryList(loc.id);
        if (cancelled) return;
        setLocationOptions(entries);
      } catch {
        if (!cancelled) setLocationOptions(null);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [open, sceneId]);

  const setLocation = useCallback(
    async (locationId: string | null) => {
      try {
        await ipc.sceneSetLocation({ sceneId, locationId });
        await reload();
      } catch {
        /* swallow */
      }
    },
    [sceneId, reload],
  );

  if (!meta) {
    return (
      <Sheet open={open} onClose={onClose} title="Scene details">
        <div role="status" className="water-loading">Loading</div>
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

  const saveSummary = async (next: string) => {
    try {
      await ipc.sceneSetSummary(sceneId, next.trim() === "" ? null : next);
      await reload();
    } catch {
      /* swallow */
    }
  };

  return (
    <Sheet open={open} onClose={onClose} title="Scene details">
      <SceneAutosuggestChips
        sceneId={sceneId}
        alreadyLinkedIds={linkedIds}
        currentLocationId={meta.location?.id ?? null}
        onLinked={() => void reload()}
      />
      <SummaryField
        initial={meta.summary ?? ""}
        onCommit={(s) => void saveSummary(s)}
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
          POV character
        </h3>
        <GlassSelect
          ariaLabel="POV character"
          value={meta.pov_character_id ?? ""}
          options={[
            { value: "", label: "— none —" },
            ...povOptions.map((c) => ({
              value: c.id,
              label: c.full_name || "(unnamed)",
            })),
          ]}
          onChange={(next) => void setPov(next === "" ? null : next)}
          triggerStyle={{ fontSize: "var(--water-fs-ui)" }}
        />
      </section>
      {locationOptions !== null && (
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
            Location
          </h3>
          <GlassSelect
            ariaLabel="Location"
            value={meta.location?.id ?? ""}
            options={[
              { value: "", label: "— none —" },
              ...locationOptions.map((opt) => ({
                value: opt.id,
                label: opt.name || "(unnamed)",
              })),
            ]}
            onChange={(next) => void setLocation(next === "" ? null : next)}
            triggerStyle={{ fontSize: "var(--water-fs-ui)" }}
          />
        </section>
      )}
    </Sheet>
  );
}

/**
 * Brief-summary editor with debounced auto-save. Local state holds
 * the textarea value; we commit on blur OR after 800ms of inactivity
 * so the writer's typing stream isn't interrupted by IPC round-trips.
 */
function SummaryField({
  initial,
  onCommit,
}: {
  initial: string;
  onCommit: (next: string) => void;
}) {
  const [value, setValue] = useState(initial);
  const [lastCommitted, setLastCommitted] = useState(initial);

  // Sync local state when the parent reloads (e.g., a different scene
  // opens). Compare against lastCommitted so user-in-progress edits
  // aren't clobbered by a stale reload.
  useEffect(() => {
    if (initial !== lastCommitted) {
      setValue(initial);
      setLastCommitted(initial);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [initial]);

  useEffect(() => {
    if (value === lastCommitted) return;
    const t = window.setTimeout(() => {
      onCommit(value);
      setLastCommitted(value);
    }, 800);
    return () => window.clearTimeout(t);
  }, [value, lastCommitted, onCommit]);

  return (
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
        Brief summary
      </h3>
      <textarea
        aria-label="Scene summary"
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onBlur={() => {
          if (value !== lastCommitted) {
            onCommit(value);
            setLastCommitted(value);
          }
        }}
        placeholder="What happens here? (visible on the macro canvas while you rearrange)"
        rows={3}
        style={{
          width: "100%",
          padding: "8px 10px",
          fontFamily: "var(--water-font-sans)",
          fontSize: "var(--water-fs-ui)",
          lineHeight: 1.5,
          color: "var(--water-fg-default)",
          background: "var(--water-bg-raised)",
          border:
            "1px solid color-mix(in srgb, var(--water-fg-faint) 18%, transparent)",
          borderRadius: "var(--water-r-8)",
          resize: "vertical",
          outline: "none",
        }}
      />
    </section>
  );
}
