import { useCallback, useEffect, useState } from "react";
import { ChevronDown, ChevronLeft, MoreHorizontal, Plus } from "lucide-react";
import { ipc, type SceneInfo } from "../ipc/commands";

interface Props {
  projectName: string;
  activeSceneId: string | null;
  onSelectScene: (id: string) => void;
  onCreateScene: (info: SceneInfo) => void;
  onOpenProjectMenu: () => void;
  collapsed: boolean;
  onToggleCollapsed: () => void;
  /**
   * Bumping this value triggers a scene-list reload without remounting the
   * component, so internal state (e.g. scroll position) is preserved.
   */
  reloadToken?: number;
  /**
   * Opens the SceneMetadataSheet for the given scene (M3 T21). The
   * Details button per row calls this; the parent (App) owns the
   * sheet's open state. Optional with a no-op default so existing
   * callers and tests don't break, but in production App always
   * supplies a real handler.
   */
  onOpenDetails?: (id: string) => void;
}

export function ScenesPanel({
  projectName,
  activeSceneId,
  onSelectScene,
  onCreateScene,
  onOpenProjectMenu,
  collapsed,
  onToggleCollapsed,
  reloadToken = 0,
  onOpenDetails,
}: Props) {
  const [scenes, setScenes] = useState<SceneInfo[]>([]);

  const refresh = useCallback(async () => {
    try {
      const list = await ipc.sceneList();
      setScenes(list);
    } catch {
      /* swallow — sidebar shows empty list */
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh, reloadToken]);

  const handleCreate = async () => {
    try {
      const name = `Scene ${scenes.length + 1}`;
      const info = await ipc.sceneCreate(name);
      onCreateScene(info);
      await refresh();
    } catch {
      /* swallow */
    }
  };

  return (
    <aside
      aria-label="scenes"
      data-collapsed={collapsed ? "true" : "false"}
      style={{
        width: collapsed ? 0 : "var(--water-scenes-w)",
        flexShrink: 0,
        overflow: "hidden",
        transition: `width var(--water-dur-medium) var(--water-ease-out-soft)`,
        background: "var(--water-bg-canvas)",
        display: "flex",
        flexDirection: "column",
      }}
    >
      <div
        style={{
          position: "relative",
          display: "flex",
          alignItems: "center",
          gap: 4,
          padding: "12px 12px 8px 12px",
        }}
      >
        <button
          type="button"
          onClick={onOpenProjectMenu}
          style={{
            flex: 1,
            display: "flex",
            alignItems: "center",
            gap: 6,
            border: "none",
            background: "transparent",
            color: "var(--water-fg-default)",
            fontFamily: "var(--water-font-sans)",
            fontSize: "var(--water-fs-ui)",
            fontWeight: 500,
            padding: "4px 8px",
            borderRadius: "var(--water-r-8)",
            cursor: "pointer",
            textAlign: "left",
          }}
        >
          <span style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
            {projectName}
          </span>
          <ChevronDown size={14} strokeWidth={1.5} />
        </button>
        <button
          type="button"
          aria-label="Collapse scenes"
          onClick={onToggleCollapsed}
          style={{
            width: 28,
            height: 28,
            display: "grid",
            placeItems: "center",
            border: "none",
            background: "transparent",
            color: "var(--water-fg-muted)",
            cursor: "pointer",
            borderRadius: "var(--water-r-8)",
          }}
        >
          <ChevronLeft size={14} strokeWidth={1.5} />
        </button>
      </div>

      <button
        type="button"
        onClick={handleCreate}
        style={{
          margin: "0 12px 8px 12px",
          padding: "6px 10px",
          display: "flex",
          alignItems: "center",
          gap: 6,
          border: "none",
          background: "transparent",
          color: "var(--water-fg-muted)",
          fontFamily: "var(--water-font-sans)",
          fontSize: "var(--water-fs-ui)",
          cursor: "pointer",
          borderRadius: "var(--water-r-8)",
          textAlign: "left",
        }}
      >
        <Plus size={14} strokeWidth={1.5} />
        new scene
      </button>

      <ul style={{ listStyle: "none", padding: 0, margin: 0, overflowY: "auto", flex: 1 }}>
        {scenes.map((s) => {
          const isActive = s.id === activeSceneId;
          return (
            <li
              key={s.id}
              data-scene-row
              style={{
                position: "relative",
                display: "flex",
                alignItems: "center",
              }}
            >
              <button
                type="button"
                aria-label={s.name}
                onClick={() => onSelectScene(s.id)}
                style={{
                  display: "flex",
                  alignItems: "baseline",
                  width: "calc(100% - 16px)",
                  margin: "2px 8px",
                  padding: "6px 10px",
                  border: "none",
                  background: isActive
                    ? "color-mix(in srgb, var(--water-hue-flow) 30%, transparent)"
                    : "transparent",
                  color: "var(--water-fg-default)",
                  fontFamily: "var(--water-font-sans)",
                  fontSize: "var(--water-fs-ui)",
                  borderRadius: "var(--water-r-8)",
                  cursor: "pointer",
                  textAlign: "left",
                  gap: 8,
                }}
              >
                <span style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                  {s.name}
                </span>
                <span
                  style={{
                    color: "var(--water-fg-faint)",
                    fontSize: "var(--water-fs-meta)",
                    flexShrink: 0,
                  }}
                >
                  {s.word_count}
                </span>
              </button>
              {onOpenDetails && (
                <button
                  type="button"
                  data-scene-details
                  aria-label={`Scene details: ${s.name}`}
                  onClick={(e) => {
                    e.stopPropagation();
                    onOpenDetails(s.id);
                  }}
                  style={{
                    position: "absolute",
                    right: 14,
                    top: "50%",
                    transform: "translateY(-50%)",
                    width: 22,
                    height: 22,
                    display: "grid",
                    placeItems: "center",
                    border: "none",
                    background: "transparent",
                    color: "var(--water-fg-muted)",
                    cursor: "pointer",
                    borderRadius: "var(--water-r-8)",
                  }}
                >
                  <MoreHorizontal size={14} strokeWidth={1.5} />
                </button>
              )}
            </li>
          );
        })}
      </ul>
    </aside>
  );
}
