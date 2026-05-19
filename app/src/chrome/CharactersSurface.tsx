import { useCallback, useEffect, useState } from "react";
import { ipc, type CharacterIndexEntry } from "../ipc/commands";
import { CharacterIntakeSheet } from "../intake/CharacterIntakeSheet";

/**
 * Temporary scaffold for the Characters surface (M3 T17).
 *
 * Provides the minimum entry points needed to exercise Conversational
 * Intake end-to-end before the polished CharacterIndex lands in Phase F2:
 *  1. Lists characters via `ipc.characterList`.
 *  2. "+ New character" → `ipc.characterCreate` → opens
 *     `CharacterIntakeSheet` for the new id.
 *  3. Per-row "Continue intake" CTA when `completion < 100`.
 *
 * Styling is intentionally bare; T20 (Phase G) replaces this with the
 * styled CharacterIndex. The `data-hue-token` attribute is plumbed
 * through now so downstream styling work has a stable hook.
 */
export function CharactersSurface() {
  const [chars, setChars] = useState<CharacterIndexEntry[]>([]);
  const [intakeCharId, setIntakeCharId] = useState<string | null>(null);

  const reload = useCallback(async () => {
    try {
      const list = await ipc.characterList();
      setChars(list);
    } catch {
      /* swallow — the scaffold has no error UI; F2 will surface this */
    }
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  const handleNew = useCallback(async () => {
    const created = await ipc.characterCreate();
    await reload();
    setIntakeCharId(created.id);
  }, [reload]);

  return (
    <div>
      <header>
        <h2>Characters</h2>
        <button type="button" onClick={() => void handleNew()}>
          + New character
        </button>
      </header>
      <ul>
        {chars.length === 0 && <li role="status">No characters yet.</li>}
        {chars.map((c) => (
          <li key={c.id}>
            <span data-hue-token={c.hue_token}>
              {c.full_name || "(unnamed)"}
            </span>
            <span>{c.completion}% complete</span>
            {c.completion < 100 && (
              <button type="button" onClick={() => setIntakeCharId(c.id)}>
                Continue intake
              </button>
            )}
          </li>
        ))}
      </ul>
      {intakeCharId && (
        <CharacterIntakeSheet
          characterId={intakeCharId}
          open={true}
          onClose={() => setIntakeCharId(null)}
          onCompleted={() => {
            void reload();
          }}
        />
      )}
    </div>
  );
}
