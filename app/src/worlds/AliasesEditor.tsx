import { useState } from "react";

/**
 * Tiny alias-list editor (M4 T23).
 *
 * Renders the current `aliases` as removable chips plus a text input.
 * `onChange` is fired with the *next* full array on add/remove so the
 * parent owns the canonical list — this component is purely presentational
 * + draft state. Duplicate aliases (exact-match) are silently rejected
 * client-side; the Rust side already dedupes case-insensitively for the
 * autosuggest index, so this guard is just UX polish.
 *
 * Empty/whitespace drafts are dropped on add. Enter in the input commits;
 * Tab/blur do not (a stray text in the input is a draft, not an alias).
 */
export function AliasesEditor({
  aliases,
  onChange,
}: {
  aliases: string[];
  onChange: (next: string[]) => void;
}) {
  const [draft, setDraft] = useState("");

  function addAlias() {
    const trimmed = draft.trim();
    if (!trimmed) return;
    if (aliases.includes(trimmed)) {
      setDraft("");
      return;
    }
    onChange([...aliases, trimmed]);
    setDraft("");
  }

  function removeAlias(i: number) {
    onChange(aliases.filter((_, idx) => idx !== i));
  }

  return (
    <div className="aliases-editor">
      <label>Aliases</label>
      <ul>
        {aliases.map((a, i) => (
          <li key={`${a}-${i}`}>
            {a}{" "}
            <button
              type="button"
              onClick={() => removeAlias(i)}
              data-testid={`remove-alias-${i}`}
              aria-label={`Remove alias ${a}`}
            >
              &times;
            </button>
          </li>
        ))}
      </ul>
      <input
        type="text"
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            addAlias();
          }
        }}
        placeholder="Add an alias&hellip;"
        data-testid="alias-input"
        aria-label="Add an alias"
      />
      <button
        type="button"
        onClick={addAlias}
        data-testid="alias-add-button"
      >
        Add
      </button>
    </div>
  );
}
