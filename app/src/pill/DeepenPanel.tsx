import { useEffect, useRef, useState } from "react";
import { ChevronLeft, Sparkles } from "lucide-react";
import { ipc } from "../ipc/commands";
import { onWaterEvent } from "../ipc/events";
import type { Pill } from "./types";

/**
 * Phase 4 — Rabbit-hole deepen panel (UX_SPEC §D).
 *
 * A side-slide panel that replaces the M2 bouquet expansion when
 * the writer clicks a pill. The panel renders the current parent
 * thought at the top, four child cards beneath, and a breadcrumb
 * row of ancestors above the parent so the writer can ascend by
 * clicking any level (or Esc to step up one).
 *
 * Wire shape:
 *   - On mount, dispatch `ipc.pillDeepen(pillId)`; the orchestrator
 *     creates a root thought from the pill and dispatches the
 *     `rabbit_fan_4` LLM call.
 *   - The component subscribes to `deepen:ready` and pushes the
 *     four children into its `path` state.
 *   - Clicking a child dispatches `ipc.rabbitDeepenThought(childId)`
 *     and (optimistically) pushes a new level. The next
 *     `deepen:ready` populates that level's children.
 *   - Resonance toggle calls `ipc.rabbitSetResonance` and flips a
 *     local flag for immediate visual feedback.
 *
 * Cost (per spec D.5): ~600 in / 400 out tokens per click at
 * Sonnet 4.6 = ~$0.005/click. Not budget-metered in this commit;
 * a follow-up will charge against an `LlmBudget` slot.
 */
interface DeepenChild {
  id: string;
  direction: string; // "closer" | "wider" | "opposite" | "deeper"
  text: string;
  resonant?: boolean;
}

interface DeepenLevel {
  /** The id of the parent thought at this level. */
  parentId: string;
  /** Display text for the parent (the pill text for the root). */
  parentText: string;
  /** Direction that arrived at this parent — empty for the root. */
  parentDirection: string;
  /** The 4 fanned children. `null` while LLM is in flight. */
  children: DeepenChild[] | null;
}

interface Props {
  rootPill: Pill;
  onClose: () => void;
}

export function DeepenPanel({ rootPill, onClose }: Props) {
  // Path is ordered root → deepest. Each entry's children fan out
  // beneath it. When the writer clicks a child, we push a new
  // pending level (children=null) and wait for the deepen:ready.
  const [path, setPath] = useState<DeepenLevel[]>([
    {
      parentId: "", // assigned by orchestrator; "" until first deepen:ready
      parentText: rootPill.text,
      parentDirection: "",
      children: null,
    },
  ]);
  const pathRef = useRef(path);
  pathRef.current = path;

  // Reason text surfaced when the fan can't complete (no LLM
  // configured, pill not found, etc.). Drives the failed-state
  // banner in the children area.
  const [failureReason, setFailureReason] = useState<string | null>(null);

  // Subscribe to the orchestrator's deepen events FIRST, then
  // dispatch the `pillDeepen` call. Subscribing in a separate
  // `useEffect` after dispatch races: if the orchestrator's
  // failure path (e.g. "no LLM provider configured") emits
  // `deepen:failed` before the renderer-side `onWaterEvent`
  // subscription lands, the panel sits in pending forever. The
  // single-effect order guarantees the subscription is live
  // before the IPC fires.
  //
  // Also includes a 45 s safety timer — if neither event lands,
  // we surface a generic failure so the panel never spins
  // indefinitely from any unforeseen path.
  useEffect(() => {
    let cancelled = false;
    let unsubReady: (() => void) | undefined;
    let unsubFailed: (() => void) | undefined;
    let safetyTimer: number | undefined;
    void (async () => {
      const u1 = await onWaterEvent("deepen:ready", (e) => {
        setPath((prev) => {
          // The very first deepen:ready writes the root's id back
          // (it was minted server-side). After that, deepen:ready
          // events correspond to the deepest-pending level whose
          // parentId we set when the writer descended.
          if (prev.length === 0) return prev;
          const lastIx = prev.length - 1;
          const last = prev[lastIx]!;
          if (last.parentId === "" || last.parentId === e.parent_id) {
            const next = prev.slice();
            next[lastIx] = {
              ...last,
              parentId: e.parent_id,
              children: e.children.map((c) => ({
                id: c.id,
                direction: c.direction,
                text: c.text,
                resonant: false,
              })),
            };
            return next;
          }
          // No match — silently drop. Could happen if the writer
          // ascended before the LLM call returned for a deeper level.
          return prev;
        });
        if (safetyTimer !== undefined) {
          window.clearTimeout(safetyTimer);
          safetyTimer = undefined;
        }
      });
      if (cancelled) {
        u1();
        return;
      }
      unsubReady = u1;
      const u2 = await onWaterEvent("deepen:failed", (e) => {
        setPath((prev) => {
          if (prev.length === 0) return prev;
          const lastIx = prev.length - 1;
          const last = prev[lastIx]!;
          if (last.parentId === "" || last.parentId === e.parent_id) {
            // Mark as empty so the spinner stops; the failure
            // reason banner takes over.
            const next = prev.slice();
            next[lastIx] = { ...last, children: [] };
            return next;
          }
          return prev;
        });
        setFailureReason(e.reason || "the model declined.");
        if (safetyTimer !== undefined) {
          window.clearTimeout(safetyTimer);
          safetyTimer = undefined;
        }
      });
      if (cancelled) {
        u2();
        return;
      }
      unsubFailed = u2;

      // Subscriptions live — dispatch. Pass the renderer's Pill
      // record through verbatim: the service-side Pill.text never
      // gets written back after the LLM call lands, so the
      // orchestrator needs us to hand it the actual text.
      try {
        await ipc.pillDeepen(
          rootPill.pill_id,
          rootPill.text,
          rootPill.speaker_id,
          rootPill.block_target_id,
        );
      } catch {
        if (!cancelled) {
          setFailureReason("couldn't reach the orchestrator.");
          setPath((prev) => {
            if (prev.length === 0) return prev;
            const lastIx = prev.length - 1;
            const last = prev[lastIx]!;
            const next = prev.slice();
            next[lastIx] = { ...last, children: [] };
            return next;
          });
        }
      }

      // Safety net: if neither event arrives within 45 s, surface
      // a generic failure so the panel never hangs.
      safetyTimer = window.setTimeout(() => {
        if (cancelled) return;
        setPath((prev) => {
          if (prev.length === 0) return prev;
          const lastIx = prev.length - 1;
          const last = prev[lastIx]!;
          if (last.children !== null) return prev;
          setFailureReason((prev) => prev ?? "deepen timed out.");
          const next = prev.slice();
          next[lastIx] = { ...last, children: [] };
          return next;
        });
      }, 45_000);
    })();
    return () => {
      cancelled = true;
      unsubReady?.();
      unsubFailed?.();
      if (safetyTimer !== undefined) window.clearTimeout(safetyTimer);
    };
  }, [rootPill.pill_id]);

  // Esc ascends one level (or closes when at the root).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      setPath((prev) => {
        if (prev.length <= 1) {
          onClose();
          return prev;
        }
        return prev.slice(0, prev.length - 1);
      });
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const descend = (child: DeepenChild) => {
    setPath((prev) => [
      ...prev,
      {
        parentId: child.id,
        parentText: child.text,
        parentDirection: child.direction,
        children: null,
      },
    ]);
    void ipc.rabbitDeepenThought(child.id).catch(() => {});
  };

  const ascendTo = (level: number) => {
    setPath((prev) => prev.slice(0, level + 1));
  };

  const toggleResonance = (childIx: number) => {
    setPath((prev) => {
      if (prev.length === 0) return prev;
      const lastIx = prev.length - 1;
      const last = prev[lastIx];
      if (!last || !last.children) return prev;
      const kid = last.children[childIx];
      if (!kid) return prev;
      const next = prev.slice();
      const nextKids = last.children.slice();
      nextKids[childIx] = { ...kid, resonant: !kid.resonant };
      next[lastIx] = { ...last, children: nextKids };
      // Fire-and-forget. The orchestrator persists asynchronously.
      void ipc.rabbitSetResonance(kid.id, !kid.resonant).catch(() => {});
      return next;
    });
  };

  const currentLevel = path[path.length - 1]!;
  const ancestors = path.slice(0, path.length - 1);

  return (
    <div
      data-testid="deepen-panel"
      role="dialog"
      aria-label="Deepen pill"
      style={{
        position: "relative",
        // Fill the available width of the nudge panel (which is
        // 280 px wide with 16 px horizontal padding ≈ 248 px usable).
        // The previous hardcoded 360 px overflowed by ~110 px,
        // pushing the deepen panel off-screen to the right.
        width: "100%",
        maxWidth: "100%",
        boxSizing: "border-box",
        minWidth: 0,
        maxHeight: 680,
        display: "flex",
        flexDirection: "column",
        padding: 12,
        borderRadius: "var(--water-r-16)",
        background:
          "color-mix(in srgb, var(--water-bg-paper) 72%, transparent)",
        backdropFilter: "blur(28px) saturate(170%) contrast(105%)",
        WebkitBackdropFilter: "blur(28px) saturate(170%) contrast(105%)",
        border:
          "1px solid color-mix(in srgb, var(--water-hairline) 55%, transparent)",
        boxShadow:
          "var(--water-elev-3), inset 0 1px 0 color-mix(in srgb, white 22%, transparent)",
        animation:
          "water-pill-fade-in var(--water-dur-medium) var(--water-ease-out-soft) both",
        pointerEvents: "auto",
        gap: 10,
      }}
    >
      {/* Breadcrumb row */}
      {ancestors.length > 0 && (
        <div
          data-testid="deepen-breadcrumb"
          style={{
            display: "flex",
            flexDirection: "column",
            gap: 4,
          }}
        >
          {ancestors.map((lvl, ix) => (
            <button
              key={`${lvl.parentId}-${ix}`}
              type="button"
              onClick={() => ascendTo(ix)}
              style={{
                display: "flex",
                alignItems: "center",
                gap: 6,
                padding: "4px 6px",
                border: "none",
                background: "transparent",
                color: "var(--water-fg-muted)",
                fontFamily: "var(--water-font-sans)",
                fontSize: 11,
                cursor: "pointer",
                textAlign: "left",
                borderRadius: "var(--water-r-8)",
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.background =
                  "color-mix(in srgb, var(--water-fg-faint) 10%, transparent)";
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.background = "transparent";
              }}
            >
              <ChevronLeft size={10} strokeWidth={1.75} />
              <span
                style={{
                  whiteSpace: "nowrap",
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                }}
              >
                {lvl.parentDirection ? `${lvl.parentDirection} · ` : ""}
                {lvl.parentText}
              </span>
            </button>
          ))}
        </div>
      )}

      {/* Current parent */}
      <div
        data-testid="deepen-parent"
        style={{
          padding: "8px 12px",
          borderRadius: "var(--water-r-16)",
          background:
            "color-mix(in srgb, var(--water-sea-100) 22%, var(--water-bg-paper))",
          color: "var(--water-fg-default)",
          fontFamily: "var(--water-font-sans)",
          fontSize: 12,
          lineHeight: 1.5,
        }}
      >
        {currentLevel.parentDirection && (
          <div
            style={{
              fontSize: 9,
              fontWeight: 700,
              textTransform: "uppercase",
              letterSpacing: 0.6,
              color: "var(--water-fg-muted)",
              marginBottom: 4,
            }}
          >
            {currentLevel.parentDirection}
          </div>
        )}
        {currentLevel.parentText}
      </div>

      {/* Children */}
      <div
        data-testid="deepen-children"
        style={{ display: "flex", flexDirection: "column", gap: 6 }}
      >
        {currentLevel.children === null ? (
          <PendingFan />
        ) : currentLevel.children.length === 0 ? (
          <div
            data-testid="deepen-failed-banner"
            style={{
              padding: "8px 12px",
              fontFamily: "var(--water-font-sans)",
              fontSize: 11,
              lineHeight: 1.5,
              color: "var(--water-fg-default)",
              textAlign: "left",
              background:
                "color-mix(in srgb, var(--water-sea-100) 30%, transparent)",
              border:
                "1px solid color-mix(in srgb, var(--water-sea-300) 40%, transparent)",
              borderRadius: "var(--water-r-8)",
            }}
          >
            {failureReason ?? "the model declined this fan. try again later."}
          </div>
        ) : (
          currentLevel.children.map((child, ix) => (
            <ChildCard
              key={child.id}
              child={child}
              onClick={() => descend(child)}
              onToggleResonance={() => toggleResonance(ix)}
            />
          ))
        )}
      </div>
    </div>
  );
}

function PendingFan() {
  return (
    <div
      data-testid="deepen-pending"
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        gap: 8,
        padding: "16px 12px",
        color: "var(--water-fg-muted)",
        fontFamily: "var(--water-font-sans)",
        fontSize: 11,
      }}
    >
      <div className="water-loading" aria-label="Deepening" />
      <span>fanning four directions…</span>
    </div>
  );
}

interface ChildCardProps {
  child: DeepenChild;
  onClick: () => void;
  onToggleResonance: () => void;
}

function ChildCard({ child, onClick, onToggleResonance }: ChildCardProps) {
  return (
    <div
      data-testid={`deepen-child-${child.direction}`}
      data-resonant={child.resonant ? "true" : undefined}
      style={{
        position: "relative",
        display: "flex",
        flexDirection: "column",
        gap: 4,
        padding: "8px 12px",
        borderRadius: "var(--water-r-16)",
        background: child.resonant
          ? "color-mix(in srgb, var(--water-sea-glow) 14%, var(--water-bg-paper))"
          : "color-mix(in srgb, var(--water-bg-paper) 70%, transparent)",
        border:
          "1px solid color-mix(in srgb, var(--water-hairline) 50%, transparent)",
        boxShadow: "var(--water-elev-1)",
        cursor: "pointer",
        transition:
          "background var(--water-dur-tiny) var(--water-ease-out-soft), transform var(--water-dur-tiny) var(--water-ease-out-soft)",
      }}
      onClick={(e) => {
        // Resonance button has its own onClick + stopPropagation;
        // any click here means "descend into this child."
        if ((e.target as HTMLElement).dataset.resonanceToggle === "true") return;
        onClick();
      }}
      onMouseEnter={(e) => {
        e.currentTarget.style.transform = "translateY(-1px)";
      }}
      onMouseLeave={(e) => {
        e.currentTarget.style.transform = "translateY(0)";
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: 6,
        }}
      >
        <span
          style={{
            fontSize: 9,
            fontWeight: 700,
            textTransform: "uppercase",
            letterSpacing: 0.6,
            color: "var(--water-fg-muted)",
          }}
        >
          {child.direction}
        </span>
        <button
          type="button"
          data-resonance-toggle="true"
          aria-label={
            child.resonant ? "Unmark as resonant" : "Mark as resonant"
          }
          title={child.resonant ? "resonant" : "mark resonant"}
          onClick={(e) => {
            e.stopPropagation();
            onToggleResonance();
          }}
          style={{
            display: "inline-flex",
            alignItems: "center",
            justifyContent: "center",
            width: 18,
            height: 18,
            padding: 0,
            border: "none",
            background: "transparent",
            color: child.resonant
              ? "var(--water-sea-glow)"
              : "var(--water-fg-faint)",
            cursor: "pointer",
            borderRadius: "var(--water-r-8)",
            transition:
              "color var(--water-dur-tiny) var(--water-ease-out-soft)",
          }}
        >
          <Sparkles size={12} strokeWidth={1.75} />
        </button>
      </div>
      <div
        style={{
          color: "var(--water-fg-default)",
          fontFamily: "var(--water-font-sans)",
          fontSize: 12,
          lineHeight: 1.5,
        }}
      >
        {child.text}
      </div>
    </div>
  );
}
