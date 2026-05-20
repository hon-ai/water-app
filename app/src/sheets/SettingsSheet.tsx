import { useCallback, useEffect, useState } from "react";
import { Sheet } from "./Sheet";
import { ipc, type DiagnosticsStatus } from "../ipc/commands";
import { onWaterEvent } from "../ipc/events";
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

/**
 * Format the lowercase provider id for display. Title-case the known
 * adapters, with explicit branding for OpenAI / Anthropic.
 */
function prettyProviderName(id: string): string {
  switch (id) {
    case "anthropic":
      return "Anthropic";
    case "openai":
      return "OpenAI";
    case "ollama":
      return "Ollama";
    case "llamacpp":
      return "llama.cpp";
    case "canned":
      return "Canned (test stub)";
    default:
      return id.charAt(0).toUpperCase() + id.slice(1);
  }
}

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

  // Initial snapshot fetch on open; then subscribe to sidecar:status +
  // provider:status events for incremental updates (no more 3-second
  // polling). Both subscriptions live in the same effect and share a
  // single `cancelled` guard so a fast close-before-resolve does not leak.
  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    let unsubSidecar: (() => void) | undefined;
    let unsubProvider: (() => void) | undefined;
    refresh();
    (async () => {
      const u = await onWaterEvent("sidecar:status", (p) => {
        setStatus((prev) => {
          if (prev === null) {
            // Cold-start race: an event arrived before the initial snapshot.
            // Re-fetch the snapshot so we don't drop the transition.
            void refresh();
            return prev;
          }
          return {
            ...prev,
            sidecar: prev.sidecar
              ? { ...prev.sidecar, status: p.status, last_status_detail: p.detail }
              : { base_url: "", status: p.status, last_status_detail: p.detail },
          };
        });
      });
      if (cancelled) {
        u();
        return;
      }
      unsubSidecar = u;
    })();
    (async () => {
      const u = await onWaterEvent("provider:status", (p) => {
        setStatus((prev) => {
          if (prev === null) {
            // Same cold-start race as sidecar:status — re-snapshot.
            void refresh();
            return prev;
          }
          // Locate the matching provider; if not present (router added a
          // provider mid-session), append it so the UI still reflects the
          // change.
          const idx = prev.provider_health.findIndex((ph) => ph.id === p.provider_id);
          const next = [...prev.provider_health];
          if (idx === -1) {
            next.push({ id: p.provider_id, ok: p.ok, error: p.error });
          } else {
            const existing = next[idx]!;
            next[idx] = { id: existing.id, ok: p.ok, error: p.error };
          }
          return { ...prev, provider_health: next };
        });
      });
      if (cancelled) {
        u();
        return;
      }
      unsubProvider = u;
    })();
    return () => {
      cancelled = true;
      unsubSidecar?.();
      unsubProvider?.();
    };
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
        <p
          style={{
            margin: "-4px 0 12px 0",
            fontSize: "var(--water-fs-meta)",
            color: "var(--water-fg-muted)",
            lineHeight: 1.5,
          }}
        >
          API keys live in <code style={{ fontFamily: "var(--water-font-mono)", background: "color-mix(in srgb, var(--water-fg-faint) 12%, transparent)", padding: "1px 4px", borderRadius: 4 }}>~/.water/dev-keys.toml</code>. Click Test to activate a provider for this session.
        </p>
        <ul style={{ listStyle: "none", margin: 0, padding: 0, display: "flex", flexDirection: "column", gap: 6 }}>
          {providers.map((p) => (
            <li
              key={p.id}
              style={{
                display: "flex",
                alignItems: "center",
                gap: 10,
                padding: "10px 14px",
                background: "var(--water-bg-canvas)",
                borderRadius: "var(--water-r-8)",
                border:
                  "1px solid color-mix(in srgb, var(--water-fg-faint) 10%, transparent)",
                transition:
                  "border-color var(--water-dur-tiny) var(--water-ease-out-soft)",
              }}
            >
              <span
                aria-hidden
                style={{
                  width: 8,
                  height: 8,
                  borderRadius: "50%",
                  background: p.ok
                    ? "color-mix(in srgb, var(--water-hue-flow) 80%, transparent)"
                    : "color-mix(in srgb, var(--water-fg-faint) 60%, transparent)",
                  boxShadow: p.ok
                    ? "0 0 10px color-mix(in srgb, var(--water-hue-flow) 60%, transparent)"
                    : "none",
                  flexShrink: 0,
                }}
              />
              <span
                style={{
                  flex: 1,
                  fontFamily: "var(--water-font-sans)",
                  fontSize: "var(--water-fs-ui)",
                  fontWeight: 500,
                }}
              >
                {prettyProviderName(p.id)}
              </span>
              {p.error && (
                <span
                  title={p.error}
                  style={{
                    fontSize: "var(--water-fs-meta)",
                    color: "var(--water-hue-drift)",
                    fontFamily: "var(--water-font-sans)",
                    maxWidth: 140,
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    whiteSpace: "nowrap",
                  }}
                >
                  {p.error}
                </span>
              )}
              <button
                type="button"
                onClick={() => handleTest(p.id)}
                disabled={testingId === p.id}
                style={{
                  padding: "5px 12px",
                  border: "none",
                  background: p.ok
                    ? "transparent"
                    : "color-mix(in srgb, var(--water-hue-flow) 22%, transparent)",
                  color: "var(--water-fg-default)",
                  cursor: testingId === p.id ? "wait" : "pointer",
                  borderRadius: "var(--water-r-8)",
                  boxShadow: p.ok
                    ? "inset 0 0 0 1px color-mix(in srgb, var(--water-fg-faint) 30%, transparent)"
                    : "none",
                  fontFamily: "var(--water-font-sans)",
                  fontSize: "var(--water-fs-meta)",
                  fontWeight: 500,
                  transition:
                    "background-color var(--water-dur-tiny) var(--water-ease-out-soft)",
                }}
              >
                {testingId === p.id ? "Testing…" : p.ok ? "Re-test" : "Test"}
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
