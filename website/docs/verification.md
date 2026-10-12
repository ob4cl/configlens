# Homepage verification

Verified locally on 2026-10-12 before publication. These are executed checks,
not performance or compatibility claims about upstream AI clients.

- Clean `npm ci --ignore-scripts` from committed lockfile; `npm audit`: zero vulnerabilities.
- `npm run lint`: formatting passes. `npm run typecheck`: zero errors/warnings.
- `npm test`: seven passing unit tests, including every source-inclusion combination,
  literal Unicode/markup, provenance, immutability and preset/CLI parity records.
- `npm run fixtures`: four observed `/model` winners verified against the actual
  ConfigLens CLI in an isolated bubblewrap sandbox. All remain partial, fixture-verified;
  no effective value or upstream runtime-conformance claim. Six terminal captures
  and the protected-value projection come from actual successful CLI executions.
- `npm run build` and `npm run test:build`: production output, subpath assets,
  anchors, repository targets, metadata/schema, OG dimensions and runtime-sink audit pass.
- `npm run test:browser`: Chromium at 1440, 1024, 768, 390 and 320 CSS pixels;
  no page overflow, console errors, unexpected requests or axe WCAG A/AA violations.
  Covers 20 interaction groups including keyboard/touch, all presets, edits, exclusion,
  reset, animation cancellation, reduced motion, disclosures, six terminal captures,
  three-platform download/verification controls and clipboard success/failure.
- No-JavaScript regression: platform selector disabled, fallback identified as Linux,
  and separate, correct Windows/macOS/Linux static download links remain available.
- Clipboard tests mock API success/failure; they do not certify an OS clipboard.
- Desktop and mobile screenshots visually inspected; crops are in `screenshots/`.
- Existing CLI unchanged: `cargo fmt --all -- --check`, Clippy with warnings denied,
  324 Rust tests and 17 Python release-tool tests pass locally.
- Pages workflow validated with `actionlint`; all six Actions pins verified against
  corresponding upstream release tags. Default permissions stay read-only;
  write/OIDC permissions belong only to the main-only deployment job.

An independent review found no concrete security defect but identified a no-JavaScript
platform-selection mismatch. This was fixed with progressive enhancement and static
platform links; the regression test now passes. The review's scope suggestion was
also applied: edited demo strings are literal, without interpolation/redaction simulation.

## Boundaries

Automated axe checks are not a complete WCAG certification. No physical iPhone,
Android device, screen reader, Firefox or Safari has been tested in this local pass.
Lighthouse and anonymous live-origin checks must be performed after deployment;
no unmeasured score is claimed here. Project-subpath `robots.txt` does not control
the origin-root robots policy. Binaries remain unsigned; macOS remains unnotarized.

The website introduces no analytics, API keys, external fonts, runtime AI requests,
configuration upload or backend. Browser edits stay in memory. CLI compatibility
coverage remains bounded; this page does not turn fixture support into runtime conformance.
