/**
 * Module-level easing state for the water-ribbon. Lives outside React
 * so the eased anchors + clock survive across surface mount/unmount
 * cycles. Each `WaterRibbon` instance reads from / writes to this
 * module; switching surfaces (canvas → editor → world → ...) keeps
 * the ribbon's shape and phase continuous — no blink, no reset.
 *
 * Anchor lifecycle:
 *  - New target anchor (id not in displayed) appears with weight 0
 *    and eases its weight up to the target weight.
 *  - Existing anchor (id in both) eases x, y, and weight toward
 *    the target values.
 *  - Removed anchor (id in displayed but not target) decays its
 *    weight toward 0 and drops once near-zero.
 *
 * Result: when the canvas surface unmounts (or a scene is deleted),
 * the ribbon doesn't jump — its bends fade away gradually. When the
 * canvas mounts (or a scene appears), the bends fade in.
 */
import type { Anchor } from "./WaterRibbon";

// Easing constants were originally tuned at 60fps. Doubled to keep
// the same perceptual convergence speed at 30fps.
const POS_EASE = 0.1; // ~1s for x/y to converge at 30fps
const WEIGHT_EASE = 0.08; // ~1.5s for weight to fade in/out at 30fps
const DROP_THRESHOLD = 0.02;

// Throttle the clock to 30fps. The wave is slow and graceful; 30fps
// looks identical to 60fps for this kind of easing while halving the
// per-frame work (React force-update + buildStrand path regeneration
// + SVG diff + WebKit repaint). Was the dominant lag cause through
// alpha.5: a continuous 60fps loop kept the main thread saturated
// even when the user was idle.
const TARGET_FRAME_MS = 1000 / 30;

let displayed: Anchor[] = [];
let target: Anchor[] = [];
let rafId = 0;
let lastTickAt = 0;
const listeners = new Set<() => void>();

function tick(now: number) {
  if (now - lastTickAt < TARGET_FRAME_MS) {
    rafId = requestAnimationFrame(tick);
    return;
  }
  lastTickAt = now;

  const targetMap = new Map(target.map((t) => [t.id, t]));
  const next: Anchor[] = [];
  // Targets: ease toward, or fade in if newly appeared.
  for (const t of target) {
    const c = displayed.find((a) => a.id === t.id);
    if (!c) {
      next.push({ id: t.id, x: t.x, y: t.y, weight: 0 });
    } else {
      const cw = c.weight ?? 0;
      const tw = t.weight ?? 1;
      next.push({
        id: t.id,
        x: c.x + (t.x - c.x) * POS_EASE,
        y: c.y + (t.y - c.y) * POS_EASE,
        weight: cw + (tw - cw) * WEIGHT_EASE,
      });
    }
  }
  // Anchors that disappeared from the target: ease weight toward 0
  // and drop once they're near-invisible.
  for (const c of displayed) {
    if (targetMap.has(c.id)) continue;
    const cw = c.weight ?? 0;
    const w = cw * (1 - WEIGHT_EASE);
    if (w < DROP_THRESHOLD) continue;
    next.push({ ...c, weight: w });
  }
  displayed = next;
  for (const cb of listeners) cb();

  // Stop the loop when no WaterRibbon is mounted to observe it. The
  // next ensureClock() call (on subscribe or target change) restarts.
  // Without this gate the clock ran forever after first mount, which
  // was the alpha.5 lag cause.
  if (listeners.size === 0) {
    rafId = 0;
    return;
  }
  rafId = requestAnimationFrame(tick);
}

function ensureClock() {
  if (rafId === 0) {
    rafId = requestAnimationFrame(tick);
  }
}

/** Set the desired anchor list. Easing transitions toward it. */
export function setRibbonTarget(next: Anchor[]) {
  target = next;
  ensureClock();
}

/** Read the current eased anchors. Returns a stable reference per
 *  tick — components should not mutate. */
export function getRibbonDisplayed(): Anchor[] {
  return displayed;
}

/** Subscribe to per-frame updates. Returns an unsubscribe function. */
export function subscribeRibbon(cb: () => void): () => void {
  listeners.add(cb);
  ensureClock();
  return () => {
    listeners.delete(cb);
  };
}
