import type { CSSProperties } from "react";
import type { WorldEntryIndexEntry } from "../ipc/commands";

/**
 * Card tile for one collection-entry row (M4 T22).
 *
 * `hueToken` is the parent segment's `hue_token` (e.g.
 * `"--water-hue-world-2"`); we expose it as a `--card-hue` CSS custom
 * property so the entry-card stylesheet can theme borders/accents off the
 * segment's color without prop-drilling raw color values.
 *
 * Empty names render as `(unnamed)` so freshly-created draft entries
 * (from the `+ New entry` button) still have a clickable affordance
 * before the user has filled them in.
 */
export function WorldEntryCard({
  entry,
  hueToken,
  onClick,
}: {
  entry: WorldEntryIndexEntry;
  hueToken: string;
  onClick: () => void;
}) {
  const displayName = entry.name.trim() === "" ? "(unnamed)" : entry.name;
  // Cast through `CSSProperties & Record<string, string>` so the custom
  // property doesn't trip TS's strict `CSSProperties` index check.
  const style: CSSProperties = {
    ["--card-hue" as never]: `var(${hueToken})`,
  };
  return (
    <button
      className="world-entry-card"
      style={style}
      onClick={onClick}
      data-testid={`entry-card-${entry.id}`}
    >
      <div className="world-entry-card-name">{displayName}</div>
      <div className="world-entry-card-preview">{entry.preview}</div>
    </button>
  );
}
