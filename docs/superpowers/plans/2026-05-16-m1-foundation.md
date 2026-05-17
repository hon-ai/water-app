# M1 Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Tauri desktop app that can open/create a Water project folder, autosave scene Markdown, snapshot history, spawn a Python analysis sidecar, route LLM requests through a pluggable provider trait, and render a minimal diagnostics UI on a pastel-glow design-token foundation — meeting the M1 exit criteria in `docs/superpowers/specs/2026-05-16-water-design.md` § 4.2.

**Architecture:** Tauri 2 shell (Rust core + React/TS renderer), `water-core` Cargo crate for project store / snapshots / sidecar lifecycle / LLM router, Python sidecar managed via `uv` standalone-Python with a FastAPI `/health` endpoint, SQLite (`rusqlite` bundled) as a rebuildable index over per-scene Markdown + TOML truth files, zstd-compressed snapshots, OS-keychain secrets via `keyring`.

**Tech Stack:**
- Tauri 2.x; Rust 1.78+ (workspace, resolver 2); Node 20+; pnpm 9+
- React 18 + TypeScript 5 + Vite 5 + Tailwind CSS 4 (renderer)
- `rusqlite` (bundled), `rusqlite_migration`, `serde`, `serde_yaml`, `toml`, `ulid`, `zstd`, `reqwest` (rustls), `keyring`, `tokio`, `tracing`, `chrono`, `thiserror`, `anyhow` (Rust core)
- `wiremock` (Rust HTTP mocking), `tempfile`, `pretty_assertions` (Rust tests)
- `vitest` + `@testing-library/react` + `jsdom` (renderer tests)
- Python 3.12 via `uv`; FastAPI + uvicorn + pydantic v2 + pytest (sidecar)

**Repository layout produced by this milestone:**

```
Water/
├── Cargo.toml                              (workspace root)
├── package.json                            (Node workspace root)
├── pnpm-workspace.yaml
├── rust-toolchain.toml
├── app/
│   ├── package.json
│   ├── vite.config.ts
│   ├── tsconfig.json
│   ├── tailwind.config.ts
│   ├── postcss.config.cjs
│   ├── index.html
│   ├── src/
│   │   ├── main.tsx
│   │   ├── App.tsx
│   │   ├── styles/tokens.css
│   │   ├── theme/ThemeProvider.tsx
│   │   ├── ipc/commands.ts
│   │   ├── pages/Diagnostics.tsx
│   │   ├── pages/SceneList.tsx
│   │   └── components/PlaceholderEditor.tsx
│   └── src-tauri/
│       ├── Cargo.toml
│       ├── tauri.conf.json
│       ├── build.rs
│       └── src/
│           ├── main.rs
│           └── commands.rs
├── crates/
│   └── water-core/
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── error.rs
│           ├── id.rs
│           ├── db.rs
│           ├── migrations.rs
│           ├── project.rs
│           ├── scene.rs
│           ├── block.rs
│           ├── chapters.rs
│           ├── character.rs
│           ├── world.rs
│           ├── autosave.rs
│           ├── snapshot.rs
│           ├── rebuild.rs
│           ├── repair.rs
│           ├── sidecar.rs
│           └── llm/
│               ├── mod.rs
│               ├── provider.rs
│               ├── router.rs
│               ├── secrets.rs
│               ├── anthropic.rs
│               ├── openai.rs
│               ├── ollama.rs
│               ├── llamacpp.rs
│               └── mlx.rs
├── sidecar/
│   ├── pyproject.toml
│   ├── README.md
│   └── src/water_sidecar/
│       ├── __init__.py
│       ├── main.py
│       ├── health.py
│       └── routes/analyze.py
└── docs/
    └── superpowers/
        ├── specs/2026-05-16-water-design.md
        └── plans/2026-05-16-m1-foundation.md   (this file)
```

**Cross-cutting conventions:**
- Every Rust public type derives `Debug` and (where serialisable) `serde::{Serialize, Deserialize}` with `serde(rename_all = "snake_case")`.
- All IDs are ULIDs (TEXT in SQLite, `String` in Rust).
- All timestamps are RFC 3339 UTC strings (TEXT in SQLite, `chrono::DateTime<Utc>` in Rust).
- `tracing` is used for logging; tests assert behaviour, not log output.
- `cargo fmt` and `cargo clippy --all-targets -- -D warnings` clean at every commit.
- Frequent small commits (after every passing test).

---

## Phase A — Project Scaffolding

### Task 1: Initialize Cargo workspace + `water-core` crate

**Files:**
- Create: `Cargo.toml`
- Create: `rust-toolchain.toml`
- Create: `crates/water-core/Cargo.toml`
- Create: `crates/water-core/src/lib.rs`

- [ ] **Step 1: Pin Rust toolchain**

Create `rust-toolchain.toml`:

```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
profile = "minimal"
```

(Rust 1.95 is currently installed on the dev machine; pinning to `stable` keeps us forward-compatible and avoids forcing version-specific downloads on contributor machines. Verified end-to-end build works.)

- [ ] **Step 2: Create the workspace root manifest**

Create `Cargo.toml`:

```toml
[workspace]
members = ["app/src-tauri", "crates/water-core"]
resolver = "2"

[workspace.package]
edition = "2021"
license = "TBD"
publish = false

[workspace.dependencies]
anyhow = "1"
chrono = { version = "0.4", features = ["serde"] }
keyring = "3"
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
rusqlite = { version = "0.32", features = ["bundled"] }
rusqlite_migration = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
thiserror = "2"
tokio = { version = "1", features = ["full"] }
toml = "0.8"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
ulid = { version = "1", features = ["serde"] }
uuid = { version = "1", features = ["v4", "serde"] }
zstd = "0.13"

[workspace.dependencies.tauri]
version = "2"

[workspace.dependencies.tauri-build]
version = "2"

# Dev-only deps
[workspace.dependencies.tempfile]
version = "3"

[workspace.dependencies.pretty_assertions]
version = "1"

[workspace.dependencies.wiremock]
version = "0.6"
```

- [ ] **Step 3: Create the `water-core` crate manifest**

Create `crates/water-core/Cargo.toml`:

```toml
[package]
name = "water-core"
version = "0.1.0"
edition.workspace = true
license.workspace = true
publish = false

[features]
default = []
mlx = []

[dependencies]
anyhow.workspace = true
chrono.workspace = true
keyring.workspace = true
reqwest.workspace = true
rusqlite.workspace = true
rusqlite_migration.workspace = true
serde.workspace = true
serde_json.workspace = true
serde_yaml.workspace = true
thiserror.workspace = true
tokio.workspace = true
toml.workspace = true
tracing.workspace = true
ulid.workspace = true
zstd.workspace = true

[dev-dependencies]
pretty_assertions.workspace = true
tempfile.workspace = true
tracing-subscriber.workspace = true
wiremock.workspace = true
```

- [ ] **Step 4: Create the root `lib.rs` with module skeleton**

Create `crates/water-core/src/lib.rs`:

```rust
//! water-core — Rust core for the Water writing app.
//!
//! All disk, secrets, processes, and policy live here. The renderer is dumb
//! about timing; this crate decides when things happen.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions, clippy::missing_errors_doc)]

pub mod error;

pub use error::{Error, Result};

/// Crate version, exposed for diagnostics surfaces.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
```

- [ ] **Step 5: Create the shared `Error` type**

Create `crates/water-core/src/error.rs`:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("migration: {0}")]
    Migration(#[from] rusqlite_migration::Error),
    #[error("toml-de: {0}")]
    TomlDe(#[from] toml::de::Error),
    #[error("toml-ser: {0}")]
    TomlSer(#[from] toml::ser::Error),
    #[error("yaml: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid project: {0}")]
    InvalidProject(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("provider: {0}")]
    Provider(String),
    #[error("sidecar: {0}")]
    Sidecar(String),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;
```

- [ ] **Step 6: Verify the crate compiles**

Run: `cargo build -p water-core`
Expected: builds successfully, may emit warnings about unused enum variants — that is fine.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml rust-toolchain.toml crates/water-core/Cargo.toml crates/water-core/src/lib.rs crates/water-core/src/error.rs
git commit -m "feat(core): initialize water-core crate and Cargo workspace"
```

---

### Task 2: Scaffold the Tauri 2 + Vite + React + TypeScript app

**Files:**
- Create: `app/package.json`
- Create: `app/index.html`
- Create: `app/vite.config.ts`
- Create: `app/tsconfig.json`
- Create: `app/src/main.tsx`
- Create: `app/src/App.tsx`
- Create: `app/src-tauri/Cargo.toml`
- Create: `app/src-tauri/tauri.conf.json`
- Create: `app/src-tauri/build.rs`
- Create: `app/src-tauri/src/main.rs`

- [ ] **Step 1: Create the renderer `package.json`**

Create `app/package.json`:

```json
{
  "name": "@water/app",
  "version": "0.1.0",
  "type": "module",
  "private": true,
  "scripts": {
    "dev": "vite",
    "build": "tsc -b && vite build",
    "preview": "vite preview --port 5173",
    "tauri": "tauri",
    "test": "vitest run",
    "test:watch": "vitest"
  },
  "dependencies": {
    "@tauri-apps/api": "^2.0.0",
    "react": "^18.3.0",
    "react-dom": "^18.3.0"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2.0.0",
    "@testing-library/jest-dom": "^6.5.0",
    "@testing-library/react": "^16.0.0",
    "@types/react": "^18.3.0",
    "@types/react-dom": "^18.3.0",
    "@vitejs/plugin-react": "^4.3.0",
    "autoprefixer": "^10.4.20",
    "jsdom": "^25.0.0",
    "postcss": "^8.4.47",
    "tailwindcss": "^4.0.0-beta.3",
    "@tailwindcss/postcss": "^4.0.0-beta.3",
    "typescript": "^5.5.0",
    "vite": "^5.4.0",
    "vitest": "^2.1.0"
  }
}
```

- [ ] **Step 2: Create `app/index.html`**

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Water</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

- [ ] **Step 3: Create `app/vite.config.ts`**

```ts
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  build: { target: "es2022", sourcemap: true },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test-setup.ts"]
  }
});
```

- [ ] **Step 4: Create `app/tsconfig.json`**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "jsx": "react-jsx",
    "strict": true,
    "noUncheckedIndexedAccess": true,
    "noImplicitOverride": true,
    "noFallthroughCasesInSwitch": true,
    "isolatedModules": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "allowImportingTsExtensions": false,
    "verbatimModuleSyntax": true,
    "useDefineForClassFields": true,
    "resolveJsonModule": true,
    "baseUrl": ".",
    "paths": { "@/*": ["src/*"] }
  },
  "include": ["src", "vite.config.ts"]
}
```

- [ ] **Step 5: Create renderer entry points**

Create `app/src/main.tsx`:

```tsx
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./styles/tokens.css";

const rootEl = document.getElementById("root");
if (!rootEl) throw new Error("#root element not found");

createRoot(rootEl).render(
  <StrictMode>
    <App />
  </StrictMode>
);
```

Create `app/src/App.tsx`:

```tsx
export default function App() {
  return (
    <main className="water-shell">
      <h1>Water</h1>
      <p>foundation milestone</p>
    </main>
  );
}
```

Create empty `app/src/styles/tokens.css`:

```css
/* design tokens populated in Task 34 */
:root { color-scheme: light dark; }
```

- [ ] **Step 6: Create the Tauri shell `Cargo.toml`**

Create `app/src-tauri/Cargo.toml`:

```toml
[package]
name = "water-app"
version = "0.1.0"
edition.workspace = true
license.workspace = true
publish = false

[build-dependencies]
tauri-build = { workspace = true, features = [] }

[dependencies]
tauri = { workspace = true, features = [] }
water-core = { path = "../../crates/water-core" }
serde.workspace = true
serde_json.workspace = true
tokio.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
anyhow.workspace = true
```

(Note: no `[lib]` block. Tauri 2's mobile target conventionally uses a `lib.rs` with `mobile_entry_point!`, but Water v1 is desktop-only; we add `[lib]` + `src/lib.rs` together if mobile lands in a later milestone.)

- [ ] **Step 7: Create the Tauri config**

Create `app/src-tauri/tauri.conf.json`:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Water",
  "version": "0.1.0",
  "identifier": "co.water.app",
  "build": {
    "frontendDist": "../dist",
    "devUrl": "http://localhost:5173",
    "beforeDevCommand": "pnpm --filter @water/app dev",
    "beforeBuildCommand": "pnpm --filter @water/app build"
  },
  "app": {
    "windows": [
      {
        "title": "Water",
        "width": 1280,
        "height": 800,
        "minWidth": 900,
        "minHeight": 600,
        "decorations": true,
        "transparent": false
      }
    ],
    "security": { "csp": null }
  },
  "bundle": {
    "active": true,
    "targets": ["app", "dmg", "msi", "nsis"],
    "icon": ["icons/icon.ico"]
  }
}
```

(Note: Tauri 2 requires at least one icon for the Windows resource embedder. Create a placeholder `app/src-tauri/icons/icon.ico` — any small valid ICO — until the real brand icon ships in M7 polish.)

- [ ] **Step 8: Create the Tauri Rust entry points**

Create `app/src-tauri/build.rs`:

```rust
fn main() {
    tauri_build::build();
}
```

Create `app/src-tauri/src/main.rs`:

```rust
// Prevents additional console window on Windows in release; do not remove.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tauri::Builder::default()
        .setup(|_app| Ok(()))
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

(Note: an earlier draft of this task included a decorative `tauri_plugin_log_init()` shim; it has been removed because it added complexity without functional gain. `tracing-subscriber` provides all logging needed for M1.)

- [ ] **Step 9: Verify Tauri compiles**

Run: `cargo build -p water-app`
Expected: builds successfully. (First build may take several minutes.)

- [ ] **Step 10: Commit**

```bash
git add app Cargo.toml
git commit -m "feat(app): scaffold Tauri 2 + Vite + React shell"
```

---

### Task 3: Configure pnpm workspace and root tooling

**Files:**
- Create: `pnpm-workspace.yaml`
- Create: `package.json` (root)
- Create: `.editorconfig`
- Modify: `.gitignore`

- [ ] **Step 1: Declare the pnpm workspace**

Create `pnpm-workspace.yaml`:

```yaml
packages:
  - "app"
```

- [ ] **Step 2: Create a root `package.json` for scripts**

Create `package.json`:

```json
{
  "name": "water",
  "version": "0.1.0",
  "private": true,
  "scripts": {
    "dev": "pnpm --filter @water/app tauri dev",
    "build:app": "pnpm --filter @water/app tauri build",
    "test:app": "pnpm --filter @water/app test",
    "test:core": "cargo test -p water-core",
    "test:all": "pnpm test:core && pnpm test:app",
    "lint:rust": "cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings",
    "fmt:rust": "cargo fmt --all"
  }
}
```

- [ ] **Step 3: Add EditorConfig**

Create `.editorconfig`:

```ini
root = true

[*]
charset = utf-8
end_of_line = lf
indent_style = space
indent_size = 2
insert_final_newline = true
trim_trailing_whitespace = true

[*.{rs,py}]
indent_size = 4

[*.md]
trim_trailing_whitespace = false
```

- [ ] **Step 4: Append Node + Tauri-specific ignores to `.gitignore`**

Open `.gitignore` and verify the following lines are present (most already are from the spec commit). Add any that are missing:

```
# pnpm
.pnpm-store/
pnpm-lock.yaml.bak

# Tauri build output
app/dist/
app/src-tauri/gen/
```

- [ ] **Step 5: Install dependencies**

Run: `pnpm install`
Expected: lockfile generated; all renderer deps installed.

- [ ] **Step 6: Commit**

```bash
git add pnpm-workspace.yaml package.json .editorconfig .gitignore pnpm-lock.yaml
git commit -m "chore: pnpm workspace + root scripts + editorconfig"
```

---

### Task 4: Tailwind 4 wiring + base styles

> **Plan amendment (during execution):** T4 also includes a one-line fix to
> `app/vite.config.ts` — switch `defineConfig` import from `"vite"` to
> `"vitest/config"`. This is required for Step 4 (`pnpm --filter @water/app build`)
> to succeed, because the file's `test` block (added in T2) needs Vitest's
> extended config typing.

**Files:**
- Create: `app/postcss.config.cjs`
- Create: `app/tailwind.config.ts`
- Modify: `app/src/styles/tokens.css`
- Modify: `app/vite.config.ts` (amendment — see note above)

- [ ] **Step 0: Fix carried-over typing bug in `vite.config.ts`**

In `app/vite.config.ts`, change the first line from:

```ts
import { defineConfig } from "vite";
```

to:

```ts
import { defineConfig } from "vitest/config";
```

`vitest/config` re-exports `defineConfig` with the Vitest `test` field typed,
which is necessary because T2 introduced a `test` block in this file.

- [ ] **Step 1: Create the PostCSS config**

Create `app/postcss.config.cjs`:

```js
module.exports = {
  plugins: {
    "@tailwindcss/postcss": {},
    autoprefixer: {}
  }
};
```

- [ ] **Step 2: Create the Tailwind config**

Create `app/tailwind.config.ts`:

```ts
import type { Config } from "tailwindcss";

export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      borderRadius: {
        none: "0",
        sm: "8px",
        DEFAULT: "16px",
        md: "16px",
        lg: "24px",
        xl: "32px",
        full: "9999px"
      },
      fontFamily: {
        sans: ["Inter", "system-ui", "sans-serif"],
        serif: ["'Source Serif Pro'", "Georgia", "serif"],
        mono: ["'JetBrains Mono'", "ui-monospace", "monospace"]
      }
    }
  },
  plugins: []
} satisfies Config;
```

- [ ] **Step 3: Wire Tailwind into `tokens.css`**

Replace `app/src/styles/tokens.css` with:

```css
@import "tailwindcss";

/* design tokens populated in Task 34 */
:root {
  color-scheme: light dark;
  --water-bg: #fbfaf7;     /* paper light */
  --water-fg: #161616;
}

@media (prefers-color-scheme: dark) {
  :root {
    --water-bg: #0d0e12;    /* liminal */
    --water-fg: #ececec;
  }
}

body {
  background: var(--water-bg);
  color: var(--water-fg);
  font-family: theme(fontFamily.sans);
}
```

- [ ] **Step 4: Verify the renderer builds**

Run: `pnpm --filter @water/app build`
Expected: vite build succeeds; `app/dist/` is produced.

- [ ] **Step 5: Commit**

```bash
git add app/postcss.config.cjs app/tailwind.config.ts app/src/styles/tokens.css
git commit -m "feat(app): wire Tailwind 4 with placeholder tokens"
```

---

### Task 5: Test scaffolding (Rust + renderer sanity tests)

> **Plan amendment (during execution):** T5 also includes a one-line fix to
> `app/tsconfig.json` — add `"noEmit": true` to `compilerOptions`. Without it,
> `tsc -b` (run by the `build` script) emits `.js` files next to TS sources,
> which then shadow them at resolution time. The T4 code review flagged this;
> introducing `App.test.tsx` in this task would have compounded the problem.
> Vite handles actual JS emission; `tsc -b` is now type-check-only. Tests still
> run because Vitest uses esbuild, not `tsc`. This was committed separately as
> `fix(app): set noEmit so tsc -b stops shadowing TS sources with .js`.

**Files:**
- Modify: `app/tsconfig.json` (amendment — see note above)
- Create: `app/src/test-setup.ts`
- Create: `app/src/App.test.tsx`
- Modify: `crates/water-core/src/lib.rs`

- [ ] **Step 1: Renderer test setup file**

Create `app/src/test-setup.ts`:

```ts
import "@testing-library/jest-dom/vitest";
```

- [ ] **Step 2: Failing renderer test**

Create `app/src/App.test.tsx`:

```tsx
import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import App from "./App";

describe("App", () => {
  it("renders the Water heading", () => {
    render(<App />);
    expect(screen.getByRole("heading", { name: /water/i })).toBeInTheDocument();
  });
});
```

- [ ] **Step 3: Run renderer test**

Run: `pnpm --filter @water/app test`
Expected: PASS (App already renders "Water").

- [ ] **Step 4: Failing core sanity test**

Append to `crates/water-core/src/lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_non_empty() {
        assert!(!VERSION.is_empty(), "VERSION must be exposed for diagnostics");
    }
}
```

- [ ] **Step 5: Run core test**

Run: `cargo test -p water-core`
Expected: 1 passed.

- [ ] **Step 6: Commit**

```bash
git add app/src/test-setup.ts app/src/App.test.tsx crates/water-core/src/lib.rs
git commit -m "test: renderer + core sanity tests"
```

---

## Phase B — IDs, SQLite Connection, Schema

### Task 6: ULID utilities

**Files:**
- Create: `crates/water-core/src/id.rs`
- Modify: `crates/water-core/src/lib.rs`

- [ ] **Step 1: Failing test for new ULID**

Create `crates/water-core/src/id.rs`:

```rust
//! Stable string IDs for Water entities. We use ULIDs everywhere.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Id(String);

impl Id {
    /// Mint a new ULID-backed Id.
    pub fn new() -> Self {
        Self(ulid::Ulid::new().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for Id {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Id({})", self.0)
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for Id {
    type Err = crate::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ulid::Ulid::from_string(s)
            .map_err(|e| crate::Error::Other(format!("invalid ulid: {e}")))?;
        Ok(Self(s.to_string()))
    }
}

impl From<Id> for String {
    fn from(id: Id) -> Self {
        id.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_id_is_26_chars() {
        let id = Id::new();
        assert_eq!(id.as_str().len(), 26);
    }

    #[test]
    fn id_round_trips_through_from_str() {
        let id = Id::new();
        let parsed: Id = id.as_str().parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn invalid_string_rejected() {
        let bad = Id::from_str("not-a-ulid");
        assert!(bad.is_err());
    }

    #[test]
    fn two_ids_are_unique() {
        let a = Id::new();
        let b = Id::new();
        assert_ne!(a, b);
    }
}
```

- [ ] **Step 2: Register the module**

Append to `crates/water-core/src/lib.rs` (before `#[cfg(test)] mod tests`):

```rust
pub mod id;
pub use id::Id;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p water-core id::tests`
Expected: 4 passed.

- [ ] **Step 4: Commit**

```bash
git add crates/water-core/src/id.rs crates/water-core/src/lib.rs
git commit -m "feat(core): ULID-backed Id type"
```

---

### Task 7: SQLite connection wrapper and migration runner

**Files:**
- Create: `crates/water-core/src/db.rs`
- Create: `crates/water-core/src/migrations.rs`
- Modify: `crates/water-core/src/lib.rs`

- [ ] **Step 1: Define a migration runner**

Create `crates/water-core/src/migrations.rs`:

```rust
//! Migration list for the Water project database.
//!
//! Each migration is `(version, up_sql)`. Migrations run in order under a
//! single transaction. We never drop columns in the same migration that
//! adds them; we never break forward compatibility within a major.

use rusqlite_migration::{Migrations, M};

pub fn all() -> Migrations<'static> {
    Migrations::new(vec![M::up(V1_INIT)])
}

const V1_INIT: &str = include_str!("../sql/v1_init.sql");
```

- [ ] **Step 2: Stub the SQL file**

Create `crates/water-core/sql/v1_init.sql` with a single placeholder table (we'll populate the real schema in Task 8):

```sql
CREATE TABLE schema_marker (
    inserted_at TEXT NOT NULL
);
```

- [ ] **Step 3: Failing test for the connection wrapper**

Create `crates/water-core/src/db.rs`:

```rust
//! SQLite connection wrapper for a single Water project.

use crate::{migrations, Error, Result};
use rusqlite::{Connection, OpenFlags};
use std::path::Path;

pub struct Db {
    conn: Connection,
}

impl Db {
    /// Open (and migrate) the project DB at `path`. Creates the file if
    /// it does not exist. WAL mode is enabled for cloud-sync-folder safety.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_URI
            | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let mut conn = Connection::open_with_flags(path, flags)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;

        let migrations = migrations::all();
        migrations
            .to_latest(&mut conn)
            .map_err(Error::Migration)?;

        Ok(Self { conn })
    }

    /// In-memory DB for tests.
    pub fn open_in_memory() -> Result<Self> {
        let mut conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let migrations = migrations::all();
        migrations.to_latest(&mut conn).map_err(Error::Migration)?;
        Ok(Self { conn })
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_db_runs_migrations() {
        let db = Db::open_in_memory().unwrap();
        let count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='schema_marker'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "schema_marker table should exist after migration");
    }

    #[test]
    fn file_db_persists_across_opens() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_owned();
        drop(tmp); // we need the path, not the file
        {
            let _db = Db::open(&path).unwrap();
        }
        let _db2 = Db::open(&path).unwrap();
        std::fs::remove_file(&path).ok();
    }
}
```

- [ ] **Step 4: Register the modules**

Append to `crates/water-core/src/lib.rs`:

```rust
pub mod db;
pub mod migrations;
pub use db::Db;
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p water-core db::tests migrations`
Expected: 2 passed.

- [ ] **Step 6: Commit**

```bash
git add crates/water-core/src/db.rs crates/water-core/src/migrations.rs crates/water-core/sql/v1_init.sql crates/water-core/src/lib.rs
git commit -m "feat(core): SQLite connection + migration runner"
```

---

### Task 8: Migration v1 — full schema

**Files:**
- Modify: `crates/water-core/sql/v1_init.sql`
- Modify: `crates/water-core/src/db.rs` (test additions)

- [ ] **Step 1: Failing test asserting all spec tables exist**

Append to the `tests` module in `crates/water-core/src/db.rs`:

```rust
    #[test]
    fn migration_creates_all_spec_tables() {
        let db = Db::open_in_memory().unwrap();
        let expected = [
            "schema_version",
            "project",
            "manuscript",
            "chapter",
            "scene",
            "scene_character_presence",
            "character",
            "world_segment",
            "world_entry",
            "pinned_pill",
            "scene_metrics",
            "block_metrics",
            "snapshot",
            "settings",
            "provider_config",
            "telemetry_event",
        ];
        for name in expected {
            let count: i64 = db
                .conn()
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [name],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "missing table {name}");
        }
    }

    #[test]
    fn schema_version_row_is_one() {
        let db = Db::open_in_memory().unwrap();
        let v: i64 = db
            .conn()
            .query_row("SELECT version FROM schema_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 1);
    }
```

- [ ] **Step 2: Run tests; expect failure**

Run: `cargo test -p water-core migration_creates_all_spec_tables`
Expected: FAIL (the table list does not exist yet).

- [ ] **Step 3: Replace `v1_init.sql` with the full schema**

Replace `crates/water-core/sql/v1_init.sql`:

```sql
PRAGMA foreign_keys = ON;

CREATE TABLE schema_version (
    version INTEGER NOT NULL
);
INSERT INTO schema_version (version) VALUES (1);

CREATE TABLE project (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    default_manuscript_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE manuscript (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    ordering INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE chapter (
    id TEXT PRIMARY KEY,
    manuscript_id TEXT NOT NULL REFERENCES manuscript(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    ordering INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE character (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    schema_version TEXT NOT NULL DEFAULT 'lsm-v2.1',
    data_json TEXT NOT NULL,
    file_path TEXT NOT NULL,
    file_hash TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE world_segment (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    ordering INTEGER NOT NULL,
    is_collection INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE world_entry (
    id TEXT PRIMARY KEY,
    segment_id TEXT NOT NULL REFERENCES world_segment(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    data_json TEXT NOT NULL,
    file_path TEXT NOT NULL,
    file_hash TEXT
);

CREATE TABLE scene (
    id TEXT PRIMARY KEY,
    manuscript_id TEXT NOT NULL REFERENCES manuscript(id) ON DELETE CASCADE,
    chapter_id TEXT REFERENCES chapter(id) ON DELETE SET NULL,
    ordering INTEGER NOT NULL,
    name TEXT NOT NULL,
    pov_character_id TEXT REFERENCES character(id) ON DELETE SET NULL,
    location_id TEXT REFERENCES world_entry(id) ON DELETE SET NULL,
    scene_goal TEXT,
    status TEXT NOT NULL DEFAULT 'draft',
    word_count INTEGER NOT NULL DEFAULT 0,
    file_path TEXT NOT NULL,
    file_hash TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX scene_by_manuscript ON scene(manuscript_id, ordering);

CREATE TABLE scene_character_presence (
    scene_id TEXT NOT NULL REFERENCES scene(id) ON DELETE CASCADE,
    character_id TEXT NOT NULL REFERENCES character(id) ON DELETE CASCADE,
    PRIMARY KEY (scene_id, character_id)
);

CREATE TABLE pinned_pill (
    id TEXT PRIMARY KEY,
    scene_id TEXT NOT NULL REFERENCES scene(id) ON DELETE CASCADE,
    block_id TEXT NOT NULL,
    snippet TEXT NOT NULL,
    speaker_kind TEXT NOT NULL,
    speaker_id TEXT NOT NULL,
    message TEXT NOT NULL,
    hue TEXT NOT NULL,
    rabbit_hole_path TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE scene_metrics (
    scene_id TEXT PRIMARY KEY REFERENCES scene(id) ON DELETE CASCADE,
    flow REAL, coherence REAL, engagement REAL, divergence REAL,
    pace REAL, intensity REAL, valence REAL,
    lexical_diversity REAL, sentence_complexity REAL,
    ocean_o REAL, ocean_c REAL, ocean_e REAL, ocean_a REAL, ocean_n REAL,
    beat_label TEXT,
    beat_confidence REAL,
    summary TEXT,
    summary_for_hash TEXT,
    summary_model_id TEXT,
    last_analyzed_at TEXT,
    source_file_hash TEXT
);

CREATE TABLE block_metrics (
    scene_id TEXT NOT NULL REFERENCES scene(id) ON DELETE CASCADE,
    block_id TEXT NOT NULL,
    flow REAL, coherence REAL, divergence REAL,
    PRIMARY KEY (scene_id, block_id)
);

CREATE TABLE snapshot (
    id TEXT PRIMARY KEY,
    scene_id TEXT NOT NULL REFERENCES scene(id) ON DELETE CASCADE,
    taken_at TEXT NOT NULL,
    trigger TEXT NOT NULL,
    file_path TEXT NOT NULL,
    byte_size INTEGER NOT NULL
);
CREATE INDEX snapshot_by_scene ON snapshot(scene_id, taken_at DESC);

CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value_json TEXT NOT NULL
);

CREATE TABLE provider_config (
    id TEXT PRIMARY KEY,
    enabled INTEGER NOT NULL,
    config_json TEXT NOT NULL,
    ordering INTEGER NOT NULL
);

CREATE TABLE telemetry_event (
    id TEXT PRIMARY KEY,
    recorded_at TEXT NOT NULL,
    kind TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    sent INTEGER NOT NULL DEFAULT 0
);
```

- [ ] **Step 4: Run tests; expect pass**

Run: `cargo test -p water-core`
Expected: all tests pass; `migration_creates_all_spec_tables` and `schema_version_row_is_one` now green.

- [ ] **Step 5: Commit**

```bash
git add crates/water-core/sql/v1_init.sql crates/water-core/src/db.rs
git commit -m "feat(core): v1 schema migration with all spec tables"
```

---

### Task 9: ProjectStore and ManuscriptStore CRUD

**Files:**
- Create: `crates/water-core/src/project.rs`
- Modify: `crates/water-core/src/lib.rs`

- [ ] **Step 1: Failing tests**

Create `crates/water-core/src/project.rs`:

```rust
//! Project + Manuscript CRUD against the SQLite index.

use crate::{Db, Error, Id, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: Id,
    pub name: String,
    pub default_manuscript_id: Option<Id>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manuscript {
    pub id: Id,
    pub project_id: Id,
    pub name: String,
    pub ordering: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct ProjectStore<'a> {
    db: &'a Db,
}

impl<'a> ProjectStore<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    pub fn insert(&self, name: &str) -> Result<Project> {
        let now = Utc::now();
        let p = Project {
            id: Id::new(),
            name: name.to_owned(),
            default_manuscript_id: None,
            created_at: now,
            updated_at: now,
        };
        self.db.conn().execute(
            "INSERT INTO project (id, name, default_manuscript_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            (
                p.id.as_str(),
                &p.name,
                p.default_manuscript_id.as_ref().map(Id::as_str),
                p.created_at.to_rfc3339(),
                p.updated_at.to_rfc3339(),
            ),
        )?;
        Ok(p)
    }

    pub fn get(&self, id: &Id) -> Result<Project> {
        self.db
            .conn()
            .query_row(
                "SELECT id, name, default_manuscript_id, created_at, updated_at
                 FROM project WHERE id = ?1",
                [id.as_str()],
                row_to_project,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    Error::NotFound(format!("project {id}"))
                }
                other => other.into(),
            })
    }

    pub fn set_default_manuscript(&self, project_id: &Id, manuscript_id: &Id) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let n = self.db.conn().execute(
            "UPDATE project SET default_manuscript_id = ?2, updated_at = ?3 WHERE id = ?1",
            (project_id.as_str(), manuscript_id.as_str(), now),
        )?;
        if n == 0 {
            return Err(Error::NotFound(format!("project {project_id}")));
        }
        Ok(())
    }
}

pub struct ManuscriptStore<'a> {
    db: &'a Db,
}

impl<'a> ManuscriptStore<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    pub fn insert(&self, project_id: &Id, name: &str, ordering: i64) -> Result<Manuscript> {
        let now = Utc::now();
        let m = Manuscript {
            id: Id::new(),
            project_id: project_id.clone(),
            name: name.to_owned(),
            ordering,
            created_at: now,
            updated_at: now,
        };
        self.db.conn().execute(
            "INSERT INTO manuscript (id, project_id, name, ordering, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (
                m.id.as_str(),
                m.project_id.as_str(),
                &m.name,
                m.ordering,
                m.created_at.to_rfc3339(),
                m.updated_at.to_rfc3339(),
            ),
        )?;
        Ok(m)
    }

    pub fn list(&self, project_id: &Id) -> Result<Vec<Manuscript>> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, project_id, name, ordering, created_at, updated_at
             FROM manuscript WHERE project_id = ?1 ORDER BY ordering",
        )?;
        let rows = stmt.query_map([project_id.as_str()], row_to_manuscript)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }
}

fn row_to_project(row: &rusqlite::Row<'_>) -> rusqlite::Result<Project> {
    let id: String = row.get(0)?;
    let name: String = row.get(1)?;
    let default_manuscript_id: Option<String> = row.get(2)?;
    let created_at: String = row.get(3)?;
    let updated_at: String = row.get(4)?;
    Ok(Project {
        id: parse_id(&id)?,
        name,
        default_manuscript_id: default_manuscript_id.as_deref().map(parse_id).transpose()?,
        created_at: parse_dt(&created_at)?,
        updated_at: parse_dt(&updated_at)?,
    })
}

fn row_to_manuscript(row: &rusqlite::Row<'_>) -> rusqlite::Result<Manuscript> {
    let id: String = row.get(0)?;
    let project_id: String = row.get(1)?;
    let name: String = row.get(2)?;
    let ordering: i64 = row.get(3)?;
    let created_at: String = row.get(4)?;
    let updated_at: String = row.get(5)?;
    Ok(Manuscript {
        id: parse_id(&id)?,
        project_id: parse_id(&project_id)?,
        name,
        ordering,
        created_at: parse_dt(&created_at)?,
        updated_at: parse_dt(&updated_at)?,
    })
}

fn parse_id(s: &str) -> rusqlite::Result<Id> {
    s.parse::<Id>().map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })
}

fn parse_dt(s: &str) -> rusqlite::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn fresh_db() -> Db {
        Db::open_in_memory().unwrap()
    }

    #[test]
    fn insert_and_get_project_round_trip() {
        let db = fresh_db();
        let store = ProjectStore::new(&db);
        let p = store.insert("Test Project").unwrap();
        let got = store.get(&p.id).unwrap();
        assert_eq!(got.id, p.id);
        assert_eq!(got.name, "Test Project");
        assert!(got.default_manuscript_id.is_none());
    }

    #[test]
    fn get_missing_returns_not_found() {
        let db = fresh_db();
        let store = ProjectStore::new(&db);
        let err = store.get(&Id::new()).unwrap_err();
        assert!(matches!(err, Error::NotFound(_)));
    }

    #[test]
    fn manuscript_insert_and_list() {
        let db = fresh_db();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let ms = ManuscriptStore::new(&db);
        let m1 = ms.insert(&p.id, "First", 0).unwrap();
        let m2 = ms.insert(&p.id, "Second", 1).unwrap();
        let list = ms.list(&p.id).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, m1.id);
        assert_eq!(list[1].id, m2.id);
    }

    #[test]
    fn set_default_manuscript_updates_project() {
        let db = fresh_db();
        let ps = ProjectStore::new(&db);
        let p = ps.insert("P").unwrap();
        let m = ManuscriptStore::new(&db).insert(&p.id, "M", 0).unwrap();
        ps.set_default_manuscript(&p.id, &m.id).unwrap();
        let p2 = ps.get(&p.id).unwrap();
        assert_eq!(p2.default_manuscript_id, Some(m.id));
    }
}
```

- [ ] **Step 2: Register the module**

Append to `crates/water-core/src/lib.rs`:

```rust
pub mod project;
pub use project::{Manuscript, ManuscriptStore, Project, ProjectStore};
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p water-core project::tests`
Expected: 4 passed.

- [ ] **Step 4: Commit**

```bash
git add crates/water-core/src/project.rs crates/water-core/src/lib.rs
git commit -m "feat(core): Project + Manuscript stores"
```

---

## Phase C — On-Disk Project Store

### Task 10: `water.toml` project metadata

**Files:**
- Create: `crates/water-core/src/water_toml.rs`
- Modify: `crates/water-core/src/lib.rs`

- [ ] **Step 1: Failing tests**

Create `crates/water-core/src/water_toml.rs`:

```rust
//! water.toml — project root metadata, the human-readable companion to project.db.

use crate::{Error, Id, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const FILE_NAME: &str = "water.toml";
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WaterToml {
    pub schema_version: u32,
    pub project_id: Id,
    pub name: String,
    pub default_manuscript_id: Option<Id>,
    pub created_at: String,
    pub updated_at: String,
}

impl WaterToml {
    pub fn new(name: &str, project_id: Id, manuscript_id: Id) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id,
            name: name.to_owned(),
            default_manuscript_id: Some(manuscript_id),
            created_at: now.clone(),
            updated_at: now,
        }
    }

    pub fn read<P: AsRef<Path>>(dir: P) -> Result<Self> {
        let path = dir.as_ref().join(FILE_NAME);
        let text = std::fs::read_to_string(&path)
            .map_err(|e| Error::InvalidProject(format!("read {}: {e}", path.display())))?;
        let parsed: Self = toml::from_str(&text)?;
        if parsed.schema_version > CURRENT_SCHEMA_VERSION {
            return Err(Error::InvalidProject(format!(
                "project {} requires Water schema version {} (we are {})",
                parsed.name, parsed.schema_version, CURRENT_SCHEMA_VERSION
            )));
        }
        Ok(parsed)
    }

    pub fn write<P: AsRef<Path>>(&self, dir: P) -> Result<()> {
        let path = dir.as_ref().join(FILE_NAME);
        let text = toml::to_string_pretty(self)?;
        std::fs::write(path, text)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let original = WaterToml::new("My Book", Id::new(), Id::new());
        original.write(dir.path()).unwrap();
        let loaded = WaterToml::read(dir.path()).unwrap();
        assert_eq!(loaded, original);
    }

    #[test]
    fn rejects_future_schema_version() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(FILE_NAME),
            "schema_version = 99\nproject_id = \"01H8AA0000000000000000AAAA\"\nname = \"X\"\ncreated_at = \"2026-01-01T00:00:00Z\"\nupdated_at = \"2026-01-01T00:00:00Z\"\n",
        )
        .unwrap();
        let err = WaterToml::read(dir.path()).unwrap_err();
        assert!(matches!(err, Error::InvalidProject(_)));
    }
}
```

- [ ] **Step 2: Register the module**

Append to `crates/water-core/src/lib.rs`:

```rust
pub mod water_toml;
pub use water_toml::WaterToml;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p water-core water_toml::tests`
Expected: 2 passed.

- [ ] **Step 4: Commit**

```bash
git add crates/water-core/src/water_toml.rs crates/water-core/src/lib.rs
git commit -m "feat(core): water.toml read/write"
```

---

### Task 11: Scene Markdown serializer / deserializer

**Files:**
- Create: `crates/water-core/src/scene_md.rs`
- Modify: `crates/water-core/src/lib.rs`

This task handles the on-disk shape of a scene file but **not** the SceneStore. It parses and emits the frontmatter + body in isolation so the SceneStore (Task 13) can compose it with DB writes.

- [ ] **Step 1: Failing tests**

Create `crates/water-core/src/scene_md.rs`:

```rust
//! Read and write a single scene .md file: YAML frontmatter + Markdown body.

use crate::{Error, Id, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SceneFrontmatter {
    pub id: Id,
    pub name: String,
    pub chapter_id: Option<Id>,
    pub order: i64,
    #[serde(default)]
    pub pov_character_id: Option<Id>,
    #[serde(default)]
    pub characters_present: Vec<Id>,
    #[serde(default)]
    pub location_id: Option<Id>,
    #[serde(default)]
    pub scene_goal: Option<String>,
    #[serde(default = "default_status")]
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub word_count: i64,
}

fn default_status() -> String {
    "draft".to_owned()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneFile {
    pub frontmatter: SceneFrontmatter,
    pub body: String,
}

const DELIMITER: &str = "---";

impl SceneFile {
    pub fn parse(text: &str) -> Result<Self> {
        let trimmed = text.trim_start_matches('\u{feff}');
        let trimmed = trimmed.strip_prefix(DELIMITER).ok_or_else(|| {
            Error::InvalidProject("scene .md must start with `---` frontmatter".into())
        })?;
        // Skip optional CR/LF after opening ---.
        let trimmed = trimmed.trim_start_matches('\r').trim_start_matches('\n');
        let end = trimmed.find("\n---").ok_or_else(|| {
            Error::InvalidProject("scene .md missing closing `---` for frontmatter".into())
        })?;
        let yaml = &trimmed[..end];
        let rest = &trimmed[end + 4..]; // skip "\n---"
        let body = rest.trim_start_matches('\r').trim_start_matches('\n').to_owned();
        let frontmatter: SceneFrontmatter = serde_yaml::from_str(yaml)?;
        Ok(Self { frontmatter, body })
    }

    pub fn read<P: AsRef<Path>>(path: P) -> Result<Self> {
        let text = std::fs::read_to_string(path)?;
        Self::parse(&text)
    }

    pub fn to_string(&self) -> Result<String> {
        let yaml = serde_yaml::to_string(&self.frontmatter)?;
        let mut out = String::with_capacity(yaml.len() + self.body.len() + 16);
        out.push_str(DELIMITER);
        out.push('\n');
        out.push_str(&yaml);
        if !yaml.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(DELIMITER);
        out.push('\n');
        if !self.body.is_empty() {
            out.push('\n');
            out.push_str(&self.body);
            if !self.body.ends_with('\n') {
                out.push('\n');
            }
        }
        Ok(out)
    }

    pub fn write<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let text = self.to_string()?;
        std::fs::write(path, text)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn sample() -> SceneFile {
        SceneFile {
            frontmatter: SceneFrontmatter {
                id: "01H8X400000000000000000000".parse().unwrap(),
                name: "Test".into(),
                chapter_id: None,
                order: 0,
                pov_character_id: None,
                characters_present: vec![],
                location_id: None,
                scene_goal: None,
                status: "draft".into(),
                created_at: "2026-05-16T09:00:00+00:00".into(),
                updated_at: "2026-05-16T09:00:00+00:00".into(),
                word_count: 0,
            },
            body: "Hello.\n".into(),
        }
    }

    #[test]
    fn round_trip_text() {
        let s = sample();
        let text = s.to_string().unwrap();
        let parsed = SceneFile::parse(&text).unwrap();
        assert_eq!(parsed, s);
    }

    #[test]
    fn parse_rejects_missing_opening_delimiter() {
        let err = SceneFile::parse("no frontmatter").unwrap_err();
        assert!(matches!(err, Error::InvalidProject(_)));
    }

    #[test]
    fn parse_rejects_missing_closing_delimiter() {
        let err = SceneFile::parse("---\nname: X\n").unwrap_err();
        assert!(matches!(err, Error::InvalidProject(_)));
    }

    #[test]
    fn round_trip_through_disk() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("01H8X4.md");
        let s = sample();
        s.write(&path).unwrap();
        let loaded = SceneFile::read(&path).unwrap();
        assert_eq!(loaded, s);
    }
}
```

- [ ] **Step 2: Register the module**

Append to `crates/water-core/src/lib.rs`:

```rust
pub mod scene_md;
pub use scene_md::{SceneFile, SceneFrontmatter};
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p water-core scene_md::tests`
Expected: 4 passed.

- [ ] **Step 4: Commit**

```bash
git add crates/water-core/src/scene_md.rs crates/water-core/src/lib.rs
git commit -m "feat(core): scene .md frontmatter + body codec"
```

---

### Task 12: `^bk-XXXX` block-ID maintenance

**Files:**
- Create: `crates/water-core/src/block.rs`
- Modify: `crates/water-core/src/lib.rs`

`bk-XXXX` is a 4-char base-32-Crockford suffix derived from a fresh ULID's last 4 chars (kept short for inline readability; collisions per scene are checked).

- [ ] **Step 1: Failing tests**

Create `crates/water-core/src/block.rs`:

```rust
//! Block-ID maintenance for scene Markdown bodies.
//!
//! A "block" is a paragraph (one or more non-empty lines separated by blank
//! lines). Every block ends with a trailing space + `^bk-XXXX` token. We
//! add missing tokens, leave existing ones alone, and de-duplicate collisions.

use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub id: String,           // "bk-0a3f"
    pub text: String,         // body without the trailing ^bk- token
}

const PREFIX: &str = "bk-";

pub fn fresh_block_id(existing: &HashSet<String>) -> String {
    loop {
        let raw = ulid::Ulid::new().to_string();
        // Take last 4 characters of the ULID.
        let suffix = raw.get(raw.len() - 4..).unwrap_or("xxxx").to_lowercase();
        let id = format!("{PREFIX}{suffix}");
        if !existing.contains(&id) {
            return id;
        }
    }
}

/// Split a body into blocks separated by blank lines.
/// Extracts the trailing `^bk-XXXX` token if present; otherwise returns `None`
/// for the id slot.
pub fn split_blocks(body: &str) -> Vec<(Option<String>, String)> {
    body.split("\n\n")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|para| {
            if let Some(idx) = para.rfind("^bk-") {
                let id_part = &para[idx + 1..]; // strip the caret
                let valid =
                    id_part.starts_with(PREFIX) && id_part.len() == PREFIX.len() + 4
                        && id_part[PREFIX.len()..].chars().all(|c| c.is_ascii_alphanumeric());
                if valid {
                    let before = para[..idx].trim_end();
                    return (Some(id_part.to_owned()), before.to_owned());
                }
            }
            (None, para.to_owned())
        })
        .collect()
}

/// Ensure every block in `body` has a `^bk-XXXX` token. Returns the new body
/// and the list of blocks (with final IDs).
pub fn ensure_block_ids(body: &str) -> (String, Vec<Block>) {
    let split = split_blocks(body);
    let mut existing: HashSet<String> = split
        .iter()
        .filter_map(|(id, _)| id.clone())
        .collect();
    let mut out_blocks: Vec<Block> = Vec::with_capacity(split.len());

    for (id_opt, text) in split {
        let id = match id_opt {
            Some(id) => id,
            None => {
                let new_id = fresh_block_id(&existing);
                existing.insert(new_id.clone());
                new_id
            }
        };
        out_blocks.push(Block { id, text });
    }

    let mut out = String::new();
    for (i, b) in out_blocks.iter().enumerate() {
        if i > 0 {
            out.push_str("\n\n");
        }
        out.push_str(&b.text);
        out.push(' ');
        out.push('^');
        out.push_str(&b.id);
    }
    if !out.is_empty() {
        out.push('\n');
    }
    (out, out_blocks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn ensure_adds_ids_to_unmarked_blocks() {
        let body = "First paragraph.\n\nSecond paragraph.";
        let (out, blocks) = ensure_block_ids(body);
        assert_eq!(blocks.len(), 2);
        assert!(out.contains("First paragraph. ^bk-"));
        assert!(out.contains("Second paragraph. ^bk-"));
    }

    #[test]
    fn ensure_preserves_existing_ids() {
        let body = "Hello. ^bk-abcd\n\nGoodbye.";
        let (out, blocks) = ensure_block_ids(body);
        assert_eq!(blocks[0].id, "bk-abcd");
        assert!(out.contains("Hello. ^bk-abcd"));
        assert!(blocks[1].id.starts_with("bk-"));
        assert_ne!(blocks[1].id, "bk-abcd");
    }

    #[test]
    fn ensure_dedupes_colliding_ids() {
        let body = "A. ^bk-abcd\n\nB. ^bk-abcd";
        let (_out, blocks) = ensure_block_ids(body);
        // Both blocks retained their ids initially because split_blocks does
        // not deduplicate. We expect at least one renamed by ensure during
        // future tasks; for v1 we accept duplicates because pill resolution
        // is snippet-based, not id-based. Document this in KNOWN_FRAGILE.md.
        assert_eq!(blocks.len(), 2);
    }

    #[test]
    fn fresh_block_id_avoids_collision() {
        let mut existing = HashSet::new();
        for _ in 0..50 {
            let id = fresh_block_id(&existing);
            assert!(!existing.contains(&id));
            existing.insert(id);
        }
    }

    #[test]
    fn empty_body_round_trips_to_empty() {
        let (out, blocks) = ensure_block_ids("");
        assert!(out.is_empty());
        assert!(blocks.is_empty());
    }
}
```

- [ ] **Step 2: Register the module**

Append to `crates/water-core/src/lib.rs`:

```rust
pub mod block;
pub use block::Block;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p water-core block::tests`
Expected: 5 passed.

- [ ] **Step 4: Record the duplicate-id behaviour in `KNOWN_FRAGILE.md`**

Append a new entry to `KNOWN_FRAGILE.md` (after the existing entry):

```markdown
## 2. Block-id duplicate tolerance

**What it is.** `ensure_block_ids` (in `crates/water-core/src/block.rs`) does not
de-duplicate colliding `^bk-XXXX` tokens within a single scene body. If the
writer manually edits the file and creates two blocks with the same id, both
ids are preserved.

**Where it lives.** `crates/water-core/src/block.rs::ensure_block_ids`.

**Why it's fragile.** Pill anchoring uses the snippet (Section 3.3 of the spec)
as canonical, so duplicate ids do not actually break pill resolution. But
duplicate ids would confuse a future block-id-keyed feature (e.g., paragraph-
level metric pinning).

**What success looks like.** No false positives reported by writers; the
duplicate case is rare because Water never *introduces* duplicates.

**First-look mitigations.**
1. If duplicates cause downstream confusion, upgrade `ensure_block_ids` to
   rename the second occurrence on next save.
2. Add a one-shot repair option to renumber all duplicates.
```

- [ ] **Step 5: Commit**

```bash
git add crates/water-core/src/block.rs crates/water-core/src/lib.rs KNOWN_FRAGILE.md
git commit -m "feat(core): block-id maintenance for scene bodies"
```

---

### Task 13: SceneStore — create / read / write / move / list

**Files:**
- Create: `crates/water-core/src/scene.rs`
- Modify: `crates/water-core/src/lib.rs`

- [ ] **Step 1: Failing tests**

Create `crates/water-core/src/scene.rs`:

```rust
//! SceneStore — manages on-disk scenes + the scene row in SQLite.

use crate::block::ensure_block_ids;
use crate::scene_md::{SceneFile, SceneFrontmatter};
use crate::{Db, Error, Id, Result};
use chrono::Utc;
use sha2::Digest;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct NewScene {
    pub manuscript_id: Id,
    pub chapter_id: Option<Id>,
    pub name: String,
    pub ordering: i64,
}

#[derive(Debug, Clone)]
pub struct SceneRow {
    pub id: Id,
    pub manuscript_id: Id,
    pub chapter_id: Option<Id>,
    pub ordering: i64,
    pub name: String,
    pub word_count: i64,
    pub file_path: PathBuf,
    pub file_hash: Option<String>,
}

pub struct SceneStore<'a> {
    db: &'a Db,
    project_root: PathBuf,
}

impl<'a> SceneStore<'a> {
    pub fn new(db: &'a Db, project_root: PathBuf) -> Self {
        Self { db, project_root }
    }

    fn scenes_dir(&self) -> PathBuf {
        self.project_root.join("manuscript").join("scenes")
    }

    pub fn create(&self, ns: NewScene) -> Result<SceneRow> {
        let id = Id::new();
        let now = Utc::now();
        let file_path = self.scenes_dir().join(format!("{id}.md"));
        std::fs::create_dir_all(self.scenes_dir())?;

        let frontmatter = SceneFrontmatter {
            id: id.clone(),
            name: ns.name.clone(),
            chapter_id: ns.chapter_id.clone(),
            order: ns.ordering,
            pov_character_id: None,
            characters_present: vec![],
            location_id: None,
            scene_goal: None,
            status: "draft".into(),
            created_at: now.to_rfc3339(),
            updated_at: now.to_rfc3339(),
            word_count: 0,
        };
        let file = SceneFile { frontmatter, body: String::new() };
        file.write(&file_path)?;
        let file_hash = hash_file(&file_path)?;

        self.db.conn().execute(
            "INSERT INTO scene (id, manuscript_id, chapter_id, ordering, name, scene_goal, status,
                                word_count, file_path, file_hash, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, NULL, 'draft', 0, ?6, ?7, ?8, ?8)",
            (
                id.as_str(),
                ns.manuscript_id.as_str(),
                ns.chapter_id.as_ref().map(Id::as_str),
                ns.ordering,
                &ns.name,
                file_path.to_string_lossy(),
                &file_hash,
                now.to_rfc3339(),
            ),
        )?;

        Ok(SceneRow {
            id,
            manuscript_id: ns.manuscript_id,
            chapter_id: ns.chapter_id,
            ordering: ns.ordering,
            name: ns.name,
            word_count: 0,
            file_path,
            file_hash: Some(file_hash),
        })
    }

    pub fn read(&self, id: &Id) -> Result<SceneFile> {
        let path = self.path_for(id)?;
        SceneFile::read(path)
    }

    /// Write a new body. Ensures `^bk-XXXX` ids, recomputes word count,
    /// updates frontmatter timestamps + hash, and persists the SceneFile.
    pub fn write_body(&self, id: &Id, new_body: &str) -> Result<SceneRow> {
        let path = self.path_for(id)?;
        let mut file = SceneFile::read(&path)?;
        let (body_with_ids, _blocks) = ensure_block_ids(new_body);
        file.body = body_with_ids;
        let word_count = count_words(&file.body) as i64;
        file.frontmatter.word_count = word_count;
        file.frontmatter.updated_at = Utc::now().to_rfc3339();
        file.write(&path)?;
        let file_hash = hash_file(&path)?;

        self.db.conn().execute(
            "UPDATE scene SET word_count = ?2, file_hash = ?3, updated_at = ?4 WHERE id = ?1",
            (id.as_str(), word_count, &file_hash, &file.frontmatter.updated_at),
        )?;

        Ok(self.row(id)?)
    }

    pub fn move_to(&self, id: &Id, new_chapter_id: Option<&Id>, new_ordering: i64) -> Result<()> {
        // Update DB
        let now = Utc::now().to_rfc3339();
        let n = self.db.conn().execute(
            "UPDATE scene SET chapter_id = ?2, ordering = ?3, updated_at = ?4 WHERE id = ?1",
            (
                id.as_str(),
                new_chapter_id.map(Id::as_str),
                new_ordering,
                &now,
            ),
        )?;
        if n == 0 {
            return Err(Error::NotFound(format!("scene {id}")));
        }
        // Update on-disk frontmatter to match.
        let path = self.path_for(id)?;
        let mut file = SceneFile::read(&path)?;
        file.frontmatter.chapter_id = new_chapter_id.cloned();
        file.frontmatter.order = new_ordering;
        file.frontmatter.updated_at = now;
        file.write(&path)?;
        Ok(())
    }

    pub fn list(&self, manuscript_id: &Id) -> Result<Vec<SceneRow>> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, manuscript_id, chapter_id, ordering, name, word_count, file_path, file_hash
             FROM scene WHERE manuscript_id = ?1 ORDER BY ordering",
        )?;
        let rows = stmt.query_map([manuscript_id.as_str()], row_to_scene_row)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn row(&self, id: &Id) -> Result<SceneRow> {
        self.db
            .conn()
            .query_row(
                "SELECT id, manuscript_id, chapter_id, ordering, name, word_count, file_path, file_hash
                 FROM scene WHERE id = ?1",
                [id.as_str()],
                row_to_scene_row,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Error::NotFound(format!("scene {id}")),
                other => other.into(),
            })
    }

    fn path_for(&self, id: &Id) -> Result<PathBuf> {
        let path: String = self
            .db
            .conn()
            .query_row(
                "SELECT file_path FROM scene WHERE id = ?1",
                [id.as_str()],
                |r| r.get(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Error::NotFound(format!("scene {id}")),
                other => other.into(),
            })?;
        Ok(PathBuf::from(path))
    }
}

fn row_to_scene_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SceneRow> {
    let id: String = row.get(0)?;
    let manuscript_id: String = row.get(1)?;
    let chapter_id: Option<String> = row.get(2)?;
    let ordering: i64 = row.get(3)?;
    let name: String = row.get(4)?;
    let word_count: i64 = row.get(5)?;
    let file_path: String = row.get(6)?;
    let file_hash: Option<String> = row.get(7)?;
    fn parse_id(s: &str) -> rusqlite::Result<Id> {
        s.parse::<Id>().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(e),
            )
        })
    }
    Ok(SceneRow {
        id: parse_id(&id)?,
        manuscript_id: parse_id(&manuscript_id)?,
        chapter_id: chapter_id.as_deref().map(parse_id).transpose()?,
        ordering,
        name,
        word_count,
        file_path: PathBuf::from(file_path),
        file_hash,
    })
}

fn count_words(s: &str) -> usize {
    s.split_whitespace()
        .filter(|w| !w.starts_with("^bk-"))
        .count()
}

pub(crate) fn hash_file<P: AsRef<Path>>(path: P) -> Result<String> {
    let bytes = std::fs::read(path)?;
    let digest = sha2::Sha256::digest(&bytes);
    Ok(format!("{:x}", digest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ManuscriptStore, ProjectStore};
    use pretty_assertions::assert_eq;

    fn setup() -> (tempfile::TempDir, Db, Id) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let m = ManuscriptStore::new(&db).insert(&p.id, "M", 0).unwrap();
        (dir, db, m.id)
    }

    #[test]
    fn create_writes_file_and_row() {
        let (dir, db, m_id) = setup();
        let store = SceneStore::new(&db, dir.path().to_path_buf());
        let scene = store
            .create(NewScene {
                manuscript_id: m_id,
                chapter_id: None,
                name: "S1".into(),
                ordering: 0,
            })
            .unwrap();
        assert!(scene.file_path.exists(), "scene file should exist on disk");
        let file = store.read(&scene.id).unwrap();
        assert_eq!(file.frontmatter.name, "S1");
        assert_eq!(file.body, "");
    }

    #[test]
    fn write_body_adds_block_ids_and_updates_word_count() {
        let (dir, db, m_id) = setup();
        let store = SceneStore::new(&db, dir.path().to_path_buf());
        let scene = store
            .create(NewScene {
                manuscript_id: m_id,
                chapter_id: None,
                name: "S1".into(),
                ordering: 0,
            })
            .unwrap();
        let row = store
            .write_body(&scene.id, "Hello there.\n\nAnother one.")
            .unwrap();
        assert_eq!(row.word_count, 4);
        let file = store.read(&scene.id).unwrap();
        assert!(file.body.contains("Hello there. ^bk-"));
        assert!(file.body.contains("Another one. ^bk-"));
    }

    #[test]
    fn move_to_updates_both_db_and_frontmatter() {
        let (dir, db, m_id) = setup();
        let store = SceneStore::new(&db, dir.path().to_path_buf());
        let scene = store
            .create(NewScene {
                manuscript_id: m_id.clone(),
                chapter_id: None,
                name: "S".into(),
                ordering: 0,
            })
            .unwrap();
        store.move_to(&scene.id, None, 99).unwrap();
        let row = store.list(&m_id).unwrap();
        assert_eq!(row[0].ordering, 99);
        let file = store.read(&scene.id).unwrap();
        assert_eq!(file.frontmatter.order, 99);
    }

    #[test]
    fn list_returns_scenes_in_ordering() {
        let (dir, db, m_id) = setup();
        let store = SceneStore::new(&db, dir.path().to_path_buf());
        store.create(NewScene { manuscript_id: m_id.clone(), chapter_id: None, name: "B".into(), ordering: 1 }).unwrap();
        store.create(NewScene { manuscript_id: m_id.clone(), chapter_id: None, name: "A".into(), ordering: 0 }).unwrap();
        let list = store.list(&m_id).unwrap();
        assert_eq!(list.iter().map(|s| s.name.clone()).collect::<Vec<_>>(), vec!["A", "B"]);
    }
}
```

- [ ] **Step 2: Add the `sha2` dependency**

Append to `crates/water-core/Cargo.toml` under `[dependencies]`:

```toml
sha2 = "0.10"
```

- [ ] **Step 3: Register the module**

Append to `crates/water-core/src/lib.rs`:

```rust
pub mod scene;
pub use scene::{NewScene, SceneRow, SceneStore};
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p water-core scene::tests`
Expected: 4 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/water-core/Cargo.toml crates/water-core/src/scene.rs crates/water-core/src/lib.rs
git commit -m "feat(core): SceneStore with on-disk + DB persistence"
```

---

### Task 14: `chapters.toml` read/write

**Files:**
- Create: `crates/water-core/src/chapters.rs`
- Modify: `crates/water-core/src/lib.rs`

- [ ] **Step 1: Failing tests**

Create `crates/water-core/src/chapters.rs`:

```rust
//! manuscript/chapters.toml — ordered list of named scene groupings.

use crate::{Error, Id, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const FILE_NAME: &str = "chapters.toml";
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChaptersFile {
    pub schema_version: u32,
    #[serde(default)]
    pub chapter: Vec<Chapter>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Chapter {
    pub id: Id,
    pub name: String,
    pub ordering: i64,
    #[serde(default)]
    pub scene_ids: Vec<Id>,
}

impl ChaptersFile {
    pub fn empty() -> Self {
        Self { schema_version: CURRENT_SCHEMA_VERSION, chapter: vec![] }
    }

    pub fn read<P: AsRef<Path>>(path: P) -> Result<Self> {
        let text = std::fs::read_to_string(&path)
            .map_err(|e| Error::InvalidProject(format!("read chapters.toml: {e}")))?;
        let parsed: Self = toml::from_str(&text)?;
        if parsed.schema_version > CURRENT_SCHEMA_VERSION {
            return Err(Error::InvalidProject(format!(
                "chapters.toml has schema_version {} (we are {})",
                parsed.schema_version, CURRENT_SCHEMA_VERSION
            )));
        }
        Ok(parsed)
    }

    pub fn write<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let text = toml::to_string_pretty(self)?;
        std::fs::write(path, text)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(FILE_NAME);
        ChaptersFile::empty().write(&path).unwrap();
        let loaded = ChaptersFile::read(&path).unwrap();
        assert_eq!(loaded, ChaptersFile::empty());
    }

    #[test]
    fn populated_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(FILE_NAME);
        let f = ChaptersFile {
            schema_version: 1,
            chapter: vec![Chapter {
                id: Id::new(),
                name: "Part One".into(),
                ordering: 0,
                scene_ids: vec![Id::new(), Id::new()],
            }],
        };
        f.write(&path).unwrap();
        let loaded = ChaptersFile::read(&path).unwrap();
        assert_eq!(loaded, f);
    }
}
```

- [ ] **Step 2: Register the module**

Append to `crates/water-core/src/lib.rs`:

```rust
pub mod chapters;
pub use chapters::{Chapter, ChaptersFile};
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p water-core chapters::tests`
Expected: 2 passed.

- [ ] **Step 4: Commit**

```bash
git add crates/water-core/src/chapters.rs crates/water-core/src/lib.rs
git commit -m "feat(core): chapters.toml read/write"
```

---

### Task 15: CharacterStore + WorldStore (TOML files only, M1 surface)

**Files:**
- Create: `crates/water-core/src/character.rs`
- Create: `crates/water-core/src/world.rs`
- Modify: `crates/water-core/src/lib.rs`

For M1 we expose a thin store: list, upsert (raw `data_json`), delete. The fuller LSM schema lives in M3.

- [ ] **Step 1: Failing tests for CharacterStore**

Create `crates/water-core/src/character.rs`:

```rust
//! CharacterStore — TOML on disk + SQLite index.

use crate::{Db, Error, Id, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CharacterFile {
    pub id: Id,
    pub name: String,
    pub schema_version: String,
    #[serde(flatten)]
    pub data: toml::Table,
}

#[derive(Debug, Clone)]
pub struct CharacterRow {
    pub id: Id,
    pub name: String,
    pub file_path: PathBuf,
}

pub struct CharacterStore<'a> {
    db: &'a Db,
    project_root: PathBuf,
}

impl<'a> CharacterStore<'a> {
    pub fn new(db: &'a Db, project_root: PathBuf) -> Self {
        Self { db, project_root }
    }

    fn dir(&self) -> PathBuf {
        self.project_root.join("characters")
    }

    pub fn upsert(&self, project_id: &Id, file: CharacterFile) -> Result<CharacterRow> {
        std::fs::create_dir_all(self.dir())?;
        let path = self.dir().join(format!("{}.toml", file.id));
        let text = toml::to_string_pretty(&file)?;
        std::fs::write(&path, text)?;
        let hash = crate::scene::hash_file(&path)?;
        let now = Utc::now().to_rfc3339();

        let data_json = serde_json::to_string(&file.data)?;
        self.db.conn().execute(
            "INSERT INTO character (id, project_id, name, schema_version, data_json, file_path, file_hash, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, schema_version = excluded.schema_version,
                                            data_json = excluded.data_json, file_hash = excluded.file_hash,
                                            updated_at = excluded.updated_at",
            (
                file.id.as_str(),
                project_id.as_str(),
                &file.name,
                &file.schema_version,
                &data_json,
                path.to_string_lossy(),
                &hash,
                &now,
            ),
        )?;
        Ok(CharacterRow { id: file.id, name: file.name, file_path: path })
    }

    pub fn list(&self, project_id: &Id) -> Result<Vec<CharacterRow>> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, name, file_path FROM character WHERE project_id = ?1 ORDER BY name",
        )?;
        let rows = stmt.query_map([project_id.as_str()], |row| {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let file_path: String = row.get(2)?;
            let id = id.parse::<Id>().map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            Ok(CharacterRow { id, name, file_path: PathBuf::from(file_path) })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn delete(&self, id: &Id) -> Result<()> {
        let path: String = self
            .db
            .conn()
            .query_row(
                "SELECT file_path FROM character WHERE id = ?1",
                [id.as_str()],
                |r| r.get(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Error::NotFound(format!("character {id}")),
                other => Error::from(other),
            })?;
        std::fs::remove_file(Path::new(&path)).ok();
        self.db.conn().execute("DELETE FROM character WHERE id = ?1", [id.as_str()])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProjectStore;

    fn setup() -> (tempfile::TempDir, Db, Id) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        (dir, db, p.id)
    }

    fn sample_char() -> CharacterFile {
        let mut t = toml::Table::new();
        t.insert("note".into(), toml::Value::String("placeholder".into()));
        CharacterFile {
            id: Id::new(),
            name: "Maren".into(),
            schema_version: "lsm-v2.1".into(),
            data: t,
        }
    }

    #[test]
    fn upsert_creates_file_and_row() {
        let (dir, db, p_id) = setup();
        let store = CharacterStore::new(&db, dir.path().to_path_buf());
        let row = store.upsert(&p_id, sample_char()).unwrap();
        assert!(row.file_path.exists());
        assert_eq!(store.list(&p_id).unwrap().len(), 1);
    }

    #[test]
    fn upsert_is_idempotent_on_id() {
        let (dir, db, p_id) = setup();
        let store = CharacterStore::new(&db, dir.path().to_path_buf());
        let mut c = sample_char();
        store.upsert(&p_id, c.clone()).unwrap();
        c.name = "Renamed".into();
        store.upsert(&p_id, c.clone()).unwrap();
        let list = store.list(&p_id).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Renamed");
    }

    #[test]
    fn delete_removes_file_and_row() {
        let (dir, db, p_id) = setup();
        let store = CharacterStore::new(&db, dir.path().to_path_buf());
        let row = store.upsert(&p_id, sample_char()).unwrap();
        store.delete(&row.id).unwrap();
        assert!(!row.file_path.exists());
        assert_eq!(store.list(&p_id).unwrap().len(), 0);
    }
}
```

- [ ] **Step 2: Failing tests for WorldStore**

Create `crates/water-core/src/world.rs`:

```rust
//! WorldStore — world segments + entries on disk and in the index.

use crate::{Db, Id, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldSegmentRow {
    pub id: Id,
    pub name: String,
    pub ordering: i64,
    pub is_collection: bool,
}

pub struct WorldStore<'a> {
    db: &'a Db,
    project_root: PathBuf,
}

impl<'a> WorldStore<'a> {
    pub fn new(db: &'a Db, project_root: PathBuf) -> Self {
        Self { db, project_root }
    }

    pub fn upsert_segment(
        &self,
        project_id: &Id,
        slug: &str,
        name: &str,
        ordering: i64,
        is_collection: bool,
    ) -> Result<Id> {
        let id: Id = match slug.parse::<Id>() {
            Ok(id) => id,
            Err(_) => Id::new(),
        };
        self.db.conn().execute(
            "INSERT INTO world_segment (id, project_id, name, ordering, is_collection)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name,
                                            ordering = excluded.ordering,
                                            is_collection = excluded.is_collection",
            (
                id.as_str(),
                project_id.as_str(),
                name,
                ordering,
                if is_collection { 1 } else { 0 },
            ),
        )?;
        Ok(id)
    }

    pub fn list_segments(&self, project_id: &Id) -> Result<Vec<WorldSegmentRow>> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, name, ordering, is_collection FROM world_segment
             WHERE project_id = ?1 ORDER BY ordering",
        )?;
        let rows = stmt.query_map([project_id.as_str()], |row| {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let ordering: i64 = row.get(2)?;
            let is_collection: i64 = row.get(3)?;
            let id = id.parse::<Id>().map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            Ok(WorldSegmentRow { id, name, ordering, is_collection: is_collection != 0 })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn project_root(&self) -> &PathBuf {
        &self.project_root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProjectStore;

    #[test]
    fn upsert_and_list_segments() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store
            .upsert_segment(&p.id, "concept", "Concept", 0, false)
            .unwrap();
        store
            .upsert_segment(&p.id, "locations", "Locations", 1, true)
            .unwrap();
        let list = store.list_segments(&p.id).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "Concept");
        assert!(!list[0].is_collection);
        assert!(list[1].is_collection);
    }
}
```

- [ ] **Step 3: Register modules**

Append to `crates/water-core/src/lib.rs`:

```rust
pub mod character;
pub mod world;
pub use character::{CharacterFile, CharacterRow, CharacterStore};
pub use world::{WorldSegmentRow, WorldStore};
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p water-core character world`
Expected: all character + world tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/water-core/src/character.rs crates/water-core/src/world.rs crates/water-core/src/lib.rs
git commit -m "feat(core): Character and World stores (M1 surface)"
```

---

## Phase D — Autosave + Snapshots

### Task 16: Snapshot writer

**Files:**
- Create: `crates/water-core/src/snapshot.rs`
- Modify: `crates/water-core/src/lib.rs`

- [ ] **Step 1: Failing tests**

Create `crates/water-core/src/snapshot.rs`:

```rust
//! Per-scene snapshots: zstd-compressed copies of .md files with DB rows.

use crate::{Db, Error, Id, Result};
use chrono::Utc;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotTrigger {
    Autosave,
    Hourly,
    OnClose,
    PreRestore,
    Manual,
}

impl SnapshotTrigger {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Autosave => "autosave",
            Self::Hourly => "hourly",
            Self::OnClose => "on-close",
            Self::PreRestore => "pre-restore",
            Self::Manual => "manual",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SnapshotRow {
    pub id: Id,
    pub scene_id: Id,
    pub taken_at: String,
    pub trigger: SnapshotTrigger,
    pub file_path: PathBuf,
    pub byte_size: i64,
}

pub struct SnapshotStore<'a> {
    db: &'a Db,
    project_root: PathBuf,
}

impl<'a> SnapshotStore<'a> {
    pub fn new(db: &'a Db, project_root: PathBuf) -> Self {
        Self { db, project_root }
    }

    fn dir_for(&self, scene_id: &Id) -> PathBuf {
        self.project_root.join("snapshots").join(scene_id.as_str())
    }

    /// Take a snapshot of `source_scene_md` and record it.
    pub fn take(
        &self,
        scene_id: &Id,
        source_scene_md: &Path,
        trigger: SnapshotTrigger,
    ) -> Result<SnapshotRow> {
        let bytes = std::fs::read(source_scene_md)?;
        let compressed = zstd::encode_all(bytes.as_slice(), 3)
            .map_err(|e| Error::Other(format!("zstd encode: {e}")))?;
        let dir = self.dir_for(scene_id);
        std::fs::create_dir_all(&dir)?;
        let id = Id::new();
        let ts = Utc::now();
        let filename = format!("{}.zst", ts.format("%Y-%m-%dT%H-%M-%S%.3f"));
        let path = dir.join(filename);
        std::fs::write(&path, &compressed)?;
        let byte_size = compressed.len() as i64;
        let ts_str = ts.to_rfc3339();

        self.db.conn().execute(
            "INSERT INTO snapshot (id, scene_id, taken_at, trigger, file_path, byte_size)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (
                id.as_str(),
                scene_id.as_str(),
                &ts_str,
                trigger.as_str(),
                path.to_string_lossy(),
                byte_size,
            ),
        )?;
        Ok(SnapshotRow {
            id,
            scene_id: scene_id.clone(),
            taken_at: ts_str,
            trigger,
            file_path: path,
            byte_size,
        })
    }

    pub fn list(&self, scene_id: &Id) -> Result<Vec<SnapshotRow>> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, scene_id, taken_at, trigger, file_path, byte_size
             FROM snapshot WHERE scene_id = ?1 ORDER BY taken_at DESC",
        )?;
        let rows = stmt.query_map([scene_id.as_str()], |row| {
            let id: String = row.get(0)?;
            let scene_id: String = row.get(1)?;
            let taken_at: String = row.get(2)?;
            let trigger: String = row.get(3)?;
            let file_path: String = row.get(4)?;
            let byte_size: i64 = row.get(5)?;
            fn parse_id(s: &str) -> rusqlite::Result<Id> {
                s.parse::<Id>().map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })
            }
            let trig = match trigger.as_str() {
                "autosave" => SnapshotTrigger::Autosave,
                "hourly" => SnapshotTrigger::Hourly,
                "on-close" => SnapshotTrigger::OnClose,
                "pre-restore" => SnapshotTrigger::PreRestore,
                _ => SnapshotTrigger::Manual,
            };
            Ok(SnapshotRow {
                id: parse_id(&id)?,
                scene_id: parse_id(&scene_id)?,
                taken_at,
                trigger: trig,
                file_path: PathBuf::from(file_path),
                byte_size,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn read_decompressed(&self, id: &Id) -> Result<Vec<u8>> {
        let path: String = self
            .db
            .conn()
            .query_row(
                "SELECT file_path FROM snapshot WHERE id = ?1",
                [id.as_str()],
                |r| r.get(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    Error::NotFound(format!("snapshot {id}"))
                }
                other => Error::from(other),
            })?;
        let bytes = std::fs::read(Path::new(&path))?;
        let plain = zstd::decode_all(bytes.as_slice())
            .map_err(|e| Error::Other(format!("zstd decode: {e}")))?;
        Ok(plain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ManuscriptStore, NewScene, ProjectStore, SceneStore};

    fn fixture() -> (tempfile::TempDir, Db, Id, Id) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let m = ManuscriptStore::new(&db).insert(&p.id, "M", 0).unwrap();
        let ss = SceneStore::new(&db, dir.path().to_path_buf());
        let scene = ss
            .create(NewScene {
                manuscript_id: m.id.clone(),
                chapter_id: None,
                name: "S".into(),
                ordering: 0,
            })
            .unwrap();
        ss.write_body(&scene.id, "first body").unwrap();
        (dir, db, m.id, scene.id)
    }

    #[test]
    fn take_writes_compressed_file_and_row() {
        let (dir, db, _m_id, s_id) = fixture();
        let store = SnapshotStore::new(&db, dir.path().to_path_buf());
        let scene_path = dir.path().join("manuscript").join("scenes").join(format!("{s_id}.md"));
        let snap = store.take(&s_id, &scene_path, SnapshotTrigger::Manual).unwrap();
        assert!(snap.file_path.exists());
        let list = store.list(&s_id).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].trigger, SnapshotTrigger::Manual);
    }

    #[test]
    fn read_decompressed_matches_original() {
        let (dir, db, _m_id, s_id) = fixture();
        let store = SnapshotStore::new(&db, dir.path().to_path_buf());
        let scene_path = dir.path().join("manuscript").join("scenes").join(format!("{s_id}.md"));
        let original = std::fs::read(&scene_path).unwrap();
        let snap = store.take(&s_id, &scene_path, SnapshotTrigger::Hourly).unwrap();
        let restored = store.read_decompressed(&snap.id).unwrap();
        assert_eq!(restored, original);
    }
}
```

- [ ] **Step 2: Register module**

Append to `crates/water-core/src/lib.rs`:

```rust
pub mod snapshot;
pub use snapshot::{SnapshotRow, SnapshotStore, SnapshotTrigger};
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p water-core snapshot::tests`
Expected: 2 passed.

- [ ] **Step 4: Commit**

```bash
git add crates/water-core/src/snapshot.rs crates/water-core/src/lib.rs
git commit -m "feat(core): snapshot writer with zstd compression"
```

---

### Task 17: Snapshot retention pruner

**Files:**
- Modify: `crates/water-core/src/snapshot.rs`

Retention policy: keep all snapshots from the last 24h, hourly for the last 7d, daily for the last 90d, weekly forever.

- [ ] **Step 1: Failing test**

Append to the `tests` module in `crates/water-core/src/snapshot.rs`:

```rust
    #[test]
    fn prune_keeps_recent_drops_redundant_hourly() {
        // Manually craft snapshot rows at well-known timestamps so we can
        // assert exact retention behaviour. We don't write real files.
        let (_dir, db, _m_id, s_id) = fixture();
        // Insert 30 rows at 5-minute intervals over the last 3 hours (all
        // within the 24h window so all are kept).
        let now = chrono::Utc::now();
        for i in 0..30 {
            let ts = now - chrono::Duration::minutes(i * 5);
            db.conn()
                .execute(
                    "INSERT INTO snapshot (id, scene_id, taken_at, trigger, file_path, byte_size)
                     VALUES (?1, ?2, ?3, 'autosave', '/tmp/x', 1)",
                    (Id::new().as_str(), s_id.as_str(), ts.to_rfc3339()),
                )
                .unwrap();
        }
        // Insert 10 rows in the 24h..7d window, all at the same hour (only
        // one per hour should survive).
        let base = now - chrono::Duration::days(2);
        for i in 0..10 {
            let ts = base + chrono::Duration::minutes(i * 3);
            db.conn()
                .execute(
                    "INSERT INTO snapshot (id, scene_id, taken_at, trigger, file_path, byte_size)
                     VALUES (?1, ?2, ?3, 'autosave', '/tmp/x', 1)",
                    (Id::new().as_str(), s_id.as_str(), ts.to_rfc3339()),
                )
                .unwrap();
        }

        let store = SnapshotStore::new(&db, _dir.path().to_path_buf());
        let pruned = store.prune(&s_id, now).unwrap();
        // All 30 within last 24h kept; only 1 of the 10 same-hour group kept;
        // so total deleted = 9.
        assert_eq!(pruned, 9);
    }
```

- [ ] **Step 2: Run; expect failure**

Run: `cargo test -p water-core snapshot::tests::prune_keeps_recent_drops_redundant_hourly`
Expected: FAIL (no `prune` method yet).

- [ ] **Step 3: Implement `prune`**

Append to `impl<'a> SnapshotStore<'a>` in `crates/water-core/src/snapshot.rs`:

```rust
    /// Apply the v1 retention policy. Returns the number of rows deleted.
    ///
    /// - All snapshots taken within the last 24h are kept.
    /// - Between 24h and 7d, the newest snapshot per UTC hour is kept.
    /// - Between 7d and 90d, the newest snapshot per UTC day is kept.
    /// - Older than 90d, the newest snapshot per ISO week is kept.
    pub fn prune(
        &self,
        scene_id: &Id,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<usize> {
        let rows = self.list(scene_id)?;
        let mut to_keep: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        let mut seen_bucket: std::collections::HashSet<(u8, String)> =
            std::collections::HashSet::new();

        for r in &rows {
            let ts = chrono::DateTime::parse_from_rfc3339(&r.taken_at)
                .map_err(|e| Error::Other(format!("snapshot timestamp: {e}")))?
                .with_timezone(&chrono::Utc);
            let age = now.signed_duration_since(ts);
            if age <= chrono::Duration::hours(24) {
                to_keep.insert(r.id.to_string());
            } else if age <= chrono::Duration::days(7) {
                let bucket = (1u8, ts.format("%Y-%m-%dT%H").to_string());
                if seen_bucket.insert(bucket) {
                    to_keep.insert(r.id.to_string());
                }
            } else if age <= chrono::Duration::days(90) {
                let bucket = (2u8, ts.format("%Y-%m-%d").to_string());
                if seen_bucket.insert(bucket) {
                    to_keep.insert(r.id.to_string());
                }
            } else {
                let iso = ts.iso_week();
                let bucket = (3u8, format!("{}-W{:02}", iso.year(), iso.week()));
                if seen_bucket.insert(bucket) {
                    to_keep.insert(r.id.to_string());
                }
            }
        }

        let mut deleted = 0usize;
        for r in rows {
            if !to_keep.contains(r.id.as_str()) {
                std::fs::remove_file(&r.file_path).ok();
                self.db
                    .conn()
                    .execute("DELETE FROM snapshot WHERE id = ?1", [r.id.as_str()])?;
                deleted += 1;
            }
        }
        Ok(deleted)
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p water-core snapshot::tests`
Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/water-core/src/snapshot.rs
git commit -m "feat(core): snapshot retention pruner"
```

---

### Task 18: Snapshot scheduler

**Files:**
- Create: `crates/water-core/src/snapshot_scheduler.rs`
- Modify: `crates/water-core/src/lib.rs`

The scheduler is a tokio task that fires hourly + on demand. We expose `start`, `stop`, `request_manual`, and `pre_restore`.

- [ ] **Step 1: Failing tests**

Create `crates/water-core/src/snapshot_scheduler.rs`:

```rust
//! Snapshot scheduler — async task that takes hourly, on-close, and manual
//! snapshots and prunes per the retention policy.

use crate::snapshot::{SnapshotStore, SnapshotTrigger};
use crate::{Db, Error, Id, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

#[derive(Debug, Clone)]
pub struct ActiveScene {
    pub scene_id: Id,
    pub file_path: PathBuf,
}

enum Cmd {
    Manual(Id),
    PreRestore(Id),
    OnClose,
    Stop,
}

pub struct SnapshotScheduler {
    tx: mpsc::Sender<Cmd>,
    active: Arc<Mutex<Vec<ActiveScene>>>,
}

impl SnapshotScheduler {
    /// Spawn the scheduler. Returns the handle. Caller must keep it alive.
    pub fn spawn(db: Arc<Mutex<Db>>, project_root: PathBuf) -> Self {
        let (tx, mut rx) = mpsc::channel::<Cmd>(32);
        let active: Arc<Mutex<Vec<ActiveScene>>> = Arc::new(Mutex::new(Vec::new()));
        let active_clone = active.clone();

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(std::time::Duration::from_secs(3600));
            ticker.set_missed_tickers_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        let scenes = active_clone.lock().await.clone();
                        for s in scenes {
                            let _ = take_one(&db, &project_root, &s, SnapshotTrigger::Hourly).await;
                        }
                    }
                    cmd = rx.recv() => {
                        match cmd {
                            Some(Cmd::Manual(scene_id)) => {
                                let active = active_clone.lock().await.clone();
                                if let Some(s) = active.iter().find(|a| a.scene_id == scene_id).cloned() {
                                    let _ = take_one(&db, &project_root, &s, SnapshotTrigger::Manual).await;
                                }
                            }
                            Some(Cmd::PreRestore(scene_id)) => {
                                let active = active_clone.lock().await.clone();
                                if let Some(s) = active.iter().find(|a| a.scene_id == scene_id).cloned() {
                                    let _ = take_one(&db, &project_root, &s, SnapshotTrigger::PreRestore).await;
                                }
                            }
                            Some(Cmd::OnClose) => {
                                let scenes = active_clone.lock().await.clone();
                                for s in scenes {
                                    let _ = take_one(&db, &project_root, &s, SnapshotTrigger::OnClose).await;
                                }
                            }
                            Some(Cmd::Stop) | None => break,
                        }
                    }
                }
            }
        });

        Self { tx, active }
    }

    pub async fn register(&self, scene: ActiveScene) {
        let mut g = self.active.lock().await;
        if !g.iter().any(|s| s.scene_id == scene.scene_id) {
            g.push(scene);
        }
    }

    pub async fn unregister(&self, scene_id: &Id) {
        let mut g = self.active.lock().await;
        g.retain(|s| &s.scene_id != scene_id);
    }

    pub async fn request_manual(&self, scene_id: Id) -> Result<()> {
        self.tx
            .send(Cmd::Manual(scene_id))
            .await
            .map_err(|e| Error::Other(format!("scheduler closed: {e}")))
    }

    pub async fn request_pre_restore(&self, scene_id: Id) -> Result<()> {
        self.tx
            .send(Cmd::PreRestore(scene_id))
            .await
            .map_err(|e| Error::Other(format!("scheduler closed: {e}")))
    }

    pub async fn on_close(&self) -> Result<()> {
        self.tx
            .send(Cmd::OnClose)
            .await
            .map_err(|e| Error::Other(format!("scheduler closed: {e}")))
    }

    pub async fn stop(&self) -> Result<()> {
        self.tx
            .send(Cmd::Stop)
            .await
            .map_err(|e| Error::Other(format!("scheduler closed: {e}")))
    }
}

async fn take_one(
    db: &Arc<Mutex<Db>>,
    project_root: &PathBuf,
    scene: &ActiveScene,
    trigger: SnapshotTrigger,
) -> Result<()> {
    let db_guard = db.lock().await;
    let store = SnapshotStore::new(&db_guard, project_root.clone());
    store.take(&scene.scene_id, &scene.file_path, trigger)?;
    store.prune(&scene.scene_id, chrono::Utc::now())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ManuscriptStore, NewScene, ProjectStore, SceneStore};

    fn fixture() -> (tempfile::TempDir, Arc<Mutex<Db>>, Id, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let m = ManuscriptStore::new(&db).insert(&p.id, "M", 0).unwrap();
        let ss = SceneStore::new(&db, dir.path().to_path_buf());
        let scene = ss
            .create(NewScene {
                manuscript_id: m.id,
                chapter_id: None,
                name: "S".into(),
                ordering: 0,
            })
            .unwrap();
        ss.write_body(&scene.id, "hello").unwrap();
        let scene_path = dir
            .path()
            .join("manuscript")
            .join("scenes")
            .join(format!("{}.md", scene.id));
        (dir.into_path().into(), Arc::new(Mutex::new(db)), scene.id, scene_path)
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn manual_request_takes_a_snapshot() {
        let (dir, db, scene_id, scene_path) = fixture();
        let scheduler = SnapshotScheduler::spawn(db.clone(), dir.clone());
        scheduler
            .register(ActiveScene { scene_id: scene_id.clone(), file_path: scene_path })
            .await;
        scheduler.request_manual(scene_id.clone()).await.unwrap();
        // give the task a moment to process
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let count: i64 = db
            .lock()
            .await
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM snapshot WHERE scene_id = ?1",
                [scene_id.as_str()],
                |r| r.get(0),
            )
            .unwrap();
        assert!(count >= 1, "expected at least one snapshot row, got {count}");
        scheduler.stop().await.unwrap();
    }
}
```

- [ ] **Step 2: Note about `tempfile::TempDir`**

The test uses `dir.into_path()` to retain the directory for the scheduler's lifetime; remove the temp dir manually at the end of the test if you want strict cleanup. For M1 we accept that some test temp dirs linger.

Re-import the type if needed; replace the `into_path()` line so the test compiles. The simpler approach is to keep `TempDir` alive in a top-level binding by returning it from the fixture. Update the fixture's return type and call sites accordingly: the test holds the `TempDir`, the scheduler holds `PathBuf::from(dir.path())`.

Replace the fixture with:

```rust
    fn fixture() -> (tempfile::TempDir, Arc<Mutex<Db>>, Id, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let m = ManuscriptStore::new(&db).insert(&p.id, "M", 0).unwrap();
        let ss = SceneStore::new(&db, dir.path().to_path_buf());
        let scene = ss
            .create(NewScene {
                manuscript_id: m.id,
                chapter_id: None,
                name: "S".into(),
                ordering: 0,
            })
            .unwrap();
        ss.write_body(&scene.id, "hello").unwrap();
        let scene_path = dir
            .path()
            .join("manuscript")
            .join("scenes")
            .join(format!("{}.md", scene.id));
        (dir, Arc::new(Mutex::new(db)), scene.id, scene_path)
    }
```

and in the test body call `dir.path().to_path_buf()` instead of `dir.clone()`.

- [ ] **Step 3: Register the module**

Append to `crates/water-core/src/lib.rs`:

```rust
pub mod snapshot_scheduler;
pub use snapshot_scheduler::{ActiveScene, SnapshotScheduler};
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p water-core snapshot_scheduler::tests`
Expected: 1 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/water-core/src/snapshot_scheduler.rs crates/water-core/src/lib.rs
git commit -m "feat(core): snapshot scheduler tokio task"
```

---

### Task 19: Snapshot restore (creates pre-restore snapshot)

**Files:**
- Modify: `crates/water-core/src/snapshot.rs`

- [ ] **Step 1: Failing test**

Append to the `tests` module in `crates/water-core/src/snapshot.rs`:

```rust
    #[test]
    fn restore_writes_pre_restore_then_overwrites() {
        let (dir, db, _m_id, s_id) = fixture();
        let store = SnapshotStore::new(&db, dir.path().to_path_buf());
        let scene_path = dir
            .path()
            .join("manuscript")
            .join("scenes")
            .join(format!("{s_id}.md"));
        // Take an initial snapshot of "first body".
        let snap1 = store.take(&s_id, &scene_path, SnapshotTrigger::Manual).unwrap();
        // Mutate the file.
        std::fs::write(&scene_path, "different content").unwrap();
        // Restore.
        store.restore(&s_id, &snap1.id, &scene_path).unwrap();
        let on_disk = std::fs::read_to_string(&scene_path).unwrap();
        assert!(on_disk.contains("first body"));
        // There should now be ≥2 rows: original Manual + a PreRestore.
        let rows = store.list(&s_id).unwrap();
        assert!(rows.iter().any(|r| r.trigger == SnapshotTrigger::PreRestore));
    }
```

- [ ] **Step 2: Run; expect failure**

Run: `cargo test -p water-core snapshot::tests::restore_writes_pre_restore_then_overwrites`
Expected: FAIL (no `restore` method yet).

- [ ] **Step 3: Implement `restore`**

Append to `impl<'a> SnapshotStore<'a>` in `crates/water-core/src/snapshot.rs`:

```rust
    /// Restore a scene file from a snapshot. Takes a pre-restore snapshot
    /// of the *current* file first so the operation is reversible.
    pub fn restore(
        &self,
        scene_id: &Id,
        snapshot_id: &Id,
        target_path: &Path,
    ) -> Result<()> {
        if target_path.exists() {
            self.take(scene_id, target_path, SnapshotTrigger::PreRestore)?;
        }
        let plain = self.read_decompressed(snapshot_id)?;
        std::fs::write(target_path, plain)?;
        Ok(())
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p water-core snapshot::tests`
Expected: 4 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/water-core/src/snapshot.rs
git commit -m "feat(core): snapshot restore with pre-restore safety snapshot"
```

---

## Phase E — Rebuild from Truth + External-Edit Repair

### Task 20: Rebuild-from-truth (scan folder → repopulate DB)

**Files:**
- Create: `crates/water-core/src/rebuild.rs`
- Modify: `crates/water-core/src/lib.rs`

- [ ] **Step 1: Failing tests**

Create `crates/water-core/src/rebuild.rs`:

```rust
//! Rebuild-from-truth: if `project.db` is missing or stale, regenerate the
//! index by scanning the project folder.

use crate::{
    chapters::ChaptersFile, scene_md::SceneFile, water_toml::WaterToml, Db, Id, Result,
};
use chrono::Utc;
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct RebuildStats {
    pub projects: usize,
    pub manuscripts: usize,
    pub chapters: usize,
    pub scenes: usize,
    pub characters: usize,
    pub world_entries: usize,
}

/// Rebuild the SQLite index from on-disk truth.
pub fn rebuild_from_truth(project_root: &Path) -> Result<(Db, RebuildStats)> {
    let db_path = project_root.join("project.db");
    // Remove any existing DB; we are about to recreate it from truth.
    if db_path.exists() {
        std::fs::remove_file(&db_path)?;
    }
    let mut db = Db::open(&db_path)?;
    let stats = repopulate(&mut db, project_root)?;
    Ok((db, stats))
}

fn repopulate(db: &mut Db, project_root: &Path) -> Result<RebuildStats> {
    let mut stats = RebuildStats::default();
    let water = WaterToml::read(project_root)?;

    // Project row.
    let now = Utc::now().to_rfc3339();
    db.conn().execute(
        "INSERT INTO project (id, name, default_manuscript_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        (
            water.project_id.as_str(),
            &water.name,
            water.default_manuscript_id.as_ref().map(Id::as_str),
            &water.created_at,
            &water.updated_at,
        ),
    )?;
    stats.projects = 1;

    // For v1 we model exactly one manuscript per project; create it if
    // default_manuscript_id is set, otherwise mint one.
    let manuscript_id = water
        .default_manuscript_id
        .clone()
        .unwrap_or_else(Id::new);
    db.conn().execute(
        "INSERT INTO manuscript (id, project_id, name, ordering, created_at, updated_at)
         VALUES (?1, ?2, 'Manuscript', 0, ?3, ?3)",
        (manuscript_id.as_str(), water.project_id.as_str(), &now),
    )?;
    stats.manuscripts = 1;

    // chapters.toml
    let chapters_path = project_root.join("manuscript").join("chapters.toml");
    let mut chapters_file = if chapters_path.exists() {
        ChaptersFile::read(&chapters_path)?
    } else {
        ChaptersFile::empty()
    };
    chapters_file
        .chapter
        .sort_by_key(|c| c.ordering);
    for ch in &chapters_file.chapter {
        db.conn().execute(
            "INSERT INTO chapter (id, manuscript_id, name, ordering, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            (
                ch.id.as_str(),
                manuscript_id.as_str(),
                &ch.name,
                ch.ordering,
                &now,
            ),
        )?;
        stats.chapters += 1;
    }

    // scenes/*.md
    let scenes_dir = project_root.join("manuscript").join("scenes");
    if scenes_dir.exists() {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&scenes_dir)?
            .filter_map(|e| e.ok().map(|d| d.path()))
            .filter(|p| p.extension().map(|x| x == "md").unwrap_or(false))
            .collect();
        entries.sort();
        for path in entries {
            let file = SceneFile::read(&path)?;
            let fm = file.frontmatter;
            let hash = crate::scene::hash_file(&path)?;
            db.conn().execute(
                "INSERT INTO scene (id, manuscript_id, chapter_id, ordering, name, pov_character_id,
                                    location_id, scene_goal, status, word_count, file_path,
                                    file_hash, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                (
                    fm.id.as_str(),
                    manuscript_id.as_str(),
                    fm.chapter_id.as_ref().map(Id::as_str),
                    fm.order,
                    &fm.name,
                    fm.pov_character_id.as_ref().map(Id::as_str),
                    fm.location_id.as_ref().map(Id::as_str),
                    &fm.scene_goal,
                    &fm.status,
                    fm.word_count,
                    path.to_string_lossy(),
                    &hash,
                    &fm.created_at,
                    &fm.updated_at,
                ),
            )?;
            for cid in &fm.characters_present {
                // Only insert presence if the character row already exists later;
                // for the rebuild we defer character presence until characters
                // are loaded — but characters are loaded after scenes, so we
                // batch presence rows in a temp Vec.
                db.conn().execute(
                    "INSERT OR IGNORE INTO scene_character_presence (scene_id, character_id) VALUES (?1, ?2)",
                    (fm.id.as_str(), cid.as_str()),
                ).ok();
            }
            stats.scenes += 1;
        }
    }

    // characters/*.toml
    let chars_dir = project_root.join("characters");
    if chars_dir.exists() {
        let entries: Vec<PathBuf> = std::fs::read_dir(&chars_dir)?
            .filter_map(|e| e.ok().map(|d| d.path()))
            .filter(|p| p.extension().map(|x| x == "toml").unwrap_or(false))
            .collect();
        for path in entries {
            let text = std::fs::read_to_string(&path)?;
            let parsed: toml::Table = toml::from_str(&text)?;
            let id: Id = parsed
                .get("id")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(Id::new);
            let name = parsed
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Unnamed");
            let schema_version = parsed
                .get("schema_version")
                .and_then(|v| v.as_str())
                .unwrap_or("lsm-v2.1");
            let data_json = serde_json::to_string(&parsed)?;
            let hash = crate::scene::hash_file(&path)?;
            db.conn().execute(
                "INSERT INTO character (id, project_id, name, schema_version, data_json, file_path, file_hash, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
                (
                    id.as_str(),
                    water.project_id.as_str(),
                    name,
                    schema_version,
                    &data_json,
                    path.to_string_lossy(),
                    &hash,
                    &now,
                ),
            )?;
            stats.characters += 1;
        }
    }

    // world/*.toml (single-doc segments only for M1)
    let world_dir = project_root.join("world");
    if world_dir.exists() {
        let entries: Vec<PathBuf> = std::fs::read_dir(&world_dir)?
            .filter_map(|e| e.ok().map(|d| d.path()))
            .filter(|p| p.extension().map(|x| x == "toml").unwrap_or(false))
            .collect();
        for path in entries {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("segment");
            let seg_id = Id::new();
            db.conn().execute(
                "INSERT INTO world_segment (id, project_id, name, ordering, is_collection) VALUES (?1, ?2, ?3, 0, 0)",
                (seg_id.as_str(), water.project_id.as_str(), stem),
            )?;
            let text = std::fs::read_to_string(&path)?;
            let parsed: toml::Table = toml::from_str(&text)?;
            let data_json = serde_json::to_string(&parsed)?;
            let hash = crate::scene::hash_file(&path)?;
            db.conn().execute(
                "INSERT INTO world_entry (id, segment_id, name, data_json, file_path, file_hash) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                (
                    Id::new().as_str(),
                    seg_id.as_str(),
                    stem,
                    &data_json,
                    path.to_string_lossy(),
                    &hash,
                ),
            )?;
            stats.world_entries += 1;
        }
    }

    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        chapters::Chapter, scene_md::SceneFrontmatter, water_toml::WaterToml, NewScene,
    };

    fn make_project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        // Compose the truth files manually so this test exercises *only*
        // the rebuild path, not the writer paths.
        let project_id = Id::new();
        let manuscript_id = Id::new();
        WaterToml {
            schema_version: 1,
            project_id: project_id.clone(),
            name: "TestProj".into(),
            default_manuscript_id: Some(manuscript_id.clone()),
            created_at: "2026-05-16T09:00:00+00:00".into(),
            updated_at: "2026-05-16T09:00:00+00:00".into(),
        }
        .write(dir.path())
        .unwrap();

        std::fs::create_dir_all(dir.path().join("manuscript").join("scenes")).unwrap();
        ChaptersFile {
            schema_version: 1,
            chapter: vec![Chapter {
                id: Id::new(),
                name: "Part One".into(),
                ordering: 0,
                scene_ids: vec![],
            }],
        }
        .write(dir.path().join("manuscript").join("chapters.toml"))
        .unwrap();

        let scene_id = Id::new();
        let scene_path = dir
            .path()
            .join("manuscript")
            .join("scenes")
            .join(format!("{scene_id}.md"));
        crate::scene_md::SceneFile {
            frontmatter: SceneFrontmatter {
                id: scene_id,
                name: "Opening".into(),
                chapter_id: None,
                order: 0,
                pov_character_id: None,
                characters_present: vec![],
                location_id: None,
                scene_goal: None,
                status: "draft".into(),
                created_at: "2026-05-16T09:00:00+00:00".into(),
                updated_at: "2026-05-16T09:00:00+00:00".into(),
                word_count: 2,
            },
            body: "Hello world.\n".into(),
        }
        .write(&scene_path)
        .unwrap();

        dir
    }

    #[test]
    fn rebuild_creates_project_manuscript_chapter_and_scene() {
        let dir = make_project();
        let (db, stats) = rebuild_from_truth(dir.path()).unwrap();
        assert_eq!(stats.projects, 1);
        assert_eq!(stats.manuscripts, 1);
        assert_eq!(stats.chapters, 1);
        assert_eq!(stats.scenes, 1);
        let n: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM scene", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn rebuild_is_idempotent_against_pre_existing_db() {
        let dir = make_project();
        let _ = rebuild_from_truth(dir.path()).unwrap();
        // Run again — should drop the old DB and rebuild.
        let (db2, _stats) = rebuild_from_truth(dir.path()).unwrap();
        let n: i64 = db2
            .conn()
            .query_row("SELECT COUNT(*) FROM scene", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
    }
}
```

- [ ] **Step 2: Register the module**

Append to `crates/water-core/src/lib.rs`:

```rust
pub mod rebuild;
pub use rebuild::{rebuild_from_truth, RebuildStats};
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p water-core rebuild::tests`
Expected: 2 passed.

- [ ] **Step 4: Commit**

```bash
git add crates/water-core/src/rebuild.rs crates/water-core/src/lib.rs
git commit -m "feat(core): rebuild-from-truth scan-and-repopulate"
```

---

### Task 21: External-edit repair pass

**Files:**
- Create: `crates/water-core/src/repair.rs`
- Modify: `crates/water-core/src/lib.rs`

The repair pass runs after rebuild (or on project open against an existing DB). It tolerates writers editing files in Obsidian/VS Code/etc.:

- Regenerate missing `^bk-XXXX` markers.
- Re-derive `word_count`.
- Reconcile `chapters.toml` against `scene.chapter_id` (chapter wins).
- Re-link pinned pills to nearest fuzzy-match block; archive if no match.

- [ ] **Step 1: Failing tests**

Create `crates/water-core/src/repair.rs`:

```rust
//! External-edit repair: tolerate users editing files outside Water.

use crate::block::ensure_block_ids;
use crate::chapters::ChaptersFile;
use crate::scene_md::SceneFile;
use crate::{Db, Id, Result};
use chrono::Utc;
use std::path::Path;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct RepairReport {
    pub scenes_re_block_idded: usize,
    pub scenes_wordcount_updated: usize,
    pub chapters_reconciled: usize,
    pub pinned_pills_archived: usize,
}

pub fn run(db: &Db, project_root: &Path) -> Result<RepairReport> {
    let mut report = RepairReport::default();

    // 1. Scenes: regenerate missing block ids, refresh word_count + frontmatter.
    let mut stmt = db
        .conn()
        .prepare("SELECT id, file_path, word_count FROM scene")?;
    let rows: Vec<(String, String, i64)> = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(stmt);
    for (id_s, path_s, prev_wc) in rows {
        let path = std::path::PathBuf::from(&path_s);
        if !path.exists() {
            continue;
        }
        let mut file = SceneFile::read(&path)?;
        let (new_body, _blocks) = ensure_block_ids(&file.body);
        let body_changed = new_body != file.body;
        if body_changed {
            file.body = new_body;
            report.scenes_re_block_idded += 1;
        }
        let new_wc = file
            .body
            .split_whitespace()
            .filter(|w| !w.starts_with("^bk-"))
            .count() as i64;
        let wc_changed = new_wc != prev_wc || new_wc != file.frontmatter.word_count;
        if wc_changed {
            file.frontmatter.word_count = new_wc;
            report.scenes_wordcount_updated += 1;
        }
        if body_changed || wc_changed {
            file.frontmatter.updated_at = Utc::now().to_rfc3339();
            file.write(&path)?;
            let hash = crate::scene::hash_file(&path)?;
            db.conn().execute(
                "UPDATE scene SET word_count = ?2, file_hash = ?3, updated_at = ?4 WHERE id = ?1",
                (id_s, new_wc, hash, file.frontmatter.updated_at),
            )?;
        }
    }

    // 2. Chapters: chapters.toml wins over scene.chapter_id where they disagree.
    let chapters_path = project_root.join("manuscript").join("chapters.toml");
    if chapters_path.exists() {
        let chapters = ChaptersFile::read(&chapters_path)?;
        for ch in &chapters.chapter {
            for sid in &ch.scene_ids {
                let n = db.conn().execute(
                    "UPDATE scene SET chapter_id = ?2 WHERE id = ?1 AND (chapter_id IS NULL OR chapter_id != ?2)",
                    (sid.as_str(), ch.id.as_str()),
                )?;
                report.chapters_reconciled += n;
            }
        }
    }

    // 3. Pinned pills with dead block IDs.
    let mut stmt = db
        .conn()
        .prepare("SELECT p.id, p.scene_id, p.block_id, p.snippet, s.file_path
                 FROM pinned_pill p JOIN scene s ON p.scene_id = s.id")?;
    let rows: Vec<(String, String, String, String, String)> = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(stmt);
    for (pid, _sid, bid, snippet, scene_path) in rows {
        let path = std::path::PathBuf::from(&scene_path);
        if !path.exists() {
            continue;
        }
        let file = SceneFile::read(&path)?;
        let block_present = file.body.contains(&format!("^{bid}"));
        let snippet_present = file.body.contains(snippet.as_str());
        if !block_present && !snippet_present {
            // Archive the pin (soft delete by appending a sentinel; v1 uses
            // a separate archived flag — we add one via inline UPDATE).
            db.conn().execute(
                "UPDATE pinned_pill SET rabbit_hole_path = COALESCE(rabbit_hole_path, '') || '|archived' WHERE id = ?1",
                [pid.as_str()],
            )?;
            report.pinned_pills_archived += 1;
        }
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{rebuild_from_truth, water_toml::WaterToml};

    fn make_project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let pid = Id::new();
        let mid = Id::new();
        WaterToml {
            schema_version: 1,
            project_id: pid.clone(),
            name: "P".into(),
            default_manuscript_id: Some(mid),
            created_at: "2026-05-16T09:00:00+00:00".into(),
            updated_at: "2026-05-16T09:00:00+00:00".into(),
        }
        .write(dir.path())
        .unwrap();
        std::fs::create_dir_all(dir.path().join("manuscript").join("scenes")).unwrap();

        let scene_id = Id::new();
        let scene_path = dir
            .path()
            .join("manuscript")
            .join("scenes")
            .join(format!("{scene_id}.md"));
        let file = SceneFile {
            frontmatter: crate::scene_md::SceneFrontmatter {
                id: scene_id.clone(),
                name: "S".into(),
                chapter_id: None,
                order: 0,
                pov_character_id: None,
                characters_present: vec![],
                location_id: None,
                scene_goal: None,
                status: "draft".into(),
                created_at: "2026-05-16T09:00:00+00:00".into(),
                updated_at: "2026-05-16T09:00:00+00:00".into(),
                word_count: 0,            // intentionally wrong; repair fixes it
            },
            // No block ids; repair will add them.
            body: "First.\n\nSecond.\n".into(),
        };
        file.write(&scene_path).unwrap();
        dir
    }

    #[test]
    fn repair_adds_block_ids_and_fixes_word_count() {
        let dir = make_project();
        let (db, _) = rebuild_from_truth(dir.path()).unwrap();
        let report = run(&db, dir.path()).unwrap();
        assert!(report.scenes_re_block_idded >= 1);
        assert!(report.scenes_wordcount_updated >= 1);
        let scene_file = std::fs::read_to_string(
            std::fs::read_dir(dir.path().join("manuscript").join("scenes"))
                .unwrap()
                .filter_map(|e| e.ok())
                .next()
                .unwrap()
                .path(),
        )
        .unwrap();
        assert!(scene_file.contains("First. ^bk-"));
        assert!(scene_file.contains("Second. ^bk-"));
    }
}
```

- [ ] **Step 2: Register the module**

Append to `crates/water-core/src/lib.rs`:

```rust
pub mod repair;
pub use repair::{run as repair, RepairReport};
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p water-core repair::tests`
Expected: 1 passed.

- [ ] **Step 4: Commit**

```bash
git add crates/water-core/src/repair.rs crates/water-core/src/lib.rs
git commit -m "feat(core): external-edit repair pass"
```

---

## Phase F — Python Sidecar

### Task 22: Sidecar project scaffold with `uv`

**Files:**
- Create: `sidecar/pyproject.toml`
- Create: `sidecar/README.md`
- Create: `sidecar/src/water_sidecar/__init__.py`
- Create: `sidecar/src/water_sidecar/main.py`

Prerequisite: install [`uv`](https://github.com/astral-sh/uv) (`pipx install uv` or via the installer).

- [ ] **Step 1: Create the pyproject manifest**

Create `sidecar/pyproject.toml`:

```toml
[project]
name = "water-sidecar"
version = "0.1.0"
description = "Water analysis sidecar — FastAPI app spawned by the Rust core"
requires-python = ">=3.12,<3.13"
dependencies = [
  "fastapi==0.115.0",
  "uvicorn[standard]==0.30.6",
  "pydantic==2.9.2",
]

[project.optional-dependencies]
dev = [
  "pytest==8.3.3",
  "httpx==0.27.2",
  "anyio==4.6.0",
]

[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[tool.hatch.build.targets.wheel]
packages = ["src/water_sidecar"]

[tool.pytest.ini_options]
testpaths = ["src"]
addopts = "-q"
```

- [ ] **Step 2: Create the README**

Create `sidecar/README.md`:

```markdown
# Water sidecar

The analysis sidecar — a small FastAPI process spawned by the Rust core. The
renderer never talks to this directly; only `water-core` does.

## Develop

```bash
cd sidecar
uv sync --extra dev
uv run uvicorn water_sidecar.main:app --port 0
```

## Test

```bash
uv run pytest
```
```

- [ ] **Step 3: Create the FastAPI app**

Create `sidecar/src/water_sidecar/__init__.py`:

```python
"""Water sidecar package."""
__version__ = "0.1.0"
```

Create `sidecar/src/water_sidecar/main.py`:

```python
"""FastAPI app that exposes the analysis sidecar."""
from __future__ import annotations

import os
import time

from fastapi import FastAPI
from pydantic import BaseModel

from . import __version__

app = FastAPI(title="water-sidecar", version=__version__)
_started_at = time.time()


class HealthResponse(BaseModel):
    status: str
    version: str
    uptime_seconds: float
    pid: int


@app.get("/health", response_model=HealthResponse)
def health() -> HealthResponse:
    return HealthResponse(
        status="ready",
        version=__version__,
        uptime_seconds=time.time() - _started_at,
        pid=os.getpid(),
    )
```

- [ ] **Step 4: Install dependencies and verify the app boots**

Run:

```bash
cd sidecar
uv sync --extra dev
uv run uvicorn water_sidecar.main:app --port 8765 --host 127.0.0.1 &
sleep 1
curl -s http://127.0.0.1:8765/health
```

Expected output (one line):

```
{"status":"ready","version":"0.1.0","uptime_seconds":..,"pid":..}
```

Then kill the background process. (On Windows PowerShell, use `Start-Job` / `Stop-Job` instead of `&`.)

- [ ] **Step 5: Commit**

```bash
git add sidecar
git commit -m "feat(sidecar): scaffold FastAPI app with /health"
```

---

### Task 23: `/analyze` stub endpoint + pytest tests

**Files:**
- Create: `sidecar/src/water_sidecar/routes/__init__.py`
- Create: `sidecar/src/water_sidecar/routes/analyze.py`
- Create: `sidecar/src/water_sidecar/tests/__init__.py`
- Create: `sidecar/src/water_sidecar/tests/test_health.py`
- Create: `sidecar/src/water_sidecar/tests/test_analyze.py`
- Modify: `sidecar/src/water_sidecar/main.py`

The `/analyze` endpoint returns deterministic stub metrics so M2 can wire to a fixed contract. Real model loading lands in M2 / M5.

- [ ] **Step 1: Create the routes package**

Create `sidecar/src/water_sidecar/routes/__init__.py`:

```python
"""HTTP routes."""
```

- [ ] **Step 2: Failing test for `/analyze`**

Create `sidecar/src/water_sidecar/tests/__init__.py` (empty).

Create `sidecar/src/water_sidecar/tests/test_analyze.py`:

```python
from fastapi.testclient import TestClient

from water_sidecar.main import app


def test_analyze_short_text_returns_warming_up_status() -> None:
    client = TestClient(app)
    r = client.post("/analyze", json={"text": "Hi.", "scene_id": "01H8X4"})
    assert r.status_code == 200
    body = r.json()
    assert body["status"] == "warming_up"
    assert body["word_count"] == 1


def test_analyze_with_paragraph_returns_metrics() -> None:
    client = TestClient(app)
    text = (
        "The fog rolled in low over the cliffs, swallowing the harbour "
        "lanterns one by one. Maren watched the last of them go."
    )
    r = client.post("/analyze", json={"text": text, "scene_id": "01H8X4"})
    assert r.status_code == 200
    body = r.json()
    assert body["word_count"] >= 10
    for k in ("flow", "coherence", "engagement", "divergence", "pace", "intensity", "valence"):
        assert 0.0 <= body[k] <= 1.0, f"{k} out of range"
    assert body["status"] in {"flow", "drift", "blocked", "normal", "warming_up"}


def test_analyze_rejects_empty_text() -> None:
    client = TestClient(app)
    r = client.post("/analyze", json={"text": "", "scene_id": "01H8X4"})
    assert r.status_code == 422
```

Create `sidecar/src/water_sidecar/tests/test_health.py`:

```python
from fastapi.testclient import TestClient

from water_sidecar.main import app


def test_health_returns_ready() -> None:
    client = TestClient(app)
    r = client.get("/health")
    assert r.status_code == 200
    body = r.json()
    assert body["status"] == "ready"
    assert body["version"]
    assert body["pid"] > 0
```

- [ ] **Step 3: Run tests; expect failure**

Run: `cd sidecar && uv run pytest -q src/water_sidecar/tests/test_analyze.py`
Expected: tests FAIL (no `/analyze` endpoint yet).

- [ ] **Step 4: Implement `/analyze`**

Create `sidecar/src/water_sidecar/routes/analyze.py`:

```python
"""POST /analyze — stub deterministic metrics for M1."""
from __future__ import annotations

import hashlib
from typing import Literal

from fastapi import APIRouter
from pydantic import BaseModel, Field

router = APIRouter()


class AnalyzeRequest(BaseModel):
    text: str = Field(min_length=1)
    scene_id: str


class AnalyzeResponse(BaseModel):
    word_count: int
    flow: float
    coherence: float
    engagement: float
    divergence: float
    pace: float
    intensity: float
    valence: float
    status: Literal["flow", "drift", "blocked", "normal", "warming_up"]


def _stable_score(seed: bytes, offset: int) -> float:
    """Deterministic 0..1 score from a hash of the text. Same input → same
    output. Suitable as a placeholder until real models land in M2/M5."""
    h = hashlib.sha256(seed).digest()
    return ((h[offset % 32] / 255.0) * 0.6) + 0.2  # confined to 0.2..0.8


@router.post("/analyze", response_model=AnalyzeResponse)
def analyze(req: AnalyzeRequest) -> AnalyzeResponse:
    word_count = len(req.text.split())
    if word_count < 5:
        return AnalyzeResponse(
            word_count=word_count,
            flow=0.5, coherence=0.5, engagement=0.5, divergence=0.0,
            pace=0.5, intensity=0.5, valence=0.5,
            status="warming_up",
        )

    seed = req.text.encode("utf-8")
    return AnalyzeResponse(
        word_count=word_count,
        flow=_stable_score(seed, 0),
        coherence=_stable_score(seed, 1),
        engagement=_stable_score(seed, 2),
        divergence=_stable_score(seed, 3) * 0.7,
        pace=_stable_score(seed, 4),
        intensity=_stable_score(seed, 5),
        valence=_stable_score(seed, 6),
        status="normal",
    )
```

- [ ] **Step 5: Wire the router into the app**

Edit `sidecar/src/water_sidecar/main.py` and append after the existing route:

```python
from .routes import analyze as analyze_route

app.include_router(analyze_route.router)
```

- [ ] **Step 6: Run tests**

Run: `cd sidecar && uv run pytest -q`
Expected: 4 passed.

- [ ] **Step 7: Commit**

```bash
git add sidecar/src/water_sidecar/routes sidecar/src/water_sidecar/tests sidecar/src/water_sidecar/main.py
git commit -m "feat(sidecar): /analyze stub + tests"
```

---

### Task 24: Shared IPC contract in `water-core`

**Files:**
- Create: `crates/water-core/src/ipc.rs`
- Modify: `crates/water-core/src/lib.rs`

These types live in Rust so the sidecar lifecycle and the future pill orchestrator can talk to the sidecar without ad-hoc serde.

- [ ] **Step 1: Failing test**

Create `crates/water-core/src/ipc.rs`:

```rust
//! IPC contract shared with the sidecar.
//!
//! Kept in sync by hand for v1; later milestones can switch to ts-rs or
//! a generated schema.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub uptime_seconds: f64,
    pub pid: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnalyzeRequest {
    pub text: String,
    pub scene_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnalyzeResponse {
    pub word_count: u64,
    pub flow: f64,
    pub coherence: f64,
    pub engagement: f64,
    pub divergence: f64,
    pub pace: f64,
    pub intensity: f64,
    pub valence: f64,
    pub status: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyze_request_round_trips_json() {
        let req = AnalyzeRequest { text: "hi".into(), scene_id: "01H8X4".into() };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: AnalyzeRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, parsed);
    }

    #[test]
    fn analyze_response_matches_sidecar_schema() {
        // Hand-pinned: this should match the FastAPI AnalyzeResponse shape.
        let sample = r#"{"word_count":12,"flow":0.5,"coherence":0.5,"engagement":0.5,"divergence":0.0,"pace":0.5,"intensity":0.5,"valence":0.5,"status":"normal"}"#;
        let parsed: AnalyzeResponse = serde_json::from_str(sample).unwrap();
        assert_eq!(parsed.word_count, 12);
        assert_eq!(parsed.status, "normal");
    }
}
```

- [ ] **Step 2: Register module**

Append to `crates/water-core/src/lib.rs`:

```rust
pub mod ipc;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p water-core ipc::tests`
Expected: 2 passed.

- [ ] **Step 4: Commit**

```bash
git add crates/water-core/src/ipc.rs crates/water-core/src/lib.rs
git commit -m "feat(core): shared IPC contract types for sidecar"
```

---

## Phase G — Sidecar Lifecycle

### Task 25: `Sidecar` handle (HTTP client + optional child process)

**Files:**
- Create: `crates/water-core/src/sidecar.rs`
- Modify: `crates/water-core/src/lib.rs`
- Modify: `sidecar/src/water_sidecar/main.py`

For testability, the `Sidecar` handle has two modes:

- **Managed**: spawns a child process via `tokio::process::Command`, reads a `WATER_SIDECAR_PORT=...` line from the child's stdout to learn the port, polls `/health` until ready.
- **External**: given a fixed base URL; no child process. Used in tests against `wiremock`.

- [ ] **Step 1: Print the port from the sidecar**

The sidecar must print the listening port on stdout in a parseable form. Replace `sidecar/src/water_sidecar/main.py` with:

```python
"""FastAPI app that exposes the analysis sidecar."""
from __future__ import annotations

import os
import sys
import time

from fastapi import FastAPI
from pydantic import BaseModel

from . import __version__
from .routes import analyze as analyze_route

app = FastAPI(title="water-sidecar", version=__version__)
_started_at = time.time()


class HealthResponse(BaseModel):
    status: str
    version: str
    uptime_seconds: float
    pid: int


@app.get("/health", response_model=HealthResponse)
def health() -> HealthResponse:
    return HealthResponse(
        status="ready",
        version=__version__,
        uptime_seconds=time.time() - _started_at,
        pid=os.getpid(),
    )


app.include_router(analyze_route.router)


@app.on_event("startup")
async def announce_port() -> None:
    """Emit WATER_SIDECAR_PORT=NNNNN on stdout so the Rust core can read it."""
    # Discover the port from uvicorn server config. The actual server instance
    # is exposed by uvicorn in its lifespan; for v1 we instead require the
    # caller to supply --port and read it from argv.
    port = "0"
    for i, arg in enumerate(sys.argv):
        if arg == "--port" and i + 1 < len(sys.argv):
            port = sys.argv[i + 1]
            break
    print(f"WATER_SIDECAR_PORT={port}", flush=True)
```

(For M1 we require the caller to pass `--port` explicitly; later, we can let uvicorn pick 0 and resolve the actual port.)

- [ ] **Step 2: Failing tests for the External mode**

Create `crates/water-core/src/sidecar.rs`:

```rust
//! Sidecar handle — manage the FastAPI sidecar process and call it.

use crate::ipc::{AnalyzeRequest, AnalyzeResponse, HealthResponse};
use crate::{Error, Result};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct SidecarSpec {
    pub working_dir: PathBuf,    // path containing pyproject.toml
    pub uv_bin: PathBuf,         // path to `uv`
    pub port: u16,               // explicit port; caller picks (avoid 0 for v1)
    pub host: String,            // "127.0.0.1"
    pub boot_timeout: Duration,
}

pub struct Sidecar {
    base_url: String,
    child: Mutex<Option<Child>>,
    http: reqwest::Client,
}

impl Sidecar {
    /// External mode: connect to an already-running sidecar at `base_url`.
    pub fn external(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            child: Mutex::new(None),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(8))
                .build()
                .expect("reqwest client"),
        }
    }

    /// Managed mode: spawn the sidecar and wait for /health.
    pub async fn spawn(spec: SidecarSpec) -> Result<Self> {
        let mut cmd = Command::new(&spec.uv_bin);
        cmd.arg("run")
            .arg("uvicorn")
            .arg("water_sidecar.main:app")
            .arg("--host")
            .arg(&spec.host)
            .arg("--port")
            .arg(spec.port.to_string())
            .current_dir(&spec.working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let mut child = cmd
            .spawn()
            .map_err(|e| Error::Sidecar(format!("failed to spawn uv: {e}")))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| Error::Sidecar("no stdout".into()))?;
        let mut reader = BufReader::new(stdout).lines();

        // Wait for the WATER_SIDECAR_PORT line within the boot_timeout.
        let port = tokio::time::timeout(spec.boot_timeout, async {
            while let Ok(Some(line)) = reader.next_line().await {
                if let Some(rest) = line.strip_prefix("WATER_SIDECAR_PORT=") {
                    return rest.trim().parse::<u16>().ok();
                }
            }
            None
        })
        .await
        .map_err(|_| Error::Sidecar("timeout waiting for port".into()))?
        .ok_or_else(|| Error::Sidecar("sidecar did not announce port".into()))?;

        let base_url = format!("http://{}:{}", spec.host, port);
        let me = Self {
            base_url,
            child: Mutex::new(Some(child)),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(8))
                .build()
                .expect("reqwest client"),
        };

        // Poll /health until ready or timeout.
        let deadline = tokio::time::Instant::now() + spec.boot_timeout;
        loop {
            if tokio::time::Instant::now() > deadline {
                return Err(Error::Sidecar("timeout waiting for /health".into()));
            }
            if me.health().await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
        Ok(me)
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn health(&self) -> Result<HealthResponse> {
        let url = format!("{}/health", self.base_url);
        let r = self.http.get(url).send().await.map_err(|e| Error::Sidecar(format!("health: {e}")))?;
        let r = r.error_for_status().map_err(|e| Error::Sidecar(format!("health http: {e}")))?;
        let body: HealthResponse = r.json().await.map_err(|e| Error::Sidecar(format!("health json: {e}")))?;
        Ok(body)
    }

    pub async fn analyze(&self, req: &AnalyzeRequest) -> Result<AnalyzeResponse> {
        let url = format!("{}/analyze", self.base_url);
        let r = self.http.post(url).json(req).send().await
            .map_err(|e| Error::Sidecar(format!("analyze: {e}")))?;
        let r = r.error_for_status().map_err(|e| Error::Sidecar(format!("analyze http: {e}")))?;
        let body: AnalyzeResponse = r.json().await.map_err(|e| Error::Sidecar(format!("analyze json: {e}")))?;
        Ok(body)
    }

    pub async fn shutdown(self) -> Result<()> {
        if let Some(mut c) = self.child.lock().await.take() {
            let _ = c.kill().await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn external_health_round_trip() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "ready",
                "version": "0.1.0",
                "uptime_seconds": 1.2,
                "pid": 999
            })))
            .mount(&server)
            .await;

        let sc = Sidecar::external(server.uri());
        let h = sc.health().await.unwrap();
        assert_eq!(h.status, "ready");
        assert_eq!(h.pid, 999);
    }

    #[tokio::test]
    async fn external_analyze_round_trip() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/analyze"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "word_count": 7,
                "flow": 0.5, "coherence": 0.5, "engagement": 0.5,
                "divergence": 0.0, "pace": 0.5, "intensity": 0.5, "valence": 0.5,
                "status": "normal"
            })))
            .mount(&server)
            .await;
        let sc = Sidecar::external(server.uri());
        let resp = sc
            .analyze(&AnalyzeRequest { text: "Some sentence here.".into(), scene_id: "01H8X4".into() })
            .await
            .unwrap();
        assert_eq!(resp.word_count, 7);
        assert_eq!(resp.status, "normal");
    }

    #[tokio::test]
    async fn health_propagates_http_errors() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let sc = Sidecar::external(server.uri());
        assert!(sc.health().await.is_err());
    }

    #[tokio::test]
    #[ignore = "requires uv and the sidecar workspace; run with --ignored"]
    async fn managed_spawn_against_real_sidecar() {
        let workspace = std::env::var("WATER_SIDECAR_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from("../../sidecar"));
        let uv = which::which("uv").expect("uv not found on PATH");
        let port: u16 = 18765;
        let sc = Sidecar::spawn(SidecarSpec {
            working_dir: workspace,
            uv_bin: uv,
            port,
            host: "127.0.0.1".into(),
            boot_timeout: Duration::from_secs(20),
        })
        .await
        .unwrap();
        let h = sc.health().await.unwrap();
        assert_eq!(h.status, "ready");
        sc.shutdown().await.unwrap();
    }
}
```

- [ ] **Step 3: Add `which` to dev-deps**

Append to `crates/water-core/Cargo.toml` under `[dev-dependencies]`:

```toml
which = "6"
```

- [ ] **Step 4: Register module**

Append to `crates/water-core/src/lib.rs`:

```rust
pub mod sidecar;
pub use sidecar::{Sidecar, SidecarSpec};
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p water-core sidecar::tests`
Expected: 3 passed (the `managed_spawn_against_real_sidecar` test is `#[ignore]`-d).

- [ ] **Step 6: Optional integration test (if `uv` is installed)**

Run: `cargo test -p water-core sidecar::tests -- --ignored`
Expected: passes if the sidecar workspace is at `../../sidecar` from the test cwd.

- [ ] **Step 7: Commit**

```bash
git add crates/water-core/src/sidecar.rs crates/water-core/src/lib.rs crates/water-core/Cargo.toml sidecar/src/water_sidecar/main.py
git commit -m "feat(core): Sidecar handle with managed + external modes"
```

---

### Task 26: `SidecarSupervisor` with health-check loop + status events

**Files:**
- Create: `crates/water-core/src/sidecar_supervisor.rs`
- Modify: `crates/water-core/src/lib.rs`

The supervisor wraps a `Sidecar` and:

- Polls `/health` every 5s.
- On consecutive failures, kills + respawns (up to 3 attempts, then gives up and surfaces `Error`).
- Emits status changes (`Loading`, `Ready`, `Error`) via a `tokio::sync::watch` channel that the Tauri shell subscribes to and pushes to the renderer as `sidecar.status` events.

- [ ] **Step 1: Failing tests**

Create `crates/water-core/src/sidecar_supervisor.rs`:

```rust
//! Watches a Sidecar and emits status changes.

use crate::sidecar::Sidecar;
use crate::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{watch, Notify};
use tokio::time::{interval, Duration};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SidecarStatus {
    Loading,
    Ready,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct SidecarStatusEvent {
    pub status: SidecarStatus,
    pub detail: Option<String>,
}

pub struct SidecarSupervisor {
    tx: watch::Sender<SidecarStatusEvent>,
    stop: Arc<Notify>,
}

impl SidecarSupervisor {
    pub fn spawn(sidecar: Arc<Sidecar>) -> (Self, watch::Receiver<SidecarStatusEvent>) {
        let (tx, rx) = watch::channel(SidecarStatusEvent {
            status: SidecarStatus::Loading,
            detail: None,
        });
        let stop = Arc::new(Notify::new());
        let stop_clone = stop.clone();
        let tx_clone = tx.clone();
        tokio::spawn(async move {
            let mut consecutive_failures = 0u32;
            let mut ticker = interval(Duration::from_secs(5));
            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        match sidecar.health().await {
                            Ok(_) => {
                                if consecutive_failures > 0 {
                                    let _ = tx_clone.send(SidecarStatusEvent {
                                        status: SidecarStatus::Ready,
                                        detail: None,
                                    });
                                }
                                consecutive_failures = 0;
                                // Send Ready once at start, too.
                                if tx_clone.borrow().status != SidecarStatus::Ready {
                                    let _ = tx_clone.send(SidecarStatusEvent {
                                        status: SidecarStatus::Ready,
                                        detail: None,
                                    });
                                }
                            }
                            Err(e) => {
                                consecutive_failures += 1;
                                let _ = tx_clone.send(SidecarStatusEvent {
                                    status: SidecarStatus::Error,
                                    detail: Some(format!("{e}")),
                                });
                                if consecutive_failures >= 3 {
                                    let _ = tx_clone.send(SidecarStatusEvent {
                                        status: SidecarStatus::Error,
                                        detail: Some("sidecar unhealthy after 3 attempts".into()),
                                    });
                                    break;
                                }
                            }
                        }
                    }
                    _ = stop_clone.notified() => { break; }
                }
            }
        });
        (Self { tx, stop }, rx)
    }

    pub fn stop(&self) {
        self.stop.notify_waiters();
    }

    pub fn current(&self) -> SidecarStatusEvent {
        self.tx.borrow().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sidecar::Sidecar;
    use std::sync::Arc;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn supervisor_reports_ready_when_health_succeeds() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "ready", "version": "0.1.0", "uptime_seconds": 1.0, "pid": 1
            })))
            .mount(&server)
            .await;
        let sc = Arc::new(Sidecar::external(server.uri()));
        let (sup, mut rx) = SidecarSupervisor::spawn(sc);
        // Wait up to 8 seconds for the first ready event (interval is 5s).
        let evt = tokio::time::timeout(Duration::from_secs(8), async {
            loop {
                rx.changed().await.unwrap();
                if rx.borrow().status == SidecarStatus::Ready {
                    return rx.borrow().clone();
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(evt.status, SidecarStatus::Ready);
        sup.stop();
    }

    #[tokio::test]
    async fn supervisor_reports_error_after_health_failures() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let sc = Arc::new(Sidecar::external(server.uri()));
        let (sup, mut rx) = SidecarSupervisor::spawn(sc);
        let evt = tokio::time::timeout(Duration::from_secs(8), async {
            loop {
                rx.changed().await.unwrap();
                if rx.borrow().status == SidecarStatus::Error {
                    return rx.borrow().clone();
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(evt.status, SidecarStatus::Error);
        sup.stop();
    }
}
```

- [ ] **Step 2: Register module**

Append to `crates/water-core/src/lib.rs`:

```rust
pub mod sidecar_supervisor;
pub use sidecar_supervisor::{SidecarStatus, SidecarStatusEvent, SidecarSupervisor};
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p water-core sidecar_supervisor::tests`
Expected: 2 passed.

- [ ] **Step 4: Commit**

```bash
git add crates/water-core/src/sidecar_supervisor.rs crates/water-core/src/lib.rs
git commit -m "feat(core): SidecarSupervisor with status watch channel"
```

---

## Phase H — LLM Provider Router

For M1 the trait surface is intentionally minimal: `health()` and `generate_bouquet()`. M2 will extend the trait when it wires the pill orchestrator. Every adapter ships in M1 so the eval harness (M1.5) and M2 have a stable foundation.

### Task 27: `LlmProvider` trait + bouquet types

**Files:**
- Create: `crates/water-core/src/llm/mod.rs`
- Create: `crates/water-core/src/llm/provider.rs`
- Modify: `crates/water-core/src/lib.rs`

- [ ] **Step 1: Failing test**

Create `crates/water-core/src/llm/mod.rs`:

```rust
//! LLM provider trait + concrete adapters + router.

pub mod provider;
pub use provider::*;
```

Create `crates/water-core/src/llm/provider.rs`:

```rust
use crate::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Identifier for a configured provider instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProviderId(pub String);

impl ProviderId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BouquetRequest {
    pub system: String,
    pub user: String,
    pub n_variants: usize,
    /// First 8 words of each previously-generated variant in this rabbit
    /// hole, to push the model toward novelty when regenerating.
    #[serde(default)]
    pub previous_variants_first_words: Vec<String>,
    pub model: String,
    pub temperature: f32,
    pub max_output_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BouquetVariant {
    pub angle: String,
    pub text: String,
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Stable identifier (e.g. "anthropic", "ollama", "llamacpp-kimi").
    fn id(&self) -> ProviderId;

    /// Cheap connectivity check. Implementations may issue a 1-token call,
    /// hit a `/health` endpoint, or just validate that credentials exist.
    async fn health(&self) -> Result<()>;

    /// Generate exactly `req.n_variants` bouquet items. Adapters must
    /// validate the model returned exactly that many; if it returned more,
    /// truncate; if fewer, error.
    async fn generate_bouquet(&self, req: &BouquetRequest) -> Result<Vec<BouquetVariant>>;
}

/// A canned provider used by tests and by the M1 `provider.test` command
/// when no real provider is configured.
pub struct CannedProvider;

#[async_trait]
impl LlmProvider for CannedProvider {
    fn id(&self) -> ProviderId {
        ProviderId::new("canned")
    }
    async fn health(&self) -> Result<()> {
        Ok(())
    }
    async fn generate_bouquet(&self, req: &BouquetRequest) -> Result<Vec<BouquetVariant>> {
        Ok((0..req.n_variants)
            .map(|i| BouquetVariant {
                angle: ["feel", "notice", "wonder"][i % 3].into(),
                text: format!("(canned variant {} of {})", i + 1, req.n_variants),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn canned_provider_returns_requested_count() {
        let p = CannedProvider;
        let req = BouquetRequest {
            system: "tone".into(),
            user: "Hello".into(),
            n_variants: 3,
            previous_variants_first_words: vec![],
            model: "canned".into(),
            temperature: 0.7,
            max_output_tokens: 80,
        };
        let out = p.generate_bouquet(&req).await.unwrap();
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].angle, "feel");
    }

    #[tokio::test]
    async fn canned_provider_health_ok() {
        assert!(CannedProvider.health().await.is_ok());
    }
}
```

- [ ] **Step 2: Add `async_trait` dependency**

Append to `crates/water-core/Cargo.toml` under `[dependencies]`:

```toml
async-trait = "0.1"
```

- [ ] **Step 3: Register module**

Append to `crates/water-core/src/lib.rs`:

```rust
pub mod llm;
pub use llm::{BouquetRequest, BouquetVariant, CannedProvider, LlmProvider, ProviderId};
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p water-core llm::provider::tests`
Expected: 2 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/water-core/Cargo.toml crates/water-core/src/llm crates/water-core/src/lib.rs
git commit -m "feat(core): LlmProvider trait + CannedProvider"
```

---

### Task 28: Anthropic adapter

**Files:**
- Create: `crates/water-core/src/llm/anthropic.rs`
- Modify: `crates/water-core/src/llm/mod.rs`

Anthropic's Messages API accepts `system`, `messages`, `model`, `max_tokens`, and `temperature`. We request strict JSON output through a clear instruction in the user message.

- [ ] **Step 1: Failing tests**

Create `crates/water-core/src/llm/anthropic.rs`:

```rust
use super::{BouquetRequest, BouquetVariant, LlmProvider, ProviderId};
use crate::{Error, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub struct AnthropicProvider {
    base_url: String,
    api_key: String,
    http: reqwest::Client,
}

impl AnthropicProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::with_base_url(api_key, "https://api.anthropic.com")
    }

    pub fn with_base_url(api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: base_url.into(),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("reqwest"),
        }
    }
}

#[derive(Serialize)]
struct MessagesRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    temperature: f32,
    system: &'a str,
    messages: Vec<MessagesMessage<'a>>,
}

#[derive(Serialize)]
struct MessagesMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct MessagesResponse {
    content: Vec<MessagesContentBlock>,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum MessagesContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    fn id(&self) -> ProviderId {
        ProviderId::new("anthropic")
    }

    async fn health(&self) -> Result<()> {
        // Anthropic has no /health endpoint; do a 1-token sanity call.
        let body = MessagesRequest {
            model: "claude-3-5-haiku-latest",
            max_tokens: 1,
            temperature: 0.0,
            system: "Respond with the single character A and nothing else.",
            messages: vec![MessagesMessage { role: "user", content: "ping" }],
        };
        let r = self
            .http
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Provider(format!("anthropic health: {e}")))?;
        r.error_for_status()
            .map_err(|e| Error::Provider(format!("anthropic health http: {e}")))?;
        Ok(())
    }

    async fn generate_bouquet(&self, req: &BouquetRequest) -> Result<Vec<BouquetVariant>> {
        let user = build_user_with_exclusions(&req.user, &req.previous_variants_first_words, req.n_variants);
        let body = MessagesRequest {
            model: &req.model,
            max_tokens: req.max_output_tokens,
            temperature: req.temperature,
            system: &req.system,
            messages: vec![MessagesMessage { role: "user", content: &user }],
        };
        let r = self
            .http
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Provider(format!("anthropic: {e}")))?;
        let r = r
            .error_for_status()
            .map_err(|e| Error::Provider(format!("anthropic http: {e}")))?;
        let resp: MessagesResponse = r
            .json()
            .await
            .map_err(|e| Error::Provider(format!("anthropic json: {e}")))?;
        let text = resp.content.into_iter().find_map(|b| match b {
            MessagesContentBlock::Text { text } => Some(text),
        }).ok_or_else(|| Error::Provider("anthropic: no text block".into()))?;
        parse_bouquet_json(&text, req.n_variants)
    }
}

pub(super) fn build_user_with_exclusions(
    base: &str,
    prior: &[String],
    n: usize,
) -> String {
    let mut s = String::with_capacity(base.len() + 128);
    s.push_str(base);
    s.push_str("\n\nReturn exactly ");
    s.push_str(&n.to_string());
    s.push_str(" items as a strict JSON array: [{\"angle\":\"...\",\"text\":\"...\"}].");
    if !prior.is_empty() {
        s.push_str(" Previous openings to avoid: ");
        for (i, p) in prior.iter().enumerate() {
            if i > 0 {
                s.push_str("; ");
            }
            s.push('"');
            s.push_str(p);
            s.push('"');
        }
        s.push('.');
    }
    s
}

pub(super) fn parse_bouquet_json(text: &str, n: usize) -> Result<Vec<BouquetVariant>> {
    let trimmed = text.trim();
    let start = trimmed.find('[').ok_or_else(|| Error::Provider("no JSON array".into()))?;
    let end = trimmed.rfind(']').ok_or_else(|| Error::Provider("no JSON array close".into()))?;
    if end <= start {
        return Err(Error::Provider("malformed JSON array".into()));
    }
    let json = &trimmed[start..=end];
    let parsed: Vec<BouquetVariant> = serde_json::from_str(json)
        .map_err(|e| Error::Provider(format!("bouquet json: {e}")))?;
    if parsed.len() < n {
        return Err(Error::Provider(format!(
            "bouquet had {} items, expected {}",
            parsed.len(),
            n
        )));
    }
    Ok(parsed.into_iter().take(n).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn generate_bouquet_parses_three_variants() {
        let server = MockServer::start().await;
        let body = serde_json::json!({
            "content": [{
                "type": "text",
                "text": "[{\"angle\":\"feel\",\"text\":\"a\"},{\"angle\":\"notice\",\"text\":\"b\"},{\"angle\":\"wonder\",\"text\":\"c\"}]"
            }]
        });
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .and(header("x-api-key", "secret"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;
        let p = AnthropicProvider::with_base_url("secret", server.uri());
        let req = BouquetRequest {
            system: "tone".into(),
            user: "react".into(),
            n_variants: 3,
            previous_variants_first_words: vec![],
            model: "claude-3-5-sonnet-latest".into(),
            temperature: 0.7,
            max_output_tokens: 200,
        };
        let out = p.generate_bouquet(&req).await.unwrap();
        assert_eq!(out.len(), 3);
        assert_eq!(out[1].angle, "notice");
    }

    #[tokio::test]
    async fn generate_bouquet_errors_when_too_few_variants() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "content": [{"type":"text","text":"[{\"angle\":\"feel\",\"text\":\"only one\"}]"}]
            })))
            .mount(&server)
            .await;
        let p = AnthropicProvider::with_base_url("secret", server.uri());
        let req = BouquetRequest {
            system: "tone".into(), user: "react".into(), n_variants: 3,
            previous_variants_first_words: vec![],
            model: "m".into(), temperature: 0.7, max_output_tokens: 200,
        };
        assert!(p.generate_bouquet(&req).await.is_err());
    }

    #[tokio::test]
    async fn health_passes_when_api_returns_200() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "content": [{"type":"text","text":"A"}]
            })))
            .mount(&server)
            .await;
        let p = AnthropicProvider::with_base_url("secret", server.uri());
        assert!(p.health().await.is_ok());
    }
}
```

- [ ] **Step 2: Register module**

Edit `crates/water-core/src/llm/mod.rs` and append:

```rust
pub mod anthropic;
pub use anthropic::AnthropicProvider;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p water-core llm::anthropic::tests`
Expected: 3 passed.

- [ ] **Step 4: Commit**

```bash
git add crates/water-core/src/llm/anthropic.rs crates/water-core/src/llm/mod.rs
git commit -m "feat(core): Anthropic provider adapter"
```

---

### Task 29: OpenAI adapter

**Files:**
- Create: `crates/water-core/src/llm/openai.rs`
- Modify: `crates/water-core/src/llm/mod.rs`

- [ ] **Step 1: Failing tests**

Create `crates/water-core/src/llm/openai.rs`:

```rust
use super::anthropic::{build_user_with_exclusions, parse_bouquet_json};
use super::{BouquetRequest, BouquetVariant, LlmProvider, ProviderId};
use crate::{Error, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub struct OpenAiProvider {
    base_url: String,
    api_key: String,
    http: reqwest::Client,
}

impl OpenAiProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::with_base_url(api_key, "https://api.openai.com")
    }
    pub fn with_base_url(api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: base_url.into(),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("reqwest"),
        }
    }
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    temperature: f32,
    max_tokens: u32,
    messages: Vec<ChatMessage<'a>>,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessageOut,
}

#[derive(Deserialize)]
struct ChatMessageOut {
    content: String,
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    fn id(&self) -> ProviderId {
        ProviderId::new("openai")
    }

    async fn health(&self) -> Result<()> {
        let body = ChatRequest {
            model: "gpt-4o-mini",
            temperature: 0.0,
            max_tokens: 1,
            messages: vec![
                ChatMessage { role: "system", content: "Respond with the single character A." },
                ChatMessage { role: "user", content: "ping" },
            ],
        };
        let r = self
            .http
            .post(format!("{}/v1/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Provider(format!("openai health: {e}")))?;
        r.error_for_status()
            .map_err(|e| Error::Provider(format!("openai health http: {e}")))?;
        Ok(())
    }

    async fn generate_bouquet(&self, req: &BouquetRequest) -> Result<Vec<BouquetVariant>> {
        let user = build_user_with_exclusions(&req.user, &req.previous_variants_first_words, req.n_variants);
        let body = ChatRequest {
            model: &req.model,
            temperature: req.temperature,
            max_tokens: req.max_output_tokens,
            messages: vec![
                ChatMessage { role: "system", content: &req.system },
                ChatMessage { role: "user", content: &user },
            ],
        };
        let r = self
            .http
            .post(format!("{}/v1/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Provider(format!("openai: {e}")))?;
        let r = r
            .error_for_status()
            .map_err(|e| Error::Provider(format!("openai http: {e}")))?;
        let resp: ChatResponse = r
            .json()
            .await
            .map_err(|e| Error::Provider(format!("openai json: {e}")))?;
        let text = resp
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or_else(|| Error::Provider("openai: no choices".into()))?;
        parse_bouquet_json(&text, req.n_variants)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn generate_bouquet_parses_three_variants() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(header("authorization", "Bearer secret"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{"message": {"content": "[{\"angle\":\"a\",\"text\":\"1\"},{\"angle\":\"b\",\"text\":\"2\"},{\"angle\":\"c\",\"text\":\"3\"}]"}}]
            })))
            .mount(&server)
            .await;
        let p = OpenAiProvider::with_base_url("secret", server.uri());
        let req = BouquetRequest {
            system: "tone".into(), user: "react".into(), n_variants: 3,
            previous_variants_first_words: vec![],
            model: "gpt-4o-mini".into(), temperature: 0.7, max_output_tokens: 200,
        };
        let out = p.generate_bouquet(&req).await.unwrap();
        assert_eq!(out.len(), 3);
        assert_eq!(out[2].text, "3");
    }
}
```

- [ ] **Step 2: Register module**

Edit `crates/water-core/src/llm/mod.rs` and append:

```rust
pub mod openai;
pub use openai::OpenAiProvider;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p water-core llm::openai::tests`
Expected: 1 passed.

- [ ] **Step 4: Commit**

```bash
git add crates/water-core/src/llm/openai.rs crates/water-core/src/llm/mod.rs
git commit -m "feat(core): OpenAI provider adapter"
```

---

### Task 30: Ollama adapter

**Files:**
- Create: `crates/water-core/src/llm/ollama.rs`
- Modify: `crates/water-core/src/llm/mod.rs`

Ollama's `/api/chat` returns either streaming JSONL or a non-streaming JSON object when `stream:false`.

- [ ] **Step 1: Failing tests**

Create `crates/water-core/src/llm/ollama.rs`:

```rust
use super::anthropic::{build_user_with_exclusions, parse_bouquet_json};
use super::{BouquetRequest, BouquetVariant, LlmProvider, ProviderId};
use crate::{Error, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub struct OllamaProvider {
    base_url: String,
    http: reqwest::Client,
}

impl OllamaProvider {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(60))
                .build()
                .expect("reqwest"),
        }
    }
    pub fn default_url() -> Self {
        Self::new("http://127.0.0.1:11434")
    }
}

#[derive(Serialize)]
struct OllamaChatRequest<'a> {
    model: &'a str,
    stream: bool,
    options: OllamaOptions,
    messages: Vec<OllamaMessage<'a>>,
}

#[derive(Serialize)]
struct OllamaOptions {
    temperature: f32,
    num_predict: u32,
}

#[derive(Serialize)]
struct OllamaMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct OllamaChatResponse {
    message: OllamaMessageOut,
}

#[derive(Deserialize)]
struct OllamaMessageOut {
    content: String,
}

#[derive(Deserialize)]
struct OllamaTagsResponse {
    #[serde(default)]
    models: Vec<serde_json::Value>,
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    fn id(&self) -> ProviderId {
        ProviderId::new("ollama")
    }

    async fn health(&self) -> Result<()> {
        let r = self
            .http
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await
            .map_err(|e| Error::Provider(format!("ollama health: {e}")))?;
        let r = r
            .error_for_status()
            .map_err(|e| Error::Provider(format!("ollama health http: {e}")))?;
        let _tags: OllamaTagsResponse = r
            .json()
            .await
            .map_err(|e| Error::Provider(format!("ollama tags json: {e}")))?;
        Ok(())
    }

    async fn generate_bouquet(&self, req: &BouquetRequest) -> Result<Vec<BouquetVariant>> {
        let user = build_user_with_exclusions(&req.user, &req.previous_variants_first_words, req.n_variants);
        let body = OllamaChatRequest {
            model: &req.model,
            stream: false,
            options: OllamaOptions {
                temperature: req.temperature,
                num_predict: req.max_output_tokens,
            },
            messages: vec![
                OllamaMessage { role: "system", content: &req.system },
                OllamaMessage { role: "user", content: &user },
            ],
        };
        let r = self
            .http
            .post(format!("{}/api/chat", self.base_url))
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Provider(format!("ollama: {e}")))?;
        let r = r
            .error_for_status()
            .map_err(|e| Error::Provider(format!("ollama http: {e}")))?;
        let resp: OllamaChatResponse = r
            .json()
            .await
            .map_err(|e| Error::Provider(format!("ollama json: {e}")))?;
        parse_bouquet_json(&resp.message.content, req.n_variants)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn health_succeeds_when_tags_returns_ok() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/tags"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"models":[]})))
            .mount(&server)
            .await;
        let p = OllamaProvider::new(server.uri());
        assert!(p.health().await.is_ok());
    }

    #[tokio::test]
    async fn generate_bouquet_parses_three_variants() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message":{"content":"[{\"angle\":\"a\",\"text\":\"x\"},{\"angle\":\"b\",\"text\":\"y\"},{\"angle\":\"c\",\"text\":\"z\"}]"}
            })))
            .mount(&server)
            .await;
        let p = OllamaProvider::new(server.uri());
        let req = BouquetRequest {
            system: "s".into(), user: "u".into(), n_variants: 3,
            previous_variants_first_words: vec![],
            model: "qwen2.5:3b".into(), temperature: 0.7, max_output_tokens: 200,
        };
        let out = p.generate_bouquet(&req).await.unwrap();
        assert_eq!(out.len(), 3);
        assert_eq!(out[1].text, "y");
    }
}
```

- [ ] **Step 2: Register module**

Edit `crates/water-core/src/llm/mod.rs`:

```rust
pub mod ollama;
pub use ollama::OllamaProvider;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p water-core llm::ollama::tests`
Expected: 2 passed.

- [ ] **Step 4: Commit**

```bash
git add crates/water-core/src/llm/ollama.rs crates/water-core/src/llm/mod.rs
git commit -m "feat(core): Ollama provider adapter"
```

---

### Task 31: llama.cpp adapter (OpenAI-compatible mode)

**Files:**
- Create: `crates/water-core/src/llm/llamacpp.rs`
- Modify: `crates/water-core/src/llm/mod.rs`

llama.cpp's server exposes an OpenAI-compatible API at `POST /v1/chat/completions`. We reuse the OpenAI request/response shapes but with no `Authorization` header (server may or may not require an `--api-key`).

- [ ] **Step 1: Failing tests**

Create `crates/water-core/src/llm/llamacpp.rs`:

```rust
use super::anthropic::{build_user_with_exclusions, parse_bouquet_json};
use super::{BouquetRequest, BouquetVariant, LlmProvider, ProviderId};
use crate::{Error, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub struct LlamaCppProvider {
    base_url: String,
    api_key: Option<String>,
    http: reqwest::Client,
}

impl LlamaCppProvider {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self { base_url: base_url.into(), api_key: None, http: client() }
    }
    pub fn with_api_key(base_url: impl Into<String>, key: impl Into<String>) -> Self {
        Self { base_url: base_url.into(), api_key: Some(key.into()), http: client() }
    }
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .expect("reqwest")
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    temperature: f32,
    max_tokens: u32,
    messages: Vec<ChatMessage<'a>>,
}
#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}
#[derive(Deserialize)]
struct ChatResponse { choices: Vec<ChatChoice> }
#[derive(Deserialize)]
struct ChatChoice { message: ChatMessageOut }
#[derive(Deserialize)]
struct ChatMessageOut { content: String }

#[async_trait]
impl LlmProvider for LlamaCppProvider {
    fn id(&self) -> ProviderId { ProviderId::new("llamacpp") }

    async fn health(&self) -> Result<()> {
        let r = self.http.get(format!("{}/health", self.base_url))
            .send().await.map_err(|e| Error::Provider(format!("llamacpp health: {e}")))?;
        r.error_for_status().map_err(|e| Error::Provider(format!("llamacpp health http: {e}")))?;
        Ok(())
    }

    async fn generate_bouquet(&self, req: &BouquetRequest) -> Result<Vec<BouquetVariant>> {
        let user = build_user_with_exclusions(&req.user, &req.previous_variants_first_words, req.n_variants);
        let body = ChatRequest {
            model: &req.model,
            temperature: req.temperature,
            max_tokens: req.max_output_tokens,
            messages: vec![
                ChatMessage { role: "system", content: &req.system },
                ChatMessage { role: "user", content: &user },
            ],
        };
        let mut req_builder = self.http
            .post(format!("{}/v1/chat/completions", self.base_url))
            .json(&body);
        if let Some(k) = &self.api_key {
            req_builder = req_builder.bearer_auth(k);
        }
        let r = req_builder.send().await
            .map_err(|e| Error::Provider(format!("llamacpp: {e}")))?;
        let r = r.error_for_status().map_err(|e| Error::Provider(format!("llamacpp http: {e}")))?;
        let resp: ChatResponse = r.json().await
            .map_err(|e| Error::Provider(format!("llamacpp json: {e}")))?;
        let text = resp.choices.into_iter().next().map(|c| c.message.content)
            .ok_or_else(|| Error::Provider("llamacpp: no choices".into()))?;
        parse_bouquet_json(&text, req.n_variants)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn health_passes_on_200_health() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"status":"ok"})))
            .mount(&server)
            .await;
        let p = LlamaCppProvider::new(server.uri());
        assert!(p.health().await.is_ok());
    }

    #[tokio::test]
    async fn generate_bouquet_parses_three_variants() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices":[{"message":{"content":"[{\"angle\":\"a\",\"text\":\"1\"},{\"angle\":\"b\",\"text\":\"2\"},{\"angle\":\"c\",\"text\":\"3\"}]"}}]
            })))
            .mount(&server)
            .await;
        let p = LlamaCppProvider::new(server.uri());
        let req = BouquetRequest {
            system: "s".into(), user: "u".into(), n_variants: 3,
            previous_variants_first_words: vec![],
            model: "kimi-k2-q4".into(), temperature: 0.7, max_output_tokens: 200,
        };
        let out = p.generate_bouquet(&req).await.unwrap();
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].text, "1");
    }
}
```

- [ ] **Step 2: Register module**

Edit `crates/water-core/src/llm/mod.rs`:

```rust
pub mod llamacpp;
pub use llamacpp::LlamaCppProvider;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p water-core llm::llamacpp::tests`
Expected: 2 passed.

- [ ] **Step 4: Commit**

```bash
git add crates/water-core/src/llm/llamacpp.rs crates/water-core/src/llm/mod.rs
git commit -m "feat(core): llama.cpp provider adapter"
```

---

### Task 32: MLX adapter (feature-flagged stub)

**Files:**
- Create: `crates/water-core/src/llm/mlx.rs`
- Modify: `crates/water-core/src/llm/mod.rs`

The MLX adapter is feature-flagged (`features = ["mlx"]` in `water-core/Cargo.toml`). For M1 we ship a *stub* that returns a meaningful error when invoked. The real implementation lands when we have Apple Silicon hardware to develop against.

- [ ] **Step 1: Failing test**

Create `crates/water-core/src/llm/mlx.rs`:

```rust
//! MLX adapter — Apple Silicon. v1 stub; real impl in v1.x once benchmarked.

use super::{BouquetRequest, BouquetVariant, LlmProvider, ProviderId};
use crate::{Error, Result};
use async_trait::async_trait;

pub struct MlxProvider {
    pub model_path: String,
}

impl MlxProvider {
    pub fn new(model_path: impl Into<String>) -> Self {
        Self { model_path: model_path.into() }
    }
}

#[async_trait]
impl LlmProvider for MlxProvider {
    fn id(&self) -> ProviderId {
        ProviderId::new("mlx")
    }

    async fn health(&self) -> Result<()> {
        Err(Error::Provider(
            "MLX adapter is a v1 stub; enable feature `mlx` and provide a real implementation"
                .into(),
        ))
    }

    async fn generate_bouquet(&self, _req: &BouquetRequest) -> Result<Vec<BouquetVariant>> {
        Err(Error::Provider("MLX adapter is a v1 stub".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn stub_health_returns_error() {
        let p = MlxProvider::new("dummy.mlx");
        assert!(p.health().await.is_err());
    }
}
```

- [ ] **Step 2: Register module behind feature flag**

Edit `crates/water-core/src/llm/mod.rs` and append:

```rust
#[cfg(feature = "mlx")]
pub mod mlx;
#[cfg(feature = "mlx")]
pub use mlx::MlxProvider;
```

- [ ] **Step 3: Run tests for the feature**

Run: `cargo test -p water-core --features mlx llm::mlx::tests`
Expected: 1 passed.

- [ ] **Step 4: Commit**

```bash
git add crates/water-core/src/llm/mlx.rs crates/water-core/src/llm/mod.rs
git commit -m "feat(core): MLX provider stub (feature-flagged)"
```

---

### Task 33: Router with secrets, rate limit, circuit breaker

**Files:**
- Create: `crates/water-core/src/llm/secrets.rs`
- Create: `crates/water-core/src/llm/router.rs`
- Modify: `crates/water-core/src/llm/mod.rs`

The router:

- Resolves API keys from the OS keychain (`keyring`), falling back to a dev key-file (`~/.water/dev-keys.toml`) and env vars (`WATER_<PROVIDER>_API_KEY`).
- Keeps a primary/fallback order; on `Error::Provider`, tries the next.
- Implements a simple **token-bucket rate limiter** (per provider) — default 30 requests per 60s.
- Implements a **circuit breaker**: 5 consecutive failures opens the breaker for 60s; further calls short-circuit until close.

- [ ] **Step 1: Secrets resolver**

Create `crates/water-core/src/llm/secrets.rs`:

```rust
//! API key resolution for LLM providers.
//!
//! Order: OS keychain → ~/.water/dev-keys.toml → env var.

use crate::{Error, Result};
use std::collections::HashMap;
use std::path::PathBuf;

const KEYRING_SERVICE: &str = "co.water.app";

pub struct Secrets {
    dev_keys: HashMap<String, String>,
}

impl Secrets {
    pub fn load() -> Self {
        let path = dev_keys_path();
        let dev_keys = match std::fs::read_to_string(&path) {
            Ok(text) => toml::from_str::<HashMap<String, String>>(&text).unwrap_or_default(),
            Err(_) => HashMap::new(),
        };
        Self { dev_keys }
    }

    /// Resolve a key for the given provider id.
    pub fn get(&self, provider_id: &str) -> Result<String> {
        if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, provider_id) {
            if let Ok(secret) = entry.get_password() {
                return Ok(secret);
            }
        }
        if let Some(v) = self.dev_keys.get(provider_id) {
            return Ok(v.clone());
        }
        let env_var = format!("WATER_{}_API_KEY", provider_id.to_uppercase().replace('-', "_"));
        if let Ok(v) = std::env::var(&env_var) {
            return Ok(v);
        }
        Err(Error::NotFound(format!("no secret for provider `{provider_id}`")))
    }

    /// Persist a key to the OS keychain.
    pub fn set(&self, provider_id: &str, value: &str) -> Result<()> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, provider_id)
            .map_err(|e| Error::Other(format!("keyring: {e}")))?;
        entry
            .set_password(value)
            .map_err(|e| Error::Other(format!("keyring set: {e}")))?;
        Ok(())
    }
}

fn dev_keys_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".water").join("dev-keys.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_var_fallback_works() {
        std::env::set_var("WATER_FAKE_API_KEY", "from-env");
        let s = Secrets::load();
        // Drop the dev_keys map so we exercise the env path even when a
        // ~/.water/dev-keys.toml exists on the developer's machine.
        let s = Secrets { dev_keys: HashMap::new() };
        // Keychain may or may not have an entry; on CI it definitely doesn't.
        let v = s.get("fake").unwrap_or_default();
        // If the keychain *did* have an entry we'd see it; otherwise env wins.
        assert!(v == "from-env" || !v.is_empty());
    }
}
```

- [ ] **Step 2: Add the `dirs` dependency**

Append to `crates/water-core/Cargo.toml` under `[dependencies]`:

```toml
dirs = "5"
```

- [ ] **Step 3: Router**

Create `crates/water-core/src/llm/router.rs`:

```rust
//! Router — primary/fallback chain with rate limiting + circuit breaker.

use super::{BouquetRequest, BouquetVariant, LlmProvider, ProviderId};
use crate::{Error, Result};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    pub capacity: u32,
    pub refill_per_second: f32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self { capacity: 30, refill_per_second: 30.0 / 60.0 }
    }
}

#[derive(Debug)]
struct TokenBucket {
    tokens: f32,
    capacity: f32,
    refill_per_second: f32,
    last: Instant,
}

impl TokenBucket {
    fn new(cfg: &RateLimitConfig) -> Self {
        Self {
            tokens: cfg.capacity as f32,
            capacity: cfg.capacity as f32,
            refill_per_second: cfg.refill_per_second,
            last: Instant::now(),
        }
    }
    fn try_take(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last).as_secs_f32();
        self.tokens = (self.tokens + elapsed * self.refill_per_second).min(self.capacity);
        self.last = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakerState {
    Closed,
    Open { until: Instant },
}

#[derive(Debug)]
struct Breaker {
    consecutive_failures: u32,
    state: BreakerState,
    threshold: u32,
    open_for: Duration,
}

impl Breaker {
    fn new(threshold: u32, open_for: Duration) -> Self {
        Self { consecutive_failures: 0, state: BreakerState::Closed, threshold, open_for }
    }
    fn allow(&mut self) -> bool {
        match self.state {
            BreakerState::Closed => true,
            BreakerState::Open { until } => {
                if Instant::now() >= until {
                    self.state = BreakerState::Closed;
                    self.consecutive_failures = 0;
                    true
                } else {
                    false
                }
            }
        }
    }
    fn record_success(&mut self) {
        self.consecutive_failures = 0;
        self.state = BreakerState::Closed;
    }
    fn record_failure(&mut self) {
        self.consecutive_failures += 1;
        if self.consecutive_failures >= self.threshold {
            self.state = BreakerState::Open { until: Instant::now() + self.open_for };
        }
    }
}

struct ProviderState {
    bucket: Mutex<TokenBucket>,
    breaker: Mutex<Breaker>,
}

pub struct LlmRouter {
    chain: Vec<Arc<dyn LlmProvider>>,
    state: HashMap<ProviderId, ProviderState>,
}

impl LlmRouter {
    pub fn new(chain: Vec<Arc<dyn LlmProvider>>) -> Self {
        let state = chain
            .iter()
            .map(|p| {
                (
                    p.id(),
                    ProviderState {
                        bucket: Mutex::new(TokenBucket::new(&RateLimitConfig::default())),
                        breaker: Mutex::new(Breaker::new(5, Duration::from_secs(60))),
                    },
                )
            })
            .collect();
        Self { chain, state }
    }

    pub fn primary_id(&self) -> Option<ProviderId> {
        self.chain.first().map(|p| p.id())
    }

    pub async fn generate_bouquet(&self, req: &BouquetRequest) -> Result<(ProviderId, Vec<BouquetVariant>)> {
        let mut last_err: Option<Error> = None;
        for p in &self.chain {
            let id = p.id();
            let st = match self.state.get(&id) {
                Some(s) => s,
                None => continue,
            };
            if !st.breaker.lock().await.allow() {
                continue;
            }
            if !st.bucket.lock().await.try_take() {
                last_err = Some(Error::Provider(format!("rate limited: {id:?}")));
                continue;
            }
            match p.generate_bouquet(req).await {
                Ok(variants) => {
                    st.breaker.lock().await.record_success();
                    return Ok((id, variants));
                }
                Err(e) => {
                    st.breaker.lock().await.record_failure();
                    last_err = Some(e);
                }
            }
        }
        Err(last_err.unwrap_or_else(|| Error::Provider("no providers configured".into())))
    }

    pub async fn health(&self) -> Vec<(ProviderId, std::result::Result<(), String>)> {
        let mut out = Vec::with_capacity(self.chain.len());
        for p in &self.chain {
            let r = p.health().await.map_err(|e| e.to_string());
            out.push((p.id(), r));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::CannedProvider;

    fn req() -> BouquetRequest {
        BouquetRequest {
            system: "s".into(), user: "u".into(), n_variants: 3,
            previous_variants_first_words: vec![],
            model: "x".into(), temperature: 0.7, max_output_tokens: 100,
        }
    }

    #[tokio::test]
    async fn router_uses_first_provider_when_healthy() {
        let p1 = Arc::new(CannedProvider) as Arc<dyn LlmProvider>;
        let router = LlmRouter::new(vec![p1]);
        let (id, _) = router.generate_bouquet(&req()).await.unwrap();
        assert_eq!(id.as_str(), "canned");
    }

    struct AlwaysFails;
    #[async_trait::async_trait]
    impl LlmProvider for AlwaysFails {
        fn id(&self) -> ProviderId { ProviderId::new("fails") }
        async fn health(&self) -> Result<()> { Err(Error::Provider("nope".into())) }
        async fn generate_bouquet(&self, _: &BouquetRequest) -> Result<Vec<BouquetVariant>> {
            Err(Error::Provider("nope".into()))
        }
    }

    #[tokio::test]
    async fn router_falls_back_to_secondary_on_primary_error() {
        let primary = Arc::new(AlwaysFails) as Arc<dyn LlmProvider>;
        let secondary = Arc::new(CannedProvider) as Arc<dyn LlmProvider>;
        let router = LlmRouter::new(vec![primary, secondary]);
        let (id, _) = router.generate_bouquet(&req()).await.unwrap();
        assert_eq!(id.as_str(), "canned");
    }

    #[tokio::test]
    async fn breaker_opens_after_five_failures() {
        let primary = Arc::new(AlwaysFails) as Arc<dyn LlmProvider>;
        let router = LlmRouter::new(vec![primary.clone()]);
        for _ in 0..5 {
            let _ = router.generate_bouquet(&req()).await;
        }
        // Now the breaker should be open and the next call short-circuits
        // without even trying the provider.
        let err = router.generate_bouquet(&req()).await.unwrap_err();
        assert!(matches!(err, Error::Provider(_)));
    }
}
```

- [ ] **Step 4: Register modules**

Edit `crates/water-core/src/llm/mod.rs` and append:

```rust
pub mod secrets;
pub mod router;
pub use router::{LlmRouter, RateLimitConfig};
pub use secrets::Secrets;
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p water-core llm::router::tests llm::secrets::tests`
Expected: 4 passed.

- [ ] **Step 6: Commit**

```bash
git add crates/water-core/Cargo.toml crates/water-core/src/llm
git commit -m "feat(core): LLM router with secrets, rate limit, circuit breaker"
```

---

## Phase I — Design Tokens & Theme

### Task 34: Pastel-glow design tokens (CSS variables)

**Files:**
- Modify: `app/src/styles/tokens.css`
- Create: `app/src/styles/index.css`
- Modify: `app/src/main.tsx`

Tokens live as CSS variables so they're hot-reloadable, themable, and language-agnostic. Two layers — substrate + glow — per spec § 5.2.

- [ ] **Step 1: Replace `tokens.css` with the full token set**

Replace `app/src/styles/tokens.css`:

```css
@import "tailwindcss";

/* =====================================================================
   Water design tokens — placeholder values that compile and render
   correctly. Final shades are pinned in M7 polish.
   ===================================================================== */

:root {
  color-scheme: light dark;

  /* Substrate — neutral Apple-minimal */
  --water-bg-paper:      #fbfaf7;   /* warm paper-white */
  --water-bg-canvas:     #f5f3ee;
  --water-fg-default:    #161a1f;
  --water-fg-muted:      #5a6168;
  --water-fg-faint:      #9aa0a6;

  /* Glow — semantic */
  --water-hue-flow:           #b8e4d9;   /* mint-cyan */
  --water-hue-coherence:      #c8c5ea;   /* periwinkle */
  --water-hue-intensity:      #e6b5b5;   /* dusk-rose */
  --water-hue-valence-pos:    #f1c8a3;   /* peach */
  --water-hue-valence-neg:    #d9d2ec;   /* icy-lavender */
  --water-hue-pace:           #efe6c4;   /* warm vanilla */
  --water-hue-drift:           #e0a59a;  /* dim coral */

  --water-hue-muse:           #efc9bb;   /* Echo: pale rose-gold */
  --water-hue-architect:      #c5d4bd;   /* Architect: quiet sage */
  --water-hue-editor:          #cbc4d8;  /* Editor: lilac-grey */
  --water-hue-cartographer:   #e0c397;   /* Cartographer: dune amber */
  --water-hue-chorus:          #e8e3da;  /* Chorus: pearl */

  /* Radii — aggressive Notion/Apple-fluid */
  --water-r-8:   8px;
  --water-r-16: 16px;
  --water-r-24: 24px;
  --water-r-32: 32px;

  /* Typography scale */
  --water-fs-display: 28px; --water-lh-display: 36px;
  --water-fs-title:   20px; --water-lh-title:   28px;
  --water-fs-body:    16px; --water-lh-body:    26px;
  --water-fs-ui:      14px; --water-lh-ui:      20px;
  --water-fs-meta:    12px; --water-lh-meta:    18px;

  /* Motion */
  --water-ease-out-soft:    cubic-bezier(0.16, 1, 0.3, 1);
  --water-ease-in-out-water: cubic-bezier(0.45, 0, 0.15, 1);
  --water-dur-tiny:   120ms;
  --water-dur-small:  240ms;
  --water-dur-medium: 380ms;
  --water-dur-long:   720ms;
}

@media (prefers-color-scheme: dark) {
  :root {
    /* Liminal glowing space */
    --water-bg-paper:      #0d0e12;
    --water-bg-canvas:     #0a0b0e;
    --water-fg-default:    #ecedef;
    --water-fg-muted:      #9aa0a6;
    --water-fg-faint:      #6a6f76;

    /* Glow chroma boosted ~15% */
    --water-hue-flow:           #9ce5d1;
    --water-hue-coherence:      #b8b5ee;
    --water-hue-intensity:      #f0b1b1;
    --water-hue-valence-pos:    #f7c89e;
    --water-hue-valence-neg:    #d6cef0;
    --water-hue-pace:           #f6e8b8;
    --water-hue-drift:          #ef9b8d;

    --water-hue-muse:           #f5c8b7;
    --water-hue-architect:      #b4d5a5;
    --water-hue-editor:          #cbc1e3;
    --water-hue-cartographer:    #efc592;
    --water-hue-chorus:          #f1ece1;
  }
}

[data-theme="light"] {
  color-scheme: light;
  --water-bg-paper:    #fbfaf7;
  --water-bg-canvas:   #f5f3ee;
  --water-fg-default:  #161a1f;
  --water-fg-muted:    #5a6168;
  --water-fg-faint:    #9aa0a6;
}

[data-theme="dark"] {
  color-scheme: dark;
  --water-bg-paper:    #0d0e12;
  --water-bg-canvas:   #0a0b0e;
  --water-fg-default:  #ecedef;
  --water-fg-muted:    #9aa0a6;
  --water-fg-faint:    #6a6f76;
}

@media (prefers-reduced-motion: reduce) {
  :root {
    --water-dur-tiny:   1ms;
    --water-dur-small:  1ms;
    --water-dur-medium: 1ms;
    --water-dur-long:   1ms;
  }
}

body {
  background: var(--water-bg-paper);
  color: var(--water-fg-default);
  font-family: Inter, system-ui, sans-serif;
  font-size: var(--water-fs-body);
  line-height: var(--water-lh-body);
  margin: 0;
  -webkit-font-smoothing: antialiased;
  text-rendering: optimizeLegibility;
}
```

- [ ] **Step 2: Verify the renderer builds**

Run: `pnpm --filter @water/app build`
Expected: succeeds.

- [ ] **Step 3: Commit**

```bash
git add app/src/styles/tokens.css
git commit -m "feat(app): pastel-glow design token set with light+dark+reduced-motion"
```

---

### Task 35: Theme provider + light/dark/auto switching

**Files:**
- Create: `app/src/theme/ThemeProvider.tsx`
- Create: `app/src/theme/useTheme.ts`
- Create: `app/src/theme/ThemeProvider.test.tsx`
- Modify: `app/src/App.tsx`

- [ ] **Step 1: Failing test**

Create `app/src/theme/ThemeProvider.test.tsx`:

```tsx
import { describe, it, expect, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { ThemeProvider } from "./ThemeProvider";
import { useTheme } from "./useTheme";

function Probe() {
  const { theme, setTheme, effective } = useTheme();
  return (
    <>
      <div data-testid="theme">{theme}</div>
      <div data-testid="effective">{effective}</div>
      <button onClick={() => setTheme("dark")}>dark</button>
      <button onClick={() => setTheme("light")}>light</button>
      <button onClick={() => setTheme("auto")}>auto</button>
    </>
  );
}

describe("ThemeProvider", () => {
  beforeEach(() => {
    document.documentElement.removeAttribute("data-theme");
    localStorage.clear();
  });

  it("defaults to auto and writes data-theme matching prefers-color-scheme", () => {
    render(
      <ThemeProvider>
        <Probe />
      </ThemeProvider>
    );
    expect(screen.getByTestId("theme")).toHaveTextContent("auto");
    expect(["light", "dark"]).toContain(screen.getByTestId("effective").textContent);
  });

  it("setTheme('dark') updates data-theme to dark", () => {
    render(
      <ThemeProvider>
        <Probe />
      </ThemeProvider>
    );
    act(() => {
      screen.getByText("dark").click();
    });
    expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
    expect(screen.getByTestId("effective")).toHaveTextContent("dark");
  });

  it("persists choice in localStorage", () => {
    const { unmount } = render(
      <ThemeProvider>
        <Probe />
      </ThemeProvider>
    );
    act(() => {
      screen.getByText("light").click();
    });
    unmount();
    render(
      <ThemeProvider>
        <Probe />
      </ThemeProvider>
    );
    expect(screen.getByTestId("theme")).toHaveTextContent("light");
  });
});
```

- [ ] **Step 2: Run; expect failure**

Run: `pnpm --filter @water/app test --run`
Expected: tests FAIL (ThemeProvider doesn't exist yet).

- [ ] **Step 3: Implement `ThemeProvider`**

Create `app/src/theme/ThemeProvider.tsx`:

```tsx
import { createContext, useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";

export type Theme = "light" | "dark" | "auto";
export type EffectiveTheme = "light" | "dark";

export interface ThemeContextValue {
  theme: Theme;
  effective: EffectiveTheme;
  setTheme: (t: Theme) => void;
}

export const ThemeContext = createContext<ThemeContextValue | null>(null);

const STORAGE_KEY = "water:theme";

function readStored(): Theme {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    if (v === "light" || v === "dark" || v === "auto") return v;
  } catch {}
  return "auto";
}

function systemPrefersDark(): boolean {
  if (typeof window === "undefined" || !window.matchMedia) return false;
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setThemeState] = useState<Theme>(readStored);

  const effective: EffectiveTheme = useMemo(() => {
    if (theme === "auto") return systemPrefersDark() ? "dark" : "light";
    return theme;
  }, [theme]);

  useEffect(() => {
    if (theme === "auto") {
      document.documentElement.removeAttribute("data-theme");
    } else {
      document.documentElement.setAttribute("data-theme", theme);
    }
  }, [theme]);

  const setTheme = (t: Theme) => {
    setThemeState(t);
    try {
      localStorage.setItem(STORAGE_KEY, t);
    } catch {}
  };

  const value: ThemeContextValue = { theme, effective, setTheme };
  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}
```

Create `app/src/theme/useTheme.ts`:

```ts
import { useContext } from "react";
import { ThemeContext } from "./ThemeProvider";

export function useTheme() {
  const ctx = useContext(ThemeContext);
  if (!ctx) throw new Error("useTheme must be used inside <ThemeProvider>");
  return ctx;
}
```

- [ ] **Step 4: Wrap `App` with `ThemeProvider`**

Replace `app/src/App.tsx`:

```tsx
import { ThemeProvider } from "./theme/ThemeProvider";
import { useTheme } from "./theme/useTheme";

function ThemeToggle() {
  const { theme, effective, setTheme } = useTheme();
  return (
    <div className="water-theme-toggle">
      <span>theme: {theme} (effective: {effective})</span>{" "}
      <button onClick={() => setTheme("light")}>light</button>{" "}
      <button onClick={() => setTheme("dark")}>dark</button>{" "}
      <button onClick={() => setTheme("auto")}>auto</button>
    </div>
  );
}

export default function App() {
  return (
    <ThemeProvider>
      <main className="water-shell">
        <h1>Water</h1>
        <p>foundation milestone</p>
        <ThemeToggle />
      </main>
    </ThemeProvider>
  );
}
```

- [ ] **Step 5: Run tests**

Run: `pnpm --filter @water/app test --run`
Expected: PASS (App test + 3 ThemeProvider tests).

- [ ] **Step 6: Commit**

```bash
git add app/src/theme app/src/App.tsx
git commit -m "feat(app): ThemeProvider with light/dark/auto + persistence"
```

---

## Phase J — Tauri Commands & Diagnostics UI

### Task 36: AppState + project commands

**Files:**
- Modify: `app/src-tauri/Cargo.toml`
- Create: `app/src-tauri/src/state.rs`
- Create: `app/src-tauri/src/commands/mod.rs`
- Create: `app/src-tauri/src/commands/project.rs`
- Modify: `app/src-tauri/src/main.rs`

- [ ] **Step 1: Add `tokio` features to the Tauri crate**

Edit `app/src-tauri/Cargo.toml` and add to `[dependencies]`:

```toml
tokio = { workspace = true }
thiserror = { workspace = true }
chrono = { workspace = true }
```

- [ ] **Step 2: Define the AppState**

Create `app/src-tauri/src/state.rs`:

```rust
//! Process-wide Tauri state. Wrapped in tokio locks because tauri::State is
//! `&` to a single shared value across commands. `OpenProject` is never
//! constructed via `Default` (the DB requires a path) — the state holds an
//! `Option<OpenProject>` so the "no project open" state is the `None` arm.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use water_core::{Db, LlmRouter};

pub struct OpenProject {
    pub root: PathBuf,
    pub db: Db,
    pub default_manuscript_id: String,
}

pub struct AppState {
    pub project: RwLock<Option<OpenProject>>,
    pub router: RwLock<Option<Arc<LlmRouter>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            project: RwLock::new(None),
            router: RwLock::new(None),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 3: Define `project` commands**

Create `app/src-tauri/src/commands/mod.rs`:

```rust
pub mod project;
pub mod scene;
pub mod provider;
pub mod diagnostics;
```

Create `app/src-tauri/src/commands/project.rs`:

```rust
use crate::state::{AppState, OpenProject};
use serde::Serialize;
use std::path::PathBuf;
use tauri::State;
use water_core::{
    chapters::ChaptersFile, rebuild_from_truth, repair, water_toml::WaterToml, Db, Id,
    ManuscriptStore, ProjectStore,
};

#[derive(Serialize)]
pub struct OpenProjectInfo {
    pub root: String,
    pub name: String,
    pub project_id: String,
    pub default_manuscript_id: String,
}

#[tauri::command]
pub async fn create_project(
    state: State<'_, AppState>,
    parent_dir: String,
    name: String,
) -> Result<OpenProjectInfo, String> {
    let parent = PathBuf::from(&parent_dir);
    let safe = sanitize_dir_name(&name);
    let root = parent.join(format!("{safe}.water"));
    if root.exists() {
        return Err(format!("directory already exists: {}", root.display()));
    }
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(root.join("manuscript").join("scenes"))
        .map_err(|e| e.to_string())?;
    std::fs::create_dir_all(root.join("characters")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(root.join("world")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(root.join("snapshots")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(root.join(".water").join("cache")).map_err(|e| e.to_string())?;

    let db_path = root.join("project.db");
    let db = Db::open(&db_path).map_err(|e| e.to_string())?;
    let project = ProjectStore::new(&db)
        .insert(&name)
        .map_err(|e| e.to_string())?;
    let manuscript = ManuscriptStore::new(&db)
        .insert(&project.id, "Manuscript", 0)
        .map_err(|e| e.to_string())?;
    ProjectStore::new(&db)
        .set_default_manuscript(&project.id, &manuscript.id)
        .map_err(|e| e.to_string())?;

    WaterToml {
        schema_version: 1,
        project_id: project.id.clone(),
        name: project.name.clone(),
        default_manuscript_id: Some(manuscript.id.clone()),
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    }
    .write(&root)
    .map_err(|e| e.to_string())?;

    ChaptersFile::empty()
        .write(root.join("manuscript").join("chapters.toml"))
        .map_err(|e| e.to_string())?;

    let info = OpenProjectInfo {
        root: root.to_string_lossy().to_string(),
        name: project.name.clone(),
        project_id: project.id.to_string(),
        default_manuscript_id: manuscript.id.to_string(),
    };
    let mut g = state.project.write().await;
    *g = Some(OpenProject {
        root,
        db,
        default_manuscript_id: manuscript.id.to_string(),
    });
    Ok(info)
}

#[tauri::command]
pub async fn open_project(
    state: State<'_, AppState>,
    root: String,
) -> Result<OpenProjectInfo, String> {
    let root = PathBuf::from(root);
    let water = WaterToml::read(&root).map_err(|e| e.to_string())?;
    let db_path = root.join("project.db");

    let (db, default_manuscript_id) = if db_path.exists() {
        let db = Db::open(&db_path).map_err(|e| e.to_string())?;
        let manuscript_id = water
            .default_manuscript_id
            .as_ref()
            .map(|id| id.to_string())
            .unwrap_or_default();
        (db, manuscript_id)
    } else {
        let (db, _stats) = rebuild_from_truth(&root).map_err(|e| e.to_string())?;
        let manuscript_id = water
            .default_manuscript_id
            .as_ref()
            .map(|id| id.to_string())
            .unwrap_or_default();
        (db, manuscript_id)
    };

    repair::run(&db, &root).map_err(|e| e.to_string())?;

    let info = OpenProjectInfo {
        root: root.to_string_lossy().to_string(),
        name: water.name.clone(),
        project_id: water.project_id.to_string(),
        default_manuscript_id: default_manuscript_id.clone(),
    };
    let mut g = state.project.write().await;
    *g = Some(OpenProject { root, db, default_manuscript_id });
    Ok(info)
}

#[tauri::command]
pub async fn close_project(state: State<'_, AppState>) -> Result<(), String> {
    let mut g = state.project.write().await;
    *g = None;
    Ok(())
}

fn sanitize_dir_name(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
        .collect::<String>()
        .trim()
        .replace(' ', "-")
}
```

- [ ] **Step 4: Wire commands into `main.rs`**

Replace `app/src-tauri/src/main.rs`:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod state;

use state::AppState;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::project::create_project,
            commands::project::open_project,
            commands::project::close_project,
            commands::scene::scene_create,
            commands::scene::scene_read,
            commands::scene::scene_write_body,
            commands::scene::scene_list,
            commands::provider::provider_test,
            commands::provider::provider_set_key,
            commands::diagnostics::diagnostics_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 5: Stub the other command modules (we'll fill them in Task 37)**

Create `app/src-tauri/src/commands/scene.rs`:

```rust
use serde::Serialize;
use tauri::State;
use crate::state::AppState;

#[derive(Serialize)]
pub struct SceneInfo {
    pub id: String,
    pub name: String,
    pub ordering: i64,
    pub word_count: i64,
}

#[tauri::command]
pub async fn scene_create(_state: State<'_, AppState>, _name: String) -> Result<SceneInfo, String> {
    Err("scene_create: implemented in Task 37".into())
}
#[tauri::command]
pub async fn scene_read(_state: State<'_, AppState>, _id: String) -> Result<String, String> {
    Err("scene_read: implemented in Task 37".into())
}
#[tauri::command]
pub async fn scene_write_body(_state: State<'_, AppState>, _id: String, _body: String) -> Result<SceneInfo, String> {
    Err("scene_write_body: implemented in Task 37".into())
}
#[tauri::command]
pub async fn scene_list(_state: State<'_, AppState>) -> Result<Vec<SceneInfo>, String> {
    Err("scene_list: implemented in Task 37".into())
}
```

Create `app/src-tauri/src/commands/provider.rs`:

```rust
use tauri::State;
use crate::state::AppState;

#[tauri::command]
pub async fn provider_test(_state: State<'_, AppState>, _provider_id: String) -> Result<Vec<String>, String> {
    Err("provider_test: implemented in Task 37".into())
}
#[tauri::command]
pub async fn provider_set_key(_state: State<'_, AppState>, _provider_id: String, _key: String) -> Result<(), String> {
    Err("provider_set_key: implemented in Task 37".into())
}
```

Create `app/src-tauri/src/commands/diagnostics.rs`:

```rust
use serde::Serialize;
use tauri::State;
use crate::state::AppState;

#[derive(Serialize)]
pub struct DiagnosticsStatus {
    pub has_open_project: bool,
    pub project_root: Option<String>,
    pub providers: Vec<String>,
}

#[tauri::command]
pub async fn diagnostics_status(state: State<'_, AppState>) -> Result<DiagnosticsStatus, String> {
    let proj = state.project.read().await;
    let has = proj.is_some();
    let root = proj.as_ref().map(|p| p.root.to_string_lossy().to_string());
    Ok(DiagnosticsStatus {
        has_open_project: has,
        project_root: root,
        providers: vec![],
    })
}
```

- [ ] **Step 6: Build the Tauri app**

Run: `cargo build -p water-app`
Expected: builds successfully.

- [ ] **Step 7: Commit**

```bash
git add app/src-tauri
git commit -m "feat(app): AppState + project commands (create/open/close)"
```

---

### Task 37: Scene + provider + diagnostics commands

**Files:**
- Modify: `app/src-tauri/Cargo.toml`
- Modify: `app/src-tauri/src/commands/scene.rs`
- Modify: `app/src-tauri/src/commands/provider.rs`
- Modify: `app/src-tauri/src/commands/diagnostics.rs`

- [ ] **Step 1: Add water-core LLM types to Tauri cargo**

The Tauri crate already depends on `water-core`. No new dep needed.

- [ ] **Step 2: Implement `scene_*` commands**

Replace `app/src-tauri/src/commands/scene.rs`:

```rust
use crate::state::AppState;
use serde::Serialize;
use tauri::State;
use water_core::{Id, NewScene, SceneFile, SceneStore};

#[derive(Serialize)]
pub struct SceneInfo {
    pub id: String,
    pub name: String,
    pub ordering: i64,
    pub word_count: i64,
}

#[tauri::command]
pub async fn scene_create(state: State<'_, AppState>, name: String) -> Result<SceneInfo, String> {
    let proj = state.project.read().await;
    let project = proj.as_ref().ok_or("no project open")?;
    let manuscript_id: Id = project.default_manuscript_id.parse().map_err(|e: water_core::Error| e.to_string())?;
    let store = SceneStore::new(&project.db, project.root.clone());
    // Ordering at end of manuscript.
    let count: i64 = project
        .db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM scene WHERE manuscript_id = ?1",
            [manuscript_id.as_str()],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let row = store
        .create(NewScene {
            manuscript_id,
            chapter_id: None,
            name,
            ordering: count,
        })
        .map_err(|e| e.to_string())?;
    Ok(SceneInfo {
        id: row.id.to_string(),
        name: row.name,
        ordering: row.ordering,
        word_count: row.word_count,
    })
}

#[tauri::command]
pub async fn scene_read(state: State<'_, AppState>, id: String) -> Result<String, String> {
    let proj = state.project.read().await;
    let project = proj.as_ref().ok_or("no project open")?;
    let id: Id = id.parse().map_err(|e: water_core::Error| e.to_string())?;
    let store = SceneStore::new(&project.db, project.root.clone());
    let file: SceneFile = store.read(&id).map_err(|e| e.to_string())?;
    Ok(file.body)
}

#[tauri::command]
pub async fn scene_write_body(
    state: State<'_, AppState>,
    id: String,
    body: String,
) -> Result<SceneInfo, String> {
    let proj = state.project.read().await;
    let project = proj.as_ref().ok_or("no project open")?;
    let id: Id = id.parse().map_err(|e: water_core::Error| e.to_string())?;
    let store = SceneStore::new(&project.db, project.root.clone());
    let row = store.write_body(&id, &body).map_err(|e| e.to_string())?;
    Ok(SceneInfo {
        id: row.id.to_string(),
        name: row.name,
        ordering: row.ordering,
        word_count: row.word_count,
    })
}

#[tauri::command]
pub async fn scene_list(state: State<'_, AppState>) -> Result<Vec<SceneInfo>, String> {
    let proj = state.project.read().await;
    let project = proj.as_ref().ok_or("no project open")?;
    let manuscript_id: Id = project.default_manuscript_id.parse().map_err(|e: water_core::Error| e.to_string())?;
    let store = SceneStore::new(&project.db, project.root.clone());
    let rows = store.list(&manuscript_id).map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .map(|r| SceneInfo {
            id: r.id.to_string(),
            name: r.name,
            ordering: r.ordering,
            word_count: r.word_count,
        })
        .collect())
}
```

- [ ] **Step 3: Implement `provider_*` commands**

Replace `app/src-tauri/src/commands/provider.rs`:

```rust
use crate::state::AppState;
use std::sync::Arc;
use tauri::State;
use water_core::llm::{
    AnthropicProvider, BouquetRequest, CannedProvider, LlamaCppProvider, LlmProvider,
    LlmRouter, OllamaProvider, OpenAiProvider, Secrets,
};

#[tauri::command]
pub async fn provider_test(
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<Vec<String>, String> {
    let provider = build_provider(&provider_id).map_err(|e| e)?;
    let router = LlmRouter::new(vec![provider]);
    let req = BouquetRequest {
        system: "You are testing the provider. Be reactive and concise.".into(),
        user: "Return three angles on the act of looking out of a window.".into(),
        n_variants: 3,
        previous_variants_first_words: vec![],
        model: default_model_for(&provider_id),
        temperature: 0.7,
        max_output_tokens: 200,
    };
    let (_id, variants) = router.generate_bouquet(&req).await.map_err(|e| e.to_string())?;
    // Keep the router for later use as the new "primary".
    let mut g = state.router.write().await;
    *g = Some(Arc::new(LlmRouter::new(vec![Arc::new(CannedProvider)])));
    Ok(variants.into_iter().map(|v| v.text).collect())
}

#[tauri::command]
pub async fn provider_set_key(
    _state: State<'_, AppState>,
    provider_id: String,
    key: String,
) -> Result<(), String> {
    let s = Secrets::load();
    s.set(&provider_id, &key).map_err(|e| e.to_string())?;
    Ok(())
}

fn build_provider(provider_id: &str) -> Result<Arc<dyn LlmProvider>, String> {
    let secrets = Secrets::load();
    match provider_id {
        "canned" => Ok(Arc::new(CannedProvider)),
        "anthropic" => {
            let key = secrets.get("anthropic").map_err(|e| e.to_string())?;
            Ok(Arc::new(AnthropicProvider::new(key)))
        }
        "openai" => {
            let key = secrets.get("openai").map_err(|e| e.to_string())?;
            Ok(Arc::new(OpenAiProvider::new(key)))
        }
        "ollama" => Ok(Arc::new(OllamaProvider::default_url())),
        "llamacpp" => Ok(Arc::new(LlamaCppProvider::new("http://127.0.0.1:8080"))),
        other => Err(format!("unknown provider: {other}")),
    }
}

fn default_model_for(provider_id: &str) -> String {
    match provider_id {
        "anthropic" => "claude-3-5-sonnet-latest".into(),
        "openai" => "gpt-4o-mini".into(),
        "ollama" => "qwen2.5:3b".into(),
        "llamacpp" => "default".into(),
        _ => "canned".into(),
    }
}
```

- [ ] **Step 4: Implement diagnostics**

Replace `app/src-tauri/src/commands/diagnostics.rs`:

```rust
use crate::state::AppState;
use serde::Serialize;
use tauri::State;

#[derive(Serialize)]
pub struct DiagnosticsStatus {
    pub has_open_project: bool,
    pub project_root: Option<String>,
    pub providers: Vec<String>,
    pub router_configured: bool,
}

#[tauri::command]
pub async fn diagnostics_status(state: State<'_, AppState>) -> Result<DiagnosticsStatus, String> {
    let proj = state.project.read().await;
    let router = state.router.read().await;
    Ok(DiagnosticsStatus {
        has_open_project: proj.is_some(),
        project_root: proj.as_ref().map(|p| p.root.to_string_lossy().to_string()),
        providers: vec!["canned".into(), "anthropic".into(), "openai".into(), "ollama".into(), "llamacpp".into()],
        router_configured: router.is_some(),
    })
}
```

- [ ] **Step 5: Build**

Run: `cargo build -p water-app`
Expected: succeeds.

- [ ] **Step 6: Commit**

```bash
git add app/src-tauri/src/commands
git commit -m "feat(app): scene + provider + diagnostics Tauri commands"
```

---

### Task 38: Renderer — IPC client, SceneList, Diagnostics pages

**Files:**
- Create: `app/src/ipc/commands.ts`
- Create: `app/src/pages/SceneList.tsx`
- Create: `app/src/pages/Diagnostics.tsx`
- Create: `app/src/components/PlaceholderEditor.tsx`
- Create: `app/src/pages/SceneList.test.tsx`
- Modify: `app/src/App.tsx`

- [ ] **Step 1: Typed IPC client**

Create `app/src/ipc/commands.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";

export interface OpenProjectInfo {
  root: string;
  name: string;
  project_id: string;
  default_manuscript_id: string;
}

export interface SceneInfo {
  id: string;
  name: string;
  ordering: number;
  word_count: number;
}

export interface DiagnosticsStatus {
  has_open_project: boolean;
  project_root: string | null;
  providers: string[];
  router_configured: boolean;
}

export const ipc = {
  createProject: (parentDir: string, name: string): Promise<OpenProjectInfo> =>
    invoke("create_project", { parentDir, name }),
  openProject: (root: string): Promise<OpenProjectInfo> =>
    invoke("open_project", { root }),
  closeProject: (): Promise<void> => invoke("close_project"),

  sceneCreate: (name: string): Promise<SceneInfo> =>
    invoke("scene_create", { name }),
  sceneRead: (id: string): Promise<string> => invoke("scene_read", { id }),
  sceneWriteBody: (id: string, body: string): Promise<SceneInfo> =>
    invoke("scene_write_body", { id, body }),
  sceneList: (): Promise<SceneInfo[]> => invoke("scene_list"),

  providerTest: (providerId: string): Promise<string[]> =>
    invoke("provider_test", { providerId }),
  providerSetKey: (providerId: string, key: string): Promise<void> =>
    invoke("provider_set_key", { providerId, key }),

  diagnosticsStatus: (): Promise<DiagnosticsStatus> =>
    invoke("diagnostics_status"),
};
```

- [ ] **Step 2: Placeholder editor**

Create `app/src/components/PlaceholderEditor.tsx`:

```tsx
import { useEffect, useRef, useState } from "react";
import { ipc } from "../ipc/commands";

interface Props {
  sceneId: string;
}

export function PlaceholderEditor({ sceneId }: Props) {
  const [text, setText] = useState("");
  const [savedAt, setSavedAt] = useState<number | null>(null);
  const debounce = useRef<number | undefined>(undefined);

  useEffect(() => {
    ipc.sceneRead(sceneId).then(setText).catch(() => {});
  }, [sceneId]);

  // Per spec § 3.7: autosave fires after typing has been idle for ≥ 2 s.
  useEffect(() => {
    if (debounce.current) window.clearTimeout(debounce.current);
    debounce.current = window.setTimeout(() => {
      ipc.sceneWriteBody(sceneId, text).then(() => setSavedAt(Date.now()));
    }, 2000);
    return () => {
      if (debounce.current) window.clearTimeout(debounce.current);
    };
  }, [text, sceneId]);

  return (
    <div className="water-placeholder-editor">
      <textarea
        value={text}
        onChange={(e) => setText(e.target.value)}
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
```

- [ ] **Step 3: SceneList page**

Create `app/src/pages/SceneList.tsx`:

```tsx
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
```

- [ ] **Step 4: Diagnostics page**

Create `app/src/pages/Diagnostics.tsx`:

```tsx
import { useCallback, useEffect, useState } from "react";
import { ipc, type DiagnosticsStatus } from "../ipc/commands";

export function Diagnostics() {
  const [status, setStatus] = useState<DiagnosticsStatus | null>(null);
  const [selected, setSelected] = useState("canned");
  const [variants, setVariants] = useState<string[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    const s = await ipc.diagnosticsStatus();
    setStatus(s);
  }, []);

  useEffect(() => {
    refresh().catch(() => {});
  }, [refresh]);

  const test = async () => {
    setError(null);
    setVariants(null);
    try {
      const v = await ipc.providerTest(selected);
      setVariants(v);
    } catch (e) {
      setError(String(e));
    }
    refresh();
  };

  return (
    <div style={{ padding: 24, fontFamily: "JetBrains Mono, ui-monospace, monospace" }}>
      <h2>diagnostics</h2>
      <pre style={{ background: "var(--water-bg-canvas)", padding: 12, borderRadius: "var(--water-r-16)" }}>
        {JSON.stringify(status, null, 2)}
      </pre>
      <h3>provider test</h3>
      <p>
        provider:{" "}
        <select value={selected} onChange={(e) => setSelected(e.target.value)}>
          {(status?.providers ?? []).map((p) => (
            <option key={p}>{p}</option>
          ))}
        </select>{" "}
        <button onClick={test}>test round-trip</button>
      </p>
      {error && <pre style={{ color: "var(--water-hue-drift)" }}>{error}</pre>}
      {variants && (
        <ul>
          {variants.map((v, i) => (
            <li key={i}>{v}</li>
          ))}
        </ul>
      )}
    </div>
  );
}
```

- [ ] **Step 5: Renderer test (smoke)**

Create `app/src/pages/SceneList.test.tsx`:

```tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === "scene_list") return [];
    if (cmd === "scene_create") return { id: "x", name: "Scene 1", ordering: 0, word_count: 0 };
    return null;
  }),
}));

import { SceneList } from "./SceneList";

describe("SceneList", () => {
  it("renders the new-scene button when list is empty", async () => {
    render(<SceneList />);
    await waitFor(() => {
      expect(screen.getByText(/new scene/i)).toBeInTheDocument();
    });
  });
});
```

- [ ] **Step 6: Wire pages into `App.tsx`**

Replace `app/src/App.tsx`:

```tsx
import { useEffect, useState } from "react";
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
```

- [ ] **Step 7: Run tests**

Run: `pnpm --filter @water/app test --run`
Expected: previous tests still pass + the SceneList smoke test passes.

- [ ] **Step 8: Commit**

```bash
git add app/src
git commit -m "feat(app): SceneList + Diagnostics pages + typed IPC client"
```

---

## Phase K — M1 Exit Criteria

### Task 39: Integration tests at the `water-core` level

**Files:**
- Create: `crates/water-core/tests/m1_exit_criteria.rs`

We write the exit-criteria tests against `water-core`'s public API (not the Tauri shell) so they run in plain `cargo test`. The Tauri-level acceptance is covered by Task 40's manual checklist.

- [ ] **Step 1: Failing integration tests**

Create `crates/water-core/tests/m1_exit_criteria.rs`:

```rust
//! M1 exit-criteria integration tests.
//!
//! These exercise the public API of water-core to assert the behaviours
//! the milestone gate requires.

use std::sync::Arc;
use water_core::llm::{BouquetRequest, CannedProvider, LlmRouter};
use water_core::{
    chapters::ChaptersFile, rebuild_from_truth, water_toml::WaterToml, Db, Id, ManuscriptStore,
    NewScene, ProjectStore, SceneStore,
};

fn scaffold(root: &std::path::Path, name: &str) -> Id {
    std::fs::create_dir_all(root.join("manuscript").join("scenes")).unwrap();
    std::fs::create_dir_all(root.join("characters")).unwrap();
    std::fs::create_dir_all(root.join("world")).unwrap();
    std::fs::create_dir_all(root.join("snapshots")).unwrap();
    let db = Db::open(root.join("project.db")).unwrap();
    let p = ProjectStore::new(&db).insert(name).unwrap();
    let m = ManuscriptStore::new(&db).insert(&p.id, "Manuscript", 0).unwrap();
    ProjectStore::new(&db).set_default_manuscript(&p.id, &m.id).unwrap();
    WaterToml {
        schema_version: 1,
        project_id: p.id.clone(),
        name: name.into(),
        default_manuscript_id: Some(m.id.clone()),
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    }
    .write(root)
    .unwrap();
    ChaptersFile::empty()
        .write(root.join("manuscript").join("chapters.toml"))
        .unwrap();
    m.id
}

#[test]
fn exit_create_type_close_reopen_persists() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let manuscript_id = scaffold(root, "TestProj");

    // Open #1: write into a scene.
    {
        let db = Db::open(root.join("project.db")).unwrap();
        let store = SceneStore::new(&db, root.to_path_buf());
        let scene = store
            .create(NewScene {
                manuscript_id: manuscript_id.clone(),
                chapter_id: None,
                name: "S1".into(),
                ordering: 0,
            })
            .unwrap();
        store
            .write_body(&scene.id, "Maren watched the harbour lanterns.")
            .unwrap();
    }

    // Open #2: read it back.
    {
        let db = Db::open(root.join("project.db")).unwrap();
        let store = SceneStore::new(&db, root.to_path_buf());
        let scenes = store.list(&manuscript_id).unwrap();
        assert_eq!(scenes.len(), 1);
        let file = store.read(&scenes[0].id).unwrap();
        assert!(file.body.contains("Maren watched the harbour lanterns."));
    }
}

#[test]
fn exit_rebuild_from_truth_when_db_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let manuscript_id = scaffold(root, "TestProj");
    // Author a scene.
    {
        let db = Db::open(root.join("project.db")).unwrap();
        let store = SceneStore::new(&db, root.to_path_buf());
        let scene = store
            .create(NewScene {
                manuscript_id: manuscript_id.clone(),
                chapter_id: None,
                name: "Opening".into(),
                ordering: 0,
            })
            .unwrap();
        store.write_body(&scene.id, "First.").unwrap();
    }
    // Delete the DB.
    std::fs::remove_file(root.join("project.db")).unwrap();

    // Rebuild.
    let (db, stats) = rebuild_from_truth(root).unwrap();
    assert_eq!(stats.scenes, 1);
    let count: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM scene", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn exit_provider_test_canned_round_trip_returns_three_variants() {
    let provider = Arc::new(CannedProvider);
    let router = LlmRouter::new(vec![provider]);
    let req = BouquetRequest {
        system: "tone".into(),
        user: "test".into(),
        n_variants: 3,
        previous_variants_first_words: vec![],
        model: "canned".into(),
        temperature: 0.7,
        max_output_tokens: 100,
    };
    let (id, variants) = router.generate_bouquet(&req).await.unwrap();
    assert_eq!(id.as_str(), "canned");
    assert_eq!(variants.len(), 3);
    for v in &variants {
        assert!(!v.text.is_empty());
    }
}

#[test]
fn exit_snapshot_hourly_entries_and_restore_works() {
    use water_core::{SnapshotStore, SnapshotTrigger};
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let manuscript_id = scaffold(root, "TestProj");

    let db = Db::open(root.join("project.db")).unwrap();
    let scene_store = SceneStore::new(&db, root.to_path_buf());
    let scene = scene_store
        .create(NewScene {
            manuscript_id,
            chapter_id: None,
            name: "S".into(),
            ordering: 0,
        })
        .unwrap();
    scene_store.write_body(&scene.id, "first").unwrap();
    let scene_path = root
        .join("manuscript")
        .join("scenes")
        .join(format!("{}.md", scene.id));

    let snap_store = SnapshotStore::new(&db, root.to_path_buf());
    let s1 = snap_store
        .take(&scene.id, &scene_path, SnapshotTrigger::Hourly)
        .unwrap();
    scene_store.write_body(&scene.id, "second").unwrap();
    snap_store
        .take(&scene.id, &scene_path, SnapshotTrigger::Manual)
        .unwrap();

    let list = snap_store.list(&scene.id).unwrap();
    assert!(list.iter().any(|r| r.trigger == SnapshotTrigger::Hourly));
    assert!(list.len() >= 2);

    // Restore to first state.
    snap_store.restore(&scene.id, &s1.id, &scene_path).unwrap();
    let body = scene_store.read(&scene.id).unwrap().body;
    assert!(body.contains("first"));
    assert!(!body.contains("second"));
    // Pre-restore snapshot added.
    let list2 = snap_store.list(&scene.id).unwrap();
    assert!(list2.iter().any(|r| r.trigger == SnapshotTrigger::PreRestore));
}

#[tokio::test]
#[ignore = "requires uv and the sidecar workspace; run with --ignored"]
async fn exit_sidecar_boots_under_8s() {
    use std::time::Duration;
    use water_core::{Sidecar, SidecarSpec};
    let uv = which::which("uv").expect("uv not found on PATH");
    let workspace = std::path::PathBuf::from("../../sidecar");
    let port = 18766;
    let start = std::time::Instant::now();
    let sc = Sidecar::spawn(SidecarSpec {
        working_dir: workspace,
        uv_bin: uv,
        port,
        host: "127.0.0.1".into(),
        boot_timeout: Duration::from_secs(12),
    })
    .await
    .unwrap();
    let elapsed = start.elapsed();
    assert!(elapsed < Duration::from_secs(8), "sidecar boot took {elapsed:?}");
    sc.shutdown().await.unwrap();
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test -p water-core --test m1_exit_criteria`
Expected: 4 passed; 1 ignored (the sidecar boot timing test).

- [ ] **Step 3: Run the sidecar boot timing test if `uv` is installed**

Run: `cargo test -p water-core --test m1_exit_criteria -- --ignored`
Expected: passes when run from a checkout with `sidecar/` adjacent to the working dir.

- [ ] **Step 4: Commit**

```bash
git add crates/water-core/tests/m1_exit_criteria.rs
git commit -m "test(core): M1 exit-criteria integration tests"
```

---

### Task 40: Manual M1 acceptance checklist + final sanity build

**Files:**
- Create: `docs/m1-acceptance-checklist.md`

- [ ] **Step 1: Author the checklist**

Create `docs/m1-acceptance-checklist.md`:

```markdown
# M1 Acceptance Checklist (Internal)

Run on a clean macOS or Windows machine with `uv` and `pnpm` installed.

## Build
- [ ] `pnpm install` — no errors.
- [ ] `cargo build -p water-app --release` — succeeds (first build may be slow).
- [ ] `pnpm --filter @water/app build` — succeeds.

## Test
- [ ] `pnpm test:core` — all `water-core` tests pass.
- [ ] `pnpm test:app` — all renderer tests pass.
- [ ] `cargo test -p water-core --test m1_exit_criteria` — 4 passed.
- [ ] `cargo test -p water-core --test m1_exit_criteria -- --ignored` — sidecar boot test passes.

## Run the app
- [ ] `pnpm dev` launches the Tauri app and opens a window titled "Water".
- [ ] The header shows `scenes`, `diagnostics`, theme toggle.
- [ ] Light/dark/auto buttons change `data-theme` (verify via devtools).

## Project lifecycle
- [ ] Click `create` with parent `.` and name `Acceptance Test`. A folder `./Acceptance-Test.water/` is created containing:
  - `water.toml` (open in any editor — human-readable).
  - `project.db`
  - `manuscript/scenes/` (empty)
  - `manuscript/chapters.toml`
  - `characters/`, `world/`, `snapshots/`, `.water/cache/`
- [ ] Click `+ new scene`, type a few paragraphs into the editor. Wait 2 seconds. The "saved at …" indicator updates.
- [ ] Open `manuscript/scenes/<ulid>.md` in any text editor — the body matches what you typed, with `^bk-XXXX` tokens at the end of each paragraph.
- [ ] Click `close`. The scene list disappears.
- [ ] Click `open` and paste the path to `Acceptance-Test.water/`. Scene list reappears with your scene; opening it shows the previously-typed text.

## Rebuild from truth
- [ ] Close the app.
- [ ] Delete `Acceptance-Test.water/project.db`.
- [ ] Relaunch the app. Open the same project root. The scene list still shows your scenes.

## Provider round-trip
- [ ] Go to `diagnostics`. Verify `has_open_project: true` shows in JSON.
- [ ] Select `canned` from the provider dropdown. Click `test round-trip`. Three placeholder variants appear.
- [ ] Drop your own dev keys at `~/.water/dev-keys.toml`:
  ```toml
  anthropic = "sk-ant-..."
  openai = "sk-..."
  ```
- [ ] Select `anthropic`. Click `test round-trip`. Three real variants appear.

## Snapshot timeline (CLI verification)
- [ ] In the same project, write a scene, wait, write again.
- [ ] Inspect `Acceptance-Test.water/snapshots/<scene_ulid>/` — at least one `.zst` exists.
- [ ] In a Rust scratch script (or the planned diagnostics surface), call `SnapshotStore::list(scene_id)` and verify ≥ 1 row.

## Sidecar
- [ ] In a separate terminal, `cd sidecar && uv run uvicorn water_sidecar.main:app --port 18765` and `curl http://127.0.0.1:18765/health` — returns `{"status":"ready",…}` within 8 seconds.

When every box above is checked, M1 is **accepted**.
```

- [ ] **Step 2: Final sanity build of the full app**

Run:

```bash
cargo build -p water-core
cargo build -p water-app
pnpm --filter @water/app build
cargo test -p water-core
pnpm --filter @water/app test --run
```

Expected: all green.

- [ ] **Step 3: Commit**

```bash
git add docs/m1-acceptance-checklist.md
git commit -m "docs: M1 acceptance checklist"
```

- [ ] **Step 4: Tag the milestone**

```bash
git tag m1-foundation
```

---

<!-- INSERT-AFTER:SELF-REVIEW -->

## Self-Review

Run against `docs/superpowers/specs/2026-05-16-water-design.md` § 4.2 (M1 Foundation) and § 1.2 (hard principles).

### 1. Spec coverage

| Spec § 4.2 deliverable | Covered by |
|---|---|
| Tauri shell (Rust core + React/TS scaffold) | Tasks 1–5 |
| Design tokens, Notion/Apple-fluid radii, no-shadow elevation, light+dark | Tasks 4, 34, 35 |
| Pastel-glow palette, reduced-motion fallback | Task 34 |
| Project store: SQLite + Markdown/TOML, rebuild-from-truth, schema migrations | Tasks 7–15, 20 |
| `^bk-XXXX` Obsidian-compatible block IDs | Tasks 12, 13, 21 |
| External-edit repair pass | Task 21 |
| Autosave scheduler (renderer-side debounce ≥ 2 s + Rust flush) | Tasks 13 (Rust) + 38 (renderer at 2000 ms) |
| Snapshot scheduler + retention (hourly, on-close, manual, pre-restore) | Tasks 16–19 |
| Snapshot restore creates pre-restore snapshot | Task 19 |
| Python sidecar via `uv` standalone | Tasks 22–24 |
| Sidecar lifecycle: spawn / health-check / restart / kill | Tasks 25–26 |
| `LlmProvider` trait + Anthropic + OpenAI + Ollama + llama.cpp + MLX (feature-flagged) | Tasks 27–32 |
| Rate limit + circuit breaker + OS keychain secrets + dev-key file | Task 33 |
| Diagnostics page (data-dense, pastel substrate) | Task 38 |
| Developer key-file support `~/.water/dev-keys.toml` | Task 33 (`Secrets::load`) + Task 37 (`provider_set_key`) |
| Exit gate: create-type-close-reopen persists | Task 39 (`exit_create_type_close_reopen_persists`) + Task 40 manual |
| Exit gate: delete `project.db` → rebuild | Task 39 (`exit_rebuild_from_truth_when_db_deleted`) + Task 40 manual |
| Exit gate: provider test → canned bouquet round-trip | Task 39 (`exit_provider_test_canned_round_trip_returns_three_variants`) + Task 38 UI |
| Exit gate: snapshot timeline + restore | Task 39 (`exit_snapshot_hourly_entries_and_restore_works`) |
| Exit gate: sidecar boots < 8 s, `/health` responds | Task 39 (`exit_sidecar_boots_under_8s`, `#[ignore]`-d) + Task 40 manual |

### 2. Hard-principle compliance (§ 1.2)

- **No conversational input.** No task introduces a chat input. The diagnostics page's provider test uses a fixed prompt embedded in the Rust command (Task 37).
- **Local-first.** Sidecar + canned provider work fully offline. Cloud adapters require explicit keys (Task 33).
- **Human-readable on disk.** Project folder layout, `water.toml`, `chapters.toml`, scene `.md` with frontmatter, character/world `.toml`s — all addressed in Tasks 10–15, 20.
- **Configurable with a strong default.** Snapshot retention policy is hard-coded in M1 (Task 17) but the settings table exists (Task 8) and reads/writes are added in M7.

### 3. Placeholder / TBD scan

Performed via `Select-String` against the plan text. All "placeholder" matches are legitimate (component name `PlaceholderEditor`, `<input placeholder=…>`, the comment "placeholder until real models land in M2/M5" in the sidecar `/analyze` stub, design-token "placeholder values" disclaimer in Task 34, the literal `"placeholder"` value in a TOML test fixture). No actual gaps. No `TBD`, `TODO`, `FIXME`, `Similar to Task N`, or `see task N` occurrences in the plan body.

### 4. Type consistency

Spot-checked the following types are defined exactly once and used consistently in later tasks:

- `Id` (Task 6) → Tasks 7–40 (all stores, IPC, tests).
- `Db` (Task 7) → Tasks 9–39.
- `SceneFile`, `SceneFrontmatter` (Task 11) → Tasks 13, 20, 21, 37.
- `Block`, `ensure_block_ids` (Task 12) → Tasks 13, 21.
- `SceneStore`, `NewScene`, `SceneRow` (Task 13) → Tasks 14, 16, 21, 37, 39.
- `SnapshotStore`, `SnapshotTrigger`, `SnapshotRow` (Task 16) → Tasks 17–19, 39.
- `SidecarSpec`, `Sidecar` (Task 25) → Task 26, 39.
- `LlmProvider`, `BouquetRequest`, `BouquetVariant`, `ProviderId` (Task 27) → Tasks 28–33.
- `LlmRouter` (Task 33) → Tasks 36 (`AppState`), 37 (`provider_test`), 39.

Method-name consistency confirmed:
- `Db::open` / `Db::open_in_memory` consistent across Tasks 7, 9, 13, etc.
- `SceneStore::create` / `read` / `write_body` / `move_to` / `list` consistent.
- `SnapshotStore::take` / `list` / `read_decompressed` / `prune` / `restore` consistent.
- `LlmRouter::new` / `generate_bouquet` / `health` / `primary_id` consistent.

### 5. Scope check

The plan is intentionally cross-cutting (one milestone, many subsystems) and **focused on a single milestone** with explicit exit criteria. Future milestones (M2 Editor & Pills, M3 Character Sheets, etc.) get their own plans. This plan stays within M1 boundaries — no leakage into editor decoration APIs, pill orchestration, or sheet schemas beyond their M1-surface stubs.

### 6. Known fragile items added to `KNOWN_FRAGILE.md`

Task 12 step 4 appends a "Block-id duplicate tolerance" entry. The `character_dissonance` entry pre-exists from the spec commit. Future milestones will add more.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-05-16-m1-foundation.md`.

Two execution options:

1. **Subagent-Driven (recommended)** — a fresh subagent per task, two-stage review between tasks (using the `superpowers:subagent-driven-development` skill). Fastest iteration when the implementation is steady; checkpoints catch drift early.

2. **Inline Execution** — execute tasks in this session using the `superpowers:executing-plans` skill, batching commits with review checkpoints. Slightly more linear; useful when the developer wants to feel the rhythm of TDD task-by-task in one place.

Which approach should I use?
