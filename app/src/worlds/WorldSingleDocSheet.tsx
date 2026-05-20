import { useEffect, useState } from "react";
import {
  ipc,
  type IntakeField,
  type WorldEntryFile,
  type WorldSegment,
  type WorldTemplateField,
  type WorldTemplateSchema,
} from "../ipc/commands";
import { flattenSerdeFlatten } from "../util/flattenSerdeFlatten";
import { InlineField } from "../characters/InlineField";

/**
 * Single-doc segment sheet (M4 T21).
 *
 * Renders one `InlineField` per `WorldTemplateField` against the segment's
 * lazily-materialized `WorldEntryFile`. The on-disk shape uses
 * `#[serde(flatten)]` on the Rust side so `[main]` / `[lists]` sections
 * land at the top level of the JSON payload — `flattenSerdeFlatten`
 * collapses those back into the dotted `main.<key>` / `lists.<key>` ids
 * the templates use, matching how `flattenCharacterData` worked in M3.
 *
 * **InlineField adapter:** `InlineField` is strictly typed against
 * `IntakeField` (M3 — has `section`/`helper`/`examples`/`prompt_question`),
 * while `WorldTemplateField` (M4) drops `section`/`helper`/`examples`. The
 * shared shape is `id` / `label` / `prompt_question` / `kind` /
 * `optional_skip`, all serialized identically. We adapt by synthesizing
 * the missing fields with safe defaults — the inline editor only reads
 * `label`, `kind`, and `optional_skip` so the synthetics never surface.
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

export function WorldSingleDocSheet({ segment }: { segment: WorldSegment }) {
  const [schema, setSchema] = useState<WorldTemplateSchema | null>(null);
  const [values, setValues] = useState<Record<string, unknown>>({});
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    let cancelled = false;
    Promise.all([
      ipc.worldIntakeSchema(segment.id),
      ipc.worldSingleDocRead(segment.id),
    ]).then(([sch, file]: [WorldTemplateSchema, WorldEntryFile]) => {
      if (cancelled) return;
      setSchema(sch);
      setValues(flattenSerdeFlatten(file as Record<string, unknown>));
      setLoaded(true);
    });
    return () => {
      cancelled = true;
    };
  }, [segment.id]);

  if (!loaded || !schema) return <div className="water-loading">Loading</div>;

  return (
    <div className="world-single-doc-sheet">
      <h2>{segment.name}</h2>
      <div className="world-single-doc-fields">
        {schema.fields.map((field) => (
          <InlineField
            key={field.id}
            field={toIntakeField(field)}
            value={values[field.id]}
            onSave={async (newValue) => {
              await ipc.worldSingleDocUpdateField({
                segmentId: segment.id,
                fieldId: field.id,
                value: newValue,
              });
              setValues((prev) => ({ ...prev, [field.id]: newValue }));
            }}
          />
        ))}
      </div>
    </div>
  );
}
