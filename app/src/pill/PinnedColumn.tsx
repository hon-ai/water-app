import { useEffect, useState } from "react";
import { ipc } from "../ipc/commands";
import { onWaterEvent } from "../ipc/events";
import { PinnedPillDetail } from "./PinnedPillDetail";
import type { Pill } from "./types";

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
export function PinnedColumn() {
  const [pins, setPins] = useState<Pill[]>([]);
  const [openId, setOpenId] = useState<string | null>(null);

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

  return (
    <>
      <aside
        aria-label="pinned column"
        style={{
          position: "absolute",
          top: 0,
          right: 0,
          bottom: 0,
          width: "56px",
          padding: "72px 12px",
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          gap: 16,
          pointerEvents: "auto",
        }}
      >
        {pins.map((p) => (
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
