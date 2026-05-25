import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./styles/fonts.css";
import "./styles/tokens.css";
import "./styles/editor.css";
import "./styles/worlds.css";
import "./styles/characters.css";
import "./styles/sheets.css";
import "./styles/intake.css";
// alpha.4 diagnostic — imported last so its !important rules win.
import "./styles/perf-diagnostic.css";
import { loadAndApplyFont } from "./theme/fonts";
import { initSentry } from "./boot";

// Apply the writer's saved manuscript serif (or the default) before
// the first React render so the editor never flashes with the wrong
// face. Safe no-op in SSR / test environments.
loadAndApplyFont();

// Sentry: only initializes when VITE_SENTRY_DSN is set at build
// time. Dev builds without the var are completely offline.
initSentry();

const rootEl = document.getElementById("root");
if (!rootEl) throw new Error("#root element not found");

// Dev-only editor bake-off harnesses. Production builds skip the dynamic
// import entirely thanks to `import.meta.env.DEV` being statically false.
async function maybeMountBakeoff(): Promise<boolean> {
  if (!import.meta.env.DEV) return false;
  const params = new URLSearchParams(window.location.search);
  const which = params.get("bakeoff");
  if (which === "pm") {
    const mod = await import("./editor-bakeoff-pm/index");
    createRoot(rootEl!).render(
      <StrictMode>
        <mod.default />
      </StrictMode>,
    );
    return true;
  }
  return false;
}

void maybeMountBakeoff().then((mounted) => {
  if (mounted) return;
  createRoot(rootEl!).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
});
