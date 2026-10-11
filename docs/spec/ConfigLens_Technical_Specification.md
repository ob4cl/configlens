# ConfigLens — Technical specification and implementation plan

Version: proposed 0.1 specification • Research checked: 10 October 2026
Status: design deliverable; no implementation, package availability or compatibility certification is implied.

## 1. Product decision

**ConfigLens explains why an AI coding tool uses a configuration value, where it came from, and what could override it.**

Build a local, read-only Rust CLI with deterministic resolution, source provenance, safe diagnostics and explicitly bounded compatibility. Start with OpenCode local configuration and a bounded Claude Code settings adapter. Add Codex and Gemini after the core model is proven. Four fully accurate adapters, instructions, enterprise policy and packaging are too much for a credible first month; ship two bounded adapters well.

Primary users: developers switching AI clients, maintainers supporting teammates, and teams diagnosing unexpected models, permissions or MCP configuration. Common questions: “Why did my model change?”, “Why is this setting ignored?”, “Which file should I edit?”, and “Is my report safe to attach to an issue?”

The strongest demo is a real overridden setting traced from its original file to its winning source. Rust alone will not create reputation. Correct semantics, reproducible fixtures, good explanations and honest limitations are the engineering substance.

### Goals

- Explain supported configuration keys with an ordered event trace, including ignored sources.
- Preserve original client namespaces; do not pretend different permission models are equivalent.
- Separate configuration defaults, instruction assembly and enforced policy.
- Work offline without credentials, accounts, model calls, telemetry or subprocess execution.
- Produce safe terminal output and stable machine-readable reports.
- Make adapter behaviour auditable against pinned upstream evidence.

### Non-goals for 0.1

No configuration rewriting, automatic fixes, agent launcher, live process inspection, MCP connections, plugin loading, remote configuration fetching, complete permission evaluator, TUI, LLM explanation or universal instructions translator. Do not claim to reproduce an already running agent session. Instructions discovery is 0.2 inventory work, not semantic conflict detection.

## 2. Evidence and competitive differentiation

Sources below were retrieved on the research date. Documentation is mutable; implementation must pin exact upstream versions and commits. “Current docs” is not a compatibility range.

| Alternative | Verified role | ConfigLens differentiation |
|---|---|---|
| Ruler [S1] | Applies shared rules across coding agents, with configuration generation and propagation | Inspect existing native files without requiring a new source-of-truth format; explain per-key origin and shadowing |
| Rulesync [S2] | Generates, imports and converts AI tool configurations | Diagnose the generated files alongside user, local and managed sources; complement rather than replace its authoring workflow |
| Client-native configuration UI [S3, S6] | Claude and Gemini expose configuration/settings interfaces | Cross-client report structure, offline fixtures, shareable explanations and explicit evidence coverage |
| jq, generic JSON/TOML viewers | Inspect or transform data | Encode client-specific discovery, merge and applicability rules rather than just print files |
| ConfigLens itself | Proposed name | Name, crates.io availability, trademarks and direct competing projects remain release-time checks |

A primary issue in Claude Code reports inconsistent documented managed-versus-CLI precedence [S7]. Another reports missing MCP deduplication and precedence documentation [S8]. These demonstrate real explanation problems, but not market size or willingness to install ConfigLens. An exploratory study analyses configuration mechanisms across coding tools [S9]; it establishes complexity, not demand for this specific product.

**Positioning hypothesis:** configuration authoring tools leave room for an independent provenance debugger. This is not proof that no competitor offers similar diagnostics. Before substantial expansion, test with five maintainers and ten developers using multiple clients. Ask each to reproduce a recent confusing setting. A useful validation result is several users solving an actual incident faster with a trace, not stars on an announcement.

### Reputation and adoption

Use a 20-second terminal demo; downloadable binaries; truthful adapter coverage; a clean README; examples that work without installing an AI client; and a fixture-backed compatibility registry. Measure successful diagnoses, repeat users who volunteer feedback, integration PRs and issue resolution. Stars are secondary signals; no numerical star forecast is justified.

## 3. Supported-client research and release scope

All precedence sequences below run **lowest to highest** unless explicitly stated. They are research summaries, not substitutes for version-specific tests.

| Client | Researched configuration behaviour | Proposed support |
|---|---|---|
| Claude Code [S3] | JSON user, shared project, local and managed settings; session inputs also matter. Lists can combine, some keys have special scope/merge rules, and environment precedence is pair-specific | 0.1: verified scalar model and selected display settings, file inventory, selected list provenance only once tested. Managed/runtime gaps always visible |
| OpenCode [S4] | JSON/JSONC; remote defaults → global → custom path → project → .opencode directories → inline content → managed files → macOS managed preferences. Conflicting keys override; unrelated keys survive | 0.1: verified local scalar settings and nested object merges. Remote, dynamic directory contributions and MDM are unresolved unless a valid snapshot is supplied |
| Codex [S5] | Built-ins → Unix system → cloud defaults → user → selected profile file → trusted project layers root-to-cwd → flags/config overrides. Requirements constrain defaults separately | 0.2: TOML, trust gating, current profile-file model, model/provider and selected sandbox/approval defaults. Enterprise enforcement separate |
| Gemini CLI [S6] | Built-ins → system defaults → user → project → system settings → applicable environment → arguments; JSON files, platform-specific system paths | 0.2: settings files, validated key-specific overrides and trust behaviour. .env and policy engine need separate verification |
| Aider [S10] | Official configuration guide exposes command-line, YAML and .env configuration | Backlog: do not implement discovery or merge order from memory; inspect loader and add conformance evidence first |
| Cursor, Copilot CLI, Cline, Roo, Hermes | Not sufficiently investigated for this specification | Candidate backlog, explicitly unsupported; no compatibility claim based on shared file names |

### Essential adapter discipline

1. Never reuse one global precedence list for all clients.
2. Never equate permission-list concatenation with permission authorization.
3. A file containing `env` is not the same as the process environment.
4. A missing managed file does not prove that enterprise policy is absent.
5. An instructions file is text supplied to a model, not a map of executable settings.
6. Unknown settings must be preserved as opaque inventory entries, not silently discarded or resolved with generic semantics.
7. Client binaries are not executed even for `--version`; users supply version context.
8. Unknown defaults remain unknown; provider/account-specific defaults must not be guessed.

### Compatibility registry

Each adapter release carries `adapter_revision`, explicit tested client versions, source URLs, upstream commit SHA where available, verification date, platform and per-key support. Do not publish broad version intervals until tested boundaries justify them. Unknown client version: mark output provisional; `--strict` fails supported-value assertions. Untested version: inventory remains available but exact resolution claims become unsupported.

Required support states: `inventory_only`, `documented`, `fixture_verified`, `upstream_conformance_verified`, `unsupported`. Documentation and fixture tests alone do not establish runtime equivalence. Every advertised merge rule requires a rule ID, evidence reference and test IDs.

## 4. CLI contract

Binary/package working name: `configlens`.

```sh
configlens clients
configlens scan --client claude --cwd . --client-version <tested-version>
configlens explain /model --client claude --cwd . --client-version <tested-version>
configlens explain /model --client opencode --input-context ./context.json
configlens doctor --client opencode --format json --strict
configlens snapshot --client claude --output ./before.json
configlens diff ./before.json ./after.json --format text
configlens schema --kind report
```

All commands and output shown here are proposed contracts. Use RFC 6901 JSON Pointer keys (`/model`, `/mcp/example/enabled`) rather than ambiguous dotted keys. Client-native field names remain unchanged.

| Command | Behaviour |
|---|---|
| `clients` | Report implemented adapters and supported keys/versions; unsupported clients are labelled clearly |
| `scan` | Inventory candidate sources, applicability, readable state and verified settings; no recursive repository crawl |
| `explain KEY` | Show observed definitions, transformations, winner or unresolved result, and relevant coverage gaps |
| `doctor` | Parse/schema/applicability/coverage diagnostics. Never label an entire configuration “secure” |
| `snapshot` | Write the same redacted report schema used by JSON output, with explicit destination; no raw content |
| `diff` | Compare two reports from the same client and compatible schema; distinguish value, provenance and coverage changes |
| `schema` | Emit bundled schema without network access |

### Shared options

`--client` required for resolution; `scan` alone may inventory known client markers. `--cwd PATH` defaults to current directory. `--client-version VERSION` supplies claimed runtime version. `--format text|json` defaults to text. `--strict`, `--no-env`, `--input-context PATH`, `--color auto|always|never`, `--paths local|share` and `--allow-read-root PATH` are supported. `--output` exists only for snapshot. Honour `NO_COLOR` and non-TTY defaults.

Use captured process environment only for a finite adapter allowlist. `--no-env` disables it. Context file data explicitly overrides captured context; mark it `simulated`. Do not accept arbitrary key/value overrides on the command line, where secrets enter shell history. Client arguments can be represented in the context file; ConfigLens never launches them.

### Input context

```json
{
  "schema_version": "1.0",
  "client": "opencode",
  "client_version": "EXACT_TESTED_VERSION",
  "environment": {},
  "client_arguments": [],
  "project_trust": "unknown",
  "sources": []
}
```

`client_arguments` is an argv array, not shell text. Adapters recognize only verified supported options; unknown flags make relevant resolution partial. `sources` can reference allowed local policy/remote snapshots with declared role, producer and version. A supplied snapshot is user-provided evidence, not proof that the runtime consumed it. Reject mismatched client/version and duplicate source identity. Context files are themselves sensitive inputs.

### Text example (synthetic fixture)

```text
/model = model-b
Observed winner: $PROJECT/.claude/settings.local.json:2:12
  model-a  $PROJECT/.claude/settings.json:2:12  replaced by local setting
  model-b  $PROJECT/.claude/settings.local.json:2:12  observed winner
Rule: claude.model.local-over-project
Coverage: partial — runtime arguments and managed sources not supplied
Conclusion: model-b wins among observed applicable sources.
```

If a potentially higher-priority source is unavailable, label this an **observed winner**, not the runtime effective value. Where verified rules guarantee the missing layer cannot change this key, completeness may be determined per key.

### Exit codes

0: successful report, ordinary non-fatal findings permitted. 1: doctor findings meet `--fail-on warning|error` (default error). 2: usage or incompatible input/report. 3: requested key/client semantics unsupported or strict mode detects incomplete context. 4: required file inaccessible, malformed or over resource limit. Priority if several apply: 2 → 4 → 3 → 1 → 0. JSON commands emit one valid report envelope on stdout, including structured fatal errors; stderr contains only sanitized operational notices. No ANSI in JSON. Broken pipe exits without backtrace.

## 5. Resolution model and data contracts

A naive `deep_merge` is insufficient. Separate discovery, parsing, applicability, key-specific reduction and policy constraints.

### Pipeline

1. Construct explicit resolution context: OS, cwd, root, home, approved paths, client version, trust and supplied session data.
2. Discover only adapter-known sources. Record expected-but-unobservable sources.
3. Open bounded regular files; parse with spans; preserve duplicate-key errors and unknown keys.
4. Determine whether each source applies; do not remove ignored sources from the report.
5. Select a verified rule per key and version; order applicable definitions.
6. Execute typed reduction events while retaining provenance for every contribution.
7. Apply verified constraints in a distinct policy phase. Otherwise annotate unresolved enforcement.
8. Calculate per-key completeness, support level and confidence.
9. Sanitize/redact into a public report; render only that report.

### Internal types

```rust
struct SourceId(u32);
struct DefinitionId(u32);
struct Span { start_byte: usize, end_byte: usize }
struct Definition {
    id: DefinitionId,
    source: SourceId,
    pointer: String,
    value: SensitiveValue,
    span: Option<Span>,
}
// Sketch: names and ownership may evolve without changing report contracts.
enum ResolutionEvent {
    Selected { definition: DefinitionId, rule_id: String },
    Replaced { earlier: DefinitionId, later: DefinitionId, rule_id: String },
    Contributed { definition: DefinitionId, rule_id: String },
    Ignored { definition: DefinitionId, reason_code: String },
    Constrained { policy_source: SourceId, rule_id: String },
    Unresolved { reason_code: String },
}
```

Objects, arrays and scalar replacement must retain child-level events. An array result needs per-element source references; duplicate handling and order depend on the adapter rule. Supported operators: scalar replacement, recursive object merge, ordered replacement, stable concatenation, verified deduplication and explicit client-specific reducers. Do not assign an operator to an unknown key. `null`, empty lists and missing values are different inputs. Invalid higher layers do not silently become absent layers.

### Public report schema 1.0

Required fields: `schema_version`, `tool_version`, `client`, `adapter_revision`, `compatibility`, `context`, `sources`, `resolutions`, `diagnostics`, `coverage`, `redaction` and `errors`. Use sorted maps and stable deterministic source/event ordering. Omit timestamps from normal reports; optional timestamp excluded from diffs. Paths get display aliases in share mode; machine-specific path bytes never become stable IDs.

For each resolution: pointer, `observed_value`, `effective_value` (only present when justified), `status`, contributions, rule IDs, evidence refs, coverage gaps and support level. Status is `resolved`, `partial`, `unsupported`, `invalid` or `absent`. Confidence labels reflect evidence class, not an invented probability.

A secret value is represented as `{"redacted":true,"reason":"sensitive_field"}`. Diff reports say `comparison:"unknown_redacted"` for protected values; two masks do not prove equality. Do not export hashes of secrets for comparison. Source-level public IDs identify location roles, not content fingerprints.

## 6. Rust architecture

Use one crate with a library and binary for 0.1; split into a workspace only if boundaries justify it. No async runtime or network stack is necessary.

```text
Cargo.toml / Cargo.lock / rust-toolchain.toml
src/lib.rs, main.rs, cli.rs
src/context.rs, discovery.rs, filesystem.rs
src/parse/{mod,json,jsonc,toml}.rs
src/model/{mod,source,definition,event,report}.rs
src/resolve/{mod,operators,coverage}.rs
src/adapters/{mod,claude,opencode}.rs
src/security/{mod,redact,sanitize,path_policy}.rs
src/render/{mod,text,json}.rs
src/doctor.rs, snapshot.rs, diff.rs
tests/{cli,security,discovery,conformance}/
fixtures/{claude,opencode}/
docs/{semantics,compatibility,report-schema,security}.md
compatibility/{claude,opencode}/
benches/resolution.rs
.github/workflows/{ci,release}.yml
README.md, CONTRIBUTING.md, SECURITY.md, CHANGELOG.md
LICENSE-MIT, LICENSE-APACHE
```

Dependencies to evaluate at implementation time: clap/clap_complete for CLI, serde/serde_json for serialization, a span-preserving JSON/JSONC parser such as jsonc-parser, toml_edit for TOML, thiserror for typed failures, directories for platform paths, semver, zeroize where useful, tracing only with safe structured fields. Tests: tempfile, assert_cmd, predicates, insta, proptest, Criterion and cargo-fuzz. Validate current versions, APIs, licences and maintenance before pinning. Do not use a JSON parser that silently loses duplicate-key evidence; write a detection pass if necessary. Avoid regex comment stripping for JSONC.

Adapter interface:

```rust
trait ClientAdapter {
    fn metadata(&self) -> AdapterMetadata;
    fn discover(&self, ctx: &ResolutionContext) -> DiscoveryPlan;
    fn classify(&self, source: &ParsedSource, ctx: &ResolutionContext)
        -> Applicability;
    fn rule_for(&self, pointer: &str, version: &VersionContext)
        -> RuleSupport;
    fn resolve(&self, inputs: &ResolutionInputs) -> ClientResolution;
}
```

Inject `FileAccess` and captured context rather than reading the real environment in pure reducers. Adapter code has no direct stdout, subprocess or network access. Generic operators must not encode client policy. Keep unredacted values out of `Debug`, `Display`, tracing and serializable public structs. `SensitiveValue` deliberately has no public Serialize implementation. Only the redaction projection creates report values.

### Paths and platforms

Resolve home and XDG paths through adapter-aware rules, not universal assumptions. Handle Windows drive roots, UNC paths, case-insensitive identity where applicable, macOS paths and WSL as separate environments. Preserve OS paths internally; JSON escapes a safe display form. Repository root discovery supports `.git` directory and worktree `.git` file without invoking Git or reading hooks. If root cannot be safely identified, require `--project-root` or mark boundary uncertain. Upward discovery ends at the verified root or adapter boundary; never recurse through unrelated repositories.

## 7. Security and redaction specification

Threat model: malicious repository configuration, accidental secret disclosure in support reports, misleading partial results, unsafe traversal and dependency compromise. ConfigLens is a parser, not an agent executor.

### Mandatory rules

- No networking, subprocesses, shell expansion, hooks, plugins, MCP startup, credential helpers, keychain reads or auth-store reads. `$schema` references are descriptive and never fetched.
- Open only known config paths under explicit project/home/system roots. External file references require `--allow-read-root`; show blocked references without reading them. Never evaluate shell substitutions.
- Parse env/file interpolation only when adapter-verified. Expansion uses bounded literal references; redaction taint propagates into derived values. Unsupported interpolation remains unresolved.
- Remote, MDM and cloud state are not assumed absent. Native registry/keychain access is deferred; supplied snapshots remain explicitly unverified runtime inputs.
- Secrets: redact credentials, token/key/password fields, authorization/cookie headers, entire MCP environment value maps, arbitrary environment values and unknown free-text fields by default. Public values are allowlisted per adapter (for example verified model names and booleans).
- Redact URL userinfo, query values and fragments. In share mode pseudonymize private hosts, MCP names, user/repo paths and identifiers. In local mode show config locations for diagnosis but still redact secret values.
- Do not expose instructions bodies, command arguments, hook bodies, plugin options, raw source excerpts, credential file names containing secrets or inline-context content in reports. Show role and location only where safe.
- Unknown keys themselves may contain credentials: substitute opaque field IDs in share reports; do not expose arbitrary untrusted object-key strings.
- No global `--show-secrets` option in 0.1. Redaction overrides can only make output stricter.
- Sanitize terminal control sequences (including OSC hyperlinks), carriage returns, bidi controls and untrusted newlines. Apply to errors, paths, pointer displays, help-context errors and all user-derived strings.
- Never emit parser error source excerpts or raw clap argument echoes from sensitive context. Diagnostics use code, source role and line/column. Panic hooks must suppress input content.

### Resource and file safety

Defaults: 2 MiB/file, 16 MiB total bytes, 128 sources, nesting depth 64, include depth 8 and 2,000 trace events/key. Hitting a limit gives a diagnostic and invalid/partial result; no silent truncation. Reject devices, sockets, FIFOs and directories as config files. Read through a bounded handle and validate file metadata; avoid “size check then unbounded read.”

Detect symlink/junction loops. Follow links only if the resulting regular file remains within an authorized root; implement platform-aware handle checks to reduce traversal races. Do not claim complete TOCTOU immunity across all OSes. Treat file changes observed during scanning as inconsistent input rather than an atomic runtime snapshot.

Snapshot output: explicitly requested path, create-new by default, atomic write via same-directory temporary file, owner-only permissions on Unix; Windows use restrictive ACL where implemented or clearly report inability. `--overwrite` must be explicit; reject output symlink targets. Redacted output is still potentially private. No automatic snapshots, caches or persistent log files.

## 8. Test and validation plan

| Layer | Required cases | Release criterion |
|---|---|---|
| Parsers | JSON/JSONC comments and strings, trailing comma parity, duplicates, malformed input, spans, Unicode, limits | No silent duplicate loss; reliable locations or explicit unavailable span |
| Discovery | cwd depth, home overrides, XDG, worktrees, Windows/UNC, missing/unreadable file, blocked symlink, no markers | Bounded reads and correct applicability fixtures |
| Reducers | Scalar/object/list/null/missing, replacement chains, scope exclusions, unknown keys | Every advertised rule has positive, negative and boundary cases |
| Coverage | Missing runtime arguments, MDM, remote defaults, unknown trust/version | No false complete/effective claim |
| Security | Seeded secrets in fields, keys, paths, URLs, args, malformed JSON, env, errors and malicious terminal escapes | Seed values absent from stdout, stderr, snapshot, diff and debug output |
| CLI | Exit codes, strict, JSON, NO_COLOR, non-TTY, unknown pointer, incompatible snapshots | Stable envelopes and consistent exit-code precedence |
| Conformance | Pinned upstream loader output against the same synthetic fixtures where safe | Exact match for advertised supported rules; otherwise scope downgraded |
| Portability | Linux, macOS and Windows runner tests | All promoted release targets pass |

Property tests: deterministic output for identical context, redaction idempotence, irrelevant definitions do not affect supported resolutions, every surviving contribution has provenance, no resolved status when a relevant required source is missing. Replacement-specific properties must not be incorrectly imposed on union rules.

Fuzz parsers, JSON Pointer handling, reducers, sanitization and bounded traversal helpers. Check no panics and no unbounded growth. Run short fuzz smoke in CI; longer scheduled runs outside PR latency budget.

Conformance testing is a maintainer task separate from normal ConfigLens execution: use pinned upstream source or a safe loader-only harness in an isolated synthetic home, blocked network and no real credentials. Do not launch a complete agent or connect MCP servers. Claude runtime internals may not be available for a safe harness; retain documented/fixture-verified status instead of fabricating parity.

CI: fmt check, clippy with warnings denied, locked tests on three OSes, declared MSRV build, bundled JSON-schema validation, secret leak cases, dependency audit/licence checks and release asset smoke tests. Use least-privilege workflows and pinned action commits. No publishing credentials in pull-request jobs.

Performance target, not benchmark claim: under 150 ms warm run for 20 small files and 1,000 definitions on a named laptop; memory below 50 MiB. Publish hardware, fixture size and measured distributions before adding speed claims. Correctness wins over parallel discovery.

## 9. Acceptance examples

1. Two supported scalar definitions: local source replaces project source; both locations and the replacement reason remain visible.
2. Known list contributions: explain each contributing element with the documented ordering and dedup rule; do not assert permission authorization.
3. Unknown managed/runtime context: observed winner displayed, effective value withheld, strict exits 3.
4. Unsupported client/version/key: inventory available but no invented merge result.
5. Project untrusted where adapter rules require trust: record skipped project layer and explain why; unknown trust remains conditional.
6. Malformed high-priority file: error with safe location; never quietly claim lower-priority value is runtime effective.
7. Secret in malformed input or object key: no seeded secret in any output channel.
8. Two redacted snapshots: report protected comparison unknown rather than unchanged.
9. No config files: valid empty inventory, clearly distinguished from unobservable enterprise/runtime state.
10. Runtime arguments supplied via context: recognized flags affect verified keys; unknown flags introduce coverage gaps.

## 10. Proposed README (adapt after implementation)

The following is a complete starting draft. Replace planned capabilities with actual support before release; do not present it as a working repository today.

---

# ConfigLens

**Understand which AI configuration wins—and why.**

ConfigLens is a local Rust CLI that traces supported AI coding-tool settings through configuration files and supplied session inputs. It shows where a value came from, what replaced it and what could still change it.

[Insert a real terminal recording from the bundled demo fixture.]

### Quick start

Build from a checkout until a package is published:

```sh
cargo build --release --locked
./target/release/configlens clients
./target/release/configlens explain /model --client opencode --cwd ./examples/demo --client-version <supported-version>
```

### What you get

- Source locations and replacement/contribution traces for supported keys.
- Client-specific rules and explicit compatibility information.
- Redacted text and JSON reports.
- Coverage warnings when runtime or managed state is unavailable.
- No network requests, model calls, telemetry or execution of your configuration.

### What “effective” means

ConfigLens statically analyses the inputs it can read. A result is effective only within a complete verified context. Otherwise it shows an observed winner or an unresolved result. It cannot inspect an existing session or guarantee what a remote administrator supplied.

### Commands

```sh
configlens scan --client claude
configlens explain /model --client claude
configlens doctor --client opencode --strict --format json
configlens snapshot --client claude --output before.json
configlens diff before.json after.json
```

Run `configlens clients` for the current support matrix. Only implemented, fixture-tested commands appear in release documentation.

### Privacy

Secret fields and arbitrary sensitive text are redacted. Reports can still describe your project structure; use `--paths share` before attaching one to a public issue. Never paste raw credential files into an issue. ConfigLens does not connect to MCP servers or run hooks/plugins.

### Compatibility and limits

See `docs/compatibility.md` for tested versions and keys, and `docs/semantics.md` for rule evidence. Unknown versions and missing policy/session context produce partial results. Instructions and permission enforcement are distinct from settings resolution.

### Contributing

Good first contributions include synthetic fixtures, clearer diagnostics and version-specific rule evidence. Every adapter change needs a primary source, a declared compatibility scope and positive/negative tests. Do not submit real secrets or proprietary instructions.

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked
```

Report vulnerabilities through the private channel specified in `SECURITY.md` once configured. Licence: MIT OR Apache-2.0.

---

## 11. Research sources and evidence boundaries

[S1] Ruler primary repository: https://github.com/intellectronica/ruler

[S2] Rulesync primary repository: https://github.com/dyoshikawa/rulesync

[S3] Claude Code settings files/precedence: https://code.claude.com/docs/en/settings

[S4] OpenCode configuration: https://docs.opencode.ai/docs/config/

[S5] Codex configuration basics (official entry; redirects to ChatGPT Learn): https://developers.openai.com/codex/config-basic

[S6] Gemini CLI configuration: https://geminicli.com/docs/reference/configuration/

[S7] Claude Code issue #19369, inconsistent documented managed/CLI hierarchy (19 January 2026): https://github.com/anthropics/claude-code/issues/19369

[S8] Claude Code issue #39109, MCP precedence/deduplication documentation request: https://github.com/anthropics/claude-code/issues/39109

[S9] Galster et al., “Configuring Agentic AI Coding Tools: An Exploratory Study” (16 February 2026): https://arxiv.org/abs/2602.14690

[S10] Aider configuration entry guide: https://aider.chat/docs/config.html

Research limitations: this design uses live primary documentation, primary repository descriptions and reported issues. It does not pin loader commits, reproduce installed-client behaviour, establish full enterprise semantics, verify package-name availability or prove broad demand. Those are explicit implementation/validation gates. Product boundaries, architecture, estimates and launch strategy are recommendations; source summaries are verified documentation observations. No star-growth promises or unsupported competitor feature-absence claims are made.
