import { useState } from "react";
import { ThemeProvider } from "./theme/ThemeProvider";
import { useTheme } from "./theme/useTheme";
import { SceneList } from "./pages/SceneList";
import { Diagnostics } from "./pages/Diagnostics";
import { ipc } from "./ipc/commands";

type Tab = "scenes" | "diagnostics";

function ThemeToggle() {
  const { theme, effective, setTheme } = useTheme();
  return (
    <span style={{ fontSize: "var(--water-fs-meta)", color: "var(--water-fg-faint)" }}>
      {theme} ({effective}){" "}
      <button onClick={() => setTheme("light")}>L</button>
      <button onClick={() => setTheme("dark")}>D</button>
      <button onClick={() => setTheme("auto")}>A</button>
    </span>
  );
}

function ProjectBar({ onOpened }: { onOpened: () => void }) {
  const [name, setName] = useState("Test Project");
  const [parent, setParent] = useState(".");
  const [root, setRoot] = useState("");
  return (
    <div style={{ display: "flex", gap: 8, alignItems: "center", padding: "8px 16px",
                  background: "var(--water-bg-canvas)" }}>
      <input value={parent} onChange={(e) => setParent(e.target.value)} placeholder="parent dir" />
      <input value={name} onChange={(e) => setName(e.target.value)} placeholder="project name" />
      <button onClick={async () => { await ipc.createProject(parent, name); onOpened(); }}>
        create
      </button>
      <input value={root} onChange={(e) => setRoot(e.target.value)} placeholder="project root to open" />
      <button onClick={async () => { await ipc.openProject(root); onOpened(); }}>open</button>
      <button onClick={async () => { await ipc.closeProject(); onOpened(); }}>close</button>
    </div>
  );
}

export default function App() {
  const [tab, setTab] = useState<Tab>("scenes");
  const [reloadKey, setReloadKey] = useState(0);

  return (
    <ThemeProvider>
      <header style={{ display: "flex", justifyContent: "space-between",
                       alignItems: "center", padding: "8px 16px" }}>
        <strong>Water</strong>
        <nav style={{ display: "flex", gap: 8 }}>
          <button onClick={() => setTab("scenes")}>scenes</button>
          <button onClick={() => setTab("diagnostics")}>diagnostics</button>
        </nav>
        <ThemeToggle />
      </header>
      <ProjectBar onOpened={() => setReloadKey((k) => k + 1)} />
      {tab === "scenes" ? <SceneList key={reloadKey} /> : <Diagnostics />}
    </ThemeProvider>
  );
}
