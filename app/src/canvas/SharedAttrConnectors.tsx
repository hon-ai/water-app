import { useMemo } from "react";
import type { CanvasCard } from "./CanvasSurface";
import { CARD_W, CARD_H } from "./SceneCard";

interface Props {
  cards: CanvasCard[];
  /** Bounding box for the SVG. Sized to comfortably cover the
   *  canvas-space content; the parent's transform handles pan/zoom. */
  width: number;
  height: number;
  /** Left/top of the SVG in canvas-space, so we can render at
   *  (minX, minY) without negative coords. */
  offsetX: number;
  offsetY: number;
}

/**
 * Draws faint cubic-Bezier arcs between any pair of scenes that
 * share at least one location or one character. Only canonical
 * (isPrimary) cards participate so each scene is one anchor point,
 * not N (avoids visual noise from ghosts).
 *
 * Two stroke colors per arc — half drawn in flow-hue (location
 * basis), half in character-hue (character basis) — so the writer
 * can tell at a glance whether a connection is "same setting" vs
 * "same cast." When a pair shares both, both halves render and they
 * read as a single richer arc.
 */
export function SharedAttrConnectors({
  cards,
  width,
  height,
  offsetX,
  offsetY,
}: Props) {
  const anchors = useMemo(
    () => cards.filter((c) => c.isPrimary),
    [cards],
  );

  const arcs = useMemo(() => {
    type Arc = {
      key: string;
      ax: number;
      ay: number;
      bx: number;
      by: number;
      sharesLoc: boolean;
      sharesChar: boolean;
    };
    const out: Arc[] = [];
    // O(N^2) — fine up to a few hundred scenes. Beyond that we'd
    // index by attribute id and connect within clusters.
    for (let i = 0; i < anchors.length; i++) {
      for (let j = i + 1; j < anchors.length; j++) {
        const a = anchors[i]!;
        const b = anchors[j]!;
        const aLocs = new Set(a.location_presences.map((p) => p.id));
        const bLocs = new Set(b.location_presences.map((p) => p.id));
        const aChars = new Set(a.character_presences.map((p) => p.id));
        const bChars = new Set(b.character_presences.map((p) => p.id));
        const sharesLoc = [...aLocs].some((id) => bLocs.has(id));
        const sharesChar = [...aChars].some((id) => bChars.has(id));
        if (!sharesLoc && !sharesChar) continue;
        out.push({
          key: `${a.placementKey}__${b.placementKey}`,
          ax: a.x + CARD_W / 2,
          ay: a.y + CARD_H / 2,
          bx: b.x + CARD_W / 2,
          by: b.y + CARD_H / 2,
          sharesLoc,
          sharesChar,
        });
      }
    }
    return out;
  }, [anchors]);

  if (arcs.length === 0) return null;

  /**
   * Cubic Bezier control points midway along the segment, offset
   * perpendicular by a fraction of the segment length. Gives every
   * arc a gentle bow so overlapping straight lines don't blur into
   * one stripe.
   */
  function arcPath(
    ax: number,
    ay: number,
    bx: number,
    by: number,
  ): string {
    const dx = bx - ax;
    const dy = by - ay;
    const len = Math.hypot(dx, dy);
    if (len === 0) return "";
    const bow = Math.min(80, len * 0.15);
    const nx = -dy / len;
    const ny = dx / len;
    const mx = (ax + bx) / 2 + nx * bow;
    const my = (ay + by) / 2 + ny * bow;
    return `M ${ax - offsetX} ${ay - offsetY} Q ${mx - offsetX} ${my - offsetY} ${bx - offsetX} ${by - offsetY}`;
  }

  return (
    <svg
      data-testid="canvas-shared-connectors"
      style={{
        position: "absolute",
        left: offsetX,
        top: offsetY,
        width,
        height,
        pointerEvents: "none",
        overflow: "visible",
      }}
    >
      {arcs.map((arc) => (
        <g key={arc.key}>
          {arc.sharesLoc && (
            <path
              d={arcPath(arc.ax, arc.ay, arc.bx, arc.by)}
              fill="none"
              stroke="color-mix(in srgb, var(--water-hue-flow) 70%, transparent)"
              strokeWidth={1}
              strokeOpacity={0.45}
              strokeDasharray={arc.sharesChar ? "none" : "4 4"}
            />
          )}
          {arc.sharesChar && !arc.sharesLoc && (
            <path
              d={arcPath(arc.ax, arc.ay, arc.bx, arc.by)}
              fill="none"
              stroke="color-mix(in srgb, var(--water-hue-character-2) 70%, transparent)"
              strokeWidth={1}
              strokeOpacity={0.45}
              strokeDasharray="2 5"
            />
          )}
        </g>
      ))}
    </svg>
  );
}
