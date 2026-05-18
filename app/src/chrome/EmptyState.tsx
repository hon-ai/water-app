import { Droplet } from "lucide-react";

interface Props {
  onCreate: () => void;
  onOpen: () => void;
}

export function EmptyState({ onCreate, onOpen }: Props) {
  const btn: React.CSSProperties = {
    padding: "10px 18px",
    border: "none",
    borderRadius: "var(--water-r-16)",
    fontFamily: "var(--water-font-sans)",
    fontSize: "var(--water-fs-ui)",
    cursor: "pointer",
  };

  return (
    <main
      style={{
        flex: 1,
        background: "var(--water-bg-paper)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          gap: 16,
          maxWidth: 420,
          textAlign: "center",
        }}
      >
        <div aria-hidden style={{ color: "var(--water-hue-flow)" }}>
          <Droplet size={56} strokeWidth={1.25} />
        </div>
        <h1
          style={{
            margin: 0,
            fontFamily: "var(--water-font-serif)",
            fontSize: "var(--water-fs-display)",
            lineHeight: "var(--water-lh-display)",
            fontWeight: 500,
            color: "var(--water-fg-default)",
          }}
        >
          Water
        </h1>
        <p
          style={{
            margin: 0,
            color: "var(--water-fg-muted)",
            fontFamily: "var(--water-font-sans)",
            fontSize: "var(--water-fs-body)",
            lineHeight: "var(--water-lh-body)",
          }}
        >
          Begin a project, or open one you've already started.
        </p>
        <div style={{ display: "flex", gap: 12, marginTop: 8 }}>
          <button
            type="button"
            onClick={onCreate}
            style={{
              ...btn,
              background: "color-mix(in srgb, var(--water-hue-flow) 50%, transparent)",
              color: "var(--water-fg-default)",
            }}
          >
            Create new project
          </button>
          <button
            type="button"
            onClick={onOpen}
            style={{
              ...btn,
              background: "transparent",
              color: "var(--water-fg-default)",
              boxShadow: "inset 0 0 0 1px color-mix(in srgb, var(--water-fg-faint) 30%, transparent)",
            }}
          >
            Open existing
          </button>
        </div>
      </div>
    </main>
  );
}
