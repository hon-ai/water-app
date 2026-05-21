import { useCallback, useRef, useState } from "react";
import { CharacterIndex } from "../characters/CharacterIndex";
import { CharacterSheet } from "../characters/CharacterSheet";
import { CharacterIntakeSheet } from "../intake/CharacterIntakeSheet";
import { WaterRibbon } from "./WaterRibbon";
import { useElementWidth } from "../pill/useElementWidth";

/**
 * Characters surface router (M3 T20, spec § 9).
 *
 * Replaces the T17 scaffold with a three-view router:
 *  - `index`  — the grid of CharacterCards (CharacterIndex, T19).
 *  - `sheet`  — inline-editable LSM v2.1 sheet (CharacterSheet, T18).
 *  - `intake` — overlaid Conversational Intake (CharacterIntakeSheet, T13);
 *               can open on top of either underlying view.
 *
 * **Scroll preservation:** the index stays mounted (under `display: none`)
 * while the sheet is shown so scroll position and search/sort state
 * survive a round-trip to the sheet and back. This is the analog of M2
 * T24's `reloadToken`-keyed canvas — cheap, no router dep.
 *
 * **Reload after intake:** `reloadKey` bumps when the intake sheet
 * reports `onCompleted`. CharacterIndex re-effects on the key change
 * and refetches `characterList`, so freshly-completed characters'
 * completion percent updates without remounting (scroll preserved).
 * CharacterSheet has its own load effect keyed on `characterId`; we
 * don't need to nudge it here.
 *
 * **hueToken plumbing:** `CharacterFile` (the raw TOML) does NOT carry
 * hue info — that lives on the `character` SQLite row and only surfaces
 * via `CharacterIndexEntry`. So the router captures `hue_token` when
 * the user clicks a card and threads it into the sheet view rather than
 * forcing CharacterSheet to do an extra `characterList` lookup.
 */
type View =
  | { kind: "index" }
  | { kind: "sheet"; characterId: string; hueToken: string };

export function CharactersSurface() {
  const [view, setView] = useState<View>({ kind: "index" });
  const [intakeCharId, setIntakeCharId] = useState<string | null>(null);
  const [reloadKey, setReloadKey] = useState(0);
  const wrapRef = useRef<HTMLDivElement | null>(null);
  const wrapWidth = useElementWidth(wrapRef);

  const openCharacter = useCallback((id: string, hueToken: string) => {
    setView({ kind: "sheet", characterId: id, hueToken });
  }, []);

  const openIntake = useCallback((id: string) => {
    setIntakeCharId(id);
  }, []);

  const backToIndex = useCallback(() => {
    setView({ kind: "index" });
  }, []);

  const closeIntake = useCallback(() => {
    setIntakeCharId(null);
  }, []);

  const onIntakeCompleted = useCallback(() => {
    // Nudge the still-mounted index to refetch. Sheet view (if active)
    // self-reloads on its own field-save path; intake exit doesn't need
    // to push to it.
    setReloadKey((k) => k + 1);
  }, []);

  return (
    <div
      ref={wrapRef}
      style={{ flex: 1, position: "relative", overflow: "auto" }}
    >
      <WaterRibbon parentWidth={wrapWidth} />
      <div
        style={{ display: view.kind === "index" ? "block" : "none" }}
      >
        <CharacterIndex
          onOpenCharacter={openCharacter}
          onOpenIntake={openIntake}
          reloadKey={reloadKey}
        />
      </div>
      {view.kind === "sheet" && (
        <CharacterSheet
          key={view.characterId}
          characterId={view.characterId}
          hueToken={view.hueToken}
          onBackToIndex={backToIndex}
          onContinueIntake={() => openIntake(view.characterId)}
        />
      )}
      {intakeCharId !== null && (
        <CharacterIntakeSheet
          characterId={intakeCharId}
          open={true}
          onClose={closeIntake}
          onCompleted={onIntakeCompleted}
        />
      )}
    </div>
  );
}
