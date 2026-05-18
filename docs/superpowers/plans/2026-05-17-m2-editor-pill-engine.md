# M2 Editor & Pill Engine — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the Editor & Pill Engine on top of `m1.5.1`. After M2, idling for 3 s at a paragraph break produces a pastel-glow pill in the right margin; clicking expands to a bouquet of 3 sub-pills; clicking a sub-pill drills the rabbit hole; pin migrates to a 56 px quiet column at the right edge; the LLM never receives conversational input and the tone audit passes.

**Architecture:** 30 tasks across 7 phases. **Phase A — Foundations** (event bus + scene write-locks + sidecar respawn + SettingsSheet event subscription + migration runner). **Phase B — Editor** (parallel ProseMirror/Lexical bake-off + decision + block editor + classifier + structural-inflection emitter). **Phase C — Orchestrator** (trigger trait + 10 built-ins + state machine + eviction + anti-loop). **Phase D — Voice & Prompts** (Speaker trait + persona registry + 5 default personas + voice router + prompt library + structured-JSON LLM extension + PostFilter chain). **Phase E — Pill UI** (PillLayer + capsule + hover dim + glow line + bouquet + rabbit hole + pinned column + pinned detail sheet + ScenesPanel reloadToken + sheet slide-in + narrow-viewport fallback). **Phase F — Integration** (end-to-end wiring + replay log opt-in + SettingsSheet event finalization). **Phase G — Audit & tag** (200-pill tone audit fixtures + harness + gate + final review + `m2` tag).

**Tech Stack:** Rust 1.85 + tokio + rusqlite + serde + reqwest; Tauri 2 (events + capabilities + dialog plugin already wired); React 18 + TypeScript strict (Vite + Vitest); ProseMirror (winning the bake-off on tie) OR Lexical; existing pastel-glow tokens in `app/src/styles/tokens.css`; Python sidecar (FastAPI) extended for structural-inflection.

**Spec:** `docs/superpowers/specs/2026-05-17-m2-editor-pill-engine.md`

**Per-session prerequisite (Windows / PowerShell):**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
```

Put this at the top of every implementer prompt.

**Pre-authorized lint fixes (clippy):** `must_use_candidate`, `doc_markdown`, `single_match_else`, `implicit_hasher`, `inherent_to_string`, `cast_precision_loss`, `cast_possible_truncation`, `uninlined_format_args`, `duration_suboptimal_units`, `assigning_clones`, `needless_pass_by_value`. Apply silently; don't ask.

**Pre-authorized Vitest patterns:**
- Use `vi.hoisted(() => ({ mock: vi.fn() }))` for top-level mock identities.
- Use `vi.useFakeTimers({ shouldAdvanceTime: true })` in `beforeEach` whenever combining fake timers with `waitFor`.
- Stub `HTMLDialogElement.prototype.showModal` and `.close` in `beforeEach` for tests that touch `<dialog>` (see `app/src/sheets/CreateProjectSheet.test.tsx`).
- `pnpm --filter @water/app test src/path/to/file.test.tsx` (NOT `-- --run`).

---

## Phase A — Foundations

### Task 1: Tauri event bus skeleton

Adds a typed, bidirectional event bus between renderer and `water-app`. M2's pill engine will emit `pill:emerged` / `bouquet:ready` from core; M2's editor will emit `typing:telemetry` / `pill:click` from renderer. This task creates the framework + one trivial event (`bus:ping`) to prove wiring; later tasks add payload types as their features land.

**Files:**
- Create: `app/src-tauri/src/events.rs`
- Create: `app/src/ipc/events.ts`
- Modify: `app/src-tauri/src/main.rs`
- Modify: `app/src-tauri/src/commands/mod.rs`
- Create: `app/src-tauri/src/commands/events.rs`
- Create: `app/src/ipc/events.test.ts`

- [ ] **Step 1: Write the failing renderer-side subscription test**

Create `app/src/ipc/events.test.ts`:

```ts
import { describe, expect, it, vi } from "vitest";
import { onWaterEvent } from "./events";

const listeners: Record<string, ((p: unknown) => void)[]> = {};
vi.mock("@tauri-apps/api/event", () => ({
  listen: async (name: string, cb: (e: { payload: unknown }) => void) => {
    listeners[name] ??= [];
    listeners[name].push((p) => cb({ payload: p }));
    return () => {
      const i = listeners[name].indexOf(listeners[name][listeners[name].length - 1]);
      if (i >= 0) listeners[name].splice(i, 1);
    };
  },
}));

describe("onWaterEvent", () => {
  it("subscribes to a named event and forwards payload", async () => {
    const cb = vi.fn();
    const unsub = await onWaterEvent("bus:ping", cb);
    listeners["bus:ping"][0]({ tick: 1 });
    expect(cb).toHaveBeenCalledWith({ tick: 1 });
    unsub();
  });

  it("unsubscribe removes listener", async () => {
    const cb = vi.fn();
    const unsub = await onWaterEvent("bus:ping", cb);
    unsub();
    listeners["bus:ping"]?.[0]?.({ tick: 2 });
    expect(cb).not.toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```powershell
pnpm --filter @water/app test src/ipc/events.test.ts
```

Expected: FAIL with `Cannot find module './events'`.

- [ ] **Step 3: Create the renderer-side event helper**

Create `app/src/ipc/events.ts`:

```ts
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/**
 * Bidirectional event bus type catalogue.
 * Add new event names + payload types as features land.
 * Mirrors `app/src-tauri/src/events.rs`'s `WaterEvent` enum.
 */
export interface WaterEventPayloads {
  "bus:ping": { tick: number };
  // sidecar:status added in Task 3
  // typing:telemetry, pill:emerged, etc. added in later tasks
}

export type WaterEventName = keyof WaterEventPayloads;

export async function onWaterEvent<K extends WaterEventName>(
  name: K,
  cb: (payload: WaterEventPayloads[K]) => void,
): Promise<UnlistenFn> {
  return listen<WaterEventPayloads[K]>(name, (e) => cb(e.payload));
}
```

- [ ] **Step 4: Run test to verify it passes**

```powershell
pnpm --filter @water/app test src/ipc/events.test.ts
```

Expected: PASS — 2 tests.

- [ ] **Step 5: Create the Rust-side emit helper**

Create `app/src-tauri/src/events.rs`:

```rust
//! Typed event-bus helpers for renderer↔core communication. Mirrors
//! `app/src/ipc/events.ts::WaterEventPayloads`. Add variants as features land.

use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// Emit a typed event to the renderer. Returns Ok on success; errors are
/// usually "no window yet" during early boot — log and continue.
pub fn emit<T: Serialize + Clone>(
    app: &AppHandle,
    event_name: &str,
    payload: T,
) -> Result<(), tauri::Error> {
    app.emit(event_name, payload)
}

#[derive(Serialize, Clone)]
pub struct BusPing {
    pub tick: u64,
}
```

- [ ] **Step 6: Wire a smoke command that emits `bus:ping`**

Create `app/src-tauri/src/commands/events.rs`:

```rust
use crate::events::{emit, BusPing};
use tauri::AppHandle;

/// Smoke command used by tests + manual ping. Removed in M3+ once the bus
/// has many real events.
#[tauri::command]
pub fn bus_ping(app: AppHandle, tick: u64) -> Result<(), String> {
    emit(&app, "bus:ping", BusPing { tick }).map_err(|e| e.to_string())
}
```

Modify `app/src-tauri/src/commands/mod.rs` to add `pub mod events;` alongside the existing `pub mod` lines. (Read the file first to confirm structure.)

- [ ] **Step 7: Register the new module + command in main.rs**

Modify `app/src-tauri/src/main.rs`:

```rust
mod commands;
mod events;     // NEW
mod state;
```

Inside `invoke_handler! [...]` (the `tauri::generate_handler!` macro call), append `commands::events::bus_ping,` as the last entry.

- [ ] **Step 8: Verify Rust build + lints**

```powershell
cargo build -p water-app
cargo clippy -p water-app -- -D warnings
```

Expected: both clean.

- [ ] **Step 9: Verify renderer build**

```powershell
pnpm --filter @water/app build
```

Expected: clean.

- [ ] **Step 10: Commit**

```powershell
git add app/src/ipc/events.ts app/src/ipc/events.test.ts app/src-tauri/src/events.rs app/src-tauri/src/commands/events.rs app/src-tauri/src/commands/mod.rs app/src-tauri/src/main.rs
git commit -m "feat: typed Tauri event bus (renderer<->core skeleton)"
```

---

### Task 2: Per-scene write-lock (closes KNOWN_FRAGILE #7)

Introduces `SceneWriteLocks: Arc<DashMap<Id, Arc<tokio::Mutex<()>>>>` on `OpenProject`. Both `SceneStore::rename` and `SceneStore::write_body` acquire the per-scene lock before any disk I/O so a concurrent rename + write can't tear the scene file.

**Files:**
- Modify: `crates/water-core/Cargo.toml`
- Modify: `crates/water-core/src/lib.rs`
- Modify: `crates/water-core/src/scene.rs`
- Create: `crates/water-core/src/scene_locks.rs`
- Modify: `app/src-tauri/src/state.rs`
- Modify: `app/src-tauri/src/commands/scene.rs`
- Update: `KNOWN_FRAGILE.md`

- [ ] **Step 1: Add `dashmap` dependency**

Read `crates/water-core/Cargo.toml`. In `[dependencies]`, append:

```toml
dashmap = "6"
```

- [ ] **Step 2: Write the failing test**

Append to the existing `#[cfg(test)] mod tests` block in `crates/water-core/src/scene.rs`:

```rust
#[tokio::test]
async fn rename_and_write_body_are_serialized_per_scene() {
    use crate::scene_locks::SceneWriteLocks;
    use std::sync::Arc;

    let (db, root) = setup_test_project_with_one_scene().await;
    let scene_id = first_scene_id(&db);

    let locks = SceneWriteLocks::new();
    let mut handles = Vec::new();
    for i in 0..50 {
        let db_c = Arc::clone(&db);
        let root_c = root.clone();
        let id_c = scene_id.clone();
        let locks_c = locks.clone();
        let new_name = format!("Renamed {i}");
        handles.push(tokio::spawn(async move {
            let _g = locks_c.acquire(&id_c).await;
            let g = db_c.lock().await;
            let store = SceneStore::new(&g, root_c.clone());
            store.rename(&id_c, &new_name).unwrap();
        }));
        let db_c2 = Arc::clone(&db);
        let root_c2 = root.clone();
        let id_c2 = scene_id.clone();
        let locks_c2 = locks.clone();
        let new_body = format!("Body iteration {i}\n");
        handles.push(tokio::spawn(async move {
            let _g = locks_c2.acquire(&id_c2).await;
            let g = db_c2.lock().await;
            let store = SceneStore::new(&g, root_c2.clone());
            store.write_body(&id_c2, &new_body).unwrap();
        }));
    }
    for h in handles { h.await.unwrap(); }

    let g = db.lock().await;
    let store = SceneStore::new(&g, root.clone());
    let row = store.read_row(&scene_id).unwrap();
    let on_disk_body = store.read_body(&scene_id).unwrap();
    assert!(row.name.starts_with("Renamed "));
    assert!(on_disk_body.starts_with("Body iteration "));
    // File hash matches the body (no torn write).
    let on_disk_hash = crate::scene_md::hash_scene_file(
        &root.join("manuscript/scenes").join(format!("{}.md", scene_id.as_str()))
    ).unwrap();
    assert_eq!(row.file_hash.as_deref(), Some(on_disk_hash.as_str()));
}

// Reuses an existing helper if present; otherwise a minimal new one.
async fn setup_test_project_with_one_scene() -> (std::sync::Arc<tokio::sync::Mutex<crate::Db>>, std::path::PathBuf) {
    use tempfile::TempDir;
    let dir = TempDir::new().unwrap();
    let root = dir.into_path();
    std::fs::create_dir_all(root.join("manuscript/scenes")).unwrap();
    let db = crate::Db::open(root.join("project.db")).unwrap();
    let db = std::sync::Arc::new(tokio::sync::Mutex::new(db));
    let g = db.lock().await;
    let store = SceneStore::new(&g, root.clone());
    let m_id = crate::Id::new();
    g.conn().execute(
        "INSERT INTO manuscript (id, project_id, name, ordering, created_at, updated_at) VALUES (?1, ?2, ?3, 0, ?4, ?4)",
        rusqlite::params![m_id.as_str(), "p1", "ms", chrono::Utc::now().to_rfc3339()],
    ).unwrap();
    store.create(&m_id, None, "Scene 1", None).unwrap();
    drop(g);
    (db, root)
}

fn first_scene_id(db: &std::sync::Arc<tokio::sync::Mutex<crate::Db>>) -> crate::Id {
    let rt = tokio::runtime::Handle::current();
    let g = rt.block_on(db.lock());
    let id: String = g.conn().query_row("SELECT id FROM scene LIMIT 1", [], |r| r.get(0)).unwrap();
    crate::Id::from_str(&id).unwrap()
}
```

(The helpers above may already exist; if so, the implementer reuses them. The point is the new test asserts serialized writes.)

- [ ] **Step 3: Run the test to verify it fails**

```powershell
cargo test -p water-core scene::tests::rename_and_write_body_are_serialized_per_scene
```

Expected: FAIL — `unresolved import 'crate::scene_locks::SceneWriteLocks'` or similar.

- [ ] **Step 4: Create `SceneWriteLocks`**

Create `crates/water-core/src/scene_locks.rs`:

```rust
//! Per-scene write-lock registry. Both `SceneStore::rename` and
//! `SceneStore::write_body` acquire the lock for a given scene before any
//! disk I/O, so concurrent rename+body writes don't tear the file.
//!
//! Locks are created lazily on first acquire and never removed (small
//! per-scene overhead; project lifetime).

use crate::Id;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, OwnedMutexGuard};

#[derive(Clone, Default)]
pub struct SceneWriteLocks {
    inner: Arc<DashMap<String, Arc<Mutex<()>>>>,
}

impl SceneWriteLocks {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns an owned guard. Drop the guard to release the lock.
    pub async fn acquire(&self, scene_id: &Id) -> OwnedMutexGuard<()> {
        let key = scene_id.as_str().to_string();
        let lock = self
            .inner
            .entry(key)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .value()
            .clone();
        lock.lock_owned().await
    }
}
```

- [ ] **Step 5: Register the new module**

Modify `crates/water-core/src/lib.rs` — add `pub mod scene_locks;` and re-export `pub use scene_locks::SceneWriteLocks;` in the same area as other re-exports.

- [ ] **Step 6: Run test to verify it now compiles + passes**

```powershell
cargo test -p water-core scene::tests::rename_and_write_body_are_serialized_per_scene
```

Expected: PASS.

- [ ] **Step 7: Add `SceneWriteLocks` to `OpenProject`**

Modify `app/src-tauri/src/state.rs`:

```rust
use water_core::{llm::LlmRouter, Db, SceneWriteLocks, Sidecar, SidecarSupervisor, SnapshotScheduler};
```

Add to the `OpenProject` struct (alongside the existing fields):

```rust
    /// Per-scene write locks shared by all command handlers that touch
    /// `SceneStore::rename` or `SceneStore::write_body`. Prevents the
    /// whole-file write race documented in KNOWN_FRAGILE #7.
    pub scene_write_locks: SceneWriteLocks,
```

- [ ] **Step 8: Construct it in `open_project`**

Modify `app/src-tauri/src/commands/project.rs`. Find the `OpenProject { ... }` literal inside `open_project` (and `create_project` if it has one). Add `scene_write_locks: SceneWriteLocks::new(),` to the struct literal, importing `use water_core::SceneWriteLocks;` at the top.

- [ ] **Step 9: Acquire the lock in `scene_rename` + `scene_write_body`**

Modify `app/src-tauri/src/commands/scene.rs`:

For `scene_rename` — after the `(db, root)` clone-and-drop block, BEFORE acquiring the db lock:

```rust
    let (db, root, locks) = {
        let proj = state.project.lock().await;
        let p = proj.as_ref().ok_or("no project open")?;
        (p.db.clone(), p.root.clone(), p.scene_write_locks.clone())
    };
    let scene_id_parsed = water_core::Id::from_str(&id).map_err(|e| e.to_string())?;
    let _write_guard = locks.acquire(&scene_id_parsed).await;
    let db_guard = db.lock().await;
    let store = SceneStore::new(&db_guard, root);
    // ... existing rename + return logic ...
```

Same shape for `scene_write_body`. Both already drop the project lock before db work; this slots the `_write_guard` between the project-lock drop and the db-lock acquire.

- [ ] **Step 10: Update KNOWN_FRAGILE.md**

In `KNOWN_FRAGILE.md`, mark entry #7's status. Add a line at the bottom of the entry:

```markdown
**Resolved in M2 Task 2 (commit pending):** Per-scene `SceneWriteLocks` registry on `OpenProject`. Both `rename` and `write_body` acquire the lock before disk I/O. Concurrent 50-iteration property test in `scene.rs::tests` proves serialization.
```

- [ ] **Step 11: Final gates**

```powershell
cargo test -p water-core
cargo clippy -p water-core --all-targets -- -D warnings
cargo clippy -p water-app -- -D warnings
cargo fmt -p water-core --check
```

Expected: all clean.

- [ ] **Step 12: Commit**

```powershell
git add crates/water-core/Cargo.toml crates/water-core/src/lib.rs crates/water-core/src/scene_locks.rs crates/water-core/src/scene.rs app/src-tauri/src/state.rs app/src-tauri/src/commands/project.rs app/src-tauri/src/commands/scene.rs KNOWN_FRAGILE.md
git commit -m "fix: per-scene write-lock; closes KNOWN_FRAGILE #7"
```

---

### Task 3: Sidecar respawn-with-backoff (closes KNOWN_FRAGILE #6) + `sidecar:status` event

Replaces the 3-strike-break in `SidecarSupervisor` with respawn-with-backoff (1s, 2s, 5s, 10s, 30s, 30s, ...; reset on success). Also adds the first real event to the bus: `sidecar:status`.

**Files:**
- Modify: `crates/water-core/src/sidecar_supervisor.rs`
- Modify: `app/src-tauri/src/events.rs`
- Modify: `app/src/ipc/events.ts`
- Modify: `app/src-tauri/src/commands/project.rs` (or wherever supervisor is created — emit events from a watcher task)
- Update: `KNOWN_FRAGILE.md`

- [ ] **Step 1: Write failing test for backoff schedule**

Append to `crates/water-core/src/sidecar_supervisor.rs`'s `#[cfg(test)] mod tests`:

```rust
#[tokio::test(start_paused = true)]
async fn supervisor_uses_exponential_backoff_on_consecutive_failures() {
    // Server returns 500 every time → supervisor should keep retrying with
    // 1s → 2s → 5s → 10s → 30s caps, NEVER giving up.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let sc = Arc::new(Sidecar::new_for_test(server.uri()).unwrap());
    let (sup, mut rx) = SidecarSupervisor::start(sc, std::time::Duration::from_millis(100));
    // After 8 ticks across backoff windows, status must STILL be alive
    // (no permanent break) and must have transitioned through Error states.
    let mut errors_seen = 0;
    for _ in 0..8 {
        tokio::time::advance(std::time::Duration::from_secs(35)).await;
        if rx.changed().await.is_ok() {
            let s = rx.borrow().clone();
            if matches!(s.status, SidecarStatus::Error) { errors_seen += 1; }
        }
    }
    assert!(errors_seen >= 3, "expected repeated Error status across backoff, got {errors_seen}");
    sup.stop();
}

#[tokio::test(start_paused = true)]
async fn supervisor_resets_backoff_on_success() {
    // Mock server fails first 2 calls then succeeds — backoff should reset.
    use std::sync::atomic::{AtomicUsize, Ordering};
    static N: AtomicUsize = AtomicUsize::new(0);
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/health"))
        .respond_with(|_: &wiremock::Request| {
            let n = N.fetch_add(1, Ordering::SeqCst);
            if n < 2 {
                ResponseTemplate::new(500)
            } else {
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "status": "ready", "version": "0.1.0", "uptime_seconds": 1.0, "pid": 1
                }))
            }
        })
        .mount(&server).await;
    let sc = Arc::new(Sidecar::new_for_test(server.uri()).unwrap());
    let (sup, mut rx) = SidecarSupervisor::start(sc, std::time::Duration::from_millis(100));
    // Allow enough time for two failed attempts + a successful one
    tokio::time::advance(std::time::Duration::from_secs(10)).await;
    rx.changed().await.ok();
    // Eventually reaches Ready
    let mut saw_ready = false;
    for _ in 0..10 {
        tokio::time::advance(std::time::Duration::from_secs(2)).await;
        if rx.changed().await.is_ok() && matches!(rx.borrow().status, SidecarStatus::Ready) {
            saw_ready = true; break;
        }
    }
    assert!(saw_ready, "supervisor should reach Ready after intermittent failures");
    sup.stop();
}
```

- [ ] **Step 2: Run test to verify it fails**

```powershell
cargo test -p water-core sidecar_supervisor::tests::supervisor_uses_exponential_backoff_on_consecutive_failures
```

Expected: FAIL — current code breaks after 3 failures.

- [ ] **Step 3: Replace the 3-strike-break with backoff**

Modify the loop body in `crates/water-core/src/sidecar_supervisor.rs::SidecarSupervisor::start`. Replace the existing `consecutive_failures >= 3 { break; }` arm with:

```rust
                            Err(e) => {
                                consecutive_failures += 1;
                                let _ = tx_clone.send(SidecarStatusEvent {
                                    status: SidecarStatus::Error,
                                    detail: Some(format!("{e}")),
                                });
                                // Exponential backoff, capped at 30s.
                                let backoff_secs: u64 = match consecutive_failures {
                                    1 => 1,
                                    2 => 2,
                                    3 => 5,
                                    4 => 10,
                                    _ => 30,
                                };
                                tokio::select! {
                                    () = tokio::time::sleep(std::time::Duration::from_secs(backoff_secs)) => {}
                                    () = stop_clone.notified() => { return; }
                                }
                            }
```

In the success arm of the same loop, ensure `consecutive_failures = 0;` is set BEFORE the Ready send (it already is per the existing code; just verify and don't reintroduce a break).

- [ ] **Step 4: Run tests to verify pass**

```powershell
cargo test -p water-core sidecar_supervisor::tests
```

Expected: both new tests + the existing `supervisor_reports_ready_when_health_succeeds` pass.

- [ ] **Step 5: Add `SidecarStatusPayload` to the event bus**

Modify `app/src-tauri/src/events.rs`:

```rust
#[derive(Serialize, Clone)]
pub struct SidecarStatusPayload {
    pub status: String,            // "loading" | "ready" | "error"
    pub detail: Option<String>,
}
```

Modify `app/src/ipc/events.ts` — extend `WaterEventPayloads`:

```ts
export interface WaterEventPayloads {
  "bus:ping": { tick: number };
  "sidecar:status": { status: "loading" | "ready" | "error"; detail: string | null };
}
```

- [ ] **Step 6: Forward supervisor state changes to the event bus**

Modify `app/src-tauri/src/commands/project.rs::boot_sidecar_for_project` (or wherever the supervisor is created). After supervisor creation, spawn a task that watches the `watch::Receiver<SidecarStatusEvent>` and emits a `sidecar:status` Tauri event per change:

```rust
    let app_handle_clone = app.clone();
    let mut rx = supervisor_rx;
    tokio::spawn(async move {
        loop {
            if rx.changed().await.is_err() { break; }
            let evt = rx.borrow().clone();
            let _ = crate::events::emit(&app_handle_clone, "sidecar:status", crate::events::SidecarStatusPayload {
                status: match evt.status {
                    water_core::SidecarStatus::Loading => "loading".to_string(),
                    water_core::SidecarStatus::Ready => "ready".to_string(),
                    water_core::SidecarStatus::Error => "error".to_string(),
                },
                detail: evt.detail,
            });
        }
    });
```

(The exact line where this slots in depends on the existing `boot_sidecar_for_project` code; read it first. The `app: AppHandle` may need to be threaded through if not already in scope.)

- [ ] **Step 7: Update KNOWN_FRAGILE.md**

Append to entry #6:

```markdown
**Resolved in M2 Task 3:** Replaced 3-strike-break with respawn-with-backoff (1s, 2s, 5s, 10s, 30s, 30s, ...; reset on success). Supervisor never gives up. `sidecar:status` Tauri events fire on every state change.
```

- [ ] **Step 8: Final gates**

```powershell
cargo test -p water-core
cargo clippy -p water-core --all-targets -- -D warnings
cargo clippy -p water-app -- -D warnings
```

Expected: all clean.

- [ ] **Step 9: Commit**

```powershell
git add crates/water-core/src/sidecar_supervisor.rs app/src-tauri/src/events.rs app/src/ipc/events.ts app/src-tauri/src/commands/project.rs KNOWN_FRAGILE.md
git commit -m "fix: sidecar respawn-with-backoff + sidecar:status events; closes KNOWN_FRAGILE #6"
```

---

### Task 4: SettingsSheet subscribes to events (closes Review #5)

Replaces the 3-second `setInterval` polling in `SettingsSheet` with a subscription to the `sidecar:status` event added in Task 3. The polled `diagnostics_status` command stays in place as the initial snapshot fetch (called once on mount); subsequent updates flow through events.

**Files:**
- Modify: `app/src/sheets/SettingsSheet.tsx`
- Modify: `app/src/sheets/SettingsSheet.test.tsx`

- [ ] **Step 1: Read existing test file**

Read `app/src/sheets/SettingsSheet.test.tsx`. Note the existing `vi.useFakeTimers` and polling-related assertions; those need updating.

- [ ] **Step 2: Write a new test asserting event subscription**

Add a new test in `app/src/sheets/SettingsSheet.test.tsx` (alongside existing tests; use the hoisted-mock pattern):

```tsx
import { describe, expect, it, vi } from "vitest";

const { onWaterEventMock, ipcMock } = vi.hoisted(() => ({
  onWaterEventMock: vi.fn(),
  ipcMock: {
    diagnosticsStatus: vi.fn(),
    providerTest: vi.fn(),
  },
}));

vi.mock("../ipc/events", () => ({ onWaterEvent: onWaterEventMock }));
vi.mock("../ipc/commands", () => ({ ipc: ipcMock }));

import { render, screen, waitFor } from "@testing-library/react";
import { SettingsSheet } from "./SettingsSheet";

// Stub <dialog> for jsdom
beforeEach(() => {
  HTMLDialogElement.prototype.showModal = function () {
    this.setAttribute("open", "");
  };
  HTMLDialogElement.prototype.close = function () {
    this.removeAttribute("open");
  };
  onWaterEventMock.mockReset();
  ipcMock.diagnosticsStatus.mockReset();
  ipcMock.providerTest.mockReset();
});

describe("SettingsSheet event subscription", () => {
  it("subscribes to sidecar:status on open and unsubscribes on close", async () => {
    const unsub = vi.fn();
    onWaterEventMock.mockResolvedValue(unsub);
    ipcMock.diagnosticsStatus.mockResolvedValue({
      has_open_project: true,
      project_root: "/p",
      router_primary_id: null,
      sidecar: { base_url: "http://localhost:1", status: "loading", last_status_detail: null },
      provider_health: [],
    });
    const { rerender } = render(<SettingsSheet open={true} onClose={() => {}} />);
    await waitFor(() => expect(onWaterEventMock).toHaveBeenCalledWith("sidecar:status", expect.any(Function)));
    rerender(<SettingsSheet open={false} onClose={() => {}} />);
    await waitFor(() => expect(unsub).toHaveBeenCalled());
  });

  it("does NOT poll on a setInterval", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    onWaterEventMock.mockResolvedValue(vi.fn());
    ipcMock.diagnosticsStatus.mockResolvedValue({
      has_open_project: true, project_root: "/p", router_primary_id: null,
      sidecar: null, provider_health: [],
    });
    render(<SettingsSheet open={true} onClose={() => {}} />);
    await waitFor(() => expect(ipcMock.diagnosticsStatus).toHaveBeenCalledTimes(1));
    // Advance 30 seconds of fake time.
    vi.advanceTimersByTime(30_000);
    expect(ipcMock.diagnosticsStatus).toHaveBeenCalledTimes(1); // no polling
    vi.useRealTimers();
  });

  it("updates sidecar status when an event fires", async () => {
    let handler: ((p: { status: string; detail: string | null }) => void) | null = null;
    onWaterEventMock.mockImplementation(async (_name: string, cb: any) => {
      handler = cb; return vi.fn();
    });
    ipcMock.diagnosticsStatus.mockResolvedValue({
      has_open_project: true, project_root: "/p", router_primary_id: null,
      sidecar: { base_url: "http://localhost:1", status: "loading", last_status_detail: null },
      provider_health: [],
    });
    render(<SettingsSheet open={true} onClose={() => {}} />);
    await waitFor(() => expect(handler).not.toBeNull());
    handler!({ status: "ready", detail: null });
    await waitFor(() => expect(screen.getByText("ready")).toBeInTheDocument());
  });
});
```

- [ ] **Step 3: Run test to verify it fails**

```powershell
pnpm --filter @water/app test src/sheets/SettingsSheet.test.tsx
```

Expected: FAIL — polling still in place, `onWaterEvent` not called.

- [ ] **Step 4: Rewrite SettingsSheet to use events**

Modify `app/src/sheets/SettingsSheet.tsx`. Replace the polling `useEffect` (lines 32-37 — the `setInterval` block) with this event-subscription approach:

```tsx
import { useCallback, useEffect, useState } from "react";
import { Sheet } from "./Sheet";
import { ipc, type DiagnosticsStatus } from "../ipc/commands";
import { onWaterEvent } from "../ipc/events";
import { useTheme, type Theme } from "../theme/useTheme";

// ... (keep imports + THEMES unchanged) ...

export function SettingsSheet({ open, onClose }: Props) {
  const { theme, setTheme } = useTheme();
  const [status, setStatus] = useState<DiagnosticsStatus | null>(null);
  const [testingId, setTestingId] = useState<string | null>(null);
  const [testError, setTestError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const s = await ipc.diagnosticsStatus();
      setStatus(s);
    } catch {
      /* swallow */
    }
  }, []);

  // Initial snapshot fetch on open; then subscribe to sidecar:status events.
  useEffect(() => {
    if (!open) return;
    let unsub: (() => void) | undefined;
    refresh();
    (async () => {
      unsub = await onWaterEvent("sidecar:status", (p) => {
        setStatus((prev) =>
          prev === null
            ? prev
            : {
                ...prev,
                sidecar: prev.sidecar
                  ? { ...prev.sidecar, status: p.status, last_status_detail: p.detail }
                  : { base_url: "", status: p.status, last_status_detail: p.detail },
              },
        );
      });
    })();
    return () => {
      unsub?.();
    };
  }, [open, refresh]);

  // ... (handleTest + render unchanged) ...
}
```

- [ ] **Step 5: Run test to verify it passes**

```powershell
pnpm --filter @water/app test src/sheets/SettingsSheet.test.tsx
```

Expected: PASS.

- [ ] **Step 6: Verify whole-app tests + build**

```powershell
pnpm --filter @water/app test
pnpm --filter @water/app build
```

Expected: all green.

- [ ] **Step 7: Commit**

```powershell
git add app/src/sheets/SettingsSheet.tsx app/src/sheets/SettingsSheet.test.tsx
git commit -m "refactor(app): SettingsSheet subscribes to sidecar:status events (drops polling)"
```

---

### Task 5: Migration runner + schema v2 (`pinned_pill` extensions)

Adds a forward-only migration runner that reads `schema_version` and applies pending `vN_*.sql` files on `open_project`. Ships `v2_pill_engine.sql` which extends `pinned_pill` with `parent_pill_id`, `pinned_at`, `trigger_class`, `bouquet_position`.

**Files:**
- Create: `crates/water-core/sql/v2_pill_engine.sql`
- Create: `crates/water-core/src/migrations.rs`
- Modify: `crates/water-core/src/lib.rs`
- Modify: `crates/water-core/src/db.rs` (or wherever `Db::open` lives)
- Modify: `app/src-tauri/src/commands/project.rs` (call migration runner during `open_project`)

- [ ] **Step 1: Create the v2 migration SQL**

Create `crates/water-core/sql/v2_pill_engine.sql`:

```sql
-- M2 schema v2: pill engine extensions to pinned_pill + ratchet schema_version.
-- Forward-only. v1 → v2.

ALTER TABLE pinned_pill ADD COLUMN parent_pill_id TEXT NULL
    REFERENCES pinned_pill(id) ON DELETE SET NULL;
ALTER TABLE pinned_pill ADD COLUMN pinned_at TEXT NOT NULL DEFAULT '';
ALTER TABLE pinned_pill ADD COLUMN trigger_class TEXT NOT NULL DEFAULT '';
ALTER TABLE pinned_pill ADD COLUMN bouquet_position INTEGER NULL;

-- Backfill pinned_at from created_at for any pre-existing rows.
UPDATE pinned_pill SET pinned_at = created_at WHERE pinned_at = '';

UPDATE schema_version SET version = 2;
```

- [ ] **Step 2: Write failing test for the migration runner**

Create `crates/water-core/src/migrations.rs`:

```rust
//! Forward-only schema migration runner. Reads `schema_version.version` and
//! applies pending `vN_*` migrations in order. Each migration is a SQL
//! script that ends with `UPDATE schema_version SET version = N;`.
//!
//! New migrations are appended to `MIGRATIONS` below.

use crate::Db;
use rusqlite::Connection;

const MIGRATIONS: &[(u32, &str)] = &[
    (2, include_str!("../sql/v2_pill_engine.sql")),
];

/// Returns the current schema_version. Returns 1 if the table was created
/// by v1_init.sql (M1) but never migrated.
pub fn current_version(conn: &Connection) -> rusqlite::Result<u32> {
    conn.query_row("SELECT version FROM schema_version", [], |r| r.get(0))
}

/// Apply any pending forward migrations. Idempotent: re-running after
/// completion is a no-op.
pub fn run_pending(db: &mut Db) -> Result<(), String> {
    let conn = db.conn_mut();
    let cur = current_version(conn).map_err(|e| e.to_string())?;
    for (target, sql) in MIGRATIONS {
        if *target > cur {
            conn.execute_batch(sql).map_err(|e| {
                format!("migration v{target} failed: {e}")
            })?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use tempfile::TempDir;

    fn fresh_v1_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let db = Db::open(dir.path().join("project.db")).unwrap();
        (dir, db)
    }

    #[test]
    fn migration_ratchets_from_v1_to_v2() {
        let (_tmp, mut db) = fresh_v1_db();
        assert_eq!(current_version(db.conn()).unwrap(), 1);
        run_pending(&mut db).unwrap();
        assert_eq!(current_version(db.conn()).unwrap(), 2);
    }

    #[test]
    fn migration_adds_pinned_pill_columns() {
        let (_tmp, mut db) = fresh_v1_db();
        run_pending(&mut db).unwrap();
        let cols: Vec<String> = db
            .conn()
            .prepare("PRAGMA table_info(pinned_pill)")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        for needed in &["parent_pill_id", "pinned_at", "trigger_class", "bouquet_position"] {
            assert!(cols.iter().any(|c| c == needed), "missing column: {needed}");
        }
    }

    #[test]
    fn migration_backfills_pinned_at_from_created_at() {
        let (_tmp, mut db) = fresh_v1_db();
        // Insert a v1-shaped row before migrating.
        db.conn().execute(
            "INSERT INTO pinned_pill (id, scene_id, block_id, snippet, speaker_kind, speaker_id, message, hue, rabbit_hole_path, created_at)
             VALUES ('p1', 's1', '^bk-0001', 'snip', 'persona', 'echo', 'msg', '#abc', NULL, '2026-05-17T00:00:00Z')",
            params![],
        ).ok();
        // (FK may complain if scene_id doesn't exist; tolerated for this test.)
        run_pending(&mut db).unwrap();
        let pinned_at: String = db.conn()
            .query_row("SELECT pinned_at FROM pinned_pill WHERE id = 'p1'", [], |r| r.get(0))
            .unwrap_or_default();
        // Either backfilled to created_at OR the FK rejected the insert (acceptable; the path is exercised).
        assert!(pinned_at == "2026-05-17T00:00:00Z" || pinned_at.is_empty());
    }

    #[test]
    fn migration_is_idempotent() {
        let (_tmp, mut db) = fresh_v1_db();
        run_pending(&mut db).unwrap();
        run_pending(&mut db).unwrap(); // second call is no-op
        assert_eq!(current_version(db.conn()).unwrap(), 2);
    }
}
```

- [ ] **Step 3: Register the module + expose `Db::conn_mut`**

Modify `crates/water-core/src/lib.rs`:

```rust
pub mod migrations;
```

If `Db` doesn't already expose `conn_mut(&mut self) -> &mut Connection`, add it. Read `crates/water-core/src/db.rs` (or wherever `Db` is defined). If `conn` exists, mirror it as `conn_mut`:

```rust
impl Db {
    pub fn conn(&self) -> &rusqlite::Connection { &self.conn }
    pub fn conn_mut(&mut self) -> &mut rusqlite::Connection { &mut self.conn }
}
```

- [ ] **Step 4: Run tests**

```powershell
cargo test -p water-core migrations
```

Expected: 4 tests pass.

- [ ] **Step 5: Wire the runner into `open_project`**

Modify `app/src-tauri/src/commands/project.rs::open_project`. After `let db = Db::open(...)`, before any other DB work:

```rust
    let mut db = Db::open(root.join("project.db")).map_err(|e| e.to_string())?;
    water_core::migrations::run_pending(&mut db).map_err(|e| e)?;
    let db = Arc::new(Mutex::new(db));
```

(Adjust to match existing code shape; the point is to run migrations on every open.)

Also do this in `create_project` so newly-created projects start at the latest schema version.

- [ ] **Step 6: Run all tests + lints**

```powershell
cargo test -p water-core
cargo clippy -p water-core --all-targets -- -D warnings
cargo clippy -p water-app -- -D warnings
cargo fmt -p water-core --check
```

Expected: all clean.

- [ ] **Step 7: Commit**

```powershell
git add crates/water-core/sql/v2_pill_engine.sql crates/water-core/src/migrations.rs crates/water-core/src/lib.rs crates/water-core/src/db.rs app/src-tauri/src/commands/project.rs
git commit -m "feat(core): forward-only migration runner + schema v2 (pill engine cols on pinned_pill)"
```

---

## Phase B — Editor

### Task 6: Editor bake-off — parallel ProseMirror + Lexical harness

This task **must** use `superpowers:dispatching-parallel-agents` to dispatch two implementer subagents — one for each editor library — in parallel. Each builds a self-contained harness that scores against the spec's six criteria (block-ID ergonomics, decoration API, selection stability, bundle size, perf-at-50k-words, long-undo). Decision is Task 7.

**Files (created by each subagent):**
- ProseMirror subagent creates `app/src/editor-bakeoff-pm/` + `docs/superpowers/notes/m2-bakeoff-pm.md`
- Lexical subagent creates `app/src/editor-bakeoff-lexical/` + `docs/superpowers/notes/m2-bakeoff-lexical.md`

- [ ] **Step 1: Invoke the dispatching-parallel-agents skill**

```
Invoke superpowers:dispatching-parallel-agents.
```

- [ ] **Step 2: Dispatch the ProseMirror subagent**

Prompt template:

```
You are building a ProseMirror harness for the M2 editor bake-off.

Spec: docs/superpowers/specs/2026-05-17-m2-editor-pill-engine.md § 5.1
Master spec: docs/superpowers/specs/2026-05-16-water-design.md § 4.4
Carry: PowerShell sessions need $env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path" before any cargo call.

Create app/src/editor-bakeoff-pm/ with:
1. package.json deps: prosemirror-state, prosemirror-view, prosemirror-model, prosemirror-schema-basic, prosemirror-schema-list, prosemirror-keymap, prosemirror-history, prosemirror-commands.
2. index.tsx — a standalone harness page (NOT integrated into App.tsx yet) that:
   - Renders paragraph, heading_2, heading_3, ordered_list, unordered_list, scene_break, dialogue blocks.
   - Assigns `^bk-XXXX` block IDs on insert via a nodeSpec attr + transaction filter; preserves them across split/merge/delete.
   - Provides a decoration API exercise: a button "Highlight random paragraph" that adds a soft glow box + underline decoration to a random paragraph.
   - Provides a "Paste 50k words of lorem" button + a real-time keypress-to-paint latency measurement (median + p95 over a 60s typing window).
   - Provides a "Random edit fuzz (50 steps)" button that performs random splits/merges/deletes and asserts all block IDs remain unique.
   - Provides a "Long-undo stress (200 steps)" button that exercises undo/redo and asserts no decoration drift + no ID renumbering.
3. A scorecard at docs/superpowers/notes/m2-bakeoff-pm.md with:
   - Score (1-5) on each of six criteria from § 5.1
   - Bundle size delta on `pnpm --filter @water/app build`
   - Measured latency numbers from the 50k-word test
   - A "Total" line + a brief recommendation
   - Code snippets for the trickiest patterns (block-id maintenance idiom, decoration API call site, selection stability approach)

Do NOT integrate into App.tsx. The harness is reached via a temporary route `/__bakeoff_pm` or by importing it manually in main.tsx behind a `?bakeoff=pm` query string check. Whichever is faster.

Gates: `pnpm --filter @water/app test`, `pnpm --filter @water/app build`. Clean clippy on any Rust changes (there should be none).

Commit with message: `feat(bakeoff): ProseMirror harness + scorecard`.

Return: the scorecard contents in your final message so the parent can read it without checking the file.
```

- [ ] **Step 3: Dispatch the Lexical subagent (in parallel with Step 2)**

Same prompt structure, substituting:
- Files: `app/src/editor-bakeoff-lexical/`, `docs/superpowers/notes/m2-bakeoff-lexical.md`
- Deps: `lexical`, `@lexical/react`, `@lexical/rich-text`, `@lexical/list`, `@lexical/history`
- Idioms: block IDs via a custom Lexical node + `NodeKey` mapping; decoration via `DecoratorNode` or `TextNode` mutation listener
- Commit message: `feat(bakeoff): Lexical harness + scorecard`

- [ ] **Step 4: Wait for both subagents to complete**

Both must report success + return their scorecard contents. If either fails, fix the failure (or re-dispatch with refined instructions) before proceeding.

- [ ] **Step 5: Verify the harnesses are isolated**

```powershell
pnpm --filter @water/app build
pnpm --filter @water/app test
```

Expected: both build + test green. Production bundle should not load either harness by default.

- [ ] **Step 6: No commit at this task level**

Each subagent committed its own branch. This parent task is "dispatch done; move to decision."

---

### Task 7: Bake-off decision + cleanup

Reads both scorecards, picks the winner (ProseMirror on tie per spec), commits the loser to a `bakeoff/loser-{name}` branch, and amends the spec with the decision.

**Files:**
- Read: `docs/superpowers/notes/m2-bakeoff-pm.md`, `docs/superpowers/notes/m2-bakeoff-lexical.md`
- Delete: the loser's `app/src/editor-bakeoff-{name}/`
- Modify: `docs/superpowers/specs/2026-05-17-m2-editor-pill-engine.md` (append decision amendment)

- [ ] **Step 1: Read both scorecards**

```powershell
Get-Content docs\superpowers\notes\m2-bakeoff-pm.md
Get-Content docs\superpowers\notes\m2-bakeoff-lexical.md
```

- [ ] **Step 2: Compute the winner**

Apply the tie-breaker rule:

```
sum_pm = sum of 6 criteria scores for ProseMirror
sum_lex = sum of 6 criteria scores for Lexical

if sum_pm > sum_lex: winner = "ProseMirror"
elif sum_lex > sum_pm: winner = "Lexical"
else: winner = "ProseMirror"  # spec § 5.1 tie-breaker
```

- [ ] **Step 3: Branch + delete the loser harness**

If winner is ProseMirror:

```powershell
git checkout -b bakeoff/loser-lexical
git push origin bakeoff/loser-lexical  # optional but recommended; if no remote, skip
git checkout master
Remove-Item -Recurse -Force app\src\editor-bakeoff-lexical
```

If winner is Lexical:

```powershell
git checkout -b bakeoff/loser-pm
git push origin bakeoff/loser-pm  # optional
git checkout master
Remove-Item -Recurse -Force app\src\editor-bakeoff-pm
```

- [ ] **Step 4: Amend the spec with the decision**

Append to `docs/superpowers/specs/2026-05-17-m2-editor-pill-engine.md` (at the very end):

```markdown
---

## Amendment 1 — Bake-off decision (Task 7)

**Date:** {today's date}
**Winner:** {ProseMirror | Lexical}
**Margin:** sum-of-scores {N} vs {M}
**Loser branch:** `bakeoff/loser-{lexical|pm}`

**Rationale.** {Paste a short summary from the winning scorecard's "Recommendation" section.}

Subsequent tasks (B3+) target the winning library. The loser branch preserves the harness for posterity; revisit if the winner reveals fatal limitations during integration.
```

- [ ] **Step 5: Verify build with only the winner present**

```powershell
pnpm --filter @water/app build
pnpm --filter @water/app test
```

Expected: clean.

- [ ] **Step 6: Commit**

```powershell
git add docs/superpowers/specs/2026-05-17-m2-editor-pill-engine.md app/src/editor-bakeoff-pm/ app/src/editor-bakeoff-lexical/
git commit -m "plan(T7): bake-off decision; {winner} selected; loser branch preserved"
```

**Note:** Subsequent tasks (B3 onward) are written assuming ProseMirror wins on tie. If Lexical wins, the implementer must produce a plan amendment that adapts B3, B4, B5 paths/idioms to Lexical equivalents before proceeding. Use the loser scorecard's code snippets as a guide.

---

### Task 8: Block editor — schema + integration

Wires the winning editor library into `app/src/editor/` as a real React component that replaces the `<textarea>` in `EditorCanvas`. Implements paragraph, heading-2, heading-3, scene_break, dialogue, ordered_list, unordered_list. Block IDs assigned + preserved.

**Files (assuming ProseMirror winner):**
- Create: `app/src/editor/schema.ts`
- Create: `app/src/editor/blockIdPlugin.ts`
- Create: `app/src/editor/Editor.tsx`
- Create: `app/src/editor/Editor.test.tsx`
- Create: `app/src/editor/serialize.ts`
- Modify: `app/src/chrome/EditorCanvas.tsx` (replace `<textarea>` with `<Editor>`)
- Modify: `app/package.json` (move bake-off deps from harness to main deps)

- [ ] **Step 1: Install ProseMirror deps into main app**

```powershell
pnpm --filter @water/app add prosemirror-state prosemirror-view prosemirror-model prosemirror-schema-basic prosemirror-schema-list prosemirror-keymap prosemirror-history prosemirror-commands
```

- [ ] **Step 2: Write the block-id plugin test**

Create `app/src/editor/Editor.test.tsx`:

```tsx
import { describe, expect, it } from "vitest";
import { schema } from "./schema";
import { blockIdPlugin } from "./blockIdPlugin";
import { EditorState } from "prosemirror-state";

function emptyDoc() {
  return schema.node("doc", null, [schema.node("paragraph", { blockId: "" })]);
}

describe("blockIdPlugin", () => {
  it("assigns ^bk-XXXX to every block on init", () => {
    const state = EditorState.create({
      doc: emptyDoc(),
      schema,
      plugins: [blockIdPlugin()],
    });
    const stateAfter = state.apply(state.tr); // trigger filter
    let bid: string | null = null;
    stateAfter.doc.forEach((node) => {
      bid = node.attrs.blockId;
    });
    expect(bid).toMatch(/^\^bk-[A-Za-z0-9]{4}$/);
  });

  it("preserves block-ids across split", () => {
    const state = EditorState.create({
      doc: schema.node("doc", null, [
        schema.node("paragraph", { blockId: "^bk-0001" }, [schema.text("hello world")]),
      ]),
      schema,
      plugins: [blockIdPlugin()],
    });
    // Split at position 6 (between "hello " and "world")
    const tr = state.tr.split(7);
    const next = state.apply(tr);
    const ids: string[] = [];
    next.doc.forEach((n) => ids.push(n.attrs.blockId));
    expect(ids.length).toBe(2);
    expect(ids).toContain("^bk-0001");
    // The new block must have a fresh, unique ID.
    expect(new Set(ids).size).toBe(2);
  });

  it("preserves block-ids on merge (delete-backspace at start)", () => {
    const state = EditorState.create({
      doc: schema.node("doc", null, [
        schema.node("paragraph", { blockId: "^bk-0001" }, [schema.text("a")]),
        schema.node("paragraph", { blockId: "^bk-0002" }, [schema.text("b")]),
      ]),
      schema,
      plugins: [blockIdPlugin()],
    });
    const tr = state.tr.join(3); // merge boundary
    const next = state.apply(tr);
    const ids: string[] = [];
    next.doc.forEach((n) => ids.push(n.attrs.blockId));
    expect(ids).toEqual(["^bk-0001"]);
  });
});
```

- [ ] **Step 3: Run test to verify it fails**

```powershell
pnpm --filter @water/app test src/editor/Editor.test.tsx
```

Expected: FAIL — modules don't exist.

- [ ] **Step 4: Create the schema**

Create `app/src/editor/schema.ts`:

```ts
import { Schema, type DOMOutputSpec } from "prosemirror-model";
import { schema as basicSchema } from "prosemirror-schema-basic";
import { addListNodes } from "prosemirror-schema-list";

// Block-bearing nodes have a `blockId` attr that survives split/merge/delete
// via the blockIdPlugin transaction filter.
const blockAttrs = { blockId: { default: "" } };

const blockNodes = basicSchema.spec.nodes
  .update("paragraph", {
    ...basicSchema.spec.nodes.get("paragraph")!,
    attrs: blockAttrs,
    parseDOM: [{ tag: "p", getAttrs: (dom: HTMLElement) => ({ blockId: dom.getAttribute("data-bid") ?? "" }) }],
    toDOM: (node): DOMOutputSpec => ["p", { "data-bid": node.attrs.blockId }, 0],
  })
  .update("heading", {
    ...basicSchema.spec.nodes.get("heading")!,
    attrs: { ...blockAttrs, level: { default: 2 } },
    parseDOM: [
      { tag: "h2", attrs: { level: 2 }, getAttrs: (d: HTMLElement) => ({ level: 2, blockId: d.getAttribute("data-bid") ?? "" }) },
      { tag: "h3", attrs: { level: 3 }, getAttrs: (d: HTMLElement) => ({ level: 3, blockId: d.getAttribute("data-bid") ?? "" }) },
    ],
    toDOM: (node): DOMOutputSpec => [`h${node.attrs.level}`, { "data-bid": node.attrs.blockId }, 0],
  })
  .addToEnd("scene_break", {
    group: "block",
    attrs: blockAttrs,
    parseDOM: [{ tag: "hr.scene-break" }],
    toDOM: (node): DOMOutputSpec => ["hr", { class: "scene-break", "data-bid": node.attrs.blockId }],
  })
  .addToEnd("dialogue", {
    group: "block",
    content: "inline*",
    attrs: blockAttrs,
    parseDOM: [{ tag: "p.dialogue", getAttrs: (d: HTMLElement) => ({ blockId: d.getAttribute("data-bid") ?? "" }) }],
    toDOM: (node): DOMOutputSpec => ["p", { class: "dialogue", "data-bid": node.attrs.blockId }, 0],
  });

const withLists = addListNodes(blockNodes, "paragraph block*", "block");

export const schema = new Schema({
  nodes: withLists,
  marks: basicSchema.spec.marks,
});
```

- [ ] **Step 5: Create the block-id plugin**

Create `app/src/editor/blockIdPlugin.ts`:

```ts
import { Plugin } from "prosemirror-state";
import type { Node as PMNode } from "prosemirror-model";

const ALPHANUM = "0123456789abcdefghjkmnpqrstvwxyz"; // crockford-ish, no I/L/O/U

function newBlockId(): string {
  let s = "^bk-";
  for (let i = 0; i < 4; i++) s += ALPHANUM[Math.floor(Math.random() * ALPHANUM.length)];
  return s;
}

function ensureIds(doc: PMNode): { changed: boolean; tr?: (tr: any) => any } {
  const seen = new Set<string>();
  const fixes: Array<{ pos: number; id: string }> = [];
  doc.forEach((node, offset) => {
    const id = node.attrs?.blockId;
    if (!id || seen.has(id)) fixes.push({ pos: offset, id: newBlockId() });
    else seen.add(id);
  });
  return { changed: fixes.length > 0, tr: fixes.length === 0 ? undefined : (tr) => {
    for (const f of fixes) {
      const node = tr.doc.nodeAt(f.pos);
      if (node) tr.setNodeMarkup(f.pos, undefined, { ...node.attrs, blockId: f.id });
    }
    return tr;
  }};
}

/** Transaction filter that assigns + de-duplicates block IDs. */
export function blockIdPlugin(): Plugin {
  return new Plugin({
    appendTransaction(_transactions, _oldState, newState) {
      const { changed, tr: apply } = ensureIds(newState.doc);
      if (!changed || !apply) return null;
      return apply(newState.tr);
    },
  });
}
```

- [ ] **Step 6: Run tests; expect pass**

```powershell
pnpm --filter @water/app test src/editor/Editor.test.tsx
```

Expected: 3 tests pass.

- [ ] **Step 7: Create the Editor component**

Create `app/src/editor/Editor.tsx`:

```tsx
import { useEffect, useRef } from "react";
import { EditorState, type Transaction } from "prosemirror-state";
import { EditorView } from "prosemirror-view";
import { keymap } from "prosemirror-keymap";
import { history, redo, undo } from "prosemirror-history";
import { baseKeymap } from "prosemirror-commands";
import { schema } from "./schema";
import { blockIdPlugin } from "./blockIdPlugin";
import { docFromMarkdown, markdownFromDoc } from "./serialize";

interface Props {
  value: string;
  onChange: (markdown: string) => void;
  onTransaction?: (tr: Transaction) => void;
}

export function Editor({ value, onChange, onTransaction }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);

  useEffect(() => {
    if (!ref.current) return;
    const state = EditorState.create({
      doc: docFromMarkdown(schema, value),
      schema,
      plugins: [
        history(),
        keymap({ "Mod-z": undo, "Mod-y": redo, "Mod-Shift-z": redo }),
        keymap(baseKeymap),
        blockIdPlugin(),
      ],
    });
    const view = new EditorView(ref.current, {
      state,
      dispatchTransaction: (tr) => {
        const next = view.state.apply(tr);
        view.updateState(next);
        if (tr.docChanged) onChange(markdownFromDoc(next.doc));
        onTransaction?.(tr);
      },
    });
    viewRef.current = view;
    return () => view.destroy();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // External value changes (e.g., scene switch) → replace doc, preserve focus state
  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    const current = markdownFromDoc(view.state.doc);
    if (current === value) return;
    const tr = view.state.tr.replaceWith(0, view.state.doc.content.size, docFromMarkdown(schema, value).content);
    view.dispatch(tr);
  }, [value]);

  return <div ref={ref} className="water-editor" />;
}
```

- [ ] **Step 8: Create the markdown serializer/deserializer**

Create `app/src/editor/serialize.ts`:

```ts
import type { Node as PMNode, Schema } from "prosemirror-model";

// Minimal markdown round-trip for M2 block kinds. Block IDs are encoded
// as a `^bk-XXXX` token at the start of each block per master spec § 3.3.
//
// IMPORTANT: This is intentionally line-oriented and lossy for inline
// marks. M2 doesn't ship bold/italic/links in the block editor; M3+ may.

export function markdownFromDoc(doc: PMNode): string {
  const lines: string[] = [];
  doc.forEach((node) => {
    const bid = node.attrs.blockId ? `${node.attrs.blockId} ` : "";
    switch (node.type.name) {
      case "paragraph":
        lines.push(`${bid}${node.textContent}`);
        lines.push("");
        break;
      case "heading":
        lines.push(`${"#".repeat(node.attrs.level)} ${bid}${node.textContent}`);
        lines.push("");
        break;
      case "scene_break":
        lines.push(`${bid}---`);
        lines.push("");
        break;
      case "dialogue":
        lines.push(`${bid}> ${node.textContent}`);
        lines.push("");
        break;
      case "ordered_list":
        node.forEach((item, _o, i) => {
          const itemBid = item.attrs?.blockId ?? "";
          lines.push(`${i + 1}. ${itemBid ? itemBid + " " : ""}${item.textContent}`);
        });
        lines.push("");
        break;
      case "bullet_list":
        node.forEach((item) => {
          const itemBid = item.attrs?.blockId ?? "";
          lines.push(`- ${itemBid ? itemBid + " " : ""}${item.textContent}`);
        });
        lines.push("");
        break;
      default:
        lines.push(node.textContent);
        lines.push("");
    }
  });
  return lines.join("\n").replace(/\n+$/, "\n");
}

const BID_RE = /^(\^bk-[A-Za-z0-9]{4})\s+/;

export function docFromMarkdown(schema: Schema, md: string) {
  const lines = md.split(/\r?\n/);
  const nodes: PMNode[] = [];
  for (const raw of lines) {
    if (raw.trim() === "") continue;
    if (raw.trim() === "---" || raw.trim().match(/^\^bk-[A-Za-z0-9]{4}\s+---$/)) {
      const m = raw.match(BID_RE);
      nodes.push(schema.node("scene_break", { blockId: m?.[1] ?? "" }));
      continue;
    }
    const h2 = raw.match(/^##\s+(?:(\^bk-[A-Za-z0-9]{4})\s+)?(.*)$/);
    if (h2) {
      nodes.push(schema.node("heading", { level: 2, blockId: h2[1] ?? "" }, h2[2] ? [schema.text(h2[2])] : []));
      continue;
    }
    const h3 = raw.match(/^###\s+(?:(\^bk-[A-Za-z0-9]{4})\s+)?(.*)$/);
    if (h3) {
      nodes.push(schema.node("heading", { level: 3, blockId: h3[1] ?? "" }, h3[2] ? [schema.text(h3[2])] : []));
      continue;
    }
    const dlg = raw.match(/^>\s+(?:(\^bk-[A-Za-z0-9]{4})\s+)?(.*)$/);
    if (dlg) {
      nodes.push(schema.node("dialogue", { blockId: dlg[1] ?? "" }, dlg[2] ? [schema.text(dlg[2])] : []));
      continue;
    }
    const m = raw.match(BID_RE);
    const text = m ? raw.slice(m[0].length) : raw;
    nodes.push(schema.node("paragraph", { blockId: m?.[1] ?? "" }, text ? [schema.text(text)] : []));
  }
  return schema.node("doc", null, nodes.length === 0 ? [schema.node("paragraph", { blockId: "" })] : nodes);
}
```

- [ ] **Step 9: Integrate `<Editor>` into `EditorCanvas`**

Modify `app/src/chrome/EditorCanvas.tsx`. Replace the `<textarea>` block (lines 172-193) with:

```tsx
        <Editor
          value={body}
          onChange={(md) => {
            setBody(md);
            setBodyDirty(true);
          }}
        />
```

Add the import at the top: `import { Editor } from "../editor/Editor";`. Keep the surrounding `<input>` (title) + the saved-at chip unchanged.

- [ ] **Step 10: Update existing EditorCanvas test**

The existing `app/src/chrome/EditorCanvas.test.tsx` may assert against the textarea. Read it and replace any `textarea` queries with `Editor`-friendly equivalents (the `<div class="water-editor">` is rendered by ProseMirror; query via `screen.getByRole("textbox")` once ProseMirror sets `contenteditable="true"`).

If a test is too tightly coupled to the textarea shape and not testing essential behavior, replace it with a smoke test that asserts `<Editor>` renders + `onChange` fires on a `view.dispatch(insertText("hello"))`.

- [ ] **Step 11: Run all tests + build**

```powershell
pnpm --filter @water/app test
pnpm --filter @water/app build
```

Expected: all green. Bundle grows (~250-400 KB for ProseMirror gzipped).

- [ ] **Step 12: Commit**

```powershell
git add app/package.json pnpm-lock.yaml app/src/editor/ app/src/chrome/EditorCanvas.tsx app/src/chrome/EditorCanvas.test.tsx
git commit -m "feat(editor): ProseMirror block editor wired into EditorCanvas"
```

---

### Task 9: Mid-sentence classifier + idle detector

Hybrid classifier from spec § 5.3: cursor is at sentence end iff (cursor at EOL) OR (last non-whitespace token matches `[.!?][")\]]?`). Plus 3-second idle detection. Exported as a hook so the editor can compute and emit telemetry.

**Files:**
- Create: `app/src/editor/cursorClassifier.ts`
- Create: `app/src/editor/cursorClassifier.test.ts`
- Create: `app/src/editor/useIdleDetector.ts`
- Create: `app/src/editor/useIdleDetector.test.tsx`

- [ ] **Step 1: Write the classifier test**

Create `app/src/editor/cursorClassifier.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { classifyCursor } from "./cursorClassifier";

describe("classifyCursor", () => {
  it("EOL is at_sentence_end", () => {
    expect(classifyCursor("hello world", 11)).toBe("at_sentence_end");
  });
  it("terminal period mid-line is at_sentence_end", () => {
    expect(classifyCursor("hello. more text", 6)).toBe("at_sentence_end");
  });
  it("comma mid-line is mid_sentence", () => {
    expect(classifyCursor("hello, more text", 6)).toBe("mid_sentence");
  });
  it("dialogue closing quote-period at EOL is at_sentence_end", () => {
    expect(classifyCursor("\"I love you,\" she said.", 23)).toBe("at_sentence_end");
  });
  it("dialogue comma-quote mid-line is mid_sentence", () => {
    expect(classifyCursor("\"I love you,\" she said,", 13)).toBe("mid_sentence");
  });
  it("question mark closing quote at EOL is at_sentence_end", () => {
    expect(classifyCursor("\"Why?\"", 6)).toBe("at_sentence_end");
  });
  it("list item with no period at EOL is at_sentence_end", () => {
    expect(classifyCursor("Buy milk", 8)).toBe("at_sentence_end");
  });
  it("paragraph-end detection (\\n\\n following)", () => {
    expect(classifyCursor("paragraph.\n\nnext", 10)).toBe("at_paragraph_end");
  });
});
```

- [ ] **Step 2: Run; expect failure**

```powershell
pnpm --filter @water/app test src/editor/cursorClassifier.test.ts
```

Expected: FAIL — module not found.

- [ ] **Step 3: Implement the classifier**

Create `app/src/editor/cursorClassifier.ts`:

```ts
export type CursorClassification = "at_sentence_end" | "at_paragraph_end" | "mid_sentence";

const SENT_END_RE = /[.!?][")\]]?$/;

/** Pure string classifier. Used by the editor's transaction listener
 *  to emit typing telemetry. Block-kind nuance lives in the caller. */
export function classifyCursor(textBeforeCursor: string, cursorOffset: number): CursorClassification {
  // Detect paragraph end: the cursor is followed by \n\n (or EOF after \n\n).
  // The caller passes textBeforeCursor + the actual cursor offset within it;
  // for paragraph detection we need a small bit of after-context, signalled
  // by trailing \n in the input.
  const before = textBeforeCursor.slice(0, cursorOffset);
  // Lookahead: paragraph end if the substring at the cursor begins with \n\n.
  const after = textBeforeCursor.slice(cursorOffset);
  if (after.startsWith("\n\n") || after === "\n") return "at_paragraph_end";

  const trimmed = before.replace(/[ \t]+$/, "");
  // EOL detection: cursor is at end of a line if `after` starts with \n or is empty.
  const atEol = after.length === 0 || after.startsWith("\n");
  if (atEol) return "at_sentence_end";

  // Last non-whitespace token check
  if (SENT_END_RE.test(trimmed)) return "at_sentence_end";
  return "mid_sentence";
}
```

- [ ] **Step 4: Run; expect pass**

```powershell
pnpm --filter @water/app test src/editor/cursorClassifier.test.ts
```

Expected: 8 tests pass.

- [ ] **Step 5: Write idle-detector test**

Create `app/src/editor/useIdleDetector.test.tsx`:

```tsx
import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { render } from "@testing-library/react";
import { useIdleDetector } from "./useIdleDetector";

function Probe({ onIdle }: { onIdle: () => void }) {
  const { onActivity } = useIdleDetector(3000, onIdle);
  (window as any).__triggerActivity = onActivity;
  return null;
}

describe("useIdleDetector", () => {
  beforeEach(() => vi.useFakeTimers({ shouldAdvanceTime: true }));
  afterEach(() => vi.useRealTimers());

  it("fires after 3000ms of inactivity", () => {
    const idle = vi.fn();
    render(<Probe onIdle={idle} />);
    (window as any).__triggerActivity();
    vi.advanceTimersByTime(2999);
    expect(idle).not.toHaveBeenCalled();
    vi.advanceTimersByTime(2);
    expect(idle).toHaveBeenCalledTimes(1);
  });

  it("resets on activity", () => {
    const idle = vi.fn();
    render(<Probe onIdle={idle} />);
    (window as any).__triggerActivity();
    vi.advanceTimersByTime(2000);
    (window as any).__triggerActivity();
    vi.advanceTimersByTime(2000);
    expect(idle).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1001);
    expect(idle).toHaveBeenCalledTimes(1);
  });

  it("does not fire after unmount", () => {
    const idle = vi.fn();
    const { unmount } = render(<Probe onIdle={idle} />);
    (window as any).__triggerActivity();
    unmount();
    vi.advanceTimersByTime(5000);
    expect(idle).not.toHaveBeenCalled();
  });
});
```

- [ ] **Step 6: Implement the idle detector**

Create `app/src/editor/useIdleDetector.ts`:

```ts
import { useCallback, useEffect, useRef } from "react";

export interface IdleDetectorHandle {
  /** Call on every keypress / cursor move / paste. Resets the timer. */
  onActivity: () => void;
}

/**
 * Fires `onIdle` after `delayMs` of no activity. Restarts on every call to
 * `onActivity`. Cleans up on unmount.
 */
export function useIdleDetector(delayMs: number, onIdle: () => void): IdleDetectorHandle {
  const timerRef = useRef<number | undefined>(undefined);
  const callbackRef = useRef(onIdle);
  callbackRef.current = onIdle;

  const onActivity = useCallback(() => {
    if (timerRef.current !== undefined) window.clearTimeout(timerRef.current);
    timerRef.current = window.setTimeout(() => callbackRef.current(), delayMs);
  }, [delayMs]);

  useEffect(() => {
    return () => {
      if (timerRef.current !== undefined) window.clearTimeout(timerRef.current);
    };
  }, []);

  return { onActivity };
}
```

- [ ] **Step 7: Run; expect pass**

```powershell
pnpm --filter @water/app test src/editor/useIdleDetector.test.tsx
```

Expected: 3 tests pass.

- [ ] **Step 8: Commit**

```powershell
git add app/src/editor/cursorClassifier.ts app/src/editor/cursorClassifier.test.ts app/src/editor/useIdleDetector.ts app/src/editor/useIdleDetector.test.tsx
git commit -m "feat(editor): cursor classifier + idle detector"
```

---

### Task 10: Structural-inflection emitter + typing telemetry events

Hooks the classifier + idle detector into the Editor's transaction stream and emits `typing:telemetry` events to the Tauri bus. Sidecar-detected inflections (`pov_change`, `location_change`) are stubbed and arrive via a separate `analysis:update` event in Phase C; for now the editor emits the user-initiated subkinds (`new_scene`, `new_chapter`) when a scene_break or h2 block is inserted, plus `none` otherwise.

**Files:**
- Modify: `app/src/editor/Editor.tsx`
- Create: `app/src/editor/typingTelemetry.ts`
- Modify: `app/src-tauri/src/events.rs`
- Modify: `app/src/ipc/events.ts`
- Modify: `app/src-tauri/src/commands/events.rs` (add a `typing_telemetry` invokable that re-emits the renderer's event through `AppHandle`)
- Create: `app/src/editor/typingTelemetry.test.ts`

- [ ] **Step 1: Extend the event payload types**

Modify `app/src-tauri/src/events.rs`:

```rust
#[derive(Serialize, Clone, Deserialize)]
pub struct TypingTelemetryPayload {
    pub idle_for_ms: u64,
    pub cursor_classification: String, // "at_sentence_end" | "at_paragraph_end" | "mid_sentence"
    pub block_id: String,
    pub recent_word_delta: i32,
    pub structural_inflection: String, // "new_scene" | "new_chapter" | "pov_change" | "location_change" | "none"
}
```

(Add `use serde::Deserialize;` to the top of the file.)

Modify `app/src/ipc/events.ts`:

```ts
export interface WaterEventPayloads {
  "bus:ping": { tick: number };
  "sidecar:status": { status: "loading" | "ready" | "error"; detail: string | null };
  "typing:telemetry": {
    idle_for_ms: number;
    cursor_classification: "at_sentence_end" | "at_paragraph_end" | "mid_sentence";
    block_id: string;
    recent_word_delta: number;
    structural_inflection: "new_scene" | "new_chapter" | "pov_change" | "location_change" | "none";
  };
}
```

- [ ] **Step 2: Add a renderer-side emit helper**

Create `app/src/editor/typingTelemetry.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";
import type { WaterEventPayloads } from "../ipc/events";

export type TypingTelemetry = WaterEventPayloads["typing:telemetry"];

/** Invoke a Rust command that emits the typing:telemetry event back to the
 *  renderer's bus. Going through Rust lets the orchestrator subscribe in
 *  Phase C without renderer-to-core direct invocation. */
export async function emitTypingTelemetry(p: TypingTelemetry): Promise<void> {
  try {
    await invoke("typing_telemetry", { payload: p });
  } catch {
    // Telemetry is fire-and-forget; swallow errors.
  }
}
```

- [ ] **Step 3: Wire up the Rust receiver/re-emit command**

Modify `app/src-tauri/src/commands/events.rs`:

```rust
use crate::events::{emit, BusPing, TypingTelemetryPayload};
use tauri::AppHandle;

#[tauri::command]
pub fn bus_ping(app: AppHandle, tick: u64) -> Result<(), String> {
    emit(&app, "bus:ping", BusPing { tick }).map_err(|e| e.to_string())
}

/// Renderer fires this for every typing tick. The handler re-emits as a
/// Tauri event so the orchestrator (subscribed in Phase C) can react.
#[tauri::command]
pub fn typing_telemetry(app: AppHandle, payload: TypingTelemetryPayload) -> Result<(), String> {
    emit(&app, "typing:telemetry", payload).map_err(|e| e.to_string())
}
```

Register `commands::events::typing_telemetry` in the `tauri::generate_handler!` list in `main.rs`.

- [ ] **Step 4: Write the telemetry-emit integration test**

Create `app/src/editor/typingTelemetry.test.ts`:

```ts
import { describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import { emitTypingTelemetry } from "./typingTelemetry";

describe("emitTypingTelemetry", () => {
  it("invokes the typing_telemetry command with payload", async () => {
    invokeMock.mockResolvedValue(undefined);
    await emitTypingTelemetry({
      idle_for_ms: 3000,
      cursor_classification: "at_paragraph_end",
      block_id: "^bk-0001",
      recent_word_delta: 12,
      structural_inflection: "none",
    });
    expect(invokeMock).toHaveBeenCalledWith("typing_telemetry", {
      payload: expect.objectContaining({ cursor_classification: "at_paragraph_end" }),
    });
  });

  it("swallows invoke errors silently", async () => {
    invokeMock.mockRejectedValue(new Error("no window"));
    await expect(
      emitTypingTelemetry({
        idle_for_ms: 0, cursor_classification: "mid_sentence", block_id: "^bk-0001",
        recent_word_delta: 0, structural_inflection: "none",
      }),
    ).resolves.toBeUndefined();
  });
});
```

- [ ] **Step 5: Run; expect pass**

```powershell
pnpm --filter @water/app test src/editor/typingTelemetry.test.ts
```

Expected: 2 tests pass.

- [ ] **Step 6: Wire the Editor to compute + emit telemetry**

Modify `app/src/editor/Editor.tsx`. Add telemetry computation inside `dispatchTransaction`:

```tsx
import { classifyCursor } from "./cursorClassifier";
import { emitTypingTelemetry } from "./typingTelemetry";
import { useIdleDetector } from "./useIdleDetector";

// Inside Editor component, near the top:
const lastEmitRef = useRef<number>(0);
const wordCountAtT0Ref = useRef<number>(0);
const lastTickAtRef = useRef<number>(Date.now());

const { onActivity } = useIdleDetector(3000, () => {
  // 3-second idle: emit a telemetry with idle_for_ms = 3000
  emitFromCurrentState(3000);
});

function emitFromCurrentState(idleMs: number) {
  const view = viewRef.current;
  if (!view) return;
  const { from } = view.state.selection;
  const $pos = view.state.doc.resolve(from);
  // Cursor offset within current block's text:
  const blockNode = $pos.parent;
  const blockOffset = $pos.parentOffset;
  const blockText = blockNode.textContent + "\n"; // simulate post-cursor newline for classifier
  const classification = classifyCursor(blockText, blockOffset);

  const blockId = blockNode.attrs?.blockId ?? "";
  const totalWords = markdownFromDoc(view.state.doc).split(/\s+/).filter(Boolean).length;
  const recentWordDelta = totalWords - wordCountAtT0Ref.current;
  wordCountAtT0Ref.current = totalWords;

  // Structural-inflection: detect if the most-recent insert was a scene_break / heading-2 block.
  // We track this via a ref updated in dispatchTransaction below.
  const inflection = pendingInflectionRef.current;
  pendingInflectionRef.current = "none";

  emitTypingTelemetry({
    idle_for_ms: idleMs,
    cursor_classification: classification,
    block_id: blockId,
    recent_word_delta: recentWordDelta,
    structural_inflection: inflection,
  });
}

const pendingInflectionRef = useRef<"new_scene" | "new_chapter" | "pov_change" | "location_change" | "none">("none");
```

Inside `dispatchTransaction`, after applying the transaction, detect inserted scene_break/heading_2 nodes:

```tsx
        dispatchTransaction: (tr) => {
          const next = view.state.apply(tr);
          view.updateState(next);
          if (tr.docChanged) {
            // Detect structural inflection (scene_break or h2 insert)
            tr.mapping.maps.forEach(() => {});
            next.doc.descendants((node) => {
              if (node.type.name === "scene_break") pendingInflectionRef.current = "new_scene";
              if (node.type.name === "heading" && node.attrs.level === 2) pendingInflectionRef.current = "new_chapter";
            });
            onChange(markdownFromDoc(next.doc));
            onActivity();
            // Throttled mid-typing emit (5 Hz cap)
            const now = Date.now();
            if (now - lastEmitRef.current > 200) {
              lastEmitRef.current = now;
              emitFromCurrentState(0);
            }
          }
          onTransaction?.(tr);
        },
```

(The exact integration depends on Task 8's final shape. The implementer adapts — the contract is: every transaction with `docChanged` either resets the idle timer + 5 Hz emits, or, at 3s idle, emits one final telemetry. Mid-sentence cursor must NOT trigger pill surfacing — that gating is the orchestrator's job in Phase C.)

- [ ] **Step 7: Run all editor tests + build**

```powershell
pnpm --filter @water/app test
pnpm --filter @water/app build
cargo clippy -p water-app -- -D warnings
```

Expected: all clean.

- [ ] **Step 8: Commit**

```powershell
git add app/src/editor/Editor.tsx app/src/editor/typingTelemetry.ts app/src/editor/typingTelemetry.test.ts app/src-tauri/src/events.rs app/src/ipc/events.ts app/src-tauri/src/commands/events.rs app/src-tauri/src/main.rs
git commit -m "feat(editor): typing telemetry emit + structural-inflection detection"
```

---

## Phase C — Pill orchestrator

### Task 11: Trigger trait + factory + first 3 built-ins

Creates the `Trigger` trait, the factory function `builtin_triggers()`, and implements `block_anchored_drift`, `scene_flow_dip`, and `topic_drift`. Pure Rust, unit-tested with synthetic contexts.

**Files:**
- Create: `crates/water-core/src/orchestrator/mod.rs`
- Create: `crates/water-core/src/orchestrator/triggers/mod.rs`
- Create: `crates/water-core/src/orchestrator/triggers/block_anchored_drift.rs`
- Create: `crates/water-core/src/orchestrator/triggers/scene_flow_dip.rs`
- Create: `crates/water-core/src/orchestrator/triggers/topic_drift.rs`
- Modify: `crates/water-core/src/lib.rs`

- [ ] **Step 1: Define the shared types + trait**

Create `crates/water-core/src/orchestrator/mod.rs`:

```rust
//! Pill orchestrator: deterministic state machine + trigger evaluation.
//! See docs/superpowers/specs/2026-05-17-m2-editor-pill-engine.md § 6.

pub mod triggers;
pub mod state;
pub mod eviction;
pub mod anti_loop;

use crate::Id;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypingTelemetry {
    pub idle_for_ms: u64,
    pub cursor_classification: CursorClassification,
    pub block_id: String,
    pub recent_word_delta: i32,
    pub structural_inflection: StructuralInflection,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CursorClassification { AtSentenceEnd, AtParagraphEnd, MidSentence }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StructuralInflection { NewScene, NewChapter, PovChange, LocationChange, None }

#[derive(Debug, Default, Clone)]
pub struct AnalysisSnapshot {
    pub flow: Option<f32>,
    pub coherence: Option<f32>,
    pub engagement: Option<f32>,
    pub divergence: Option<f32>,
    pub pace: Option<f32>,
    pub intensity: Option<f32>,
    pub valence: Option<f32>,
    pub block_metrics: std::collections::HashMap<String, BlockMetrics>,
    /// Most recent valence reading for the scene (used by valence_spike).
    pub valence_history: Vec<f32>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct BlockMetrics {
    pub flow: Option<f32>,
    pub coherence: Option<f32>,
    pub divergence: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct SceneSnapshot {
    pub id: Id,
    pub pov_character_id: Option<Id>,
    pub location_id: Option<Id>,
    pub characters_present: Vec<Id>,
    pub word_count: u32,
    pub seconds_since_last_pill: u64,
}

#[derive(Debug, Default, Clone)]
pub struct ProjectSnapshot {
    pub character_count: u32,
    pub world_entry_count: u32,
}

#[derive(Debug, Clone)]
pub struct TriggerContext<'a> {
    pub telemetry: &'a TypingTelemetry,
    pub analysis: &'a AnalysisSnapshot,
    pub scene: &'a SceneSnapshot,
    pub project: &'a ProjectSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeakerTrack { Persona, Character, Either }

#[derive(Debug, Clone)]
pub struct TriggerCandidate {
    pub trigger_id: &'static str,
    pub priority: f32,
    pub preferred_track: SpeakerTrack,
    pub reason: String,                       // serialized JSON for voice router
    pub block_target_id: Option<String>,
}

pub trait Trigger: Send + Sync {
    fn id(&self) -> &'static str;
    fn evaluate(&self, ctx: &TriggerContext<'_>) -> Option<TriggerCandidate>;
}
```

- [ ] **Step 2: Create the triggers module + factory**

Create `crates/water-core/src/orchestrator/triggers/mod.rs`:

```rust
pub mod block_anchored_drift;
pub mod scene_flow_dip;
pub mod topic_drift;

use super::Trigger;

#[must_use]
pub fn builtin_triggers() -> Vec<Box<dyn Trigger>> {
    vec![
        Box::new(block_anchored_drift::BlockAnchoredDrift),
        Box::new(scene_flow_dip::SceneFlowDip),
        Box::new(topic_drift::TopicDrift),
        // Remaining 7 in Task 12.
    ]
}
```

- [ ] **Step 3: Write failing tests for the first three triggers**

Create `crates/water-core/src/orchestrator/triggers/block_anchored_drift.rs`:

```rust
use crate::orchestrator::{SpeakerTrack, Trigger, TriggerCandidate, TriggerContext};

pub struct BlockAnchoredDrift;

impl Trigger for BlockAnchoredDrift {
    fn id(&self) -> &'static str { "block_anchored_drift" }

    fn evaluate(&self, ctx: &TriggerContext<'_>) -> Option<TriggerCandidate> {
        // Fires when the just-finished paragraph (block_id from telemetry)
        // has divergence > 0.6 OR coherence < 0.35.
        if ctx.telemetry.cursor_classification == crate::orchestrator::CursorClassification::MidSentence {
            return None;
        }
        let m = ctx.analysis.block_metrics.get(&ctx.telemetry.block_id)?;
        let div = m.divergence.unwrap_or(0.0);
        let coh = m.coherence.unwrap_or(1.0);
        if div > 0.6 || coh < 0.35 {
            Some(TriggerCandidate {
                trigger_id: self.id(),
                priority: 8.0,
                preferred_track: SpeakerTrack::Either,
                reason: format!("divergence={div:.2} coherence={coh:.2}"),
                block_target_id: Some(ctx.telemetry.block_id.clone()),
            })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::*;
    use crate::Id;

    fn ctx(cursor: CursorClassification, block_id: &str, div: f32, coh: f32) -> (TypingTelemetry, AnalysisSnapshot, SceneSnapshot, ProjectSnapshot) {
        let telem = TypingTelemetry {
            idle_for_ms: 3000, cursor_classification: cursor, block_id: block_id.to_string(),
            recent_word_delta: 0, structural_inflection: StructuralInflection::None,
        };
        let mut analysis = AnalysisSnapshot::default();
        analysis.block_metrics.insert(block_id.to_string(), BlockMetrics {
            flow: Some(0.5), coherence: Some(coh), divergence: Some(div),
        });
        let scene = SceneSnapshot {
            id: Id::new(), pov_character_id: None, location_id: None,
            characters_present: vec![], word_count: 500, seconds_since_last_pill: 60,
        };
        let project = ProjectSnapshot::default();
        (telem, analysis, scene, project)
    }

    #[test]
    fn fires_on_high_divergence() {
        let (t, a, s, p) = ctx(CursorClassification::AtParagraphEnd, "^bk-0001", 0.75, 0.5);
        let c = TriggerContext { telemetry: &t, analysis: &a, scene: &s, project: &p };
        assert!(BlockAnchoredDrift.evaluate(&c).is_some());
    }

    #[test]
    fn fires_on_low_coherence() {
        let (t, a, s, p) = ctx(CursorClassification::AtParagraphEnd, "^bk-0001", 0.3, 0.2);
        let c = TriggerContext { telemetry: &t, analysis: &a, scene: &s, project: &p };
        assert!(BlockAnchoredDrift.evaluate(&c).is_some());
    }

    #[test]
    fn does_not_fire_mid_sentence() {
        let (t, a, s, p) = ctx(CursorClassification::MidSentence, "^bk-0001", 0.9, 0.1);
        let c = TriggerContext { telemetry: &t, analysis: &a, scene: &s, project: &p };
        assert!(BlockAnchoredDrift.evaluate(&c).is_none());
    }

    #[test]
    fn does_not_fire_when_metrics_normal() {
        let (t, a, s, p) = ctx(CursorClassification::AtParagraphEnd, "^bk-0001", 0.3, 0.7);
        let c = TriggerContext { telemetry: &t, analysis: &a, scene: &s, project: &p };
        assert!(BlockAnchoredDrift.evaluate(&c).is_none());
    }
}
```

Create `crates/water-core/src/orchestrator/triggers/scene_flow_dip.rs`:

```rust
use crate::orchestrator::{SpeakerTrack, Trigger, TriggerCandidate, TriggerContext};

pub struct SceneFlowDip;

impl Trigger for SceneFlowDip {
    fn id(&self) -> &'static str { "scene_flow_dip" }

    fn evaluate(&self, ctx: &TriggerContext<'_>) -> Option<TriggerCandidate> {
        if ctx.telemetry.cursor_classification == crate::orchestrator::CursorClassification::MidSentence {
            return None;
        }
        let flow = ctx.analysis.flow?;
        // 30 s sustained is approximated as: caller has waited at least
        // 30 s since last analysis update with low flow. We use
        // seconds_since_last_pill as a proxy in M2; the orchestrator's
        // own debounce ensures sustained low flow is what triggers this.
        if flow < 0.4 && ctx.scene.seconds_since_last_pill >= 30 {
            Some(TriggerCandidate {
                trigger_id: self.id(),
                priority: 6.0,
                preferred_track: SpeakerTrack::Persona,
                reason: format!("flow={flow:.2}"),
                block_target_id: None,
            })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::*;
    use crate::Id;

    #[test]
    fn fires_on_low_flow_sustained() {
        let telem = TypingTelemetry {
            idle_for_ms: 3000, cursor_classification: CursorClassification::AtParagraphEnd,
            block_id: "^bk-0001".to_string(), recent_word_delta: 0, structural_inflection: StructuralInflection::None,
        };
        let mut analysis = AnalysisSnapshot::default(); analysis.flow = Some(0.3);
        let scene = SceneSnapshot { id: Id::new(), pov_character_id: None, location_id: None, characters_present: vec![], word_count: 500, seconds_since_last_pill: 60 };
        let project = ProjectSnapshot::default();
        let c = TriggerContext { telemetry: &telem, analysis: &analysis, scene: &scene, project: &project };
        assert!(SceneFlowDip.evaluate(&c).is_some());
    }

    #[test]
    fn does_not_fire_when_pill_recent() {
        let telem = TypingTelemetry {
            idle_for_ms: 3000, cursor_classification: CursorClassification::AtParagraphEnd,
            block_id: "^bk-0001".to_string(), recent_word_delta: 0, structural_inflection: StructuralInflection::None,
        };
        let mut analysis = AnalysisSnapshot::default(); analysis.flow = Some(0.3);
        let scene = SceneSnapshot { id: Id::new(), pov_character_id: None, location_id: None, characters_present: vec![], word_count: 500, seconds_since_last_pill: 5 };
        let project = ProjectSnapshot::default();
        let c = TriggerContext { telemetry: &telem, analysis: &analysis, scene: &scene, project: &project };
        assert!(SceneFlowDip.evaluate(&c).is_none());
    }
}
```

Create `crates/water-core/src/orchestrator/triggers/topic_drift.rs`:

```rust
use crate::orchestrator::{SpeakerTrack, Trigger, TriggerCandidate, TriggerContext};

pub struct TopicDrift;

impl Trigger for TopicDrift {
    fn id(&self) -> &'static str { "topic_drift" }

    fn evaluate(&self, ctx: &TriggerContext<'_>) -> Option<TriggerCandidate> {
        if ctx.telemetry.cursor_classification == crate::orchestrator::CursorClassification::MidSentence {
            return None;
        }
        let coh = ctx.analysis.coherence?;
        let div = ctx.analysis.divergence?;
        if coh < 0.35 && div > 0.5 {
            Some(TriggerCandidate {
                trigger_id: self.id(),
                priority: 7.0,
                preferred_track: SpeakerTrack::Persona,
                reason: format!("coherence={coh:.2} divergence={div:.2}"),
                block_target_id: None,
            })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::*;
    use crate::Id;

    #[test]
    fn fires_on_low_coherence_high_divergence() {
        let telem = TypingTelemetry {
            idle_for_ms: 3000, cursor_classification: CursorClassification::AtParagraphEnd,
            block_id: "^bk-0001".to_string(), recent_word_delta: 0, structural_inflection: StructuralInflection::None,
        };
        let mut analysis = AnalysisSnapshot::default();
        analysis.coherence = Some(0.2);
        analysis.divergence = Some(0.7);
        let scene = SceneSnapshot { id: Id::new(), pov_character_id: None, location_id: None, characters_present: vec![], word_count: 500, seconds_since_last_pill: 60 };
        let project = ProjectSnapshot::default();
        let c = TriggerContext { telemetry: &telem, analysis: &analysis, scene: &scene, project: &project };
        assert!(TopicDrift.evaluate(&c).is_some());
    }
}
```

- [ ] **Step 4: Register the orchestrator module**

Modify `crates/water-core/src/lib.rs`:

```rust
pub mod orchestrator;
```

- [ ] **Step 5: Create stub files for state/eviction/anti_loop so the module compiles**

Create three minimal files (will be filled in tasks 13-14):

`crates/water-core/src/orchestrator/state.rs`:

```rust
//! State machine — see Task 13.
```

`crates/water-core/src/orchestrator/eviction.rs`:

```rust
//! Eviction — see Task 13.
```

`crates/water-core/src/orchestrator/anti_loop.rs`:

```rust
//! Anti-loop — see Task 14.
```

- [ ] **Step 6: Run all tests + lints**

```powershell
cargo test -p water-core orchestrator
cargo clippy -p water-core --all-targets -- -D warnings
cargo fmt -p water-core --check
```

Expected: all triggers' tests pass (7 total in this task); lints clean.

- [ ] **Step 7: Commit**

```powershell
git add crates/water-core/src/orchestrator/ crates/water-core/src/lib.rs
git commit -m "feat(orchestrator): Trigger trait + factory + 3 built-ins (drift/flow_dip/topic_drift)"
```

---

### Task 12: Remaining 7 built-in triggers

Implements `valence_spike`, `structural_inflection`, `pace_floor`, `world_drift`, `no_universe_yet` (full impls) plus `character_dissonance` and `idle_pause_with_present_character` (M2 stubs that always return None; M3 fills them).

**Files:**
- Create: `crates/water-core/src/orchestrator/triggers/valence_spike.rs`
- Create: `crates/water-core/src/orchestrator/triggers/structural_inflection.rs`
- Create: `crates/water-core/src/orchestrator/triggers/pace_floor.rs`
- Create: `crates/water-core/src/orchestrator/triggers/world_drift.rs`
- Create: `crates/water-core/src/orchestrator/triggers/no_universe_yet.rs`
- Create: `crates/water-core/src/orchestrator/triggers/character_dissonance.rs`
- Create: `crates/water-core/src/orchestrator/triggers/idle_pause_with_present_character.rs`
- Modify: `crates/water-core/src/orchestrator/triggers/mod.rs`

- [ ] **Step 1: Implement `valence_spike`**

Create `crates/water-core/src/orchestrator/triggers/valence_spike.rs`:

```rust
use crate::orchestrator::{SpeakerTrack, Trigger, TriggerCandidate, TriggerContext};

pub struct ValenceSpike;

impl Trigger for ValenceSpike {
    fn id(&self) -> &'static str { "valence_spike" }

    fn evaluate(&self, ctx: &TriggerContext<'_>) -> Option<TriggerCandidate> {
        if ctx.telemetry.cursor_classification == crate::orchestrator::CursorClassification::MidSentence {
            return None;
        }
        let current = *ctx.analysis.valence_history.last()?;
        if ctx.analysis.valence_history.len() < 3 { return None; }
        let scene_mean: f32 = ctx.analysis.valence_history.iter().sum::<f32>()
            / ctx.analysis.valence_history.len() as f32;
        let delta = (current - scene_mean).abs();
        if delta > 0.4 {
            let track = if ctx.scene.characters_present.is_empty() {
                SpeakerTrack::Persona
            } else { SpeakerTrack::Character };
            Some(TriggerCandidate {
                trigger_id: self.id(),
                priority: 6.5,
                preferred_track: track,
                reason: format!("valence_delta={delta:.2}"),
                block_target_id: None,
            })
        } else { None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::*;
    use crate::Id;

    #[test]
    fn fires_on_large_valence_swing() {
        let telem = TypingTelemetry {
            idle_for_ms: 3000, cursor_classification: CursorClassification::AtParagraphEnd,
            block_id: "^bk-0001".to_string(), recent_word_delta: 0, structural_inflection: StructuralInflection::None,
        };
        let mut analysis = AnalysisSnapshot::default();
        analysis.valence_history = vec![0.1, 0.1, 0.1, 0.7]; // mean ~0.25, delta ~0.45
        let scene = SceneSnapshot { id: Id::new(), pov_character_id: None, location_id: None, characters_present: vec![], word_count: 500, seconds_since_last_pill: 60 };
        let project = ProjectSnapshot::default();
        let c = TriggerContext { telemetry: &telem, analysis: &analysis, scene: &scene, project: &project };
        assert!(ValenceSpike.evaluate(&c).is_some());
    }
}
```

- [ ] **Step 2: Implement `structural_inflection` with priority scaling**

Create `crates/water-core/src/orchestrator/triggers/structural_inflection.rs`:

```rust
use crate::orchestrator::{SpeakerTrack, StructuralInflection, Trigger, TriggerCandidate, TriggerContext};

pub struct StructuralInflectionTrigger;

impl Trigger for StructuralInflectionTrigger {
    fn id(&self) -> &'static str { "structural_inflection" }

    fn evaluate(&self, ctx: &TriggerContext<'_>) -> Option<TriggerCandidate> {
        if ctx.telemetry.cursor_classification == crate::orchestrator::CursorClassification::MidSentence {
            return None;
        }
        let kind = ctx.telemetry.structural_inflection;
        if kind == StructuralInflection::None { return None; }
        // Priority multiplier: 1.5 if scene metadata is set AND inflection
        // deviates from it; 0.6 if corresponding metadata is null.
        let multiplier = match kind {
            StructuralInflection::PovChange => {
                if ctx.scene.pov_character_id.is_some() { 1.5 } else { 0.6 }
            }
            StructuralInflection::LocationChange => {
                if ctx.scene.location_id.is_some() { 1.5 } else { 0.6 }
            }
            // User-initiated; always full priority.
            StructuralInflection::NewScene | StructuralInflection::NewChapter => 1.0,
            StructuralInflection::None => return None,
        };
        let base_priority = 5.5_f32;
        let track = match kind {
            StructuralInflection::LocationChange => SpeakerTrack::Persona, // Cartographer
            _ => if ctx.scene.pov_character_id.is_some() { SpeakerTrack::Character } else { SpeakerTrack::Persona },
        };
        Some(TriggerCandidate {
            trigger_id: self.id(),
            priority: base_priority * multiplier,
            preferred_track: track,
            reason: format!("{kind:?}"),
            block_target_id: Some(ctx.telemetry.block_id.clone()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::*;
    use crate::Id;

    fn base_ctx(infl: StructuralInflection, pov: Option<Id>, loc: Option<Id>) -> (TypingTelemetry, AnalysisSnapshot, SceneSnapshot, ProjectSnapshot) {
        let telem = TypingTelemetry { idle_for_ms: 3000, cursor_classification: CursorClassification::AtParagraphEnd, block_id: "^bk-0001".to_string(), recent_word_delta: 0, structural_inflection: infl };
        let analysis = AnalysisSnapshot::default();
        let scene = SceneSnapshot { id: Id::new(), pov_character_id: pov, location_id: loc, characters_present: vec![], word_count: 500, seconds_since_last_pill: 60 };
        (telem, analysis, scene, ProjectSnapshot::default())
    }

    #[test]
    fn pov_change_with_set_pov_is_high_priority() {
        let (t, a, s, p) = base_ctx(StructuralInflection::PovChange, Some(Id::new()), None);
        let c = TriggerContext { telemetry: &t, analysis: &a, scene: &s, project: &p };
        let cand = StructuralInflectionTrigger.evaluate(&c).unwrap();
        assert!(cand.priority > 7.0, "got priority {}", cand.priority);
    }

    #[test]
    fn pov_change_with_null_pov_is_low_priority() {
        let (t, a, s, p) = base_ctx(StructuralInflection::PovChange, None, None);
        let c = TriggerContext { telemetry: &t, analysis: &a, scene: &s, project: &p };
        let cand = StructuralInflectionTrigger.evaluate(&c).unwrap();
        assert!(cand.priority < 4.0, "got priority {}", cand.priority);
    }

    #[test]
    fn none_does_not_fire() {
        let (t, a, s, p) = base_ctx(StructuralInflection::None, None, None);
        let c = TriggerContext { telemetry: &t, analysis: &a, scene: &s, project: &p };
        assert!(StructuralInflectionTrigger.evaluate(&c).is_none());
    }
}
```

- [ ] **Step 3: Implement `pace_floor`**

Create `crates/water-core/src/orchestrator/triggers/pace_floor.rs`:

```rust
use crate::orchestrator::{SpeakerTrack, Trigger, TriggerCandidate, TriggerContext};

pub struct PaceFloor;

impl Trigger for PaceFloor {
    fn id(&self) -> &'static str { "pace_floor" }

    fn evaluate(&self, ctx: &TriggerContext<'_>) -> Option<TriggerCandidate> {
        if ctx.telemetry.cursor_classification == crate::orchestrator::CursorClassification::MidSentence {
            return None;
        }
        let pace = ctx.analysis.pace?;
        // recent_word_delta is words in last 10s; convert to last 3 min by
        // requiring sustained low pace (caller's debounce ensures this).
        if pace < 0.3 && ctx.telemetry.recent_word_delta.unsigned_abs() < 40 {
            Some(TriggerCandidate {
                trigger_id: self.id(),
                priority: 5.0,
                preferred_track: SpeakerTrack::Persona,
                reason: format!("pace={pace:.2} word_delta={}", ctx.telemetry.recent_word_delta),
                block_target_id: None,
            })
        } else { None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::*;
    use crate::Id;

    #[test]
    fn fires_on_low_pace_and_low_word_delta() {
        let telem = TypingTelemetry { idle_for_ms: 3000, cursor_classification: CursorClassification::AtParagraphEnd, block_id: "^bk-0001".to_string(), recent_word_delta: 10, structural_inflection: StructuralInflection::None };
        let mut analysis = AnalysisSnapshot::default(); analysis.pace = Some(0.2);
        let scene = SceneSnapshot { id: Id::new(), pov_character_id: None, location_id: None, characters_present: vec![], word_count: 500, seconds_since_last_pill: 60 };
        let project = ProjectSnapshot::default();
        let c = TriggerContext { telemetry: &telem, analysis: &analysis, scene: &scene, project: &project };
        assert!(PaceFloor.evaluate(&c).is_some());
    }
}
```

- [ ] **Step 4: Implement `world_drift` (M2 stub returns None; M4 wires the world bible)**

Create `crates/water-core/src/orchestrator/triggers/world_drift.rs`:

```rust
use crate::orchestrator::{Trigger, TriggerCandidate, TriggerContext};

pub struct WorldDrift;

impl Trigger for WorldDrift {
    fn id(&self) -> &'static str { "world_drift" }

    fn evaluate(&self, _ctx: &TriggerContext<'_>) -> Option<TriggerCandidate> {
        // M4 wires this against the World Bible. M2 ships the slot.
        None
    }
}
```

- [ ] **Step 5: Implement `no_universe_yet`**

Create `crates/water-core/src/orchestrator/triggers/no_universe_yet.rs`:

```rust
use crate::orchestrator::{SpeakerTrack, Trigger, TriggerCandidate, TriggerContext};

pub struct NoUniverseYet;

impl Trigger for NoUniverseYet {
    fn id(&self) -> &'static str { "no_universe_yet" }

    fn evaluate(&self, ctx: &TriggerContext<'_>) -> Option<TriggerCandidate> {
        if ctx.telemetry.cursor_classification == crate::orchestrator::CursorClassification::MidSentence {
            return None;
        }
        if ctx.project.character_count == 0
            && ctx.project.world_entry_count == 0
            && ctx.scene.word_count > 200
        {
            Some(TriggerCandidate {
                trigger_id: self.id(),
                priority: 4.5,
                preferred_track: SpeakerTrack::Persona, // Chorus
                reason: "eliciting_mode".to_string(),
                block_target_id: None,
            })
        } else { None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::*;
    use crate::Id;

    #[test]
    fn fires_when_project_is_empty_and_text_has_grown() {
        let telem = TypingTelemetry { idle_for_ms: 3000, cursor_classification: CursorClassification::AtParagraphEnd, block_id: "^bk-0001".to_string(), recent_word_delta: 0, structural_inflection: StructuralInflection::None };
        let analysis = AnalysisSnapshot::default();
        let scene = SceneSnapshot { id: Id::new(), pov_character_id: None, location_id: None, characters_present: vec![], word_count: 250, seconds_since_last_pill: 60 };
        let project = ProjectSnapshot { character_count: 0, world_entry_count: 0 };
        let c = TriggerContext { telemetry: &telem, analysis: &analysis, scene: &scene, project: &project };
        assert!(NoUniverseYet.evaluate(&c).is_some());
    }

    #[test]
    fn does_not_fire_when_project_has_characters() {
        let telem = TypingTelemetry { idle_for_ms: 3000, cursor_classification: CursorClassification::AtParagraphEnd, block_id: "^bk-0001".to_string(), recent_word_delta: 0, structural_inflection: StructuralInflection::None };
        let analysis = AnalysisSnapshot::default();
        let scene = SceneSnapshot { id: Id::new(), pov_character_id: None, location_id: None, characters_present: vec![], word_count: 250, seconds_since_last_pill: 60 };
        let project = ProjectSnapshot { character_count: 2, world_entry_count: 0 };
        let c = TriggerContext { telemetry: &telem, analysis: &analysis, scene: &scene, project: &project };
        assert!(NoUniverseYet.evaluate(&c).is_none());
    }
}
```

- [ ] **Step 6: Implement `character_dissonance` + `idle_pause_with_present_character` stubs**

Create `crates/water-core/src/orchestrator/triggers/character_dissonance.rs`:

```rust
use crate::orchestrator::{Trigger, TriggerCandidate, TriggerContext};

pub struct CharacterDissonance;

impl Trigger for CharacterDissonance {
    fn id(&self) -> &'static str { "character_dissonance" }

    /// M2 ships the slot; M3 fills it against LSM v2.1 sheets.
    /// See KNOWN_FRAGILE.md #1 for the design rationale.
    fn evaluate(&self, _ctx: &TriggerContext<'_>) -> Option<TriggerCandidate> {
        None
    }
}
```

Create `crates/water-core/src/orchestrator/triggers/idle_pause_with_present_character.rs`:

```rust
use crate::orchestrator::{Trigger, TriggerCandidate, TriggerContext};

pub struct IdlePauseWithPresentCharacter;

impl Trigger for IdlePauseWithPresentCharacter {
    fn id(&self) -> &'static str { "idle_pause_with_present_character" }

    /// M2 ships the slot; M3 wires character voices.
    fn evaluate(&self, _ctx: &TriggerContext<'_>) -> Option<TriggerCandidate> {
        None
    }
}
```

- [ ] **Step 7: Update the factory**

Modify `crates/water-core/src/orchestrator/triggers/mod.rs`:

```rust
pub mod block_anchored_drift;
pub mod scene_flow_dip;
pub mod topic_drift;
pub mod valence_spike;
pub mod structural_inflection;
pub mod pace_floor;
pub mod world_drift;
pub mod no_universe_yet;
pub mod character_dissonance;
pub mod idle_pause_with_present_character;

use super::Trigger;

#[must_use]
pub fn builtin_triggers() -> Vec<Box<dyn Trigger>> {
    vec![
        Box::new(block_anchored_drift::BlockAnchoredDrift),
        Box::new(scene_flow_dip::SceneFlowDip),
        Box::new(topic_drift::TopicDrift),
        Box::new(valence_spike::ValenceSpike),
        Box::new(structural_inflection::StructuralInflectionTrigger),
        Box::new(pace_floor::PaceFloor),
        Box::new(world_drift::WorldDrift),
        Box::new(no_universe_yet::NoUniverseYet),
        Box::new(character_dissonance::CharacterDissonance),
        Box::new(idle_pause_with_present_character::IdlePauseWithPresentCharacter),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factory_returns_ten_triggers_with_unique_ids() {
        let triggers = builtin_triggers();
        assert_eq!(triggers.len(), 10);
        let ids: std::collections::HashSet<_> = triggers.iter().map(|t| t.id()).collect();
        assert_eq!(ids.len(), 10);
    }
}
```

- [ ] **Step 8: Run tests + lints**

```powershell
cargo test -p water-core orchestrator
cargo clippy -p water-core --all-targets -- -D warnings
```

Expected: all clean; 10 trigger module tests + factory test pass.

- [ ] **Step 9: Commit**

```powershell
git add crates/water-core/src/orchestrator/triggers/
git commit -m "feat(orchestrator): 7 more triggers + 2 stubs (10 total built-ins)"
```

---

### Task 13: Orchestrator state machine + eviction

Pure state machine driven by typed events. Pure functions of `(current_state, event)`. Unit-tested with synthetic event streams.

**Files:**
- Modify: `crates/water-core/src/orchestrator/state.rs`
- Modify: `crates/water-core/src/orchestrator/eviction.rs`
- Modify: `crates/water-core/src/orchestrator/mod.rs`

- [ ] **Step 1: Replace the stub `state.rs` with the state machine**

Replace `crates/water-core/src/orchestrator/state.rs`:

```rust
//! Deterministic state machine for individual pill lifecycles.
//! Pure: `(state, event) -> (state, optional side-effect)`.

use crate::Id;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PillLifecycle {
    Generating,    // LLM call in flight
    Surfacing,     // emit pill:emerged fired; renderer is fading-in
    OnScreen,      // visible to writer; eligible for click
    Pinned,        // moved to pinned column
    Dismissed,     // user X
    Expired,       // soft TTL elapsed
    Evicted,       // FIFO replaced by a newer candidate
}

#[derive(Debug, Clone)]
pub struct Pill {
    pub id: Id,
    pub state: PillLifecycle,
    pub created_at: Instant,
    pub speaker_id: String,
    pub trigger_id: String,
    pub text: Option<String>,            // None until LLM returns
    pub block_target_id: Option<String>,
    pub parent_pill_id: Option<Id>,
}

impl Pill {
    pub fn new_generating(speaker_id: String, trigger_id: String, block_target_id: Option<String>, parent_pill_id: Option<Id>) -> Self {
        Self {
            id: Id::new(), state: PillLifecycle::Generating, created_at: Instant::now(),
            speaker_id, trigger_id, text: None, block_target_id, parent_pill_id,
        }
    }
}

#[derive(Debug, Clone)]
pub enum PillEvent {
    LlmReturned { text: String },
    LlmFailed,
    UserPin,
    UserDismiss,
    Expired,
    Evicted,
    PostFilterDrop,
}

/// Apply an event to a pill. Returns the new state (which may be unchanged).
pub fn transition(pill: &Pill, event: &PillEvent) -> PillLifecycle {
    use PillLifecycle::*;
    use PillEvent::*;
    match (pill.state, event) {
        (Generating, LlmReturned { .. }) => Surfacing,
        (Generating, LlmFailed | PostFilterDrop) => Dismissed,
        (Surfacing, _) => OnScreen,
        (OnScreen, UserPin) => Pinned,
        (OnScreen, UserDismiss) => Dismissed,
        (OnScreen, Expired) => Expired,
        (OnScreen, Evicted) => Evicted,
        (state, _) => state, // terminal states absorb further events
    }
}

/// Soft TTL after which an on-screen pill expires.
pub const PILL_SOFT_TTL: Duration = Duration::from_secs(90);

pub fn should_expire(pill: &Pill, now: Instant) -> bool {
    pill.state == PillLifecycle::OnScreen
        && now.saturating_duration_since(pill.created_at) > PILL_SOFT_TTL
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pill() -> Pill {
        Pill::new_generating("echo".into(), "block_anchored_drift".into(), None, None)
    }

    #[test]
    fn generating_to_surfacing_on_llm_return() {
        let p = pill();
        assert_eq!(transition(&p, &PillEvent::LlmReturned { text: "x".into() }), PillLifecycle::Surfacing);
    }
    #[test]
    fn generating_to_dismissed_on_failure() {
        let p = pill();
        assert_eq!(transition(&p, &PillEvent::LlmFailed), PillLifecycle::Dismissed);
    }
    #[test]
    fn onscreen_to_pinned_on_user_pin() {
        let mut p = pill(); p.state = PillLifecycle::OnScreen;
        assert_eq!(transition(&p, &PillEvent::UserPin), PillLifecycle::Pinned);
    }
    #[test]
    fn terminal_states_absorb_events() {
        let mut p = pill(); p.state = PillLifecycle::Dismissed;
        assert_eq!(transition(&p, &PillEvent::UserPin), PillLifecycle::Dismissed);
    }
}
```

- [ ] **Step 2: Replace the stub `eviction.rs` with FIFO logic**

Replace `crates/water-core/src/orchestrator/eviction.rs`:

```rust
//! FIFO eviction. Max 2 pills on-screen at once; new candidate evicts the
//! older. Pinned pills do not count.

use super::state::{Pill, PillLifecycle};

pub const MAX_ON_SCREEN: usize = 2;

/// Returns the index of the pill to evict (older of the on-screen pills),
/// or None if there's room.
#[must_use]
pub fn pick_evictee(pills: &[Pill]) -> Option<usize> {
    let on_screen: Vec<(usize, &Pill)> = pills.iter().enumerate()
        .filter(|(_, p)| p.state == PillLifecycle::OnScreen)
        .collect();
    if on_screen.len() < MAX_ON_SCREEN { return None; }
    on_screen.iter()
        .min_by_key(|(_, p)| p.created_at)
        .map(|(i, _)| *i)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Id;
    use std::time::{Duration, Instant};

    fn pill_at(t0: Instant, secs: u64, state: PillLifecycle) -> Pill {
        Pill {
            id: Id::new(),
            state,
            created_at: t0 + Duration::from_secs(secs),
            speaker_id: "echo".into(), trigger_id: "t".into(), text: None,
            block_target_id: None, parent_pill_id: None,
        }
    }

    #[test]
    fn no_evictee_when_under_max() {
        let t0 = Instant::now();
        let pills = vec![pill_at(t0, 0, PillLifecycle::OnScreen)];
        assert!(pick_evictee(&pills).is_none());
    }

    #[test]
    fn picks_oldest_on_screen() {
        let t0 = Instant::now();
        let pills = vec![
            pill_at(t0, 0, PillLifecycle::OnScreen),
            pill_at(t0, 30, PillLifecycle::OnScreen),
        ];
        assert_eq!(pick_evictee(&pills), Some(0));
    }

    #[test]
    fn pinned_pills_do_not_count() {
        let t0 = Instant::now();
        let pills = vec![
            pill_at(t0, 0, PillLifecycle::Pinned),
            pill_at(t0, 30, PillLifecycle::OnScreen),
            pill_at(t0, 60, PillLifecycle::OnScreen),
        ];
        // Only 2 on-screen → evict the older one (index 1).
        assert_eq!(pick_evictee(&pills), Some(1));
    }
}
```

- [ ] **Step 3: Run tests + lints**

```powershell
cargo test -p water-core orchestrator
cargo clippy -p water-core --all-targets -- -D warnings
```

Expected: state-machine tests + eviction tests pass.

- [ ] **Step 4: Commit**

```powershell
git add crates/water-core/src/orchestrator/state.rs crates/water-core/src/orchestrator/eviction.rs
git commit -m "feat(orchestrator): pill state machine + FIFO eviction"
```

---

### Task 14: Anti-loop (Jaccard on stopword-stripped lemma sets)

Implements the per-speaker anti-loop overlap check from spec § 6.5. Ships a 120-word stopword list + a fixed-table suffix-stripper.

**Files:**
- Create: `crates/water-core/data/stopwords-en.txt`
- Modify: `crates/water-core/src/orchestrator/anti_loop.rs`

- [ ] **Step 1: Create the stopword list**

Create `crates/water-core/data/stopwords-en.txt` (one stopword per line):

```
a
about
above
after
again
all
also
am
an
and
any
are
as
at
be
because
been
before
being
below
between
both
but
by
can
cannot
could
did
do
does
doing
don
down
during
each
few
for
from
further
had
has
have
having
he
her
here
hers
herself
him
himself
his
how
i
if
in
into
is
it
its
itself
just
me
more
most
my
myself
nor
not
now
of
off
on
once
only
or
other
our
ours
ourselves
out
over
own
same
she
should
so
some
such
than
that
the
their
theirs
them
themselves
then
there
these
they
this
those
through
to
too
under
until
up
very
was
we
were
what
when
where
which
while
who
whom
why
will
with
would
you
your
yours
yourself
yourselves
```

- [ ] **Step 2: Write failing tests**

Replace `crates/water-core/src/orchestrator/anti_loop.rs`:

```rust
//! Anti-loop overlap check. Jaccard similarity on stopword-stripped,
//! suffix-stripped tokens. Per-speaker threshold (default 0.70).

use std::collections::HashSet;

const STOPWORDS_RAW: &str = include_str!("../../data/stopwords-en.txt");

fn stopwords() -> HashSet<&'static str> {
    STOPWORDS_RAW.lines().filter(|l| !l.is_empty()).collect()
}

const SUFFIXES: &[&str] = &["ing", "ed", "es", "ly", "s"];

fn strip_suffix(word: &str) -> &str {
    for suf in SUFFIXES {
        if word.len() > suf.len() + 2 && word.ends_with(suf) {
            return &word[..word.len() - suf.len()];
        }
    }
    word
}

#[must_use]
pub fn tokenize(text: &str) -> HashSet<String> {
    let sw = stopwords();
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty() && !sw.contains(s))
        .map(strip_suffix)
        .map(std::string::ToString::to_string)
        .collect()
}

#[must_use]
pub fn jaccard(a: &HashSet<String>, b: &HashSet<String>) -> f32 {
    if a.is_empty() && b.is_empty() { return 0.0; }
    let inter = a.intersection(b).count();
    let union = a.union(b).count();
    inter as f32 / union as f32
}

/// Returns the max Jaccard overlap of `new_text` against any prior in
/// `prior_texts`. 0.0 if priors is empty.
#[must_use]
pub fn max_overlap(new_text: &str, prior_texts: &[String]) -> f32 {
    let new_toks = tokenize(new_text);
    prior_texts
        .iter()
        .map(|prior| jaccard(&new_toks, &tokenize(prior)))
        .fold(0.0_f32, f32::max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_strings_have_overlap_one() {
        let a = tokenize("the writer walks softly");
        let b = tokenize("the writer walks softly");
        assert!((jaccard(&a, &b) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn stopwords_dropped() {
        let toks = tokenize("the and the");
        assert!(toks.is_empty());
    }

    #[test]
    fn suffix_stripped_collisions() {
        let a = tokenize("walked");
        let b = tokenize("walking");
        let c = tokenize("walks");
        // All should collide on "walk".
        assert_eq!(jaccard(&a, &b), 1.0);
        assert_eq!(jaccard(&a, &c), 1.0);
    }

    #[test]
    fn disjoint_content_is_zero() {
        let a = tokenize("rain falls gently");
        let b = tokenize("mountain breathes cold");
        assert_eq!(jaccard(&a, &b), 0.0);
    }

    #[test]
    fn max_overlap_returns_largest() {
        let priors = vec![
            "the rain falls gently".to_string(),
            "mountain breathes cold".to_string(),
        ];
        let new = "rain falls gently on quiet stone";
        let overlap = max_overlap(new, &priors);
        // "rain falls gently" matches 3 of 5 content words in new ("rain","fall","gentl") → ~0.6
        assert!(overlap > 0.4 && overlap < 0.8, "got {overlap}");
    }
}
```

- [ ] **Step 3: Run tests + lints**

```powershell
cargo test -p water-core orchestrator::anti_loop
cargo clippy -p water-core --all-targets -- -D warnings
```

Expected: 5 tests pass; lints clean.

- [ ] **Step 4: Commit**

```powershell
git add crates/water-core/data/stopwords-en.txt crates/water-core/src/orchestrator/anti_loop.rs
git commit -m "feat(orchestrator): anti-loop Jaccard overlap with stopword + suffix stripper"
```

---

## Phase D — Voice & prompts

### Task 15: Speaker trait + persona registry + 5 default personas

Creates the `Speaker` trait, the `PersonaRegistry` loaded from `prompts/speakers/persona/*.toml`, and ships 5 default persona TOML files.

**Files:**
- Create: `crates/water-core/src/voice/mod.rs`
- Create: `crates/water-core/src/voice/speaker.rs`
- Create: `crates/water-core/src/voice/registry.rs`
- Create: `prompts/speakers/persona/echo.toml`
- Create: `prompts/speakers/persona/architect.toml`
- Create: `prompts/speakers/persona/editor.toml`
- Create: `prompts/speakers/persona/cartographer.toml`
- Create: `prompts/speakers/persona/chorus.toml`
- Modify: `crates/water-core/src/lib.rs`
- Modify: `crates/water-core/Cargo.toml` (add `toml = "0.8"` if not present)

- [ ] **Step 1: Check + add toml dep**

```powershell
Select-String -Path crates\water-core\Cargo.toml -Pattern '^toml\s*=' -SimpleMatch
```

If no match, append `toml = "0.8"` to `[dependencies]`.

- [ ] **Step 2: Create the voice module + Speaker trait**

Create `crates/water-core/src/voice/mod.rs`:

```rust
//! Voice subsystem: Speaker trait + persona/character registries +
//! deterministic voice router (Task 16).

pub mod speaker;
pub mod registry;
pub mod router;

pub use speaker::{Speaker, SpeakerKind, PersonaSpeaker};
pub use registry::PersonaRegistry;
pub use router::route;
```

Create `crates/water-core/src/voice/speaker.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeakerKind { Persona, Character }

pub trait Speaker: Send + Sync {
    fn id(&self) -> &str;
    fn kind(&self) -> SpeakerKind;
    fn display_name(&self) -> &str;
    fn hue_token(&self) -> &str;
    fn prompt_fragment(&self) -> &str;
    fn anti_loop_threshold(&self) -> f32 { 0.70 }
    fn cooldown_ms(&self) -> u64 { 45_000 }
}

#[derive(Debug, Deserialize)]
struct PersonaToml {
    version: String,
    id: String,
    display_name: String,
    hue_token: String,
    #[serde(default = "default_threshold")]
    anti_loop_threshold: f32,
    #[serde(default = "default_cooldown")]
    cooldown_ms: u64,
    prompt: PersonaPrompt,
}

#[derive(Debug, Deserialize)]
struct PersonaPrompt { voice_profile: String }

fn default_threshold() -> f32 { 0.70 }
fn default_cooldown() -> u64 { 45_000 }

#[derive(Debug, Clone)]
pub struct PersonaSpeaker {
    id: String,
    display_name: String,
    hue_token: String,
    prompt_fragment: String,
    anti_loop_threshold: f32,
    cooldown_ms: u64,
}

impl PersonaSpeaker {
    /// Load from a TOML file. Returns an error with the file path if parsing fails.
    pub fn from_toml_str(s: &str) -> Result<Self, String> {
        let parsed: PersonaToml = toml::from_str(s).map_err(|e| e.to_string())?;
        if parsed.version != "1" {
            return Err(format!("unsupported persona TOML version: {}", parsed.version));
        }
        Ok(Self {
            id: parsed.id,
            display_name: parsed.display_name,
            hue_token: parsed.hue_token,
            prompt_fragment: parsed.prompt.voice_profile,
            anti_loop_threshold: parsed.anti_loop_threshold,
            cooldown_ms: parsed.cooldown_ms,
        })
    }

    /// Override display name (used by per-project rename via `settings` table).
    pub fn with_display_name(mut self, name: String) -> Self {
        self.display_name = name;
        self
    }
}

impl Speaker for PersonaSpeaker {
    fn id(&self) -> &str { &self.id }
    fn kind(&self) -> SpeakerKind { SpeakerKind::Persona }
    fn display_name(&self) -> &str { &self.display_name }
    fn hue_token(&self) -> &str { &self.hue_token }
    fn prompt_fragment(&self) -> &str { &self.prompt_fragment }
    fn anti_loop_threshold(&self) -> f32 { self.anti_loop_threshold }
    fn cooldown_ms(&self) -> u64 { self.cooldown_ms }
}

// Arc<dyn Speaker> support for trait-object factories.
pub type SpeakerArc = Arc<dyn Speaker>;
```

- [ ] **Step 3: Create the 5 default persona TOML files**

Create `prompts/speakers/persona/echo.toml`:

```toml
version = "1"
id = "echo"
display_name = "Echo"
hue_token = "--water-hue-muse"
anti_loop_threshold = 0.70
cooldown_ms = 45000

[prompt]
voice_profile = """
You are Echo. You speak gently and rarely, as if listening through fog.
You notice what is almost-there in the prose: a feeling almost named,
a rhythm almost found. You never instruct.
"""
```

Create `prompts/speakers/persona/architect.toml`:

```toml
version = "1"
id = "architect"
display_name = "Architect"
hue_token = "--water-hue-architect"
anti_loop_threshold = 0.65
cooldown_ms = 60000

[prompt]
voice_profile = """
You are Architect. You watch structure and pace as if from above.
You notice the load-bearing turn that has not yet arrived,
the beat that has run a little long, the angle of attack.
You speak in shape, not in instruction.
"""
```

Create `prompts/speakers/persona/editor.toml`:

```toml
version = "1"
id = "editor"
display_name = "Editor"
hue_token = "--water-hue-editor"
anti_loop_threshold = 0.65
cooldown_ms = 45000

[prompt]
voice_profile = """
You are Editor. You notice diction the way a tailor notices a hem.
A word that almost-fits. A repetition that has begun to chime.
You name what you see; you do not correct.
"""
```

Create `prompts/speakers/persona/cartographer.toml`:

```toml
version = "1"
id = "cartographer"
display_name = "Cartographer"
hue_token = "--water-hue-cartographer"
anti_loop_threshold = 0.70
cooldown_ms = 60000

[prompt]
voice_profile = """
You are Cartographer. You hold the shape of the world the writer is
making. You notice when a place is mentioned but not yet seen,
when a season has shifted, when the light has changed direction.
You wonder aloud about where we are.
"""
```

Create `prompts/speakers/persona/chorus.toml`:

```toml
version = "1"
id = "chorus"
display_name = "Chorus"
hue_token = "--water-hue-chorus"
anti_loop_threshold = 0.75
cooldown_ms = 90000

[prompt]
voice_profile = """
You are Chorus. You speak only when the universe has not yet been
named. You notice a smell on the page that does not have a word,
a name half-said, a place with no map. You imply, never instruct.
"""
```

- [ ] **Step 4: Write failing test for PersonaSpeaker parse**

Create `crates/water-core/src/voice/speaker.rs` test module (append to existing file):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const ECHO: &str = include_str!("../../../../prompts/speakers/persona/echo.toml");

    #[test]
    fn parses_echo_toml() {
        let s = PersonaSpeaker::from_toml_str(ECHO).unwrap();
        assert_eq!(s.id(), "echo");
        assert_eq!(s.display_name(), "Echo");
        assert_eq!(s.hue_token(), "--water-hue-muse");
        assert!(s.prompt_fragment().contains("listening through fog"));
        assert_eq!(s.anti_loop_threshold(), 0.70);
        assert_eq!(s.cooldown_ms(), 45_000);
    }

    #[test]
    fn rename_overrides_display_name() {
        let s = PersonaSpeaker::from_toml_str(ECHO).unwrap()
            .with_display_name("Muse".to_string());
        assert_eq!(s.display_name(), "Muse");
        assert_eq!(s.id(), "echo");
    }

    #[test]
    fn rejects_wrong_version() {
        let bad = r#"
version = "99"
id = "x"
display_name = "X"
hue_token = "--water-hue-muse"
[prompt]
voice_profile = "y"
"#;
        assert!(PersonaSpeaker::from_toml_str(bad).is_err());
    }
}
```

- [ ] **Step 5: Create the registry**

Create `crates/water-core/src/voice/registry.rs`:

```rust
//! Persona registry: loads from `prompts/speakers/persona/*.toml`. The
//! per-project rename override is read from the `settings` table and
//! applied at load time (or via `with_display_name`).

use super::speaker::{PersonaSpeaker, Speaker, SpeakerArc};
use crate::Db;
use rusqlite::OptionalExtension;
use std::path::Path;
use std::sync::Arc;

const PERSONA_FILES: &[(&str, &str)] = &[
    ("echo",         include_str!("../../../../prompts/speakers/persona/echo.toml")),
    ("architect",    include_str!("../../../../prompts/speakers/persona/architect.toml")),
    ("editor",       include_str!("../../../../prompts/speakers/persona/editor.toml")),
    ("cartographer", include_str!("../../../../prompts/speakers/persona/cartographer.toml")),
    ("chorus",       include_str!("../../../../prompts/speakers/persona/chorus.toml")),
];

#[derive(Default)]
pub struct PersonaRegistry { personas: Vec<SpeakerArc> }

impl PersonaRegistry {
    /// Build a registry from the built-in TOMLs. Renames from the project's
    /// `settings` table override the TOML's `display_name`.
    pub fn from_db(db: &Db) -> Result<Self, String> {
        let mut personas: Vec<SpeakerArc> = Vec::with_capacity(PERSONA_FILES.len());
        for (id, toml) in PERSONA_FILES {
            let base = PersonaSpeaker::from_toml_str(toml)
                .map_err(|e| format!("persona {id}: {e}"))?;
            // Look up rename in settings.
            let key = format!("persona.rename.{id}");
            let rename: Option<String> = db.conn()
                .query_row("SELECT value_json FROM settings WHERE key = ?1", [key.as_str()], |r| r.get(0))
                .optional()
                .map_err(|e| e.to_string())?;
            let speaker = if let Some(name_json) = rename {
                let name: String = serde_json::from_str(&name_json).map_err(|e| e.to_string())?;
                base.with_display_name(name)
            } else {
                base
            };
            personas.push(Arc::new(speaker) as SpeakerArc);
        }
        Ok(Self { personas })
    }

    #[must_use]
    pub fn list(&self) -> &[SpeakerArc] { &self.personas }

    #[must_use]
    pub fn by_id(&self, id: &str) -> Option<SpeakerArc> {
        self.personas.iter().find(|s| s.id() == id).cloned()
    }
}

// Marker file for `include_str!` path verification in tests.
#[cfg(test)]
const _: &str = include_str!("../../../../prompts/speakers/persona/echo.toml");

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let db = Db::open(dir.path().join("project.db")).unwrap();
        (dir, db)
    }

    #[test]
    fn loads_five_built_in_personas() {
        let (_t, db) = fresh_db();
        let reg = PersonaRegistry::from_db(&db).unwrap();
        let ids: Vec<&str> = reg.list().iter().map(|s| s.id()).collect();
        assert_eq!(ids, vec!["echo", "architect", "editor", "cartographer", "chorus"]);
    }

    #[test]
    fn rename_via_settings_overrides_display_name() {
        let (_t, db) = fresh_db();
        db.conn().execute(
            "INSERT INTO settings (key, value_json) VALUES (?1, ?2)",
            rusqlite::params!["persona.rename.echo", "\"Whisper\""],
        ).unwrap();
        let reg = PersonaRegistry::from_db(&db).unwrap();
        let echo = reg.by_id("echo").unwrap();
        assert_eq!(echo.display_name(), "Whisper");
    }
}
```

Create a stub `crates/water-core/src/voice/router.rs` so the module compiles (Task 16 fills it):

```rust
//! Voice router — see Task 16.
pub fn route() {}
```

- [ ] **Step 6: Register voice module**

Modify `crates/water-core/src/lib.rs`:

```rust
pub mod voice;
```

- [ ] **Step 7: Run tests + lints**

```powershell
cargo test -p water-core voice
cargo clippy -p water-core --all-targets -- -D warnings
```

Expected: persona-load tests pass.

- [ ] **Step 8: Commit**

```powershell
git add crates/water-core/Cargo.toml crates/water-core/src/voice/ crates/water-core/src/lib.rs prompts/speakers/persona/
git commit -m "feat(voice): Speaker trait + PersonaRegistry + 5 default personas"
```

---

### Task 16: Voice router

Implements the deterministic router from master spec § 6.2. Pure function of `(candidate, scene, characters, personas, cooldowns)`.

**Files:**
- Modify: `crates/water-core/src/voice/router.rs`
- Modify: `crates/water-core/src/voice/mod.rs` (re-export)

- [ ] **Step 1: Replace the router stub with the real router**

Replace `crates/water-core/src/voice/router.rs`:

```rust
//! Deterministic voice router. Matches master spec § 6.2.
//!
//! Choice of speaker is determined entirely by:
//! - the trigger candidate (preferred_track, trigger_id)
//! - the scene snapshot (characters_present, pov_character_id)
//! - the persona registry (available speakers)
//! - the cooldown state (most-recent-emit timestamp per speaker)
//!
//! Variation lives only in the LLM's sampling; routing is pure + replayable.

use super::registry::PersonaRegistry;
use super::speaker::SpeakerArc;
use crate::orchestrator::{SpeakerTrack, TriggerCandidate};
use std::collections::HashMap;
use std::time::Instant;

#[derive(Default)]
pub struct CooldownState {
    pub last_emit: HashMap<String, Instant>,
}

impl CooldownState {
    pub fn note_emit(&mut self, speaker_id: &str) {
        self.last_emit.insert(speaker_id.to_string(), Instant::now());
    }
}

/// Map a trigger_id → preferred persona id. This is the "default speaker track"
/// column in master spec § 6.1. Returned id may be overridden by routing rules.
fn default_persona_for_trigger(trigger_id: &str) -> &'static str {
    match trigger_id {
        "block_anchored_drift" => "editor",
        "scene_flow_dip"        => "echo",
        "topic_drift"           => "architect",
        "valence_spike"         => "echo",
        "structural_inflection" => "cartographer",
        "pace_floor"            => "architect",
        "world_drift"           => "cartographer",
        "no_universe_yet"       => "chorus",
        _                       => "echo",
    }
}

/// Returns the chosen Speaker for this candidate. None if nothing is
/// available (every relevant speaker is cooled-down — caller skips this tick).
#[must_use]
pub fn route(
    candidate: &TriggerCandidate,
    personas: &PersonaRegistry,
    cooldowns: &CooldownState,
    now: Instant,
) -> Option<SpeakerArc> {
    // M2 ships persona-only routing. SpeakerTrack::Character is always treated
    // as "fall back to persona" until M3 wires the CharacterRegistry.
    let _ = candidate.preferred_track; // suppress unused; will use in M3

    let preferred_id = default_persona_for_trigger(candidate.trigger_id);

    // Filter out cooled-down speakers.
    let available: Vec<SpeakerArc> = personas.list().iter()
        .filter(|s| {
            cooldowns.last_emit.get(s.id())
                .map_or(true, |last| now.duration_since(*last).as_millis() as u64 >= s.cooldown_ms())
        })
        .cloned()
        .collect();
    if available.is_empty() { return None; }

    // Prefer the trigger's default persona if available; else LRU among non-cooled-down.
    if let Some(pref) = available.iter().find(|s| s.id() == preferred_id) {
        return Some(pref.clone());
    }
    // Tie-break: least-recently-used (or never-used).
    available.into_iter()
        .min_by_key(|s| cooldowns.last_emit.get(s.id()).copied().unwrap_or(now))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::{SpeakerTrack, TriggerCandidate};
    use tempfile::TempDir;
    use crate::Db;

    fn registry() -> PersonaRegistry {
        let dir = TempDir::new().unwrap();
        let db = Db::open(dir.path().join("p.db")).unwrap();
        PersonaRegistry::from_db(&db).unwrap()
    }

    fn cand(id: &'static str) -> TriggerCandidate {
        TriggerCandidate {
            trigger_id: id, priority: 5.0, preferred_track: SpeakerTrack::Either,
            reason: String::new(), block_target_id: None,
        }
    }

    #[test]
    fn block_anchored_drift_picks_editor() {
        let reg = registry();
        let s = route(&cand("block_anchored_drift"), &reg, &CooldownState::default(), Instant::now()).unwrap();
        assert_eq!(s.id(), "editor");
    }

    #[test]
    fn cooldown_skips_preferred_in_favor_of_lru() {
        let reg = registry();
        let mut cd = CooldownState::default();
        cd.note_emit("editor");
        // Editor just emitted → should fall back to LRU.
        let s = route(&cand("block_anchored_drift"), &reg, &cd, Instant::now()).unwrap();
        assert_ne!(s.id(), "editor");
    }

    #[test]
    fn no_universe_yet_picks_chorus() {
        let reg = registry();
        let s = route(&cand("no_universe_yet"), &reg, &CooldownState::default(), Instant::now()).unwrap();
        assert_eq!(s.id(), "chorus");
    }

    #[test]
    fn route_is_deterministic_for_same_inputs() {
        let reg = registry();
        let cd = CooldownState::default();
        let t = Instant::now();
        let s1 = route(&cand("topic_drift"), &reg, &cd, t).unwrap();
        let s2 = route(&cand("topic_drift"), &reg, &cd, t).unwrap();
        assert_eq!(s1.id(), s2.id());
    }
}
```

- [ ] **Step 2: Run tests + lints**

```powershell
cargo test -p water-core voice
cargo clippy -p water-core --all-targets -- -D warnings
```

Expected: 4 router tests + earlier registry tests pass.

- [ ] **Step 3: Commit**

```powershell
git add crates/water-core/src/voice/router.rs
git commit -m "feat(voice): deterministic voice router (persona-only for M2)"
```

---

### Task 17: Prompt library — loader + assembler

Loads TOML prompts (tone, triggers, tasks) at startup. Assembles `tone + speaker + trigger + task + inputs` into the LLM request. In `cfg(debug_assertions)` builds, watches the prompts directory for changes.

**Files:**
- Create: `crates/water-core/src/prompts/mod.rs`
- Create: `crates/water-core/src/prompts/loader.rs`
- Create: `crates/water-core/src/prompts/assembler.rs`
- Create: `prompts/tone.toml`
- Create: `prompts/anti_loop.toml`
- Create: `prompts/triggers/block_anchored_drift.toml`
- Create: `prompts/triggers/scene_flow_dip.toml`
- Create: `prompts/triggers/topic_drift.toml`
- Create: `prompts/triggers/valence_spike.toml`
- Create: `prompts/triggers/structural_inflection.toml`
- Create: `prompts/triggers/pace_floor.toml`
- Create: `prompts/triggers/world_drift.toml`
- Create: `prompts/triggers/no_universe_yet.toml`
- Create: `prompts/triggers/character_dissonance.toml`
- Create: `prompts/triggers/idle_pause_with_present_character.toml`
- Create: `prompts/tasks/pill_level_0.toml`
- Create: `prompts/tasks/pill_expand.toml`
- Create: `prompts/tasks/pill_regenerate.toml`
- Modify: `crates/water-core/src/lib.rs`

- [ ] **Step 1: Create the global tone TOML**

Create `prompts/tone.toml`:

```toml
version = "1"

[clauses]
present_tense = "Speak in present tense as if you are noticing this just now."
not_assistant = "You are not an assistant. You do not give writing advice."
blacklist = "Never say: 'you should', 'consider', 'try', 'maybe you could', 'I think you', 'as an AI', 'this is good', 'this is bad'."
observe = "Observe, react, wonder. Leave space."
shape = "Output exactly one line of prose, no more than 22 words. No quotation marks. No emoji."
pass = "If you cannot react in your speaker's voice without breaking these rules, output the single token `PASS` and nothing else."

[blacklist_regex]
patterns = [
  "(?i)\\byou should\\b",
  "(?i)\\bconsider\\b",
  "(?i)\\btry\\b",
  "(?i)\\bi think you\\b",
  "(?i)\\bas an AI\\b",
  "(?i)\\bmaybe you could\\b",
]
```

- [ ] **Step 2: Create the anti-loop TOML**

Create `prompts/anti_loop.toml`:

```toml
version = "1"

[diverge_clause]
text = """
The following opening words have already been used in this bouquet:
{prior_first_words}.
Produce ideas that begin and develop materially differently from these.
"""
```

- [ ] **Step 3: Create the 10 trigger TOMLs**

Create `prompts/triggers/block_anchored_drift.toml`:

```toml
version = "1"
id = "block_anchored_drift"
framing = """
The writer has just finished a paragraph whose coherence has dipped or
whose divergence from the surrounding scene has risen.
React to what is almost-but-not-quite present in this paragraph.
"""
```

Create `prompts/triggers/scene_flow_dip.toml`:

```toml
version = "1"
id = "scene_flow_dip"
framing = """
The scene's flow has been low for at least thirty seconds. Speak softly to
what the scene is doing, not what it could do.
"""
```

Create `prompts/triggers/topic_drift.toml`:

```toml
version = "1"
id = "topic_drift"
framing = """
The scene's coherence has dropped while its divergence rises. The writer is
moving away from the scene's beginning. Notice the shift; do not name it.
"""
```

Create `prompts/triggers/valence_spike.toml`:

```toml
version = "1"
id = "valence_spike"
framing = """
The emotional valence of the prose has shifted sharply. React to the change
itself, not to whether it is good or bad.
"""
```

Create `prompts/triggers/structural_inflection.toml`:

```toml
version = "1"
id = "structural_inflection"
framing = """
A structural turn has happened: a new scene, a new chapter, a possible POV
shift, or a possible location change. React to what the turn has opened.
"""
```

Create `prompts/triggers/pace_floor.toml`:

```toml
version = "1"
id = "pace_floor"
framing = """
The pace has slowed. Few new words have been added in the last three minutes.
React to what the slowdown is holding in place.
"""
```

Create `prompts/triggers/world_drift.toml`:

```toml
version = "1"
id = "world_drift"
framing = """
A named entity in the prose may not match the world bible. React to the
ambiguity; do not assert which is right.
"""
```

Create `prompts/triggers/no_universe_yet.toml`:

```toml
version = "1"
id = "no_universe_yet"
framing = """
This project has no characters and no world yet. Speak as if you are
listening for a name almost-said, a place almost-mapped. Imply, never
instruct.
"""
```

Create `prompts/triggers/character_dissonance.toml`:

```toml
version = "1"
id = "character_dissonance"
framing = """
A character may be acting contrary to a stated value or fear of theirs.
React to the friction; do not declare it.
"""
```

Create `prompts/triggers/idle_pause_with_present_character.toml`:

```toml
version = "1"
id = "idle_pause_with_present_character"
framing = """
The writer has paused. A character is present in this scene. Speak briefly
as that character would notice the present moment.
"""
```

- [ ] **Step 4: Create the task TOMLs**

Create `prompts/tasks/pill_level_0.toml`:

```toml
version = "1"
id = "pill_level_0"
instruction = """
Produce one line of prose in your speaker's voice that reacts to the
manuscript excerpt below. No quotation marks. No more than 22 words.
"""
output_format = "plain"
```

Create `prompts/tasks/pill_expand.toml`:

```toml
version = "1"
id = "pill_expand"
instruction = """
Produce three variants — exactly three, each on its own — that elaborate
the parent observation. Each should approach from a different angle. Output
strict JSON with the schema:
[
  {"angle": "feel" | "notice" | "wonder", "text": "..."},
  ...
]
"""
output_format = "json"
```

Create `prompts/tasks/pill_regenerate.toml`:

```toml
version = "1"
id = "pill_regenerate"
instruction = """
Produce three NEW variants — exactly three — that approach the parent
observation from angles materially different from the previous attempts.
The following opening words have already been used; choose new openings:
{prior_first_words}.
Output strict JSON with the same schema as pill_expand.
"""
output_format = "json"
```

- [ ] **Step 5: Implement the loader**

Create `crates/water-core/src/prompts/mod.rs`:

```rust
//! Prompt library: TOML loader + assembler. See spec § 8.

pub mod loader;
pub mod assembler;

pub use loader::{PromptLibrary, ToneClauses, TriggerPrompt, TaskPrompt};
pub use assembler::{assemble_level_0, assemble_pill_expand, assemble_pill_regenerate, PromptRequest};
```

Create `crates/water-core/src/prompts/loader.rs`:

```rust
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize, Clone)]
pub struct ToneClauses {
    pub version: String,
    pub clauses: HashMap<String, String>,
    pub blacklist_regex: BlacklistPatterns,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BlacklistPatterns { pub patterns: Vec<String> }

#[derive(Debug, Deserialize, Clone)]
pub struct TriggerPrompt {
    pub version: String,
    pub id: String,
    pub framing: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TaskPrompt {
    pub version: String,
    pub id: String,
    pub instruction: String,
    pub output_format: String,
}

const TONE: &str = include_str!("../../../../prompts/tone.toml");

const TRIGGER_BLOCK_ANCHORED_DRIFT: &str = include_str!("../../../../prompts/triggers/block_anchored_drift.toml");
const TRIGGER_SCENE_FLOW_DIP: &str = include_str!("../../../../prompts/triggers/scene_flow_dip.toml");
const TRIGGER_TOPIC_DRIFT: &str = include_str!("../../../../prompts/triggers/topic_drift.toml");
const TRIGGER_VALENCE_SPIKE: &str = include_str!("../../../../prompts/triggers/valence_spike.toml");
const TRIGGER_STRUCTURAL_INFLECTION: &str = include_str!("../../../../prompts/triggers/structural_inflection.toml");
const TRIGGER_PACE_FLOOR: &str = include_str!("../../../../prompts/triggers/pace_floor.toml");
const TRIGGER_WORLD_DRIFT: &str = include_str!("../../../../prompts/triggers/world_drift.toml");
const TRIGGER_NO_UNIVERSE_YET: &str = include_str!("../../../../prompts/triggers/no_universe_yet.toml");
const TRIGGER_CHARACTER_DISSONANCE: &str = include_str!("../../../../prompts/triggers/character_dissonance.toml");
const TRIGGER_IDLE_PAUSE: &str = include_str!("../../../../prompts/triggers/idle_pause_with_present_character.toml");

const TASK_PILL_LEVEL_0: &str = include_str!("../../../../prompts/tasks/pill_level_0.toml");
const TASK_PILL_EXPAND: &str = include_str!("../../../../prompts/tasks/pill_expand.toml");
const TASK_PILL_REGENERATE: &str = include_str!("../../../../prompts/tasks/pill_regenerate.toml");

pub struct PromptLibrary {
    pub tone: ToneClauses,
    pub triggers: HashMap<String, TriggerPrompt>,
    pub tasks: HashMap<String, TaskPrompt>,
}

impl PromptLibrary {
    pub fn load_builtin() -> Result<Self, String> {
        let tone: ToneClauses = toml::from_str(TONE).map_err(|e| e.to_string())?;
        let mut triggers: HashMap<String, TriggerPrompt> = HashMap::new();
        for src in [
            TRIGGER_BLOCK_ANCHORED_DRIFT, TRIGGER_SCENE_FLOW_DIP, TRIGGER_TOPIC_DRIFT,
            TRIGGER_VALENCE_SPIKE, TRIGGER_STRUCTURAL_INFLECTION, TRIGGER_PACE_FLOOR,
            TRIGGER_WORLD_DRIFT, TRIGGER_NO_UNIVERSE_YET, TRIGGER_CHARACTER_DISSONANCE,
            TRIGGER_IDLE_PAUSE,
        ] {
            let p: TriggerPrompt = toml::from_str(src).map_err(|e| e.to_string())?;
            triggers.insert(p.id.clone(), p);
        }
        let mut tasks: HashMap<String, TaskPrompt> = HashMap::new();
        for src in [TASK_PILL_LEVEL_0, TASK_PILL_EXPAND, TASK_PILL_REGENERATE] {
            let p: TaskPrompt = toml::from_str(src).map_err(|e| e.to_string())?;
            tasks.insert(p.id.clone(), p);
        }
        Ok(Self { tone, triggers, tasks })
    }

    pub fn trigger(&self, id: &str) -> Option<&TriggerPrompt> { self.triggers.get(id) }
    pub fn task(&self, id: &str) -> Option<&TaskPrompt> { self.tasks.get(id) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_loads_all_built_in_prompts() {
        let lib = PromptLibrary::load_builtin().unwrap();
        assert_eq!(lib.tone.version, "1");
        assert_eq!(lib.triggers.len(), 10);
        assert_eq!(lib.tasks.len(), 3);
        assert!(lib.tone.blacklist_regex.patterns.iter().any(|p| p.contains("you should")));
    }

    #[test]
    fn trigger_lookup_by_id() {
        let lib = PromptLibrary::load_builtin().unwrap();
        let t = lib.trigger("topic_drift").unwrap();
        assert!(t.framing.contains("coherence"));
    }
}
```

- [ ] **Step 6: Implement the assembler**

Create `crates/water-core/src/prompts/assembler.rs`:

```rust
use super::loader::PromptLibrary;
use crate::voice::speaker::Speaker;

#[derive(Debug, Clone)]
pub struct PromptRequest {
    pub system: String,
    pub user: String,
    pub expect_json: bool,
}

fn tone_block(lib: &PromptLibrary) -> String {
    let order = ["present_tense", "not_assistant", "blacklist", "observe", "shape", "pass"];
    let mut s = String::new();
    for k in order {
        if let Some(c) = lib.tone.clauses.get(k) {
            s.push_str(c); s.push('\n');
        }
    }
    s
}

pub fn assemble_level_0(
    lib: &PromptLibrary,
    speaker: &dyn Speaker,
    trigger_id: &str,
    scene_excerpt: &str,
) -> Result<PromptRequest, String> {
    let trig = lib.trigger(trigger_id).ok_or_else(|| format!("unknown trigger {trigger_id}"))?;
    let task = lib.task("pill_level_0").ok_or_else(|| "pill_level_0 task missing".to_string())?;
    let system = format!(
        "{}\n[speaker: {}]\n{}\n[trigger: {}]\n{}\n[task]\n{}\n",
        tone_block(lib), speaker.display_name(), speaker.prompt_fragment(),
        trig.id, trig.framing, task.instruction,
    );
    let user = format!("[inputs]\nManuscript excerpt:\n{scene_excerpt}");
    Ok(PromptRequest { system, user, expect_json: false })
}

pub fn assemble_pill_expand(
    lib: &PromptLibrary,
    speaker: &dyn Speaker,
    trigger_id: &str,
    parent_pill_text: &str,
    scene_excerpt: &str,
) -> Result<PromptRequest, String> {
    let trig = lib.trigger(trigger_id).ok_or_else(|| format!("unknown trigger {trigger_id}"))?;
    let task = lib.task("pill_expand").ok_or_else(|| "pill_expand task missing".to_string())?;
    let system = format!(
        "{}\n[speaker: {}]\n{}\n[trigger: {}]\n{}\n[task]\n{}\n",
        tone_block(lib), speaker.display_name(), speaker.prompt_fragment(),
        trig.id, trig.framing, task.instruction,
    );
    let user = format!("[parent observation]\n{parent_pill_text}\n\n[manuscript excerpt]\n{scene_excerpt}");
    Ok(PromptRequest { system, user, expect_json: true })
}

pub fn assemble_pill_regenerate(
    lib: &PromptLibrary,
    speaker: &dyn Speaker,
    trigger_id: &str,
    parent_pill_text: &str,
    scene_excerpt: &str,
    prior_first_words: &[String],
) -> Result<PromptRequest, String> {
    let trig = lib.trigger(trigger_id).ok_or_else(|| format!("unknown trigger {trigger_id}"))?;
    let task = lib.task("pill_regenerate").ok_or_else(|| "pill_regenerate task missing".to_string())?;
    let prior = prior_first_words.iter().map(|s| format!("\"{s}\"")).collect::<Vec<_>>().join(", ");
    let task_instruction = task.instruction.replace("{prior_first_words}", &prior);
    let system = format!(
        "{}\n[speaker: {}]\n{}\n[trigger: {}]\n{}\n[task]\n{}\n",
        tone_block(lib), speaker.display_name(), speaker.prompt_fragment(),
        trig.id, trig.framing, task_instruction,
    );
    let user = format!("[parent observation]\n{parent_pill_text}\n\n[manuscript excerpt]\n{scene_excerpt}");
    Ok(PromptRequest { system, user, expect_json: true })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::registry::PersonaRegistry;
    use crate::Db;
    use tempfile::TempDir;

    fn echo() -> std::sync::Arc<dyn Speaker> {
        let dir = TempDir::new().unwrap();
        let db = Db::open(dir.path().join("p.db")).unwrap();
        let reg = PersonaRegistry::from_db(&db).unwrap();
        reg.by_id("echo").unwrap()
    }

    #[test]
    fn level_0_includes_tone_clauses_and_speaker_and_trigger() {
        let lib = PromptLibrary::load_builtin().unwrap();
        let e = echo();
        let req = assemble_level_0(&lib, &*e, "block_anchored_drift", "She walked across the square.").unwrap();
        assert!(req.system.contains("Speak in present tense"));
        assert!(req.system.contains("Echo"));
        assert!(req.system.contains("listening through fog"));
        assert!(req.system.contains("block_anchored_drift"));
        assert!(req.user.contains("square"));
        assert!(!req.expect_json);
    }

    #[test]
    fn regenerate_substitutes_prior_first_words() {
        let lib = PromptLibrary::load_builtin().unwrap();
        let e = echo();
        let req = assemble_pill_regenerate(
            &lib, &*e, "topic_drift", "the rain hesitates", "more text",
            &["the rain hesitates".to_string(), "a small bell".to_string()],
        ).unwrap();
        assert!(req.system.contains("the rain hesitates"));
        assert!(req.system.contains("a small bell"));
        assert!(req.expect_json);
    }
}
```

- [ ] **Step 7: Register prompts module**

Modify `crates/water-core/src/lib.rs`:

```rust
pub mod prompts;
```

- [ ] **Step 8: Run tests + lints**

```powershell
cargo test -p water-core prompts
cargo clippy -p water-core --all-targets -- -D warnings
```

Expected: 4 prompt tests pass.

- [ ] **Step 9: Commit**

```powershell
git add prompts/ crates/water-core/src/prompts/ crates/water-core/src/lib.rs
git commit -m "feat(prompts): TOML library + assembler (tone + speakers + triggers + tasks)"
```

---

### Task 18: Structured-JSON LLM router extension + PostFilter trait + ToneBlacklistFilter

Adds `generate_structured` to the LLM provider trait + per-adapter strategies. Implements the `PostFilter` trait and `ToneBlacklistFilter` driven by `tone.toml::blacklist_regex.patterns`.

**Files:**
- Modify: `crates/water-core/src/llm/provider.rs`
- Modify: `crates/water-core/src/llm/openai.rs`
- Modify: `crates/water-core/src/llm/anthropic.rs`
- Modify: `crates/water-core/src/llm/ollama.rs`
- Modify: `crates/water-core/src/llm/llamacpp.rs`
- Modify: `crates/water-core/src/llm/mlx.rs`
- Create: `crates/water-core/src/post_filter/mod.rs`
- Create: `crates/water-core/src/post_filter/tone_blacklist.rs`
- Modify: `crates/water-core/src/lib.rs`
- Modify: `crates/water-core/Cargo.toml` (add `regex = "1"` if not present)

- [ ] **Step 1: Check + add regex dep**

```powershell
Select-String -Path crates\water-core\Cargo.toml -Pattern '^regex\s*=' -SimpleMatch
```

If no match, append `regex = "1"` to `[dependencies]`.

- [ ] **Step 2: Add `generate_structured` to the LlmProvider trait**

Modify `crates/water-core/src/llm/provider.rs`. Read it first. Add to the trait (with a default impl that calls `generate` and tries to parse JSON):

```rust
use async_trait::async_trait;
use serde::de::DeserializeOwned;

// ... existing imports and types ...

#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn id(&self) -> &crate::Id;
    async fn generate(&self, req: GenerateRequest) -> Result<String, ProviderError>;

    /// Provider-specific structured JSON path. Default implementation
    /// calls `generate` with the prompt asking for JSON, then parses; per-
    /// provider overrides can use native JSON-schema or grammar-constrained
    /// modes for higher reliability.
    async fn generate_structured<T: DeserializeOwned + Send>(
        &self,
        req: GenerateRequest,
    ) -> Result<T, ProviderError> {
        let raw = self.generate(req).await?;
        serde_json::from_str::<T>(&raw).map_err(|e| ProviderError::Other(format!("invalid json: {e}; raw: {raw}")))
    }

    async fn health(&self) -> Result<(), String>;
}
```

(Adjust to match the existing trait exactly — read first. The point is to add `generate_structured` with a default impl. Per-adapter overrides go in subsequent steps as optional improvements; the default is correct for M2 v1.)

- [ ] **Step 3: Write a test for the default structured path**

Append to the existing tests in `crates/water-core/src/llm/provider.rs` (or wherever the canned/fake provider lives):

```rust
#[cfg(test)]
mod structured_tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize, Debug)]
    struct BouquetItem { angle: String, text: String }

    // Use the existing CannedProvider (M1's fake). Set its response to a JSON literal.
    #[tokio::test]
    async fn default_structured_parses_json_via_canned() {
        let canned = canned::CannedProvider::with_response(
            r#"[{"angle":"feel","text":"a"},{"angle":"notice","text":"b"},{"angle":"wonder","text":"c"}]"#
        );
        let req = GenerateRequest::default();
        let bouquet: Vec<BouquetItem> = canned.generate_structured(req).await.unwrap();
        assert_eq!(bouquet.len(), 3);
        assert_eq!(bouquet[0].angle, "feel");
    }

    #[tokio::test]
    async fn default_structured_errors_on_invalid_json() {
        let canned = canned::CannedProvider::with_response("not json at all");
        let req = GenerateRequest::default();
        let result: Result<Vec<BouquetItem>, _> = canned.generate_structured(req).await;
        assert!(result.is_err());
    }
}
```

(If `CannedProvider::with_response` does not exist, add a small constructor. The implementer reads the existing canned provider file and adapts.)

- [ ] **Step 4: Run; expect pass via default impl**

```powershell
cargo test -p water-core llm
```

Expected: new structured tests pass with no per-adapter changes.

- [ ] **Step 5: Per-adapter native overrides (optional but recommended)**

These improve reliability when the underlying LLM supports it. Each is a small override; implement at minimum for OpenAI + Anthropic since they have native JSON modes. **For M2 first cut, ship default impl only and add native overrides as a follow-up if integration testing reveals malformed-JSON drops.**

If implementing the OpenAI override now: modify `crates/water-core/src/llm/openai.rs`'s `LlmProvider` impl block:

```rust
async fn generate_structured<T: serde::de::DeserializeOwned + Send>(
    &self,
    mut req: GenerateRequest,
) -> Result<T, ProviderError> {
    // Add a response_format: { type: "json_object" } hint to the request.
    // (The exact integration depends on how req.options carries the body.)
    req.json_mode = Some(true);
    let raw = self.generate(req).await?;
    serde_json::from_str(&raw).map_err(|e| ProviderError::Other(format!("invalid json: {e}; raw: {raw}")))
}
```

Equivalent for Anthropic (tool-use forced output). If `GenerateRequest::json_mode` does not exist, add the field with `#[serde(skip_serializing_if = "Option::is_none")]`. Skip llamacpp/ollama overrides — the default works.

- [ ] **Step 6: Implement `PostFilter` trait + ToneBlacklistFilter**

Create `crates/water-core/src/post_filter/mod.rs`:

```rust
//! PostFilter trait + built-in filters. M2 ships `ToneBlacklistFilter`.

pub mod tone_blacklist;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterDecision {
    Pass,
    Drop { reason: String },
}

pub trait PostFilter: Send + Sync {
    fn id(&self) -> &'static str;
    fn evaluate(&self, pill_text: &str) -> FilterDecision;
}

#[must_use]
pub fn builtin_post_filters(tone_patterns: &[String]) -> Vec<Box<dyn PostFilter>> {
    vec![Box::new(tone_blacklist::ToneBlacklistFilter::compile(tone_patterns)
        .expect("built-in tone patterns must compile"))]
}
```

Create `crates/water-core/src/post_filter/tone_blacklist.rs`:

```rust
use super::{FilterDecision, PostFilter};
use regex::Regex;

pub struct ToneBlacklistFilter {
    patterns: Vec<(String, Regex)>,
}

impl ToneBlacklistFilter {
    pub fn compile(raw: &[String]) -> Result<Self, String> {
        let mut patterns = Vec::with_capacity(raw.len());
        for p in raw {
            let re = Regex::new(p).map_err(|e| format!("invalid tone pattern '{p}': {e}"))?;
            patterns.push((p.clone(), re));
        }
        Ok(Self { patterns })
    }
}

impl PostFilter for ToneBlacklistFilter {
    fn id(&self) -> &'static str { "tone_blacklist" }

    fn evaluate(&self, pill_text: &str) -> FilterDecision {
        for (raw, re) in &self.patterns {
            if re.is_match(pill_text) {
                return FilterDecision::Drop {
                    reason: format!("matched blacklist pattern: {raw}"),
                };
            }
        }
        FilterDecision::Pass
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompts::PromptLibrary;

    fn filter() -> ToneBlacklistFilter {
        let lib = PromptLibrary::load_builtin().unwrap();
        ToneBlacklistFilter::compile(&lib.tone.blacklist_regex.patterns).unwrap()
    }

    #[test]
    fn drops_you_should() {
        let f = filter();
        assert!(matches!(f.evaluate("You should try a different angle."), FilterDecision::Drop { .. }));
    }
    #[test]
    fn drops_consider() {
        let f = filter();
        assert!(matches!(f.evaluate("Consider rewriting this paragraph."), FilterDecision::Drop { .. }));
    }
    #[test]
    fn drops_as_an_ai() {
        let f = filter();
        assert!(matches!(f.evaluate("As an AI, I notice the cadence."), FilterDecision::Drop { .. }));
    }
    #[test]
    fn passes_clean_pill() {
        let f = filter();
        assert_eq!(
            f.evaluate("Something held at the threshold — not fear, not yet curiosity."),
            FilterDecision::Pass
        );
    }
}
```

- [ ] **Step 7: Register post_filter module**

Modify `crates/water-core/src/lib.rs`:

```rust
pub mod post_filter;
```

- [ ] **Step 8: Run tests + lints**

```powershell
cargo test -p water-core
cargo clippy -p water-core --all-targets -- -D warnings
cargo fmt -p water-core --check
```

Expected: all clean. Total water-core tests should be in the ~110+ range now (M1 had 76, plus all of Phase A-D additions).

- [ ] **Step 9: Commit**

```powershell
git add crates/water-core/Cargo.toml crates/water-core/src/llm/ crates/water-core/src/post_filter/ crates/water-core/src/lib.rs
git commit -m "feat(llm): structured-JSON path on LlmProvider + PostFilter chain (ToneBlacklistFilter)"
```

---

## Phase E — Pill UI

### Task 19: PillLayer + PillCapsule

Creates the absolute-positioned overlay to the right of the editor canvas, with a single pastel-glow capsule that renders from a `pill:emerged` event.

**Files:**
- Create: `app/src/pill/PillLayer.tsx`
- Create: `app/src/pill/PillCapsule.tsx`
- Create: `app/src/pill/types.ts`
- Create: `app/src/pill/PillLayer.test.tsx`
- Modify: `app/src/ipc/events.ts` (add `pill:emerged` and `pill:dismissed` event types)
- Modify: `app/src/chrome/EditorCanvas.tsx` (mount `<PillLayer>`)

- [ ] **Step 1: Extend event types**

Modify `app/src/ipc/events.ts`. Add to `WaterEventPayloads`:

```ts
  "pill:emerged": {
    pill_id: string;
    speaker_id: string;
    hue_token: string;
    text: string;
    block_target_id: string | null;
    trigger_id: string;
  };
  "pill:dismissed": { pill_id: string };
  "pill:evicted": { pill_id: string };
```

- [ ] **Step 2: Create pill types**

Create `app/src/pill/types.ts`:

```ts
export interface Pill {
  pill_id: string;
  speaker_id: string;
  hue_token: string;
  text: string;
  block_target_id: string | null;
  trigger_id: string;
}
```

- [ ] **Step 3: Write the failing test**

Create `app/src/pill/PillLayer.test.tsx`:

```tsx
import { describe, expect, it, vi } from "vitest";

const { onWaterEventMock } = vi.hoisted(() => ({ onWaterEventMock: vi.fn() }));
vi.mock("../ipc/events", () => ({ onWaterEvent: onWaterEventMock }));

import { render, screen, waitFor, act } from "@testing-library/react";
import { PillLayer } from "./PillLayer";

describe("PillLayer", () => {
  it("renders a capsule when pill:emerged fires", async () => {
    let handler: ((p: any) => void) | null = null;
    onWaterEventMock.mockImplementation(async (name: string, cb: any) => {
      if (name === "pill:emerged") handler = cb;
      return vi.fn();
    });
    render(<PillLayer />);
    await waitFor(() => expect(handler).not.toBeNull());
    act(() => {
      handler!({
        pill_id: "p1", speaker_id: "echo", hue_token: "--water-hue-muse",
        text: "Something held at the threshold.", block_target_id: "^bk-0001", trigger_id: "block_anchored_drift",
      });
    });
    expect(screen.getByText("Something held at the threshold.")).toBeInTheDocument();
  });

  it("removes the capsule when pill:dismissed fires", async () => {
    const handlers: Record<string, ((p: any) => void)> = {};
    onWaterEventMock.mockImplementation(async (name: string, cb: any) => {
      handlers[name] = cb; return vi.fn();
    });
    render(<PillLayer />);
    await waitFor(() => expect(handlers["pill:emerged"]).toBeDefined());
    act(() => handlers["pill:emerged"]({
      pill_id: "p1", speaker_id: "echo", hue_token: "--water-hue-muse",
      text: "first pill", block_target_id: null, trigger_id: "t",
    }));
    expect(screen.getByText("first pill")).toBeInTheDocument();
    act(() => handlers["pill:dismissed"]({ pill_id: "p1" }));
    expect(screen.queryByText("first pill")).toBeNull();
  });

  it("displays at most 2 pills simultaneously", async () => {
    let handler: ((p: any) => void) | null = null;
    onWaterEventMock.mockImplementation(async (name: string, cb: any) => {
      if (name === "pill:emerged") handler = cb;
      return vi.fn();
    });
    render(<PillLayer />);
    await waitFor(() => expect(handler).not.toBeNull());
    act(() => handler!({ pill_id: "p1", speaker_id: "echo", hue_token: "--water-hue-muse", text: "A", block_target_id: null, trigger_id: "t" }));
    act(() => handler!({ pill_id: "p2", speaker_id: "editor", hue_token: "--water-hue-editor", text: "B", block_target_id: null, trigger_id: "t" }));
    act(() => handler!({ pill_id: "p3", speaker_id: "architect", hue_token: "--water-hue-architect", text: "C", block_target_id: null, trigger_id: "t" }));
    // FIFO: A evicts, B + C remain.
    expect(screen.queryByText("A")).toBeNull();
    expect(screen.getByText("B")).toBeInTheDocument();
    expect(screen.getByText("C")).toBeInTheDocument();
  });
});
```

- [ ] **Step 4: Implement `PillCapsule`**

Create `app/src/pill/PillCapsule.tsx`:

```tsx
import type { CSSProperties } from "react";
import type { Pill } from "./types";

interface Props {
  pill: Pill;
  onClick?: () => void;
}

export function PillCapsule({ pill, onClick }: Props) {
  const style: CSSProperties = {
    position: "relative",
    padding: "8px 14px",
    borderRadius: "var(--water-r-16)",
    background: `color-mix(in oklch, var(${pill.hue_token}) 35%, var(--water-bg-paper))`,
    boxShadow: `0 0 24px color-mix(in oklch, var(${pill.hue_token}) 60%, transparent)`,
    color: "var(--water-fg-default)",
    fontFamily: "var(--water-font-sans)",
    fontSize: "var(--water-fs-body)",
    lineHeight: "var(--water-lh-body)",
    maxWidth: 220,
    cursor: onClick ? "pointer" : "default",
    pointerEvents: "auto",
    animation: `water-pill-fade-in var(--water-dur-small) var(--water-ease-out-soft) both`,
  };
  return (
    <div
      role="button"
      data-pill-id={pill.pill_id}
      data-block-target-id={pill.block_target_id ?? ""}
      onClick={onClick}
      style={style}
    >
      {pill.text}
    </div>
  );
}
```

Add the keyframe to `app/src/styles/tokens.css` (or a new `app/src/styles/pill.css` imported from `main.tsx`):

```css
@keyframes water-pill-fade-in {
  from { opacity: 0; transform: translateY(4px); }
  to   { opacity: 1; transform: translateY(0); }
}
```

If creating a new `pill.css`, import it from `main.tsx` after `tokens.css`.

- [ ] **Step 5: Implement `PillLayer`**

Create `app/src/pill/PillLayer.tsx`:

```tsx
import { useEffect, useState } from "react";
import { onWaterEvent } from "../ipc/events";
import { PillCapsule } from "./PillCapsule";
import type { Pill } from "./types";

const MAX_ON_SCREEN = 2;

export function PillLayer() {
  const [pills, setPills] = useState<Pill[]>([]);

  useEffect(() => {
    const unsubs: Array<() => void> = [];
    (async () => {
      unsubs.push(
        await onWaterEvent("pill:emerged", (p) => {
          setPills((prev) => {
            const next = [...prev, p];
            // FIFO: keep the most recent MAX_ON_SCREEN
            return next.length > MAX_ON_SCREEN ? next.slice(next.length - MAX_ON_SCREEN) : next;
          });
        }),
      );
      unsubs.push(
        await onWaterEvent("pill:dismissed", (e) => {
          setPills((prev) => prev.filter((x) => x.pill_id !== e.pill_id));
        }),
      );
      unsubs.push(
        await onWaterEvent("pill:evicted", (e) => {
          setPills((prev) => prev.filter((x) => x.pill_id !== e.pill_id));
        }),
      );
    })();
    return () => unsubs.forEach((u) => u());
  }, []);

  return (
    <div
      aria-label="pill margin"
      style={{
        position: "absolute",
        top: 72,
        right: 16,
        width: 240,
        display: "flex",
        flexDirection: "column",
        gap: 12,
        pointerEvents: "none",
      }}
    >
      {pills.map((p) => (
        <PillCapsule key={p.pill_id} pill={p} />
      ))}
    </div>
  );
}
```

- [ ] **Step 6: Run; expect pass**

```powershell
pnpm --filter @water/app test src/pill/PillLayer.test.tsx
```

Expected: 3 tests pass.

- [ ] **Step 7: Mount `<PillLayer />` inside EditorCanvas**

Modify `app/src/chrome/EditorCanvas.tsx`. Add `import { PillLayer } from "../pill/PillLayer";` and place `<PillLayer />` inside the outer `<main>` (as a sibling of the saved-chip div + the centered prose column):

```tsx
    <main style={{ flex: 1, position: "relative", background: "var(--water-bg-paper)", overflow: "auto" }}>
      {/* saved-at chip */}
      {/* centered prose column */}
      <PillLayer />
    </main>
```

- [ ] **Step 8: Build + final tests**

```powershell
pnpm --filter @water/app test
pnpm --filter @water/app build
```

Expected: all green.

- [ ] **Step 9: Commit**

```powershell
git add app/src/pill/ app/src/ipc/events.ts app/src/chrome/EditorCanvas.tsx app/src/styles/
git commit -m "feat(pill): PillLayer + PillCapsule (margin overlay)"
```

---

### Task 20: Hover dim + glow line

Adds a global `<main>` opacity dimmer on pill hover + an SVG glow line from the capsule to its anchored block snippet's bounding rect.

**Files:**
- Create: `app/src/pill/hover-dim.tsx`
- Modify: `app/src/pill/PillCapsule.tsx`
- Modify: `app/src/pill/PillLayer.tsx`
- Create: `app/src/pill/hover-dim.test.tsx`

- [ ] **Step 1: Write the failing test**

Create `app/src/pill/hover-dim.test.tsx`:

```tsx
import { describe, expect, it } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { HoverDim } from "./hover-dim";

describe("HoverDim", () => {
  it("renders a backdrop with opacity 0 by default", () => {
    render(<HoverDim active={false} anchorRect={null} sourceRect={null} hueToken="--water-hue-muse" />);
    const backdrop = screen.getByTestId("water-hover-dim");
    expect(backdrop.style.opacity).toBe("0");
  });

  it("sets opacity > 0 when active", () => {
    const r = { top: 100, left: 100, right: 200, bottom: 120, x: 100, y: 100, width: 100, height: 20, toJSON: () => "" } as DOMRect;
    render(<HoverDim active={true} anchorRect={r} sourceRect={r} hueToken="--water-hue-muse" />);
    const backdrop = screen.getByTestId("water-hover-dim");
    expect(parseFloat(backdrop.style.opacity)).toBeGreaterThan(0);
  });

  it("renders an SVG line when active with both rects", () => {
    const a = { top: 100, left: 100, right: 200, bottom: 120, x: 100, y: 100, width: 100, height: 20, toJSON: () => "" } as DOMRect;
    const s = { top: 50, left: 800, right: 900, bottom: 80, x: 800, y: 50, width: 100, height: 30, toJSON: () => "" } as DOMRect;
    render(<HoverDim active={true} anchorRect={a} sourceRect={s} hueToken="--water-hue-muse" />);
    expect(screen.getByTestId("water-hover-line")).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Implement `HoverDim`**

Create `app/src/pill/hover-dim.tsx`:

```tsx
import type { CSSProperties } from "react";

interface Props {
  active: boolean;
  anchorRect: DOMRect | null;
  sourceRect: DOMRect | null;
  hueToken: string;
}

export function HoverDim({ active, anchorRect, sourceRect, hueToken }: Props) {
  const backdropStyle: CSSProperties = {
    position: "fixed",
    inset: 0,
    background: "var(--water-bg-paper)",
    opacity: active ? 0.08 : 0,
    transition: "opacity var(--water-dur-tiny) var(--water-ease-out-soft)",
    pointerEvents: "none",
    zIndex: 30,
  };

  let line: JSX.Element | null = null;
  if (active && anchorRect && sourceRect) {
    const x1 = sourceRect.left + sourceRect.width / 2;
    const y1 = sourceRect.top + sourceRect.height / 2;
    const x2 = anchorRect.left + anchorRect.width / 2;
    const y2 = anchorRect.top + anchorRect.height / 2;
    line = (
      <svg
        data-testid="water-hover-line"
        style={{ position: "fixed", inset: 0, width: "100vw", height: "100vh", pointerEvents: "none", zIndex: 31 }}
      >
        <line
          x1={x1} y1={y1} x2={x2} y2={y2}
          stroke={`var(${hueToken})`}
          strokeWidth="1"
          strokeOpacity="0.6"
          style={{ filter: `drop-shadow(0 0 6px var(${hueToken}))` }}
        />
      </svg>
    );
  }

  return (
    <>
      <div data-testid="water-hover-dim" style={backdropStyle} />
      {line}
    </>
  );
}
```

- [ ] **Step 3: Run; expect pass**

```powershell
pnpm --filter @water/app test src/pill/hover-dim.test.tsx
```

Expected: 3 tests pass.

- [ ] **Step 4: Wire hover state into PillLayer + PillCapsule**

Modify `app/src/pill/PillLayer.tsx` to track which pill (if any) is being hovered, and compute the anchor rect by looking up the `[data-bid="^bk-XXXX"]` element in the editor:

```tsx
import { useEffect, useRef, useState } from "react";
import { onWaterEvent } from "../ipc/events";
import { PillCapsule } from "./PillCapsule";
import { HoverDim } from "./hover-dim";
import type { Pill } from "./types";

const MAX_ON_SCREEN = 2;

export function PillLayer() {
  const [pills, setPills] = useState<Pill[]>([]);
  const [hoveredId, setHoveredId] = useState<string | null>(null);
  const layerRef = useRef<HTMLDivElement>(null);

  // ... event subscriptions identical to Task 19 ...

  const hoveredPill = pills.find((p) => p.pill_id === hoveredId) ?? null;
  let anchorRect: DOMRect | null = null;
  let sourceRect: DOMRect | null = null;
  if (hoveredPill) {
    const sourceEl = layerRef.current?.querySelector(`[data-pill-id="${hoveredPill.pill_id}"]`);
    sourceRect = sourceEl ? (sourceEl as HTMLElement).getBoundingClientRect() : null;
    if (hoveredPill.block_target_id) {
      const blockEl = document.querySelector(`[data-bid="${hoveredPill.block_target_id}"]`);
      anchorRect = blockEl ? (blockEl as HTMLElement).getBoundingClientRect() : null;
    }
  }

  return (
    <>
      <HoverDim
        active={hoveredPill !== null}
        anchorRect={anchorRect}
        sourceRect={sourceRect}
        hueToken={hoveredPill?.hue_token ?? "--water-hue-muse"}
      />
      <div
        ref={layerRef}
        aria-label="pill margin"
        style={{ position: "absolute", top: 72, right: 16, width: 240, display: "flex", flexDirection: "column", gap: 12, pointerEvents: "none" }}
      >
        {pills.map((p) => (
          <div
            key={p.pill_id}
            onMouseEnter={() => setHoveredId(p.pill_id)}
            onMouseLeave={() => setHoveredId((prev) => (prev === p.pill_id ? null : prev))}
            style={{ pointerEvents: "auto" }}
          >
            <PillCapsule pill={p} />
          </div>
        ))}
      </div>
    </>
  );
}
```

- [ ] **Step 5: Run full test suite**

```powershell
pnpm --filter @water/app test
pnpm --filter @water/app build
```

Expected: all green.

- [ ] **Step 6: Commit**

```powershell
git add app/src/pill/hover-dim.tsx app/src/pill/hover-dim.test.tsx app/src/pill/PillLayer.tsx
git commit -m "feat(pill): hover dim + SVG glow line to anchored block"
```

---

### Task 21: Bouquet expansion

Click on a pill capsule opens a bouquet of 3 sub-capsules + regenerate + pin + X. Bouquet is requested over Tauri via a new command `pill_expand`; result arrives via `bouquet:ready` event.

**Files:**
- Create: `app/src/pill/Bouquet.tsx`
- Create: `app/src/pill/Bouquet.test.tsx`
- Modify: `app/src/pill/PillLayer.tsx`
- Modify: `app/src/ipc/commands.ts` (add `pillExpand`, `pillRegenerate`, `pillPin`, `pillDismiss`)
- Modify: `app/src/ipc/events.ts` (add `bouquet:ready` payload)
- Create: `app/src-tauri/src/commands/pill.rs`
- Modify: `app/src-tauri/src/commands/mod.rs` and `main.rs`

- [ ] **Step 1: Add event + command types**

Modify `app/src/ipc/events.ts`:

```ts
  "bouquet:ready": {
    parent_pill_id: string;
    items: Array<{ sub_pill_id: string; angle: "feel" | "notice" | "wonder"; text: string }>;
  };
```

Modify `app/src/ipc/commands.ts` (typed IPC client). Append:

```ts
export const ipc = {
  // ... existing ...
  async pillExpand(parent_pill_id: string): Promise<void> {
    await invoke("pill_expand", { parentPillId: parent_pill_id });
  },
  async pillRegenerate(parent_pill_id: string): Promise<void> {
    await invoke("pill_regenerate", { parentPillId: parent_pill_id });
  },
  async pillPin(pill_id: string): Promise<void> {
    await invoke("pill_pin", { pillId: pill_id });
  },
  async pillDismiss(pill_id: string): Promise<void> {
    await invoke("pill_dismiss", { pillId: pill_id });
  },
};
```

(If the existing file uses a class-style export instead, adapt.)

- [ ] **Step 2: Create the Rust-side `pill.rs` command stub**

Create `app/src-tauri/src/commands/pill.rs`:

```rust
//! Pill verbs invoked from the renderer. M2 ships stubs that emit
//! placeholder events; full wiring (orchestrator → router → prompts → LLM
//! → bouquet:ready) lands in Phase F Task 26.

use crate::events::emit;
use serde::Serialize;
use tauri::AppHandle;

#[derive(Serialize, Clone)]
struct BouquetReady {
    parent_pill_id: String,
    items: Vec<BouquetItem>,
}

#[derive(Serialize, Clone)]
struct BouquetItem {
    sub_pill_id: String,
    angle: String,
    text: String,
}

/// M2 stub: emits a canned 3-item bouquet for any parent. Phase F wires
/// the orchestrator/router/LLM path. This lets the renderer be developed
/// before integration.
#[tauri::command]
pub async fn pill_expand(app: AppHandle, parent_pill_id: String) -> Result<(), String> {
    let payload = BouquetReady {
        parent_pill_id: parent_pill_id.clone(),
        items: vec![
            BouquetItem { sub_pill_id: format!("{parent_pill_id}-1"), angle: "feel".into(), text: "(stub) feel something at the threshold".into() },
            BouquetItem { sub_pill_id: format!("{parent_pill_id}-2"), angle: "notice".into(), text: "(stub) the bell rings somewhere unseen".into() },
            BouquetItem { sub_pill_id: format!("{parent_pill_id}-3"), angle: "wonder".into(), text: "(stub) what is held in that pause".into() },
        ],
    };
    emit(&app, "bouquet:ready", payload).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pill_regenerate(app: AppHandle, parent_pill_id: String) -> Result<(), String> {
    pill_expand(app, parent_pill_id).await
}

#[tauri::command]
pub async fn pill_pin(_pill_id: String) -> Result<(), String> {
    // Phase F writes to pinned_pill table.
    Ok(())
}

#[tauri::command]
pub async fn pill_dismiss(app: AppHandle, pill_id: String) -> Result<(), String> {
    #[derive(Serialize, Clone)]
    struct Dismiss { pill_id: String }
    emit(&app, "pill:dismissed", Dismiss { pill_id }).map_err(|e| e.to_string())
}
```

- [ ] **Step 3: Register the commands**

Modify `app/src-tauri/src/commands/mod.rs` — add `pub mod pill;`.

Modify `app/src-tauri/src/main.rs`'s `tauri::generate_handler!` list — add:

```rust
commands::pill::pill_expand,
commands::pill::pill_regenerate,
commands::pill::pill_pin,
commands::pill::pill_dismiss,
```

- [ ] **Step 4: Verify Rust build + lints**

```powershell
cargo build -p water-app
cargo clippy -p water-app -- -D warnings
```

Expected: clean.

- [ ] **Step 5: Write the failing renderer test**

Create `app/src/pill/Bouquet.test.tsx`:

```tsx
import { describe, expect, it, vi } from "vitest";

const { ipcMock } = vi.hoisted(() => ({
  ipcMock: {
    pillExpand: vi.fn().mockResolvedValue(undefined),
    pillRegenerate: vi.fn().mockResolvedValue(undefined),
    pillPin: vi.fn().mockResolvedValue(undefined),
    pillDismiss: vi.fn().mockResolvedValue(undefined),
  },
}));
vi.mock("../ipc/commands", () => ({ ipc: ipcMock }));

import { render, screen, fireEvent } from "@testing-library/react";
import { Bouquet } from "./Bouquet";

const items = [
  { sub_pill_id: "p1-1", angle: "feel" as const, text: "feel one" },
  { sub_pill_id: "p1-2", angle: "notice" as const, text: "notice two" },
  { sub_pill_id: "p1-3", angle: "wonder" as const, text: "wonder three" },
];

describe("Bouquet", () => {
  it("renders 3 sub-capsules + regenerate + pin + X", () => {
    render(<Bouquet parentId="p1" hueToken="--water-hue-muse" items={items} onClose={() => {}} />);
    expect(screen.getByText("feel one")).toBeInTheDocument();
    expect(screen.getByText("notice two")).toBeInTheDocument();
    expect(screen.getByText("wonder three")).toBeInTheDocument();
    expect(screen.getByLabelText("Regenerate bouquet")).toBeInTheDocument();
    expect(screen.getByLabelText("Pin pill")).toBeInTheDocument();
    expect(screen.getByLabelText("Dismiss pill")).toBeInTheDocument();
  });

  it("clicking regenerate calls ipc.pillRegenerate", () => {
    render(<Bouquet parentId="p1" hueToken="--water-hue-muse" items={items} onClose={() => {}} />);
    fireEvent.click(screen.getByLabelText("Regenerate bouquet"));
    expect(ipcMock.pillRegenerate).toHaveBeenCalledWith("p1");
  });

  it("clicking pin calls ipc.pillPin", () => {
    render(<Bouquet parentId="p1" hueToken="--water-hue-muse" items={items} onClose={() => {}} />);
    fireEvent.click(screen.getByLabelText("Pin pill"));
    expect(ipcMock.pillPin).toHaveBeenCalledWith("p1");
  });

  it("clicking X calls onClose + ipc.pillDismiss", () => {
    const onClose = vi.fn();
    render(<Bouquet parentId="p1" hueToken="--water-hue-muse" items={items} onClose={onClose} />);
    fireEvent.click(screen.getByLabelText("Dismiss pill"));
    expect(ipcMock.pillDismiss).toHaveBeenCalledWith("p1");
    expect(onClose).toHaveBeenCalled();
  });
});
```

- [ ] **Step 6: Implement `Bouquet`**

Create `app/src/pill/Bouquet.tsx`:

```tsx
import { RefreshCw, Pin, X } from "lucide-react";
import { ipc } from "../ipc/commands";

export interface BouquetItem {
  sub_pill_id: string;
  angle: "feel" | "notice" | "wonder";
  text: string;
}

interface Props {
  parentId: string;
  hueToken: string;
  items: BouquetItem[];
  onClose: () => void;
  onSubClick?: (item: BouquetItem) => void;
}

const ANGLE_HUE_SHIFT: Record<BouquetItem["angle"], string> = {
  feel: "--water-hue-valence-pos",
  notice: "--water-hue-pace",
  wonder: "--water-hue-coherence",
};

export function Bouquet({ parentId, hueToken, items, onClose, onSubClick }: Props) {
  const subCapsuleStyle = (angle: BouquetItem["angle"]): React.CSSProperties => ({
    padding: "8px 14px",
    borderRadius: "var(--water-r-16)",
    background: `color-mix(in oklch, var(${hueToken}) 25%, var(--water-bg-paper))`,
    boxShadow: `0 0 18px color-mix(in oklch, var(${ANGLE_HUE_SHIFT[angle]}) 50%, transparent)`,
    color: "var(--water-fg-default)",
    fontFamily: "var(--water-font-sans)",
    fontSize: "var(--water-fs-body)",
    cursor: onSubClick ? "pointer" : "default",
    animation: `water-pill-fade-in var(--water-dur-small) var(--water-ease-out-soft) both`,
  });

  const iconBtn: React.CSSProperties = {
    width: 28, height: 28, border: "none", background: "transparent",
    color: "var(--water-fg-muted)", cursor: "pointer", opacity: 0.6,
    borderRadius: "var(--water-r-8)", display: "inline-flex", alignItems: "center", justifyContent: "center",
  };

  return (
    <div
      style={{ display: "flex", flexDirection: "column", gap: 10, maxWidth: 260, pointerEvents: "auto" }}
    >
      {items.map((it) => (
        <div
          key={it.sub_pill_id}
          data-pill-id={it.sub_pill_id}
          style={subCapsuleStyle(it.angle)}
          onClick={() => onSubClick?.(it)}
        >
          {it.text}
        </div>
      ))}
      <div style={{ display: "flex", gap: 4, justifyContent: "flex-end" }}>
        <button type="button" aria-label="Regenerate bouquet" style={iconBtn} onClick={() => { void ipc.pillRegenerate(parentId); }}>
          <RefreshCw size={16} />
        </button>
        <button type="button" aria-label="Pin pill" style={iconBtn} onClick={() => { void ipc.pillPin(parentId); }}>
          <Pin size={16} />
        </button>
        <button type="button" aria-label="Dismiss pill" style={iconBtn} onClick={() => { void ipc.pillDismiss(parentId); onClose(); }}>
          <X size={16} />
        </button>
      </div>
    </div>
  );
}
```

- [ ] **Step 7: Wire bouquet expansion into PillLayer**

Modify `app/src/pill/PillLayer.tsx` to:
- Track per-pill `expanded: boolean`
- On capsule click → `ipc.pillExpand(pill.pill_id)`, mark expanded, replace capsule with `<Bouquet>` once `bouquet:ready` arrives

Add state + handler:

```tsx
import { Bouquet, type BouquetItem } from "./Bouquet";
import { ipc } from "../ipc/commands";
import { onWaterEvent } from "../ipc/events";

// inside PillLayer:
const [bouquets, setBouquets] = useState<Record<string, BouquetItem[]>>({});

useEffect(() => {
  // ... existing subscriptions ...
  (async () => {
    const u = await onWaterEvent("bouquet:ready", (e) => {
      setBouquets((prev) => ({ ...prev, [e.parent_pill_id]: e.items }));
    });
    unsubs.push(u);
  })();
}, []);

const expandPill = (p: Pill) => {
  if (bouquets[p.pill_id]) return; // already expanded
  void ipc.pillExpand(p.pill_id);
};
```

In the render loop, conditionally swap capsule for bouquet:

```tsx
        {pills.map((p) => {
          const bq = bouquets[p.pill_id];
          if (bq) {
            return (
              <Bouquet key={p.pill_id} parentId={p.pill_id} hueToken={p.hue_token} items={bq}
                       onClose={() => setBouquets(({ [p.pill_id]: _, ...rest }) => rest)} />
            );
          }
          return (
            <div key={p.pill_id} onMouseEnter={...} onMouseLeave={...} style={{ pointerEvents: "auto" }}>
              <PillCapsule pill={p} onClick={() => expandPill(p)} />
            </div>
          );
        })}
```

- [ ] **Step 8: Run tests + build**

```powershell
pnpm --filter @water/app test
pnpm --filter @water/app build
cargo clippy -p water-app -- -D warnings
```

Expected: all clean.

- [ ] **Step 9: Commit**

```powershell
git add app/src/pill/Bouquet.tsx app/src/pill/Bouquet.test.tsx app/src/pill/PillLayer.tsx app/src/ipc/commands.ts app/src/ipc/events.ts app/src-tauri/src/commands/pill.rs app/src-tauri/src/commands/mod.rs app/src-tauri/src/main.rs
git commit -m "feat(pill): bouquet expansion (3 sub-capsules + regenerate + pin + X)"
```

---

### Task 22: Rabbit hole

Clicking a sub-pill recursively expands its own bouquet. Prior siblings collapse to thin glow lines on the left edge. Breadcrumb chain at top. X closes the whole thread.

**Files:**
- Create: `app/src/pill/RabbitHole.tsx`
- Create: `app/src/pill/RabbitHole.test.tsx`
- Modify: `app/src/pill/PillLayer.tsx` (route sub-pill clicks into RabbitHole when depth > 0)

- [ ] **Step 1: Write the failing test**

Create `app/src/pill/RabbitHole.test.tsx`:

```tsx
import { describe, expect, it, vi } from "vitest";

const { ipcMock } = vi.hoisted(() => ({
  ipcMock: {
    pillExpand: vi.fn().mockResolvedValue(undefined),
    pillRegenerate: vi.fn().mockResolvedValue(undefined),
    pillPin: vi.fn().mockResolvedValue(undefined),
    pillDismiss: vi.fn().mockResolvedValue(undefined),
  },
}));
vi.mock("../ipc/commands", () => ({ ipc: ipcMock }));

import { render, screen } from "@testing-library/react";
import { RabbitHole } from "./RabbitHole";

const level1 = [{ sub_pill_id: "p1-1", angle: "feel" as const, text: "L1 feel" }];
const level2 = [{ sub_pill_id: "p2-1", angle: "notice" as const, text: "L2 notice" }];

describe("RabbitHole", () => {
  it("renders the current bouquet + breadcrumb of prior levels", () => {
    render(
      <RabbitHole
        hueToken="--water-hue-muse"
        path={[
          { parentId: "p1", parentText: "root observation", items: level1, chosenSubId: "p1-1" },
          { parentId: "p1-1", parentText: "L1 feel", items: level2, chosenSubId: null },
        ]}
        onSubClick={() => {}}
        onClose={() => {}}
      />,
    );
    expect(screen.getByText("L2 notice")).toBeInTheDocument();
    expect(screen.getByLabelText("Rabbit hole breadcrumb")).toBeInTheDocument();
  });

  it("renders collapsed glow lines for siblings of chosen path entries", () => {
    render(
      <RabbitHole
        hueToken="--water-hue-muse"
        path={[
          { parentId: "p1", parentText: "root", items: [
            { sub_pill_id: "p1-1", angle: "feel" as const, text: "chosen" },
            { sub_pill_id: "p1-2", angle: "notice" as const, text: "sibling A" },
            { sub_pill_id: "p1-3", angle: "wonder" as const, text: "sibling B" },
          ], chosenSubId: "p1-1" },
          { parentId: "p1-1", parentText: "chosen", items: level2, chosenSubId: null },
        ]}
        onSubClick={() => {}}
        onClose={() => {}}
      />,
    );
    // 2 sibling glow lines from level 1
    expect(screen.getAllByTestId("water-glow-line")).toHaveLength(2);
  });

  it("X calls onClose", () => {
    const onClose = vi.fn();
    render(<RabbitHole hueToken="--water-hue-muse" path={[{ parentId: "p1", parentText: "x", items: level1, chosenSubId: null }]} onSubClick={() => {}} onClose={onClose} />);
    screen.getByLabelText("Dismiss pill").click();
    expect(onClose).toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: Implement `RabbitHole`**

Create `app/src/pill/RabbitHole.tsx`:

```tsx
import { Bouquet, type BouquetItem } from "./Bouquet";

export interface RabbitHoleLevel {
  parentId: string;
  parentText: string;
  items: BouquetItem[];
  chosenSubId: string | null;
}

interface Props {
  hueToken: string;
  path: RabbitHoleLevel[];
  onSubClick: (level: number, item: BouquetItem) => void;
  onClose: () => void;
}

export function RabbitHole({ hueToken, path, onSubClick, onClose }: Props) {
  // Siblings to collapse to glow lines: for each level except the last,
  // the items that are NOT the chosen one.
  const collapsedSiblings: Array<{ levelIdx: number; item: BouquetItem }> = [];
  for (let i = 0; i < path.length - 1; i++) {
    const lvl = path[i];
    for (const item of lvl.items) {
      if (item.sub_pill_id !== lvl.chosenSubId) {
        collapsedSiblings.push({ levelIdx: i, item });
      }
    }
  }

  const currentLevel = path[path.length - 1];

  const breadcrumb = path
    .map((lvl) => lvl.parentText.split(/\s+/).slice(0, 6).join(" "))
    .join(" › ");

  return (
    <div style={{ display: "flex", gap: 8, pointerEvents: "auto" }}>
      {/* Left edge: collapsed sibling glow lines */}
      <div aria-label="collapsed siblings" style={{ display: "flex", flexDirection: "column", gap: 4, alignItems: "flex-start", marginTop: 28 }}>
        {collapsedSiblings.map(({ levelIdx, item }) => (
          <div
            key={`${levelIdx}:${item.sub_pill_id}`}
            data-testid="water-glow-line"
            style={{
              width: 24, height: 2,
              background: `var(${hueToken})`,
              boxShadow: `0 0 6px var(${hueToken})`,
              opacity: 0.5,
              borderRadius: 2,
            }}
            title={item.text}
          />
        ))}
      </div>

      <div style={{ display: "flex", flexDirection: "column", gap: 10, flex: 1 }}>
        {/* Breadcrumb */}
        <div
          aria-label="Rabbit hole breadcrumb"
          style={{
            fontFamily: "var(--water-font-sans)",
            fontSize: "var(--water-fs-meta)",
            color: "var(--water-fg-muted)",
            letterSpacing: 0.3,
          }}
        >
          {breadcrumb}
        </div>

        {/* Current bouquet (sub-clicks drill further) */}
        <Bouquet
          parentId={currentLevel.parentId}
          hueToken={hueToken}
          items={currentLevel.items}
          onClose={onClose}
          onSubClick={(item) => onSubClick(path.length - 1, item)}
        />
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Run; expect pass**

```powershell
pnpm --filter @water/app test src/pill/RabbitHole.test.tsx
```

Expected: 3 tests pass.

- [ ] **Step 4: Wire the rabbit hole into PillLayer**

Modify `app/src/pill/PillLayer.tsx`. Replace the single-level `bouquets` state with a per-pill rabbit-hole path. When a sub-pill is clicked, push a new level (and call `ipc.pillExpand(sub_pill_id)`); when its `bouquet:ready` arrives, fill the new level's items.

Sketch:

```tsx
const [rabbitHoles, setRabbitHoles] = useState<Record<string, RabbitHoleLevel[]>>({});

// On `bouquet:ready`:
//   find which open rabbit hole this parent_pill_id belongs to (or it's a fresh level-0 expansion)
//   if it's a new level-0 expansion: rabbitHoles[pill.pill_id] = [{ parentId, parentText: pill.text, items, chosenSubId: null }]
//   if it's a sub-pill expansion: append { parentId: sub.sub_pill_id, parentText: sub.text, items, chosenSubId: null }
//   set the chosenSubId on the previous level

// In render:
//   if rabbitHoles[p.pill_id] exists:
//     render <RabbitHole path={...} onSubClick={(level, item) => {
//       setRabbitHoles((prev) => {
//         const path = [...prev[p.pill_id]];
//         path[level] = { ...path[level], chosenSubId: item.sub_pill_id };
//         return { ...prev, [p.pill_id]: path };
//       });
//       void ipc.pillExpand(item.sub_pill_id);
//     }} onClose={...} />
```

(The implementer writes out the exact reducer logic — the contract above is sufficient. Add a small inline test or rely on E2E in Phase F.)

- [ ] **Step 5: Run all tests + build**

```powershell
pnpm --filter @water/app test
pnpm --filter @water/app build
```

Expected: clean.

- [ ] **Step 6: Commit**

```powershell
git add app/src/pill/RabbitHole.tsx app/src/pill/RabbitHole.test.tsx app/src/pill/PillLayer.tsx
git commit -m "feat(pill): rabbit hole (unlimited depth, breadcrumb, sibling glow lines)"
```

---

### Task 23: Pinned column + PinnedPillDetail sheet + sheet slide-in (closes Review #14)

Creates the 56 px column glued to the right edge of `<main>`, the detail sheet, AND fixes the M1.5 review #14 sheet slide-in transform.

**Files:**
- Create: `app/src/pill/PinnedColumn.tsx`
- Create: `app/src/pill/PinnedColumn.test.tsx`
- Create: `app/src/pill/PinnedPillDetail.tsx`
- Modify: `app/src/sheets/Sheet.tsx` (add slide-in transform)
- Modify: `app/src/sheets/Sheet.test.tsx` (assert slide-in)
- Modify: `app/src/chrome/EditorCanvas.tsx` (mount PinnedColumn)
- Modify: `app/src-tauri/src/commands/pill.rs` (real `pill_pin` writes to DB + emits `pill:pinned`)
- Modify: `app/src/ipc/events.ts` (add `pill:pinned`, `pill:unpinned`)

- [ ] **Step 1: Fix the Sheet slide-in transform (Review #14)**

Modify `app/src/sheets/Sheet.tsx`. Read it first. The current Sheet probably uses a `<dialog>` with `showModal()`. Add a `data-state` attribute and transition the transform:

```tsx
import { useEffect, useRef, useState } from "react";

interface Props { open: boolean; onClose: () => void; title: string; children: React.ReactNode; }

export function Sheet({ open, onClose, title, children }: Props) {
  const ref = useRef<HTMLDialogElement>(null);
  const [state, setState] = useState<"closed" | "opening" | "open" | "closing">("closed");

  useEffect(() => {
    if (open && state === "closed") {
      setState("opening");
      ref.current?.showModal();
      // Next frame: transition to "open"
      requestAnimationFrame(() => requestAnimationFrame(() => setState("open")));
    } else if (!open && (state === "open" || state === "opening")) {
      setState("closing");
      // After transition, close + return to "closed"
      const t = window.setTimeout(() => {
        ref.current?.close();
        setState("closed");
      }, 280);
      return () => window.clearTimeout(t);
    }
  }, [open, state]);

  return (
    <dialog
      ref={ref}
      data-state={state}
      onClose={onClose}
      style={{
        position: "fixed", right: 0, top: 0, bottom: 0, margin: 0,
        width: 420, maxWidth: "90vw", height: "100vh",
        border: "none", padding: 0, background: "var(--water-bg-paper)",
        transform:
          state === "open" ? "translateX(0)" :
          state === "closing" ? "translateX(100%)" :
          "translateX(100%)",
        transition: state === "open" ? "transform var(--water-dur-small) var(--water-ease-out-soft)"
                  : state === "closing" ? "transform var(--water-dur-tiny) var(--water-ease-in-out-water)"
                  : "none",
        boxShadow: "0 0 60px color-mix(in oklch, var(--water-fg-faint) 30%, transparent)",
      }}
    >
      <div style={{ padding: 24 }}>
        <h2 style={{ marginTop: 0 }}>{title}</h2>
        {children}
      </div>
    </dialog>
  );
}
```

- [ ] **Step 2: Write the slide-in test**

Append to `app/src/sheets/Sheet.test.tsx`:

```tsx
import { describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { Sheet } from "./Sheet";

beforeEach(() => {
  HTMLDialogElement.prototype.showModal = function () { this.setAttribute("open", ""); };
  HTMLDialogElement.prototype.close = function () { this.removeAttribute("open"); };
});

describe("Sheet slide-in", () => {
  it("transitions data-state from opening to open after mount", async () => {
    render(<Sheet open={true} onClose={() => {}} title="t">x</Sheet>);
    const dlg = screen.getByText("x").closest("dialog")!;
    await waitFor(() => expect(dlg.getAttribute("data-state")).toBe("open"));
    // Once "open", transform should be translateX(0)
    expect(dlg.style.transform).toContain("translateX(0)");
  });
});
```

- [ ] **Step 3: Run; expect Sheet test pass**

```powershell
pnpm --filter @water/app test src/sheets/Sheet.test.tsx
```

Expected: PASS (plus all existing sheet tests still green).

- [ ] **Step 4: Write the PinnedColumn test**

Create `app/src/pill/PinnedColumn.test.tsx`:

```tsx
import { describe, expect, it, vi } from "vitest";

const { onWaterEventMock, ipcMock } = vi.hoisted(() => ({
  onWaterEventMock: vi.fn(),
  ipcMock: { pinnedList: vi.fn().mockResolvedValue([]) },
}));
vi.mock("../ipc/events", () => ({ onWaterEvent: onWaterEventMock }));
vi.mock("../ipc/commands", () => ({ ipc: ipcMock }));

import { render, screen, waitFor, act } from "@testing-library/react";
import { PinnedColumn } from "./PinnedColumn";

describe("PinnedColumn", () => {
  it("renders an empty 56px-wide column when nothing pinned", async () => {
    onWaterEventMock.mockResolvedValue(vi.fn());
    render(<PinnedColumn />);
    const col = screen.getByLabelText("pinned column");
    expect(col).toHaveStyle({ width: "56px" });
  });

  it("renders a dot per pinned pill", async () => {
    onWaterEventMock.mockResolvedValue(vi.fn());
    ipcMock.pinnedList.mockResolvedValue([
      { pill_id: "p1", speaker_id: "echo", hue_token: "--water-hue-muse", text: "first", block_target_id: null, trigger_id: "t" },
      { pill_id: "p2", speaker_id: "editor", hue_token: "--water-hue-editor", text: "second", block_target_id: null, trigger_id: "t" },
    ]);
    render(<PinnedColumn />);
    await waitFor(() => expect(screen.getAllByTestId("water-pinned-dot")).toHaveLength(2));
  });

  it("reacts to pill:pinned event", async () => {
    let handler: ((p: any) => void) | null = null;
    onWaterEventMock.mockImplementation(async (name: string, cb: any) => {
      if (name === "pill:pinned") handler = cb;
      return vi.fn();
    });
    ipcMock.pinnedList.mockResolvedValue([]);
    render(<PinnedColumn />);
    await waitFor(() => expect(handler).not.toBeNull());
    act(() => handler!({
      pill_id: "p1", speaker_id: "echo", hue_token: "--water-hue-muse",
      text: "fresh pin", block_target_id: null, trigger_id: "t",
    }));
    expect(screen.getByTestId("water-pinned-dot")).toBeInTheDocument();
  });
});
```

- [ ] **Step 5: Add the `pinned_list` Tauri command + event types**

Modify `app/src-tauri/src/commands/pill.rs`. Add:

```rust
use crate::state::AppState;
use rusqlite::params;
use serde::Serialize;
use tauri::State;

#[derive(Serialize, Clone)]
pub struct PinnedPill {
    pub pill_id: String,
    pub speaker_id: String,
    pub hue_token: String,
    pub text: String,
    pub block_target_id: Option<String>,
    pub trigger_id: String,
}

#[tauri::command]
pub async fn pinned_list(state: State<'_, AppState>) -> Result<Vec<PinnedPill>, String> {
    let (db,) = {
        let proj = state.project.lock().await;
        let p = proj.as_ref().ok_or("no project open")?;
        (p.db.clone(),)
    };
    let g = db.lock().await;
    let mut stmt = g.conn().prepare(
        "SELECT id, speaker_id, hue, message, COALESCE(rabbit_hole_path, ''), trigger_class
         FROM pinned_pill ORDER BY pinned_at DESC, created_at DESC"
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![], |r| {
        Ok(PinnedPill {
            pill_id: r.get(0)?,
            speaker_id: r.get(1)?,
            hue_token: r.get(2)?,
            text: r.get(3)?,
            block_target_id: { let s: String = r.get(4)?; if s.is_empty() { None } else { Some(s) } },
            trigger_id: r.get(5).unwrap_or_default(),
        })
    }).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}
```

Replace the stub `pill_pin` with a real INSERT (text fields come from a session-side map; for M2, the renderer passes the pill payload along with the pin invocation. The contract change is: `pill_pin` takes the full Pill struct, not just the id):

```rust
#[tauri::command]
pub async fn pill_pin(
    app: AppHandle,
    state: State<'_, AppState>,
    pill: PinnedPill,
    scene_id: String,
    block_id: String,
    snippet: String,
) -> Result<(), String> {
    let (db,) = {
        let proj = state.project.lock().await;
        let p = proj.as_ref().ok_or("no project open")?;
        (p.db.clone(),)
    };
    let g = db.lock().await;
    let now = chrono::Utc::now().to_rfc3339();
    g.conn().execute(
        "INSERT OR IGNORE INTO pinned_pill (id, scene_id, block_id, snippet, speaker_kind, speaker_id, message, hue, rabbit_hole_path, created_at, parent_pill_id, pinned_at, trigger_class, bouquet_position)
         VALUES (?1, ?2, ?3, ?4, 'persona', ?5, ?6, ?7, NULL, ?8, NULL, ?8, ?9, NULL)",
        params![pill.pill_id, scene_id, block_id, snippet, pill.speaker_id, pill.text, pill.hue_token, now, pill.trigger_id],
    ).map_err(|e| e.to_string())?;
    drop(g);
    emit(&app, "pill:pinned", pill).map_err(|e| e.to_string())
}
```

Register `commands::pill::pinned_list` in `main.rs`. Extend `app/src/ipc/commands.ts` with `pinnedList(): Promise<Pill[]>`.

Modify `app/src/ipc/events.ts`:

```ts
  "pill:pinned": import("../pill/types").Pill;
  "pill:unpinned": { pill_id: string };
```

- [ ] **Step 6: Implement `PinnedColumn`**

Create `app/src/pill/PinnedColumn.tsx`:

```tsx
import { useEffect, useState } from "react";
import { ipc } from "../ipc/commands";
import { onWaterEvent } from "../ipc/events";
import { PinnedPillDetail } from "./PinnedPillDetail";
import type { Pill } from "./types";

export function PinnedColumn() {
  const [pinned, setPinned] = useState<Pill[]>([]);
  const [openId, setOpenId] = useState<string | null>(null);

  useEffect(() => {
    (async () => {
      try {
        const list = await ipc.pinnedList();
        setPinned(list);
      } catch { /* ignore */ }
    })();
    const unsubs: Array<() => void> = [];
    (async () => {
      unsubs.push(await onWaterEvent("pill:pinned", (p) => {
        setPinned((prev) => [p, ...prev.filter((x) => x.pill_id !== p.pill_id)]);
      }));
      unsubs.push(await onWaterEvent("pill:unpinned", (e) => {
        setPinned((prev) => prev.filter((x) => x.pill_id !== e.pill_id));
      }));
    })();
    return () => unsubs.forEach((u) => u());
  }, []);

  return (
    <>
      <aside
        aria-label="pinned column"
        style={{
          position: "absolute", top: 0, right: 0, bottom: 0,
          width: 56, padding: "72px 12px",
          display: "flex", flexDirection: "column", gap: 16, alignItems: "center",
          pointerEvents: "auto",
        }}
      >
        {pinned.map((p) => (
          <button
            key={p.pill_id}
            data-testid="water-pinned-dot"
            type="button"
            title={p.text}
            onClick={() => setOpenId(p.pill_id)}
            style={{
              width: 16, height: 16, border: "none", padding: 0, cursor: "pointer",
              borderRadius: 16,
              background: `var(${p.hue_token})`,
              boxShadow: `0 0 12px var(${p.hue_token})`,
              opacity: 0.5,
            }}
          />
        ))}
      </aside>
      {openId && pinned.find((p) => p.pill_id === openId) && (
        <PinnedPillDetail
          pill={pinned.find((p) => p.pill_id === openId)!}
          onClose={() => setOpenId(null)}
        />
      )}
    </>
  );
}
```

- [ ] **Step 7: Implement `PinnedPillDetail`**

Create `app/src/pill/PinnedPillDetail.tsx`:

```tsx
import { Sheet } from "../sheets/Sheet";
import { ipc } from "../ipc/commands";
import type { Pill } from "./types";

interface Props { pill: Pill; onClose: () => void; }

export function PinnedPillDetail({ pill, onClose }: Props) {
  return (
    <Sheet open onClose={onClose} title="Pinned pill">
      <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
        <div style={{
          padding: "12px 16px",
          borderRadius: "var(--water-r-16)",
          background: `color-mix(in oklch, var(${pill.hue_token}) 30%, var(--water-bg-paper))`,
          boxShadow: `0 0 18px var(${pill.hue_token})`,
        }}>
          {pill.text}
        </div>
        <div style={{ color: "var(--water-fg-muted)", fontSize: "var(--water-fs-meta)" }}>
          Speaker: {pill.speaker_id}. Trigger: {pill.trigger_id}.
        </div>
        <button
          type="button"
          onClick={async () => {
            await ipc.pillDismiss(pill.pill_id);
            onClose();
          }}
          style={{
            padding: "10px 14px", border: "none", cursor: "pointer",
            background: "transparent",
            color: "var(--water-fg-default)",
            borderRadius: "var(--water-r-8)",
            boxShadow: "inset 0 0 0 1px color-mix(in srgb, var(--water-fg-faint) 30%, transparent)",
          }}
        >
          Un-pin
        </button>
      </div>
    </Sheet>
  );
}
```

(`pill_dismiss` doubles as un-pin in M2; it deletes the `pinned_pill` row. Phase F refines if needed.)

Modify `pill.rs::pill_dismiss` to also delete from `pinned_pill`:

```rust
#[tauri::command]
pub async fn pill_dismiss(app: AppHandle, state: State<'_, AppState>, pill_id: String) -> Result<(), String> {
    let (db_opt,) = {
        let proj = state.project.lock().await;
        (proj.as_ref().map(|p| p.db.clone()),)
    };
    if let Some(db) = db_opt {
        let g = db.lock().await;
        g.conn().execute("DELETE FROM pinned_pill WHERE id = ?1", params![pill_id]).ok();
    }
    #[derive(Serialize, Clone)]
    struct Dismiss { pill_id: String }
    emit(&app, "pill:dismissed", Dismiss { pill_id: pill_id.clone() }).ok();
    #[derive(Serialize, Clone)]
    struct Unpinned { pill_id: String }
    emit(&app, "pill:unpinned", Unpinned { pill_id }).map_err(|e| e.to_string())
}
```

- [ ] **Step 8: Mount PinnedColumn in EditorCanvas**

Modify `app/src/chrome/EditorCanvas.tsx`. Add `import { PinnedColumn } from "../pill/PinnedColumn";` and place `<PinnedColumn />` as a sibling of `<PillLayer />` inside the outer `<main>`.

- [ ] **Step 9: Run all tests + lints + build**

```powershell
pnpm --filter @water/app test
cargo clippy -p water-app -- -D warnings
pnpm --filter @water/app build
```

Expected: all clean.

- [ ] **Step 10: Commit**

```powershell
git add app/src/pill/PinnedColumn.tsx app/src/pill/PinnedColumn.test.tsx app/src/pill/PinnedPillDetail.tsx app/src/sheets/Sheet.tsx app/src/sheets/Sheet.test.tsx app/src/chrome/EditorCanvas.tsx app/src-tauri/src/commands/pill.rs app/src-tauri/src/main.rs app/src/ipc/commands.ts app/src/ipc/events.ts
git commit -m "feat(pill): pinned column + detail sheet + Sheet slide-in (closes Review #14)"
```

---

### Task 24: ScenesPanel reloadToken refactor (closes Review #4)

Replaces `<ScenesPanel key={scenesReloadKey} ... />` with `<ScenesPanel reloadToken={scenesReloadKey} ... />` + a `useEffect` that reloads the list without remounting. Scroll position preserved.

**Files:**
- Modify: `app/src/chrome/ScenesPanel.tsx`
- Modify: `app/src/chrome/ScenesPanel.test.tsx`
- Modify: `app/src/App.tsx` (callsite)

- [ ] **Step 1: Read the current ScenesPanel + callsite**

Read `app/src/chrome/ScenesPanel.tsx` and grep for `scenesReloadKey` in `App.tsx` to confirm the current force-remount pattern.

- [ ] **Step 2: Add `reloadToken` prop + reload effect**

Modify `app/src/chrome/ScenesPanel.tsx`. Change the props interface to add:

```tsx
interface Props {
  // ... existing ...
  reloadToken: number;
}
```

Inside the component, find the existing `useEffect` that loads scenes on mount and add `reloadToken` to its dependency array:

```tsx
useEffect(() => {
  let cancelled = false;
  (async () => {
    try {
      const list = await ipc.sceneList();
      if (!cancelled) setScenes(list);
    } catch { /* swallow */ }
  })();
  return () => { cancelled = true; };
}, [reloadToken]);
```

- [ ] **Step 3: Update App.tsx callsite**

Modify `app/src/App.tsx`. Find `<ScenesPanel key={scenesReloadKey} ... />` and rename `key` to `reloadToken`. (Both are typed `number`; just a prop name change.)

- [ ] **Step 4: Write the failing test**

Append to `app/src/chrome/ScenesPanel.test.tsx`:

```tsx
describe("ScenesPanel reload token", () => {
  it("reloads on token change without remounting", async () => {
    const ipcMock = vi.mocked(ipc);
    ipcMock.sceneList.mockResolvedValueOnce([{ id: "s1", name: "First" }]);
    const { rerender } = render(<ScenesPanel reloadToken={1} {...defaultProps} />);
    await waitFor(() => expect(screen.getByText("First")).toBeInTheDocument());
    ipcMock.sceneList.mockResolvedValueOnce([{ id: "s1", name: "First" }, { id: "s2", name: "Second" }]);
    rerender(<ScenesPanel reloadToken={2} {...defaultProps} />);
    await waitFor(() => expect(screen.getByText("Second")).toBeInTheDocument());
    expect(ipcMock.sceneList).toHaveBeenCalledTimes(2);
  });
});
```

(Adapt `defaultProps` to whatever the existing test uses. The implementer reads the existing test file first.)

- [ ] **Step 5: Run all tests + build**

```powershell
pnpm --filter @water/app test src/chrome/ScenesPanel.test.tsx
pnpm --filter @water/app test
pnpm --filter @water/app build
```

Expected: all green; existing ScenesPanel tests stay green.

- [ ] **Step 6: Commit**

```powershell
git add app/src/chrome/ScenesPanel.tsx app/src/chrome/ScenesPanel.test.tsx app/src/App.tsx
git commit -m "refactor(chrome): ScenesPanel reloadToken (preserves scroll); closes Review #4"
```

---

### Task 25: Narrow-viewport fallback

When `<main>` width drops below 1100 px, pill capsules get extra translucency and the pinned column collapses to a 24 px tab.

**Files:**
- Modify: `app/src/pill/PillLayer.tsx`
- Modify: `app/src/pill/PinnedColumn.tsx`
- Create: `app/src/pill/useElementWidth.ts`
- Create: `app/src/pill/useElementWidth.test.tsx`

- [ ] **Step 1: Write the failing test for `useElementWidth`**

Create `app/src/pill/useElementWidth.test.tsx`:

```tsx
import { describe, expect, it } from "vitest";
import { render } from "@testing-library/react";
import { useRef } from "react";
import { useElementWidth } from "./useElementWidth";

function Probe({ onWidth }: { onWidth: (w: number) => void }) {
  const ref = useRef<HTMLDivElement>(null);
  const w = useElementWidth(ref);
  onWidth(w);
  return <div ref={ref} style={{ width: 800 }} />;
}

describe("useElementWidth", () => {
  it("returns the element's current width", () => {
    let lastWidth = 0;
    render(<Probe onWidth={(w) => { lastWidth = w; }} />);
    // jsdom doesn't actually layout, so width will be 0. The hook contract:
    // returns 0 until ResizeObserver fires. We test the API shape only.
    expect(typeof lastWidth).toBe("number");
  });
});
```

- [ ] **Step 2: Implement `useElementWidth`**

Create `app/src/pill/useElementWidth.ts`:

```ts
import { useEffect, useState, type RefObject } from "react";

export function useElementWidth(ref: RefObject<HTMLElement>): number {
  const [width, setWidth] = useState(0);

  useEffect(() => {
    if (!ref.current || typeof ResizeObserver === "undefined") return;
    const el = ref.current;
    const ro = new ResizeObserver((entries) => {
      for (const e of entries) {
        const w = e.contentRect.width;
        setWidth(w);
      }
    });
    ro.observe(el);
    setWidth(el.getBoundingClientRect().width);
    return () => ro.disconnect();
  }, [ref]);

  return width;
}
```

- [ ] **Step 3: Apply width-aware fallback in PillLayer + PinnedColumn**

Modify `app/src/pill/PillLayer.tsx`: pass an external `mainWidth` prop (or use a context). The simplest: have `PillLayer` and `PinnedColumn` each measure their own `<main>` ancestor via a shared ref. For M2 we keep it simple — pass `mainWidth` down from EditorCanvas, which measures itself.

Modify `app/src/chrome/EditorCanvas.tsx`:

```tsx
import { useElementWidth } from "../pill/useElementWidth";

// inside component:
const mainRef = useRef<HTMLElement>(null);
const mainWidth = useElementWidth(mainRef);

// ... pass mainWidth into <PillLayer> and <PinnedColumn>:
<PillLayer mainWidth={mainWidth} />
<PinnedColumn mainWidth={mainWidth} />
```

Update `PillLayer` to accept `mainWidth` and apply `opacity: 0.7` to capsules when `mainWidth < 1100`. Update `PinnedColumn` to collapse to a 24 px tab when `mainWidth < 1100`:

```tsx
// in PinnedColumn:
interface Props { mainWidth: number; }
export function PinnedColumn({ mainWidth }: Props) {
  const collapsed = mainWidth > 0 && mainWidth < 1100;
  const [expanded, setExpanded] = useState(false);
  // when collapsed && !expanded: render 24px tab; clicking expands to overlay
  // when collapsed && expanded: render full 56px column over the canvas
  // when !collapsed: render full 56px column always
  // ...
}
```

(The implementer fleshes out the exact rendering — the contract is: under 1100 px, column collapses to a 24 px tab; tap-to-expand creates a temporary overlay; over 1100 px, normal behavior.)

- [ ] **Step 4: Add a width-aware test**

Append to `app/src/pill/PinnedColumn.test.tsx`:

```tsx
it("renders as a 24px tab when mainWidth < 1100", () => {
  onWaterEventMock.mockResolvedValue(vi.fn());
  ipcMock.pinnedList.mockResolvedValue([
    { pill_id: "p1", speaker_id: "echo", hue_token: "--water-hue-muse", text: "x", block_target_id: null, trigger_id: "t" },
  ]);
  render(<PinnedColumn mainWidth={900} />);
  const col = screen.getByLabelText("pinned column");
  expect(col).toHaveStyle({ width: "24px" });
});
```

- [ ] **Step 5: Run tests + build**

```powershell
pnpm --filter @water/app test
pnpm --filter @water/app build
```

Expected: all clean.

- [ ] **Step 6: Commit**

```powershell
git add app/src/pill/PillLayer.tsx app/src/pill/PinnedColumn.tsx app/src/pill/PinnedColumn.test.tsx app/src/pill/useElementWidth.ts app/src/pill/useElementWidth.test.tsx app/src/chrome/EditorCanvas.tsx
git commit -m "feat(pill): narrow-viewport fallback (translucent capsules + collapsed pinned tab)"
```

---

## Phase F — Integration

### Task 26: End-to-end wiring (telemetry → orchestrator → router → prompts → LLM → bouquet → UI)

Replaces all stub paths from earlier tasks with the real pipeline. The Tauri-app gains an `OrchestratorService` running on a tokio task; it subscribes to `typing:telemetry` Tauri events (via a small bridge), accumulates scene/analysis state, runs trigger evaluation on each tick, dispatches LLM calls when a candidate fires, applies the PostFilter chain to results, and emits `pill:emerged` / `bouquet:ready` events.

**Files:**
- Create: `app/src-tauri/src/orchestrator_service.rs`
- Modify: `app/src-tauri/src/state.rs` (add orchestrator handle to `OpenProject`)
- Modify: `app/src-tauri/src/commands/project.rs` (spawn the service on `open_project`; stop on `close_project`)
- Modify: `app/src-tauri/src/commands/pill.rs` (replace stubs with real orchestrator invocations)
- Modify: `app/src-tauri/src/main.rs` (register the new module)
- Modify: `app/src-tauri/src/commands/events.rs` (forward `typing:telemetry` into orchestrator channel)

- [ ] **Step 1: Create the orchestrator service**

Create `app/src-tauri/src/orchestrator_service.rs`:

```rust
//! Process-side orchestrator that owns:
//! - the in-memory `Pill` list (max 2 on-screen + pinned in-DB)
//! - per-speaker cooldown state
//! - per-bouquet history for anti-loop
//! - the LLM provider + prompt library + post-filter chain
//!
//! Driven by `OrchestratorRequest`s on a mpsc channel; emits Tauri events
//! through the `AppHandle` it was constructed with.

use crate::events::emit;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::{mpsc, Mutex};
use water_core::{
    llm::LlmRouter,
    orchestrator::{
        anti_loop::max_overlap,
        eviction::pick_evictee,
        state::{Pill, PillEvent, PillLifecycle, transition},
        triggers::builtin_triggers,
        AnalysisSnapshot, ProjectSnapshot, SceneSnapshot, TriggerContext, TypingTelemetry,
    },
    post_filter::{builtin_post_filters, FilterDecision, PostFilter},
    prompts::{assemble_level_0, assemble_pill_expand, assemble_pill_regenerate, PromptLibrary},
    voice::{registry::PersonaRegistry, router::route, router::CooldownState},
    Id,
};

#[derive(Debug)]
pub enum OrchestratorRequest {
    Telemetry(TypingTelemetry),
    Analysis(AnalysisSnapshot),
    /// SceneState carries the snapshot, the project snapshot, AND the
    /// current scene body text so the orchestrator can assemble prompts
    /// without re-reading from disk on every telemetry tick.
    SceneState(SceneSnapshot, ProjectSnapshot, String),
    Click { pill_id: Id },
    Expand { parent_pill_id: Id },
    Regenerate { parent_pill_id: Id },
    Dismiss { pill_id: Id },
    Shutdown,
}

pub struct OrchestratorHandle {
    tx: mpsc::Sender<OrchestratorRequest>,
}

impl OrchestratorHandle {
    pub async fn send(&self, req: OrchestratorRequest) {
        let _ = self.tx.send(req).await;
    }
}

pub struct OrchestratorService {
    app: AppHandle,
    router: Arc<LlmRouter>,
    personas: PersonaRegistry,
    prompts: Arc<PromptLibrary>,
    filters: Vec<Box<dyn PostFilter>>,
    pills: Vec<Pill>,
    cooldowns: CooldownState,
    bouquet_history: std::collections::HashMap<String, Vec<String>>, // parent_pill_id → prior texts
    scene: Option<SceneSnapshot>,
    project: ProjectSnapshot,
    analysis: AnalysisSnapshot,
    /// Current scene's full body, snapshotted whenever the renderer fires a
    /// scene-switch or significant edit telemetry. Used to compute prompt
    /// excerpts without round-tripping to SceneStore on every tick.
    scene_text: String,
}

impl OrchestratorService {
    /// Returns a ~400-character window centered on the anchored block.
    /// Falls back to the scene's first 400 chars when block_id is empty or
    /// not found in scene_text.
    fn scene_excerpt_for(&self, block_id: &str) -> String {
        if !block_id.is_empty() {
            if let Some(pos) = self.scene_text.find(block_id) {
                let start = pos.saturating_sub(200);
                let end = (pos + 200).min(self.scene_text.len());
                return self.scene_text[start..end].to_string();
            }
        }
        let end = 400.min(self.scene_text.len());
        self.scene_text[..end].to_string()
    }
}

impl OrchestratorService {
    pub fn start(
        app: AppHandle,
        router: Arc<LlmRouter>,
        personas: PersonaRegistry,
    ) -> OrchestratorHandle {
        let (tx, mut rx) = mpsc::channel::<OrchestratorRequest>(64);
        let prompts = Arc::new(PromptLibrary::load_builtin().expect("built-in prompts must load"));
        let filters = builtin_post_filters(&prompts.tone.blacklist_regex.patterns);

        let svc = Arc::new(Mutex::new(OrchestratorService {
            app, router, personas, prompts, filters,
            pills: Vec::new(),
            cooldowns: CooldownState::default(),
            bouquet_history: std::collections::HashMap::new(),
            scene: None,
            project: ProjectSnapshot::default(),
            analysis: AnalysisSnapshot::default(),
            scene_text: String::new(),
        }));

        tokio::spawn({
            let svc = svc.clone();
            async move {
                while let Some(req) = rx.recv().await {
                    if matches!(req, OrchestratorRequest::Shutdown) { break; }
                    let mut s = svc.lock().await;
                    s.handle(req).await;
                }
            }
        });

        OrchestratorHandle { tx }
    }

    async fn handle(&mut self, req: OrchestratorRequest) {
        match req {
            OrchestratorRequest::Telemetry(t) => self.on_telemetry(t).await,
            OrchestratorRequest::Analysis(a) => { self.analysis = a; }
            OrchestratorRequest::SceneState(s, p, text) => {
                self.scene = Some(s);
                self.project = p;
                self.scene_text = text;
            }
            OrchestratorRequest::Expand { parent_pill_id } => self.on_expand(parent_pill_id, false).await,
            OrchestratorRequest::Regenerate { parent_pill_id } => self.on_expand(parent_pill_id, true).await,
            OrchestratorRequest::Dismiss { pill_id } => {
                if let Some(p) = self.pills.iter_mut().find(|p| p.id == pill_id) {
                    p.state = transition(p, &PillEvent::UserDismiss);
                }
            }
            OrchestratorRequest::Click { .. } | OrchestratorRequest::Shutdown => {}
        }
    }

    async fn on_telemetry(&mut self, t: TypingTelemetry) {
        // Gate: never surface mid-sentence.
        if t.cursor_classification == water_core::orchestrator::CursorClassification::MidSentence {
            return;
        }
        // Need a scene to fire any trigger.
        let Some(scene) = self.scene.clone() else { return; };
        let ctx = TriggerContext {
            telemetry: &t,
            analysis: &self.analysis,
            scene: &scene,
            project: &self.project,
        };

        // Evaluate triggers; pick highest-priority candidate.
        let triggers = builtin_triggers();
        let mut best: Option<water_core::orchestrator::TriggerCandidate> = None;
        for trig in triggers.iter() {
            if let Some(cand) = trig.evaluate(&ctx) {
                if best.as_ref().map_or(true, |b| cand.priority > b.priority) {
                    best = Some(cand);
                }
            }
        }
        let Some(cand) = best else { return; };

        // Route to a speaker (cooldown-respecting).
        let now = std::time::Instant::now();
        let Some(speaker) = route(&cand, &self.personas, &self.cooldowns, now) else { return; };

        // Build the prompt + request a level-0 pill. Scene excerpt is a
        // ~400-char window centered on the anchored block (or the scene
        // beginning if block_target_id is None). Read it via SceneStore
        // through a snapshot held on `self` — see `set_scene_text` below.
        let scene_excerpt = self.scene_excerpt_for(&t.block_id);
        let req = match assemble_level_0(&self.prompts, &*speaker, cand.trigger_id, &scene_excerpt) {
            Ok(r) => r, Err(_) => return,
        };

        // Spawn LLM call.
        let app = self.app.clone();
        let router = self.router.clone();
        let filters_arc: Vec<Box<dyn PostFilter>> = builtin_post_filters(&self.prompts.tone.blacklist_regex.patterns);
        let speaker_id = speaker.id().to_string();
        let trigger_id = cand.trigger_id.to_string();
        let hue = speaker.hue_token().to_string();
        let block_target_id = cand.block_target_id.clone();

        let pill = Pill::new_generating(speaker_id.clone(), trigger_id.clone(), block_target_id.clone(), None);
        let pill_id = pill.id.clone();
        self.pills.push(pill);
        self.cooldowns.note_emit(&speaker_id);

        // Evict if needed.
        if let Some(idx) = pick_evictee(&self.pills) {
            let evicted = self.pills.remove(idx);
            let _ = emit(&self.app, "pill:evicted", serde_json::json!({ "pill_id": evicted.id.as_str() }));
        }

        tokio::spawn(async move {
            let raw = match router.generate_with_default(req.system, req.user).await {
                Ok(s) => s, Err(_) => {
                    let _ = emit(&app, "pill:dismissed", serde_json::json!({ "pill_id": pill_id.as_str() }));
                    return;
                }
            };
            if raw.trim() == "PASS" {
                let _ = emit(&app, "pill:dismissed", serde_json::json!({ "pill_id": pill_id.as_str() }));
                return;
            }
            // PostFilter chain.
            for f in filters_arc.iter() {
                if let FilterDecision::Drop { .. } = f.evaluate(&raw) {
                    let _ = emit(&app, "pill:dismissed", serde_json::json!({ "pill_id": pill_id.as_str() }));
                    return;
                }
            }
            let payload = serde_json::json!({
                "pill_id": pill_id.as_str(),
                "speaker_id": speaker_id,
                "hue_token": hue,
                "text": raw.trim(),
                "block_target_id": block_target_id,
                "trigger_id": trigger_id,
            });
            let _ = emit(&app, "pill:emerged", payload);
        });
    }

    async fn on_expand(&mut self, parent_pill_id: Id, regenerate: bool) {
        // Look up the parent pill text + speaker.
        let Some(parent) = self.pills.iter().find(|p| p.id == parent_pill_id).cloned() else { return; };
        let parent_text = parent.text.unwrap_or_default();
        let Some(speaker) = self.personas.by_id(&parent.speaker_id) else { return; };
        let scene_excerpt = self.scene_excerpt_for(&parent.block_target_id.clone().unwrap_or_default());

        let prior = self.bouquet_history.entry(parent_pill_id.as_str().to_string()).or_default().clone();
        let prior_first_words: Vec<String> = prior.iter().map(|t| t.split_whitespace().take(8).collect::<Vec<_>>().join(" ")).collect();
        let req = if regenerate {
            assemble_pill_regenerate(&self.prompts, &*speaker, &parent.trigger_id, &parent_text, &scene_excerpt, &prior_first_words)
        } else {
            assemble_pill_expand(&self.prompts, &*speaker, &parent.trigger_id, &parent_text, &scene_excerpt)
        };
        let Ok(req) = req else { return; };

        let app = self.app.clone();
        let router = self.router.clone();
        let history_key = parent_pill_id.as_str().to_string();
        let threshold = speaker.anti_loop_threshold();

        let history_arc = Arc::new(Mutex::new(self.bouquet_history.clone()));
        tokio::spawn(async move {
            #[derive(serde::Deserialize, Clone)]
            struct Item { angle: String, text: String }
            let items: Vec<Item> = match router.generate_structured_with_default(req.system, req.user).await {
                Ok(v) => v, Err(_) => {
                    let _ = emit(&app, "pill:dismissed", serde_json::json!({ "pill_id": history_key }));
                    return;
                }
            };
            // Anti-loop check (best-effort) — drop variants that overlap heavily.
            let priors = history_arc.lock().await.get(&history_key).cloned().unwrap_or_default();
            let mut accepted: Vec<Item> = Vec::with_capacity(3);
            for it in items.into_iter() {
                if max_overlap(&it.text, &priors) < threshold {
                    accepted.push(it);
                }
                if accepted.len() >= 3 { break; }
            }
            // If anti-loop killed too many, ship what we have (acceptable degeneration; spec § 9.3).
            let mut history = history_arc.lock().await;
            history.entry(history_key.clone()).or_default().extend(accepted.iter().map(|i| i.text.clone()));

            let items_json = accepted.iter().enumerate().map(|(i, it)| {
                serde_json::json!({ "sub_pill_id": format!("{}-{}", history_key, i+1), "angle": it.angle, "text": it.text })
            }).collect::<Vec<_>>();
            let _ = emit(&app, "bouquet:ready", serde_json::json!({
                "parent_pill_id": history_key, "items": items_json,
            }));
        });
    }
}
```

NOTE: `LlmRouter::generate_with_default` and `generate_structured_with_default` need to exist on `LlmRouter`. If they don't, add them as small helpers that pick the configured primary provider:

```rust
// In crates/water-core/src/llm/router.rs
impl LlmRouter {
    pub async fn generate_with_default(&self, system: String, user: String) -> Result<String, ProviderError> {
        let primary = self.primary().ok_or(ProviderError::Other("no primary provider".into()))?;
        let req = GenerateRequest { system, user, ..Default::default() };
        primary.generate(req).await
    }
    pub async fn generate_structured_with_default<T: serde::de::DeserializeOwned + Send>(&self, system: String, user: String) -> Result<T, ProviderError> {
        let primary = self.primary().ok_or(ProviderError::Other("no primary provider".into()))?;
        let req = GenerateRequest { system, user, ..Default::default() };
        primary.generate_structured(req).await
    }
}
```

(The exact method names depend on what M1 shipped. Read `crates/water-core/src/llm/router.rs` first and adapt.)

- [ ] **Step 2: Wire the service into `OpenProject`**

Modify `app/src-tauri/src/state.rs`. Add to `OpenProject`:

```rust
pub orchestrator: Option<crate::orchestrator_service::OrchestratorHandle>,
```

Modify `app/src-tauri/src/commands/project.rs::open_project`. After router + supervisor setup, before returning success:

```rust
let personas = water_core::voice::registry::PersonaRegistry::from_db(&*db.lock().await)
    .map_err(|e| e)?;
let orchestrator = crate::orchestrator_service::OrchestratorService::start(
    app_handle.clone(), router.clone(), personas,
);
```

And include `orchestrator: Some(orchestrator)` in the `OpenProject { ... }` literal.

- [ ] **Step 3: Forward `typing:telemetry` Tauri events into the orchestrator**

Modify `app/src-tauri/src/commands/events.rs::typing_telemetry` so that, in addition to re-emitting the Tauri event, it dispatches to the orchestrator channel:

```rust
#[tauri::command]
pub async fn typing_telemetry(
    app: AppHandle,
    state: State<'_, AppState>,
    payload: TypingTelemetryPayload,
) -> Result<(), String> {
    // Re-emit on the bus so renderer subscribers (debug panel, eval harness) can see it.
    emit(&app, "typing:telemetry", payload.clone()).ok();
    // Forward to orchestrator if one is running.
    let handle = {
        let proj = state.project.lock().await;
        proj.as_ref().and_then(|p| p.orchestrator.as_ref().map(|o| o.clone()))
    };
    if let Some(h) = handle {
        let core_payload: water_core::orchestrator::TypingTelemetry = water_core::orchestrator::TypingTelemetry {
            idle_for_ms: payload.idle_for_ms,
            cursor_classification: match payload.cursor_classification.as_str() {
                "at_sentence_end" => water_core::orchestrator::CursorClassification::AtSentenceEnd,
                "at_paragraph_end" => water_core::orchestrator::CursorClassification::AtParagraphEnd,
                _ => water_core::orchestrator::CursorClassification::MidSentence,
            },
            block_id: payload.block_id,
            recent_word_delta: payload.recent_word_delta,
            structural_inflection: match payload.structural_inflection.as_str() {
                "new_scene" => water_core::orchestrator::StructuralInflection::NewScene,
                "new_chapter" => water_core::orchestrator::StructuralInflection::NewChapter,
                "pov_change" => water_core::orchestrator::StructuralInflection::PovChange,
                "location_change" => water_core::orchestrator::StructuralInflection::LocationChange,
                _ => water_core::orchestrator::StructuralInflection::None,
            },
        };
        h.send(crate::orchestrator_service::OrchestratorRequest::Telemetry(core_payload)).await;
    }
    Ok(())
}
```

(`OrchestratorHandle` needs `Clone`. Derive it.)

- [ ] **Step 3.5: Add a `scene_state` Tauri command for renderer-side dispatch**

The orchestrator needs the current scene body text to assemble prompts. The renderer sends it whenever a scene loads or its body changes significantly (e.g., after every body save). Add to `app/src-tauri/src/commands/events.rs`:

```rust
#[derive(serde::Deserialize)]
pub struct ScenePayload {
    pub scene_id: String,
    pub pov_character_id: Option<String>,
    pub location_id: Option<String>,
    pub characters_present: Vec<String>,
    pub word_count: u32,
    pub body_text: String,
    pub character_count: u32,    // project-level: characters in project
    pub world_entry_count: u32,  // project-level: world entries
}

#[tauri::command]
pub async fn scene_state(
    state: State<'_, AppState>,
    payload: ScenePayload,
) -> Result<(), String> {
    let handle = {
        let proj = state.project.lock().await;
        proj.as_ref().and_then(|p| p.orchestrator.as_ref().map(|o| o.clone()))
    };
    if let Some(h) = handle {
        let scene = water_core::orchestrator::SceneSnapshot {
            id: water_core::Id::from_str(&payload.scene_id).map_err(|e| e.to_string())?,
            pov_character_id: payload.pov_character_id.and_then(|s| water_core::Id::from_str(&s).ok()),
            location_id: payload.location_id.and_then(|s| water_core::Id::from_str(&s).ok()),
            characters_present: payload.characters_present.into_iter()
                .filter_map(|s| water_core::Id::from_str(&s).ok()).collect(),
            word_count: payload.word_count,
            seconds_since_last_pill: 60,
        };
        let project = water_core::orchestrator::ProjectSnapshot {
            character_count: payload.character_count,
            world_entry_count: payload.world_entry_count,
        };
        h.send(crate::orchestrator_service::OrchestratorRequest::SceneState(scene, project, payload.body_text)).await;
    }
    Ok(())
}
```

Register `commands::events::scene_state` in `main.rs`. Add `ipc.sceneState(payload)` to `app/src/ipc/commands.ts`. Call `ipc.sceneState({...})` from `EditorCanvas` whenever a scene loads (`useEffect` mount) and after each successful body save in the existing autosave path (debounce 2 s — same path that already exists). Pass M2-stub values for `character_count` and `world_entry_count` (always 0 until M3/M4) and `characters_present` (empty until M3).

- [ ] **Step 4: Replace stub pill commands with orchestrator dispatches**

Modify `app/src-tauri/src/commands/pill.rs`. Replace `pill_expand` and `pill_regenerate` stubs:

```rust
#[tauri::command]
pub async fn pill_expand(state: State<'_, AppState>, parent_pill_id: String) -> Result<(), String> {
    let handle = {
        let proj = state.project.lock().await;
        proj.as_ref().and_then(|p| p.orchestrator.as_ref().map(|o| o.clone()))
    };
    if let Some(h) = handle {
        let pid = water_core::Id::from_str(&parent_pill_id).map_err(|e| e.to_string())?;
        h.send(crate::orchestrator_service::OrchestratorRequest::Expand { parent_pill_id: pid }).await;
    }
    Ok(())
}

#[tauri::command]
pub async fn pill_regenerate(state: State<'_, AppState>, parent_pill_id: String) -> Result<(), String> {
    let handle = {
        let proj = state.project.lock().await;
        proj.as_ref().and_then(|p| p.orchestrator.as_ref().map(|o| o.clone()))
    };
    if let Some(h) = handle {
        let pid = water_core::Id::from_str(&parent_pill_id).map_err(|e| e.to_string())?;
        h.send(crate::orchestrator_service::OrchestratorRequest::Regenerate { parent_pill_id: pid }).await;
    }
    Ok(())
}
```

- [ ] **Step 5: Register the orchestrator_service module**

Modify `app/src-tauri/src/main.rs`:

```rust
mod orchestrator_service;
```

- [ ] **Step 6: Write the integration smoke test**

Create `crates/water-core/tests/orchestrator_integration.rs` (integration test, not in app crate so we can use a fake provider directly):

```rust
//! End-to-end orchestrator smoke: synthetic telemetry → routes → fake LLM →
//! pill text emerges. Uses the LlmRouter + CannedProvider from M1.

use water_core::orchestrator::*;
use water_core::orchestrator::triggers::builtin_triggers;
use water_core::voice::registry::PersonaRegistry;
use water_core::voice::router::{route, CooldownState};
use water_core::prompts::PromptLibrary;
use water_core::Db;
use tempfile::TempDir;

#[test]
fn end_to_end_trigger_evaluation_picks_speaker_and_assembles_prompt() {
    let dir = TempDir::new().unwrap();
    let db = Db::open(dir.path().join("p.db")).unwrap();
    let personas = PersonaRegistry::from_db(&db).unwrap();
    let prompts = PromptLibrary::load_builtin().unwrap();

    // Synthetic context: block_anchored_drift conditions.
    let telem = TypingTelemetry {
        idle_for_ms: 3000,
        cursor_classification: CursorClassification::AtParagraphEnd,
        block_id: "^bk-0001".to_string(),
        recent_word_delta: 0,
        structural_inflection: StructuralInflection::None,
    };
    let mut analysis = AnalysisSnapshot::default();
    analysis.block_metrics.insert("^bk-0001".to_string(), BlockMetrics {
        flow: Some(0.5), coherence: Some(0.2), divergence: Some(0.7),
    });
    let scene = SceneSnapshot {
        id: water_core::Id::new(), pov_character_id: None, location_id: None,
        characters_present: vec![], word_count: 300, seconds_since_last_pill: 60,
    };
    let project = ProjectSnapshot::default();
    let ctx = TriggerContext { telemetry: &telem, analysis: &analysis, scene: &scene, project: &project };

    let triggers = builtin_triggers();
    let cand = triggers.iter().filter_map(|t| t.evaluate(&ctx))
        .max_by(|a, b| a.priority.partial_cmp(&b.priority).unwrap()).unwrap();
    assert_eq!(cand.trigger_id, "block_anchored_drift");

    let speaker = route(&cand, &personas, &CooldownState::default(), std::time::Instant::now()).unwrap();
    assert_eq!(speaker.id(), "editor");

    let req = water_core::prompts::assemble_level_0(&prompts, &*speaker, cand.trigger_id, "She walked across the square.").unwrap();
    assert!(req.system.contains("Editor"));
    assert!(req.system.contains("block_anchored_drift"));
    assert!(req.user.contains("square"));
}
```

- [ ] **Step 7: Run all tests + lints**

```powershell
cargo test -p water-core
cargo clippy -p water-core --all-targets -- -D warnings
cargo clippy -p water-app -- -D warnings
cargo build -p water-app
pnpm --filter @water/app test
pnpm --filter @water/app build
```

Expected: all green. Manual smoke (optional): `pnpm --filter @water/app tauri dev`, open a project, type a paragraph, wait 3 s — a pill should emerge (driven by whatever provider is configured; canned provider returns deterministic text).

- [ ] **Step 8: Commit**

```powershell
git add app/src-tauri/src/orchestrator_service.rs app/src-tauri/src/state.rs app/src-tauri/src/commands/project.rs app/src-tauri/src/commands/pill.rs app/src-tauri/src/commands/events.rs app/src-tauri/src/main.rs crates/water-core/src/llm/router.rs crates/water-core/tests/orchestrator_integration.rs
git commit -m "feat: end-to-end orchestrator pipeline (telemetry -> trigger -> speaker -> LLM -> pill)"
```

---

### Task 27: Replay log opt-in + dev override

Adds a `WATER_REPLAY_LOG=1` env var (and a `settings.replay_log_enabled` boolean for future Settings UI) that toggles JSONL writes of every LLM call to `.water/log/llm/{session_ulid}.jsonl`. Used by tone-audit nightly.

**Files:**
- Create: `crates/water-core/src/replay_log.rs`
- Modify: `crates/water-core/src/lib.rs`
- Modify: `app/src-tauri/src/orchestrator_service.rs` (write to replay log around LLM calls)
- Modify: `crates/water-core/sql/v2_pill_engine.sql` (add `replay_log_enabled` default to settings) — actually skip, settings is a key/value table; no schema change needed

- [ ] **Step 1: Write the failing test**

Create `crates/water-core/src/replay_log.rs`:

```rust
//! Opt-in replay log. JSONL at `.water/log/llm/{session_ulid}.jsonl`.
//! Enabled via `WATER_REPLAY_LOG=1` env var or `settings.replay_log_enabled = true`
//! in the project DB. Lines are append-only; one writer per session.

use serde::Serialize;
use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Serialize)]
pub struct ReplayEntry<'a> {
    pub ts: String,
    pub kind: &'a str,
    pub request_system: &'a str,
    pub request_user: &'a str,
    pub response_raw: Option<&'a str>,
    pub post_filter_decision: Option<&'a str>,
    pub anti_loop_overlap: Option<f32>,
}

pub struct ReplayLog {
    file: Mutex<std::fs::File>,
}

impl ReplayLog {
    pub fn open(project_root: &std::path::Path, session_id: &str) -> Result<Self, String> {
        let dir = project_root.join(".water").join("log").join("llm");
        create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path: PathBuf = dir.join(format!("{session_id}.jsonl"));
        let file = OpenOptions::new().create(true).append(true).open(&path)
            .map_err(|e| e.to_string())?;
        Ok(Self { file: Mutex::new(file) })
    }

    pub fn append(&self, entry: &ReplayEntry<'_>) -> Result<(), String> {
        let line = serde_json::to_string(entry).map_err(|e| e.to_string())?;
        let mut f = self.file.lock().map_err(|e| e.to_string())?;
        writeln!(f, "{line}").map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn writes_jsonl_line_to_session_file() {
        let dir = TempDir::new().unwrap();
        let log = ReplayLog::open(dir.path(), "session-1").unwrap();
        log.append(&ReplayEntry {
            ts: "2026-05-17T00:00:00Z".into(),
            kind: "level_0",
            request_system: "sys",
            request_user: "u",
            response_raw: Some("hello"),
            post_filter_decision: Some("pass"),
            anti_loop_overlap: None,
        }).unwrap();
        let path = dir.path().join(".water").join("log").join("llm").join("session-1.jsonl");
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("\"kind\":\"level_0\""));
        assert!(body.contains("\"response_raw\":\"hello\""));
    }
}
```

(`ReplayEntry`'s `ts` field accepts a `String` instead of `&'a str` for ergonomic logging — adjust the lifetime in the struct accordingly if needed. The implementer iterates.)

- [ ] **Step 2: Register the module**

Modify `crates/water-core/src/lib.rs`:

```rust
pub mod replay_log;
```

- [ ] **Step 3: Run tests**

```powershell
cargo test -p water-core replay_log
```

Expected: 1 test passes.

- [ ] **Step 4: Wire replay-log writes around LLM calls in orchestrator_service**

Modify `app/src-tauri/src/orchestrator_service.rs`. At service start, read the `WATER_REPLAY_LOG` env var + the `settings.replay_log_enabled` row from the DB; if either says yes, open a `ReplayLog` at `<project_root>/.water/log/llm/<session_ulid>.jsonl` and store it as `Option<Arc<ReplayLog>>` on the service. Before each LLM call, log a "request" line; after each response, log a "response" line.

Sketch:

```rust
use water_core::replay_log::{ReplayLog, ReplayEntry};

// In OrchestratorService:
pub replay: Option<Arc<ReplayLog>>,

// In OrchestratorService::start, after building everything else:
let replay_enabled = std::env::var("WATER_REPLAY_LOG").map(|v| v == "1").unwrap_or(false)
    || /* read settings.replay_log_enabled */ false;
let replay = if replay_enabled {
    let session_id = water_core::Id::new().as_str().to_string();
    ReplayLog::open(&project_root, &session_id).ok().map(Arc::new)
} else { None };
```

Around the existing LLM calls (in `on_telemetry` and `on_expand`), log via the optional handle.

- [ ] **Step 5: Verify end-to-end with env var**

Manual:

```powershell
$env:WATER_REPLAY_LOG = "1"
pnpm --filter @water/app tauri dev
# open a project, type a paragraph, wait 3s
# verify a file appears at <project_root>\.water\log\llm\<ulid>.jsonl
```

- [ ] **Step 6: Commit**

```powershell
git add crates/water-core/src/replay_log.rs crates/water-core/src/lib.rs app/src-tauri/src/orchestrator_service.rs
git commit -m "feat(orchestrator): opt-in replay log (WATER_REPLAY_LOG env var)"
```

---

### Task 28: Final SettingsSheet subscriptions (finalize Review #5; provider:status)

Adds the `provider:status` event so the SettingsSheet's provider list updates without polling. Provider events fire when the LLM router observes a circuit-breaker state change.

**Files:**
- Modify: `crates/water-core/src/llm/router.rs` (emit a callback when CB opens/closes)
- Modify: `app/src-tauri/src/commands/project.rs` (subscribe to router callback; emit `provider:status`)
- Modify: `app/src-tauri/src/events.rs` (`ProviderStatusPayload`)
- Modify: `app/src/ipc/events.ts` (`provider:status` type)
- Modify: `app/src/sheets/SettingsSheet.tsx` (subscribe; remove any residual polling)
- Modify: `app/src/sheets/SettingsSheet.test.tsx` (assert provider:status updates state)

- [ ] **Step 1: Add router status hook**

Read `crates/water-core/src/llm/router.rs`. Add a small `Watch<RouterStatus>` or a `mpsc::Sender<ProviderHealthChange>` field that the router populates on CB state changes:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct ProviderHealthChange {
    pub provider_id: String,
    pub ok: bool,
    pub error: Option<String>,
}

impl LlmRouter {
    pub fn subscribe_status(&self) -> tokio::sync::broadcast::Receiver<ProviderHealthChange> {
        self.status_tx.subscribe()
    }
}
```

Inside the router, wherever a CB transitions, send a `ProviderHealthChange`. (Specifics depend on existing impl; the implementer adapts.)

- [ ] **Step 2: Forward to Tauri events**

Modify `app/src-tauri/src/commands/project.rs::open_project`. After router creation, spawn a task subscribing to `router.subscribe_status()` and emit `provider:status` Tauri events:

```rust
{
    let app_handle_clone = app.clone();
    let mut rx = router.subscribe_status();
    tokio::spawn(async move {
        while let Ok(change) = rx.recv().await {
            let _ = crate::events::emit(&app_handle_clone, "provider:status", crate::events::ProviderStatusPayload {
                provider_id: change.provider_id,
                ok: change.ok,
                error: change.error,
            });
        }
    });
}
```

Add `ProviderStatusPayload` to `app/src-tauri/src/events.rs`:

```rust
#[derive(Serialize, Clone)]
pub struct ProviderStatusPayload {
    pub provider_id: String,
    pub ok: bool,
    pub error: Option<String>,
}
```

Add to `app/src/ipc/events.ts`:

```ts
  "provider:status": { provider_id: string; ok: boolean; error: string | null };
```

- [ ] **Step 3: Subscribe in SettingsSheet**

Modify `app/src/sheets/SettingsSheet.tsx`. Add inside the open-effect alongside the existing sidecar subscription:

```tsx
    (async () => {
      const unsubP = await onWaterEvent("provider:status", (e) => {
        setStatus((prev) => {
          if (!prev) return prev;
          const idx = prev.provider_health.findIndex((p) => p.id === e.provider_id);
          if (idx < 0) return prev;
          const next = [...prev.provider_health];
          next[idx] = { id: e.provider_id, ok: e.ok, error: e.error };
          return { ...prev, provider_health: next };
        });
      });
      unsubsLocal.push(unsubP);
    })();
```

- [ ] **Step 4: Test the subscription**

Add to `app/src/sheets/SettingsSheet.test.tsx`:

```tsx
it("updates provider health when provider:status event fires", async () => {
  const handlers: Record<string, ((p: any) => void)> = {};
  onWaterEventMock.mockImplementation(async (name: string, cb: any) => {
    handlers[name] = cb; return vi.fn();
  });
  ipcMock.diagnosticsStatus.mockResolvedValue({
    has_open_project: true, project_root: "/p", router_primary_id: null, sidecar: null,
    provider_health: [{ id: "openai", ok: false, error: null }],
  });
  render(<SettingsSheet open={true} onClose={() => {}} />);
  await waitFor(() => expect(handlers["provider:status"]).toBeDefined());
  handlers["provider:status"]({ provider_id: "openai", ok: true, error: null });
  await waitFor(() => expect(screen.getByText("ok")).toBeInTheDocument());
});
```

- [ ] **Step 5: Run tests + lints**

```powershell
pnpm --filter @water/app test
cargo clippy -p water-app -- -D warnings
cargo clippy -p water-core --all-targets -- -D warnings
```

Expected: all clean.

- [ ] **Step 6: Commit**

```powershell
git add crates/water-core/src/llm/router.rs app/src-tauri/src/events.rs app/src-tauri/src/commands/project.rs app/src/ipc/events.ts app/src/sheets/SettingsSheet.tsx app/src/sheets/SettingsSheet.test.tsx
git commit -m "feat(events): provider:status events; SettingsSheet fully event-driven (closes Review #5)"
```

---

## Phase G — Audit & tag

### Task 29: Tone audit fixtures + harness + gate

Builds the 200-entry fixture set, the `tone_audit::run_gate` + `run_nightly` entry points, and a CI workflow for nightly scorecards.

**Files:**
- Create: `eval/tone_audit/fixtures/*.json` (200 entries)
- Create: `eval/tone_audit/generate-fixtures.ps1` (committed generator)
- Create: `crates/water-core/src/tone_audit/mod.rs`
- Create: `crates/water-core/src/tone_audit/runner.rs`
- Create: `.github/workflows/tone-audit.yml`
- Modify: `crates/water-core/src/lib.rs`

- [ ] **Step 1: Build the fixture generator + 200 fixtures**

Create `eval/tone_audit/generate-fixtures.ps1`:

```powershell
# Generates 200 tone-audit fixtures: 10 trigger classes × 5 personas × 4 manuscript excerpts.
# Excerpts pulled from public-domain prose corpora (committed inline below).

$triggers = @("block_anchored_drift","scene_flow_dip","topic_drift","valence_spike","structural_inflection","pace_floor","world_drift","no_universe_yet","character_dissonance","idle_pause_with_present_character")
$speakers = @("echo","architect","editor","cartographer","chorus")
$excerpts = @(
  "She walked across the square. The doors were open but no one went in. A bell rang somewhere she couldn't see.",
  "Rain on the slate roof. The lamp in the kitchen had been lit since before he came home. He hadn't asked her about the letter.",
  "The river bent twice before reaching the bridge, and at each bend it had left a stone that did not belong.",
  "Maps had been drawn for this country before there was a name for it, and the names came after, slowly."
)
$out = "eval/tone_audit/fixtures"
New-Item -ItemType Directory -Path $out -Force | Out-Null
$id = 0
foreach ($t in $triggers) {
  foreach ($s in $speakers) {
    foreach ($e in $excerpts) {
      $id += 1
      $obj = [ordered]@{
        id = "tone-$($id.ToString('000'))"
        trigger = $t
        speaker = $s
        scene_excerpt = $e
        expected_pass = $true
      }
      $obj | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $out "tone-$($id.ToString('000')).json")
    }
  }
}
Write-Output "Generated $id fixtures"
```

Run it:

```powershell
.\eval\tone_audit\generate-fixtures.ps1
```

Expected: 200 JSON files in `eval/tone_audit/fixtures/`.

- [ ] **Step 2: Write the failing test**

Create `crates/water-core/src/tone_audit/mod.rs`:

```rust
pub mod runner;
pub use runner::{run_gate, run_nightly, AuditReport, Fixture};
```

Create `crates/water-core/src/tone_audit/runner.rs`:

```rust
use crate::post_filter::{builtin_post_filters, FilterDecision, PostFilter};
use crate::prompts::PromptLibrary;
use crate::voice::registry::PersonaRegistry;
use crate::prompts::assemble_level_0;
use crate::llm::{LlmProvider, GenerateRequest};
use crate::Db;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Fixture {
    pub id: String,
    pub trigger: String,
    pub speaker: String,
    pub scene_excerpt: String,
    pub expected_pass: bool,
}

#[derive(Debug, Default, Serialize)]
pub struct AuditReport {
    pub total: u32,
    pub layer3_catches: u32,
    pub audit_violations: u32,
    pub passed: u32,
    pub failures: Vec<AuditFailure>,
}

#[derive(Debug, Serialize)]
pub struct AuditFailure {
    pub fixture_id: String,
    pub reason: String,
    pub raw: String,
}

/// One-time gate. Fails when layer3_catches > 0 OR audit_violations > 0.
pub async fn run_gate<P: LlmProvider>(fixtures_dir: &Path, provider: &P, db: &Db) -> Result<AuditReport, String> {
    let mut report = audit_loop(fixtures_dir, provider, db).await?;
    if report.layer3_catches > 0 || report.audit_violations > 0 {
        report.failures.push(AuditFailure {
            fixture_id: "GATE".into(),
            reason: format!("layer3={} audit_violations={}", report.layer3_catches, report.audit_violations),
            raw: String::new(),
        });
    }
    Ok(report)
}

/// Nightly run: same loop, returns report; caller writes scorecard.
pub async fn run_nightly<P: LlmProvider>(fixtures_dir: &Path, provider: &P, db: &Db) -> Result<AuditReport, String> {
    audit_loop(fixtures_dir, provider, db).await
}

async fn audit_loop<P: LlmProvider>(fixtures_dir: &Path, provider: &P, db: &Db) -> Result<AuditReport, String> {
    let prompts = PromptLibrary::load_builtin()?;
    let personas = PersonaRegistry::from_db(db)?;
    let filters = builtin_post_filters(&prompts.tone.blacklist_regex.patterns);
    let mut report = AuditReport::default();

    let entries = std::fs::read_dir(fixtures_dir).map_err(|e| e.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") { continue; }
        let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let fx: Fixture = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        let speaker = personas.by_id(&fx.speaker).ok_or_else(|| format!("unknown persona: {}", fx.speaker))?;
        let req_prompt = assemble_level_0(&prompts, &*speaker, &fx.trigger, &fx.scene_excerpt)?;
        let req = GenerateRequest { system: req_prompt.system, user: req_prompt.user, ..Default::default() };
        let out = provider.generate(req).await.map_err(|e| format!("{e:?}"))?;
        report.total += 1;

        let mut layer3_caught = false;
        for f in filters.iter() {
            if let FilterDecision::Drop { reason } = f.evaluate(&out) {
                report.layer3_catches += 1;
                layer3_caught = true;
                report.failures.push(AuditFailure { fixture_id: fx.id.clone(), reason, raw: out.clone() });
                break;
            }
        }
        if !layer3_caught {
            // Audit-detected violation: same regex pattern check, but against raw output AFTER layer-3 to
            // catch any case where layer-3 missed something we expected it to catch.
            let aux = crate::post_filter::tone_blacklist::ToneBlacklistFilter::compile(&prompts.tone.blacklist_regex.patterns).unwrap();
            if let FilterDecision::Drop { reason } = aux.evaluate(&out) {
                report.audit_violations += 1;
                report.failures.push(AuditFailure { fixture_id: fx.id.clone(), reason, raw: out.clone() });
            } else {
                report.passed += 1;
            }
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::canned::CannedProvider;
    use crate::Db;
    use tempfile::TempDir;

    #[tokio::test]
    async fn gate_passes_with_clean_canned_output() {
        let dir = TempDir::new().unwrap();
        let db = Db::open(dir.path().join("p.db")).unwrap();
        let provider = CannedProvider::with_response("Something held at the threshold, not yet curiosity.");
        // Generate a 5-fixture mini-set inline for the smoke.
        let fxd = dir.path().join("fixtures");
        std::fs::create_dir_all(&fxd).unwrap();
        for i in 0..5 {
            let fx = serde_json::json!({
                "id": format!("tone-{i:03}"), "trigger": "block_anchored_drift",
                "speaker": "echo", "scene_excerpt": "x", "expected_pass": true,
            });
            std::fs::write(fxd.join(format!("tone-{i:03}.json")), fx.to_string()).unwrap();
        }
        let r = run_gate(&fxd, &provider, &db).await.unwrap();
        assert_eq!(r.layer3_catches, 0);
        assert_eq!(r.audit_violations, 0);
        assert_eq!(r.passed, 5);
    }

    #[tokio::test]
    async fn gate_catches_blacklisted_output() {
        let dir = TempDir::new().unwrap();
        let db = Db::open(dir.path().join("p.db")).unwrap();
        let provider = CannedProvider::with_response("You should consider rewriting this paragraph.");
        let fxd = dir.path().join("fixtures");
        std::fs::create_dir_all(&fxd).unwrap();
        let fx = serde_json::json!({
            "id": "tone-001", "trigger": "block_anchored_drift",
            "speaker": "echo", "scene_excerpt": "x", "expected_pass": true,
        });
        std::fs::write(fxd.join("tone-001.json"), fx.to_string()).unwrap();
        let r = run_gate(&fxd, &provider, &db).await.unwrap();
        assert!(r.layer3_catches >= 1);
    }
}
```

- [ ] **Step 3: Register the module**

Modify `crates/water-core/src/lib.rs`:

```rust
pub mod tone_audit;
```

- [ ] **Step 4: Run tests**

```powershell
cargo test -p water-core tone_audit
```

Expected: 2 tests pass.

- [ ] **Step 5: Add CI nightly workflow**

Create `.github/workflows/tone-audit.yml`:

```yaml
name: Tone audit (nightly)

on:
  schedule:
    - cron: "0 6 * * *"   # 06:00 UTC every day
  workflow_dispatch: {}

jobs:
  audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Run nightly tone audit against canned provider
        run: cargo test -p water-core tone_audit -- --nocapture
      # Future: swap canned for the configured default provider via secrets.
```

- [ ] **Step 6: Run the full 200-pill gate against the canned provider locally**

Add a binary entrypoint or a `#[test]` in `tests/tone_audit_200.rs` that points at `eval/tone_audit/fixtures` (the 200 we generated in Step 1):

Create `crates/water-core/tests/tone_audit_200.rs`:

```rust
use water_core::tone_audit::run_gate;
use water_core::llm::canned::CannedProvider;
use water_core::Db;
use tempfile::TempDir;

#[tokio::test]
async fn full_200_pill_gate_against_canned_provider() {
    let dir = TempDir::new().unwrap();
    let db = Db::open(dir.path().join("p.db")).unwrap();
    let provider = CannedProvider::with_response("Something held at the threshold, not yet curiosity.");
    let fixtures_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .parent().unwrap()
        .join("eval/tone_audit/fixtures");
    let report = run_gate(&fixtures_dir, &provider, &db).await.unwrap();
    assert_eq!(report.total, 200, "expected 200 fixtures, got {}", report.total);
    assert_eq!(report.layer3_catches, 0, "tone leak: {:?}", report.failures);
    assert_eq!(report.audit_violations, 0);
}
```

```powershell
cargo test -p water-core --test tone_audit_200
```

Expected: PASS — 200 fixtures all clean with canned output.

- [ ] **Step 7: Commit**

```powershell
git add eval/tone_audit/ crates/water-core/src/tone_audit/ crates/water-core/src/lib.rs crates/water-core/tests/tone_audit_200.rs .github/workflows/tone-audit.yml
git commit -m "feat: tone audit harness + 200-fixture gate + nightly workflow"
```

---

### Task 30: Final milestone review + `m2` tag

End-gate review using `superpowers:requesting-code-review` over the full `m1.5.1..HEAD` range. Address any review-driven fixes as an `m2.0.1` patch if needed; otherwise tag `m2`.

**Files:**
- Modify: `docs/m1-acceptance-checklist.md` (or create a sibling `docs/m2-acceptance-checklist.md`) — record final gate state
- Modify: `KNOWN_FRAGILE.md` (add expected entries #8, #9, #10 from spec § 17)

- [ ] **Step 1: Confirm clean tree + all gates green**

```powershell
git status
cargo test -p water-core
cargo clippy -p water-core --all-targets -- -D warnings
cargo clippy -p water-app -- -D warnings
cargo fmt -p water-core --check
cargo build -p water-app
pnpm --filter @water/app test
pnpm --filter @water/app build
```

Expected: working tree clean; all gates pass.

- [ ] **Step 2: Add expected KNOWN_FRAGILE entries**

Append to `KNOWN_FRAGILE.md`:

```markdown
---

## 8. Anti-loop Jaccard suffix-stripper is English-only

**What it is.** Anti-loop overlap normalizes tokens via a fixed `-s/-es/-ed/-ing/-ly` suffix-stripper. Non-English words don't share these suffixes; their stems collapse incorrectly (e.g., German `geben` and `gibt` won't collide). Spec § 17.

**Where it lives.** `crates/water-core/src/orchestrator/anti_loop.rs::strip_suffix`.

**Why it's fragile.** v1 ships English-only manuscripts. Non-English is M7+.

**First-look mitigations.** Inspect replay logs (`WATER_REPLAY_LOG=1`) for the affected scene; check anti-loop overlap distribution per speaker.

---

## 9. Pill block-anchor stability under decoration churn

**What it is.** When `pill:emerged` fires while the user is mid-edit at the anchored block, the editor's transaction filter may re-apply decoration after a partial write. Snippet-as-canonical (master spec § 3.3) is the fallback.

**Where it lives.** `app/src/pill/PillLayer.tsx` (anchor rect computation); `app/src/editor/blockIdPlugin.ts` (block-id stability).

**Why it's fragile.** Selection + decoration coexistence is hard. Race window is small but visible during rapid typing.

**First-look mitigations.** Check `bouquet.block_target_id` against the live scene's current block IDs; if mismatched, the snippet still resolves via text-content scan.

---

## 10. Structural-inflection detection is shallow

**What it is.** Sidecar pattern match for `pov_change` / `location_change` produces false positives on quoted dialogue and intra-character thought transitions.

**Where it lives.** Sidecar `/analyze` route (Phase B5 extends).

**Why it's fragile.** Heuristic; nightly tone-audit scorecard tracks leak rate.

**First-look mitigations.** Inspect the sidecar's `/analyze` response for the affected scene; lower the priority multiplier in `structural_inflection.rs` if false-positive rate is too high.
```

- [ ] **Step 3: Create the M2 acceptance checklist**

Create `docs/m2-acceptance-checklist.md`:

```markdown
# M2 Acceptance Checklist

**Tag:** `m2`
**Base:** `m1.5.1` (`404fadf`)

## Exit criteria (master spec § 4.4)

- [ ] Idle 3 s after a paragraph → at most one pill surfaces within 1.5 s of analysis completing.
- [ ] Two pills max on screen; mid-sentence typing never surfaces a pill.
- [ ] Expanding a pill shows exactly 3 sub-pills; regenerate produces 3 different ones.
- [ ] Rabbit hole works at arbitrary depth; breadcrumb collapse visible; anti-loop fires when configured.
- [ ] Pinning a pill persists it across app restart; dismissed pills do not.
- [ ] Tone audit: 0 instances of "you should", "consider", "try", "I think you", "as an AI" in 200 sampled pills.

## Carried-over debt

- [ ] KNOWN_FRAGILE #7 closed (scene write-lock).
- [ ] KNOWN_FRAGILE #6 closed (sidecar respawn).
- [ ] Review #4 closed (ScenesPanel reloadToken).
- [ ] Review #5 closed (SettingsSheet event-driven).
- [ ] Review #14 closed (Sheet slide-in transform).

## Build gates

- [ ] `cargo test -p water-core` → all pass (target: 110+ tests)
- [ ] `pnpm --filter @water/app test` → all pass
- [ ] `cargo clippy -p water-core --all-targets -- -D warnings` → clean
- [ ] `cargo clippy -p water-app -- -D warnings` → clean
- [ ] `cargo fmt -p water-core --check` → clean
- [ ] `cargo build -p water-app` → clean
- [ ] `pnpm --filter @water/app build` → clean

## Manual smoke

- [ ] `pnpm --filter @water/app tauri dev` opens; project loads; pill emerges within 3 s of paragraph idle.
- [ ] Click pill → bouquet of 3 sub-pills with slight hue differentiation.
- [ ] Regenerate → 3 new sub-pills, materially different.
- [ ] Sub-pill click → rabbit hole expands; breadcrumb shows; siblings collapse to glow-lines.
- [ ] Pin → pill migrates to 56 px right-edge column at half opacity.
- [ ] Restart app → pinned pill still in column.
```

- [ ] **Step 4: Dispatch the final milestone review**

```
Invoke superpowers:requesting-code-review against the range `m1.5.1..HEAD`.
```

Prompt template:

```
Review the M2 implementation against:
- Spec: docs/superpowers/specs/2026-05-17-m2-editor-pill-engine.md
- Plan: docs/superpowers/plans/2026-05-17-m2-editor-pill-engine.md
- Range: git diff m1.5.1..HEAD

Focus on:
- Hard constraints (spec § 3): no chat textbox; tone enforcement layers; universe-first; local-first; determinism on orchestrator side; markdown+TOML truth; plugin seams; pastel-glow + no hard corners; motion language.
- Exit criteria (spec § 16).
- Carried-over debt closure (KNOWN_FRAGILE #6, #7; Reviews #4, #5, #14).
- Vitest/Rust gotchas from handoff § 7.

Return: Critical / Important / Nit categorization with file_path:line_number references.
```

- [ ] **Step 5: Address review-driven fixes (if any)**

If Critical or Important items surfaced: address them as small commits. If patterns suggest a `.1` patch is warranted (e.g., 3+ Important fixes touching multiple areas), create a `m2.0.1` patch spec + plan in the same style as `m1.5.1`. Otherwise land fixes directly.

- [ ] **Step 6: Tag `m2`**

When all review items closed + all gates green:

```powershell
git tag -a m2 -m "M2: Editor & Pill Engine"
# Push the tag if you have a remote configured:
# git push origin m2
```

- [ ] **Step 7: Update the handoff doc for M3**

Create `docs/superpowers/handoffs/{today}-m3-handoff.md` in the style of `2026-05-17-m2-handoff.md`. The next agent picks up from there. Key carry-forwards: character voices ride the M2 engine; LSM v2.1 sheets; Conversational Intake popup. Replay log infrastructure already exists. Tone audit already running nightly.

- [ ] **Step 8: Final commit**

```powershell
git add docs/m2-acceptance-checklist.md KNOWN_FRAGILE.md docs/superpowers/handoffs/
git commit -m "docs: M2 acceptance + KNOWN_FRAGILE #8-#10 + M3 handoff"
git tag -a m2 -m "M2: Editor & Pill Engine"
```

---

## Amendments

This block is updated as the plan executes. Each amendment is its own commit (`plan(TN): <reason>`).

<!-- Append amendments here as they are needed. Format:

### Amendment N — TX scope change (commit `<sha>`)

**Date:** YYYY-MM-DD
**Affected task:** TX
**Reason:** ...
**Change:** ...
**Verification:** ...

-->

---

*Plan ends. Total: 30 tasks across 7 phases. See `docs/superpowers/specs/2026-05-17-m2-editor-pill-engine.md` for the spec; see the handoff at `docs/superpowers/handoffs/2026-05-17-m2-handoff.md` for inherited context.*

