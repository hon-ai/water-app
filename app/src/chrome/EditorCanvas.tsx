import { useCallback, useEffect, useRef, useState } from "react";
import { ipc, type SceneInfo } from "../ipc/commands";
import { Editor } from "../editor/Editor";
import { WaterRibbon } from "./WaterRibbon";
import { PillLayer } from "../pill/PillLayer";
import { HeatmapStrip } from "../heat/HeatmapStrip";
import { PinnedColumn } from "../pill/PinnedColumn";
import { useElementWidth } from "../pill/useElementWidth";
import {
  publishChips,
  type ChipSuggestion,
} from "../scenes/sceneMetadataChannel";

interface Props {
  sceneId: string;
  onRenamed: (info: SceneInfo) => void;
}

export function EditorCanvas({ sceneId, onRenamed }: Props) {
  const [title, setTitle] = useState("");
  const [titleAtLastSave, setTitleAtLastSave] = useState("");
  const [body, setBody] = useState("");
  const [bodyDirty, setBodyDirty] = useState(false);
  const [savedAt, setSavedAt] = useState<number | null>(null);
  const bodyDebounce = useRef<number | undefined>(undefined);
  // Live width of the editor pane; drives the narrow-viewport fallback for
  // both <PillLayer> (translucent capsules) and <PinnedColumn> (24 px tab).
  const mainRef = useRef<HTMLElement>(null);
  const mainWidth = useElementWidth(mainRef);

  // Stable ref so the unmount-flush effect reads the latest editor state
  // without re-running (and re-flushing) on every keystroke.
  const editorStateRef = useRef<{
    title: string;
    titleAtLastSave: string;
    body: string;
    bodyDirty: boolean;
  }>({ title, titleAtLastSave, body, bodyDirty });
  useEffect(() => {
    editorStateRef.current = { title, titleAtLastSave, body, bodyDirty };
  });

  // Flush pending body + title writes when the scene changes or the
  // component unmounts. Fire-and-forget; we can't await in cleanup.
  // Deps are [sceneId] so the cleanup only fires on scene-switch or
  // true unmount, not on every keystroke.
  useEffect(() => {
    return () => {
      const s = editorStateRef.current;
      if (s.bodyDirty) {
        ipc.sceneWriteBody(sceneId, s.body).catch(() => {});
      }
      const trimmed = s.title.trim();
      if (trimmed.length > 0 && trimmed !== s.titleAtLastSave) {
        ipc.sceneRename(sceneId, trimmed).catch(() => {});
      }
    };
  }, [sceneId]);

  // Load title (via list lookup) and body on mount or scene switch.
  // After body load, push a fresh SceneState into the orchestrator so the
  // first telemetry tick can already build an excerpt.
  useEffect(() => {
    setBodyDirty(false);
    setSavedAt(null);
    let cancelled = false;
    let loadedWordCount = 0;
    (async () => {
      try {
        const list = await ipc.sceneList();
        if (cancelled) return;
        const me = list.find((s) => s.id === sceneId);
        if (me) {
          setTitle(me.name);
          setTitleAtLastSave(me.name);
          loadedWordCount = me.word_count;
        }
      } catch {
        /* swallow */
      }
    })();
    ipc
      .sceneRead(sceneId)
      .then((b) => {
        if (cancelled) return;
        setBody(b);
        // Best-effort: M2 only knows scene-level word_count + body. POV /
        // location / present-characters / project-level counts arrive in
        // M3/M4; ship 0 / empty until then.
        ipc
          .sceneState({
            sceneId,
            povCharacterId: null,
            locationId: null,
            charactersPresent: [],
            wordCount: loadedWordCount,
            bodyText: b,
            characterCount: 0,
            worldEntryCount: 0,
          })
          .catch(() => {});
      })
      .catch(() => {});
    return () => {
      cancelled = true;
      if (bodyDebounce.current !== undefined) {
        window.clearTimeout(bodyDebounce.current);
        bodyDebounce.current = undefined;
      }
    };
  }, [sceneId]);

  // Body autosave (dirty-flag-gated, 2 s debounce). After each successful
  // save we push the latest scene snapshot into the orchestrator so the
  // prompt-excerpt cache stays current. Fire-and-forget on both the save
  // and the snapshot push — UI doesn't surface either failure.
  //
  // M3 T21: also kick the character autosuggest scan and broadcast the
  // result via `publishAutosuggest`. The SceneMetadataSheet, if open
  // for this scene, picks it up and rerenders its "Suggested present"
  // chips. The autosuggest call is wrapped in try/catch — its failure
  // (or absence on older backends) MUST NOT affect the save flow, which
  // is the writer's source of truth. `cancelled` guards against a scene
  // switch landing between the save and the autosuggest call, which
  // would otherwise publish results for the wrong sceneId.
  useEffect(() => {
    if (!bodyDirty) return;
    if (bodyDebounce.current !== undefined) {
      window.clearTimeout(bodyDebounce.current);
    }
    let cancelled = false;
    bodyDebounce.current = window.setTimeout(() => {
      ipc
        .sceneWriteBody(sceneId, body)
        .then(async (info) => {
          if (cancelled) return;
          setSavedAt(Date.now());
          setBodyDirty(false);
          // Push real scene + project state to the orchestrator so the
          // trigger evaluators can gate on the writer's current world.
          // Each lookup is wrapped in `allSettled` so one failure (e.g.
          // a missing IPC on an older backend) doesn't poison the others.
          //
          // M4 fix: prior to this, the autosave path pushed null/0
          // hardcoded values from M2 — the orchestrator never learned
          // about scene.location_id, characters_present, or whether the
          // project actually had any characters/world entries. Result:
          // every trigger that gated on those fields (world_drift,
          // character_dissonance, the no_universe_yet inverse) was
          // dark.
          void (async () => {
            const [metaR, charsR, segsR] = await Promise.allSettled([
              ipc.sceneReadMetadata(sceneId),
              ipc.characterList(),
              ipc.worldSegmentList(),
            ]);
            if (cancelled) return;
            const meta =
              metaR.status === "fulfilled" ? metaR.value : null;
            const characterCount =
              charsR.status === "fulfilled" ? charsR.value.length : 0;
            let worldEntryCount = 0;
            if (segsR.status === "fulfilled") {
              const collectionSegs = segsR.value.filter(
                (s) => s.is_collection,
              );
              const perSeg = await Promise.allSettled(
                collectionSegs.map((s) =>
                  ipc.worldEntryList(s.id).then((es) => es.length),
                ),
              );
              if (cancelled) return;
              worldEntryCount = perSeg.reduce(
                (acc, r) =>
                  acc + (r.status === "fulfilled" ? r.value : 0),
                0,
              );
            }
            ipc
              .sceneState({
                sceneId,
                povCharacterId: meta?.pov_character_id ?? null,
                locationId: meta?.location?.id ?? null,
                charactersPresent: meta?.characters_present ?? [],
                wordCount: info.word_count,
                bodyText: body,
                characterCount,
                worldEntryCount,
              })
              .catch(() => {});
          })();
          // Autosuggest fan-out. Two scanners run in parallel:
          //   1. character — name-match against linked + known characters
          //   2. world — name+alias-match against `locations`-segment entries
          // Failures in either scanner are swallowed and the other still
          // ships. `cancelled` guards against a scene switch landing
          // between save and publish.
          try {
            const [charHits, worldHits] = await Promise.allSettled([
              ipc.characterAutosuggestForScene(sceneId, body),
              ipc.worldAutosuggest({ sceneId, paragraph: body }),
            ]);
            if (cancelled) return;
            const merged: ChipSuggestion[] = [];
            if (charHits.status === "fulfilled") {
              for (const h of charHits.value) {
                merged.push({
                  kind: "character",
                  characterId: h.character_id,
                  characterName: h.full_name,
                  mentionCount: h.mention_count,
                });
              }
            }
            if (worldHits.status === "fulfilled") {
              for (const h of worldHits.value) {
                merged.push({
                  kind: "world_entry",
                  entryId: h.id,
                  entryName: h.name,
                  // `world_autosuggest_core` already filters to
                  // `locations`-segment hits, so the renderer can rely
                  // on this label.
                  segmentSlug: "locations",
                });
              }
            }
            publishChips(sceneId, merged);
          } catch {
            /* swallow — autosuggest is best-effort */
          }
        })
        .catch(() => {});
    }, 2000);
    return () => {
      cancelled = true;
      if (bodyDebounce.current !== undefined) {
        window.clearTimeout(bodyDebounce.current);
      }
    };
  }, [body, sceneId, bodyDirty]);

  const handleTitleBlur = useCallback(async () => {
    const trimmed = title.trim();
    if (trimmed.length === 0) {
      setTitle(titleAtLastSave);
      return;
    }
    if (trimmed === titleAtLastSave) return;
    try {
      const info = await ipc.sceneRename(sceneId, trimmed);
      setTitleAtLastSave(info.name);
      setSavedAt(Date.now());
      onRenamed(info);
    } catch {
      /* swallow; user will retry */
    }
  }, [title, titleAtLastSave, sceneId, onRenamed]);

  return (
    <main
      ref={mainRef}
      style={{
        flex: 1,
        position: "relative",
        background: "var(--water-bg-paper)",
        overflow: "auto",
      }}
    >
      {/* Ambient water-ribbon — flows L→R behind the text column,
          fades around the central writing area, naturally occluded
          by pills + pinned column on z-index. */}
      <WaterRibbon />
      <div
        style={{
          position: "absolute",
          top: 14,
          right: 18,
          fontSize: "var(--water-fs-meta)",
          color: "var(--water-fg-faint)",
          pointerEvents: "none",
          display: "flex",
          alignItems: "center",
          gap: 6,
          opacity: savedAt ? 0.85 : 0,
          transition:
            "opacity var(--water-dur-medium) var(--water-ease-out-soft)",
        }}
      >
        {savedAt && (
          <>
            <span
              aria-hidden
              style={{
                width: 6,
                height: 6,
                borderRadius: "50%",
                background:
                  "color-mix(in srgb, var(--water-hue-flow) 70%, transparent)",
                boxShadow:
                  "0 0 8px color-mix(in srgb, var(--water-hue-flow) 60%, transparent)",
              }}
            />
            saved · {new Date(savedAt).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
          </>
        )}
      </div>
      <div
        style={{
          maxWidth: "var(--water-canvas-max)",
          margin: "0 auto",
          padding: "72px 24px 96px 24px",
          display: "flex",
          flexDirection: "column",
          gap: 4,
        }}
      >
        <HeatmapStrip sceneId={sceneId} />
        <input
          aria-label="Scene title"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          onBlur={handleTitleBlur}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              (e.target as HTMLInputElement).blur();
            }
          }}
          placeholder="Untitled"
          style={{
            border: "none",
            outline: "none",
            background: "transparent",
            color: "var(--water-fg-default)",
            fontFamily: "var(--water-font-serif)",
            fontSize: "var(--water-fs-display)",
            lineHeight: "var(--water-lh-display)",
            padding: 0,
            letterSpacing: -0.4,
          }}
        />
        <div
          aria-hidden
          style={{
            height: 1,
            width: 48,
            margin: "8px 0 24px 0",
            background:
              "color-mix(in srgb, var(--water-fg-faint) 35%, transparent)",
          }}
        />
        <Editor
          value={body}
          onChange={(md) => {
            setBody(md);
            setBodyDirty(true);
          }}
          placeholder="Begin where the universe begins."
        />
      </div>
      <PillLayer mainWidth={mainWidth} sceneId={sceneId} />
      <PinnedColumn mainWidth={mainWidth} sceneId={sceneId} />
    </main>
  );
}
