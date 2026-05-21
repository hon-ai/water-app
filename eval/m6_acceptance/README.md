# M6 acceptance fixture — Macro Spatial Canvas

The canvas surface is interactive end-to-end; this smoke walk shapes a fresh project into a known layout and verifies the canvas reads / writes correctly.

## Setup

1. Open Water; create a fresh project (or use any project with ≥ 4 scenes).
2. From the **Scenes** nav, create or rename six scenes so the manuscript ordering is:
   - 1: `The Gate`
   - 2: `The Library`
   - 3: `The Cliff`
   - 4: `The Letter`
   - 5: `Theo Arrives`
   - 6: `Departure`
3. (Optional, for richer sparklines): paste a few paragraphs into each — different lengths so the Pacing track varies.

## Walk

1. **Click the Canvas (Map) icon** on the left rail.
2. **Auto-flow** — all six scenes should appear left-to-right in a single row, evenly spaced.
3. **Drag** — pick up `The Cliff` and drag it down + right. Release. Reload the project (close + reopen) and verify the card returns to its dragged position (frontmatter + DB round-trip).
4. **Click a card** — verify it routes to Scenes nav with that scene active.
5. **Cmd/Ctrl + scroll** — zoom in + out. Behavior: smooth around the cursor; min/max clamp.
6. **Press `O`** — reading-order overlay turns on. Smoothed cubic-Bezier curves between scenes in manuscript order, ~30% opacity. Numbered endpoints (1-6) at each card center.
7. **Click the metric chip (top-right)** — picker opens. Toggle Presence or World-refs. Verify each card's sparkline hue shifts to match the active metric.
8. **Quit and relaunch the app** — verify the canvas re-loads the persisted positions for any cards you moved.

## Expected behavior

- Cards that were never dragged stay at auto-flow positions.
- Cards that were dragged stay at the persisted (canvas_x, canvas_y).
- Reading-order overlay always reflects manuscript order (`scene.ordering`), regardless of spatial layout.
- Heat sparklines stay in sync with the editor strip (both refetch on `heat:updated`).
- Pan + zoom never lose cards off-screen unrecoverably — fit-all on first open and the zoom-around-cursor invariant ensure the writer can always find their way back.

## What this fixture does NOT cover

- Multi-manuscript projects (out of scope for M6 v1).
- Auto-layout heuristics (manual drag is the only positional tool in v1).
- Touch gestures (desktop-first; mouse + cmd-scroll).
- A11y / screen-reader narration (deferred to M7).
