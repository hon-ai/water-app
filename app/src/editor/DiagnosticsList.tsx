import { Check, X } from "lucide-react";
import type { EditorPillRow } from "../ipc/commands";

interface Props {
  pills: EditorPillRow[];
  onDismiss: (id: string) => void;
  /**
   * Phase 5.6 — fires when the writer clicks Accept on a row with a
   * suggestion. The parent splices the replacement into the manuscript
   * via the `water:accept-editor-pill` event bridge and dismisses the
   * row from local state.
   */
  onAccept: (pill: EditorPillRow) => void;
}

/**
 * Phase 5 — minimal diagnostics tab (UX_SPEC §E.4).
 *
 * A compact list of active editor pills grouped by rule. Each row
 * shows the rule label + Editor-voiced message + a dismiss
 * button. v1 lives under the editor; later iterations may move it
 * into a right-panel tab.
 *
 * Empty state renders nothing — the writer doesn't need a "no
 * diagnostics" surface cluttering the manuscript view.
 */
export function DiagnosticsList({ pills, onDismiss, onAccept }: Props) {
  if (pills.length === 0) return null;

  // Group by rule so the writer sees clusters together.
  const byRule = new Map<string, EditorPillRow[]>();
  for (const p of pills) {
    const arr = byRule.get(p.rule) ?? [];
    arr.push(p);
    byRule.set(p.rule, arr);
  }

  return (
    <section
      data-testid="diagnostics-list"
      aria-label="Editor diagnostics"
      style={{
        marginTop: 32,
        paddingTop: 16,
        borderTop:
          "1px solid color-mix(in srgb, var(--water-hairline) 50%, transparent)",
        display: "flex",
        flexDirection: "column",
        gap: 12,
        fontFamily: "var(--water-font-sans)",
        fontSize: 12,
        color: "var(--water-fg-muted)",
      }}
    >
      <div
        style={{
          fontSize: 10,
          fontWeight: 600,
          textTransform: "uppercase",
          letterSpacing: 0.6,
          color: "var(--water-fg-muted)",
        }}
      >
        Editor notices ({pills.length})
      </div>
      {[...byRule.entries()].map(([rule, rows]) => (
        <div
          key={rule}
          data-testid={`diagnostics-group-${rule}`}
          style={{ display: "flex", flexDirection: "column", gap: 6 }}
        >
          <div
            style={{
              fontSize: 10,
              letterSpacing: 0.3,
              color: "var(--water-fg-faint)",
            }}
          >
            {ruleLabel(rule)}
          </div>
          {rows.map((p) => (
            <DiagnosticRow
              key={p.id}
              pill={p}
              onDismiss={onDismiss}
              onAccept={onAccept}
            />
          ))}
        </div>
      ))}
    </section>
  );
}

function DiagnosticRow({
  pill,
  onDismiss,
  onAccept,
}: {
  pill: EditorPillRow;
  onDismiss: (id: string) => void;
  onAccept: (pill: EditorPillRow) => void;
}) {
  return (
    <div
      data-testid={`diagnostics-row-${pill.id}`}
      data-severity={pill.severity}
      style={{
        display: "flex",
        alignItems: "flex-start",
        gap: 8,
        padding: "6px 10px",
        borderRadius: "var(--water-r-8)",
        background:
          "color-mix(in srgb, var(--water-bg-paper) 60%, transparent)",
        border:
          "1px solid color-mix(in srgb, var(--water-hairline) 40%, transparent)",
        boxShadow: "var(--water-elev-1)",
      }}
    >
      <span
        aria-hidden
        style={{
          flexShrink: 0,
          marginTop: 4,
          width: 6,
          height: 6,
          borderRadius: "50%",
          background: severityHue(pill.severity),
        }}
      />
      <div style={{ flex: 1, color: "var(--water-fg-default)" }}>
        <div>{pill.message}</div>
        {pill.suggestion && (
          <div
            style={{
              marginTop: 2,
              fontSize: 11,
              color: "var(--water-fg-muted)",
            }}
          >
            suggestion: <span style={{ fontFamily: "var(--water-font-mono)" }}>{pill.suggestion}</span>
          </div>
        )}
      </div>
      {pill.suggestion && (
        <button
          type="button"
          aria-label="Accept suggestion"
          title={`accept: ${pill.suggestion}`}
          onClick={() => onAccept(pill)}
          style={{
            flexShrink: 0,
            width: 22,
            height: 22,
            display: "grid",
            placeItems: "center",
            border: "none",
            background:
              "color-mix(in srgb, var(--water-sea-300) 20%, transparent)",
            color: "var(--water-sea-500)",
            cursor: "pointer",
            borderRadius: "var(--water-r-8)",
            transition:
              "background var(--water-dur-tiny) var(--water-ease-out-soft)",
          }}
          onMouseEnter={(e) => {
            e.currentTarget.style.background =
              "color-mix(in srgb, var(--water-sea-300) 38%, transparent)";
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.background =
              "color-mix(in srgb, var(--water-sea-300) 20%, transparent)";
          }}
        >
          <Check size={12} strokeWidth={1.75} />
        </button>
      )}
      <button
        type="button"
        aria-label="Dismiss"
        onClick={() => onDismiss(pill.id)}
        style={{
          flexShrink: 0,
          width: 22,
          height: 22,
          display: "grid",
          placeItems: "center",
          border: "none",
          background: "transparent",
          color: "var(--water-fg-muted)",
          cursor: "pointer",
          borderRadius: "var(--water-r-8)",
          transition:
            "background var(--water-dur-tiny) var(--water-ease-out-soft)",
        }}
        onMouseEnter={(e) => {
          e.currentTarget.style.background =
            "color-mix(in srgb, var(--water-fg-faint) 14%, transparent)";
        }}
        onMouseLeave={(e) => {
          e.currentTarget.style.background = "transparent";
        }}
      >
        <X size={12} strokeWidth={1.5} />
      </button>
    </div>
  );
}

function ruleLabel(rule: string): string {
  switch (rule) {
    case "passive_voice":
      return "PASSIVE VOICE";
    case "adverb_density":
      return "ADVERB DENSITY";
    case "repetition":
      return "REPETITION";
    case "dialog_tag_overuse":
      return "DIALOG TAG";
    case "common_mistake":
      return "COMMON MISTAKE";
    case "weak_verb":
      return "WEAK VERB";
    case "sentence_length_variance":
      return "CADENCE";
    case "editor_polish":
      return "EDITOR";
    default:
      return rule.toUpperCase();
  }
}

function severityHue(severity: string): string {
  switch (severity) {
    case "warning":
      return "color-mix(in oklch, var(--water-sea-600), transparent 25%)";
    case "suggestion":
      return "color-mix(in oklch, var(--water-sea-300), transparent 25%)";
    default:
      return "color-mix(in oklch, var(--water-sea-200), transparent 25%)";
  }
}
