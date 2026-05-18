import { useCallback, useEffect, useState } from "react";
import { Sheet } from "./Sheet";
import { ipc, type DiagnosticsStatus } from "../ipc/commands";
import { useTheme, type Theme } from "../theme/useTheme";

interface Props {
  open: boolean;
  onClose: () => void;
}

const THEMES: { id: Theme; label: string }[] = [
  { id: "light", label: "Light" },
  { id: "dark", label: "Dark" },
  { id: "auto", label: "Auto" },
];

export function SettingsSheet({ open, onClose }: Props) {
  const { theme, setTheme } = useTheme();
  const [status, setStatus] = useState<DiagnosticsStatus | null>(null);
  const [testingId, setTestingId] = useState<string | null>(null);
  const [testError, setTestError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const s = await ipc.diagnosticsStatus();
      setStatus(s);
    } catch {
      /* swallow */
    }
  }, []);

  useEffect(() => {
    if (!open) return;
    refresh();
    const t = window.setInterval(() => refresh(), 3000);
    return () => window.clearInterval(t);
  }, [open, refresh]);

  const handleTest = async (providerId: string) => {
    setTestError(null);
    setTestingId(providerId);
    try {
      await ipc.providerTest(providerId);
    } catch (e) {
      setTestError(String(e));
    } finally {
      setTestingId(null);
      refresh();
    }
  };

  const section: React.CSSProperties = {
    borderTop: "1px solid color-mix(in srgb, var(--water-fg-faint) 20%, transparent)",
    paddingTop: 16,
    marginTop: 16,
  };
  const heading: React.CSSProperties = {
    fontFamily: "var(--water-font-sans)",
    fontSize: "var(--water-fs-ui)",
    fontWeight: 600,
    color: "var(--water-fg-default)",
    marginBottom: 12,
    textTransform: "uppercase",
    letterSpacing: 0.4,
  };

  const providers =
    status?.provider_health && status.provider_health.length > 0
      ? status.provider_health
      : [
          { id: "canned", ok: false, error: null },
          { id: "anthropic", ok: false, error: null },
          { id: "openai", ok: false, error: null },
          { id: "ollama", ok: false, error: null },
          { id: "llamacpp", ok: false, error: null },
        ];

  return (
    <Sheet open={open} onClose={onClose} title="Settings">
      <section style={{ ...section, borderTop: "none", paddingTop: 0, marginTop: 0 }}>
        <div style={heading}>Appearance</div>
        <div style={{ display: "flex", gap: 6 }}>
          {THEMES.map((t) => (
            <button
              key={t.id}
              type="button"
              onClick={() => setTheme(t.id)}
              data-active={theme === t.id ? "true" : "false"}
              style={{
                flex: 1,
                padding: "8px 12px",
                border: "none",
                borderRadius: "var(--water-r-8)",
                cursor: "pointer",
                fontFamily: "var(--water-font-sans)",
                fontSize: "var(--water-fs-ui)",
                background:
                  theme === t.id
                    ? "color-mix(in srgb, var(--water-hue-flow) 40%, transparent)"
                    : "var(--water-bg-canvas)",
                color: "var(--water-fg-default)",
              }}
            >
              {t.label}
            </button>
          ))}
        </div>
      </section>

      <section style={section}>
        <div style={heading}>Providers</div>
        <ul style={{ listStyle: "none", margin: 0, padding: 0, display: "flex", flexDirection: "column", gap: 6 }}>
          {providers.map((p) => (
            <li
              key={p.id}
              style={{
                display: "flex",
                alignItems: "center",
                gap: 8,
                padding: "8px 12px",
                background: "var(--water-bg-canvas)",
                borderRadius: "var(--water-r-8)",
              }}
            >
              <span
                style={{
                  flex: 1,
                  fontFamily: "var(--water-font-sans)",
                  fontSize: "var(--water-fs-ui)",
                }}
              >
                {p.id}
              </span>
              <span
                style={{
                  fontSize: "var(--water-fs-meta)",
                  color: p.ok ? "var(--water-hue-flow)" : "var(--water-fg-faint)",
                }}
              >
                {p.ok ? "ok" : "—"}
              </span>
              <button
                type="button"
                onClick={() => handleTest(p.id)}
                disabled={testingId === p.id}
                style={{
                  padding: "4px 10px",
                  border: "none",
                  background: "transparent",
                  color: "var(--water-fg-default)",
                  cursor: "pointer",
                  borderRadius: "var(--water-r-8)",
                  boxShadow:
                    "inset 0 0 0 1px color-mix(in srgb, var(--water-fg-faint) 30%, transparent)",
                  fontFamily: "var(--water-font-sans)",
                  fontSize: "var(--water-fs-meta)",
                }}
              >
                {testingId === p.id ? "Testing…" : "Test"}
              </button>
            </li>
          ))}
        </ul>
        {testError && (
          <pre style={{ color: "var(--water-hue-drift)", marginTop: 12, fontSize: "var(--water-fs-meta)" }}>
            {testError}
          </pre>
        )}
      </section>

      <section style={section}>
        <div style={heading}>Developer info</div>
        <details>
          <summary
            style={{
              cursor: "pointer",
              fontFamily: "var(--water-font-sans)",
              fontSize: "var(--water-fs-ui)",
              color: "var(--water-fg-muted)",
            }}
          >
            Raw diagnostics JSON
          </summary>
          <pre
            style={{
              background: "var(--water-bg-canvas)",
              padding: 12,
              borderRadius: "var(--water-r-8)",
              fontFamily: "var(--water-font-mono)",
              fontSize: "var(--water-fs-meta)",
              overflow: "auto",
              marginTop: 8,
            }}
          >
            {JSON.stringify(status, null, 2)}
          </pre>
        </details>
      </section>
    </Sheet>
  );
}
