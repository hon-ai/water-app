import { ipc } from "../ipc/commands";
import { Sheet } from "../sheets/Sheet";
import type { Pill } from "./types";

interface Props {
  pill: Pill;
  onClose: () => void;
}

/**
 * Detail sheet for a single pinned pill. Shows the pill's full message in a
 * glow capsule plus speaker + trigger metadata, with an "Un-pin" action.
 *
 * Un-pin calls `pill_dismiss`, which deletes the row from `pinned_pill` and
 * emits `pill:unpinned`. The sheet then closes; `PinnedColumn` reacts to the
 * event and removes the dot.
 */
export function PinnedPillDetail({ pill, onClose }: Props) {
  const onUnpin = () => {
    void ipc.pillDismiss(pill.pill_id);
    onClose();
  };

  return (
    <Sheet open onClose={onClose} title="Pinned pill">
      <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
        <div
          data-testid="water-pinned-pill-capsule"
          style={{
            padding: "10px 14px",
            borderRadius: "var(--water-r-16)",
            background: `color-mix(in oklch, var(${pill.hue_token}) 30%, var(--water-bg-paper))`,
            boxShadow: `0 0 24px color-mix(in oklch, var(${pill.hue_token}) 55%, transparent)`,
            color: "var(--water-fg-default)",
            fontFamily: "var(--water-font-sans)",
            fontSize: "var(--water-fs-body)",
            lineHeight: "var(--water-lh-body)",
          }}
        >
          {pill.text}
        </div>
        <dl
          style={{
            margin: 0,
            display: "grid",
            gridTemplateColumns: "auto 1fr",
            gap: "4px 12px",
            color: "var(--water-fg-muted)",
            fontSize: "var(--water-fs-meta)",
          }}
        >
          <dt>Speaker</dt>
          <dd style={{ margin: 0 }}>{pill.speaker_id}</dd>
          <dt>Trigger</dt>
          <dd style={{ margin: 0 }}>{pill.trigger_id || "—"}</dd>
        </dl>
        <button
          type="button"
          onClick={onUnpin}
          style={{
            padding: "8px 16px",
            border: "1px solid color-mix(in srgb, var(--water-fg-faint) 30%, transparent)",
            borderRadius: "var(--water-r-8)",
            background: "transparent",
            color: "var(--water-fg-default)",
            fontFamily: "var(--water-font-sans)",
            fontSize: "var(--water-fs-body)",
            cursor: "pointer",
            alignSelf: "flex-start",
          }}
        >
          Un-pin
        </button>
      </div>
    </Sheet>
  );
}
