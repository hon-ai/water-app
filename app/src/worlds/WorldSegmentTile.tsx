import { useEffect, useState } from "react";
import { ipc, type WorldSegment } from "../ipc/commands";

/**
 * One tile in the World Bible index grid (M4 T20).
 *
 * Two preview modes, chosen by `segment.is_collection`:
 *  - Collection (e.g. `locations`): show entry count + first two names.
 *    Backed by `ipc.worldEntryList(segmentId)` (server-computed previews
 *    aren't needed here — we just want a name peek).
 *  - Single-doc (e.g. `concept`, `culture`): show the first non-empty
 *    `[main]` field, truncated to 80 chars. Backed by
 *    `ipc.worldSingleDocRead(segmentId)`, which lazily materializes the
 *    empty row on first access — so the read is always non-null.
 *
 * Hue is applied via the per-segment `hue_token` CSS variable (set in
 * `theme/tokens.css` for the six built-ins and on user-created segments
 * by the segment-create flow). We expose it as a custom property
 * (`--tile-hue`) so the tile's own stylesheet can compose it with state
 * variants (hover/active) without re-threading the variable name.
 */
export function WorldSegmentTile({
  segment,
  onClick,
}: {
  segment: WorldSegment;
  onClick: () => void;
}) {
  const [preview, setPreview] = useState<string>("");

  useEffect(() => {
    let cancelled = false;
    if (segment.is_collection) {
      ipc.worldEntryList(segment.id).then((rows) => {
        if (cancelled) return;
        if (rows.length === 0) {
          setPreview("(no entries yet)");
        } else {
          const names = rows
            .slice(0, 2)
            .map((r) => r.name || "(unnamed)")
            .join(", ");
          const suffix = rows.length > 2 ? `, …` : "";
          setPreview(
            `${rows.length} ${rows.length === 1 ? "entry" : "entries"}: ${names}${suffix}`,
          );
        }
      });
    } else {
      ipc.worldSingleDocRead(segment.id).then((file) => {
        if (cancelled) return;
        const main = (file as Record<string, unknown>).main as
          | Record<string, unknown>
          | undefined;
        for (const v of Object.values(main ?? {})) {
          if (typeof v === "string" && v.trim().length > 0) {
            setPreview(v.length > 80 ? `${v.slice(0, 80)}…` : v);
            return;
          }
        }
        setPreview("(empty)");
      });
    }
    return () => {
      cancelled = true;
    };
  }, [segment.id, segment.is_collection]);

  return (
    <button
      className="world-segment-tile"
      style={{ ["--tile-hue" as string]: `var(${segment.hue_token})` }}
      onClick={onClick}
      data-testid={`segment-tile-${segment.slug || segment.id}`}
    >
      <div className="world-segment-tile-name">{segment.name}</div>
      <div className="world-segment-tile-preview">{preview}</div>
      <div className="world-segment-tile-icon">
        {segment.is_collection ? "▦" : "▢"}
      </div>
    </button>
  );
}
