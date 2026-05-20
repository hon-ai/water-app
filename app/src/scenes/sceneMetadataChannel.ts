/**
 * Tiny in-process pub/sub so the editor's autosave loop can hand
 * autosuggest results to the SceneMetadataSheet without either side
 * importing the other. Listeners receive every publish; they filter by
 * `sceneId` themselves (the sheet only cares about its own scene).
 *
 * This is a synchronous Set — no re-entrancy guards, no async dispatch.
 * Listeners must not subscribe/unsubscribe from inside their callback;
 * `Set` mutation during iteration is undefined behavior here. In
 * practice every consumer wires this through `useEffect`'s cleanup,
 * which runs outside the publish call site.
 *
 * M4 T28: the payload is a discriminated union so the same channel can
 * carry both character chips (M3) and world-entry chips (M4 — wired by
 * the editor's autosave loop in parallel with the character scan).
 */

/**
 * One chip suggestion to render in the SceneMetadataSheet. Distinguished
 * by `kind`: `"character"` chips link to the scene; `"world_entry"`
 * chips set the scene's `location_id`. World chips are limited to the
 * `locations` segment for M4 (the producer in EditorCanvas filters by
 * `segment_slug === "locations"` before publishing).
 */
export type ChipSuggestion =
  | {
      kind: "character";
      characterId: string;
      characterName: string;
      mentionCount: number;
    }
  | {
      kind: "world_entry";
      entryId: string;
      entryName: string;
      segmentSlug: string;
    };

type Listener = (sceneId: string, suggestions: ChipSuggestion[]) => void;

const listeners = new Set<Listener>();

export function publishChips(
  sceneId: string,
  suggestions: ChipSuggestion[],
): void {
  for (const l of listeners) l(sceneId, suggestions);
}

export function subscribeChips(listener: Listener): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
