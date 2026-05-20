# M5 acceptance fixture — Heatmap demo scene

A single hand-shaped scene exercising the five heat metrics so the manual smoke walk can compare the strip against a known-expected pattern.

## The scene

`heat_demo.md` — 8 paragraphs, designed with deliberate shapes:

| ¶ | Text shape | Expected metric pattern |
|---:|---|---|
| 1 | Short, clipped, contrast ("snow / dust") | Pacing **high** (front-loaded burst) · Valence **cold** |
| 2 | Tight three-beat repetition ("She moved fast. She moved fast.") | Pacing **high** · Coherence **high** with ¶1 (same beat) |
| 3 | Long sentence, slowed cadence, reflection at the window | Pacing **low** · Valence **cold→neutral** · Coherence **high** with ¶2 |
| 4 | Setting beat — describes the Library | Pacing **low** · World-refs **high** (Pell Library, east wing, lower stacks, Aren) · Coherence **mid** (shift to setting) |
| 5 | New character (Theo) arrives | Presence **high** (Mara + Aren + Theo) · Coherence **mid** (cut to new beat) |
| 6 | Theo speaks one line | Pacing **low** (one short sentence) · Presence **high** |
| 7 | Wide pan to harbor + ledgers + cove, "none of this had anything to do" | Coherence **low** (topic break — leaves the room) · World-refs **mid** (harbor, cove) |
| 8 | Mara composes herself; warm tone | Valence **warm→neutral** ("voice was warm", "child is mine to keep alive") · Coherence **high** (returns to Mara + Theo) |

## What to verify

1. Open the demo scene fresh.
2. After the autosave debounce, the strip should populate the **Pacing** track. Bars 1–2 dense; bar 3 quieter; bars 4 quiet; bars 5–6 mixed; bar 7 hold; bar 8 light.
3. Open the metric picker (▾) and toggle **Presence** on. Bar 5 + 6 should fill in (Theo + Aren + Mara). Bars without character references stay near 0.
4. Toggle **World-refs** on. Bar 4 should be the brightest cartographer-hued cell (multiple Library references); bar 7 lighter (harbor, cove).
5. Hover each column — tooltip should show metric label + an observational phrase from the bank ("dense", "slow burn", "thick cast", "world dense" depending on what's lit).
6. Drag along the strip — the editor body should scroll smoothly to the targeted paragraph.
7. Click a single column — paragraph briefly flashes its position (smooth scroll into view).
8. With an LLM provider configured, toggle **Valence**. Bar 1 should run cold (snow/dust); bar 8 should run warm ("the warmth that morning", "child is mine to keep alive").
9. Same setup, toggle **Coherence**. Bar 7 should be the lowest cell (topic break to harbor + ledgers); bars 1→2→3 should be high (same beat carried through).

## Notes

- Valence + Coherence stay empty until a provider is configured + the budget-gated compute path lands (M5 follow-up). For M5 v1, the three local metrics (Pacing, Presence, World-refs) cover the smoke.
- The Pell Library + Aren references intentionally reuse the M4 acceptance fixture's world entry. If the project has the Pell Library entry seeded, World-refs lights up on bar 4. Without it, the track is empty (the renderer's "no data" state).

Reference for the M4 character-side equivalent: `eval/m4_acceptance/pell_library.toml`.
