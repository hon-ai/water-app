# M1 Foundation — Resume Handoff (v3)

**Updated:** 2026-05-17 (session 3 closed)
**Reason:** Session 3 completed Tasks 9-12 and approached its context budget. A fresh session resumes at Task 13.

This document is everything a new controller needs to pick up where I left off without re-reading the prior conversation.

---

## TL;DR

- Plan: `docs/superpowers/plans/2026-05-16-m1-foundation.md`. 40 tasks. **Tasks 1-12 are done.** Resume at **Task 13**.
- Spec: `docs/superpowers/specs/2026-05-16-water-design.md`. Locked.
- Workflow: **subagent-driven-development** (`superpowers:subagent-driven-development` skill).
- Branch: `master`. No worktree. Plan executes directly on master.

---

## Repo state

### Session 3 commit history (newest first)

| SHA | What |
|---|---|
| `391ddbb` | **feat(core): block-id maintenance for scene bodies** (Task 12) |
| `5b43969` | **feat(core): scene .md frontmatter + body codec** (Task 11) |
| `a68c17f` | plan(T10): amend Task 10 to wrap toml parse errors with file path |
| `f208904` | fix(core): include file path in water.toml parse errors |
| `31c3747` | **feat(core): water.toml read/write** (Task 10) |
| `661d571` | plan(T9): amend Task 9 with set_default_manuscript fix and added tests |
| `d3f46b2` | fix(core): enforce project<->manuscript relationship and add coverage |
| `f9268ca` | **feat(core): Project + Manuscript stores** (Task 9) |
| `352471c` | docs: handoff v2 for M1 resume at Task 9 (session-2 handoff) |

Earlier commits documented in handoff v2 (in git history, search for "session-1 handoff" and "session-2 handoff" SHAs).

### Tree summary (significant files only, additions in session 3 marked ★)

```
Water/
├── Cargo.toml                       ← workspace; deps unchanged from v2
├── Cargo.lock                       ← committed; unchanged this session
├── rust-toolchain.toml              ← stable
├── pnpm-workspace.yaml
├── pnpm-lock.yaml
├── package.json                     ← 7 root scripts
├── .editorconfig
├── .gitignore
├── README.md
├── KNOWN_FRAGILE.md                 ★ entry 2 appended (block-id duplicate tolerance)
├── docs/superpowers/
│   ├── specs/2026-05-16-water-design.md
│   ├── plans/2026-05-16-m1-foundation.md  ← amended 7 times (T2,T4,T5,T6,T7,T9,T10)
│   └── handoff/m1-resume.md         ← this file (v3)
├── crates/water-core/
│   ├── Cargo.toml                   ← unchanged this session
│   ├── sql/v1_init.sql              ← full 16-table schema (T8)
│   └── src/
│       ├── lib.rs                   ← re-exports + #![allow(clippy::missing_errors_doc)]
│       ├── error.rs                 ← 13-variant Error enum
│       ├── id.rs                    ← ULID Id newtype
│       ├── db.rs                    ← Db wrapper
│       ├── migrations.rs            ← rusqlite_migration runner
│       ├── project.rs               ★ ProjectStore + ManuscriptStore (T9, 6 tests)
│       ├── water_toml.rs            ★ WaterToml read/write (T10, 2 tests)
│       ├── scene_md.rs              ★ SceneFile + SceneFrontmatter codec (T11, 4 tests)
│       └── block.rs                 ★ Block + ensure_block_ids/split_blocks (T12, 5 tests)
└── app/                              ← unchanged this session
    ├── package.json, tsconfig.json, vite.config.ts, postcss.config.cjs, tailwind.config.ts
    ├── index.html
    ├── src/ (App.tsx, App.test.tsx, test-setup.ts, main.tsx, styles/tokens.css)
    └── src-tauri/ (Cargo.toml, tauri.conf.json, build.rs, icons/icon.ico, src/main.rs)
```

### Verified working at HEAD (`391ddbb`)

- `cargo build -p water-core` → succeeds.
- `cargo test -p water-core` → **26 passed** (was 9 at end of session 2; +17 across T9-T12).
- `cargo clippy -p water-core --all-targets -- -D warnings` → clean.
- App-side stack (`cargo build -p water-app`, `pnpm --filter @water/app test`, `pnpm --filter @water/app build`) — not re-verified this session (T9-T12 are core-only). Last verified at end of session 2.

---

## Toolchain inventory

Unchanged from handoff v2. Confirmed still working:

| Tool | Version | Path |
|---|---|---|
| rustc / cargo | 1.95.0 | `%USERPROFILE%\.cargo\bin\` |
| node | v22.14.0 | `C:\Program Files\nodejs\` |
| pnpm | 9.15.9 | `%APPDATA%\npm\` |
| uv | (latest) | `C:\Users\H BLAUNTE\.local\bin\` |
| git | latest | `C:\Program Files\Git\cmd\` |

**PowerShell PATH workaround still required** for each bash invocation that runs cargo:
```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
```

---

## Plan amendments made during execution

The plan file has been amended **seven times** during execution (was 5 at end of session 2; +2 this session). Each amendment is a separate git commit, and the plan content at HEAD already reflects all of them.

| Commit | Amendment |
|---|---|
| `94e1494` | T1 prereq: pin toolchain to `stable`. |
| `8830071` | T2: drop `[lib]` block; drop log-plugin shim; require `icons/icon.ico`. |
| `bf36688` | T4: vite.config.ts imports `defineConfig` from `vitest/config`. |
| `20d1d3c` | T5: add `"noEmit": true` to `app/tsconfig.json`. |
| `b279f71` | T6: add `#[must_use]` to `Id::new` / `Id::as_str`. |
| `7d30770` | T7: `#[must_use]` on `migrations::all` / `Db::conn` / `Db::conn_mut`; backtick `` `SQLite` ``. |
| `661d571` | **T9: `set_default_manuscript` UPDATE now carries `AND EXISTS` subquery against `manuscript.project_id`; error names both ids; +2 tests; +2 assertions on round-trip test. Total T9 tests = 6 (was 4).** |
| `a68c17f` | **T10: `WaterToml::read` wraps `toml::from_str` errors in `Error::InvalidProject(format!("parse {} ...", path.display()))` so on-disk path is named symmetrically with io errors.** |

**Patterns observed in session 3:**

- The "fix the code review finding + amend the plan" pattern matured into a stable 3-commit shape per task when needed: `feat(...)` (impl) → `fix(...)` (code review patch) → `plan(TN): amend ...` (plan listing updated to match). T9 and T10 both took this shape. T11 and T12 were clean DONEs with no follow-ups.
- The `clippy::pedantic` warning gate continues to be the dominant source of plan amendments. Proactive `#[must_use]` hints in implementer prompts now consistently save a fix round.
- **`lib.rs` declares `#![allow(clippy::missing_errors_doc)]`** at line 8 — `pub fn` returning `Result` does NOT need `/// # Errors` docs. (Discovered this session, removed the `Errors`-doc proactive guidance from later prompts.)

---

## Sharp edges / known issues

Existing entries from handoff v2 (still apply):

1. **CRLF/LF git warnings** — ignore.
2. **Cargo.lock is committed.** New deps modify it.
3. **`app/src-tauri/icons/icon.ico`** is a placeholder — replace at M7.
4. **`Error::Other(String)` overuse footgun** — accept for v1, revisit at M7.
5. **`clippy::pedantic` is warn at crate root.** Frequent fires: `must_use_candidate`, `doc_markdown`, `single_match_else`, `implicit_hasher`. Add `#[must_use]` proactively in prompts; backtick code-like identifiers (`SQLite`, `YAML`, `Markdown`, `ULID`, `.md`) in doc comments.
6. **Plan invocation command bug:** plan T7 uses two-filter `cargo test` syntax which is invalid. Trust the test result, not the invocation text.
7. **Commit message wording:** quote plan's exact commit-message string in dispatch prompts.
8. **Unused className `"water-shell"`** in `app/src/App.tsx` — T34/T35 territory.
9. **stale `.js` artifacts** — were a problem prior to T5; should not recur.
10. **WAL sidecars** in tempfile tests — harmless.
11. **`schema_version` table is hand-managed**, separate from `PRAGMA user_version`. V2+ migrations must INSERT their version row manually.
12. **Cascade index gaps** — acceptable at M1 volumes.
13. **`schema_version_row_is_one` test** uses `query_row` — silently passes if a future migration inserts version=2.

New entries from session 3:

14. **`set_default_manuscript` now enforces project↔manuscript pairing** via `AND EXISTS (SELECT 1 FROM manuscript WHERE id = ?2 AND project_id = ?1)`. Future code that wants to *unset* the default (clear it back to NULL) does not have a method yet — add `clear_default_manuscript` if needed.
15. **`WaterToml::read` wraps both io and toml-de errors in `Error::InvalidProject`**. The `Error::TomlDe` variant is no longer reachable via `WaterToml::read`. Other callers (e.g., `chapters.toml` in T14, character TOML in T15) may either follow the same pattern or use bare `?` — choose deliberately per task. The asymmetry in error envelope across modules is acceptable for M1.
16. **`WaterToml::write` does NOT create parent directories.** Callers must `create_dir_all` themselves. T13/T14/T15 will likely all need to bear this responsibility.
17. **`SceneFile::to_string` has `#[allow(clippy::inherent_to_string)]`** since clippy fires on the inherent-vs-Display pattern even for `Result<String>` returns. Don't try to "fix" by implementing Display — the signature mismatch makes that wrong.
18. **`SceneFile` writes are not atomic** — no temp+rename. Snapshot system (T16-T19) covers recovery.
19. **`fresh_block_id` takes `&HashSet<String, S: BuildHasher>`** — generalized signature courtesy of `clippy::implicit_hasher`. Callers using default-hasher `HashSet` work transparently.
20. **`ensure_block_ids` does NOT deduplicate colliding `^bk-XXXX` tokens.** Documented in `KNOWN_FRAGILE.md` entry 2. The test `ensure_dedupes_colliding_ids` is misleadingly named — it asserts duplicate **preservation** (intentional for v1; pill anchoring is snippet-based per spec §3.3).
21. **`block.rs::fresh_block_id`** has a `unwrap_or("xxxx")` fallback that's unreachable in practice (ULIDs are always 26 chars). Defensive code; harmless.
22. **No integration tests yet** cross-coupling Project↔Manuscript↔Scene↔Block — first one comes in T13 SceneStore. Watch for it.

---

## Subagent prompt conventions (refined this session)

The v2 conventions still hold. Session 3 added:

- **Combined spec + code-quality review** in one dispatch is appropriate when the implementer reports DONE first try AND the listing was copied near-verbatim. T11 and T12 used this — clean and fast. Don't combine when there's any signal of friction (NEEDS_CONTEXT, DONE_WITH_CONCERNS, multiple lint fires).
- **`lib.rs` has `#![allow(clippy::missing_errors_doc)]`** — drop the "/// # Errors" proactive guidance from prompts; only add `#[must_use]` guidance for non-Result pub fns.
- **Pre-authorize lint fixes by name** in the prompt body so the implementer doesn't have to escalate every `clippy::pedantic` fire. Session 3 saw `implicit_hasher`, `single_match_else`, `inherent_to_string`, `doc_markdown` all auto-resolved this way.
- **Track expected total test counts** in prompts (e.g., "after this task: 26 passed"). Lets the implementer self-validate without consulting the plan.
- **Quote commit messages verbatim** continues to be a discipline worth maintaining. No commit-message divergences this session.

---

## How to resume — exact next-step prompt

Open a fresh session. After acknowledging the conversation start (per `using-superpowers`), issue this prompt:

> Resume execution of the Water M1 Foundation plan at Task 13.
>
> - Plan: `docs/superpowers/plans/2026-05-16-m1-foundation.md`
> - Spec: `docs/superpowers/specs/2026-05-16-water-design.md`
> - Handoff context: `docs/superpowers/handoff/m1-resume.md` (this file, v3)
> - Tasks 1-12 are committed on `master`. HEAD is `391ddbb`.
> - Use the `superpowers:subagent-driven-development` skill.
> - When context starts to run low, write another handoff doc (v4) and stop cleanly.
>
> Start by:
> 1. Loading the `subagent-driven-development` skill.
> 2. Reading this handoff doc and the plan's Task 13 section (line 2198).
> 3. Updating the TodoWrite (T1-T12 completed, T13 in progress, T14-T40 + final review pending).
> 4. Dispatching the Task 13 implementer with full task text inlined.
> 5. Following the subagent prompt conventions documented above.
>
> Note: Task 13 (SceneStore) is the first task that composes prior CRUD pieces (DB schema, ProjectStore/ManuscriptStore, water.toml, SceneFile, Block). It will likely surface integration issues that weren't visible in earlier unit tests. Expect at least one fix round.

---

## Remaining task index

28 remaining tasks + final review:

| # | Phase | Task | Notes |
|---|---|---|---|
| 13 | C | SceneStore (create/read/write_body/move_to/list) | First integration of all prior pieces. Composes blocks + scene_md + DB. Expect non-trivial. |
| 14 | C | `chapters.toml` read/write | TDD |
| 15 | C | CharacterStore + WorldStore | M1 thin TOML-only surface |
| 16 | D | Snapshot writer (zstd) | TDD |
| 17 | D | Snapshot retention pruner | Hourly/daily/weekly |
| 18 | D | Snapshot scheduler (tokio task) | `tokio::test(start_paused=true)` |
| 19 | D | Snapshot restore (creates pre-restore) | TDD |
| 20 | E | Rebuild-from-truth (scan folder → repopulate) | Integration-shaped test |
| 21 | E | External-edit repair pass | Builds on Rebuild |
| 22 | F | Sidecar scaffold with `uv` | Creates `sidecar/` workspace |
| 23 | F | `/analyze` stub + pytest tests | Deterministic stub for M2 wiring |
| 24 | F | Shared IPC contract (`water-core/src/ipc.rs`) | TDD |
| 25 | G | `Sidecar` handle (managed + external modes) | wiremock for unit tests |
| 26 | G | `SidecarSupervisor` (watch channel) | TDD |
| 27 | H | `LlmProvider` trait + `CannedProvider` | Foundation for all adapters |
| 28 | H | Anthropic adapter | wiremock |
| 29 | H | OpenAI adapter | wiremock |
| 30 | H | Ollama adapter | wiremock |
| 31 | H | llama.cpp adapter | wiremock |
| 32 | H | MLX adapter (feature-flagged stub) | `--features mlx` |
| 33 | H | Router with secrets, rate limit, circuit breaker | TDD |
| 34 | I | Pastel-glow design tokens (CSS variables) | **Watch for:** `tailwind.config.ts borderRadius extend.2xl/3xl gap** flagged in T4 code review |
| 35 | I | ThemeProvider (light/dark/auto) | RTL tests |
| 36 | J | AppState + project commands | Tauri commands |
| 37 | J | Scene + provider + diagnostics commands | Tauri commands |
| 38 | J | SceneList + Diagnostics UI + IPC client | First end-user surfaces |
| 39 | K | M1 exit-criteria integration tests | 4 tests + 1 `#[ignore]` |
| 40 | K | Manual acceptance checklist + final sanity | docs + build |
| — | — | Final code review of entire M1 implementation | End gate |

---

## Session statistics

For calibration of the next controller's budget planning:

### Session 3 (this one)

- **Tasks completed:** T9, T10, T11, T12 (four tasks).
- **Commits this session:** 8 (4 feats + 2 fixes + 2 plan amendments) plus this handoff = 9.
- **Implementer DONE first time, no fixes:** T11, T12 (2/4 = 50%).
- **Tasks that needed a follow-up `fix(...)` commit from code-quality review findings:** T9, T10 (2/4 = 50%).
- **Plan amendments needed:** 2 (T9 cross-project foot-gun, T10 path-context wrap).
- **Combined spec+quality reviews used:** T11, T12 (worked well for clean-first-try cases).
- **Tests grew:** 9 → 26 (+17 across four tasks).
- **No build/test regressions** at any point.

### Cumulative across sessions

- **Tasks complete:** 12 of 40 (30%).
- **Plan amendments total:** 7.
- **Tests in `water-core`:** 26 passing.

### Heuristic for next controller

A "clean DONE first-try" task (like T11 or T12) cost roughly **18-25k controller tokens** end-to-end (implementer + combined review). A "DONE with code-review fix loop" task (like T9 or T10) cost roughly **35-50k tokens** (implementer + spec review + quality review + fix + plan amendment). **Plan for ~30-40k average.**

If T13 lives up to its "first integration task" billing, expect closer to 50k tokens. Consider stopping after T13 if budget tightens.

**If the user asks "where are we"** the answer is: **"T1-T12 done, T13 (SceneStore) next. Tests at 26 passing. Plan amended 7 times. See `docs/superpowers/handoff/m1-resume.md`."**
