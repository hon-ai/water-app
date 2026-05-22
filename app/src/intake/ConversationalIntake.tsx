import { useEffect, useState } from "react";
import type { IntakeField, IntakeSchemaSection } from "../ipc/commands";
import { GlassSelect } from "../chrome/GlassSelect";

/**
 * Props for the schema-agnostic conversational intake.
 *
 * `onAnswer` is awaited per field; rejections leave the renderer on the
 * current field (the parent decides whether to surface the error).
 */
interface Props {
  schema: IntakeSchemaSection[];
  initialValues: Record<string, unknown>;
  onAnswer: (fieldId: string, value: unknown) => Promise<void>;
  onComplete: () => void;
  onClose: () => void;
}

/**
 * True when a draft is "empty" for its field kind. Used both to pick the
 * resume position and to gate the Next button on required fields.
 */
function isEmpty(v: unknown, kind: IntakeField["kind"]): boolean {
  if (v === null || v === undefined) return true;
  switch (kind.type) {
    case "short_text":
    case "long_text":
    case "choice":
      return typeof v !== "string" || v.trim() === "";
    case "string_list":
      return !Array.isArray(v) || v.length === 0;
  }
}

/**
 * Return the index of the field where the intake should resume.
 *
 * Spec § 7: prefer the first unanswered REQUIRED field; if none, fall back
 * to the first unanswered optional field; if every field has a value,
 * return `fields.length`.
 */
function findFirstUnanswered(
  fields: IntakeField[],
  values: Record<string, unknown>,
): number {
  // First pass: required fields only.
  for (let i = 0; i < fields.length; i++) {
    const f = fields[i];
    if (!f) continue;
    if (!f.optional_skip && isEmpty(values[f.id], f.kind)) return i;
  }
  // Second pass: any unanswered field (i.e. remaining optionals).
  for (let i = 0; i < fields.length; i++) {
    const f = fields[i];
    if (!f) continue;
    if (isEmpty(values[f.id], f.kind)) return i;
  }
  return fields.length;
}

/** Coerce an arbitrary stored value into a string for text/choice inputs. */
function asString(v: unknown): string {
  return typeof v === "string" ? v : "";
}

/** Coerce an arbitrary stored value into a string[] for list inputs. */
function asStringArray(v: unknown): string[] {
  return Array.isArray(v) ? (v as unknown[]).filter((x): x is string => typeof x === "string") : [];
}

/** Initial draft for the field at `index`. */
function initialDraft(
  fields: IntakeField[],
  index: number,
  values: Record<string, unknown>,
): unknown {
  const f = fields[index];
  if (!f) return "";
  const v = values[f.id];
  if (v === undefined || v === null) {
    return f.kind.type === "string_list" ? [] : "";
  }
  return v;
}

export function ConversationalIntake({
  schema,
  initialValues,
  onAnswer,
  onComplete,
  onClose,
}: Props) {
  // Flatten schema into a single ordered list. `section` is already on the
  // field, so no wrapper type is needed.
  const fields: IntakeField[] = schema.flatMap((s) => s.fields);

  const initialIndex = findFirstUnanswered(fields, initialValues);

  const [index, setIndex] = useState(initialIndex);
  const [draft, setDraft] = useState<unknown>(() =>
    initialDraft(fields, initialIndex, initialValues),
  );
  // In-session record of confirmed values. Seeded from `initialValues` at
  // mount, then updated on every successful `advance()`. Back-navigation
  // and forward-step draft seeding read from this so Back restores what
  // the user JUST typed (not the stale prop).
  const [confirmedValues, setConfirmedValues] =
    useState<Record<string, unknown>>(initialValues);
  const [busy, setBusy] = useState(false);

  // Mount-only: if the resume index is already past the end (every field
  // already answered) or the schema is empty, fire onComplete immediately.
  // Without this, an "already complete" reopen would hang on the stub.
  // Empty deps are intentional: the resume index is a mount-time concept.
  useEffect(() => {
    if (fields.length === 0 || initialIndex >= fields.length) {
      onComplete();
    }
  }, []);

  // Empty schema OR finished: just render a stub. The parent observes
  // completion via onComplete, fired above on mount and below from advance.
  if (fields.length === 0) {
    return <div role="status">Intake complete.</div>;
  }
  if (index >= fields.length) {
    return <div role="status">Intake complete.</div>;
  }

  const field = fields[index];
  if (!field) {
    // Unreachable given the bounds check above, but satisfies
    // `noUncheckedIndexedAccess`.
    return <div role="status">Intake complete.</div>;
  }

  const required = !field.optional_skip;

  const advance = async (skip: boolean) => {
    if (busy) return;
    // Compute the post-advance confirmed values up-front so we don't rely
    // on the stale `confirmedValues` closure when seeding the next draft.
    let newConfirmed = confirmedValues;
    if (!skip) {
      if (required && isEmpty(draft, field.kind)) {
        return; // stays on current field
      }
      setBusy(true);
      try {
        await onAnswer(field.id, draft);
      } finally {
        setBusy(false);
      }
      newConfirmed = { ...confirmedValues, [field.id]: draft };
      setConfirmedValues(newConfirmed);
    }
    const next = index + 1;
    if (next >= fields.length) {
      onComplete();
      return;
    }
    setIndex(next);
    setDraft(initialDraft(fields, next, newConfirmed));
  };

  const back = () => {
    if (index === 0) return;
    const prev = index - 1;
    setIndex(prev);
    setDraft(initialDraft(fields, prev, confirmedValues));
  };

  return (
    <div data-testid="conversational-intake">
      <div data-testid="section">{field.section}</div>
      <h3>{field.prompt_question}</h3>
      {field.helper !== null && field.helper !== "" && (
        <p data-testid="helper">{field.helper}</p>
      )}
      {field.examples.length > 0 && (
        <p data-testid="examples">Examples: {field.examples.join(", ")}</p>
      )}
      {/*
        `key={field.id}` forces React to unmount/remount the inner input
        when the field changes. This is required for two reasons:
        (1) `autoFocus` is a one-shot mount behavior — without a fresh
            mount, transitions between same-kind fields (short_text ->
            short_text) would not refocus.
        (2) `StringArrayEditor` holds local `text` state; remounting
            resets it so it doesn't leak between fields.
      */}
      <FieldInput
        key={field.id}
        field={field}
        value={draft}
        onChange={setDraft}
        onSubmit={() => {
          void advance(false);
        }}
      />
      <div role="group" aria-label="Intake navigation">
        <button
          type="button"
          onClick={back}
          disabled={index === 0 || busy}
        >
          Back
        </button>
        <button
          type="button"
          onClick={() => {
            void advance(true);
          }}
          disabled={required || busy}
        >
          {required ? "Required" : "Skip"}
        </button>
        <button
          type="button"
          onClick={() => {
            void advance(false);
          }}
          disabled={busy}
        >
          {busy ? "Saving\u2026" : "Next"}
        </button>
        <button type="button" onClick={onClose}>
          Save &amp; close
        </button>
      </div>
      <div data-testid="progress">
        {index + 1} / {fields.length}
      </div>
    </div>
  );
}

/** Per-variant input renderer. */
function FieldInput({
  field,
  value,
  onChange,
  onSubmit,
}: {
  field: IntakeField;
  value: unknown;
  onChange: (v: unknown) => void;
  onSubmit: () => void;
}) {
  const kind = field.kind;
  switch (kind.type) {
    case "short_text":
      return (
        <input
          type="text"
          aria-label={field.label}
          value={asString(value)}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              onSubmit();
            }
          }}
          autoFocus
        />
      );
    case "long_text":
      return (
        <textarea
          aria-label={field.label}
          value={asString(value)}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
              e.preventDefault();
              onSubmit();
            }
          }}
          autoFocus
          rows={5}
        />
      );
    case "string_list":
      return (
        <StringArrayEditor
          label={field.label}
          value={asStringArray(value)}
          onChange={(v) => onChange(v)}
          onSubmit={onSubmit}
        />
      );
    case "choice":
      return (
        <GlassSelect
          ariaLabel={field.label}
          value={asString(value)}
          placeholder="Choose…"
          options={kind.options.map((opt) => ({ value: opt, label: opt }))}
          onChange={(next) => onChange(next)}
        />
      );
  }
}

/**
 * Comma-separated text input that emits a trimmed, non-empty string[].
 * Chips below echo the parsed value in real time.
 */
function StringArrayEditor({
  label,
  value,
  onChange,
  onSubmit,
}: {
  label: string;
  value: string[];
  onChange: (v: string[]) => void;
  onSubmit: () => void;
}) {
  const [text, setText] = useState(value.join(", "));
  return (
    <>
      <input
        type="text"
        aria-label={label}
        placeholder="comma, separated, values"
        value={text}
        onChange={(e) => {
          setText(e.target.value);
          onChange(
            e.target.value
              .split(",")
              .map((s) => s.trim())
              .filter((s) => s !== ""),
          );
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            onSubmit();
          }
        }}
        autoFocus
      />
      <ul data-testid="string-array-chips">
        {value.map((v) => (
          <li key={v}>{v}</li>
        ))}
      </ul>
    </>
  );
}
