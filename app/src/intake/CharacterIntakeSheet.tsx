import { useEffect, useRef, useState } from "react";
import { Sheet } from "../sheets/Sheet";
import { ConversationalIntake } from "./ConversationalIntake";
import { ipc } from "../ipc/commands";
import type { IntakeSchemaSection } from "../ipc/commands";
import { flattenCharacterToDottedPaths } from "../characters/flattenCharacterData";

interface Props {
  characterId: string;
  open: boolean;
  onClose: () => void;
  onCompleted: () => void;
}

/**
 * Sheet-wrapped LSM v2.1 intake (M3 T16).
 *
 * Loads the schema + the character's current values whenever the sheet
 * opens (or `characterId` changes while open) and forwards each answer
 * through `characterUpdateField`. The Rust side serializes concurrent
 * writes per-character so the on-disk TOML cannot tear; this component
 * only needs to guard against the resolution-order race.
 *
 * Cancellation race (M2 T4 pattern): both the `schema + read` await
 * AND the catch path are gated on `cancelled`. Without those guards a
 * stale load could overwrite a fresh one when `characterId` changes
 * mid-flight (covered by the race test).
 */
export function CharacterIntakeSheet({
  characterId,
  open,
  onClose,
  onCompleted,
}: Props) {
  const [schema, setSchema] = useState<IntakeSchemaSection[] | null>(null);
  const [values, setValues] = useState<Record<string, unknown> | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Always-current `characterId` ref so an in-flight `characterUpdateField`
  // can detect a parent-driven character switch and bail before its
  // optimistic `setValues` runs (otherwise the old closure's captured
  // `characterId` would compare equal to itself, defeating any non-ref
  // guard). React allows ref mutation during render for the standard
  // "track current prop" pattern.
  const characterIdRef = useRef(characterId);
  characterIdRef.current = characterId;

  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    setError(null);
    setSchema(null);
    setValues(null);
    void (async () => {
      try {
        const [s, file] = await Promise.all([
          ipc.intakeSchema("lsm-v2.1"),
          ipc.characterRead(characterId),
        ]);
        if (cancelled) return;
        setSchema(s);
        setValues(flattenCharacterToDottedPaths(file));
      } catch (e) {
        if (cancelled) return;
        setError(String(e));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [open, characterId]);

  return (
    <Sheet open={open} onClose={onClose} title="Character intake">
      {error ? (
        <div role="alert">Failed to load: {error}</div>
      ) : schema && values ? (
        <ConversationalIntake
          schema={schema}
          initialValues={values}
          onAnswer={async (fieldId, value) => {
            // Capture the id at call time so a mid-write `characterId`
            // change (e.g. parent switches characters) can't poison the
            // optimistic state of the new character with this one's
            // answer. Compare against the ref (always current), not the
            // captured prop (which equals `writingFor` inside this
            // closure and would defeat the guard). Same family of
            // protection as the load-path `if (cancelled) return;`.
            // The on-disk TOML is already correct (per-character write
            // lock on the Rust side); this only protects the in-session
            // `values` cache.
            const writingFor = characterId;
            await ipc.characterUpdateField(writingFor, fieldId, value);
            if (writingFor !== characterIdRef.current) return;
            // Optimistic local-update so a subsequent resume picks up the
            // new value without waiting for a round-trip re-read.
            setValues((prev) =>
              prev ? { ...prev, [fieldId]: value } : prev,
            );
          }}
          onComplete={() => {
            onCompleted();
            onClose();
          }}
          onClose={onClose}
        />
      ) : (
        <div role="status">Loading&hellip;</div>
      )}
    </Sheet>
  );
}


