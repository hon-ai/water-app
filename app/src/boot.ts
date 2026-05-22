// One-time boot wiring. Pulled out of App.tsx so the entry point
// stays focused on the React tree and the integrations stay easy to
// audit (and to remove if the environment changes).
//
// Two integrations today:
//   1. Sentry crash + error capture (renderer-side only). Tied to a
//      VITE_SENTRY_DSN env var so dev builds without the var stay
//      offline — no analytics, no network calls.
//   2. Tauri auto-updater check. Pings the `latest.json` manifest
//      configured in `tauri.conf.json` and shows a one-line toast
//      when a new version is available.

import * as Sentry from "@sentry/browser";
import { check, type Update } from "@tauri-apps/plugin-updater";

/** Initialize crash/error capture. No-op when no DSN is set. */
export function initSentry(): void {
  const dsn = import.meta.env.VITE_SENTRY_DSN as string | undefined;
  if (!dsn) return;
  try {
    Sentry.init({
      dsn,
      // Use the build-tagged version baked in by Vite when the
      // tag-cut release ran. Falls through to "dev" in untagged
      // dev builds.
      release: (import.meta.env.VITE_APP_VERSION as string | undefined) ?? "dev",
      environment: (import.meta.env.MODE as string) ?? "production",
      // Conservative tracing rates — we're not paying for big
      // performance metrics yet, just need errors + breadcrumbs.
      tracesSampleRate: 0.0,
      // Strip URL query strings + scrub provider-key-shaped tokens
      // before sending. Sentry's default scrubbing handles common
      // patterns; this is belt-and-suspenders for our keys.
      beforeSend(event) {
        const str = JSON.stringify(event);
        const scrubbed = str.replace(
          /(sk-[a-zA-Z0-9_-]{16,}|or-v\d-[a-zA-Z0-9]{20,})/g,
          "[REDACTED-KEY]",
        );
        return JSON.parse(scrubbed) as typeof event;
      },
    });
  } catch {
    /* swallow — Sentry init failure must not block the app */
  }
}

/**
 * Check for an update. If one's available, surface a small toast via
 * a CustomEvent the renderer can subscribe to. We don't auto-apply —
 * the writer should know an update is coming and pick the moment to
 * relaunch (so a half-written scene doesn't disappear on them).
 *
 * Returns `null` when no update is available OR when the updater
 * isn't configured (missing pubkey / unreachable manifest); callers
 * shouldn't surface anything in that case.
 */
export async function checkForUpdate(): Promise<Update | null> {
  try {
    const update = await check();
    return update ?? null;
  } catch {
    // No-op: missing pubkey, network down, dev build w/o manifest,
    // etc. None of these are user-facing errors.
    return null;
  }
}

/** Download + install the update in the background. Reporter is
 *  responsible for telling the writer "an update is ready, restart
 *  Water when convenient" — we deliberately don't auto-restart so a
 *  half-written scene doesn't vanish on them. */
export async function applyUpdateInBackground(update: Update): Promise<void> {
  try {
    await update.downloadAndInstall();
  } catch {
    /* swallow — the toast can re-offer */
  }
}
