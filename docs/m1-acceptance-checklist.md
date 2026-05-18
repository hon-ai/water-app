# M1 / M1.1 / M1.5 Acceptance Checklist (Internal)

Run on a clean macOS or Windows machine with `uv` and `pnpm` installed.

## Build
- [ ] `pnpm install` — no errors.
- [ ] `cargo build -p water-app --release` — succeeds (first build may be slow).
- [ ] `pnpm --filter @water/app build` — succeeds.

## Test
- [ ] `pnpm test:core` — all `water-core` tests pass (76+ at M1.5).
- [ ] `pnpm test:app` — all renderer tests pass (~22+ at M1.5).
- [ ] `cargo test -p water-core --test m1_exit_criteria` — 6 passed, 1 ignored.
- [ ] `cargo test -p water-core --test m1_exit_criteria -- --ignored` — sidecar boot test passes (requires `uv` on PATH).

## Run the app — chrome inspection
- [ ] `pnpm dev` launches the window. **No top header.** A 56 px icon rail sits on the left with five icons: app mark (Droplet), Scenes, Characters (stub), World (stub), Settings (gear).
- [ ] On first launch (no project open), the canvas shows the EmptyState: a large Droplet, the word "Water" in serif, and two buttons — "Create new project" and "Open existing".
- [ ] Clicking the gear opens the Settings sheet from the right. Appearance shows three theme buttons (Light / Dark / Auto) as a segmented control. Click each — the page colors flip. Providers section lists the five provider ids. Developer info section has a collapsible raw JSON dump.
- [ ] Close the settings sheet by clicking the X, clicking outside the sheet, or pressing Escape.

## Project lifecycle — sheet flow
- [ ] Click "Create new project" → a sheet slides in from the right. Enter a project name. Click "Browse…" → native OS folder picker opens. Choose your Desktop (or any folder). The parent directory field fills with the chosen path. Click "Create" → the sheet closes, the scenes panel appears on the left with the project name at the top.
- [ ] (M1.5) Click "+ new scene" → a "Scene 1" row appears in the scenes list. Click it → the editor canvas centers, showing an empty Plex Serif title field above an empty Plex Sans body textarea, with no chrome.
- [ ] Type a title (e.g. "Opening"). Click outside the title (or press Enter) → the scenes-panel row updates to "Opening" within ~1 s (M1.5 B0 scene_rename fix).
- [ ] Type a few paragraphs into the body. Wait 2 s — the "saved · HH:MM" chip appears top-right of the canvas.
- [ ] Click the scenes panel's chevron — the panel collapses to width 0 px. Click again — restores to 280 px (M1.5 B2 collapse + persistence).
- [ ] Open `manuscript/scenes/<ulid>.md` in any external editor — the body matches what you typed; the YAML frontmatter has `name: "Opening"`.

## Project switching — dropdown menu
- [ ] Click the project-name button at the top of the scenes panel → a dropdown menu shows "Switch project…", "Open folder…", and "Close project".
- [ ] Click "Close project" → returns to the EmptyState.
- [ ] Click "Open existing" → native folder picker. Pick your `.water` folder → scenes panel reappears with your previous scene.

## Rebuild from truth (unchanged from M1.1)
- [ ] Close the app. Delete `<project>.water/project.db`. Relaunch. Open the project. Scene list still shows your scenes.

## Provider round-trip
- [ ] In Settings → Providers, click "Test" on the canned row. Three placeholder variants appear (via the original `ipc.providerTest`; M1.1 A1 ensures the in-state router contains the actually-tested provider).
- [ ] (Optional, with real keys at `~/.water/dev-keys.toml`) Click "Test" on the anthropic row. Three real variants come back.

## Snapshot timeline (auto-fired, unchanged from M1.1)
- [ ] Type a scene, close the project, inspect `<project>.water/snapshots/<scene_ulid>/` — at least one `.zst`.

## Sidecar (auto-spawned, unchanged from M1.1)
- [ ] Settings → Developer info → raw JSON shows `sidecar.status: "ready"` within 8 s of opening a project.

## Logs hygiene (unchanged from M1.1)
- [ ] Malformed `~/.water/dev-keys.toml` triggers a `tracing::warn!` on next `pnpm dev` launch; app does NOT crash.

When every box above is checked, M1.5 is **accepted** and ready for the `m1.5` tag.
