# Water

> A writing application that turns the act of writing into a conversation with your developing universe.

Water is a desktop writing app (Tauri, Rust + React) that leverages LLMs without ever generating text into the manuscript. Instead, soft pastel **pills** surface in the margins — sometimes as your own characters reacting to scenes they're in, sometimes as ambient personas commenting on craft — keeping the writer in flow while the universe of the story emerges around them.

## Status

Pre-implementation. The design spec is locked: see [`docs/superpowers/specs/2026-05-16-water-design.md`](docs/superpowers/specs/2026-05-16-water-design.md).

Implementation plans are produced per milestone via the `writing-plans` workflow before any code is written.

## Hard principles (architectural, non-negotiable)

1. **No conversational input to LLMs.** Inputs are the manuscript + clicks. There is no chat box, ever.
2. **Universe-first, personas-second.** Character voices dominate; personas fade as the universe matures.
3. **Flow protection beats feature density.** Pills never appear mid-sentence; max two visible; soft TTL; per-track cooldowns.
4. **Reactive, never instructive.** No `you should…`, `consider…`, tutorial register.
5. **Local-first.** Bundled local model; cloud opt-in.
6. **Human-readable on disk.** Project = visible folder of Markdown + TOML; SQLite is a rebuildable index.
7. **Configurable with a strong default.**
8. **Pastel-glow visual identity** on a Notion/Apple-fluid baseline. Light + dark mode first-class.
9. **Deterministic onboarding** via one-question-at-a-time intake.
10. **Midjourney-style exploration.** Pill rabbit hole = bouquets of 3 with translucent regenerate; unlimited depth.

See the spec for the full rationale, architecture, data model, milestones, and risks.

## Repository layout (in progress)

```
Water/
├── README.md
├── KNOWN_FRAGILE.md                    ← heuristics that may break; first stop for debug agents
├── docs/
│   └── superpowers/
│       └── specs/
│           └── 2026-05-16-water-design.md
└── .gitignore
```

App code, sidecar, prompts, and eval-harness directories arrive in M1.

## License

TBD.
