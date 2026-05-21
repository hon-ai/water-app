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
    padding: "9px 12px",
    border: "none",
    background: "transparent",
    color: "var(--water-fg-default)",
    cursor: "pointer",
    borderRadius: "var(--water-r-8)",
    fontFamily: "var(--water-font-sans)",
    fontSize: "var(--water-fs-ui)",
    transition:
      "background-color var(--water-dur-tiny) var(--water-ease-out-soft)",
  };
  const onItemEnter = (e: React.MouseEvent<HTMLButtonElement>) => {
    e.currentTarget.style.background =
      "color-mix(in srgb, var(--water-hue-flow) 14%, transparent)";
  };
  const onItemLeave = (e: React.MouseEvent<HTMLButtonElement>) => {
    e.currentTarget.style.background = "transparent";
  };

  return (
    <div
      ref={ref}
      role="menu"
      className="water-floating-panel"
      style={{
        // Positioned below the project-name button in the ScenesPanel
        // header. The wrapping container in App.tsx is a flex row that
        // also contains the full-height ScenesPanel, so `top: 100%`
        // anchors the menu at the bottom of the viewport (off-screen
        // behind the editor canvas). Anchor by a fixed offset matching
        // the panel's header padding + button height instead.
        position: "absolute",
        top: 56,
        left: 20,
        minWidth: 220,
        padding: 6,
        borderRadius: "var(--water-r-16)",
        zIndex: "var(--water-z-tooltip)" as unknown as number,
      }}
    >
      <button
        type="button"
        role="menuitem"
        style={item}
        onMouseEnter={onItemEnter}
        onMouseLeave={onItemLeave}
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
        onMouseEnter={onItemEnter}
        onMouseLeave={onItemLeave}
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
