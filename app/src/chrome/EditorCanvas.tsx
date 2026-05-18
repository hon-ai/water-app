import { useCallback, useEffect, useRef, useState } from "react";
import { ipc, type SceneInfo } from "../ipc/commands";
import { Editor } from "../editor/Editor";
import { PillLayer } from "../pill/PillLayer";
import { PinnedColumn } from "../pill/PinnedColumn";
import { useElementWidth } from "../pill/useElementWidth";

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
  useEffect(() => {
    if (!bodyDirty) return;
    if (bodyDebounce.current !== undefined) {
      window.clearTimeout(bodyDebounce.current);
    }
    bodyDebounce.current = window.setTimeout(() => {
      ipc
        .sceneWriteBody(sceneId, body)
        .then((info) => {
          setSavedAt(Date.now());
          setBodyDirty(false);
          ipc
            .sceneState({
              sceneId,
              povCharacterId: null,
              locationId: null,
              charactersPresent: [],
              wordCount: info.word_count,
              bodyText: body,
              characterCount: 0,
              worldEntryCount: 0,
            })
            .catch(() => {});
        })
        .catch(() => {});
    }, 2000);
    return () => {
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
      <div
        style={{
          position: "absolute",
          top: 12,
          right: 16,
          fontSize: "var(--water-fs-meta)",
          color: "var(--water-fg-faint)",
          pointerEvents: "none",
        }}
      >
        {savedAt ? `saved · ${new Date(savedAt).toLocaleTimeString()}` : ""}
      </div>
      <div
        style={{
          maxWidth: "var(--water-canvas-max)",
          margin: "0 auto",
          padding: "72px 24px 96px 24px",
          display: "flex",
          flexDirection: "column",
          gap: 16,
        }}
      >
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
      <PillLayer mainWidth={mainWidth} />
      <PinnedColumn mainWidth={mainWidth} />
    </main>
  );
}
