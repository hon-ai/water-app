import { useEffect, useState } from "react";
import {
  ipc,
  type IntakeField,
  type IntakeSchemaSection,
  type WorldEntryFile,
  type WorldTemplateField,
  type WorldTemplateSchema,
} from "../ipc/commands";
import { Sheet } from "../sheets/Sheet";
import { ConversationalIntake } from "../intake/ConversationalIntake";
import { flattenSerdeFlatten } from "../util/flattenSerdeFlatten";

/**
 * Overlay intake for a fresh collection-entry draft (M4 T24).
 *
 * Reuses M3's `ConversationalIntake` (which is schema-agnostic apart from
 * its `IntakeSchemaSection[]` prop shape) by widening the M4
 * `WorldTemplateSchema` into a single-section `IntakeSchemaSection[]`.
 * The `IntakeField` shim mirrors `WorldSingleDocSheet`/`WorldEntrySheet`:
 * synthesize the missing `section` / `helper` / `examples` fields with
 * safe defaults; the renderer only reads `label`, `kind`, `prompt_question`,
 * and `optional_skip`, so the synthetics never surface.
 *
 * **Orphan reaping (spec § 10):** the "+ New entry" affordance pre-creates
 * an empty draft entry so the intake can write field-by-field via
 * `world_entry_update_field`. If the user closes the sheet without ever
 * saving a value, that orphan draft is collected by
 * `world_entry_delete_if_empty` (a no-op if the entry has any content).
 * The reaper runs in `handleClose`, *before* `onClose` propagates, so the
 * caller can rely on the orphan being gone by the time it re-fetches the
 * collection index. Calling reap on `onComplete` would be incorrect — a
 * completed intake by definition has content and the reaper would be a
 * no-op, but it would still be an unnecessary round-trip.
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

/**
 * Widen a M4 `WorldTemplateSchema` into the `IntakeSchemaSection[]` shape
 * that `ConversationalIntake` expects. The M4 schema is flat (all fields
 * in one `fields` array); we group them into a single synthetic section
 * named after the schema label — the section name is shown as a small
 * header above each question, so using the schema's human-readable label
 * is the right call.
 */
function widenSchema(schema: WorldTemplateSchema): IntakeSchemaSection[] {
  return [
    {
      section: schema.label,
      fields: schema.fields.map(toIntakeField),
    },
  ];
}

export function WorldEntryIntakeSheet({
  segmentId,
  draftEntryId,
  onComplete,
  onClose,
}: {
  segmentId: string;
  draftEntryId: string;
  onComplete: (entryId: string) => void;
  onClose: () => void;
}) {
  const [schema, setSchema] = useState<WorldTemplateSchema | null>(null);
  const [values, setValues] = useState<Record<string, unknown>>({});
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    let cancelled = false;
    Promise.all([
      ipc.worldIntakeSchema(segmentId),
      ipc.worldEntryRead(draftEntryId),
    ]).then(([sch, file]: [WorldTemplateSchema, WorldEntryFile]) => {
      if (cancelled) return;
      setSchema(sch);
      setValues(flattenSerdeFlatten(file as Record<string, unknown>));
      setLoaded(true);
    });
    return () => {
      cancelled = true;
    };
  }, [segmentId, draftEntryId]);

  async function handleClose() {
    // Reap the orphan FIRST so the parent never sees a stale draft on
    // re-fetch. `worldEntryDeleteIfEmpty` is a no-op if the entry has
    // any content, so this is safe to call unconditionally.
    await ipc.worldEntryDeleteIfEmpty(draftEntryId);
    onClose();
  }

  if (!loaded || !schema) {
    return (
      <Sheet open={true} onClose={handleClose} title="New entry">
        <div role="status" className="water-loading">Loading</div>
      </Sheet>
    );
  }

  return (
    <Sheet open={true} onClose={handleClose} title="New entry">
      <ConversationalIntake
        schema={widenSchema(schema)}
        initialValues={values}
        onAnswer={async (fieldId: string, value: unknown) => {
          await ipc.worldEntryUpdateField({
            entryId: draftEntryId,
            fieldId,
            value,
          });
          setValues((prev) => ({ ...prev, [fieldId]: value }));
        }}
        onComplete={() => onComplete(draftEntryId)}
        onClose={handleClose}
      />
    </Sheet>
  );
}
