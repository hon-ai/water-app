import { FileText, User, Globe, Map, Settings } from "lucide-react";
import { StreamMark } from "./StreamMark";

export type NavTarget = "scenes" | "characters" | "world" | "canvas";

interface Props {
  active: NavTarget;
  onSelect: (target: NavTarget) => void;
  onOpenSettings: () => void;
  /**
   * Hides the Scenes / Characters / World / Canvas nav buttons when
   * there is no open project. They would otherwise be
   * visible-but-dead — clicking one sets `activeNav` but the
   * right-side surface always renders `<EmptyState>` while
   * `projectOpen === false`, so the writer perceives them as broken.
   */
  projectOpen: boolean;
  /**
   * Called when the writer clicks the StreamMark at the top of the
   * rail. The shell closes the active project (if any), returning
   * to the EmptyState splash.
   */
  onGoHome?: () => void;
}

const NAV: { id: NavTarget; label: string; Icon: typeof FileText }[] = [
  { id: "scenes", label: "Scenes", Icon: FileText },
  { id: "characters", label: "Characters", Icon: User },
  { id: "world", label: "World", Icon: Globe },
  { id: "canvas", label: "Canvas", Icon: Map },
];

export function IconRail({ active, onSelect, onOpenSettings, projectOpen, onGoHome }: Props) {
  return (
    <nav
      aria-label="primary"
      className="water-floating-panel"
      style={{
        width: "var(--water-rail-w)",
        flexShrink: 0,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        gap: 8,
        padding: "12px 0",
        // Floating glass panel: offset from the window edge, fully
        // rounded corners, hairline border on all sides. Sits over the
        // ambient background like a pebble in a stream.
        margin: "10px 0 10px 10px",
        background:
          "color-mix(in srgb, var(--water-bg-paper) 55%, transparent)",
        backdropFilter: "blur(22px) saturate(160%)",
        WebkitBackdropFilter: "blur(22px) saturate(160%)",
        border:
          "1px solid color-mix(in srgb, var(--water-hairline) 60%, transparent)",
        borderRadius: "var(--water-r-24)",
        boxShadow: "var(--water-elev-2)",
      }}
    >
      <button
        type="button"
        aria-label="Home"
        title="Return to home"
        onClick={onGoHome}
        disabled={!onGoHome}
        style={{
          padding: 8,
          border: "none",
          background: "transparent",
          color: "var(--water-sea-400)",
          cursor: onGoHome ? "pointer" : "default",
          display: "grid",
          placeItems: "center",
          borderRadius: "var(--water-r-16)",
          transition:
            "color var(--water-dur-tiny) var(--water-ease-out-soft)",
        }}
        onMouseEnter={(e) => {
          if (onGoHome)
            e.currentTarget.style.color = "var(--water-sea-glow)";
        }}
        onMouseLeave={(e) => {
          e.currentTarget.style.color = "var(--water-sea-400)";
        }}
      >
        <StreamMark size={26} />
      </button>
      <div style={{ flex: 1, display: "flex", flexDirection: "column", gap: 4, paddingTop: 12 }}>
        {projectOpen &&
          NAV.map(({ id, label, Icon }) => (
            <button
              key={id}
              type="button"
              title={label}
              aria-label={label}
              data-active={active === id ? "true" : "false"}
              onClick={() => onSelect(id)}
              style={{
                width: 40,
                height: 40,
                display: "grid",
                placeItems: "center",
                border: "none",
                borderRadius: "var(--water-r-16)",
                cursor: "pointer",
                background:
                  active === id
                    ? "color-mix(in srgb, var(--water-hue-flow) 30%, transparent)"
                    : "transparent",
                color:
                  active === id
                    ? "var(--water-fg-default)"
                    : "var(--water-fg-muted)",
                transition: `background-color var(--water-dur-tiny) var(--water-ease-out-soft)`,
              }}
            >
              <Icon size={18} strokeWidth={1.5} />
            </button>
          ))}
      </div>
      <button
        type="button"
        title="Settings"
        aria-label="Settings"
        onClick={onOpenSettings}
        style={{
          width: 40,
          height: 40,
          display: "grid",
          placeItems: "center",
          border: "none",
          borderRadius: "var(--water-r-16)",
          cursor: "pointer",
          background: "transparent",
          color: "var(--water-fg-muted)",
        }}
      >
        <Settings size={18} strokeWidth={1.5} />
      </button>
    </nav>
  );
}
