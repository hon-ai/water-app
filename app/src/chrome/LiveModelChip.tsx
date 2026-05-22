// Live-model indicator chip. Renders in the bottom-right corner of
// the writer (the editor `<main>`) showing which LLM is currently
// authoritative for pill generation. Reads:
//
//   - diagnostics_status.router_primary_id → which provider is active
//   - diagnostics_status.provider_health → whether it's green
//   - localStorage `water:provider-model:<id>` → the model the writer
//     picked in Settings (or the curated default for that provider)
//
// Self-refreshes on `provider:status` events so a Test in Settings
// flips the chip immediately without waiting for the 3-second poll.
//
// Hidden when no provider has been Tested green — the global
// `<NoProviderBanner>` already covers that case.

import { useEffect, useState } from "react";
import { Zap } from "lucide-react";
import { ipc } from "../ipc/commands";
import { onWaterEvent } from "../ipc/events";
import { getActiveModel } from "../theme/providerModels";

/** Human-readable provider name. Mirrors `prettyProviderName` in
 *  SettingsSheet but kept local to avoid pulling the whole sheet in. */
function prettyProvider(id: string): string {
  switch (id) {
    case "anthropic":
      return "Anthropic";
    case "openai":
      return "OpenAI";
    case "kimi":
      return "Kimi";
    case "openrouter":
      return "OpenRouter";
    case "gemini":
      return "Gemini";
    case "ollama":
      return "Ollama";
    case "llamacpp":
      return "llama.cpp";
    default:
      return id.charAt(0).toUpperCase() + id.slice(1);
  }
}

/** Strip the provider prefix from a model id when the prefix matches
 *  the provider, so the chip reads "Kimi · k2-0905" instead of
 *  "Kimi · moonshotai/kimi-k2-0905". Cosmetic. */
function shortenModel(model: string, providerId: string): string {
  if (!model) return "";
  // OpenRouter slugs are "vendor/model" — strip the vendor for display.
  if (providerId === "openrouter" && model.includes("/")) {
    return model.split("/").slice(1).join("/");
  }
  // Other adapters: trim known prefixes that just repeat the provider.
  return model.replace(/^(kimi-|moonshot-|gemini-|gpt-|claude-)/, "$1");
}

interface ProviderHealth {
  id: string;
  ok: boolean;
  error: string | null;
}

export function LiveModelChip() {
  const [primaryId, setPrimaryId] = useState<string | null>(null);
  const [providerOk, setProviderOk] = useState(false);

  // Initial snapshot + subscribe to provider:status for live updates.
  useEffect(() => {
    let cancelled = false;
    let unsub: (() => void) | undefined;

    const refresh = async () => {
      try {
        const s = await ipc.diagnosticsStatus();
        if (cancelled) return;
        const pid = s.router_primary_id ?? null;
        setPrimaryId(pid);
        const matching = pid
          ? (s.provider_health ?? []).find((p: ProviderHealth) => p.id === pid)
          : null;
        setProviderOk(matching?.ok === true);
      } catch {
        /* swallow — chip just stays hidden */
      }
    };
    void refresh();

    (async () => {
      const u = await onWaterEvent("provider:status", (p) => {
        // Update only if this status pertains to the primary
        // provider. A Test on a different provider doesn't flip
        // the chip; only `provider_test`'s router-swap (which we
        // pick up via the next `diagnostics_status` refresh) does.
        setPrimaryId((prev) => {
          if (p.provider_id === prev) {
            setProviderOk(p.ok);
          }
          return prev;
        });
        // Also re-snapshot in case the primary just changed (a fresh
        // Test on a previously-untested provider becomes primary).
        void refresh();
      });
      if (cancelled) {
        u();
        return;
      }
      unsub = u;
    })();

    return () => {
      cancelled = true;
      unsub?.();
    };
  }, []);

  if (!primaryId) return null;

  const model = getActiveModel(primaryId);
  const modelShort = shortenModel(model, primaryId);

  return (
    <div
      data-testid="live-model-chip"
      role="status"
      aria-label={`Live model: ${prettyProvider(primaryId)} ${model}`}
      title={
        providerOk
          ? `Nudges powered by ${prettyProvider(primaryId)} (${model})`
          : `${prettyProvider(primaryId)} configured but not currently active — open Settings to re-test.`
      }
      style={{
        position: "absolute",
        // Anchor to the editor's bottom-LEFT, not bottom-right, so
        // the chip stays clear of the pill-margin column + the
        // pinned-pill strip on the right edge. As the writer
        // collapses/expands the scenes panel the editor pane shifts
        // horizontally, and the chip moves with it naturally.
        bottom: 14,
        left: 18,
        zIndex: 5,
        display: "inline-flex",
        alignItems: "center",
        gap: 6,
        padding: "4px 10px",
        borderRadius: "var(--water-r-16)",
        background:
          "color-mix(in srgb, var(--water-bg-paper) 65%, transparent)",
        backdropFilter: "blur(14px) saturate(150%)",
        WebkitBackdropFilter: "blur(14px) saturate(150%)",
        border:
          "1px solid color-mix(in srgb, var(--water-hairline) 50%, transparent)",
        boxShadow: "var(--water-elev-1)",
        fontFamily: "var(--water-font-sans)",
        fontSize: 10,
        color: "var(--water-fg-muted)",
        // Don't intercept clicks on the editor below; the writer
        // never needs to interact with the chip directly.
        pointerEvents: "none",
        opacity: 0.78,
        userSelect: "none",
      }}
    >
      <Zap
        size={10}
        aria-hidden
        style={{
          color: providerOk
            ? "color-mix(in srgb, var(--water-hue-flow) 80%, var(--water-fg-muted))"
            : "var(--water-fg-faint)",
        }}
      />
      <span style={{ fontWeight: 600, letterSpacing: 0.2 }}>
        {prettyProvider(primaryId)}
      </span>
      {modelShort && (
        <>
          <span style={{ opacity: 0.5 }}>·</span>
          <span
            style={{
              fontFamily: "var(--water-font-mono)",
              fontSize: 9,
              maxWidth: 160,
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
            }}
          >
            {modelShort}
          </span>
        </>
      )}
    </div>
  );
}
