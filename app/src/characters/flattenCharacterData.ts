import type { CharacterFile } from "../ipc/commands";
import { flattenSerdeFlatten } from "../util/flattenSerdeFlatten";

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
 *
 * M4 T19: the actual flatten logic now lives in
 * `util/flattenSerdeFlatten.ts` so the World Bible UI can reuse it. This
 * wrapper stays as the typed entrypoint for the character call sites.
 */
export function flattenCharacterToDottedPaths(
  file: CharacterFile,
): Record<string, unknown> {
  return flattenSerdeFlatten(file as Record<string, unknown>);
}
