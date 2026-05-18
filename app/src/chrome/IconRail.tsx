import { Droplet, FileText, User, Globe, Settings } from "lucide-react";

export type NavTarget = "scenes" | "characters" | "world";

interface Props {
  active: NavTarget;
  onSelect: (target: NavTarget) => void;
  onOpenSettings: () => void;
}

const NAV: { id: NavTarget; label: string; Icon: typeof FileText }[] = [
  { id: "scenes", label: "Scenes", Icon: FileText },
  { id: "characters", label: "Characters", Icon: User },
  { id: "world", label: "World", Icon: Globe },
];

export function IconRail({ active, onSelect, onOpenSettings }: Props) {
  return (
    <nav
      aria-label="primary"
      style={{
        width: "var(--water-rail-w)",
        flexShrink: 0,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        gap: 8,
        padding: "12px 0",
        background: "var(--water-bg-canvas)",
        borderRight: "1px solid transparent",
      }}
    >
      <div aria-hidden style={{ padding: 8, color: "var(--water-hue-flow)" }}>
        <Droplet size={22} strokeWidth={1.75} />
      </div>
      <div style={{ flex: 1, display: "flex", flexDirection: "column", gap: 4, paddingTop: 12 }}>
        {NAV.map(({ id, label, Icon }) => (
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
              background: active === id ? "color-mix(in srgb, var(--water-hue-flow) 30%, transparent)" : "transparent",
              color: active === id ? "var(--water-fg-default)" : "var(--water-fg-muted)",
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
