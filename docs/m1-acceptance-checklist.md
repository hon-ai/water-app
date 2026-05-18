# M1 / M1.1 Acceptance Checklist (Internal)

Run on a clean macOS or Windows machine with `uv` and `pnpm` installed.

## Build
- [ ] `pnpm install` — no errors.
- [ ] `cargo build -p water-app --release` — succeeds (first build may be slow).
- [ ] `pnpm --filter @water/app build` — succeeds.

## Test
- [ ] `pnpm test:core` — all `water-core` tests pass (75+ at M1.1).
- [ ] `pnpm test:app` — all renderer tests pass (7+ at M1.1).
- [ ] `cargo test -p water-core --test m1_exit_criteria` — 6 passed, 1 ignored.
- [ ] `cargo test -p water-core --test m1_exit_criteria -- --ignored` — sidecar boot test passes (requires `uv` on PATH).

## Run the app
- [ ] `pnpm dev` launches the Tauri app and opens a window titled "Water".
- [ ] The header shows `scenes`, `diagnostics`, theme toggle.
- [ ] Light/dark/auto buttons change `data-theme` (verify via devtools).

## Project lifecycle
- [ ] Click `create` with parent `.` (or your Desktop path) and name `Acceptance Test`. A folder `Acceptance-Test.water/` is created containing:
  - `water.toml` (open in any editor — human-readable).
  - `project.db`
  - `manuscript/scenes/` (empty)
  - `manuscript/chapters.toml`
  - `characters/`, `world/`, `snapshots/`, `.water/cache/`
- [ ] Click `+ new scene`, type a few paragraphs into the editor. Wait 2 seconds. The "saved at …" indicator updates exactly ONCE per pause-in-typing.
- [ ] Re-open the scene later by clicking elsewhere and back. The "saved at …" indicator does NOT update unless you actually edit the body (M1.1 A3 fix).
- [ ] Open `manuscript/scenes/<ulid>.md` in any text editor — the body matches what you typed, with `^bk-XXXX` tokens at the end of each paragraph.
- [ ] Click `close`. The scene list disappears.
- [ ] Click `open` and paste the path to `Acceptance-Test.water/`. Scene list reappears with your scene; opening it shows the previously-typed text.

## Rebuild from truth
- [ ] Close the app.
- [ ] Delete `Acceptance-Test.water/project.db`.
- [ ] Relaunch the app. Open the same project root. The scene list still shows your scenes.
- [ ] (M1.1) Manually edit a scene `.md` file to set `pov_character_id: "01H8X4..."` (an ID matching a character `.toml` in `characters/`). Close, delete `project.db`, reopen. The scene appears AND the diagnostics page shows the character row count > 0 (M1.1 A2 fix — characters now load before scenes).

## Provider round-trip
- [ ] Go to `diagnostics`. Verify `project.open: yes` and `project.root: <path>` show in the project section.
- [ ] Select `canned` from the provider dropdown. Click `test round-trip`. Three placeholder variants appear.
- [ ] (M1.1) After the test succeeds, the `router` section shows `primary: canned` and the `provider_health` list contains `canned: ok` within 3 seconds (M1.1 A1 + B4 fixes).
- [ ] Drop your own dev keys at `~/.water/dev-keys.toml`:
  ```toml
  anthropic = "sk-ant-..."
  openai = "sk-..."
  ```
- [ ] Select `anthropic`. Click `test round-trip`. Three real variants appear. The `router` section updates to `primary: anthropic`.

## Snapshot timeline (auto-fired)
- [ ] (M1.1) After typing a scene, click `close`. Inspect `Acceptance-Test.water/snapshots/<scene_ulid>/` — exactly one `.zst` file should exist (the OnClose snapshot from M1.1 B1).
- [ ] Re-open the project, edit the scene further, close again. The directory now contains TWO `.zst` files.

## Sidecar (auto-spawned)
- [ ] (M1.1) After clicking `open` (or `create`), the diagnostics page's `sidecar` section shows `status: ready` within 8 seconds. No need to launch uvicorn in a separate terminal (M1.1 B2 fix).
- [ ] The Tauri dev console (Ctrl+Shift+I) shows `sidecar:status` events being emitted.
- [ ] Click `close`. The OS-level sidecar process (visible via Task Manager or `Get-Process | ? Name -match python`) terminates within 1 second.

## Logs hygiene
- [ ] (M1.1) Drop a malformed `~/.water/dev-keys.toml` (e.g., `anthropic = unquoted`). On the next `pnpm dev` launch, the terminal logs a `tracing::warn!` naming the file and the parse error (M1.1 A4 fix). The app does NOT crash; `provider_test` for a key not in the keychain or env-var fallback returns `Error::NotFound`.

When every box above is checked, M1.1 is **accepted** and ready for the `m1.1` tag.
