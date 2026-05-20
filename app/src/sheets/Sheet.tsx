import { useEffect, useRef, useState } from "react";
import { X } from "lucide-react";

interface Props {
  open: boolean;
  onClose: () => void;
  title: string;
  children: React.ReactNode;
}

type SheetState = "closed" | "opening" | "open" | "closing";

// Roughly matches --water-dur-tiny used for the close transition. Kept as a
// literal so the cleanup timer doesn't race with the CSS transition end on
// platforms that don't fire `transitionend` reliably (jsdom).
const CLOSE_MS = 280;

/**
 * Right-edge slide-in sheet. Uses an internal four-state machine driving a
 * `transform: translateX(...)` transition; `data-state` is exposed for tests
 * + downstream CSS hooks. Fixes M1.5 Review #14 (Sheet had no enter animation).
 *
 * State progression:
 *   closed   -> off-screen, no transition
 *   opening  -> off-screen for one frame, then flips to "open"
 *   open     -> translateX(0), animated with --water-dur-small/--water-ease-out-soft
 *   closing  -> back to off-screen, animated with --water-dur-tiny; after
 *               CLOSE_MS, drops to "closed" + calls dialog.close().
 */
export function Sheet({ open, onClose, title, children }: Props) {
  const dialogRef = useRef<HTMLDialogElement | null>(null);
  const [state, setState] = useState<SheetState>("closed");

  // Sync the requested `open` prop into the state machine.
  useEffect(() => {
    if (open) {
      if (state === "closed" || state === "closing") {
        setState("opening");
      }
    } else {
      if (state === "open" || state === "opening") {
        setState("closing");
      }
    }
  }, [open, state]);

  // Show/hide the underlying <dialog> based on the lifecycle phase.
  // showModal() must happen while the dialog is still off-screen so the
  // first paint shows it slid out, then the next frame transitions in.
  useEffect(() => {
    const el = dialogRef.current;
    if (!el) return;
    if (state === "opening" && !el.open) {
      el.showModal();
    }
    // Flip opening -> open on the next frame so the transform transition runs.
    if (state === "opening") {
      const id = requestAnimationFrame(() => {
        // requestAnimationFrame fires once before paint; bump one more frame
        // so the browser commits the initial translateX(100%) first.
        requestAnimationFrame(() => setState("open"));
      });
      return () => cancelAnimationFrame(id);
    }
    if (state === "closing") {
      const id = window.setTimeout(() => {
        setState("closed");
        if (el.open) el.close();
      }, CLOSE_MS);
      return () => window.clearTimeout(id);
    }
    return undefined;
  }, [state]);

  useEffect(() => {
    const el = dialogRef.current;
    if (!el) return;
    const handleCancel = (e: Event) => {
      e.preventDefault();
      onClose();
    };
    el.addEventListener("cancel", handleCancel);
    return () => el.removeEventListener("cancel", handleCancel);
  }, [onClose]);

  // Visual transform per phase.
  // "open" is the only state with translateX(0); everything else is off-screen.
  // The transition string differs by phase so opening (slow ease-out) feels
  // different from closing (faster water-style ease).
  let transform: string;
  let transition: string;
  if (state === "open") {
    transform = "translateX(0)";
    transition = "transform var(--water-dur-small) var(--water-ease-out-soft)";
  } else if (state === "closing") {
    transform = "translateX(100%)";
    transition = "transform var(--water-dur-tiny) var(--water-ease-in-out-water)";
  } else {
    // closed | opening
    transform = "translateX(100%)";
    transition = "none";
  }

  return (
    <dialog
      ref={dialogRef}
      data-state={state}
      onClick={(e) => {
        // Click outside the inner content closes the sheet.
        if (e.target === e.currentTarget) onClose();
      }}
      style={{
        margin: 0,
        marginLeft: "auto",
        height: "100vh",
        width: "min(420px, 90vw)",
        maxHeight: "100vh",
        padding: 0,
        border: "none",
        background: "var(--water-bg-raised)",
        color: "var(--water-fg-default)",
        boxShadow: "var(--water-elev-3)",
        transform,
        transition,
      }}
    >
      <header
        style={{
          display: "flex",
          alignItems: "center",
          gap: 12,
          padding: "16px 20px",
          borderBottom: "1px solid color-mix(in srgb, var(--water-fg-faint) 20%, transparent)",
        }}
      >
        <h2
          style={{
            margin: 0,
            flex: 1,
            fontFamily: "var(--water-font-serif)",
            fontSize: "var(--water-fs-title)",
            lineHeight: "var(--water-lh-title)",
            fontWeight: 500,
          }}
        >
          {title}
        </h2>
        <button
          type="button"
          aria-label="Close"
          onClick={onClose}
          style={{
            width: 32,
            height: 32,
            display: "grid",
            placeItems: "center",
            border: "none",
            background: "transparent",
            color: "var(--water-fg-muted)",
            cursor: "pointer",
            borderRadius: "var(--water-r-8)",
          }}
        >
          <X size={16} strokeWidth={1.5} />
        </button>
      </header>
      <div style={{ padding: 20, overflowY: "auto", height: "calc(100vh - 64px)" }}>{children}</div>
    </dialog>
  );
}
