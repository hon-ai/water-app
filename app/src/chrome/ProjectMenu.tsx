import { useEffect, useRef } from "react";

interface Props {
  open: boolean;
  onClose: () => void;
  onSwitchProject: () => void;
  onCloseProject: () => void;
}

export function ProjectMenu({ open, onClose, onSwitchProject, onCloseProject }: Props) {
  const ref = useRef<HTMLDivElement | null>(null);

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

  const item: React.CSSProperties = {
    display: "block",
    width: "100%",
    textAlign: "left",
    padding: "8px 12px",
    border: "none",
    background: "transparent",
    color: "var(--water-fg-default)",
    cursor: "pointer",
    borderRadius: "var(--water-r-8)",
    fontFamily: "var(--water-font-sans)",
    fontSize: "var(--water-fs-ui)",
  };

  return (
    <div
      ref={ref}
      role="menu"
      style={{
        position: "absolute",
        top: "100%",
        left: 12,
        marginTop: 4,
        minWidth: 200,
        padding: 4,
        background: "var(--water-bg-paper)",
        borderRadius: "var(--water-r-16)",
        boxShadow: "0 0 0 1px color-mix(in srgb, var(--water-fg-faint) 25%, transparent)",
        zIndex: "var(--water-z-tooltip)" as unknown as number,
      }}
    >
      <button
        type="button"
        role="menuitem"
        style={item}
        onClick={() => {
          onClose();
          onSwitchProject();
        }}
      >
        Switch project…
      </button>
      <button
        type="button"
        role="menuitem"
        style={item}
        onClick={() => {
          onClose();
          onCloseProject();
        }}
      >
        Close project
      </button>
    </div>
  );
}
