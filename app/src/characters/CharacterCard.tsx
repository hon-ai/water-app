import type { CharacterIndexEntry } from "../ipc/commands";

/**
 * One card in the CharacterIndex grid (M3 T19, spec § 9).
 *
 * Wrapped in a `<button>` so the entire card is keyboard-focusable and
 * activates `onClick`. The hue ribbon is exposed via `data-hue-token`
 * so T20's CSS can style it from the design-token map without touching
 * this component's markup.
 *
 * `full_name` may be the empty string for a freshly-created character
 * that hasn't been intake'd yet — render `(unnamed)` in that case so
 * the card is still navigable. `role` is `null` when the user hasn't
 * answered the intake question; we just omit the line.
 *
 * No styling lives here; T20 owns visuals. The `water-character-card*`
 * class names are deliberate hooks for that task.
 */
interface Props {
  character: CharacterIndexEntry;
  onClick: () => void;
}

export function CharacterCard({ character, onClick }: Props) {
  return (
    <button
      type="button"
      className="water-character-card"
      data-hue-token={character.hue_token}
      data-testid="character-card"
      onClick={onClick}
    >
      <div className="water-character-card__hue" aria-hidden />
      <div className="water-character-card__name">
        {character.full_name !== "" ? character.full_name : <em>(unnamed)</em>}
      </div>
      {character.role !== null && (
        <div className="water-character-card__role">{character.role}</div>
      )}
      <div className="water-character-card__completion">
        {character.completion}% complete
      </div>
    </button>
  );
}
