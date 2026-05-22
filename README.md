# Water

> A writing application that turns the act of writing into a conversation with your developing universe.

Water is a desktop writing app (Tauri 2, Rust + React) that leverages LLMs **without ever generating text into the manuscript**. Instead, soft pastel **pills** surface in the right margin — sometimes as the writer's own characters reacting to scenes they're in, sometimes as a council of ambient personas commenting on craft — keeping the writer in flow while the universe of the story emerges around them.

## Status

**Closed alpha.** Installers built per platform via GitHub Actions on tag push; testers download from the [Releases page](https://github.com/hon-ai/water-app/releases).

If you got here through a direct invite, see [`docs/TESTER.md`](docs/TESTER.md) for setup.

## Hard principles (architectural, non-negotiable)

1. **No conversational input to LLMs.** Inputs are the manuscript + clicks. There is no chat box, ever.
2. **Universe-first, personas-second.** Character voices dominate; personas fade as the universe matures.
3. **Flow protection beats feature density.** Pills never appear mid-sentence; max two visible; soft TTL; per-track cooldowns.
4. **Reactive, never instructive.** No `you should…`, `consider…`, tutorial register.
5. **Local-first.** Cloud LLM opt-in via your own provider key; nothing leaves your machine except the explicit calls you authorize.
6. **Human-readable on disk.** Project = visible folder of Markdown + TOML + per-project SQLite index that's rebuildable from the files.
7. **Configurable with a strong default.**
8. **Sea-palette visual identity** on a warm-neutral substrate. Light + dark mode first-class.
9. **Deterministic onboarding** via one-question-at-a-time intake.
10. **Midjourney-style exploration.** Pill rabbit hole = a 4-direction fan (closer / wider / opposite / deeper) with unlimited depth.

## Docs

- **[`docs/TESTER.md`](docs/TESTER.md)** — first-run setup, provider config, troubleshooting.
- **[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)** — full technical overview of the platform (process topology, data model, pill engine pipeline, LLM provider stack, IPC catalog, known fragilities).
- **[`docs/UX_SPEC.md`](docs/UX_SPEC.md)** — locked design direction. The spec the implementation is held to.
- **[`docs/RELEASING.md`](docs/RELEASING.md)** — release process for maintainers (signing keys, tag-cut, CI walkthrough).
- **[`KNOWN_FRAGILE.md`](KNOWN_FRAGILE.md)** — fragile code paths + heuristics worth checking first when something breaks.

## Repository layout

```
water-app/
├── README.md
├── LICENSE                              ← AGPL-3.0
├── KNOWN_FRAGILE.md
├── docs/
│   ├── ARCHITECTURE.md
│   ├── TESTER.md
│   ├── RELEASING.md
│   ├── UX_SPEC.md
│   ├── landing/                         ← static GitHub Pages landing
│   ├── m1-acceptance-checklist.md
│   └── m2-acceptance-checklist.md
├── app/
│   ├── src/                             ← React renderer
│   └── src-tauri/                       ← Tauri shell + Rust commands
├── crates/
│   └── water-core/                      ← orchestrator, triggers, LLM adapters, voice router
├── sidecar/                             ← Python FastAPI sidecar (per-paragraph stylometry)
├── prompts/                             ← TOML prompt library (personas, triggers, tasks, tone)
└── .github/workflows/release.yml        ← cross-platform CI build
```

## Build from source

You need Rust (stable), Node 20, pnpm 9, and `uv` (for the analysis sidecar; optional but recommended).

```powershell
pnpm install
pnpm --filter @water/app tauri dev      # development
pnpm --filter @water/app tauri build    # production installer
cargo test --workspace                  # Rust tests (~490)
pnpm --filter @water/app test -- --run  # frontend tests (~220)
```

## License

[AGPL-3.0](LICENSE). If you run a modified version as a network service, you must release your modifications under the same license. Pull requests welcome.
