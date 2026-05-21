import type { HeatMetricKind, HeatReadResponse } from "../ipc/commands";
import { HeatmapStripView, metricHue } from "../heat/HeatmapStripView";
import type { CanvasCard } from "./CanvasSurface";

export const CARD_W = 200;
export const CARD_H = 100;

/**
 * Scene-level intensity for one metric. Used as the card's overall
 * hue tint so the canvas reads at a glance — bright pacing here,
 * dim pacing there. Returns 0 when there's no data; the card stays
 * neutral until heat compute runs.
 */
function avgMetricIntensity(
  metrics: HeatReadResponse["metrics"] | null,
  kind: HeatMetricKind,
): number {
  if (!metrics) return 0;
  const rows = metrics[kind];
  if (!rows || rows.length === 0) return 0;
  const mean = rows.reduce((acc, r) => acc + r.value, 0) / rows.length;
  if (kind === "valence") return Math.min(1, Math.abs(mean));
  return Math.max(0, Math.min(1, mean));
}

interface Props {
  card: CanvasCard;
  metrics: HeatReadResponse["metrics"] | null;
  activeMetric: HeatMetricKind;
  onPointerDown: (e: React.PointerEvent) => void;
  onOpen: () => void;
}

/**
 * One card on the macro spatial canvas.
 *
 * Click → opens the scene in the editor. Drag (via pointerdown
 * captured by the parent CanvasSurface) moves the card.
 *
 * Heat sparkline: a small instance of `HeatmapStripView` showing
 * just the active metric — at this size (60 × 8) the strip becomes
 * an at-a-glance bar code of the scene's shape.
 */
export function SceneCard({
  card,
  metrics,
  activeMetric,
  onPointerDown,
  onOpen,
}: Props) {
  const displayName = card.name.trim() === "" ? "(untitled)" : card.name;
  // Card-level hue tint: cards with high active-metric values feel
  // heavier; quiet scenes recede. Mixed against bg-raised at up to
  // ~24% intensity so the card stays readable.
  const intensity = avgMetricIntensity(metrics, activeMetric);
  const hue = metricHue(activeMetric);
  const tintPct = Math.round(intensity * 28); // 0..28%
  // Ghosts are secondary placements of a scene in lanes other than
  // its primary. Rendered faded with a dotted border so the writer
  // sees presence at a glance but knows which card is the source of
  // truth. Click jumps focus to the primary.
  const ghost = !card.isPrimary;
  // Spanning cards cover several lane rows. Detect by height > the
  // single-row card height. They get a left rail in flow-hue so the
  // ensemble shape reads at a glance.
  const isSpanning = card.height > CARD_H + 1;
  // Collision stack: when N cards land on the same snapped cell,
  // fan them slightly so each is visible. The top card shows a
  // count badge so the writer knows others are hidden behind. The
  // fan grows up-and-to-the-right; subtle enough not to disturb
  // reading order, visible enough to invite a click.
  const stacked = card.stackSize > 1;
  const stackOffsetX = stacked ? card.stackIndex * 10 : 0;
  const stackOffsetY = stacked ? card.stackIndex * -8 : 0;
  const isStackTop = stacked && card.stackIndex === card.stackSize - 1;
  const baseBorder = ghost
    ? "1px dashed color-mix(in srgb, var(--water-fg-faint) 35%, transparent)"
    : "1px solid color-mix(in srgb, var(--water-fg-faint) 10%, transparent)";
  return (
    <div
      data-testid={`scene-card-${card.placementKey}`}
      data-scene-id={card.id}
      data-ghost={ghost ? "true" : "false"}
      onPointerDown={onPointerDown}
      onClick={(e) => {
        // Distinguish click from drag-end. Drag-end fires pointerup
        // on the container, not click on the card, so this fires
        // only on actual click.
        e.stopPropagation();
        onOpen();
      }}
      style={{
        position: "absolute",
        left: card.x + stackOffsetX,
        top: card.y + stackOffsetY,
        width: CARD_W,
        height: card.height,
        zIndex: stacked ? card.stackIndex + 1 : "auto",
        opacity: ghost ? 0.36 : 1,
        background: `color-mix(in oklch, var(${hue}) ${tintPct}%, var(--water-bg-raised))`,
        border: baseBorder,
        borderRadius: "var(--water-r-16)",
        boxShadow: ghost ? "none" : "var(--water-elev-1)",
        cursor: ghost ? "pointer" : "grab",
        padding: "10px 12px",
        display: "flex",
        flexDirection: "column",
        gap: 6,
        fontFamily: "var(--water-font-sans)",
        userSelect: "none",
        transition:
          "box-shadow var(--water-dur-tiny) var(--water-ease-out-soft), border-color var(--water-dur-tiny) var(--water-ease-out-soft), opacity var(--water-dur-tiny) var(--water-ease-out-soft)",
        // Spanning cards: hint the multi-lane shape with a thin left
        // rail of the flow hue. Subtle but reads instantly.
        borderLeft: isSpanning
          ? "3px solid color-mix(in srgb, var(--water-hue-flow) 55%, transparent)"
          : baseBorder.startsWith("1px dashed")
            ? baseBorder
            : baseBorder,
      }}
      onMouseEnter={(e) => {
        if (!ghost) {
          e.currentTarget.style.boxShadow = "var(--water-elev-2)";
          e.currentTarget.style.borderColor =
            "color-mix(in srgb, var(--water-hue-flow) 30%, transparent)";
        } else {
          e.currentTarget.style.opacity = "0.55";
        }
      }}
      onMouseLeave={(e) => {
        if (!ghost) {
          e.currentTarget.style.boxShadow = "var(--water-elev-1)";
          e.currentTarget.style.borderColor =
            "color-mix(in srgb, var(--water-fg-faint) 10%, transparent)";
        } else {
          e.currentTarget.style.opacity = "0.36";
        }
      }}
    >
      {/* Stack count badge: only on the top card of a collision
          stack. Tells the writer "N scenes share this cell" so they
          know to spread/inspect the pile. */}
      {isStackTop && (
        <div
          aria-label={`${card.stackSize} scenes stacked here`}
          style={{
            position: "absolute",
            top: -8,
            right: -8,
            minWidth: 22,
            height: 22,
            padding: "0 6px",
            borderRadius: 11,
            background: "var(--water-fg-default)",
            color: "var(--water-bg-paper)",
            fontFamily: "var(--water-font-sans)",
            fontSize: 11,
            fontWeight: 600,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            boxShadow: "var(--water-elev-2)",
            pointerEvents: "none",
          }}
        >
          +{card.stackSize - 1}
        </div>
      )}
      <div
        style={{
          fontFamily: "var(--water-font-serif)",
          fontSize: "var(--water-fs-title)",
          lineHeight: 1.1,
          fontWeight: 500,
          letterSpacing: -0.2,
          color: "var(--water-fg-default)",
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}
      >
        {displayName}
      </div>
      <div
        aria-hidden
        style={{
          height: 1,
          width: 32,
          background:
            "color-mix(in srgb, var(--water-fg-faint) 35%, transparent)",
        }}
      />
      {/* Brief summary — what happens in the scene. Lets the writer
          gauge event order at a glance when rearranging cards. */}
      {card.summary && card.summary.trim() && (
        <div
          style={{
            fontSize: 11,
            lineHeight: 1.4,
            color: "var(--water-fg-muted)",
            overflow: "hidden",
            display: "-webkit-box",
            WebkitLineClamp: 2,
            WebkitBoxOrient: "vertical",
          }}
        >
          {card.summary}
        </div>
      )}
      <div style={{ height: 8 }}>
        {metrics && (
          <HeatmapStripView
            metrics={metrics}
            activeKinds={[activeMetric]}
            height={8}
            borderRadius="var(--water-r-8)"
            ariaLabel={`${displayName} ${activeMetric} heatmap`}
          />
        )}
      </div>
      <div
        style={{
          marginTop: "auto",
          fontSize: 11,
          color: "var(--water-fg-faint)",
        }}
      >
        {card.word_count.toLocaleString()} words
        {card.canvas_group && ` · ${card.canvas_group}`}
      </div>
    </div>
  );
}
