import type { CSSProperties } from "react";
import type { Pill } from "./types";

interface Props {
  pill: Pill;
  onClick?: () => void;
}

/**
 * Known persona speaker slugs (lowercase). Anything else is treated as
 * a character id and displayed verbatim with title-casing rules below.
 * The set lives in this file so the chip-label formatter is purely local;
 * if more personas are added later, just append the slug here.
 */
const PERSONA_SLUGS: ReadonlySet<string> = new Set([
  "cartographer",
  "echo",
  "architect",
  "editor",
  "chorus",
]);

function formatSpeakerLabel(speakerId: string): string | null {
  if (!speakerId) return null;
  // Persona slugs are short, alphanumeric, and known to us: title-case.
  if (PERSONA_SLUGS.has(speakerId)) {
    return speakerId.charAt(0).toUpperCase() + speakerId.slice(1);
  }
  // ULID-shaped ids (26 uppercase alphanumeric chars) are character
  // speakers; we don't have a name registry in the capsule, so omit the
  // chip rather than show a noisy id. (Future: thread the display name
  // through the pill event so character voices get a chip too.)
  if (/^[0-9A-Z]{26}$/.test(speakerId)) {
    return null;
  }
  return speakerId;
}

/**
 * A single pastel-glow pill capsule.
 *
 * Visuals come from CSS custom properties driven by the speaker's hue token.
 * The `data-pill-id` and `data-block-target-id` attributes are read by T20
 * hover-anchor logic; the optional `onClick` is wired by T21's expand-to-
 * bouquet flow.
 *
 * Sizing is tuned to feel like a marginal note rather than a chat bubble:
 * meta-size text, narrow max-width, subdued padding. The speaker chip at
 * the top conveys author (Cartographer / Echo / etc.) at a glance — the
 * hue alone was not enough to distinguish personas during the M4 smoke walk.
 */
export function PillCapsule({ pill, onClick }: Props) {
  const speakerLabel = formatSpeakerLabel(pill.speaker_id);
  // Layered animation: the entry fade plays once; the persistent
  // breathe (defined in tokens.css on .water-pill) carries the ambient
  // gradient drift. Inline persona-tint shadow rides on top.
  const style: CSSProperties = {
    position: "relative",
    padding: "6px 10px",
    borderRadius: "var(--water-r-16)",
    boxShadow: `0 0 18px color-mix(in oklch, var(${pill.hue_token}) 50%, transparent)`,
    color: "var(--water-fg-default)",
    fontFamily: "var(--water-font-sans)",
    fontSize: "var(--water-fs-meta)",
    lineHeight: 1.4,
    maxWidth: 200,
    cursor: onClick ? "pointer" : "default",
    pointerEvents: "auto",
    // Two animations: the entry fade once, the breathe loop forever.
    // The breathe's background gradient comes from the .water-pill
    // class so prefers-reduced-motion can disable it cleanly.
    animationName: "water-pill-fade-in",
    animationDuration: "var(--water-dur-small)",
    animationTimingFunction: "var(--water-ease-out-soft)",
    animationFillMode: "both",
  };
  const chipStyle: CSSProperties = {
    display: "block",
    fontSize: 10,
    fontWeight: 600,
    textTransform: "uppercase",
    letterSpacing: 0.6,
    color: `color-mix(in oklch, var(${pill.hue_token}) 80%, var(--water-fg-default))`,
    marginBottom: 4,
    opacity: 0.85,
  };
  return (
    <div
      role="button"
      className="water-pill"
      data-pill-id={pill.pill_id}
      data-block-target-id={pill.block_target_id ?? ""}
      onClick={onClick}
      style={style}
    >
      {speakerLabel && (
        <div data-testid="pill-speaker-label" style={chipStyle}>
          {speakerLabel}
        </div>
      )}
      {pill.text}
    </div>
  );
}
