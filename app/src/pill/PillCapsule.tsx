import type { CSSProperties } from "react";
import type { Pill } from "./types";

interface Props {
  pill: Pill;
  onClick?: () => void;
}

/**
 * A single pastel-glow pill capsule.
 *
 * Visuals come from CSS custom properties driven by the speaker's hue token.
 * The `data-pill-id` and `data-block-target-id` attributes are read by T20
 * hover-anchor logic; the optional `onClick` is wired by T21's expand-to-
 * bouquet flow.
 */
export function PillCapsule({ pill, onClick }: Props) {
  const style: CSSProperties = {
    position: "relative",
    padding: "8px 14px",
    borderRadius: "var(--water-r-16)",
    background: `color-mix(in oklch, var(${pill.hue_token}) 35%, var(--water-bg-paper))`,
    boxShadow: `0 0 24px color-mix(in oklch, var(${pill.hue_token}) 60%, transparent)`,
    color: "var(--water-fg-default)",
    fontFamily: "var(--water-font-sans)",
    fontSize: "var(--water-fs-body)",
    lineHeight: "var(--water-lh-body)",
    maxWidth: 220,
    cursor: onClick ? "pointer" : "default",
    pointerEvents: "auto",
    animation: "water-pill-fade-in var(--water-dur-small) var(--water-ease-out-soft) both",
  };
  return (
    <div
      role="button"
      data-pill-id={pill.pill_id}
      data-block-target-id={pill.block_target_id ?? ""}
      onClick={onClick}
      style={style}
    >
      {pill.text}
    </div>
  );
}
