import { useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import { ipc, type HeatMetricKind } from "../ipc/commands";

interface Props {
  open: boolean;
  enabled: Partial<Record<HeatMetricKind, boolean>>;
  onToggle: (kind: HeatMetricKind, enabled: boolean) => void;
  onClose: () => void;
  /**
   * The chip button's bounding rect at the moment the picker opened.
   * Used to position the picker beneath the chip. Passing the rect (vs.
   * a ref) lets the parent compute it once on open — the picker stays
   * pinned where it appeared even if the strip later re-renders.
   */
  anchor: DOMRect | null;
  /**
   * Ref to the trigger button. Clicks on it must NOT be treated as
   * outside-clicks — otherwise the picker's outside-mousedown handler
   * closes the picker immediately before the button's onClick toggles
   * it, resulting in a re-open. Excluding the trigger lets the toggle
   * land cleanly.
   */
  triggerRef?: React.RefObject<HTMLElement | null>;
}

/**
 * Five-row popover anchored to the strip's right-edge metric chip.
 * Each row is a checkbox + name + one-line description; toggling
 * persists to the project settings via `ipc.heatSetMetricEnabled`.
 *
 * Optimistic UI: the local `enabled` map is updated first, the IPC
 * write is fire-and-forget — failures are swallowed silently, since
 * a botched settings write just means the toggle won't survive
 * restart, not that the strip stops working.
 */
export function HeatmapMetricPicker({
  open,
  enabled,
  onToggle,
  onClose,
  anchor,
  triggerRef,
}: Props) {
  const ref = useRef<HTMLDivElement | null>(null);

  // Close on outside click + Escape. Clicks on the trigger button are
  // excluded so the trigger's own onClick can toggle the picker shut
  // (without this guard, this handler closes first and the button
  // re-opens it).
  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      const target = e.target as Node;
      if (ref.current && ref.current.contains(target)) return;
      if (triggerRef?.current && triggerRef.current.contains(target)) return;
      onClose();
    };
    const esc = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("mousedown", handler);
    window.addEventListener("keydown", esc);
    return () => {
      window.removeEventListener("mousedown", handler);
      window.removeEventListener("keydown", esc);
    };
  }, [open, onClose, triggerRef]);

  if (!open || !anchor) return null;

  // Portal into document.body so the picker escapes every editor /
  // canvas / scroll-area stacking context. Position fixed against the
  // chip's bounding rect at open time. Right-aligned to the chip's
  // right edge.
  const PICKER_WIDTH = 300;
  const top = anchor.bottom + 6;
  const left = Math.max(8, anchor.right - PICKER_WIDTH);

  // React portals preserve event bubbling through the React tree, so
  // pointer events fired inside the picker still bubble UP to the
  // parent strip — whose onPointerDown captures the pointer and
  // eats the checkbox click. Stop propagation at the picker root.
  const stopProp = (e: React.SyntheticEvent) => {
    e.stopPropagation();
  };

  return createPortal(
    <div
      ref={ref}
      role="menu"
      data-testid="heatmap-metric-picker"
      onPointerDown={stopProp}
      onPointerMove={stopProp}
      onPointerUp={stopProp}
      onClick={stopProp}
      style={{
        position: "fixed",
        top,
        left,
        width: PICKER_WIDTH,
        padding: 10,
        // Deep-sea glass: the picker is the heatmap's child surface,
        // so the substrate echoes the strip's deep-blue. A bright
        // inner highlight + sea-glow outer halo make it read as
        // luminous fog rather than another floating paper panel.
        background:
          "linear-gradient(180deg, color-mix(in oklch, var(--water-sea-700), transparent 18%), color-mix(in oklch, var(--water-sea-800), transparent 8%))",
        backdropFilter: "blur(24px) saturate(160%)",
        WebkitBackdropFilter: "blur(24px) saturate(160%)",
        border:
          "1px solid color-mix(in srgb, var(--water-sea-glow) 22%, transparent)",
        borderRadius: "var(--water-r-16)",
        boxShadow:
          "0 12px 32px color-mix(in srgb, var(--water-sea-800) 35%, transparent), inset 0 1px 0 color-mix(in srgb, var(--water-sea-glow) 28%, transparent), 0 0 28px color-mix(in srgb, var(--water-sea-glow) 12%, transparent)",
        color: "color-mix(in srgb, var(--water-sea-glow) 80%, white)",
        zIndex: 10000,
        textAlign: "left",
        animation:
          "water-heat-picker-in var(--water-dur-small) var(--water-ease-out-soft) both",
      }}
    >
      <div
        style={{
          padding: "4px 10px 8px 10px",
          fontFamily: "var(--water-font-sans)",
          fontSize: 11,
          fontWeight: 600,
          textTransform: "uppercase",
          letterSpacing: 0.6,
          color: "color-mix(in srgb, var(--water-sea-glow) 70%, white)",
          textAlign: "left",
        }}
      >
        Heatmap metrics
      </div>
      {METRICS.map(({ kind, label, description, requiresLlm }) => {
        const on = enabled[kind] === true;
        return (
          <label
            key={kind}
            role="menuitemcheckbox"
            aria-checked={on ? "true" : "false"}
            data-testid={`heatmap-picker-row-${kind}`}
            style={{
              display: "grid",
              gridTemplateColumns: "auto 1fr",
              alignItems: "center",
              columnGap: 10,
              rowGap: 2,
              padding: "8px 10px",
              borderRadius: "var(--water-r-8)",
              cursor: "pointer",
              textAlign: "left",
              transition:
                "background-color var(--water-dur-tiny) var(--water-ease-out-soft)",
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.background =
                "color-mix(in srgb, var(--water-sea-glow) 14%, transparent)";
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background = "transparent";
            }}
          >
            {/* Row 1: checkbox + title (checkbox aligns to title baseline). */}
            <input
              type="checkbox"
              checked={on}
              onChange={(e) => {
                const next = e.currentTarget.checked;
                onToggle(kind, next);
                void ipc
                  .heatSetMetricEnabled(kind, next)
                  .catch(() => {
                    /* swallow */
                  });
              }}
              style={{
                gridRow: "1",
                gridColumn: "1",
                margin: 0,
                cursor: "pointer",
              }}
            />
            <div
              style={{
                gridRow: "1",
                gridColumn: "2",
                display: "flex",
                alignItems: "center",
                gap: 6,
                fontFamily: "var(--water-font-sans)",
                fontSize: "var(--water-fs-ui)",
                fontWeight: 500,
                color: "color-mix(in srgb, white 92%, var(--water-sea-100))",
                textAlign: "left",
              }}
            >
              {label}
              {requiresLlm && (
                <span
                  title="Requires a configured LLM provider"
                  style={{
                    padding: "1px 6px",
                    fontSize: 9,
                    fontWeight: 600,
                    letterSpacing: 0.4,
                    borderRadius: "var(--water-r-8)",
                    background:
                      "color-mix(in srgb, var(--water-hue-coherence) 28%, var(--water-bg-paper))",
                    color: "var(--water-fg-muted)",
                    textTransform: "uppercase",
                  }}
                >
                  LLM
                </span>
              )}
            </div>
            {/* Row 2: description spans column 2 only (sits under the
                title, indented past the checkbox column). */}
            <div
              style={{
                gridRow: "2",
                gridColumn: "2",
                fontFamily: "var(--water-font-sans)",
                fontSize: 11,
                lineHeight: 1.45,
                color:
                  "color-mix(in srgb, var(--water-sea-100) 80%, transparent)",
                textAlign: "left",
              }}
            >
              {description}
            </div>
          </label>
        );
      })}
    </div>,
    document.body,
  );
}

interface MetricDescriptor {
  kind: HeatMetricKind;
  label: string;
  description: string;
  requiresLlm: boolean;
}

const METRICS: MetricDescriptor[] = [
  {
    kind: "pacing",
    label: "Pacing",
    description: "How fast the writing moves at each point.",
    requiresLlm: false,
  },
  {
    kind: "valence",
    label: "Valence",
    description: "Emotional temperature — cold or warm.",
    requiresLlm: true,
  },
  {
    kind: "coherence",
    label: "Coherence",
    description: "How tightly each paragraph connects to the last.",
    requiresLlm: true,
  },
  {
    kind: "presence",
    label: "Presence",
    description: "Thickness of cast — how many characters in view.",
    requiresLlm: false,
  },
  {
    kind: "world_refs",
    label: "World refs",
    description: "References to places + entries from the world bible.",
    requiresLlm: false,
  },
];
