import { useCallback, useEffect, useState } from "react";
import { InlineField } from "./InlineField";
import { flattenCharacterToDottedPaths } from "./flattenCharacterData";
import { ipc } from "../ipc/commands";
import type { CharacterFile, IntakeSchemaSection } from "../ipc/commands";

/**
 * Inline-editable character sheet view (M3 T18, spec § 8).
 *
 * Renders the full LSM v2.1 sheet as a vertical-scroll page. Each field
 * is an `InlineField` cell that commits on blur via
 * `ipc.characterUpdateField`. After every successful save the sheet
 * reloads to pick up backend-driven side-effects (most importantly the
 * `main.full_name` → `main.aliases` rename cascade).
 *
 * `hueToken` is passed in by the parent because `CharacterFile` is the
 * raw on-disk TOML and does NOT carry hue info — hue lives on the
 * `character` SQLite row and is surfaced via `CharacterIndexEntry`.
 *
 * Cancellation race: matches M2 T4 / T16 pattern — the load promise
 * gates state writes on a `cancelled` flag so a `characterId` flip
 * mid-load cannot stomp the fresh load with stale data.
 */
interface Props {
  characterId: string;
  hueToken: string;
  onBackToIndex: () => void;
  onContinueIntake: () => void;
}

export function CharacterSheet({
  characterId,
  hueToken,
  onBackToIndex,
  onContinueIntake,
}: Props) {
  const [schema, setSchema] = useState<IntakeSchemaSection[] | null>(null);
  const [file, setFile] = useState<CharacterFile | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Reload helper used after every successful field save (rename-cascade
  // aliases etc. only show up after re-reading the file). Doesn't need
  // cancellation gating because it's not raced against a characterId flip.
  const reload = useCallback(async () => {
    try {
      const [s, f] = await Promise.all([
        ipc.intakeSchema("lsm-v2.1"),
        ipc.characterRead(characterId),
      ]);
      setSchema(s);
      setFile(f);
    } catch (e) {
      setError(String(e));
    }
  }, [characterId]);

  // Mount + characterId-change load with cancellation guard.
  useEffect(() => {
    let cancelled = false;
    setError(null);
    setSchema(null);
    setFile(null);
    void (async () => {
      try {
        const [s, f] = await Promise.all([
          ipc.intakeSchema("lsm-v2.1"),
          ipc.characterRead(characterId),
        ]);
        if (cancelled) return;
        setSchema(s);
        setFile(f);
      } catch (e) {
        if (cancelled) return;
        setError(String(e));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [characterId]);

  if (error)
    return (
      <div role="alert">Failed to load: {error}</div>
    );
  if (!schema || !file)
    return <div role="status" className="water-loading">Loading</div>;

  const values = flattenCharacterToDottedPaths(file);
  const completion = computeCompletion(schema, values);
  const fullName = values["main.full_name"];
  const heading = typeof fullName === "string" && fullName.trim() !== ""
    ? fullName
    : "(unnamed)";

  return (
    <div className="water-character-sheet" data-hue-token={hueToken}>
      <header>
        <button
          type="button"
          className="water-button water-button-ghost"
          onClick={onBackToIndex}
        >
          ← All characters
        </button>
        <h1>{heading}</h1>
        <div>{completion}% complete</div>
        {completion < 100 && (
          <button
            type="button"
            className="water-button water-button-primary"
            onClick={onContinueIntake}
          >
            Continue intake
          </button>
        )}
      </header>
      {schema.map((section) => (
        <section
          key={section.section}
          aria-labelledby={`section-${section.section}`}
        >
          <h2 id={`section-${section.section}`}>{section.section}</h2>
          {section.fields.map((field) => (
            <InlineField
              key={field.id}
              field={field}
              value={values[field.id]}
              onSave={async (v) => {
                await ipc.characterUpdateField(characterId, field.id, v);
                // Reload so backend-driven side effects (rename cascade,
                // computed completion, etc.) become visible.
                await reload();
              }}
            />
          ))}
        </section>
      ))}
    </div>
  );
}

/**
 * Compute completion percent (0..=100) using the same rule as
 * `water_core::character::intake::completion_pct`:
 *  - "Required" = `!optional_skip`.
 *  - "Filled" = non-empty trimmed string OR non-empty array.
 *  - Empty schema / zero-required ⇒ 100.
 *  - Integer math: `floor(filled * 100 / total)`.
 *
 * Backend-side this is the authoritative function (and the value the
 * `character_list` IPC returns). The Sheet view loads via `characterRead`
 * which returns the raw `CharacterFile` (no precomputed completion), so
 * we recompute locally with the same semantics.
 */
function computeCompletion(
  schema: IntakeSchemaSection[],
  values: Record<string, unknown>,
): number {
  const required = schema.flatMap((s) =>
    s.fields.filter((f) => !f.optional_skip),
  );
  if (required.length === 0) return 100;
  const filled = required.filter((f) => isFilled(values[f.id])).length;
  return Math.floor((filled * 100) / required.length);
}

function isFilled(v: unknown): boolean {
  if (typeof v === "string") return v.trim() !== "";
  if (Array.isArray(v)) return v.length > 0;
  return false;
}
