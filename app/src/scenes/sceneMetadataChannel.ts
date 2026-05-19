import type { AutosuggestResult } from "../ipc/commands";

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
 */
type Listener = (sceneId: string, results: AutosuggestResult[]) => void;

const listeners = new Set<Listener>();

export function publishAutosuggest(
  sceneId: string,
  results: AutosuggestResult[],
): void {
  for (const l of listeners) l(sceneId, results);
}

export function subscribeAutosuggest(listener: Listener): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
