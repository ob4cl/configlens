# ConfigLens homepage

Static Astro + strict TypeScript, with local CSS and no UI framework, remote font,
analytics, API, config upload, client execution, or runtime third-party request.
Production URL: `https://ob4cl.github.io/configlens/`; base path: `/configlens/`.

## Requirements and build

Use Node 26.7.0 and npm 11.19.0 to match the verified build and CI.
Dependencies are pinned in `package-lock.json`. Astro build telemetry can be disabled
with `ASTRO_TELEMETRY_DISABLED=1` (used for verification).

```sh
npm ci --ignore-scripts
npm run typecheck
npm run lint
npm test
npm run build
npm run test:build
npm audit
```

`dist/` is the production artifact and is ignored. Publish its contents at the base
path above. All copy and the default example are pre-rendered; JavaScript enhances
controls. No server is needed for verification below. For interactive local development,
maintainers may run `npm run dev`; to preview the production build, run
`npm run build && npm run preview`. The verification workflow does not start either
server; it uses an intercepted browser. In automation, do not start a development
server on the user's behalf.

## Serverless real-browser checks

```sh
# Browser installation, if needed (downloads only; does not start a server):
npx playwright install chromium
npm run test:browser
# Or select an existing Chromium executable:
CHROMIUM_PATH=/absolute/path/to/chrome npm run test:browser
# Optionally choose where screenshots / JSON evidence are written:
SCREENSHOT_DIR=/absolute/output/path npm run test:browser
```

`tests/browser.mjs` launches headless Chromium over a debugging pipe and fulfills
all page requests directly from `dist/` with Playwright route interception at
`https://configlens.test/configlens/`. Unknown/missing requests are blocked and
fail the test. It starts no HTTP, dev, preview, API or test server. Browser transport
is not a listening HTTP server. It checks 1440, 1024, 768, 390 and 320px layouts,
horizontal overflow, axe WCAG A/AA, presets, editing, markup inertness, inclusion,
empty state, resetting, stale-animation cancellation, reduced motion, pipeline,
terminal commands, clipboard success/failure, privacy, install selectors, mobile
keyboard/Escape focus, and no-JavaScript content. Clipboard is mocked for repeatable
success/failure; actual OS clipboard integration is not claimed.

By default evidence goes to `$TMPDIR/configlens-website-screenshots` (OS temporary
directory fallback). This implementation was verified with an existing Chrome 152
binary, supplied through `CHROMIUM_PATH`. No live PageSpeed/Lighthouse score or
cross-engine result is claimed. Those checks belong to deployment verification.

## Fixture accuracy and reproducible captures

The browser resolver intentionally models **only `/model` scalar replacement** in
these included file sources, in low-to-high order:

- Claude Code 2.1.296: user → project shared → project local.
- OpenCode 1.18.35: global → project ancestor → nested project. The explicit boundary
  is `$PROJECT`; the working directory is `$PROJECT/packages/app`.

This is a browser fixture simulation, not the Rust CLI, a local scan, or an upstream
loader emulator. All conclusions remain observed / partial / fixture_verified;
effective values are withheld. Edited values are literal strings: interpolation and
redaction are outside this demo, so edited results are not CLI-parity claims.
No universal environment-variable layer is implied.
OpenCode 2.x is not supported. Inclusion means the selected source omits `/model`,
not that an unreadable source is silently skipped. Reset restores the selected preset.
The inspector intentionally displays only the scalar teaching subset; the separate
CLI capture adds a synthetic protected environment key for the privacy example.

The pinned public release is v0.1.0-beta.3 (prerelease, unsigned; macOS unnotarized).
All three archive URLs were checked against that tag's GitHub release metadata.
This site does not advertise the beta as the latest stable release.

To regenerate **real**, full CLI captures and cross-check all four fixture winners:

```sh
# Linux/WSL, Python 3, bubblewrap and an existing built ConfigLens binary required.
# This command never builds the Rust crate or launches any client.
CONFIGLENS_BIN=/absolute/path/to/configlens npm run fixtures
npm test
```

Default binary: `../target/debug/configlens`. The capture script deliberately fails
if bubblewrap is missing: it never silently downgrades to an unisolated invocation.
It uses cleared environment, synthetic HOME/XDG/cwd, explicit project boundary,
trusted simulated input context and approved sandbox root. Bubblewrap exposes the
host read-only, permits writes only under the throwaway sandbox, substitutes an
empty `/etc` so host managed configs cannot leak into results, and unshares network.
The CLI reads adapter-known synthetic paths only. No credentials are used.

`src/data/captures.json` records binary version/hash, four parity outcomes, exact
commands, full stdout/stderr and exit codes for clients/scan/explain/doctor/snapshot/
diff, plus the actual protected-key public projection. Only the ephemeral sandbox
prefix is normalized to `$SANDBOX`; this is labelled on the page. Command arguments
are not hidden. A copied captured command requires that sandbox setup; use the
separate first-command example for ordinary usage. Snapshot and report output
commands write to requested paths; diff reads reports. Captures are reproducible
for the same binary/platform, not asserted byte-identical across platforms.

## Source layout

- `src/components/`: semantic homepage sections.
- `src/styles/global.css`: layered graphite/cyan/violet design tokens and responsive CSS.
- `src/lib/resolution.ts`: pure scalar resolver; unit-tested independently of the DOM.
- `src/lib/fixtures.ts`, `src/data/fixtures.json`: typed fixture contract and source roles.
- `src/lib/motion.ts`: cancellable user-initiated timelines; immediate reduced motion.
- `src/scripts/`: safe DOM text updates and progressive interaction setup.
- `scripts/capture-fixtures.py`: CLI capture and independent parity assertions.
- `tests/`: Node unit tests and serverless Playwright/axe verification.
- `public/`: layered-source logo/favicon, 1200×630 OG PNG/source SVG, sitemap, robots.

Inputs are rendered with `textContent`/DOM nodes, never HTML interpolation. No data is
persisted or transmitted. The only `set:html` is fixed build-time JSON-LD. All external
navigation is explicit user-initiated GitHub navigation. Native select, checkbox,
button and disclosure controls are keyboard operable. System fonts avoid font loads.

## Maintainer handoff

The root Rust implementation is unchanged. `.github/workflows/website.yml` runs
strict validation, regenerates CLI fixtures in a sandbox, builds, and tests the
production artifact with Playwright and axe. Only main uploads a Pages artifact
and deploys, using a main-only `github-pages` environment and SHA-pinned Actions.
The Actions allowlist already permits GitHub-owned Actions and is not broadened.
To exercise the same browser suite against the deployed origin, use
`LIVE_SITE_URL=https://ob4cl.github.io/configlens/ npm run test:browser`.
This switches off local interception. Clipboard tests mock success/failure;
real clipboard integration still requires a browser permission/user-gesture check.
GitHub Pages root-level robots directives and security headers are hosting concerns;
a robots file below a project subpath is provided but is not a substitute for an
origin-root robots file. HTTPS and response headers must be verified after deployment.
