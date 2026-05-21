import { useCallback, useEffect, useState } from "react";
import { ChevronDown, ChevronLeft, ChevronRight, MoreHorizontal, Plus } from "lucide-react";
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

  // When collapsed we still render a thin 28px handle so the writer can
  // Single root that lives across collapse/expand so the width can
  // animate smoothly between the 36px collapsed handle and the full
  // panel. Earlier we mounted two different elements depending on
  // the state, which let React unmount/remount and skipped the
  // transition entirely. With one persistent wrapper the width
  // transition runs end-to-end; the inner content swap happens with
  // overflow:hidden clipping any temporary overflow.
  return (
    <aside
      aria-label="scenes"
      data-collapsed={collapsed ? "true" : "false"}
      className="water-floating-panel"
      style={{
        width: collapsed ? 36 : "var(--water-scenes-w)",
        flexShrink: 0,
        overflow: "hidden",
        transition: `width var(--water-dur-medium) var(--water-ease-in-out-water)`,
        display: "flex",
        flexDirection: "column",
        margin: "10px 0 10px 10px",
      }}
    >
      {collapsed && (
        <button
          type="button"
          aria-label="Expand scenes"
          data-collapsed="true"
          onClick={onToggleCollapsed}
          style={{
            flex: 1,
            width: "100%",
            padding: 0,
            border: "none",
            background: "transparent",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            color: "var(--water-fg-muted)",
            cursor: "pointer",
            transition:
              "color var(--water-dur-tiny) var(--water-ease-out-soft)",
            animation:
              "water-pill-fade-in var(--water-dur-small) var(--water-ease-out-soft) both",
          }}
          onMouseEnter={(e) => {
            e.currentTarget.style.color = "var(--water-fg-default)";
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.color = "var(--water-fg-muted)";
          }}
        >
          <ChevronRight size={16} strokeWidth={1.5} />
        </button>
      )}
      {!collapsed && (
        <ScenesPanelExpanded
          scenes={scenes}
          activeSceneId={activeSceneId}
          projectName={projectName}
          onSelectScene={onSelectScene}
          onOpenProjectMenu={onOpenProjectMenu}
          onToggleCollapsed={onToggleCollapsed}
          onCreate={handleCreate}
          onOpenDetails={onOpenDetails}
        />
      )}
    </aside>
  );
}

interface ExpandedProps {
  scenes: SceneInfo[];
  activeSceneId: string | null;
  projectName: string;
  onSelectScene: (id: string) => void;
  onOpenProjectMenu: () => void;
  onToggleCollapsed: () => void;
  onCreate: () => void;
  onOpenDetails?: (id: string) => void;
}

function ScenesPanelExpanded({
  scenes,
  activeSceneId,
  projectName,
  onSelectScene,
  onOpenProjectMenu,
  onToggleCollapsed,
  onCreate,
  onOpenDetails,
}: ExpandedProps) {
  return (
    <div
      style={{
        // Hold the expanded panel at its natural width so when the
        // wrapper transitions from 36 → 280 the content doesn't squish
        // mid-animation. overflow:hidden on the wrapper clips the
        // briefly-too-wide content while the wrapper widens.
        width: "var(--water-scenes-w)",
        flex: 1,
        display: "flex",
        flexDirection: "column",
        // Slight slide-in from the left, matching the wrapper's
        // expansion. Plays once when this branch mounts; combined
        // with the wrapper's width transition the writer perceives
        // a smooth drawer opening.
        animation:
          "water-scenes-slide-in var(--water-dur-medium) var(--water-ease-out-soft) both",
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
          aria-label="Project menu"
          title="Switch or close project"
          style={{
            flex: 1,
            display: "flex",
            alignItems: "center",
            gap: 6,
            border: "none",
            background:
              "color-mix(in srgb, var(--water-fg-faint) 8%, transparent)",
            color: "var(--water-fg-default)",
            fontFamily: "var(--water-font-sans)",
            fontSize: "var(--water-fs-ui)",
            fontWeight: 500,
            padding: "5px 10px",
            borderRadius: "var(--water-r-8)",
            cursor: "pointer",
            textAlign: "left",
            transition:
              "background-color var(--water-dur-tiny) var(--water-ease-out-soft)",
          }}
          onMouseEnter={(e) => {
            e.currentTarget.style.background =
              "color-mix(in srgb, var(--water-fg-faint) 16%, transparent)";
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.background =
              "color-mix(in srgb, var(--water-fg-faint) 8%, transparent)";
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
        onClick={onCreate}
        style={{
          margin: "0 12px 10px 12px",
          padding: "8px 12px",
          display: "flex",
          alignItems: "center",
          gap: 8,
          border: "none",
          background:
            "color-mix(in srgb, var(--water-hue-flow) 18%, transparent)",
          color: "var(--water-fg-default)",
          fontFamily: "var(--water-font-sans)",
          fontSize: "var(--water-fs-ui)",
          fontWeight: 500,
          cursor: "pointer",
          borderRadius: "var(--water-r-8)",
          textAlign: "left",
          transition:
            "background-color var(--water-dur-tiny) var(--water-ease-out-soft)",
        }}
        onMouseEnter={(e) => {
          e.currentTarget.style.background =
            "color-mix(in srgb, var(--water-hue-flow) 30%, transparent)";
        }}
        onMouseLeave={(e) => {
          e.currentTarget.style.background =
            "color-mix(in srgb, var(--water-hue-flow) 18%, transparent)";
        }}
      >
        <Plus size={14} strokeWidth={1.75} />
        New scene
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
                  // Reserve right space for the absolute-positioned
                  // details (3-dots) button so the word count doesn't
                  // collide with it on hover.
                  padding: onOpenDetails ? "6px 32px 6px 10px" : "6px 10px",
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
    </div>
  );
}
