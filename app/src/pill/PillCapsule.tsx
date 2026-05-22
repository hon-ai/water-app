import type { CSSProperties } from "react";
import { ChevronDown } from "lucide-react";
import type { Pill, PillContentSignal } from "./types";

interface Props {
  pill: Pill;
  onClick?: () => void;
  /**
   * Display name for character speakers. When the speaker is a
   * persona (echo/architect/editor/cartographer/chorus) the chip
   * uses a fixed glyph and this prop is ignored.
   *
   * Optional: when omitted, character speakers fall back to a single
   * dot inside the chip (the persona hue still reads). Threading the
   * character index from the editor is a Phase 6 prompt-payload
   * concern — for Phase 3 we accept the graceful fallback.
   */
  speakerName?: string;
  /**
   * When true the pill renders a small neutral pip near the chip,
   * signalling "anchor drifted" — the original trigger phrase
   * couldn't be located, so the highlight fell back to the whole
   * paragraph (UX_SPEC.md §C.6.b tier 4). Soft visual cue, not an
   * error state; the pill still works.
   */
  anchorDrifted?: boolean;
}

/**
 * Persona slug → single-glyph monogram for the speaker chip. The
 * mapping is from UX_SPEC.md §C.1: E/A/D/C/H for
 * Echo/Architect/Editor/Cartographer/Chorus.
 */
const PERSONA_GLYPH: Record<string, string> = {
  echo: "E",
  architect: "A",
  editor: "D",
  cartographer: "C",
  chorus: "H",
};

const PERSONA_FULL_NAME: Record<string, string> = {
  echo: "Echo",
  architect: "Architect",
  editor: "Editor",
  cartographer: "Cartographer",
  chorus: "Chorus",
};

function isPersona(speakerId: string): boolean {
  return speakerId in PERSONA_GLYPH;
}

function isUlid(speakerId: string): boolean {
  return /^[0-9A-Z]{26}$/.test(speakerId);
}

/**
 * Pick the chip glyph for a speaker. Personas resolve to their fixed
 * letter; character speakers (ULID-shaped ids) use the first letter
 * of `speakerName` if provided, else a centered dot.
 */
function chipGlyph(speakerId: string, speakerName: string | undefined): string {
  if (isPersona(speakerId)) return PERSONA_GLYPH[speakerId]!;
  if (speakerName && speakerName.trim().length > 0) {
    return speakerName.trim().charAt(0).toUpperCase();
  }
  if (isUlid(speakerId)) return "·"; // middle dot
  // Fallback: an unknown short label — use its first char.
  return speakerId.charAt(0).toUpperCase();
}

function chipAriaLabel(
  speakerId: string,
  speakerName: string | undefined,
): string {
  if (isPersona(speakerId)) return PERSONA_FULL_NAME[speakerId]!;
  if (speakerName && speakerName.trim().length > 0) return speakerName;
  return "speaker";
}

/**
 * A single matte-glass pill capsule. Phase 3 anatomy
 * (UX_SPEC.md §C.1–§C.4):
 *
 *   ┌──────────────────────────────────────────────┐
 *   │ ●  she's still avoiding his eyes.            │
 *   │    something she doesn't want him to see.  ▾ │
 *   └──────────────────────────────────────────────┘
 *
 *   ●  — 12×12 speaker chip (persona-hued; glyph or character monogram)
 *   │  — content-signal left rail (absent for `observation`)
 *   ▾  — chevron-down affordance, hover-revealed; clicking the pill
 *        opens the rabbit hole / bouquet.
 *
 * The pill is smaller than the M4 capsule by ~15%; the dominant hue
 * outside the chip is gone, freeing the outer surface to carry the
 * content signal instead of the speaker identity.
 */
export function PillCapsule({
  pill,
  onClick,
  speakerName,
  anchorDrifted,
}: Props) {
  const signal: PillContentSignal = pill.content_signal ?? "observation";
  const persona = isPersona(pill.speaker_id);

  // The chip hue is driven by the pill's `hue_token` (already
  // persona-hued for personas, character-hued for character speakers).
  // Expose it to CSS via a custom property so .water-pill-chip can
  // compose it with bg-paper for the matte interior. The fallback
  // (`--water-sea-300`) kicks in if hue_token is missing.
  const style: CSSProperties = {
    position: "relative",
    // Inner padding gives the persona label visible breathing room
    // from the rounded corner — previously 5px/8px (top/left) made
    // the label "CHORUS" / "ECHO" feel crammed against the corner.
    // Right pad still reserves space for the hover chevron.
    padding: "11px 20px 11px 14px",
    borderRadius: "var(--water-r-16)",
    color: "var(--water-fg-default)",
    fontFamily: "var(--water-font-sans)",
    fontSize: 11,
    lineHeight: 1.4,
    // Fill the container up to 220px; without `width: 100%` the
    // capsule renders at its content's natural width and overflows
    // when the parent pill column has been clamped narrow by a
    // small viewport.
    width: "100%",
    maxWidth: 220,
    boxSizing: "border-box",
    // Wrap the message instead of letting a long word push past
    // the right edge.
    overflowWrap: "anywhere",
    wordBreak: "break-word",
    cursor: onClick ? "pointer" : "default",
    pointerEvents: "auto",
    // Persona glow stays — the existing .water-pill class already
    // composes this into its box-shadow. It's now a soft accent of
    // *speaker* identity that lives inside the chip's hue ring.
    ["--water-pill-glow" as never]: `0 0 14px color-mix(in oklch, var(${pill.hue_token}) 32%, transparent)`,
    ["--water-pill-chip-hue" as never]: `var(${pill.hue_token})`,
  };

  const rowStyle: CSSProperties = {
    display: "flex",
    alignItems: "flex-start",
    gap: 6,
  };

  // Speaker chip — 12×12 disc, glyph centered. Title attribute carries
  // the full speaker name so a slow hover surfaces it (accessibility
  // backup; the visual chevron is the *interactive* affordance).
  const speakerLabel = chipAriaLabel(pill.speaker_id, speakerName);
  const glyph = chipGlyph(pill.speaker_id, speakerName);

  return (
    <div
      role="button"
      tabIndex={0}
      className="water-pill"
      data-pill-id={pill.pill_id}
      data-block-target-id={pill.block_target_id ?? ""}
      data-content-signal={signal}
      data-speaker-kind={persona ? "persona" : "character"}
      onClick={onClick}
      onKeyDown={(e) => {
        if ((e.key === "Enter" || e.key === " ") && onClick) {
          e.preventDefault();
          onClick();
        }
      }}
      style={style}
    >
      {/* Persona label — small uppercase row at the very top of the
          pill. Names the speaker so the writer can see at a glance
          who's talking (the chip glyph alone doesn't carry the
          name). Hue tinted to match the chip backing for soft
          identity reinforcement. */}
      <div
        data-testid="pill-persona-label"
        style={{
          fontSize: 9,
          fontWeight: 700,
          textTransform: "uppercase",
          letterSpacing: 0.9,
          lineHeight: 1,
          // Slightly more space between the label and the message
          // row below it. Combined with the bumped outer padding,
          // the label now reads as a settled header rather than
          // crammed into the corner.
          marginBottom: 7,
          color: `color-mix(in oklch, var(${pill.hue_token}) 78%, var(--water-fg-muted))`,
        }}
      >
        {speakerLabel}
      </div>
      <div style={rowStyle}>
        <span
          className="water-pill-chip"
          data-testid="pill-speaker-chip"
          aria-label={speakerLabel}
          title={speakerLabel}
          // Chip vertical alignment: nudge down so it sits on the
          // baseline of the first text line rather than its cap.
          style={{ marginTop: 2, position: "relative" }}
        >
          {glyph}
          {anchorDrifted && (
            <span
              data-testid="pill-anchor-drifted"
              aria-label="anchor drifted"
              title="anchor drifted"
              style={{
                position: "absolute",
                right: -2,
                bottom: -2,
                width: 5,
                height: 5,
                borderRadius: "50%",
                background: "var(--water-fg-faint)",
                boxShadow:
                  "0 0 0 1px color-mix(in srgb, var(--water-bg-paper) 70%, transparent)",
              }}
            />
          )}
        </span>
        <span style={{ flex: 1, minWidth: 0 }}>{pill.text}</span>
      </div>
      <span className="water-pill-chevron" aria-hidden>
        <ChevronDown size={12} strokeWidth={1.75} />
      </span>
    </div>
  );
}
