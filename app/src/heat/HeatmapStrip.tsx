import { useCallback, useEffect, useRef, useState } from "react";
import {
  ipc,
  type HeatMetricKind,
  type HeatReadResponse,
  type SceneInfo,
} from "../ipc/commands";
import { onWaterEvent } from "../ipc/events";
import { HeatmapMetricPicker } from "./HeatmapMetricPicker";

const INTRO_SEEN_KEY = "water:heatmap-intro-seen";

/**
 * Word-count threshold above which a scene's segment is considered
 * "finished enough to glow." Heuristic until an explicit completion
 * mark lands. Tuned to the size of a short scene (≈ 1–2 manuscript
 * pages).
 */
const LIT_WORD_THRESHOLD = 500;

interface Props {
  sceneId: string;
}

/**
 * Scene-progress strip above the editor — the lava-lamp rebuild.
 *
 * The earlier glass-chip + flat-gradient build read gray because
 * the chip's translucent paper layered over a too-pale sea wash. The
 * rebuild owns its own background entirely (no `.water-floating-chip`
 * class) and paints two stacks:
 *
 *   1. Three drifting radial-gradient blobs in deep-sea blues — the
 *      "amorphous gas." Each blob has its own slow translate + scale
 *      keyframe so the lamp breathes.
 *   2. ~24 small div orbs with `border-radius: 50%` + `box-shadow`
 *      glow — the "spray." SVG circles in the old build got
 *      horizontally stretched to lines because the viewBox didn't
 *      preserve aspect ratio; switching to absolute-positioned HTML
 *      divs sidesteps that entirely.
 *
 * On top of those: scene-segmented dividers with a per-segment
 * "lit" state when the scene's word count crosses a threshold. The
 * current scene gets a soft sea-tinted overlay.
 */
/**
 * Per-scene metric averages, keyed by scene id. Each entry holds
 * the mean over the paragraphs for every metric the sidecar has
 * computed. The picker's "active metric" selects which one feeds
 * the segment intensity. Caching avoids fanning out N `heat_read`
 * IPCs on every render — refetched on `heat:updated` for the
 * matching scene only.
 */
type SceneMetricMeans = Partial<Record<HeatMetricKind, number>>;

/** Pick the active metric: the first enabled in the canonical order. */
function activeMetric(
  enabled: Partial<Record<HeatMetricKind, boolean>>,
): HeatMetricKind {
  const order: HeatMetricKind[] = [
    "pacing",
    "valence",
    "coherence",
    "presence",
    "world_refs",
  ];
  for (const m of order) if (enabled[m] === true) return m;
  return "pacing";
}

/** Mean of a list of metric rows. Empty / missing → 0. */
function meanOf(rows: HeatReadResponse["metrics"][HeatMetricKind] | undefined): number {
  if (!rows || rows.length === 0) return 0;
  let s = 0;
  for (const r of rows) s += r.value;
  return s / rows.length;
}

export function HeatmapStrip({ sceneId }: Props) {
  const [scenes, setScenes] = useState<SceneInfo[]>([]);
  const [enabled, setEnabled] = useState<
    Partial<Record<HeatMetricKind, boolean>>
  >({});
  const [pickerOpen, setPickerOpen] = useState(false);
  const [pickerAnchor, setPickerAnchor] = useState<DOMRect | null>(null);
  const chipRef = useRef<HTMLButtonElement | null>(null);
  // Per-scene metric means, keyed by scene_id. Drives the segment
  // glow intensity once heat data lands.
  const [metricMeans, setMetricMeans] = useState<Record<string, SceneMetricMeans>>(
    {},
  );
  const [introSeen, setIntroSeen] = useState<boolean>(() => {
    try {
      return localStorage.getItem(INTRO_SEEN_KEY) === "true";
    } catch {
      return true;
    }
  });

  function dismissIntro() {
    try {
      localStorage.setItem(INTRO_SEEN_KEY, "true");
    } catch {
      /* swallow — intro will re-appear next session, no harm */
    }
    setIntroSeen(true);
  }

  // Fetch scene list once on mount + re-fetch on heat:updated. Re-
  // renders if scenes are added/removed (rare mid-session).
  const refetchScenes = useCallback(async () => {
    try {
      const list = await ipc.sceneList();
      setScenes(list);
    } catch {
      /* swallow — strip stays at last-known list */
    }
  }, []);

  /**
   * Fetch heat data for a single scene + fold its per-paragraph
   * rows into per-metric means. Stores into the keyed cache. The
   * caller refreshes a scene on initial load + on `heat:updated`
   * for that scene id only — so the strip never fans out N IPCs
   * unless every scene actually changed.
   */
  const refreshSceneMeans = useCallback(async (sid: string) => {
    try {
      const resp = await ipc.heatRead(sid);
      const means: SceneMetricMeans = {
        pacing: meanOf(resp.metrics.pacing),
        valence: meanOf(resp.metrics.valence),
        coherence: meanOf(resp.metrics.coherence),
        presence: meanOf(resp.metrics.presence),
        world_refs: meanOf(resp.metrics.world_refs),
      };
      setMetricMeans((prev) => ({ ...prev, [sid]: means }));
    } catch {
      /* swallow — segment renders without intensity */
    }
  }, []);

  useEffect(() => {
    void refetchScenes();
    void (async () => {
      try {
        const settings = await ipc.heatReadSettings();
        setEnabled({ pacing: true, ...settings.enabled });
      } catch {
        setEnabled({ pacing: true });
      }
    })();
  }, [refetchScenes]);

  // After the scene list lands, fetch metric data for every scene
  // in parallel (one IPC each). Cheap on small manuscripts (most
  // writers); 100-scene drafts trigger 100 IPCs once, then cache
  // until `heat:updated` invalidates per-scene.
  useEffect(() => {
    if (scenes.length === 0) return;
    for (const s of scenes) {
      void refreshSceneMeans(s.id);
    }
  }, [scenes, refreshSceneMeans]);

  // Subscribe to `heat:updated` so per-scene metric data refreshes
  // without re-fetching the whole list.
  useEffect(() => {
    let unsub: (() => void) | undefined;
    let cancelled = false;
    void (async () => {
      const u = await onWaterEvent("heat:updated", (e) => {
        if (cancelled) return;
        void refreshSceneMeans(e.scene_id);
      });
      if (cancelled) {
        u();
        return;
      }
      unsub = u;
    })();
    return () => {
      cancelled = true;
      unsub?.();
    };
  }, [refreshSceneMeans]);

  // Heuristic: scene "lit" once it carries enough prose to feel
  // substantive. Cheap, deterministic, no extra IPC. A real
  // completion mark lands when the scene-state surface gets a
  // checkbox (planned).
  const isScenelit = (s: SceneInfo): boolean =>
    s.word_count >= LIT_WORD_THRESHOLD;

  // Metric-driven glow: read the active metric's mean per scene
  // and clamp it to [0, 1]. Used to scale the inset glow on
  // already-lit segments + provide a faint hue tint on unlit
  // segments. Returns 0 when no metric data has landed yet.
  const activeKind = activeMetric(enabled);
  const intensityFor = (s: SceneInfo): number => {
    const v = metricMeans[s.id]?.[activeKind] ?? 0;
    return Math.max(0, Math.min(1, v));
  };

  return (
    <div
      aria-label="Heatmap"
      role="img"
      data-testid="heatmap-strip"
      className="water-heatmap-strip"
      style={{
        position: "relative",
        height: 28,
        margin: "0 0 12px 0",
        display: "flex",
        alignItems: "stretch",
        gap: 0,
        borderRadius: "var(--water-r-16)",
        overflow: "hidden",
        // Theme-driven palette: substrate + halo + border swap via
        // CSS vars set in `tokens.css`. Light mode renders a
        // baby-blue → cloud-white wash so the strip reads gently
        // against paper; dark mode keeps the deep night-sky tone
        // matching the manuscript surface.
        background: "var(--water-heat-bg)",
        boxShadow: "var(--water-heat-shadow)",
        border: "1px solid var(--water-heat-border)",
        animation:
          "water-pill-fade-in var(--water-dur-medium) var(--water-ease-out-soft) both",
      }}
    >
      {/* Amorphous gas — three drifting radial-gradient blobs. Each
          one's keyframe is tuned a little differently so they
          never align. Blurred to feel like luminous fog. */}
      <GasBlobs />

      {/* Spray — ~24 orbs scattered across the strip. Each is a
          small absolutely-positioned div with `border-radius: 50%`
          and a `box-shadow` glow, so they render as luminous dots
          rather than the stretched lines the SVG approach produced. */}
      <SprayDots />

      {/* Scene-segmented grid. Subtle hairlines between, current
          scene gets a soft fill, "lit" scenes pick up the sea-glow
          alpha. Painted above the gas so the segmentation reads. */}
      <div
        data-testid="heatmap-segments"
        style={{
          position: "relative",
          flex: 1,
          display: "flex",
          alignItems: "stretch",
          zIndex: 1,
        }}
      >
        {scenes.length === 0 ? (
          <div style={{ flex: 1 }} aria-hidden />
        ) : (
          scenes.map((s, ix) => {
            const isCurrent = s.id === sceneId;
            const lit = isScenelit(s);
            const intensity = intensityFor(s);
            // Metric-driven glow strength:
            //   - lit scenes get a base glow + a metric-scaled
            //     additional glow that brightens with intensity
            //   - non-lit substantial scenes pick up a faint
            //     metric tint (intensity > 0.4 only) so the writer
            //     sees pacing/valence movement even before the
            //     500-word threshold
            const glowAlpha = lit ? 50 - Math.round(intensity * 30) : 90;
            const tintFaint = !lit && intensity > 0.4;
            return (
              <div
                key={s.id}
                data-testid={`heatmap-segment-${ix}`}
                data-scene-id={s.id}
                data-current={isCurrent ? "true" : undefined}
                data-lit={lit ? "true" : undefined}
                data-active-metric={activeKind}
                aria-label={`Scene ${s.name || "untitled"}`}
                title={`${s.name || "untitled"} — ${s.word_count} words${lit ? " · lit" : ""} · ${activeKind} ${intensity.toFixed(2)}`}
                style={{
                  flex: 1,
                  position: "relative",
                  borderLeft:
                    ix === 0
                      ? "none"
                      : "1px solid color-mix(in srgb, var(--water-sea-glow) 18%, transparent)",
                  background: lit
                    ? `linear-gradient(180deg, color-mix(in oklch, var(--water-sea-glow), transparent ${glowAlpha}%), color-mix(in oklch, var(--water-sea-300), transparent ${glowAlpha + 15}%))`
                    : tintFaint
                      ? `color-mix(in oklch, var(--water-sea-300), transparent ${85 - Math.round(intensity * 15)}%)`
                      : isCurrent
                        ? "color-mix(in oklch, var(--water-sea-400), transparent 78%)"
                        : "transparent",
                  boxShadow: lit
                    ? `inset 0 0 ${6 + Math.round(intensity * 10)}px color-mix(in oklch, var(--water-sea-glow), transparent ${50 - Math.round(intensity * 20)}%)`
                    : "none",
                  transition:
                    "background var(--water-dur-medium) var(--water-ease-out-soft), box-shadow var(--water-dur-medium) var(--water-ease-out-soft)",
                }}
              />
            );
          })
        )}
      </div>

      {/* Right-edge metric chip + picker. Re-styled glass for the
          lava-lamp surface so the chip doesn't float as a foreign
          element. */}
      <div style={{ position: "relative", flexShrink: 0, zIndex: 2 }}>
        <button
          ref={chipRef}
          type="button"
          data-testid="heatmap-chip"
          onClick={() => {
            setPickerOpen((v) => {
              const next = !v;
              if (next && chipRef.current) {
                setPickerAnchor(chipRef.current.getBoundingClientRect());
              }
              return next;
            });
          }}
          aria-label="Heatmap metrics"
          aria-haspopup="menu"
          aria-expanded={pickerOpen ? "true" : "false"}
          className="water-heat-chip"
          style={{
            height: "100%",
            padding: "0 12px",
            display: "flex",
            alignItems: "center",
            gap: 4,
            border: "none",
            background: "var(--water-heat-chip-bg)",
            backdropFilter: "blur(8px) saturate(140%)",
            WebkitBackdropFilter: "blur(8px) saturate(140%)",
            color: "var(--water-heat-text)",
            fontFamily: "var(--water-font-sans)",
            fontSize: 10,
            fontWeight: 500,
            textTransform: "lowercase",
            letterSpacing: 0.4,
            cursor: "pointer",
            transition:
              "background var(--water-dur-small) var(--water-ease-out-soft), color var(--water-dur-small) var(--water-ease-out-soft)",
          }}
        >
          heat
          <span aria-hidden style={{ fontSize: 9, marginLeft: 2 }}>
            ▾
          </span>
        </button>
        <HeatmapMetricPicker
          open={pickerOpen}
          enabled={enabled}
          anchor={pickerAnchor}
          triggerRef={chipRef}
          onToggle={(kind, next) =>
            setEnabled((prev) => ({ ...prev, [kind]: next }))
          }
          onClose={() => setPickerOpen(false)}
        />
      </div>
      {!introSeen && <IntroOverlay onDismiss={dismissIntro} />}
    </div>
  );
}

/**
 * Three drifting radial-gradient blobs. Each is a 70%-of-width
 * circle of sea-300/400, blurred and translated through a long
 * cycle. The lamp effect comes from the staggered durations and
 * the slight scale variation in the keyframes.
 */
function GasBlobs() {
  // Hue refs live in the theme-driven CSS vars (light vs dark) so
  // the lamp reads as baby-blue in light mode and sea-glow against
  // night sky in dark.
  const blobs = [
    {
      left: "5%",
      hue: "var(--water-heat-gas-1)",
      anim: "water-heat-gas-1 22s ease-in-out infinite",
    },
    {
      left: "38%",
      hue: "var(--water-heat-gas-2)",
      anim: "water-heat-gas-2 28s ease-in-out -8s infinite",
    },
    {
      left: "70%",
      hue: "var(--water-heat-gas-3)",
      anim: "water-heat-gas-3 26s ease-in-out -4s infinite",
    },
  ];
  return (
    <div
      aria-hidden
      data-testid="heatmap-gas"
      style={{
        position: "absolute",
        inset: 0,
        pointerEvents: "none",
        overflow: "hidden",
      }}
    >
      {blobs.map((b, i) => (
        <div
          key={i}
          style={{
            position: "absolute",
            left: b.left,
            top: "-30%",
            width: "32%",
            height: "160%",
            borderRadius: "50%",
            background: `radial-gradient(circle, color-mix(in oklch, ${b.hue}, transparent 55%) 0%, color-mix(in oklch, ${b.hue}, transparent 80%) 45%, transparent 75%)`,
            filter: "blur(6px)",
            animation: b.anim,
            opacity: 0.85,
            willChange: "transform",
          }}
        />
      ))}
    </div>
  );
}

/**
 * Glowing orb field with a true lifecycle: each orb fades in,
 * holds, fades out, and respawns at a new random position. Mirrors
 * the WaterRibbon's droplet lifecycle so the heatmap doesn't read
 * as a static field of dots — orbs come and go like the ones in
 * the background stream.
 *
 * Implementation: each orb runs the `water-heat-orb-life-{0..5}`
 * keyframe (opacity 0 → peak → 0 over its lifetime). Repositioning
 * happens via `--orb-x` / `--orb-y` CSS variables that get
 * rewritten on the JS side every cycle. A `setInterval` per orb
 * stamps a new position when each lifecycle completes; total
 * scheduler cost stays under 24 timers per strip.
 */
function SprayDots() {
  const orbRefs = useRef<Array<HTMLDivElement | null>>([]);
  useEffect(() => {
    // After each orb's natural lifetime ends, mint new x/y and
    // diameter so the next cycle's fade-in lands somewhere fresh.
    // Schedules are slightly offset so the field never visibly
    // re-bursts in sync.
    const timers: number[] = DOT_SPECS.map((d, i) => {
      const reposition = () => {
        const el = orbRefs.current[i];
        if (!el) return;
        const x = Math.random() * 96 + 2;
        const y = 10 + Math.random() * 80;
        el.style.left = `${x}%`;
        el.style.top = `${y}%`;
      };
      // Reposition on the lifecycle boundary. Phase-shift the
      // initial delay so respawns don't tile.
      const periodMs = Math.max(1, d.duration) * 1000;
      const initialDelayMs = d.delay * 1000 + periodMs;
      const handle = window.setTimeout(function tick() {
        reposition();
        timers[i] = window.setTimeout(tick, periodMs);
      }, initialDelayMs);
      return handle;
    });
    return () => {
      for (const t of timers) window.clearTimeout(t);
    };
  }, []);
  return (
    <div
      aria-hidden
      data-testid="heatmap-drift-dots"
      style={{
        position: "absolute",
        inset: 0,
        pointerEvents: "none",
        overflow: "hidden",
      }}
    >
      {DOT_SPECS.map((d, i) => (
        <div
          key={i}
          ref={(el) => {
            orbRefs.current[i] = el;
          }}
          style={{
            position: "absolute",
            left: `${d.x}%`,
            top: `${d.y}%`,
            width: d.diameter,
            height: d.diameter,
            borderRadius: "50%",
            background: `radial-gradient(circle, ${d.coreHue} 0%, color-mix(in oklch, ${d.coreHue}, transparent 50%) 60%, transparent 100%)`,
            boxShadow: `0 0 ${d.diameter * 2}px ${d.glowHue}`,
            // Lifecycle: opacity 0 → peak → 0. Initial opacity is
            // 0 so the very first appearance fades in rather than
            // popping into existence.
            opacity: 0,
            animation: `water-heat-orb-life-${i % 6} ${d.duration}s ease-in-out ${d.delay}s infinite`,
            willChange: "opacity",
            transition: "left 600ms ease, top 600ms ease",
          }}
        />
      ))}
    </div>
  );
}

interface DotSpec {
  x: number;
  y: number;
  diameter: number;
  coreHue: string;
  glowHue: string;
  duration: number;
  delay: number;
}

/**
 * Deterministic seed positions (mulberry32 PRNG). Hues now resolve
 * from the theme-aware CSS vars so light mode renders pale
 * baby-blue cores against the white wash, while dark mode renders
 * sea-glow cores against the night sky. The keyframe's opacity
 * envelope handles the appear/disappear cycle; the JS-side
 * `setTimeout` repositions each orb on its lifecycle boundary so
 * it doesn't always reappear in the same spot.
 */
const DOT_SPECS: DotSpec[] = (() => {
  const out: DotSpec[] = [];
  const N = 24;
  let seed = 0x1f3a_b76d;
  const rand = () => {
    seed = (seed + 0x6d2b_79f5) | 0;
    let t = seed;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4_294_967_296;
  };
  const hueChoices = [
    {
      core: "var(--water-heat-orb-core)",
      glow: "var(--water-heat-orb-glow)",
    },
    {
      core: "var(--water-heat-orb-soft)",
      glow: "var(--water-heat-orb-glow)",
    },
    {
      core: "var(--water-heat-orb-core)",
      glow: "var(--water-heat-orb-soft)",
    },
  ];
  for (let i = 0; i < N; i++) {
    const tier = rand();
    const diameter =
      tier < 0.6 ? 1.2 + rand() * 0.5 : tier < 0.9 ? 1.8 + rand() * 0.8 : 2.6 + rand() * 1.0;
    const huePair = hueChoices[i % hueChoices.length]!;
    out.push({
      x: rand() * 96 + 2,
      y: 10 + rand() * 80,
      diameter,
      coreHue: huePair.core,
      glowHue: huePair.glow,
      duration: 10 + rand() * 16,
      delay: rand() * 14,
    });
  }
  return out;
})();

interface IntroProps {
  onDismiss: () => void;
}

/**
 * First-launch tooltip pointing at the strip. Same dismissal
 * persistence as the M5 strip. Copy reflects the new
 * scene-progress semantics.
 */
function IntroOverlay({ onDismiss }: IntroProps) {
  const stopProp = (e: React.SyntheticEvent) => {
    e.stopPropagation();
  };
  return (
    <div
      data-testid="heatmap-intro"
      role="dialog"
      aria-label="Heatmap introduction"
      onPointerDown={stopProp}
      onPointerMove={stopProp}
      onPointerUp={stopProp}
      onClick={stopProp}
      style={{
        position: "absolute",
        bottom: "calc(100% + 8px)",
        left: 0,
        minWidth: 260,
        maxWidth: 320,
        padding: "10px 12px",
        background: "var(--water-bg-paper)",
        color: "var(--water-fg-default)",
        fontFamily: "var(--water-font-sans)",
        fontSize: 12,
        lineHeight: 1.5,
        borderRadius: "var(--water-r-16)",
        boxShadow: "var(--water-elev-2)",
        zIndex: 1001,
        display: "flex",
        alignItems: "flex-start",
        gap: 8,
        cursor: "default",
        animation:
          "water-pill-fade-in var(--water-dur-medium) var(--water-ease-out-soft) both",
      }}
    >
      <div style={{ flex: 1 }}>
        the timeline. each segment is a scene. it glows when the scene crosses
        ~500 words.
      </div>
      <button
        type="button"
        aria-label="Dismiss"
        onPointerDown={stopProp}
        onClick={(e) => {
          e.stopPropagation();
          onDismiss();
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
          transition:
            "background-color var(--water-dur-tiny) var(--water-ease-out-soft)",
        }}
        onMouseEnter={(e) => {
          e.currentTarget.style.background =
            "color-mix(in srgb, var(--water-fg-faint) 14%, transparent)";
        }}
        onMouseLeave={(e) => {
          e.currentTarget.style.background = "transparent";
        }}
      >
        ×
      </button>
    </div>
  );
}
