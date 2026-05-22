import { useCallback, useEffect, useRef, useState } from "react";
import { ipc, type EditorPillRow, type SceneInfo } from "../ipc/commands";
import { onWaterEvent } from "../ipc/events";
import { Editor } from "../editor/Editor";
import { DiagnosticsList } from "../editor/DiagnosticsList";
import { extractEditorBlocks } from "../editor/extractBlocks";
import { PillLayer } from "../pill/PillLayer";
import { HeatmapStrip } from "../heat/HeatmapStrip";
import { LiveModelChip } from "./LiveModelChip";
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
  // Phase 5 — live editor pills for this scene. Drives both the
  // diagnostics list under the editor and the inline-underline
  // decorations dispatched via the `water:set-editor-underlines`
  // event bridge.
  const [editorPills, setEditorPills] = useState<EditorPillRow[]>([]);
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
    let loadedOrdering: number | null = null;
    let loadedManuscriptSceneCount: number | null = null;
    (async () => {
      try {
        const list = await ipc.sceneList();
        if (cancelled) return;
        const me = list.find((s) => s.id === sceneId);
        if (me) {
          setTitle(me.name);
          setTitleAtLastSave(me.name);
          loadedWordCount = me.word_count;
          loadedOrdering = me.ordering;
          loadedManuscriptSceneCount = list.length;
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
        // M3/M4; ship 0 / empty until then. Phase 6 — ordering fields
        // come from the scene-list fetch above; the orchestrator uses
        // them to derive the arc-position bucket for pill prompts.
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
            sceneOrdering: loadedOrdering,
            manuscriptSceneCount: loadedManuscriptSceneCount,
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

  // Phase 5 — hydrate editor pills + underlines on scene switch.
  // The store keeps non-dismissed pills across sessions, so a
  // fresh open shows the writer what was flagged last time.
  // Also subscribes to `editor_pills:updated` so a dismiss from
  // elsewhere refreshes the underlines here.
  useEffect(() => {
    let cancelled = false;
    const refetch = async () => {
      try {
        const raw = await ipc.editorPillsList(sceneId);
        // Tauri commands return their declared type in production
        // but mocks/test stubs sometimes resolve to `undefined`; the
        // coerce keeps the renderer state invariant (always an array).
        const live: EditorPillRow[] = Array.isArray(raw) ? raw : [];
        if (cancelled) return;
        setEditorPills(live);
        window.dispatchEvent(
          new CustomEvent("water:set-editor-underlines", {
            detail: live.map((p) => ({
              pillId: p.id,
              blockId: p.anchor_block_id,
              start: p.anchor_start,
              end: p.anchor_end,
              severity: p.severity,
            })),
          }),
        );
      } catch {
        /* swallow */
      }
    };
    void refetch();
    let unsub: (() => void) | undefined;
    void (async () => {
      const u = await onWaterEvent("editor_pills:updated", (e) => {
        if (e.scene_id === sceneId) void refetch();
      });
      if (cancelled) {
        u();
        return;
      }
      unsub = u;
    })();
    return () => {
      cancelled = true;
      unsub?.();
      window.dispatchEvent(new Event("water:clear-editor-underlines"));
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
            const [metaR, charsR, segsR, listR] = await Promise.allSettled([
              ipc.sceneReadMetadata(sceneId),
              ipc.characterList(),
              ipc.worldSegmentList(),
              ipc.sceneList(),
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
            // Phase 6 — scene ordering for arc-position derivation.
            // Falls to null when the list call failed; orchestrator
            // simply omits the arc line.
            let sceneOrdering: number | null = null;
            let manuscriptSceneCount: number | null = null;
            if (listR.status === "fulfilled") {
              manuscriptSceneCount = listR.value.length;
              const me = listR.value.find((s) => s.id === sceneId);
              if (me) sceneOrdering = me.ordering;
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
                sceneOrdering,
                manuscriptSceneCount,
              })
              .catch(() => {});
            // Phase 5 — run the diagnostic engine across the
            // rendered blocks. Fire-and-forget; on response we
            // update local state + dispatch the underline anchors
            // into the editor's PM plugin.
            const blocks = extractEditorBlocks();
            if (blocks.length > 0) {
              try {
                const raw = await ipc.editorPillsRun(sceneId, blocks);
                const live: EditorPillRow[] = Array.isArray(raw) ? raw : [];
                if (cancelled) return;
                setEditorPills(live);
                window.dispatchEvent(
                  new CustomEvent("water:set-editor-underlines", {
                    detail: live.map((p) => ({
                      pillId: p.id,
                      blockId: p.anchor_block_id,
                      start: p.anchor_start,
                      end: p.anchor_end,
                      severity: p.severity,
                    })),
                  }),
                );
              } catch {
                /* swallow — diagnostics is best-effort */
              }
              // Phase 5.8 — also fire LLM polish requests for blocks
              // substantial enough to warrant a paragraph-level
              // observation. The orchestrator enforces a per-scene
              // cap (5 / session) + per-block cooldown (30s), so the
              // bulk of these are no-ops. Fire-and-forget; polish
              // results arrive via `editor_pills:updated`.
              for (const b of blocks) {
                const words = b.text.split(/\s+/).filter(Boolean).length;
                if (words < 25) continue;
                void ipc
                  .editorPolishRequest(sceneId, b.blockId, b.text)
                  .catch(() => {
                    /* swallow — best-effort */
                  });
              }
            }
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
      style={{
        flex: 1,
        display: "flex",
        flexDirection: "row",
        // `position: relative` so absolutely-positioned children
        // (LiveModelChip) anchor to the outer main rather than to
        // the inner scrolling editor div. Without this, the chip
        // would scroll with the prose since it'd sit inside the
        // scroll container's coordinate system.
        position: "relative",
        // Transparent at the outer layer; the inner editor div +
        // pill panel paint their own surfaces.
        background: "transparent",
        // The inner editor column owns its own vertical scroll;
        // the panel scrolls independently.
        overflow: "hidden",
      }}
    >
      <div
        ref={mainRef}
        style={{
          flex: 1,
          position: "relative",
          background: "transparent",
          overflow: "auto",
          minWidth: 0,
        }}
      >
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
          // Wide backdrop that fades horizontally into the stream on
          // either side of the prose. Centered with margin: 0 auto;
          // a soft linear gradient transitions from transparent at
          // the very edges to opaque bg-paper across the prose area
          // and back. Result: stream visible at shoulders, gentle
          // halo around the writing column — no hard cut-off.
          maxWidth: "calc(var(--water-canvas-max) + 240px)",
          margin: "0 auto",
          padding: "72px 144px 96px 144px",
          display: "flex",
          flexDirection: "column",
          gap: 4,
          // Symmetric absolute-pixel fades so the transition feels
          // the same on both shoulders at any window size. Both
          // sides 56 px — short enough that the wrapper doesn't
          // expose a wide dark band against the nudge panel at
          // narrow widths (the previous 128 px right fade created
          // a visible dark stripe between the prose and the panel
          // that looked like a stray shadow behind the panel),
          // long enough that the bg-paper-to-stream transition
          // stays soft instead of a hard edge.
          background:
            "linear-gradient(90deg, transparent 0, var(--water-bg-paper) 56px, var(--water-bg-paper) calc(100% - 56px), transparent 100%)",
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
        {/* Title accent rule. Uses the active sea palette so it reads
            as a deliberate brand accent rather than a brownish line —
            --water-fg-faint at hsl(0, 3%, 28%) in dark mode looked
            like a brown smudge. */}
        <div
          aria-hidden
          style={{
            height: 1,
            width: 48,
            margin: "8px 0 24px 0",
            background:
              "color-mix(in srgb, var(--water-sea-300) 45%, transparent)",
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
        <DiagnosticsList
          pills={editorPills}
          onAccept={(pill) => {
            if (!pill.suggestion) return;
            // Splice the suggestion into the manuscript via the
            // editor's event bridge. The PM tr is dispatched
            // synchronously so the underline + row both clear
            // before the diagnostic engine re-runs on autosave.
            window.dispatchEvent(
              new CustomEvent("water:accept-editor-pill", {
                detail: {
                  blockId: pill.anchor_block_id,
                  start: pill.anchor_start,
                  end: pill.anchor_end,
                  replacement: pill.suggestion,
                },
              }),
            );
            // Optimistically remove the row; mirror the dismiss flow.
            setEditorPills((prev) => prev.filter((p) => p.id !== pill.id));
            window.dispatchEvent(
              new CustomEvent("water:set-editor-underlines", {
                detail: editorPills
                  .filter((p) => p.id !== pill.id)
                  .map((p) => ({
                    pillId: p.id,
                    blockId: p.anchor_block_id,
                    start: p.anchor_start,
                    end: p.anchor_end,
                    severity: p.severity,
                  })),
              }),
            );
            void ipc.editorPillDismiss(pill.id).catch(() => {});
          }}
          onDismiss={(id) => {
            // Optimistic local removal so the row + underline
            // disappear before the IPC round-trip lands.
            setEditorPills((prev) => prev.filter((p) => p.id !== id));
            window.dispatchEvent(
              new CustomEvent("water:set-editor-underlines", {
                detail: editorPills
                  .filter((p) => p.id !== id)
                  .map((p) => ({
                    pillId: p.id,
                    blockId: p.anchor_block_id,
                    start: p.anchor_start,
                    end: p.anchor_end,
                    severity: p.severity,
                  })),
              }),
            );
            void ipc.editorPillDismiss(id).catch(() => {
              /* swallow — the editor_pills:updated event from a
                 future re-run will reconcile if the dismiss
                 transiently failed. */
            });
          }}
        />
      </div>
      <PinnedColumn mainWidth={mainWidth} sceneId={sceneId} />
      </div>
      {/* LiveModelChip lives at `<main>` level (not inside the
          scrollable editor div) so its `position: absolute; bottom:
          14` anchors to main's viewport-stable bottom edge. Inside
          the editor div, `bottom` would resolve against the
          content's full scrollable height — the chip would scroll
          off-screen along with the prose. */}
      <LiveModelChip />
      {/* Right-side nudge panel — fixed-width flex sibling of the
          editor column. Replaces the previous absolute-positioned
          overlay so pills never overlap prose at any window width,
          and the column reflows correctly on window resize without
          the manual coord math (which used to leave the column cut
          off after a compress + widen cycle). */}
      <aside
        aria-label="nudges"
        className="water-floating-panel"
        style={{
          width: 280,
          flexShrink: 0,
          display: "flex",
          flexDirection: "column",
          // Left margin of 12 px creates a breathing gap between
          // the editor column and the panel — the WaterRibbon
          // shows through this gap rather than the panel's glass
          // border meeting the wrapper's right gradient fade as
          // a hard line.
          margin: "10px 10px 10px 12px",
          background:
            "color-mix(in srgb, var(--water-bg-paper) 55%, transparent)",
          backdropFilter: "blur(22px) saturate(160%)",
          WebkitBackdropFilter: "blur(22px) saturate(160%)",
          border:
            "1px solid color-mix(in srgb, var(--water-hairline) 60%, transparent)",
          borderRadius: "var(--water-r-24)",
          boxShadow: "var(--water-elev-2)",
          overflow: "hidden",
        }}
      >
        <header
          style={{
            padding: "16px 18px 10px",
            borderBottom:
              "1px solid color-mix(in srgb, var(--water-hairline) 35%, transparent)",
            fontFamily: "var(--water-font-sans)",
            fontSize: 10,
            fontWeight: 700,
            textTransform: "uppercase",
            letterSpacing: 0.6,
            color: "var(--water-fg-muted)",
          }}
        >
          Nudges
        </header>
        <div style={{ flex: 1, minHeight: 0, position: "relative" }}>
          <PillLayer mainWidth={mainWidth} sceneId={sceneId} />
        </div>
      </aside>
    </main>
  );
}
