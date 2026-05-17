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
