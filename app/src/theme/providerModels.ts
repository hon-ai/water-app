/**
 * Per-provider curated model lists + active-model persistence.
 *
 * Persistence lives in `localStorage` (one key per provider) so the
 * choice survives restarts without a server-side schema bump. The
 * runtime override is pushed into `LlmRouter::set_default_model` via
 * `ipc.providerSetModel` on boot + on every change.
 *
 * Curated lists are *suggestions* — the picker's free-text input
 * lets the writer type any model name the underlying provider
 * accepts. OpenRouter especially benefits from this: writers can
 * route through any of the hundreds of models the catalog
 * aggregates, and the curated list just surfaces the load-bearing
 * defaults.
 */

import { ipc } from "../ipc/commands";

const STORAGE_PREFIX = "water:provider-model:";

export interface ModelOption {
  /** Model id sent on the wire. */
  id: string;
  /** Human label rendered in the dropdown. */
  label: string;
  /** Optional one-line description. */
  hint?: string;
}

/**
 * Curated entries per provider. Order matters — the dropdown shows
 * them top-to-bottom; the first entry is the default selection
 * when the writer hasn't picked one. Free-text input below lets
 * the writer override any of these.
 */
export const MODEL_OPTIONS: Record<string, ModelOption[]> = {
  anthropic: [
    {
      id: "claude-sonnet-4-6",
      label: "Claude Sonnet 4.6",
      hint: "Production tier. Best observational quality for pills.",
    },
    {
      id: "claude-opus-4-7",
      label: "Claude Opus 4.7",
      hint: "Heavier — use sparingly for craft observations on long context.",
    },
    {
      id: "claude-haiku-4-5-20251001",
      label: "Claude Haiku 4.5",
      hint: "Cheap + fast. Strong 'no' bias on Stage 2 confirmations.",
    },
  ],
  openai: [
    {
      id: "gpt-4o-mini",
      label: "GPT-4o mini",
      hint: "Default. Cheap, sufficient for most pills.",
    },
    { id: "gpt-4o", label: "GPT-4o" },
    { id: "gpt-4.1-mini", label: "GPT-4.1 mini" },
    { id: "gpt-4.1", label: "GPT-4.1" },
  ],
  kimi: [
    {
      id: "kimi-k2-0905-preview",
      label: "Kimi K2 (256k)",
      hint: "Long context — embed entire drafts.",
    },
    { id: "moonshot-v1-128k", label: "Moonshot v1 (128k)" },
    { id: "moonshot-v1-32k", label: "Moonshot v1 (32k)" },
    { id: "moonshot-v1-8k", label: "Moonshot v1 (8k)" },
  ],
  gemini: [
    {
      id: "gemini-2.5-flash",
      label: "Gemini 2.5 Flash",
      hint: "Default. Fast + cheap; high free-tier quota.",
    },
    {
      id: "gemini-2.5-pro",
      label: "Gemini 2.5 Pro",
      hint: "Heavier. Best craft observations; lower free-tier quota.",
    },
    {
      id: "gemini-2.0-flash",
      label: "Gemini 2.0 Flash",
      hint: "Previous generation; still solid.",
    },
  ],
  openrouter: [
    {
      id: "moonshotai/kimi-k2",
      label: "Kimi K2",
      hint: "Long-context default.",
    },
    {
      id: "qwen/qwen3-32b",
      label: "Qwen 3 32B",
      hint: "Open-source flagship. Strong on prose.",
    },
    {
      id: "qwen/qwen3-235b-a22b",
      label: "Qwen 3 235B (MoE)",
    },
    {
      id: "anthropic/claude-sonnet-4.6",
      label: "Claude Sonnet 4.6 (via OpenRouter)",
    },
    {
      id: "openai/gpt-4o-mini",
      label: "GPT-4o mini (via OpenRouter)",
    },
    {
      id: "deepseek/deepseek-v3.2",
      label: "DeepSeek V3.2",
    },
    {
      id: "google/gemini-2.5-pro",
      label: "Gemini 2.5 Pro",
    },
  ],
  ollama: [
    { id: "qwen2.5:3b", label: "Qwen 2.5 3B (default)" },
    { id: "llama3.2:3b", label: "Llama 3.2 3B" },
    { id: "mistral:7b", label: "Mistral 7B" },
  ],
  llamacpp: [{ id: "default", label: "Default (server config)" }],
  canned: [{ id: "canned", label: "Canned response" }],
};

const STORAGE_DEFAULT: Record<string, string> = {};

/**
 * Default model id for a provider — the first curated entry's id.
 * Falls back to the empty string when the provider has no curated
 * list (which keeps the existing "use whatever the adapter's
 * hardcoded default is" behavior).
 */
export function defaultModelFor(providerId: string): string {
  if (STORAGE_DEFAULT[providerId]) return STORAGE_DEFAULT[providerId];
  const list = MODEL_OPTIONS[providerId];
  return list && list.length > 0 ? list[0]!.id : "";
}

/** Read the writer's saved model for a provider, or the default. */
export function getActiveModel(providerId: string): string {
  try {
    if (typeof localStorage !== "undefined") {
      const stored = localStorage.getItem(STORAGE_PREFIX + providerId);
      if (stored !== null) return stored;
    }
  } catch {
    /* swallow — localStorage can throw in private mode */
  }
  return defaultModelFor(providerId);
}

/** Persist + push to the orchestrator. Empty `model` clears the override. */
export async function setActiveModel(
  providerId: string,
  model: string,
): Promise<void> {
  try {
    if (typeof localStorage !== "undefined") {
      if (model === "" || model === defaultModelFor(providerId)) {
        localStorage.removeItem(STORAGE_PREFIX + providerId);
      } else {
        localStorage.setItem(STORAGE_PREFIX + providerId, model);
      }
    }
  } catch {
    /* swallow */
  }
  try {
    await ipc.providerSetModel(providerId, model);
  } catch {
    /* swallow — the runtime override is best-effort; persistence
       still landed in localStorage and re-applies on next boot. */
  }
}

/**
 * Re-apply every saved model override on app boot. Called once
 * after provider state is initialized so the router's
 * default-model map matches what the writer saw last session.
 */
export async function reapplyAllSavedModels(
  providerIds: string[],
): Promise<void> {
  for (const id of providerIds) {
    const model = getActiveModel(id);
    if (model && model !== defaultModelFor(id)) {
      try {
        await ipc.providerSetModel(id, model);
      } catch {
        /* swallow — best effort */
      }
    }
  }
}
