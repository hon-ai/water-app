import { useCallback, useEffect, useState } from "react";
import { ipc, type SceneInfo } from "../ipc/commands";
import { PlaceholderEditor } from "../components/PlaceholderEditor";

export function SceneList() {
  const [scenes, setScenes] = useState<SceneInfo[]>([]);
  const [active, setActive] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    const list = await ipc.sceneList();
    setScenes(list);
    if (!active && list.length > 0) {
      setActive(list[0]!.id);
    }
  }, [active]);

  useEffect(() => {
    refresh().catch(() => {});
  }, [refresh]);

  const onCreate = async () => {
    const name = `Scene ${scenes.length + 1}`;
    const created = await ipc.sceneCreate(name);
    await refresh();
    setActive(created.id);
  };

  return (
    <div style={{ display: "grid", gridTemplateColumns: "240px 1fr", gap: "16px", padding: "16px" }}>
      <aside>
        <button onClick={onCreate} style={{ marginBottom: 12 }}>+ new scene</button>
        <ul style={{ listStyle: "none", padding: 0, margin: 0 }}>
          {scenes.map((s) => (
            <li key={s.id} style={{ marginBottom: 6 }}>
              <button
                onClick={() => setActive(s.id)}
                style={{
                  textAlign: "left",
                  width: "100%",
                  background: s.id === active ? "var(--water-hue-flow)" : "transparent",
                  border: "none",
                  padding: "6px 10px",
                  borderRadius: "var(--water-r-16)",
                  cursor: "pointer",
                }}
              >
                {s.name}{" "}
                <span style={{ color: "var(--water-fg-faint)", fontSize: "var(--water-fs-meta)" }}>
                  ({s.word_count})
                </span>
              </button>
            </li>
          ))}
        </ul>
      </aside>
      <main>{active ? <PlaceholderEditor sceneId={active} /> : <p>create a scene to begin</p>}</main>
    </div>
  );
}
