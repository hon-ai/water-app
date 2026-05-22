import { useState } from "react";
import {
  ipc,
  type WorldTemplateField,
  type WorldTemplateFieldKind,
  type WorldTemplateSchema,
} from "../ipc/commands";
import { GlassSelect } from "../chrome/GlassSelect";

type EditableKind = "short_text" | "long_text" | "string_list";

interface EditableField {
  label: string;
  promptQuestion: string;
  kind: EditableKind;
  optional: boolean;
}

interface InitialState {
  name: string;
  isCollection: boolean;
  fields: WorldTemplateField[];
  isBuiltin: boolean;
  segmentId: string;
}

interface Props {
  mode: "create" | "edit";
  initial?: InitialState;
  onSave: (segmentId: string) => void;
  onClose: () => void;
}

function deriveFieldId(label: string, kind: EditableKind): string {
  const slug = label
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "");
  const section = kind === "string_list" ? "lists" : "main";
  return `${section}.${slug}`;
}

function kindToTs(kind: WorldTemplateFieldKind): EditableKind {
  if (kind.type === "string_list") return "string_list";
  if (kind.type === "long_text") return "long_text";
  // The `choice` shape isn't authorable in v1; fall back to short_text
  // so the field still round-trips when the editor opens a built-in
  // that doesn't ship a choice anyway.
  return "short_text";
}

/**
 * Minimal segment template authoring (M4 T31). v1 cuts: no drag-reorder,
 * no kind-edit on built-in fields, no choice authoring. Built-in
 * segments open in append-only mode — existing fields are locked but
 * new ones can be added underneath.
 */
export function SegmentTemplateEditor({
  mode,
  initial,
  onSave,
  onClose,
}: Props) {
  const [name, setName] = useState(initial?.name ?? "");
  const [isCollection, setIsCollection] = useState(
    initial?.isCollection ?? false,
  );
  const [fields, setFields] = useState<EditableField[]>(
    initial?.fields.map((f) => ({
      label: f.label,
      promptQuestion: f.prompt_question,
      kind: kindToTs(f.kind),
      optional: f.optional_skip,
    })) ?? [],
  );
  const [saving, setSaving] = useState(false);

  const isAppendOnly = mode === "edit" && (initial?.isBuiltin ?? false);
  const lockedFieldCount = isAppendOnly ? initial?.fields.length ?? 0 : 0;

  function updateField(i: number, patch: Partial<EditableField>) {
    setFields((prev) =>
      prev.map((f, idx) => (idx === i ? { ...f, ...patch } : f)),
    );
  }

  function addField() {
    setFields((prev) => [
      ...prev,
      {
        label: "",
        promptQuestion: "",
        kind: "short_text",
        optional: false,
      },
    ]);
  }

  function removeField(i: number) {
    if (i < lockedFieldCount) return;
    setFields((prev) => prev.filter((_, idx) => idx !== i));
  }

  async function handleSave() {
    if (saving) return;
    setSaving(true);
    try {
      const tplFields: WorldTemplateField[] = fields.map((f) => ({
        id: deriveFieldId(f.label, f.kind),
        label: f.label,
        prompt_question: f.promptQuestion,
        kind: { type: f.kind } as WorldTemplateFieldKind,
        optional_skip: f.optional,
      }));
      const slug = name
        .trim()
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, "_")
        .replace(/^_+|_+$/g, "");
      const template: WorldTemplateSchema = {
        id: initial?.segmentId ?? slug,
        label: name,
        fields: tplFields,
      };
      if (mode === "create") {
        const newId = await ipc.worldSegmentCreate({
          name,
          isCollection,
          template,
        });
        onSave(newId);
      } else if (initial) {
        await ipc.worldSegmentUpdateTemplate({
          segmentId: initial.segmentId,
          template,
        });
        onSave(initial.segmentId);
      }
    } finally {
      setSaving(false);
    }
  }

  return (
    <div
      className="segment-template-editor"
      data-testid="segment-template-editor"
    >
      <h3>{mode === "create" ? "New segment" : "Edit template"}</h3>
      <label
        style={{ display: "flex", flexDirection: "column", gap: 4, marginBottom: 12 }}
      >
        Name
        <input
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
          disabled={mode === "edit" && isAppendOnly}
          data-testid="segment-name-input"
        />
      </label>
      <fieldset style={{ marginBottom: 12 }}>
        <legend>Type</legend>
        <label>
          <input
            type="radio"
            checked={!isCollection}
            onChange={() => setIsCollection(false)}
            disabled={mode === "edit"}
          />
          Single document
        </label>
        <label style={{ marginLeft: 12 }}>
          <input
            type="radio"
            checked={isCollection}
            onChange={() => setIsCollection(true)}
            disabled={mode === "edit"}
          />
          Collection
        </label>
      </fieldset>
      <div className="fields-editor">
        <h4>Fields</h4>
        {fields.map((f, i) => {
          const locked = i < lockedFieldCount;
          return (
            <div
              key={i}
              className="field-row"
              data-testid={`field-row-${i}`}
              style={{
                display: "flex",
                gap: 6,
                alignItems: "center",
                marginBottom: 6,
              }}
            >
              <input
                type="text"
                value={f.label}
                onChange={(e) => updateField(i, { label: e.target.value })}
                placeholder="Label"
                disabled={locked}
                data-testid={`field-label-${i}`}
              />
              <div
                style={{ minWidth: 130 }}
                data-testid={`field-kind-${i}`}
              >
                <GlassSelect
                  ariaLabel="Field kind"
                  disabled={locked}
                  value={f.kind}
                  options={[
                    { value: "short_text", label: "short text" },
                    { value: "long_text", label: "long text" },
                    { value: "string_list", label: "list" },
                  ]}
                  onChange={(next) =>
                    updateField(i, { kind: next as EditableKind })
                  }
                />
              </div>
              <input
                type="text"
                value={f.promptQuestion}
                onChange={(e) =>
                  updateField(i, { promptQuestion: e.target.value })
                }
                placeholder="Prompt question"
                disabled={locked}
              />
              <label>
                <input
                  type="checkbox"
                  checked={f.optional}
                  onChange={(e) =>
                    updateField(i, { optional: e.target.checked })
                  }
                  disabled={locked}
                />
                optional
              </label>
              {!locked && (
                <button
                  type="button"
                  onClick={() => removeField(i)}
                  data-testid={`field-remove-${i}`}
                  aria-label={`Remove field ${i + 1}`}
                >
                  ×
                </button>
              )}
              {locked && (
                <span className="field-locked-label" aria-label="built-in">
                  (built-in)
                </span>
              )}
            </div>
          );
        })}
        <button
          type="button"
          onClick={addField}
          data-testid="add-field-button"
        >
          + Add field
        </button>
      </div>
      <div className="actions" style={{ display: "flex", gap: 8, marginTop: 12 }}>
        <button type="button" onClick={onClose}>
          Cancel
        </button>
        <button
          type="button"
          onClick={() => {
            void handleSave();
          }}
          data-testid="save-button"
          disabled={saving}
        >
          {mode === "create" ? "Create" : "Save"}
        </button>
      </div>
    </div>
  );
}
