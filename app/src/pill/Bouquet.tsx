import type { CSSProperties } from "react";
import { RefreshCw, Pin, X } from "lucide-react";
import { ipc } from "../ipc/commands";
import type { Pill } from "./types";

export interface BouquetItem {
  sub_pill_id: string;
  angle: "feel" | "notice" | "wonder";
  text: string;
}

interface Props {
  parentId: string;
  hueToken: string;
  items: BouquetItem[];
  onClose: () => void;
  onSubClick?: (item: BouquetItem) => void;
  /**
   * Full Pill record for the parent, used when the user clicks "pin".
   * Optional for M2: if absent we synthesise a minimal record from
   * `parentId`/`hueToken`. PillLayer passes the real Pill (it has the
   * top-level pill object), so top-level pins capture the correct text +
   * speaker. Sub-pill pins from RabbitHole currently fall back to the
   * synthesised shape until T26 plumbs sub-pill records.
   */
  pillForPinning?: Pill;
  /** Scene the pill belongs to. Threaded down from <EditorCanvas>;
   *  required for `ipc.pillPin` to satisfy the `pinned_pill.scene_id`
   *  FK constraint. Falls back to "" only for standalone tests. */
  sceneId?: string;
  /** Anchored block id (the manuscript block this pill reacted to).
   *  Used both as the pin's `block_id` and as the DOM selector for
   *  computing `snippet` at pin time. Falls back to "" when the pill is
   *  unanchored (e.g. deeper sub-pills without an explicit block). */
  blockId?: string;
}

const ANGLE_HUE_SHIFT: Record<BouquetItem["angle"], string> = {
  feel: "--water-hue-valence-pos",
  notice: "--water-hue-pace",
  wonder: "--water-hue-coherence",
};

/**
 * Expanded view of a pill: 3 angle-flavored sub-capsules + a controls row
 * (regenerate, pin, dismiss). Rendered in place of the parent capsule once
 * the `bouquet:ready` event arrives.
 *
 * Because the surrounding `PillLayer` is `pointer-events: none`, this
 * component's outer wrapper re-enables pointer events.
 */
export function Bouquet({
  parentId,
  hueToken,
  items,
  onClose,
  onSubClick,
  pillForPinning,
  sceneId = "",
  blockId = "",
}: Props) {
  const pinPayload: Pill =
    pillForPinning ?? {
      pill_id: parentId,
      speaker_id: "",
      hue_token: hueToken,
      text: "",
      block_target_id: null,
      trigger_id: "",
    };
  const wrapStyle: CSSProperties = {
    pointerEvents: "auto",
    display: "flex",
    flexDirection: "column",
    gap: 8,
    padding: 10,
    borderRadius: "var(--water-r-16)",
    background: `color-mix(in oklch, var(${hueToken}) 18%, var(--water-bg-paper))`,
    boxShadow: `0 0 24px color-mix(in oklch, var(${hueToken}) 40%, transparent)`,
    animation: "water-pill-fade-in var(--water-dur-small) var(--water-ease-out-soft) both",
  };

  const subCapsuleStyle = (angle: BouquetItem["angle"]): CSSProperties => {
    const accent = ANGLE_HUE_SHIFT[angle];
    return {
      padding: "6px 12px",
      borderRadius: "var(--water-r-16)",
      background: `color-mix(in oklch, var(${accent}) 30%, var(--water-bg-paper))`,
      boxShadow: `0 0 16px color-mix(in oklch, var(${accent}) 55%, transparent)`,
      color: "var(--water-fg-default)",
      fontFamily: "var(--water-font-sans)",
      fontSize: "var(--water-fs-body)",
      lineHeight: "var(--water-lh-body)",
      cursor: onSubClick ? "pointer" : "default",
      textAlign: "left",
      border: "none",
      width: "100%",
    };
  };

  const controlsRowStyle: CSSProperties = {
    display: "flex",
    flexDirection: "row",
    gap: 8,
    justifyContent: "flex-end",
    paddingTop: 4,
  };

  const iconBtnStyle: CSSProperties = {
    background: "transparent",
    border: "none",
    padding: 4,
    cursor: "pointer",
    color: "var(--water-fg-default)",
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
  };

  return (
    <div data-bouquet-parent-id={parentId} style={wrapStyle}>
      {items.map((item) => (
        <button
          key={item.sub_pill_id}
          type="button"
          data-sub-pill-id={item.sub_pill_id}
          data-angle={item.angle}
          onClick={() => onSubClick?.(item)}
          style={subCapsuleStyle(item.angle)}
        >
          {item.text}
        </button>
      ))}
      <div style={controlsRowStyle}>
        <button
          type="button"
          aria-label="Regenerate bouquet"
          onClick={() => {
            void ipc.pillRegenerate(parentId);
          }}
          style={iconBtnStyle}
        >
          <RefreshCw size={16} />
        </button>
        <button
          type="button"
          aria-label="Pin pill"
          onClick={() => {
            // Compute the snippet at click-time so it reflects the latest
            // editor state, not the props at mount. Look up the anchored
            // block element via the same [data-bid="..."] selector the
            // hover-dim glow line uses. If the block isn't in the DOM
            // (deeper sub-pills, or the block was deleted between emerge
            // and pin), pass "" — `pinned_pill.snippet` is NOT NULL but
            // accepts empty strings.
            let snippet = "";
            if (blockId.length > 0) {
              const el = document.querySelector(`[data-bid="${blockId}"]`);
              snippet = el?.textContent?.slice(0, 200) ?? "";
            }
            void ipc.pillPin(pinPayload, sceneId, blockId, snippet);
          }}
          style={iconBtnStyle}
        >
          <Pin size={16} />
        </button>
        <button
          type="button"
          aria-label="Dismiss pill"
          onClick={() => {
            void ipc.pillDismiss(parentId);
            onClose();
          }}
          style={iconBtnStyle}
        >
          <X size={16} />
        </button>
      </div>
    </div>
  );
}
