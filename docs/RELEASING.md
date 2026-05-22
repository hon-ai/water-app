# Releasing Water

Step-by-step for cutting a tester-facing release. Run through this
the first time end-to-end; later releases are `bump → tag → push`.

---

## One-time setup

These steps happen once per machine + once per repo. Skip the
machine-local ones if you're handing the release process to someone
else — they only need the repo-level secrets.

### 1. Generate Tauri signing keys

The auto-updater verifies installer signatures with an ed25519
keypair. The public key lives in `tauri.conf.json`; the private key
stays out of the repo and lives only in (a) your machine's keychain
and (b) GitHub Actions secrets.

```powershell
# Generate the keypair. Pick a strong passphrase and remember it.
pnpm --filter @water/app tauri signer generate -w "$HOME/.tauri/water.key"
```

The command prints **two values**:

- The **public key** (a base64 string starting with `dW50cnVzdGVk...`).
  Copy this into `app/src-tauri/tauri.conf.json` at
  `plugins.updater.pubkey`, replacing the placeholder.
- The **private key path** (`~/.tauri/water.key`). Keep this secret.

### 2. Set GitHub repo secrets

In your repo settings → Secrets and variables → Actions, add:

| Secret | Value |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | The full contents of `~/.tauri/water.key` (the file content, not the path). |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | The passphrase you set during `signer generate`. |

Optional (later):

| Secret | Value |
|---|---|
| `VITE_SENTRY_DSN` | Sentry DSN URL, if you've wired crash reporting. |

### 3. Update the updater endpoint

Open `app/src-tauri/tauri.conf.json` and replace
`{{REPLACE_WITH_OWNER}}/{{REPLACE_WITH_REPO}}` with your actual
GitHub user/org and repo name (e.g. `your-handle/water`). The
`endpoints` array now reads:

```json
"endpoints": [
  "https://github.com/your-handle/water/releases/latest/download/latest.json"
]
```

`tauri-action` will auto-attach the `latest.json` manifest to each
release; the running app fetches that URL on startup.

### 4. Update the landing page links

Open `docs/landing/index.html` and replace `REPO_OWNER/REPO_NAME`
everywhere (3–4 places) with your actual `user/repo`. Also swap the
`REPLACE_WITH_TALLY_ID` placeholder with your Tally form id, or
remove the iframe block if you'd rather collect feedback elsewhere.

Host the file anywhere static — GitHub Pages, Cloudflare Pages,
Vercel, Netlify. The `docs/` directory is GitHub Pages-friendly if
you turn it on in repo settings → Pages → Source = main / docs.

---

## Per-release process

Once setup's done, every release looks like this.

### 1. Smoke-test locally

Sanity-check that the dev session boots clean + pills fire:

```powershell
pnpm --filter @water/app tauri dev
```

Open Settings → Test a provider (green dot). Write a paragraph in a
scene. Verify a pill appears.

Close the dev window when done.

### 2. Run the full test suite

```powershell
cargo test --workspace
pnpm --filter @water/app test -- --run
```

Both should be green. CI runs these too but a local pass catches
"broke since last commit" before the tag push.

### 3. Bump the version

Edit three places — they need to match:

```jsonc
// app/package.json
"version": "0.1.0-alpha.2"
```
```jsonc
// app/src-tauri/tauri.conf.json
"version": "0.1.0-alpha.2"
```
```toml
# app/src-tauri/Cargo.toml
version = "0.1.0-alpha.2"
```

(There's an obvious "automate this" follow-up — `cargo-edit` +
`pnpm version` + a tiny bash script — when the cadence justifies it.)

### 4. Commit + tag

```bash
git add app/package.json app/src-tauri/tauri.conf.json app/src-tauri/Cargo.toml
git commit -m "chore: bump to 0.1.0-alpha.2"
git tag v0.1.0-alpha.2
git push origin main
git push origin v0.1.0-alpha.2
```

The tag push triggers `.github/workflows/release.yml`.

### 5. Wait for CI

Watch the Actions tab. The matrix has three runners (Windows /
macOS / Linux) and each takes ~10–15 minutes. They build the
installer for their platform and attach it to a **draft** GitHub
Release.

If a runner fails, the most likely culprits:

- **Cargo build error** — your local cargo passed but CI uses
  `--release`. Run `cargo build --release` locally to repro.
- **Frontend build error** — `pnpm --filter @water/app build` will
  surface it.
- **Signing key missing** — the build still succeeds without a key
  but produces no `.sig` files; the updater on installed Water
  refuses unsigned updates. Add the secret then re-run the workflow.

### 6. Review + publish the draft release

Go to repo → Releases. The draft has all four installer files:

- `Water_0.1.0-alpha.2_x64-setup.exe` (Windows NSIS)
- `Water_0.1.0-alpha.2_x64_en-US.msi` (Windows MSI)
- `Water_0.1.0-alpha.2_universal.dmg` (macOS, universal binary)
- `water-app_0.1.0-alpha.2_amd64.deb` + `.AppImage` (Linux)
- `latest.json` (updater manifest)

Edit the release notes — pull from your commit log since the last
release. Then click **Publish**.

The landing page's `releases/latest` link starts serving this
release immediately. The updater's `latest.json` URL too.

### 7. Notify testers

For closed alpha, a short email per tester:

> Subject: Water alpha v0.1.0-alpha.2 — small fixes + new ambient
> nudges
>
> Hi [name],
>
> New build is up at https://your-landing-page.example.com
>
> Highlights:
> - [bullet]
> - [bullet]
>
> If you already have a previous build installed, it'll pull this
> one in next time you launch — no action needed.

---

## Troubleshooting

### The release workflow says "no version specified"

The `tauri-action` step reads the version from `tauri.conf.json`
when `tagName` doesn't substitute. Make sure step 3 above bumped
all three files; otherwise the `Cargo.toml` and `tauri.conf.json`
will disagree and the build aborts.

### Testers see "SmartScreen blocked Water"

Until we add an EV code-signing certificate, every fresh installer
gets a SmartScreen warning on Windows. Tell them to click
**More info → Run anyway**. After installation the binary is
trusted; only the first launch hits the warning.

Same goes for macOS — System Settings → Privacy & Security has an
"Allow Anyway" toggle.

### A tester is stuck on an old version

The auto-updater fires once on app boot. If it failed (network blip,
endpoint mistyped, missing pubkey), they just open
https://your-landing-page.example.com and reinstall. The OS
installer handles version replacement.

### The sidecar didn't boot on a fresh install

Testers need `uv` on PATH. `TESTER.md` documents this; the app
gracefully degrades (five stylometric triggers stay dark) when uv
is missing. We're not yet bundling uv into the installer.

---

## Tier-2 polish, when you cross ~20 testers

- **Code-signing certificate** (Sectigo / SSL.com EV ~$200/year)
  removes SmartScreen.
- **macOS notarization** ($99/year Apple Developer Program) removes
  Gatekeeper.
- **PostHog free tier** for product analytics — how testers actually
  use Water (which providers, which triggers fire most, etc.).
- **Discord / Slack** for the tester community so feedback is
  socialized, not just emailed.
