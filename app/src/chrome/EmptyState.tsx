import { StreamMark } from "./StreamMark";

interface Props {
  onCreate: () => void;
  onOpen: () => void;
}

export function EmptyState({ onCreate, onOpen }: Props) {
  // Glass-on-tint primary: sea-tint gradient laid over translucent
  // paper so the stream reads through. Matches the rest of the
  // matte-glass UI system.
  const primaryBtn: React.CSSProperties = {
    padding: "10px 20px",
    border:
      "1px solid color-mix(in srgb, var(--water-hairline) 65%, transparent)",
    borderRadius: "var(--water-r-16)",
    fontFamily: "var(--water-font-sans)",
    fontSize: "var(--water-fs-ui)",
    fontWeight: 500,
    cursor: "pointer",
    background:
      "linear-gradient(135deg, color-mix(in srgb, var(--water-sea-300) 32%, transparent), color-mix(in srgb, var(--water-sea-glow) 16%, transparent)), color-mix(in srgb, var(--water-bg-paper) 68%, transparent)",
    backdropFilter: "blur(20px) saturate(160%) contrast(104%)",
    WebkitBackdropFilter: "blur(20px) saturate(160%) contrast(104%)",
    color: "var(--water-fg-default)",
    boxShadow:
      "var(--water-elev-1), 0 0 24px color-mix(in srgb, var(--water-sea-glow) 20%, transparent), inset 0 1px 0 color-mix(in srgb, white 22%, transparent)",
    transition:
      "background var(--water-dur-tiny) var(--water-ease-out-soft), box-shadow var(--water-dur-tiny) var(--water-ease-out-soft)",
  };
  const secondaryBtn: React.CSSProperties = {
    padding: "10px 20px",
    border:
      "1px solid color-mix(in srgb, var(--water-hairline) 65%, transparent)",
    borderRadius: "var(--water-r-16)",
    fontFamily: "var(--water-font-sans)",
    fontSize: "var(--water-fs-ui)",
    fontWeight: 500,
    cursor: "pointer",
    background: "color-mix(in srgb, var(--water-bg-paper) 70%, transparent)",
    backdropFilter: "blur(20px) saturate(160%) contrast(104%)",
    WebkitBackdropFilter: "blur(20px) saturate(160%) contrast(104%)",
    color: "var(--water-fg-default)",
    boxShadow:
      "var(--water-elev-1), inset 0 1px 0 color-mix(in srgb, white 18%, transparent)",
    transition:
      "background var(--water-dur-tiny) var(--water-ease-out-soft)",
  };

  return (
    <main
      style={{
        flex: 1,
        background: "transparent",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        position: "relative",
        overflow: "hidden",
      }}
    >
      {/* Ambient hue wash behind the content — soft, off-center. */}
      <div
        aria-hidden
        style={{
          position: "absolute",
          inset: 0,
          background:
            "radial-gradient(circle at 30% 40%, color-mix(in srgb, var(--water-hue-flow) 14%, transparent), transparent 60%)",
          pointerEvents: "none",
        }}
      />
      <div
        style={{
          position: "relative",
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          gap: 18,
          maxWidth: 440,
          textAlign: "center",
          padding: "0 24px",
          animation:
            "water-pill-fade-in var(--water-dur-medium) var(--water-ease-out-soft) both",
        }}
      >
        <div
          aria-hidden
          style={{
            color: "var(--water-sea-400)",
            filter:
              "drop-shadow(0 0 22px color-mix(in srgb, var(--water-sea-glow) 50%, transparent))",
          }}
        >
          <StreamMark size={72} />
        </div>
        <h1
          style={{
            margin: 0,
            fontFamily: "var(--water-font-serif)",
            fontSize: "var(--water-fs-display)",
            lineHeight: "var(--water-lh-display)",
            fontWeight: 500,
            letterSpacing: -0.4,
            color: "var(--water-fg-default)",
          }}
        >
          Just flow.
        </h1>
        <p
          style={{
            margin: 0,
            color: "var(--water-fg-muted)",
            fontFamily: "var(--water-font-sans)",
            fontSize: "var(--water-fs-body)",
            lineHeight: 1.55,
          }}
        >
          A writing surface for true immersion.
        </p>
        <div style={{ display: "flex", gap: 12, marginTop: 4 }}>
          <button type="button" onClick={onCreate} style={primaryBtn}>
            Create new project
          </button>
          <button type="button" onClick={onOpen} style={secondaryBtn}>
            Open existing
          </button>
        </div>
      </div>
    </main>
  );
}
