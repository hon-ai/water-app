import { useCallback, useEffect, useState } from "react";
import { ThemeProvider } from "./theme/ThemeProvider";
import { IconRail, type NavTarget } from "./chrome/IconRail";
import { ScenesPanel } from "./chrome/ScenesPanel";
import { ProjectMenu } from "./chrome/ProjectMenu";
import { EditorCanvas } from "./chrome/EditorCanvas";
import { EmptyState } from "./chrome/EmptyState";
import { CharactersSurface } from "./chrome/CharactersSurface";
import { WorldsSurface } from "./worlds/WorldsSurface";
import { CanvasSurface } from "./canvas/CanvasSurface";
import { CreateProjectSheet } from "./sheets/CreateProjectSheet";
import { SettingsSheet } from "./sheets/SettingsSheet";
import { SceneMetadataSheet } from "./scenes/SceneMetadataSheet";
import { ipc, type SceneInfo } from "./ipc/commands";
import { dialog } from "./ipc/dialog";

const COLLAPSED_KEY = "water:scenes-collapsed";

export default function App() {
  const [projectOpen, setProjectOpen] = useState(false);
  const [projectRoot, setProjectRoot] = useState<string | null>(null);
  const [projectName, setProjectName] = useState<string>("");
  const [activeNav, setActiveNav] = useState<NavTarget>("scenes");
  const [activeSceneId, setActiveSceneId] = useState<string | null>(null);
  const [createOpen, setCreateOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [projectMenuOpen, setProjectMenuOpen] = useState(false);
  const [scenesCollapsed, setScenesCollapsed] = useState<boolean>(() => {
    return localStorage.getItem(COLLAPSED_KEY) === "true";
  });
  const [scenesReloadKey, setScenesReloadKey] = useState(0);
  // Scene id whose metadata sheet is open (M3 T21). `null` = closed.
  const [detailsSceneId, setDetailsSceneId] = useState<string | null>(null);

  // Poll project-open status; cheap, lets the shell react to externally-triggered
  // open/close (the diagnostics_status command returns has_open_project + path).
  const refreshStatus = useCallback(async () => {
    try {
      const s = await ipc.diagnosticsStatus();
      setProjectOpen(s.has_open_project);
      setProjectRoot(s.project_root);
      if (!s.has_open_project) {
        setActiveSceneId(null);
      }
    } catch {
      /* swallow */
    }
  }, []);

  useEffect(() => {
    refreshStatus();
    const t = window.setInterval(() => refreshStatus(), 3000);
    return () => window.clearInterval(t);
  }, [refreshStatus]);

  // Derive a friendly project name from the path (last segment minus .water suffix).
  useEffect(() => {
    if (!projectRoot) {
      setProjectName("");
      return;
    }
    const slash = projectRoot.lastIndexOf("\\") >= 0 ? "\\" : "/";
    const last = projectRoot.split(slash).filter(Boolean).pop() ?? "";
    setProjectName(last.replace(/\.water$/, ""));
  }, [projectRoot]);

  const toggleCollapsed = useCallback(() => {
    setScenesCollapsed((prev) => {
      const next = !prev;
      localStorage.setItem(COLLAPSED_KEY, next ? "true" : "false");
      return next;
    });
  }, []);

  const handleCreated = useCallback(async () => {
    await refreshStatus();
    setScenesReloadKey((k) => k + 1);
  }, [refreshStatus]);

  const handleOpenExisting = useCallback(async () => {
    const root = await dialog.pickFolder();
    if (!root) return;
    try {
      await ipc.openProject(root);
      await refreshStatus();
      setScenesReloadKey((k) => k + 1);
    } catch {
      /* swallow */
    }
  }, [refreshStatus]);

  const handleCloseProject = useCallback(async () => {
    try {
      await ipc.closeProject();
      setActiveSceneId(null);
      await refreshStatus();
    } catch {
      /* swallow */
    }
  }, [refreshStatus]);

  const handleSceneCreated = useCallback((info: SceneInfo) => {
    setActiveSceneId(info.id);
  }, []);

  const handleSceneRenamed = useCallback(() => {
    setScenesReloadKey((k) => k + 1);
  }, []);

  return (
    <ThemeProvider>
      <div
        className="water-shell"
        style={{
          display: "flex",
          height: "100vh",
          width: "100vw",
          background: "var(--water-bg-paper)",
          color: "var(--water-fg-default)",
          fontFamily: "var(--water-font-sans)",
          overflow: "hidden",
        }}
      >
        <IconRail
          active={activeNav}
          onSelect={setActiveNav}
          onOpenSettings={() => setSettingsOpen(true)}
          projectOpen={projectOpen}
        />
        {!projectOpen ? (
          <EmptyState
            onCreate={() => setCreateOpen(true)}
            onOpen={handleOpenExisting}
          />
        ) : activeNav === "characters" ? (
          <div
            key="surface-characters"
            style={{ flex: 1, display: "flex", animation: "water-surface-fade var(--water-dur-medium) var(--water-ease-out-soft) both" }}
          >
            <CharactersSurface />
          </div>
        ) : activeNav === "world" ? (
          <div
            key="surface-world"
            style={{ flex: 1, display: "flex", animation: "water-surface-fade var(--water-dur-medium) var(--water-ease-out-soft) both" }}
          >
            <WorldsSurface projectId={projectRoot ?? ""} />
          </div>
        ) : activeNav === "canvas" ? (
          <div
            key="surface-canvas"
            style={{ flex: 1, display: "flex", animation: "water-surface-fade var(--water-dur-medium) var(--water-ease-out-soft) both" }}
          >
            <CanvasSurface
              onOpenScene={(id) => {
                setActiveSceneId(id);
                setActiveNav("scenes");
              }}
            />
          </div>
        ) : (
          <div
            key="surface-scenes"
            style={{
              flex: 1,
              display: "flex",
              animation:
                "water-surface-fade var(--water-dur-medium) var(--water-ease-out-soft) both",
            }}
          >
            <div style={{ position: "relative", display: "flex" }}>
              <ScenesPanel
                reloadToken={scenesReloadKey}
                projectName={projectName}
                activeSceneId={activeSceneId}
                onSelectScene={setActiveSceneId}
                onCreateScene={handleSceneCreated}
                onOpenProjectMenu={() => setProjectMenuOpen((v) => !v)}
                collapsed={scenesCollapsed}
                onToggleCollapsed={toggleCollapsed}
                onOpenDetails={(id) => setDetailsSceneId(id)}
              />
              {!scenesCollapsed && (
                <ProjectMenu
                  open={projectMenuOpen}
                  onClose={() => setProjectMenuOpen(false)}
                  onSwitchProject={handleOpenExisting}
                  onCloseProject={handleCloseProject}
                />
              )}
            </div>
            {activeSceneId ? (
              <EditorCanvas
                key={activeSceneId}
                sceneId={activeSceneId}
                onRenamed={handleSceneRenamed}
              />
            ) : (
              <main
                style={{
                  flex: 1,
                  background: "var(--water-bg-paper)",
                  display: "grid",
                  placeItems: "center",
                  fontFamily: "var(--water-font-sans)",
                  padding: 24,
                }}
              >
                <div
                  style={{
                    maxWidth: 360,
                    textAlign: "center",
                    display: "flex",
                    flexDirection: "column",
                    gap: 14,
                    color: "var(--water-fg-muted)",
                    animation:
                      "water-pill-fade-in var(--water-dur-medium) var(--water-ease-out-soft) both",
                  }}
                >
                  <div
                    style={{
                      width: 56,
                      height: 56,
                      borderRadius: "50%",
                      margin: "0 auto",
                      background:
                        "color-mix(in srgb, var(--water-hue-flow) 22%, transparent)",
                      boxShadow:
                        "0 0 32px color-mix(in srgb, var(--water-hue-flow) 35%, transparent)",
                    }}
                    aria-hidden
                  />
                  <h2
                    style={{
                      margin: 0,
                      fontSize: "var(--water-fs-title)",
                      fontWeight: 500,
                      color: "var(--water-fg-default)",
                      letterSpacing: -0.2,
                    }}
                  >
                    Begin a scene
                  </h2>
                  <p
                    style={{
                      margin: 0,
                      fontSize: "var(--water-fs-ui)",
                      lineHeight: 1.5,
                      color: "var(--water-fg-muted)",
                    }}
                  >
                    Pick one from the left panel, or use{" "}
                    <strong style={{ color: "var(--water-fg-default)" }}>
                      New scene
                    </strong>{" "}
                    to start a fresh one. Water listens as you write and
                    surfaces small noticings in the margin.
                  </p>
                </div>
              </main>
            )}
            {detailsSceneId !== null && (
              <SceneMetadataSheet
                key={detailsSceneId}
                sceneId={detailsSceneId}
                open={true}
                onClose={() => setDetailsSceneId(null)}
              />
            )}
          </div>
        )}
        <CreateProjectSheet
          open={createOpen}
          onClose={() => setCreateOpen(false)}
          onCreated={handleCreated}
        />
        <SettingsSheet open={settingsOpen} onClose={() => setSettingsOpen(false)} />
      </div>
    </ThemeProvider>
  );
}
