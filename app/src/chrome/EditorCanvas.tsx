import { useCallback, useEffect, useRef, useState } from "react";
import { ipc, type SceneInfo } from "../ipc/commands";

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

  // Load title (via list lookup) and body on mount or scene switch.
  useEffect(() => {
    setBodyDirty(false);
    setSavedAt(null);
    let cancelled = false;
    (async () => {
      try {
        const list = await ipc.sceneList();
        if (cancelled) return;
        const me = list.find((s) => s.id === sceneId);
        if (me) {
          setTitle(me.name);
          setTitleAtLastSave(me.name);
        }
      } catch {
        /* swallow */
      }
    })();
    ipc
      .sceneRead(sceneId)
      .then((b) => {
        if (!cancelled) setBody(b);
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

  // Body autosave (dirty-flag-gated, 2 s debounce).
  useEffect(() => {
    if (!bodyDirty) return;
    if (bodyDebounce.current !== undefined) {
      window.clearTimeout(bodyDebounce.current);
    }
    bodyDebounce.current = window.setTimeout(() => {
      ipc
        .sceneWriteBody(sceneId, body)
        .then(() => {
          setSavedAt(Date.now());
          setBodyDirty(false);
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
        <textarea
          value={body}
          onChange={(e) => {
            setBody(e.target.value);
            setBodyDirty(true);
          }}
          rows={20}
          placeholder="Begin where the universe begins."
          style={{
            width: "100%",
            minHeight: 480,
            border: "none",
            outline: "none",
            resize: "none",
            background: "transparent",
            color: "var(--water-fg-default)",
            fontFamily: "var(--water-font-sans)",
            fontSize: "var(--water-fs-body)",
            lineHeight: "var(--water-lh-body)",
            padding: 0,
          }}
        />
      </div>
    </main>
  );
}
