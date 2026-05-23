import { useEffect, useRef, useState } from "react";
import type { UvInstallState } from "./useUvInstall";

interface Props {
  open: boolean;
  state: UvInstallState;
  onDismissForSession: () => void;
}

/**
 * Modal prompt shown on every app reload while `uv` is not on disk.
 * Differs from the prior `UvInstallBanner` in two ways:
 *
 *  1. **No persistent dismissal.** Closing the prompt only lasts for
 *     the current session — reload the app and it reappears. The
 *     writer can't accidentally hide an outstanding install forever.
 *  2. **One-click install** via the in-app installer (`useUvInstall`).
 *     Streams stdout/stderr into a scrolling log pane so the writer
 *     can see something happening; on success offers a "Restart
 *     Water" button so the next boot's sidecar spawn finds uv.
 *
 * Kept presentation-only — all install state lives in the hook so
 * the same machinery powers the Settings Sheet's sidecar row.
 */
export function UvInstallPrompt({ open, state, onDismissForSession }: Props) {
  const { status, logs, error, install, restart } = state;
  const logsRef = useRef<HTMLPreElement | null>(null);
  // Auto-scroll the log pane as lines arrive so the writer sees the
  // tail rather than the head of a long install.
  useEffect(() => {
    const el = logsRef.current;
    if (!el) return;
    el.scrollTop = el.scrollHeight;
  }, [logs.length]);

  const isMac =
    typeof navigator !== "undefined" &&
    /Mac|iPod|iPhone|iPad/.test(navigator.platform);
  const shownCommand = isMac
    ? "curl -LsSf https://astral.sh/uv/install.sh | sh"
    : 'powershell -ExecutionPolicy ByPass -c "irm https://astral.sh/uv/install.ps1 | iex"';
  const [copiedCommand, setCopiedCommand] = useState(false);
  const copyCommand = async () => {
    try {
      await navigator.clipboard.writeText(shownCommand);
      setCopiedCommand(true);
      window.setTimeout(() => setCopiedCommand(false), 1800);
    } catch {
      /* clipboard blocked — text is still visible to hand-copy */
    }
  };

  if (!open) return null;

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="uv-install-prompt-title"
      data-testid="uv-install-prompt"
      style={{
        position: "fixed",
        inset: 0,
        zIndex: 60,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        background:
          "color-mix(in srgb, var(--water-bg-canvas) 56%, transparent)",
        backdropFilter: "blur(8px) saturate(120%)",
        WebkitBackdropFilter: "blur(8px) saturate(120%)",
        animation:
          "water-fade-in var(--water-dur-medium) var(--water-ease-out-soft) both",
        fontFamily: "var(--water-font-sans)",
        color: "var(--water-fg-default)",
      }}
    >
      <div
        style={{
          width: "min(560px, calc(100vw - 56px))",
          maxHeight: "calc(100vh - 80px)",
          display: "flex",
          flexDirection: "column",
          gap: 14,
          padding: "20px 22px 18px",
          borderRadius: "var(--water-r-20)",
          background:
            "color-mix(in srgb, var(--water-bg-paper) 88%, transparent)",
          backdropFilter: "blur(24px) saturate(160%)",
          WebkitBackdropFilter: "blur(24px) saturate(160%)",
          border:
            "1px solid color-mix(in srgb, var(--water-hairline) 56%, transparent)",
          boxShadow: "var(--water-elev-2)",
        }}
      >
        <div>
          <h2
            id="uv-install-prompt-title"
            style={{
              margin: 0,
              fontFamily: "var(--water-font-serif)",
              fontSize: "var(--water-fs-h3)",
              fontWeight: 500,
              letterSpacing: "-0.005em",
            }}
          >
            Enable analysis features
          </h2>
          <p
            style={{
              margin: "6px 0 0",
              fontSize: "var(--water-fs-body)",
              color: "var(--water-fg-muted)",
              lineHeight: 1.5,
            }}
          >
            Water uses a Python sidecar for stylometric nudges &mdash; pacing,
            character voice drift, the prose-craft pills. It runs locally and
            needs <code style={{ fontFamily: "var(--water-font-mono)" }}>uv</code>{" "}
            on your machine. Installing takes about a minute and bundles its own
            Python toolchain.
          </p>
        </div>

        {status !== "done" && (
          <div
            style={{
              display: "flex",
              gap: 8,
              alignItems: "center",
              padding: "8px 10px",
              borderRadius: "var(--water-r-12)",
              background:
                "color-mix(in srgb, var(--water-bg-canvas) 60%, transparent)",
              border:
                "1px solid color-mix(in srgb, var(--water-fg-faint) 20%, transparent)",
            }}
          >
            <code
              style={{
                flex: 1,
                minWidth: 0,
                fontFamily: "var(--water-font-mono)",
                fontSize: 11,
                color: "var(--water-fg-default)",
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
              }}
              title={shownCommand}
            >
              {shownCommand}
            </code>
            <button
              type="button"
              className="water-button water-button-ghost water-button-compact"
              onClick={() => void copyCommand()}
            >
              {copiedCommand ? "Copied" : "Copy"}
            </button>
          </div>
        )}

        {(status === "installing" ||
          status === "done" ||
          status === "failed") &&
          logs.length > 0 && (
            <pre
              ref={logsRef}
              data-testid="uv-install-prompt-log"
              style={{
                margin: 0,
                padding: "10px 12px",
                maxHeight: 200,
                minHeight: 80,
                overflow: "auto",
                fontFamily: "var(--water-font-mono)",
                fontSize: 11,
                lineHeight: 1.5,
                color: "var(--water-fg-muted)",
                background:
                  "color-mix(in srgb, var(--water-bg-canvas) 70%, transparent)",
                border:
                  "1px solid color-mix(in srgb, var(--water-fg-faint) 16%, transparent)",
                borderRadius: "var(--water-r-12)",
                whiteSpace: "pre-wrap",
                wordBreak: "break-word",
              }}
            >
              {logs.map((entry, i) => (
                <div
                  key={i}
                  style={{
                    color:
                      entry.stream === "stderr"
                        ? "var(--water-fg-default)"
                        : "var(--water-fg-muted)",
                  }}
                >
                  {entry.line}
                </div>
              ))}
            </pre>
          )}

        {status === "failed" && error && (
          <div
            role="alert"
            style={{
              fontSize: "var(--water-fs-meta)",
              color: "var(--water-fg-default)",
              padding: "8px 10px",
              borderRadius: "var(--water-r-12)",
              background:
                "color-mix(in srgb, var(--water-c-red, #b14a4a) 12%, transparent)",
              border:
                "1px solid color-mix(in srgb, var(--water-c-red, #b14a4a) 28%, transparent)",
            }}
          >
            Install failed: {error}. You can retry, or run the command above in
            your shell directly.
          </div>
        )}

        {status === "done" && (
          <div
            role="status"
            style={{
              fontSize: "var(--water-fs-meta)",
              color: "var(--water-fg-default)",
              padding: "8px 10px",
              borderRadius: "var(--water-r-12)",
              background:
                "color-mix(in srgb, var(--water-c-green, #4a8a5e) 14%, transparent)",
              border:
                "1px solid color-mix(in srgb, var(--water-c-green, #4a8a5e) 30%, transparent)",
            }}
          >
            uv installed. Restart Water to enable analysis &mdash; your open
            project will reopen automatically.
          </div>
        )}

        <div
          style={{
            display: "flex",
            justifyContent: "flex-end",
            gap: 8,
            marginTop: 2,
          }}
        >
          <button
            type="button"
            className="water-button water-button-ghost"
            onClick={onDismissForSession}
            disabled={status === "installing"}
          >
            Not now
          </button>
          {status === "done" ? (
            <button
              type="button"
              className="water-button water-button-primary"
              onClick={() => void restart()}
              autoFocus
            >
              Restart Water
            </button>
          ) : (
            <button
              type="button"
              className="water-button water-button-primary"
              onClick={() => void install()}
              disabled={status === "installing"}
              autoFocus
            >
              {status === "installing"
                ? "Installing…"
                : status === "failed"
                  ? "Retry install"
                  : "Install uv"}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
