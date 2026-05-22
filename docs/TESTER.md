# Testing Water

Quick-start for anyone trying Water before public release.

---

## Install

1. Run the Water installer (`.msi` on Windows, `.dmg` on macOS).
2. **Recommended, optional**: install [uv](https://docs.astral.sh/uv/) before
   first launch. Water ships with an analysis sidecar that needs uv to run.
   Without uv, Water still works — you just lose the five
   sidecar-dependent triggers (block_anchored_drift, topic_drift,
   pace_floor, valence_spike, scene_flow_dip). The other five fire
   from typing telemetry alone.

   ```powershell
   # Windows (PowerShell)
   powershell -ExecutionPolicy ByPass -c "irm https://astral.sh/uv/install.ps1 | iex"
   ```
   ```bash
   # macOS
   curl -LsSf https://astral.sh/uv/install.sh | sh
   ```

3. Launch Water.

## First run

You'll see a banner that says **"Set up a provider to enable nudges"** —
click **Open Settings**, then:

1. **Pick a provider.** Anthropic / OpenAI / Kimi (Moonshot) / OpenRouter
   are all supported. OpenRouter is the most flexible (one key, many
   models); Anthropic gives the highest-quality pills.
2. **Click "Get a key →"** next to the provider — opens the signup page.
3. **Paste the key** into the "Key" input and click **Save**. Water
   stores the key in your OS keychain (not in any file); it never
   leaves your machine except when calling the provider you chose.
4. The dot next to the provider should turn green within a couple of
   seconds. If it doesn't, the error block below the row will tell you
   what happened (invalid key, no credit, model not available, etc.).
5. **Optional**: change the model via the **Model** dropdown. The
   default for each provider is sensible; the curated list under it is
   what's known to work with Water's prompts.

Close Settings. You're ready to write.

## Create a project, write a scene

1. From the empty state, click **Create a new project** (left button)
   or **Open existing** (right button).
2. A project is just a folder on disk — pick anywhere you want.
3. Click **+ New scene** in the left rail to make a scene.
4. Optional: click the **⋯** next to a scene → opens the metadata
   sheet → link a character to the scene. **Character-linked scenes
   fire more triggers** (idle_pause_with_present_character,
   character_dissonance) than scenes without anyone in them.
5. Write. Pause for ≥ 3 s on a sentence end. Pills appear in the
   right margin.

## Obsidian compatibility

Water projects are markdown folders with YAML frontmatter — Obsidian
can open them directly as a vault. Scenes are tagged with
`water_scene: true` in their frontmatter so you can tell them apart
from regular notes in the vault.

Inside scene bodies, Water recognizes Obsidian-style wiki-links:
`[[Scene Name]]` or `[[Scene Name|alias text]]`. They round-trip
cleanly between Water and Obsidian.

## When things go wrong

- **No pills appearing.** Open Settings → check the provider dot. If
  it's gray, click Test. If the error block says 401, your key is
  invalid. If 402, your provider account has no credit / billing set
  up. If 429, you're rate-limited — wait a minute.
- **"Set up a provider" banner sticks around.** No provider has been
  Tested green. Click the banner button to open Settings.
- **App opens with a single scrollbar that looks weird, or a popup
  lands behind the panel.** Restart the app — likely a stale dev
  build. Shouldn't happen in installer builds.
- **Pills fire but talk about the wrong paragraph.** The pill text
  and the hover highlight should match. If they don't, send a
  screenshot — we have a recent fix here but edge cases may remain.

## What feedback to send

- **What didn't work** — error messages, friction, confusion. Settings
  is the most-touched surface; failures there block everything else.
- **What surprised you, good or bad.**
- **What pills felt useful vs. noise.** Water learns over a session
  (Settings → "Reset trigger learning" clears it), so the *first*
  hour matters most.
- **Anything that looked broken visually.**

Send screenshots if you can; they're worth a thousand words for layout
bugs especially.

## Reset

- **Reset trigger learning** (Settings): start fresh on the adaptive
  weights. Use if pills feel stuck.
- **Delete project**: just delete the folder. Water keeps no state
  outside it (other than the API key in your keychain — clear that via
  your OS keychain app if you want).
- **Uninstall**: Add/Remove Programs (Windows) or drag to Trash (Mac).

---

Thanks for testing.
