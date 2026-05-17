# M1 Foundation — Resume Handoff

**Created:** 2026-05-16
**Reason:** Initial controller session reached its context budget after completing Tasks 1 and 2. A fresh session resumes at Task 3.

This document is everything a new controller needs to pick up where I left off without re-reading the prior conversation.

---

## TL;DR

- Plan lives at `docs/superpowers/plans/2026-05-16-m1-foundation.md`. 40 tasks, ~14 calendar weeks. Tasks **1 and 2 are done**. Resume at **Task 3**.
- Spec lives at `docs/superpowers/specs/2026-05-16-water-design.md`. Locked. Do not change without explicit user approval.
- The user chose **subagent-driven development** (`superpowers:subagent-driven-development` skill). Continue that workflow.
- Toolchain is installed and verified working. No fresh installs needed.
- Branch: `master`. No worktree. The plan executes directly on master with frequent commits.

---

## Repo state (as of this handoff)

### Commit history (newest first)

| SHA | What |
|---|---|
| `8830071` | plan(T2): drop `[lib]` block, drop log-plugin shim, require `icon.ico` (plan amendment) |
| `ac88d9c` | **feat(app): scaffold Tauri 2 + Vite + React shell** (Task 2 implementation) |
| `d226aa9` | chore: commit `Cargo.lock` and document workspace-member deviation (T1 housekeeping) |
| `5a06322` | **feat(core): initialize water-core crate and Cargo workspace** (Task 1 implementation) |
| `94e1494` | plan: pin rust-toolchain to `stable` (verified locally on 1.95) |
| `6875267` | Add M1 Foundation implementation plan |
| `5310f7e` | Add Water v1 design spec and repo skeleton |

### Tree summary

```
Water/
├── Cargo.toml                       ← workspace, members = ["app/src-tauri", "crates/water-core"]
├── Cargo.lock                       ← committed
├── rust-toolchain.toml              ← stable
├── package.json                     ← N/A (no root package.json yet — that's Task 3)
├── README.md
├── KNOWN_FRAGILE.md                 ← initial char_dissonance entry
├── .gitignore
├── docs/
│   └── superpowers/
│       ├── specs/2026-05-16-water-design.md
│       ├── plans/2026-05-16-m1-foundation.md
│       └── handoff/m1-resume.md     ← this file
├── crates/
│   └── water-core/
│       ├── Cargo.toml
│       └── src/{lib.rs, error.rs}   ← only modules so far
└── app/
    ├── package.json                 ← created Task 2, no install run yet
    ├── index.html
    ├── vite.config.ts
    ├── tsconfig.json
    ├── src/{main.tsx, App.tsx, styles/tokens.css}
    └── src-tauri/
        ├── Cargo.toml
        ├── tauri.conf.json
        ├── build.rs
        ├── icons/icon.ico           ← 766-byte placeholder; replace in M7
        ├── gen/                     ← gitignored
        └── src/main.rs
```

### Verified working

- `cargo build -p water-core` → succeeds.
- `cargo build -p water-app` → succeeds (Tauri 2 dependency tree compiled cleanly).
- `cargo check -p water-app` → succeeds in 43.16s on a warm cache.
- No tests have run yet (Task 5 is when sanity tests land).

---

## Toolchain inventory

All confirmed on `C:\Users\H BLAUNTE\Water` as of handoff time. **The next controller should not reinstall anything.**

| Tool | Version | Path |
|---|---|---|
| rustc | 1.95.0 | `%USERPROFILE%\.cargo\bin\rustc.exe` |
| cargo | 1.95.0 | `%USERPROFILE%\.cargo\bin\cargo.exe` |
| rustup | 1.29.0 | `%USERPROFILE%\.cargo\bin\rustup.exe` |
| node | v22.14.0 | `C:\Program Files\nodejs\node.exe` |
| pnpm | 9.15.9 | `%APPDATA%\npm\pnpm.cmd` |
| uv | (latest) | `C:\Users\H BLAUNTE\.local\bin\uv.exe` |
| git | latest | `C:\Program Files\Git\cmd\git.exe` |

**Important PATH note for PowerShell sessions:** each `bash` tool invocation is a fresh PowerShell. The rustup installer adds `.cargo\bin` to user PATH at install time, but a *given* shell session may not see it until restarted. If a `cargo` invocation reports "command not found", prepend the path with:

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
```

`pnpm` is on user PATH via `%APPDATA%\npm` and persists across shells.

MSVC linker / Visual Studio Build Tools: verified present and functional (confirmed by successful `cargo build -p water-app`).

---

## Plan amendments made during T1–T2 execution

The plan file `docs/superpowers/plans/2026-05-16-m1-foundation.md` has been modified from its original committed state at `6875267`. The current plan content is **the authoritative spec for remaining tasks**.

Three substantive amendments:

1. **`rust-toolchain.toml` channel**: was `"1.78"`, now `"stable"` (commit `94e1494`). Rationale: dev machine has 1.95; pinning to a specific old version forced unnecessary downloads.
2. **Task 2 `[lib]` block**: removed from `app/src-tauri/Cargo.toml` (commit `8830071`). Rationale: the original plan declared `crate-type = ["staticlib", "cdylib", "rlib"]` but did not specify a `src/lib.rs`, which broke the build. Mobile targets (which use `lib.rs` + `mobile_entry_point!`) are not in M1.
3. **Task 2 `tauri_plugin_log_init` shim**: removed from `main.rs` (commit `8830071`). Rationale: was decorative; `tracing-subscriber` provides all logging needed.
4. **Task 2 `tauri.conf.json` `bundle.icon`**: changed from `[]` to `["icons/icon.ico"]` (commit `8830071`). Rationale: Windows Tauri build requires at least one icon for the Windows resource embedder.

If you re-read the plan, you will see these amendments already applied.

---

## Sharp edges / known issues to watch for

1. **CRLF/LF warnings on git operations** are environmental. Ignore them. No `.gitattributes` exists yet; consider adding one if it ever becomes a real problem.
2. **`Cargo.lock` is committed.** Every Rust task that adds deps will modify it; include it in the task's `git add`.
3. **Placeholder `icon.ico`** at `app/src-tauri/icons/icon.ico` is ugly. Replace in M7 polish.
4. **`Error::Other(String)` variant** in `crates/water-core/src/error.rs` was flagged by the T1 code reviewer as a footgun. Acceptable for v1; revisit at M7 if it gets misused.
5. **Tailwind 4 is beta** (`@tailwindcss/postcss@4.0.0-beta.3`). Task 4 will run `pnpm install`; if the beta version is yanked or behaves unexpectedly, fall back to Tailwind 3.4.x. Update `app/package.json` accordingly. The plan does NOT pin Tailwind to a specific runtime API yet — Task 4 just imports `tailwindcss` in `tokens.css`.
6. **`KNOWN_FRAGILE.md`** has two entries: `character_dissonance` (from the spec) and `block-id duplicate tolerance` (added by Task 12 in the plan — not yet executed, but Task 12 will append the entry when run).
7. **Subagent verbosity**: each `general` subagent in this environment tends toward long, prose-heavy reports. To conserve context, the next controller may want to tighten the report-format requests in the subagent prompts. The skill says no "Similar to Task N" shortcuts — that's the constraint on *task content*, not on the *report shape*. You can request "1-paragraph summary, then bullet list of files changed, then `Status: DONE`" without violating the skill.

---

## How to resume — exact next-step prompt

Open a fresh session. After acknowledging the conversation start (per `using-superpowers`), the next controller should issue this prompt to itself or to a fresh subagent:

> Resume execution of the Water M1 Foundation plan at Task 3.
>
> - Plan: `docs/superpowers/plans/2026-05-16-m1-foundation.md`
> - Spec: `docs/superpowers/specs/2026-05-16-water-design.md`
> - Handoff context: `docs/superpowers/handoff/m1-resume.md`
> - Tasks 1 and 2 are committed on `master` at SHAs `5a06322` and `ac88d9c` respectively. Both passed spec + code-quality review.
> - Use the `superpowers:subagent-driven-development` skill — fresh implementer subagent per task with full task text inlined; spec reviewer; code quality reviewer; mark complete; next task.
> - Continuous execution per the skill's rule. Stop only for BLOCKED, ambiguity, or context exhaustion.
> - When context starts to run low, write another handoff doc and stop cleanly.
>
> Start by:
> 1. Loading the `subagent-driven-development` skill.
> 2. Reading the plan file's Task 3 section.
> 3. Updating the TodoWrite (T1 and T2 completed, T3 in progress, T4–T40 + final review pending).
> 4. Dispatching the Task 3 implementer with full task text.

---

## Remaining task index

For quick navigation, here are the 38 remaining tasks with their phases. Plan-section line numbers refer to the current state of `docs/superpowers/plans/2026-05-16-m1-foundation.md` (use `grep -n "^### Task " docs/superpowers/plans/2026-05-16-m1-foundation.md` to confirm).

| # | Phase | Task | Notes |
|---|---|---|---|
| 3 | A | pnpm workspace + root tooling | Runs `pnpm install`; first time the renderer deps land on disk |
| 4 | A | Tailwind 4 wiring + base styles | If Tailwind 4 beta misbehaves, fall back to 3.4 |
| 5 | A | Test scaffolding (Rust + renderer) | Adds `test-setup.ts`, first vitest + cargo test runs |
| 6 | B | ULID utilities (`crates/water-core/src/id.rs`) | TDD |
| 7 | B | SQLite connection + migration runner | `rusqlite_migration` |
| 8 | B | Migration v1 full schema (`sql/v1_init.sql`) | All 16 spec tables |
| 9 | B | ProjectStore + ManuscriptStore CRUD | TDD |
| 10 | C | `water.toml` read/write | TDD |
| 11 | C | Scene Markdown codec (frontmatter + body) | YAML frontmatter via serde_yaml |
| 12 | C | `^bk-XXXX` block-ID maintenance | Appends entry to KNOWN_FRAGILE.md |
| 13 | C | SceneStore (create/read/write_body/move_to/list) | Composes blocks + scene_md + DB |
| 14 | C | `chapters.toml` read/write | TDD |
| 15 | C | CharacterStore + WorldStore | M1 thin surface |
| 16 | D | Snapshot writer (zstd) | TDD |
| 17 | D | Snapshot retention pruner | Hourly/daily/weekly policy |
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
| 34 | I | Pastel-glow design tokens (CSS variables) | Replaces placeholder in `tokens.css` |
| 35 | I | ThemeProvider (light/dark/auto) | React Testing Library tests |
| 36 | J | AppState + project commands | Tauri commands |
| 37 | J | Scene + provider + diagnostics commands | Tauri commands |
| 38 | J | SceneList + Diagnostics UI + IPC client | First end-user surfaces |
| 39 | K | M1 exit-criteria integration tests | 4 tests + 1 `#[ignore]` |
| 40 | K | Manual acceptance checklist + final sanity | docs + build |
| — | — | Final code review of entire M1 implementation | Per `subagent-driven-development` end gate |

---

## Conversation context the next controller does not need

For brevity: the next controller does **not** need to know about the 8-section brainstorming dialogue that produced the spec, the user's selection of "subagent-driven (Recommended)" as the execution mode, the `Rustlang.Rustup` winget install, or the Task 1 and Task 2 subagent reports verbatim. All of those are captured in their outcomes (commits + plan + this doc). Resuming from clean context is the explicit goal.

If the user asks "where are we" the right answer is: "T1 and T2 done, T3 next, plan amended 3 places — see `docs/superpowers/handoff/m1-resume.md`."
