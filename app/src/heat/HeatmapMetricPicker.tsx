import { useEffect, useRef } from "react";
import { ipc, type HeatMetricKind } from "../ipc/commands";

interface Props {
  open: boolean;
  enabled: Partial<Record<HeatMetricKind, boolean>>;
  onToggle: (kind: HeatMetricKind, enabled: boolean) => void;
  onClose: () => void;
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
}: Props) {
  const ref = useRef<HTMLDivElement | null>(null);

  // Close on outside click + Escape.
  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
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
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div
      ref={ref}
      role="menu"
      data-testid="heatmap-metric-picker"
      style={{
        position: "absolute",
        top: "calc(100% + 6px)",
        right: 0,
        minWidth: 280,
        padding: 8,
        // Fully opaque so the title behind the strip doesn't bleed
        // through. bg-paper is the canvas color; raised was bleeding
        // because color-mix layers in some downstream tokens are
        // partially transparent at this composition depth.
        background: "var(--water-bg-paper)",
        borderRadius: "var(--water-r-16)",
        boxShadow: "var(--water-elev-2)",
        zIndex: 1000,
        textAlign: "left",
        animation:
          "water-pill-fade-in var(--water-dur-tiny) var(--water-ease-out-soft) both",
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
          color: "var(--water-fg-muted)",
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
                "color-mix(in srgb, var(--water-fg-faint) 8%, transparent)";
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
                color: "var(--water-fg-default)",
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
                color: "var(--water-fg-muted)",
                textAlign: "left",
              }}
            >
              {description}
            </div>
          </label>
        );
      })}
    </div>
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
