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
import { WaterRibbon } from "./chrome/WaterRibbon";
import { applyUpdateInBackground, checkForUpdate } from "./boot";
import type { Update } from "@tauri-apps/plugin-updater";

/** Track viewport dimensions for the App-level WaterRibbon. */
function useWindowSize() {
  const [size, setSize] = useState({
    width: typeof window === "undefined" ? 0 : window.innerWidth,
    height: typeof window === "undefined" ? 0 : window.innerHeight,
  });
  useEffect(() => {
    const onResize = () =>
      setSize({ width: window.innerWidth, height: window.innerHeight });
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);
  return size;
}

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

  // True once any provider has been Tested green (or after the
  // orchestrator has cached a successful bouquet from a real pill).
  // Drives the "no provider configured" banner — without this signal
  // a tester writes for a while expecting pills, sees nothing, and
  // has no way to learn that a provider needs setup.
  const [hasActiveProvider, setHasActiveProvider] = useState(false);
  // Auto-updater state. `available` populates once `check()` returns
  // a non-null `Update`; the writer dismisses via `setAvailable(null)`
  // or applies via the toast's button.
  const [updateAvailable, setUpdateAvailable] = useState<Update | null>(null);
  const [updateApplied, setUpdateApplied] = useState(false);
  // Run the update check exactly once per app boot. The Tauri plugin
  // does its own deduping but we don't want a re-render to refetch.
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      const u = await checkForUpdate();
      if (!cancelled && u) setUpdateAvailable(u);
    })();
    return () => {
      cancelled = true;
    };
  }, []);

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
      // `provider_health` is empty until the writer Tests at least
      // one provider. Once any provider reports ok, the banner clears.
      setHasActiveProvider(
        (s.provider_health ?? []).some((p) => p.ok === true),
      );
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

  const { width: windowWidth, height: windowHeight } = useWindowSize();
  // Begin-a-scene = project open, no active scene, scenes nav. Narrow
  // stream margins for that page; open everywhere else.
  const streamMode: "open" | "narrow" =
    projectOpen && !activeSceneId && activeNav === "scenes"
      ? "narrow"
      : "open";

  return (
    <ThemeProvider>
      <div
        className="water-shell"
        style={{
          position: "relative",
          display: "flex",
          height: "100vh",
          width: "100vw",
          background: "var(--water-bg-paper)",
          color: "var(--water-fg-default)",
          fontFamily: "var(--water-font-sans)",
          overflow: "hidden",
        }}
      >
        {/* App-level water ribbon. Renders behind everything; the
            IconRail's glass surface shows it (blurred) through the
            backdrop-filter. Surfaces with bg-paper backgrounds occlude
            the ribbon in their body; surfaces that go transparent
            (EmptyState, Begin-a-scene main) let it through. */}
        <WaterRibbon
          parentWidth={windowWidth}
          baseY={Math.max(180, Math.floor(windowHeight * 0.42))}
          streamMode={streamMode}
        />
        <IconRail
          active={activeNav}
          onSelect={setActiveNav}
          onOpenSettings={() => setSettingsOpen(true)}
          projectOpen={projectOpen}
          onGoHome={projectOpen ? handleCloseProject : undefined}
        />
        {projectOpen && !hasActiveProvider && (
          <NoProviderBanner onOpenSettings={() => setSettingsOpen(true)} />
        )}
        {updateAvailable && (
          <UpdateToast
            update={updateAvailable}
            applied={updateApplied}
            onApply={() => {
              setUpdateApplied(true);
              void applyUpdateInBackground(updateAvailable);
            }}
            onDismiss={() => setUpdateAvailable(null)}
          />
        )}
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
                  // Transparent so the App-level stream flows through.
                  // The narrow streamMode mask keeps the visible band
                  // tucked against the centered text.
                  background: "transparent",
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

/**
 * Tiny glass toast in the bottom-right corner. Surfaces an
 * auto-update notification without nagging — one click to apply +
 * notice it's installed (will activate on next launch), one click to
 * dismiss until next boot.
 */
function UpdateToast({
  update,
  applied,
  onApply,
  onDismiss,
}: {
  update: Update;
  applied: boolean;
  onApply: () => void;
  onDismiss: () => void;
}) {
  return (
    <div
      data-testid="update-toast"
      role="status"
      style={{
        position: "fixed",
        bottom: 16,
        right: 16,
        zIndex: 25,
        display: "flex",
        flexDirection: "column",
        alignItems: "flex-start",
        gap: 6,
        padding: "10px 14px",
        maxWidth: 320,
        borderRadius: "var(--water-r-12, 12px)",
        background:
          "color-mix(in srgb, var(--water-bg-paper) 78%, transparent)",
        backdropFilter: "blur(18px) saturate(160%)",
        WebkitBackdropFilter: "blur(18px) saturate(160%)",
        border:
          "1px solid color-mix(in srgb, var(--water-hairline) 55%, transparent)",
        boxShadow: "var(--water-elev-2)",
        fontFamily: "var(--water-font-sans)",
        fontSize: "var(--water-fs-meta)",
        color: "var(--water-fg-default)",
        animation:
          "water-fade-in var(--water-dur-medium) var(--water-ease-out-soft) both",
      }}
    >
      <span style={{ fontWeight: 600 }}>Water {update.version} is available.</span>
      <span style={{ color: "var(--water-fg-muted)", lineHeight: 1.4 }}>
        {applied
          ? "Update downloaded. Restart Water when you have a moment to finish installing."
          : "Apply now to install on next launch."}
      </span>
      <div style={{ display: "flex", gap: 6, marginTop: 4 }}>
        {!applied && (
          <button
            type="button"
            onClick={onApply}
            style={{
              padding: "4px 10px",
              border: "none",
              borderRadius: "var(--water-r-8)",
              background:
                "color-mix(in srgb, var(--water-hue-flow) 24%, transparent)",
              color: "var(--water-fg-default)",
              cursor: "pointer",
              fontFamily: "var(--water-font-sans)",
              fontSize: "var(--water-fs-meta)",
              fontWeight: 500,
            }}
          >
            Apply
          </button>
        )}
        <button
          type="button"
          onClick={onDismiss}
          style={{
            padding: "4px 10px",
            border:
              "1px solid color-mix(in srgb, var(--water-fg-faint) 30%, transparent)",
            borderRadius: "var(--water-r-8)",
            background: "transparent",
            color: "var(--water-fg-muted)",
            cursor: "pointer",
            fontFamily: "var(--water-font-sans)",
            fontSize: "var(--water-fs-meta)",
          }}
        >
          {applied ? "Got it" : "Later"}
        </button>
      </div>
    </div>
  );
}

/**
 * Slim glass banner anchored to the top of the project area. Renders
 * only when a project is open AND no provider has been Tested green.
 *
 * Without this hint, a tester writes a paragraph or two expecting
 * pills, sees nothing happen, and has no way to learn that a provider
 * needs to be set up first — the orchestrator's "no LlmRouter
 * configured; skipping pill dispatch" warning only shows in stderr.
 */
function NoProviderBanner({ onOpenSettings }: { onOpenSettings: () => void }) {
  return (
    <div
      data-testid="no-provider-banner"
      role="status"
      style={{
        position: "fixed",
        top: 14,
        left: "50%",
        transform: "translateX(-50%)",
        zIndex: 30,
        display: "flex",
        alignItems: "center",
        gap: 12,
        padding: "8px 14px",
        borderRadius: "var(--water-r-16)",
        background:
          "color-mix(in srgb, var(--water-bg-paper) 78%, transparent)",
        backdropFilter: "blur(18px) saturate(160%)",
        WebkitBackdropFilter: "blur(18px) saturate(160%)",
        border:
          "1px solid color-mix(in srgb, var(--water-hairline) 55%, transparent)",
        boxShadow: "var(--water-elev-1)",
        fontFamily: "var(--water-font-sans)",
        fontSize: "var(--water-fs-meta)",
        color: "var(--water-fg-default)",
        animation:
          "water-fade-in var(--water-dur-medium) var(--water-ease-out-soft) both",
      }}
    >
      <span
        aria-hidden
        style={{
          width: 8,
          height: 8,
          borderRadius: "50%",
          background:
            "color-mix(in srgb, var(--water-hue-drift) 70%, transparent)",
          boxShadow:
            "0 0 8px color-mix(in srgb, var(--water-hue-drift) 60%, transparent)",
        }}
      />
      <span>Set up a provider to enable nudges.</span>
      <button
        type="button"
        onClick={onOpenSettings}
        style={{
          padding: "4px 10px",
          border: "none",
          borderRadius: "var(--water-r-8)",
          background:
            "color-mix(in srgb, var(--water-hue-flow) 22%, transparent)",
          color: "var(--water-fg-default)",
          cursor: "pointer",
          fontFamily: "var(--water-font-sans)",
          fontSize: "var(--water-fs-meta)",
          fontWeight: 500,
        }}
      >
        Open Settings
      </button>
    </div>
  );
}
