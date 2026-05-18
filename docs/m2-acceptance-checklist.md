# M2 Acceptance Checklist

**Tag:** `m2`
**Base:** `m1.5.1` (`404fadf`)
**Plan:** `docs/superpowers/plans/2026-05-17-m2-editor-pill-engine.md`
**Spec:** `docs/superpowers/specs/2026-05-17-m2-editor-pill-engine.md`

## Exit criteria (master spec § 4.4)

- [ ] Idle 3 s after a paragraph → at most one pill surfaces within 1.5 s of analysis completing.
- [ ] Two pills max on screen; mid-sentence typing never surfaces a pill.
- [ ] Expanding a pill shows exactly 3 sub-pills; regenerate produces 3 different ones.
- [ ] Rabbit hole works at arbitrary depth; breadcrumb collapse visible; anti-loop fires when configured.
- [ ] Pinning a pill persists it across app restart; dismissed pills do not.
- [x] Tone audit: 0 instances of "you should", "consider", "try", "I think you", "as an AI" in 200 sampled pills.
  - Verified by `cargo test -p water-core --test tone_audit_200` against `CannedProvider`. 200/200 fixtures pass with `layer3_catches=0`, `audit_violations=0`.

## Carried-over debt (M1.5 review + KNOWN_FRAGILE)

- [x] KNOWN_FRAGILE #6 closed — sidecar respawn-with-backoff (T3 @ `854996b`). 1s → 2s → 5s → 10s → 30s cap, reset on success. Property tests in `sidecar_supervisor.rs`.
- [x] KNOWN_FRAGILE #7 closed — per-scene `SceneWriteLocks` (T2 @ `f79f7d7`). 50-iteration concurrent rename + write_body test proves serialization.
- [x] Review #4 closed — ScenesPanel `reloadToken` (T24 @ `b26a6a9`). Scroll position preserved across reload.
- [x] Review #5 closed — SettingsSheet fully event-driven (T4 @ `4cbfccc` for `sidecar:status`; T28 @ `995fbe3` for `provider:status`). All polling deleted.
- [x] Review #14 closed — Sheet slide-in transform via `data-state` state machine (T23 @ `b9532b2`).

## New KNOWN_FRAGILE entries opened (spec § 17)

- [x] #8 — Anti-loop Jaccard suffix-stripper is English-only.
- [x] #9 — Pill block-anchor stability under decoration churn.
- [x] #10 — Structural-inflection detection is shallow.

## Build gates (verified pre-tag)

- [x] `cargo test -p water-core` — 134 lib + 6 integration + 1 orchestrator + 1 tone-audit-200 = **142 passed**, 1 ignored (sidecar-needs-uv).
- [x] `pnpm --filter @water/app test` — **69 passed** across 21 files.
- [x] `cargo clippy -p water-core --all-targets -- -D warnings` — clean.
- [x] `cargo clippy -p water-app -- -D warnings` — clean.
- [x] `cargo fmt -p water-core --check` — clean (post T26 follow-up at `3e79b11`).
- [x] `cargo build -p water-app` — clean.
- [x] `pnpm --filter @water/app build` — clean (~120 KB gzip main bundle; +66 KB delta from baseline for ProseMirror).

## Manual smoke (exit-criteria #1–#5)

The behavioral exit criteria above require an interactive session with a configured LLM provider:

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
pnpm --filter @water/app tauri dev
```

Then verify:

- [ ] Open a project; type a paragraph; idle 3 s → a pill emerges within 1.5 s of analysis completing.
- [ ] Click pill → bouquet of 3 sub-pills with slight hue differentiation (feel/notice/wonder).
- [ ] Regenerate → 3 new sub-pills, materially different in opening words.
- [ ] Sub-pill click → rabbit hole expands; breadcrumb shows; siblings collapse to glow-lines on the left edge.
- [ ] Pin → pill migrates to 56 px right-edge column at half opacity; click reopens detail sheet.
- [ ] Restart app → pinned pill still in column.
- [ ] Mid-sentence typing → no pill surfaces (gating handled by the orchestrator's cursor-classification check).

The integration test `crates/water-core/tests/orchestrator_integration.rs` covers the deterministic pre-LLM pipeline (trigger evaluation → voice routing → prompt assembly). The LLM-output side has no automated end-to-end test in M2; the tone audit (`tone_audit_200`) covers the tone-correctness specifically.

## Phase-by-phase commit map

| Phase | Tasks | Range |
|---|---|---|
| A — Foundations | T1 event bus → T5 migration runner + fmt fix | `61400e3` … `f439326` |
| B — Editor | T6 PM/Lexical bake-off → T10 telemetry events + Phase B follow-up | `bd3dba2` … `41412e7` |
| C — Orchestrator | T11 Trigger trait → T14 anti-loop | `a8cd33f` … `62b0146` |
| D — Voice & prompts | T15 personas → T18 PostFilter | `7cd9b3d` … `f2c4ca4` |
| E — Pill UI | T19 PillLayer → T25 narrow-viewport fallback | `32447fd` … `46832d8` |
| F — Integration | T26 orchestrator service → T28 provider:status + fmt fix | `8e9b418` … `3e79b11` |
| G — Audit + tag | T29 tone audit → T30 docs + tag | `b17a21f` … `m2` |

## Notes for M3

- **Character voices ride the M2 engine.** `Speaker` trait is in place; `CharacterRegistry::list()` returns empty until M3 fills it from LSM v2.1 sheets.
- **Two trigger stubs await M3:** `character_dissonance` and `idle_pause_with_present_character` return `None` today.
- **Bouquet sub-pill pin context is empty in M2** (scene_id / block_id / snippet ship as `""` from the Bouquet UI). M3 should thread real scene context through `PillLayer` → `Bouquet`.
- **Replay log + tone audit are nightly-ready.** M3 dev can enable `WATER_REPLAY_LOG=1` to capture session JSONL and replay against new triggers / new character voices.
