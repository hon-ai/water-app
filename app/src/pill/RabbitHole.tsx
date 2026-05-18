import type { CSSProperties } from "react";
import { Bouquet, type BouquetItem } from "./Bouquet";

export interface RabbitHoleLevel {
  parentId: string;
  parentText: string;
  items: BouquetItem[];
  chosenSubId: string | null;
}

interface Props {
  hueToken: string;
  path: RabbitHoleLevel[];
  onSubClick: (level: number, item: BouquetItem) => void;
  onClose: () => void;
}

/**
 * Unlimited-depth recursive bouquet view.
 *
 * `path` is the chain of expansions from the root pill down to the deepest
 * currently-open bouquet. Each level remembers which sub-pill was chosen so
 * the renderer can collapse its siblings to thin glow lines on the left edge.
 *
 * The deepest level is rendered as a full `<Bouquet>`; clicking one of its
 * sub-capsules calls `onSubClick(level, item)` so `<PillLayer>` can mark the
 * chosen sub and fire `ipc.pillExpand` for the next level.
 *
 * The top breadcrumb chain joins each level's parent text (truncated to 6
 * words) with " > ". The X (in the inner `<Bouquet>`) closes the whole
 * thread via `onClose`.
 */
export function RabbitHole({ hueToken, path, onSubClick, onClose }: Props) {
  // Siblings to collapse to glow lines: for each level except the last (the
  // currently-open one), every item whose sub_pill_id !== chosenSubId.
  const collapsedSiblings: Array<{ levelIdx: number; item: BouquetItem }> = [];
  for (let i = 0; i < path.length - 1; i++) {
    const lvl = path[i];
    if (!lvl) continue;
    for (const item of lvl.items) {
      if (item.sub_pill_id !== lvl.chosenSubId) {
        collapsedSiblings.push({ levelIdx: i, item });
      }
    }
  }

  const currentLevel = path[path.length - 1];
  if (!currentLevel) {
    // Should never happen in practice (PillLayer only mounts RabbitHole when
    // path.length >= 1), but the type system + noUncheckedIndexedAccess want
    // us to be explicit.
    return null;
  }

  const breadcrumb = path
    .map((lvl) => lvl.parentText.split(/\s+/).slice(0, 6).join(" "))
    .join(" \u203a "); // ›

  const glowLineStyle: CSSProperties = {
    width: 24,
    height: 2,
    background: `var(${hueToken})`,
    boxShadow: `0 0 6px var(${hueToken})`,
    opacity: 0.5,
    borderRadius: 2,
  };

  const breadcrumbStyle: CSSProperties = {
    fontFamily: "var(--water-font-sans)",
    fontSize: "var(--water-fs-meta)",
    color: "var(--water-fg-muted)",
    letterSpacing: 0.3,
  };

  return (
    <div style={{ display: "flex", gap: 8, pointerEvents: "auto" }}>
      {/* Left edge: collapsed sibling glow lines for every prior level. */}
      <div
        aria-label="collapsed siblings"
        style={{
          display: "flex",
          flexDirection: "column",
          gap: 4,
          alignItems: "flex-start",
          marginTop: 28,
        }}
      >
        {collapsedSiblings.map(({ levelIdx, item }) => (
          <div
            key={`${levelIdx}:${item.sub_pill_id}`}
            data-testid="water-glow-line"
            data-level={levelIdx}
            data-sub-pill-id={item.sub_pill_id}
            title={item.text}
            style={glowLineStyle}
          />
        ))}
      </div>

      <div style={{ display: "flex", flexDirection: "column", gap: 10, flex: 1 }}>
        {/* Breadcrumb chain across all levels. */}
        <div aria-label="Rabbit hole breadcrumb" style={breadcrumbStyle}>
          {breadcrumb}
        </div>

        {/* The deepest level is rendered as a full Bouquet. */}
        <Bouquet
          parentId={currentLevel.parentId}
          hueToken={hueToken}
          items={currentLevel.items}
          onClose={onClose}
          onSubClick={(item) => onSubClick(path.length - 1, item)}
        />
      </div>
    </div>
  );
}
