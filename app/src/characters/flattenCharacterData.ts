import type { CharacterFile } from "../ipc/commands";

/**
 * Flatten a `CharacterFile` into the `{ "<section>.<leaf>": value }`
 * shape that intake / sheet renderers expect (their lookup key is
 * `IntakeField.id`, which is a dotted path per
 * `water_core::character::intake`).
 *
 * The Rust side serializes `CharacterFile` with `#[serde(flatten)]` on
 * `data`, so section keys (`main`, `bonus_traits`, `arc`, `perspectives`)
 * appear at the top level alongside `id`/`name`/`schema_version`. Those
 * three scalar metadata keys are skipped — they are not intake fields.
 *
 * Any section that isn't a plain object (corrupted file, `null`, array)
 * is skipped without crashing.
 *
 * Shared by:
 *  - `intake/CharacterIntakeSheet.tsx` (T16)
 *  - `characters/CharacterSheet.tsx`  (T18)
 */
export function flattenCharacterToDottedPaths(
  file: CharacterFile,
): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  const skipKeys = new Set(["id", "name", "schema_version"]);
  for (const [section, fields] of Object.entries(file)) {
    if (skipKeys.has(section)) continue;
    if (
      typeof fields === "object" &&
      fields !== null &&
      !Array.isArray(fields)
    ) {
      for (const [k, v] of Object.entries(fields as Record<string, unknown>)) {
        out[`${section}.${k}`] = v;
      }
    }
  }
  return out;
}
