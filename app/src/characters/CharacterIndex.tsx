import { useCallback, useEffect, useMemo, useState } from "react";
import { CharacterCard } from "./CharacterCard";
import { ipc } from "../ipc/commands";
import type { CharacterIndexEntry } from "../ipc/commands";

/**
 * Top-level grid view of every character in the project (M3 T19, spec § 9).
 *
 * Features:
 *  - Substring search (case-insensitive) across `full_name` and `role`.
 *  - Sort by name (locale-aware, case-insensitive), completion (desc),
 *    or "created" (id-asc — ULIDs are monotonic so this is a stable
 *    proxy until a future migration adds `created_at`).
 *  - "+ New character" calls `ipc.characterCreate`, reloads the list,
 *    then routes to intake for the freshly-created id.
 *
 * `loaded` gates the empty-state messages so we don't flash
 * "No characters yet." during the first `characterList` round-trip.
 * Two empty messages distinguish "project has no characters" from
 * "current filter has no hits".
 *
 * No styling — T20 replaces `CharactersSurface` and adds CSS against
 * the `water-character-*` class hooks here.
 */
type SortKey = "name" | "completion" | "created";

interface Props {
  onOpenCharacter: (id: string, hueToken: string) => void;
  onOpenIntake: (id: string) => void;
  /**
   * Bumped by the parent (`CharactersSurface`) after a sibling view (e.g.
   * the intake sheet) mutates character data so the still-mounted index
   * refetches. Kept optional so component-level tests don't need to wire
   * it. See T20.
   */
  reloadKey?: number;
}

export function CharacterIndex({
  onOpenCharacter,
  onOpenIntake,
  reloadKey,
}: Props) {
  const [chars, setChars] = useState<CharacterIndexEntry[]>([]);
  const [search, setSearch] = useState("");
  const [sortKey, setSortKey] = useState<SortKey>("name");
  const [loaded, setLoaded] = useState(false);

  const reload = useCallback(async () => {
    const list = await ipc.characterList();
    setChars(list);
    setLoaded(true);
  }, []);

  useEffect(() => {
    void reload();
  }, [reload, reloadKey]);

  const handleNew = useCallback(async () => {
    const created = await ipc.characterCreate();
    await reload();
    onOpenIntake(created.id);
  }, [reload, onOpenIntake]);

  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase();
    let list = chars;
    if (q !== "") {
      list = chars.filter((c) => {
        const nameHit = c.full_name.toLowerCase().includes(q);
        const roleHit = (c.role ?? "").toLowerCase().includes(q);
        return nameHit || roleHit;
      });
    }
    return [...list].sort((a, b) => {
      switch (sortKey) {
        case "name":
          return a.full_name.localeCompare(b.full_name, undefined, {
            sensitivity: "base",
          });
        case "completion":
          return b.completion - a.completion;
        case "created":
          // Index list is ORDER BY full_name today; "created" sort defers to
          // a future migration adding `created_at`. Fall back to id (ULID is
          // monotonic).
          return a.id.localeCompare(b.id);
        default:
          return 0;
      }
    });
  }, [chars, search, sortKey]);

  const trimmedSearch = search.trim();

  return (
    <div className="water-character-index">
      <header>
        <h1>Characters</h1>
        <input
          type="search"
          placeholder="Search…"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          aria-label="Search characters"
        />
        <label>
          Sort:
          <select
            value={sortKey}
            onChange={(e) => setSortKey(e.target.value as SortKey)}
          >
            <option value="name">Name</option>
            <option value="completion">Completion</option>
            <option value="created">Created</option>
          </select>
        </label>
        <button type="button" onClick={() => void handleNew()}>
          + New character
        </button>
      </header>
      {loaded && filtered.length === 0 && (
        <div role="status">
          {trimmedSearch !== ""
            ? "No characters match your search."
            : "No characters yet."}
        </div>
      )}
      <ul className="water-character-grid">
        {filtered.map((c) => (
          <li key={c.id}>
            <CharacterCard
              character={c}
              onClick={() => onOpenCharacter(c.id, c.hue_token)}
            />
          </li>
        ))}
      </ul>
    </div>
  );
}
