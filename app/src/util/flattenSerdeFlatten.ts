/**
 * Flatten an object whose section keys (e.g. "main", "lists", "arc") land
 * at the top level via Rust-side `#[serde(flatten)]` into a dotted-path
 * key/value map: `{ "main.full_name": "Aren", "lists.themes": ["..."] }`.
 *
 * Top-level non-object scalars (`id`, `name`, `schema_version`) are NOT
 * flattened — those are object metadata, not template-driven content.
 * Arrays at the top level (e.g. `aliases`) are also skipped because they
 * aren't section objects.
 *
 * Generic over both M3 `CharacterFile` and M4 `WorldEntryFile` (and any
 * future `#[serde(flatten)]` payload). The `metadataKeys` parameter lists
 * the non-flattenable top-level keys for the caller's payload type;
 * defaults cover the union of M3 + M4 metadata fields.
 *
 * Originally extracted from `characters/flattenCharacterData.ts` (M3 T16
 * / T18) so the M4 World Bible UI (T20+) can reuse the same shape.
 */
export function flattenSerdeFlatten(
  source: Record<string, unknown>,
  metadataKeys: ReadonlySet<string> = new Set([
    "id",
    "name",
    "schema_version",
    "segment_id",
    "aliases",
  ]),
): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const [sectionKey, sectionVal] of Object.entries(source)) {
    if (metadataKeys.has(sectionKey)) continue;
    if (
      sectionVal &&
      typeof sectionVal === "object" &&
      !Array.isArray(sectionVal)
    ) {
      for (const [leafKey, leafVal] of Object.entries(
        sectionVal as Record<string, unknown>,
      )) {
        out[`${sectionKey}.${leafKey}`] = leafVal;
      }
    }
  }
  return out;
}
