import { useEffect, useState } from "react";
import { ipc } from "../ipc/commands";
import { onWaterEvent } from "../ipc/events";
import { PinnedPillDetail } from "./PinnedPillDetail";
import type { Pill } from "./types";

/** Below this `<main>` width the 56 px column eats too much of the
 *  prose. Collapse to a 24 px tab; click expands it back as an overlay. */
const NARROW_BREAKPOINT_PX = 1100;

/**
 * 56px-wide right-edge column that lists every pinned pill in the project as
 * a half-opacity glowing dot. Always allocated (renders even when empty) so
 * the editor's right margin stays stable as pills come and go.
 *
 * On mount: fetches the existing pin list via `ipc.pinnedList`.
 * On `pill:pinned`: prepends the new pill to the head of the list.
 * On `pill:unpinned`: removes the pill by id.
 *
 * Clicking a dot opens a <PinnedPillDetail> sheet with the pill's text and
 * an "Un-pin" button. The sheet calls `pill_dismiss` (which deletes from
 * `pinned_pill` + emits `pill:unpinned`), then closes.
 *
 * Subscription pattern follows T4's cancellation-race fix: the async IIFE
 * tracks a `cancelled` flag so a sheet-close before `onWaterEvent` resolves
 * still calls the resolved unsubscribe.
 */
interface PinnedColumnProps {
  /** Current width of the editor's `<main>`. `0` = not yet measured
   *  (treated the same as "wide enough"); below `NARROW_BREAKPOINT_PX`
   *  collapses to a 24 px tab. Defaults to `0` so tests / standalone
   *  renders skip the fallback. */
  mainWidth?: number;
}

export function PinnedColumn({ mainWidth = 0 }: PinnedColumnProps = {}) {
  const [pins, setPins] = useState<Pill[]>([]);
  const [openId, setOpenId] = useState<string | null>(null);
  // In tab mode the user must click the strip to reveal dots; otherwise the
  // 24 px sliver shows nothing actionable.
  const [tabExpanded, setTabExpanded] = useState(false);

  const isTab = mainWidth > 0 && mainWidth < NARROW_BREAKPOINT_PX;
  const collapsed = isTab && !tabExpanded;

  // Auto-collapse when the viewport widens back out so a stale expanded
  // state doesn't linger on the next narrow render.
  useEffect(() => {
    if (!isTab && tabExpanded) setTabExpanded(false);
  }, [isTab, tabExpanded]);

  // Initial fetch.
  useEffect(() => {
    let cancelled = false;
    ipc
      .pinnedList()
      .then((list) => {
        if (!cancelled) setPins(list);
      })
      .catch(() => {
        /* swallow - no project open is a valid early state */
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // Event subscriptions (T4 cancellation-race-safe).
  useEffect(() => {
    let cancelled = false;
    const unsubs: Array<() => void> = [];
    (async () => {
      const u1 = await onWaterEvent("pill:pinned", (p) => {
        // Prepend; ignore duplicates if the backend ever double-fires.
        setPins((prev) =>
          prev.some((x) => x.pill_id === p.pill_id) ? prev : [p, ...prev],
        );
      });
      if (cancelled) {
        u1();
        return;
      }
      unsubs.push(u1);

      const u2 = await onWaterEvent("pill:unpinned", (e) => {
        setPins((prev) => prev.filter((x) => x.pill_id !== e.pill_id));
        setOpenId((prev) => (prev === e.pill_id ? null : prev));
      });
      if (cancelled) {
        u2();
        return;
      }
      unsubs.push(u2);
    })();
    return () => {
      cancelled = true;
      for (const u of unsubs) u();
    };
  }, []);

  const opened = pins.find((p) => p.pill_id === openId) ?? null;

  // Width and padding flip in tab mode. The data-testid + aria-label stay
  // identical so tests can assert against the same element either way.
  const width = collapsed ? "24px" : "56px";
  const padding = collapsed ? "72px 4px" : "72px 12px";

  return (
    <>
      {/* When expanded over a narrow viewport, a click outside the column
       *  collapses it. Backdrop sits behind the column itself. */}
      {isTab && tabExpanded ? (
        <div
          aria-hidden="true"
          onClick={() => setTabExpanded(false)}
          style={{
            position: "absolute",
            inset: 0,
            background: "color-mix(in oklch, var(--water-bg-paper) 60%, transparent)",
            pointerEvents: "auto",
            zIndex: 1,
          }}
        />
      ) : null}
      <aside
        aria-label="pinned column"
        data-collapsed={collapsed ? "true" : undefined}
        onClick={collapsed ? () => setTabExpanded(true) : undefined}
        style={{
          position: "absolute",
          top: 0,
          right: 0,
          bottom: 0,
          width,
          padding,
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          gap: 16,
          pointerEvents: "auto",
          cursor: collapsed ? "pointer" : "default",
          // A faint vertical strip so the tab is visible even when no
          // pills are pinned.
          background: collapsed
            ? "color-mix(in oklch, var(--water-fg-faint) 8%, transparent)"
            : "transparent",
          transition: "width 160ms ease, padding 160ms ease",
          zIndex: 2,
        }}
      >
        {/* Hide dots in collapsed tab mode; tab is purely an affordance. */}
        {collapsed
          ? null
          : pins.map((p) => (
              <button
                key={p.pill_id}
                type="button"
                data-testid="water-pinned-dot"
                data-pill-id={p.pill_id}
                aria-label={`Pinned pill: ${p.text}`}
                onClick={() => setOpenId(p.pill_id)}
                style={{
                  width: 16,
                  height: 16,
                  padding: 0,
                  border: "none",
                  borderRadius: "50%",
                  background: `var(${p.hue_token})`,
                  boxShadow: `0 0 12px color-mix(in oklch, var(${p.hue_token}) 70%, transparent)`,
                  opacity: 0.5,
                  cursor: "pointer",
                }}
              />
            ))}
      </aside>
      {opened ? (
        <PinnedPillDetail pill={opened} onClose={() => setOpenId(null)} />
      ) : null}
    </>
  );
}
