import { useEffect, useRef, useState } from "react";
import type { IntakeField, IntakeFieldKind } from "../ipc/commands";

/**
 * Reusable inline-edit cell primitive (M3 T18).
 *
 * Click the cell to enter edit mode; blur (or Enter for single-line /
 * Ctrl+Enter for multi-line) commits via `onSave`; Esc reverts.
 *
 * Status chip mirrors the autosave-chip pattern (idle → saving → saved
 * → idle after 1.2s) plus an `error` terminal state on `onSave` rejection.
 *
 * The input is conditionally mounted (only when `editing`), so we don't
 * need a `key` prop to force remounts — React already remounts on every
 * edit cycle.
 */
interface Props {
  field: IntakeField;
  value: unknown;
  onSave: (value: unknown) => Promise<void>;
}

export function InlineField({ field, value, onSave }: Props) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState<unknown>(value);
  const [status, setStatus] = useState<"idle" | "saving" | "saved" | "error">(
    "idle",
  );
  const inputRef = useRef<
    HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement | null
  >(null);

  // Keep the draft in sync with incoming prop changes UNLESS the user is
  // mid-edit (don't clobber their typing).
  useEffect(() => {
    if (!editing) setDraft(value);
  }, [value, editing]);

  // Focus the freshly-mounted input on enter-edit.
  useEffect(() => {
    if (editing && inputRef.current) inputRef.current.focus();
  }, [editing]);

  // Auto-fade the "saved" chip after 1.2s.
  useEffect(() => {
    if (status === "saved") {
      const t = window.setTimeout(() => setStatus("idle"), 1200);
      return () => window.clearTimeout(t);
    }
    return undefined;
  }, [status]);

  const commit = async () => {
    if (deepEqual(draft, value)) {
      setEditing(false);
      return;
    }
    setStatus("saving");
    try {
      await onSave(draft);
      setStatus("saved");
      setEditing(false);
    } catch {
      setStatus("error");
    }
  };

  const cancel = () => {
    setDraft(value);
    setEditing(false);
    setStatus("idle");
  };

  if (!editing) {
    return (
      <div
        className="water-inline-field"
        onClick={() => setEditing(true)}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            setEditing(true);
          }
        }}
        role="button"
        tabIndex={0}
        aria-label={`Edit ${field.label}`}
      >
        <label>{field.label}</label>
        <div data-empty={isEmpty(value)}>
          {isEmpty(value) ? <em>— empty —</em> : formatValue(value, field.kind)}
        </div>
      </div>
    );
  }

  return (
    <div className="water-inline-field" data-editing="true">
      <label>{field.label}</label>
      {renderEditor(field, draft, setDraft, inputRef, commit, cancel)}
      {status === "saving" && <span data-testid="status-chip">Saving…</span>}
      {status === "saved" && <span data-testid="status-chip">Saved</span>}
      {status === "error" && (
        <span data-testid="status-chip" role="alert">
          Save failed
        </span>
      )}
    </div>
  );
}

function renderEditor(
  field: IntakeField,
  draft: unknown,
  setDraft: (v: unknown) => void,
  inputRef: React.MutableRefObject<
    HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement | null
  >,
  commit: () => Promise<void>,
  cancel: () => void,
) {
  const kind = field.kind;
  switch (kind.type) {
    case "short_text":
      return (
        <input
          ref={(el) => {
            inputRef.current = el;
          }}
          type="text"
          aria-label={field.label}
          value={asString(draft)}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={() => {
            void commit();
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              void commit();
            }
            if (e.key === "Escape") {
              e.preventDefault();
              cancel();
            }
          }}
        />
      );
    case "long_text":
      return (
        <textarea
          ref={(el) => {
            inputRef.current = el;
          }}
          aria-label={field.label}
          value={asString(draft)}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={() => {
            void commit();
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
              e.preventDefault();
              void commit();
            }
            if (e.key === "Escape") {
              e.preventDefault();
              cancel();
            }
          }}
          rows={4}
        />
      );
    case "string_list":
      return (
        <input
          ref={(el) => {
            inputRef.current = el;
          }}
          type="text"
          aria-label={field.label}
          placeholder="comma, separated, values"
          value={Array.isArray(draft) ? asStringArray(draft).join(", ") : ""}
          onChange={(e) =>
            setDraft(
              e.target.value
                .split(",")
                .map((s) => s.trim())
                .filter((s) => s !== ""),
            )
          }
          onBlur={() => {
            void commit();
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              void commit();
            }
            if (e.key === "Escape") {
              e.preventDefault();
              cancel();
            }
          }}
        />
      );
    case "choice":
      return (
        <select
          ref={(el) => {
            inputRef.current = el;
          }}
          aria-label={field.label}
          value={asString(draft)}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={() => {
            void commit();
          }}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              e.preventDefault();
              cancel();
            }
          }}
        >
          <option value="" disabled>
            Choose…
          </option>
          {kind.options.map((opt) => (
            <option key={opt} value={opt}>
              {opt}
            </option>
          ))}
        </select>
      );
  }
}

function isEmpty(v: unknown): boolean {
  if (v === null || v === undefined) return true;
  if (typeof v === "string") return v.trim() === "";
  if (Array.isArray(v)) return v.length === 0;
  return false;
}

function asString(v: unknown): string {
  return typeof v === "string" ? v : "";
}

function asStringArray(v: unknown): string[] {
  return Array.isArray(v)
    ? (v as unknown[]).filter((x): x is string => typeof x === "string")
    : [];
}

function formatValue(v: unknown, kind: IntakeFieldKind): string {
  if (isEmpty(v)) return "";
  if (kind.type === "string_list" && Array.isArray(v)) {
    return asStringArray(v).join(", ");
  }
  return typeof v === "string" ? v : String(v);
}

function deepEqual(a: unknown, b: unknown): boolean {
  if (Array.isArray(a) && Array.isArray(b)) {
    if (a.length !== b.length) return false;
    for (let i = 0; i < a.length; i++) {
      if (a[i] !== b[i]) return false;
    }
    return true;
  }
  return a === b;
}
