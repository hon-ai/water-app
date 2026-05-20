# M4 acceptance fixture — Pell Library

Reference world entry + four test scenes for validating the `world_drift` trigger end-to-end.

## Expected behaviour

| Scene                              | Stage 1 (name+overlap) | Stage 2 (LLM) | Pill fires? |
|------------------------------------|------------------------|----------------|-------------|
| `consistent.md`                    | match                  | consistent     | NO          |
| `contradiction_sunlight.md`        | match                  | contradicts    | YES         |
| `contradiction_elevation.md`       | match                  | contradicts    | YES         |
| `near_miss_unrelated.md`           | suppressed             | (skipped)      | NO          |

The `near_miss` scene contains the string "Pell" but **no content overlap** with the entry's `[main]` block — the contextual-overlap pre-check (≥ 2 content words shared with the `[main]` body) suppresses the candidate before any LLM call. This keeps the trigger cheap on incidental mentions.

`contradiction_sunlight.md` plants contradictions against:
- `main.sensory_detail` ("No natural light reaches the lower stacks" → "sunlight warming the high reading desk")
- `lists.notable_features` ("no natural light" → "gold pouring through the long windows")

`contradiction_elevation.md` plants a contradiction against:
- `main.type` ("underground library" → "perched on the cliff", "tower visible from the docks below")

## Usage

1. Open Water; create a fresh project.
2. Open the World nav → **Locations** → **+ New entry**.
3. Walk the intake. Use the values from `pell_library.toml`:
   - **Name:** The Pell Library
   - **Type:** underground library
   - **Sensory Detail:** (the long paragraph from `[main]`)
   - **Notable Features:** the sub-basement, the locked east wing, no natural light, brass lanterns
   - **Significance:** (the paragraph from `[main]`)
4. Open the entry sheet and add aliases via the AliasesEditor: **Pell**, **the library**, **Aren's old place**.
5. Create a new scene; open Scene details → Location → select **The Pell Library**.
6. Paste each test scene's text in turn into the editor body. Save (2 s debounce). Observe the right-side pill margin.

Reference for the M3 character-side equivalent: `eval/m3_acceptance/marcus_vale.toml`.

## What "YES" actually means in practice

A Cartographer pill should land within a few seconds of the debounced save (the `world_drift` trigger fires at the same `typing:telemetry` cadence as other triggers). The pill's tone must be reactive/observational (`KNOWN_FRAGILE #16`: anti-loop applies); the writer can pin it to seed a follow-up note or dismiss to suppress for the rest of the session.
