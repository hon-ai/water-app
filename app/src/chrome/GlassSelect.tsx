// Glass-styled select. Standard <select> trigger glassy popup the
// browser draws natively, which clashes with the rest of the UX —
// this component renders a glass-chip trigger and its own portal-
// mounted glass list so the dropdown matches the app's aesthetic.
//
// Drop-in replacement for a minimal <select> use: { value, onChange,
// options[{value,label,hint?}] }. Click-outside + Escape close;
// arrow keys + Enter navigate. The list portals to <body> so it
// escapes Sheet's overflow clip.
//
// Not a full a11y combobox — for the Settings sheet a simple
// listbox-style chip is enough. Falls back gracefully without
// keyboard support if window is undefined (jsdom).

import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
} from "react";
import { createPortal } from "react-dom";
import { ChevronDown } from "lucide-react";

/**
 * Find the nearest open `<dialog>` ancestor of `el` and return it as
 * a portal target. Returns null when the trigger isn't inside a
 * dialog, in which case the caller should fall back to
 * `document.body`.
 *
 * **Why this exists.** A `<dialog>` opened via `showModal()` is
 * promoted into the browser's "top layer", which renders ABOVE all
 * regular DOM content — including portals mounted on `document.body`.
 * Settings, scene metadata, world entry, and the project sheet all
 * open as modal dialogs; a GlassSelect inside any of them would
 * normally portal its menu to `body`, where the dialog's top-layer
 * stacking would render the menu *behind* the panel and steal focus
 * away from it. Portaling into the dialog instead keeps the menu in
 * the same stacking context as the trigger.
 */
function portalTarget(el: Element | null): Element | null {
  if (!el) return null;
  let cur: Element | null = el;
  while (cur && cur !== document.body) {
    if (cur.tagName === "DIALOG") return cur;
    cur = cur.parentElement;
  }
  return null;
}

export interface GlassSelectOption {
  value: string;
  label: string;
  hint?: string;
  /** Optional CSS font-family override for this row + the trigger
   *  label when this row is active (e.g. manuscript-font picker
   *  showing each option in its own face). */
  fontFamily?: string;
}

interface Props {
  value: string;
  options: GlassSelectOption[];
  onChange: (next: string) => void;
  /** Optional placeholder when value matches no known option. */
  placeholder?: string;
  /** Used for tests + scoped styling hooks. */
  testId?: string;
  /** Optional aria-label for the trigger. */
  ariaLabel?: string;
  /** Trigger styling overrides — width, fontSize, etc. */
  triggerStyle?: CSSProperties;
  /** When true, the trigger renders dimmed and the menu won't open. */
  disabled?: boolean;
}

export function GlassSelect({
  value,
  options,
  onChange,
  placeholder,
  testId,
  ariaLabel,
  triggerStyle,
  disabled,
}: Props) {
  const [open, setOpen] = useState(false);
  const triggerRef = useRef<HTMLButtonElement | null>(null);
  const listRef = useRef<HTMLDivElement | null>(null);
  const [menuPos, setMenuPos] = useState<{
    left: number;
    top: number;
    width: number;
    flipAbove: boolean;
  } | null>(null);
  const [focusIdx, setFocusIdx] = useState<number>(() =>
    Math.max(
      0,
      options.findIndex((o) => o.value === value),
    ),
  );

  const activeOption = options.find((o) => o.value === value);
  const triggerLabel = activeOption?.label ?? placeholder ?? "";

  const recomputePos = useCallback(() => {
    const trig = triggerRef.current;
    if (!trig) return;
    const r = trig.getBoundingClientRect();
    const ESTIMATED_MAX = Math.min(
      options.length * 36 + 12,
      Math.max(140, window.innerHeight - r.bottom - 24),
    );
    const spaceBelow = window.innerHeight - r.bottom - 8;
    const flipAbove = spaceBelow < ESTIMATED_MAX && r.top > ESTIMATED_MAX;
    // Coord-frame correction. When we portal the menu into a
    // containing-block ancestor (a `<dialog>` with `backdrop-filter`
    // — every Sheet in this app has this), `position: fixed` is
    // resolved against THAT ancestor, not the viewport. Subtract
    // the ancestor's bounding rect so viewport-derived
    // `getBoundingClientRect()` coords still land where the user's
    // eye expects.
    const portal = portalTarget(trig);
    let leftOffset = 0;
    let topOffset = 0;
    if (portal) {
      const pr = portal.getBoundingClientRect();
      leftOffset = pr.left;
      topOffset = pr.top;
    }
    setMenuPos({
      left: r.left - leftOffset,
      top: (flipAbove ? r.top : r.bottom + 4) - topOffset,
      width: r.width,
      flipAbove,
    });
  }, [options.length]);

  useLayoutEffect(() => {
    if (!open) return;
    recomputePos();
    const onScroll = () => recomputePos();
    window.addEventListener("scroll", onScroll, { capture: true });
    window.addEventListener("resize", recomputePos);
    return () => {
      window.removeEventListener("scroll", onScroll, {
        capture: true,
      } as EventListenerOptions);
      window.removeEventListener("resize", recomputePos);
    };
  }, [open, recomputePos]);

  // Click-outside to close. Use mousedown so a synthetic click on a
  // menu row still fires before the close runs.
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      const t = e.target as Node;
      if (
        listRef.current?.contains(t) ||
        triggerRef.current?.contains(t)
      ) {
        return;
      }
      setOpen(false);
    };
    window.addEventListener("mousedown", onDown);
    return () => window.removeEventListener("mousedown", onDown);
  }, [open]);

  // Keyboard navigation while open.
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        setOpen(false);
        triggerRef.current?.focus();
        return;
      }
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setFocusIdx((i) => Math.min(options.length - 1, i + 1));
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setFocusIdx((i) => Math.max(0, i - 1));
        return;
      }
      if (e.key === "Enter") {
        e.preventDefault();
        const next = options[focusIdx];
        if (next) {
          onChange(next.value);
          setOpen(false);
          triggerRef.current?.focus();
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, focusIdx, options, onChange]);

  const triggerBase: CSSProperties = {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: 8,
    width: "100%",
    padding: "7px 12px",
    border:
      "1px solid color-mix(in srgb, var(--water-fg-faint) 22%, transparent)",
    borderRadius: "var(--water-r-8)",
    background:
      "color-mix(in srgb, var(--water-bg-paper) 55%, transparent)",
    backdropFilter: "blur(10px) saturate(140%)",
    WebkitBackdropFilter: "blur(10px) saturate(140%)",
    color: "var(--water-fg-default)",
    fontFamily: activeOption?.fontFamily ?? "var(--water-font-sans)",
    fontSize: "var(--water-fs-meta)",
    cursor: disabled ? "not-allowed" : "pointer",
    opacity: disabled ? 0.55 : 1,
    transition:
      "border-color var(--water-dur-tiny) var(--water-ease-out-soft), background-color var(--water-dur-tiny) var(--water-ease-out-soft)",
    textAlign: "left",
    ...triggerStyle,
  };

  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={ariaLabel}
        data-testid={testId}
        data-glass-select="true"
        disabled={disabled}
        onClick={() => {
          if (disabled) return;
          setOpen((o) => !o);
          setFocusIdx(
            Math.max(
              0,
              options.findIndex((o) => o.value === value),
            ),
          );
        }}
        style={triggerBase}
      >
        <span
          style={{
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
            flex: 1,
          }}
        >
          {triggerLabel}
        </span>
        <ChevronDown
          size={14}
          aria-hidden
          style={{
            opacity: 0.55,
            transform: open ? "rotate(180deg)" : "none",
            transition:
              "transform var(--water-dur-tiny) var(--water-ease-out-soft)",
            flexShrink: 0,
          }}
        />
      </button>
      {open && menuPos && typeof document !== "undefined"
        ? createPortal(
            <div
              ref={listRef}
              role="listbox"
              data-testid={testId ? `${testId}-list` : undefined}
              style={{
                position: "fixed",
                left: menuPos.left,
                top: menuPos.top,
                width: menuPos.width,
                transform: menuPos.flipAbove
                  ? "translateY(-100%) translateY(-4px)"
                  : "none",
                maxHeight: Math.min(
                  options.length * 40 + 8,
                  Math.max(160, window.innerHeight - 80),
                ),
                overflowY: "auto",
                padding: 4,
                borderRadius: "var(--water-r-12, 12px)",
                border:
                  "1px solid color-mix(in srgb, var(--water-fg-faint) 24%, transparent)",
                background:
                  "color-mix(in srgb, var(--water-bg-paper) 78%, transparent)",
                backdropFilter: "blur(18px) saturate(160%)",
                WebkitBackdropFilter: "blur(18px) saturate(160%)",
                boxShadow:
                  "0 18px 40px -16px color-mix(in srgb, var(--water-fg-default) 30%, transparent), 0 2px 6px color-mix(in srgb, var(--water-fg-default) 10%, transparent)",
                zIndex: 200,
                // Opacity-only fade. The flip-above branch sets
                // `transform: translateY(-100%) translateY(-4px)` to
                // anchor the menu above the trigger; the
                // `water-pill-fade-in` keyframe would overwrite that
                // transform and the menu would slam down on top of
                // the trigger.
                animation:
                  "water-fade-in var(--water-dur-small) var(--water-ease-out-soft) both",
              }}
            >
              {options.map((o, i) => {
                const selected = o.value === value;
                const focused = i === focusIdx;
                return (
                  <button
                    key={o.value}
                    type="button"
                    role="option"
                    aria-selected={selected}
                    onMouseEnter={() => setFocusIdx(i)}
                    onClick={() => {
                      onChange(o.value);
                      setOpen(false);
                      triggerRef.current?.focus();
                    }}
                    style={{
                      display: "flex",
                      flexDirection: "column",
                      alignItems: "flex-start",
                      gap: 2,
                      width: "100%",
                      padding: "7px 10px",
                      border: "none",
                      borderRadius: "var(--water-r-8)",
                      background: focused
                        ? "color-mix(in srgb, var(--water-hue-flow) 22%, transparent)"
                        : "transparent",
                      color: "var(--water-fg-default)",
                      fontFamily: o.fontFamily ?? "var(--water-font-sans)",
                      fontSize: "var(--water-fs-meta)",
                      cursor: "pointer",
                      textAlign: "left",
                      transition:
                        "background-color var(--water-dur-tiny) var(--water-ease-out-soft)",
                    }}
                  >
                    <span
                      style={{
                        fontWeight: selected ? 600 : 500,
                        display: "flex",
                        alignItems: "center",
                        gap: 6,
                      }}
                    >
                      {selected && (
                        <span
                          aria-hidden
                          style={{
                            width: 5,
                            height: 5,
                            borderRadius: "50%",
                            background:
                              "color-mix(in srgb, var(--water-hue-flow) 90%, transparent)",
                            boxShadow:
                              "0 0 6px color-mix(in srgb, var(--water-hue-flow) 80%, transparent)",
                            flexShrink: 0,
                          }}
                        />
                      )}
                      {o.label}
                    </span>
                    {o.hint && (
                      <span
                        style={{
                          fontSize: 10,
                          color: "var(--water-fg-muted)",
                          lineHeight: 1.4,
                        }}
                      >
                        {o.hint}
                      </span>
                    )}
                  </button>
                );
              })}
            </div>,
            portalTarget(triggerRef.current) ?? document.body,
          )
        : null}
    </>
  );
}
