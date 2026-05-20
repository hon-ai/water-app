import { useEffect, useState } from "react";
import {
  ipc,
  type IntakeField,
  type WorldEntryFile,
  type WorldSegment,
  type WorldTemplateField,
  type WorldTemplateSchema,
} from "../ipc/commands";
import { InlineField } from "../characters/InlineField";
import { flattenSerdeFlatten } from "../util/flattenSerdeFlatten";
import { AliasesEditor } from "./AliasesEditor";

/**
 * Collection-entry editor sheet (M4 T23).
 *
 * Mirrors `WorldSingleDocSheet`'s shape (T21) but adds:
 *  - The entry's display name + an "(unnamed)" fallback for drafts whose
 *    `main.name` is still empty.
 *  - An `AliasesEditor` wired to `world_entry_update_aliases`.
 *
 * Like the single-doc sheet, we adapt `WorldTemplateField` -> `IntakeField`
 * so `InlineField` (M3 T18, strictly typed against `IntakeField`) can be
 * reused. See `WorldSingleDocSheet.toIntakeField` for the rationale; this
 * is the same shim duplicated locally rather than extracted, since
 * the function is six lines and a shared util would muddy the import
 * graph for no real benefit.
 *
 * Name-edit feedback loop: when the user edits `main.name`, the sheet
 * header re-renders by also updating `file.name` so the `<h2>` reflects
 * the new value without waiting for a re-read.
 */
function toIntakeField(field: WorldTemplateField): IntakeField {
  return {
    id: field.id,
    section: field.id.includes(".") ? field.id.split(".")[0]! : "main",
    label: field.label,
    prompt_question: field.prompt_question,
    helper: null,
    examples: [],
    kind: field.kind,
    optional_skip: field.optional_skip,
  };
}

export function WorldEntrySheet({
  segmentId,
  entryId,
}: {
  segmentId: string;
  entryId: string;
}) {
  const [segment, setSegment] = useState<WorldSegment | null>(null);
  const [schema, setSchema] = useState<WorldTemplateSchema | null>(null);
  const [file, setFile] = useState<WorldEntryFile | null>(null);
  const [values, setValues] = useState<Record<string, unknown>>({});

  useEffect(() => {
    let cancelled = false;
    Promise.all([
      ipc.worldSegmentList(),
      ipc.worldIntakeSchema(segmentId),
      ipc.worldEntryRead(entryId),
    ]).then(([segs, sch, f]) => {
      if (cancelled) return;
      setSegment(segs.find((s) => s.id === segmentId) ?? null);
      setSchema(sch);
      setFile(f);
      setValues(flattenSerdeFlatten(f as Record<string, unknown>));
    });
    return () => {
      cancelled = true;
    };
  }, [segmentId, entryId]);

  if (!segment || !schema || !file) return <div className="water-loading">Loading</div>;

  const displayName = file.name.trim() === "" ? "(unnamed)" : file.name;

  return (
    <div className="world-entry-sheet">
      <h2 data-testid="entry-name">{displayName}</h2>
      <AliasesEditor
        aliases={file.aliases}
        onChange={async (next) => {
          await ipc.worldEntryUpdateAliases({ entryId, aliases: next });
          setFile({ ...file, aliases: next });
        }}
      />
      <div className="world-entry-fields">
        {schema.fields.map((field) => (
          <InlineField
            key={field.id}
            field={toIntakeField(field)}
            value={values[field.id]}
            onSave={async (newValue) => {
              await ipc.worldEntryUpdateField({
                entryId,
                fieldId: field.id,
                value: newValue,
              });
              setValues((prev) => ({ ...prev, [field.id]: newValue }));
              // Header reflects name edits without a re-read.
              if (field.id === "main.name" && typeof newValue === "string") {
                setFile({ ...file, name: newValue });
              }
            }}
          />
        ))}
      </div>
    </div>
  );
}
