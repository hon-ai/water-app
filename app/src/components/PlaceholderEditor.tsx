import { useEffect, useRef, useState } from "react";
import { ipc } from "../ipc/commands";

interface Props {
  sceneId: string;
}

export function PlaceholderEditor({ sceneId }: Props) {
  const [text, setText] = useState("");
  const [savedAt, setSavedAt] = useState<number | null>(null);
  const [dirty, setDirty] = useState(false);
  const debounce = useRef<number | undefined>(undefined);

  // Reset dirty + clear pending writes whenever the scene changes.
  useEffect(() => {
    setDirty(false);
    setSavedAt(null);
    ipc.sceneRead(sceneId).then(setText).catch(() => {});
    return () => {
      if (debounce.current !== undefined) {
        window.clearTimeout(debounce.current);
        debounce.current = undefined;
      }
    };
  }, [sceneId]);

  // Per spec § 3.7: autosave fires after typing has been idle for ≥ 2 s.
  // Only fires when the user has actually changed the body (dirty=true).
  useEffect(() => {
    if (!dirty) return;
    if (debounce.current !== undefined) {
      window.clearTimeout(debounce.current);
    }
    debounce.current = window.setTimeout(() => {
      ipc
        .sceneWriteBody(sceneId, text)
        .then(() => {
          setSavedAt(Date.now());
          setDirty(false);
        })
        .catch(() => {});
    }, 2000);
    return () => {
      if (debounce.current !== undefined) {
        window.clearTimeout(debounce.current);
      }
    };
  }, [text, sceneId, dirty]);

  return (
    <div className="water-placeholder-editor">
      <textarea
        value={text}
        onChange={(e) => {
          setText(e.target.value);
          setDirty(true);
        }}
        rows={20}
        style={{
          width: "100%",
          fontFamily: "inherit",
          fontSize: "var(--water-fs-body)",
          lineHeight: "var(--water-lh-body)",
          background: "var(--water-bg-paper)",
          color: "var(--water-fg-default)",
          border: "none",
          outline: "none",
          padding: "16px",
          borderRadius: "var(--water-r-16)",
        }}
        placeholder="Begin where the universe begins."
      />
      <div style={{ fontSize: "var(--water-fs-meta)", color: "var(--water-fg-faint)" }}>
        {savedAt ? `saved at ${new Date(savedAt).toLocaleTimeString()}` : "unsaved"}
      </div>
    </div>
  );
}
