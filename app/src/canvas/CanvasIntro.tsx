import { useState } from "react";

const INTRO_KEY = "water:canvas-intro-seen";

/**
 * One-time tooltip near the canvas's center pointing the writer at
 * the gestures they can use. Persistence via localStorage mirrors
 * the M5 Heatmap intro pattern.
 */
export function CanvasIntro() {
  const [dismissed, setDismissed] = useState<boolean>(() => {
    try {
      return localStorage.getItem(INTRO_KEY) === "true";
    } catch {
      return true;
    }
  });

  function dismiss() {
    try {
      localStorage.setItem(INTRO_KEY, "true");
    } catch {
      /* swallow */
    }
    setDismissed(true);
  }

  if (dismissed) return null;

  return (
    <div
      data-testid="canvas-intro"
      role="dialog"
      aria-label="Canvas introduction"
      onPointerDown={(e) => e.stopPropagation()}
      style={{
        position: "absolute",
        left: "50%",
        bottom: 32,
        transform: "translateX(-50%)",
        minWidth: 360,
        maxWidth: 440,
        padding: "12px 16px",
        background: "var(--water-bg-paper)",
        color: "var(--water-fg-default)",
        fontFamily: "var(--water-font-sans)",
        fontSize: 12,
        lineHeight: 1.55,
        borderRadius: "var(--water-r-16)",
        boxShadow: "var(--water-elev-3)",
        zIndex: 1001,
        display: "flex",
        alignItems: "flex-start",
        gap: 12,
        animation:
          "water-pill-fade-in var(--water-dur-medium) var(--water-ease-out-soft) both",
      }}
    >
      <div style={{ flex: 1 }}>
        drag scenes anywhere. cmd-scroll zooms. press{" "}
        <code
          style={{
            padding: "1px 6px",
            background:
              "color-mix(in srgb, var(--water-fg-faint) 14%, transparent)",
            borderRadius: 4,
            fontFamily: "var(--water-font-mono)",
          }}
        >
          O
        </code>{" "}
        to toggle reading order.
      </div>
      <button
        type="button"
        aria-label="Dismiss"
        onPointerDown={(e) => e.stopPropagation()}
        onClick={(e) => {
          e.stopPropagation();
          dismiss();
        }}
        style={{
          border: "none",
          background: "transparent",
          color: "var(--water-fg-muted)",
          cursor: "pointer",
          padding: "2px 8px",
          fontSize: 16,
          lineHeight: 1,
          borderRadius: "var(--water-r-8)",
        }}
      >
        ×
      </button>
    </div>
  );
}
