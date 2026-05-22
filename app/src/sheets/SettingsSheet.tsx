import { useCallback, useEffect, useRef, useState } from "react";
import { Eye, EyeOff, ExternalLink, Upload, Trash2 } from "lucide-react";
import { Sheet } from "./Sheet";
import { ipc, type DiagnosticsStatus } from "../ipc/commands";
import { onWaterEvent } from "../ipc/events";
import { useTheme, type Theme } from "../theme/useTheme";
import {
  allFontOptions,
  currentFontId,
  FONT_OPTIONS,
  importCustomFont,
  loadCustomFonts,
  removeCustomFont,
  setFont,
  type CustomFont,
} from "../theme/fonts";
import {
  getActiveModel,
  MODEL_OPTIONS,
  setActiveModel,
} from "../theme/providerModels";
import { GlassSelect } from "../chrome/GlassSelect";

/**
 * Where the writer goes to mint a fresh API key for a given provider.
 * Used by the "Get a key →" link next to each provider row in
 * Settings so testers without an existing key know where to look.
 *
 * Local-only providers (Ollama, llama.cpp) return null — they don't
 * need a key. `canned` is a test stub; same.
 */
function keyProviderUrl(providerId: string): string | null {
  switch (providerId) {
    case "anthropic":
      return "https://console.anthropic.com/settings/keys";
    case "openai":
      return "https://platform.openai.com/api-keys";
    case "kimi":
      return "https://platform.moonshot.ai/console/api-keys";
    case "openrouter":
      return "https://openrouter.ai/keys";
    case "gemini":
      return "https://aistudio.google.com/apikey";
    default:
      return null;
  }
}

/**
 * True for providers that don't require an API key (local inference).
 * Their rows skip the key input and only show Test + model picker.
 */
function needsApiKey(providerId: string): boolean {
  return (
    providerId === "anthropic" ||
    providerId === "openai" ||
    providerId === "kimi" ||
    providerId === "openrouter" ||
    providerId === "gemini"
  );
}

/**
 * Translate the raw provider error string from the orchestrator into
 * a sentence a non-developer can act on. Falls through to the raw
 * message when we don't recognize the shape — better noisy than
 * silently swallowing detail we'll need for support.
 */
function friendlyError(raw: string, providerId: string): string {
  const pretty = prettyProviderName(providerId);
  const lower = raw.toLowerCase();
  if (lower.includes("no secret for provider")) {
    return `${pretty} is missing an API key. Paste one below and click Save.`;
  }
  if (lower.includes("no primary provider")) {
    return `${pretty} isn't active yet. Click Test to activate it for this session.`;
  }
  if (lower.includes("401") || lower.includes("unauthorized")) {
    return `${pretty} rejected the key as invalid (401). Double-check the value and try again.`;
  }
  if (lower.includes("402") || lower.includes("payment")) {
    return `${pretty} reports no credit / billing on the account (402). Add a payment method on the provider's site and re-test.`;
  }
  if (lower.includes("403") || lower.includes("forbidden")) {
    return `${pretty} forbade the request (403). The key may not have access to the chosen model — try a different one in the Model picker.`;
  }
  if (lower.includes("404")) {
    return `${pretty} doesn't recognize the model id. Open the Model picker and pick a different one (the curated list always works).`;
  }
  if (lower.includes("429")) {
    return `${pretty} rate-limited the request (429). Wait a minute and re-test, or check your account's quota on the provider's site.`;
  }
  if (
    lower.includes("connection refused") ||
    lower.includes("dns") ||
    lower.includes("could not resolve") ||
    lower.includes("network")
  ) {
    return `Couldn't reach ${pretty}. Check your internet connection.`;
  }
  if (lower.includes("bouquet json") || lower.includes("no json array")) {
    return `${pretty} returned a response Water couldn't parse. Try a different model in the Model picker — some smaller models don't reliably output JSON.`;
  }
  return raw;
}

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
    case "kimi":
      return "Kimi (Moonshot)";
    case "openrouter":
      return "OpenRouter";
    case "gemini":
      return "Google Gemini";
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
      // Pass the writer's chosen model so Test exercises THAT model,
      // not the adapter's hardcoded default. Empty/unset selection
      // falls back to default server-side.
      const model = getActiveModel(providerId);
      await ipc.providerTest(providerId, model || undefined);
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
          { id: "kimi", ok: false, error: null },
          { id: "openrouter", ok: false, error: null },
          { id: "gemini", ok: false, error: null },
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
        <div style={heading}>Manuscript font</div>
        <p
          style={{
            margin: "-4px 0 12px 0",
            fontSize: "var(--water-fs-meta)",
            color: "var(--water-fg-muted)",
            lineHeight: 1.5,
          }}
        >
          The serif used in the editor body and scene titles. UI fonts
          stay the same.
        </p>
        <ManuscriptFontPicker />
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
          Paste an API key, then click Test. Keys live in your OS keychain —
          Water never stores them in plain files.
        </p>
        <ul style={{ listStyle: "none", margin: 0, padding: 0, display: "flex", flexDirection: "column", gap: 6 }}>
          {providers.map((p) => (
            <li
              key={p.id}
              style={{
                display: "flex",
                flexDirection: "column",
                gap: 8,
                padding: "10px 14px",
                background: "var(--water-bg-canvas)",
                borderRadius: "var(--water-r-8)",
                border:
                  "1px solid color-mix(in srgb, var(--water-fg-faint) 10%, transparent)",
                transition:
                  "border-color var(--water-dur-tiny) var(--water-ease-out-soft)",
              }}
            >
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 10,
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
              </div>
              <ProviderModelPicker providerId={p.id} />
              {needsApiKey(p.id) && (
                <ProviderKeyInput
                  providerId={p.id}
                  onSaved={() => void handleTest(p.id)}
                />
              )}
              {p.error && (
                <div
                  data-testid={`provider-error-${p.id}`}
                  style={{
                    margin: "0",
                    padding: "8px 10px",
                    background:
                      "color-mix(in srgb, var(--water-hue-drift) 12%, transparent)",
                    border:
                      "1px solid color-mix(in srgb, var(--water-hue-drift) 28%, transparent)",
                    borderRadius: "var(--water-r-8)",
                    color: "var(--water-hue-drift)",
                    fontFamily: "var(--water-font-sans)",
                    fontSize: "var(--water-fs-meta)",
                    lineHeight: 1.5,
                  }}
                >
                  {friendlyError(p.error, p.id)}
                </div>
              )}
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
      <section style={section}>
        <div style={heading}>Adaptive nudges</div>
        <p
          style={{
            margin: "0 0 12px 0",
            color: "var(--water-fg-muted)",
            fontFamily: "var(--water-font-sans)",
            fontSize: "var(--water-fs-meta)",
            lineHeight: 1.5,
          }}
        >
          Water learns which nudges you engage with and dials each trigger up
          or down. Reset wipes that learning for this project — the next
          session starts neutral.
        </p>
        <ResetLearningButton />
      </section>
    </Sheet>
  );
}

/**
 * Two-step "reset trigger learning" affordance. First click puts the
 * button into a confirmation state; the second confirms. Avoids a
 * modal dialog (which would break the SettingsSheet's flow) while
 * still making the destructive action deliberate.
 */
function ResetLearningButton() {
  const [phase, setPhase] = useState<"idle" | "confirm" | "resetting" | "done">(
    "idle",
  );
  const onClick = async () => {
    if (phase === "idle") {
      setPhase("confirm");
      return;
    }
    if (phase === "confirm") {
      setPhase("resetting");
      try {
        await ipc.feedbackReset();
        setPhase("done");
        window.setTimeout(() => setPhase("idle"), 2200);
      } catch {
        setPhase("idle");
      }
    }
  };
  const onBlur = () => {
    // Cancel the confirm prompt if the writer clicks away without
    // committing. Resetting/done are sticky.
    if (phase === "confirm") setPhase("idle");
  };
  const label =
    phase === "idle"
      ? "Reset trigger learning"
      : phase === "confirm"
        ? "Click again to confirm"
        : phase === "resetting"
          ? "Resetting…"
          : "Reset.";
  return (
    <button
      type="button"
      onClick={onClick}
      onBlur={onBlur}
      data-testid="reset-trigger-learning"
      data-phase={phase}
      style={{
        padding: "8px 14px",
        border:
          "1px solid color-mix(in srgb, var(--water-fg-faint) 30%, transparent)",
        borderRadius: "var(--water-r-8)",
        background:
          phase === "confirm"
            ? "color-mix(in srgb, var(--water-sea-600) 12%, transparent)"
            : "transparent",
        color: "var(--water-fg-default)",
        fontFamily: "var(--water-font-sans)",
        fontSize: "var(--water-fs-meta)",
        cursor: phase === "resetting" ? "wait" : "pointer",
        transition:
          "background var(--water-dur-tiny) var(--water-ease-out-soft)",
      }}
      disabled={phase === "resetting"}
    >
      {label}
    </button>
  );
}

/**
 * Manuscript-serif picker. Reads the active id from
 * `theme/fonts.currentFontId()` on mount, renders a styled <select>
 * over the curated `FONT_OPTIONS`, and applies the chosen family via
 * `setFont` on change. The CSS custom property override propagates
 * through everywhere the editor uses `var(--water-font-serif)` so
 * the change shows up on the next paint without an editor reload.
 */
function ManuscriptFontPicker() {
  const [active, setActive] = useState<string>(currentFontId());
  const [customFonts, setCustomFonts] = useState<CustomFont[]>(
    loadCustomFonts(),
  );
  const [importError, setImportError] = useState<string | null>(null);
  const [importing, setImporting] = useState(false);
  const fileInputRef = useRef<HTMLInputElement | null>(null);

  const handleImport = async (file: File) => {
    setImportError(null);
    setImporting(true);
    try {
      const font = await importCustomFont(file);
      setCustomFonts(loadCustomFonts());
      // Auto-switch to the newly-imported font so the writer
      // immediately sees their import take effect.
      setFont(font.id);
      setActive(font.id);
    } catch (e) {
      setImportError(e instanceof Error ? e.message : String(e));
    } finally {
      setImporting(false);
      // Clear the input so re-importing the same file fires onChange.
      if (fileInputRef.current) fileInputRef.current.value = "";
    }
  };

  const handleRemove = (id: string) => {
    removeCustomFont(id);
    setCustomFonts(loadCustomFonts());
    if (active === id) {
      // The active font just got removed — fall back to the default.
      const fallback = FONT_OPTIONS[0]!;
      setFont(fallback.id);
      setActive(fallback.id);
    }
  };

  const options = [
    ...FONT_OPTIONS.map((o) => ({
      value: o.id,
      label: o.label,
      fontFamily: o.family,
    })),
    ...customFonts.map((o) => ({
      value: o.id,
      label: `${o.label} (imported)`,
      fontFamily: o.family,
    })),
  ];

  const activeOption = allFontOptions().find((o) => o.id === active);
  const activeIsCustom = customFonts.some((c) => c.id === active);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      <GlassSelect
        testId="manuscript-font-picker"
        ariaLabel="Manuscript font"
        value={active}
        options={options}
        onChange={(next) => {
          setFont(next);
          setActive(next);
        }}
        triggerStyle={{ fontSize: "var(--water-fs-ui)" }}
      />
      <div
        // Live preview rendered in the picked face so the writer sees
        // the choice without leaving the Settings sheet.
        style={{
          marginTop: 4,
          padding: "10px 12px",
          borderRadius: "var(--water-r-8)",
          background:
            "color-mix(in srgb, var(--water-bg-paper) 60%, transparent)",
          border:
            "1px solid color-mix(in srgb, var(--water-hairline) 40%, transparent)",
          fontFamily: activeOption?.family ?? "serif",
          fontSize: 15,
          lineHeight: 1.55,
          color: "var(--water-fg-default)",
        }}
      >
        She walked through the wet street, and the bell rang somewhere
        behind them — softly, as if listening.
      </div>
      <div
        style={{
          fontSize: "var(--water-fs-meta)",
          color: "var(--water-fg-muted)",
          lineHeight: 1.5,
        }}
      >
        {activeOption?.hint ?? ""}
      </div>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 8,
          marginTop: 4,
        }}
      >
        <input
          ref={fileInputRef}
          data-testid="manuscript-font-import-input"
          type="file"
          accept=".ttf,.otf,.woff,.woff2,font/ttf,font/otf,font/woff,font/woff2"
          style={{ display: "none" }}
          onChange={(e) => {
            const file = e.currentTarget.files?.[0];
            if (file) void handleImport(file);
          }}
        />
        <button
          type="button"
          data-testid="manuscript-font-import-button"
          onClick={() => fileInputRef.current?.click()}
          disabled={importing}
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: 6,
            padding: "5px 12px",
            border:
              "1px solid color-mix(in srgb, var(--water-fg-faint) 22%, transparent)",
            borderRadius: "var(--water-r-8)",
            background:
              "color-mix(in srgb, var(--water-bg-paper) 70%, transparent)",
            color: "var(--water-fg-default)",
            cursor: importing ? "wait" : "pointer",
            fontFamily: "var(--water-font-sans)",
            fontSize: "var(--water-fs-meta)",
          }}
        >
          <Upload size={12} aria-hidden />
          {importing ? "Importing…" : "Import font…"}
        </button>
        {activeIsCustom && (
          <button
            type="button"
            data-testid="manuscript-font-remove-button"
            title="Remove this imported font"
            onClick={() => handleRemove(active)}
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: 6,
              padding: "5px 12px",
              border:
                "1px solid color-mix(in srgb, var(--water-hue-drift) 28%, transparent)",
              borderRadius: "var(--water-r-8)",
              background:
                "color-mix(in srgb, var(--water-hue-drift) 12%, transparent)",
              color: "var(--water-fg-default)",
              cursor: "pointer",
              fontFamily: "var(--water-font-sans)",
              fontSize: "var(--water-fs-meta)",
            }}
          >
            <Trash2 size={12} aria-hidden />
            Remove
          </button>
        )}
        <span
          style={{
            fontSize: 10,
            color: "var(--water-fg-muted)",
            lineHeight: 1.4,
            flex: 1,
          }}
        >
          .ttf · .otf · .woff · .woff2 — stays on this machine.
        </span>
      </div>
      {importError && (
        <div
          data-testid="manuscript-font-import-error"
          style={{
            padding: "8px 10px",
            background:
              "color-mix(in srgb, var(--water-hue-drift) 12%, transparent)",
            border:
              "1px solid color-mix(in srgb, var(--water-hue-drift) 28%, transparent)",
            borderRadius: "var(--water-r-8)",
            color: "var(--water-hue-drift)",
            fontFamily: "var(--water-font-sans)",
            fontSize: "var(--water-fs-meta)",
            lineHeight: 1.5,
          }}
        >
          {importError}
        </div>
      )}
    </div>
  );
}

/**
 * Per-provider model picker. Dropdown of curated entries +
 * free-text input for arbitrary model ids (OpenRouter writers can
 * type any model in the catalog). Choice persists in localStorage
 * and pushes into the live router via `ipc.providerSetModel`.
 */
/**
 * Per-provider API-key input. Writers paste their key here and click
 * Save; the key persists into the OS keychain via `providerSetKey`
 * (one-way write — we never read the key back). After a successful
 * save we auto-fire Test so the writer immediately sees the green
 * dot and doesn't have to click twice.
 *
 * Show/hide toggle defaults to hidden (`type="password"`) so the key
 * isn't visible at rest. Local-only providers (Ollama, llama.cpp)
 * skip this entirely.
 */
function ProviderKeyInput({
  providerId,
  onSaved,
}: {
  providerId: string;
  onSaved?: () => void;
}) {
  const [value, setValue] = useState("");
  const [show, setShow] = useState(false);
  const [phase, setPhase] = useState<"idle" | "saving" | "saved" | "error">(
    "idle",
  );
  const [savedMsg, setSavedMsg] = useState<string | null>(null);

  // Fade the "Saved" chip after a moment so it doesn't linger.
  useEffect(() => {
    if (phase !== "saved") return;
    const id = window.setTimeout(() => {
      setPhase("idle");
      setSavedMsg(null);
    }, 2500);
    return () => window.clearTimeout(id);
  }, [phase]);

  const url = keyProviderUrl(providerId);

  const handleSave = async () => {
    const trimmed = value.trim();
    if (trimmed.length === 0) return;
    setPhase("saving");
    try {
      await ipc.providerSetKey(providerId, trimmed);
      setSavedMsg("Saved. Testing…");
      setPhase("saved");
      setValue("");
      setShow(false);
      onSaved?.();
    } catch {
      setPhase("error");
    }
  };

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 4,
        paddingLeft: 18,
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 8,
        }}
      >
        <label
          style={{
            fontFamily: "var(--water-font-sans)",
            fontSize: 10,
            color: "var(--water-fg-muted)",
            fontWeight: 600,
            textTransform: "uppercase",
            letterSpacing: 0.4,
            minWidth: 38,
          }}
          htmlFor={`provider-key-${providerId}`}
        >
          Key
        </label>
        <div style={{ flex: 1, display: "flex", gap: 6, minWidth: 0 }}>
          <input
            id={`provider-key-${providerId}`}
            data-testid={`provider-key-input-${providerId}`}
            type={show ? "text" : "password"}
            value={value}
            placeholder={phase === "saved" ? "Saved" : "Paste API key"}
            onChange={(e) => setValue(e.currentTarget.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                void handleSave();
              }
            }}
            autoComplete="off"
            spellCheck={false}
            style={{
              flex: 1,
              minWidth: 0,
              padding: "4px 8px",
              border:
                "1px solid color-mix(in srgb, var(--water-fg-faint) 22%, transparent)",
              borderRadius: "var(--water-r-8)",
              background:
                "color-mix(in srgb, var(--water-bg-paper) 70%, transparent)",
              color: "var(--water-fg-default)",
              fontFamily: "var(--water-font-mono)",
              fontSize: "var(--water-fs-meta)",
            }}
          />
          <button
            type="button"
            aria-label={show ? "Hide key" : "Show key"}
            title={show ? "Hide" : "Show"}
            onClick={() => setShow((s) => !s)}
            style={{
              width: 24,
              height: 24,
              display: "grid",
              placeItems: "center",
              border:
                "1px solid color-mix(in srgb, var(--water-fg-faint) 22%, transparent)",
              borderRadius: "var(--water-r-8)",
              background: "transparent",
              color: "var(--water-fg-muted)",
              cursor: "pointer",
            }}
          >
            {show ? <EyeOff size={12} /> : <Eye size={12} />}
          </button>
          <button
            type="button"
            data-testid={`provider-key-save-${providerId}`}
            onClick={() => void handleSave()}
            disabled={phase === "saving" || value.trim().length === 0}
            style={{
              padding: "4px 12px",
              border: "none",
              background:
                value.trim().length === 0
                  ? "color-mix(in srgb, var(--water-fg-faint) 18%, transparent)"
                  : "color-mix(in srgb, var(--water-hue-flow) 22%, transparent)",
              color: "var(--water-fg-default)",
              cursor:
                phase === "saving" || value.trim().length === 0
                  ? "default"
                  : "pointer",
              borderRadius: "var(--water-r-8)",
              fontFamily: "var(--water-font-sans)",
              fontSize: "var(--water-fs-meta)",
              fontWeight: 500,
            }}
          >
            {phase === "saving"
              ? "Saving…"
              : phase === "saved"
                ? "Saved"
                : "Save"}
          </button>
        </div>
      </div>
      <div
        style={{
          paddingLeft: 46,
          fontSize: 10,
          color: "var(--water-fg-muted)",
          lineHeight: 1.4,
          display: "flex",
          alignItems: "center",
          gap: 6,
        }}
      >
        {url && (
          <a
            href={url}
            onClick={(e) => {
              e.preventDefault();
              void ipc.openExternalLink(url).catch(() => {});
            }}
            style={{
              color: "var(--water-fg-muted)",
              textDecoration: "none",
              display: "inline-flex",
              alignItems: "center",
              gap: 3,
              borderBottom:
                "1px dotted color-mix(in srgb, var(--water-fg-muted) 60%, transparent)",
            }}
          >
            Get a key
            <ExternalLink size={10} aria-hidden />
          </a>
        )}
        {phase === "error" && (
          <span style={{ color: "var(--water-hue-drift)" }}>
            Save failed — try again.
          </span>
        )}
        {savedMsg && phase === "saved" && (
          <span style={{ color: "var(--water-hue-flow)" }}>{savedMsg}</span>
        )}
      </div>
    </div>
  );
}

function ProviderModelPicker({ providerId }: { providerId: string }) {
  const options = MODEL_OPTIONS[providerId] ?? [];
  const [active, setActive] = useState<string>(() => getActiveModel(providerId));
  const [custom, setCustom] = useState<string>("");
  // True when the active model isn't in the curated list — the
  // dropdown shows "Custom" and the input below is enabled.
  const knownIds = options.map((o) => o.id);
  const isCustom = active.length > 0 && !knownIds.includes(active);
  return (
    <div
      data-testid={`provider-model-picker-${providerId}`}
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 4,
        paddingLeft: 18,
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 8,
        }}
      >
        <label
          style={{
            fontFamily: "var(--water-font-sans)",
            fontSize: 10,
            color: "var(--water-fg-muted)",
            fontWeight: 600,
            textTransform: "uppercase",
            letterSpacing: 0.4,
            minWidth: 38,
          }}
        >
          Model
        </label>
        <div style={{ flex: 1, minWidth: 0 }}>
          <GlassSelect
            testId={`provider-model-select-${providerId}`}
            ariaLabel={`Model for ${providerId}`}
            value={isCustom ? "__custom__" : active}
            options={[
              ...options.map((o) => ({
                value: o.id,
                label: o.label,
                hint: o.hint,
              })),
              { value: "__custom__", label: "Custom…" },
            ]}
            onChange={(next) => {
              if (next === "__custom__") {
                setActive(custom || "");
                void setActiveModel(providerId, custom || "");
              } else {
                setActive(next);
                void setActiveModel(providerId, next);
              }
            }}
          />
        </div>
      </div>
      {isCustom && (
        <div style={{ display: "flex", gap: 8, paddingLeft: 46 }}>
          <input
            type="text"
            value={custom || active}
            placeholder="model id"
            onChange={(e) => setCustom(e.currentTarget.value)}
            onBlur={() => {
              const next = custom.trim();
              if (next) {
                setActive(next);
                void setActiveModel(providerId, next);
              }
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                const next = custom.trim();
                if (next) {
                  setActive(next);
                  void setActiveModel(providerId, next);
                }
              }
            }}
            style={{
              flex: 1,
              padding: "3px 8px",
              border:
                "1px solid color-mix(in srgb, var(--water-fg-faint) 22%, transparent)",
              borderRadius: "var(--water-r-8)",
              background:
                "color-mix(in srgb, var(--water-bg-paper) 70%, transparent)",
              color: "var(--water-fg-default)",
              fontFamily: "var(--water-font-mono)",
              fontSize: "var(--water-fs-meta)",
            }}
          />
        </div>
      )}
      {!isCustom && options.find((o) => o.id === active)?.hint && (
        <div
          style={{
            paddingLeft: 46,
            fontSize: 10,
            color: "var(--water-fg-muted)",
            lineHeight: 1.4,
          }}
        >
          {options.find((o) => o.id === active)!.hint}
        </div>
      )}
    </div>
  );
}
