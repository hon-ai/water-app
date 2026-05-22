import { useEffect, useRef, useState, type CSSProperties, type KeyboardEvent } from "react";
import { createPortal } from "react-dom";

interface Props {
  anchor: { left: number; top: number };
  initialUrl: string;
  editing: boolean;
  onApply: (url: string) => void;
  onRemove: () => void;
  onClose: () => void;
}

function isAcceptable(raw: string): boolean {
  const url = raw.trim();
  if (!url) return false;
  if (/^javascript:/i.test(url)) return false;
  if (/^data:/i.test(url)) return false;
  return true;
}

export function LinkPopup({
  anchor,
  initialUrl,
  editing,
  onApply,
  onRemove,
  onClose,
}: Props) {
  const [value, setValue] = useState(initialUrl);
  const inputRef = useRef<HTMLInputElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
  }, []);

  // Close on outside click.
  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      if (!containerRef.current) return;
      if (!containerRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    window.addEventListener("mousedown", onDown);
    return () => window.removeEventListener("mousedown", onDown);
  }, [onClose]);

  const handleApply = () => {
    const trimmed = value.trim();
    if (trimmed === "" && editing) {
      onRemove();
      onClose();
      return;
    }
    if (!isAcceptable(trimmed)) {
      // Keep popup open; do not close.
      return;
    }
    onApply(trimmed);
    onClose();
  };

  const handleRemove = () => {
    onRemove();
    onClose();
  };

  const containerStyle: CSSProperties = {
    position: "fixed",
    left: anchor.left,
    top: anchor.top,
    transform: "translateX(-50%)",
    display: "flex",
    flexDirection: "row",
    gap: 8,
    alignItems: "center",
    padding: "8px 12px",
    borderRadius: "var(--water-r-16)",
    background:
      "color-mix(in oklch, var(--water-hue-coherence) 30%, var(--water-bg-paper))",
    boxShadow:
      "0 0 18px color-mix(in oklch, var(--water-hue-coherence) 50%, transparent)",
    pointerEvents: "auto",
    zIndex: 41,
    minWidth: 280,
    // Opacity-only fade — `water-pill-fade-in` would override the
    // `translateX(-50%)` horizontal centering and jump the popup
    // sideways once the animation settles.
    animation:
      "water-fade-in var(--water-dur-small) var(--water-ease-out-soft) both",
  };

  const inputStyle: CSSProperties = {
    flex: 1,
    border: "none",
    outline: "none",
    background: "transparent",
    color: "var(--water-fg-default)",
    fontFamily: "var(--water-font-sans)",
    fontSize: "var(--water-fs-ui)",
    padding: "4px 6px",
  };

  const buttonStyle: CSSProperties = {
    border: "none",
    background: "transparent",
    color: "var(--water-fg-default)",
    cursor: "pointer",
    padding: "4px 10px",
    borderRadius: "var(--water-r-8)",
    fontFamily: "var(--water-font-sans)",
    fontSize: "var(--water-fs-meta)",
    boxShadow:
      "inset 0 0 0 1px color-mix(in srgb, var(--water-fg-faint) 30%, transparent)",
  };

  const onInputKey = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      handleApply();
    } else if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    }
  };

  return createPortal(
    <div ref={containerRef} role="dialog" aria-label="Link" style={containerStyle}>
      <input
        ref={inputRef}
        aria-label="URL"
        type="url"
        placeholder="https://"
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={onInputKey}
        style={inputStyle}
      />
      <button type="button" onClick={handleApply} style={buttonStyle}>
        Apply
      </button>
      {editing && (
        <button type="button" onClick={handleRemove} style={buttonStyle}>
          Remove
        </button>
      )}
    </div>,
    document.body,
  );
}
