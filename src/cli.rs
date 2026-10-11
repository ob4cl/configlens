//! Command-line contract (spec §4) and the sanitized error boundary.

use std::collections::BTreeMap;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::adapters::{adapter_for, Client};
use crate::context::{parse_supplied_context, FailOn, ResolutionContext, Trust, VersionContext};
use crate::error::{arbitrate, CliError, ErrorDto, Outcome};
use crate::filesystem::RealFileAccess;
use crate::model::report::{
    build as build_report, Compatibility, EvidenceReport, Report, ReportInput, REPORT_SCHEMA_JSON,
};
use crate::render::{json as render_json, text as render_text};
use crate::resolve::{Engine, GapKind, RunOutput, Severity};
use crate::security::{PathMode, PathPolicy};

/// Environment variables ConfigLens may read: a finite adapter allowlist (spec §4).
pub const ENV_ALLOWLIST: &[&str] = &[
    "HOME",
    "XDG_CONFIG_HOME",
    "PROGRAMDATA",
    "USERPROFILE",
    "CLAUDE_CONFIG_DIR",
    "OPENCODE_CONFIG",
    "OPENCODE_CONFIG_DIR",
    "OPENCODE_CONFIG_CONTENT",
    "OPENCODE_TUI_CONFIG",
    "ANTHROPIC_MODEL",
    "ANTHROPIC_DEFAULT_MODEL",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Format {
    Text,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ColorChoice {
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PathsChoice {
    Local,
    Share,
}

#[derive(Parser, Debug)]
#[command(
    name = "configlens",
    version,
    about = "Explain which AI configuration value wins, and why",
    long_about = "ConfigLens is a local, read-only CLI that traces supported AI coding-tool settings \
                  through configuration files and supplied session inputs. It never executes your \
                  configuration, makes network requests or reads credentials.",
    after_help = "Examples:\n  \
                  configlens clients\n  \
                  configlens scan --client claude --cwd . --client-version 2.1.296\n  \
                  configlens explain /model --client claude --paths share\n  \
                  configlens doctor --client opencode --format json --strict\n  \
                  configlens snapshot --client claude --output before.json\n  \
                  configlens diff before.json after.json\n  \
                  configlens support-report --client claude --preview\n  \
                  configlens completions --shell bash"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Report implemented adapters and supported keys/versions.
    Clients(ClientsArgs),
    /// Inventory candidate sources, applicability and readability.
    Scan(CommonArgs),
    /// Explain one configuration key.
    #[command(after_help = "Examples:\n  \
                            configlens explain /model --client claude\n  \
                            configlens explain /permissions/allow --client claude --format json\n  \
                            configlens explain /mcp/jira/url --client opencode --paths share")]
    Explain(ExplainArgs),
    /// Parse/schema/applicability/coverage diagnostics.
    Doctor(DoctorArgs),
    /// Write the redacted report to an explicit destination.
    Snapshot(SnapshotArgs),
    /// Compare two reports from the same client and schema.
    Diff(DiffArgs),
    /// Emit the bundled report schema.
    Schema(SchemaArgs),
    /// Emit a shell completion script (offline; nothing is written).
    #[command(after_help = "Examples:\n  \
                            configlens completions --shell bash\n  \
                            source <(configlens completions --shell bash)")]
    Completions(CompletionsArgs),
    /// Build a sanitized, share-mode report to attach to a support issue.
    #[command(after_help = "Examples:\n  \
                            configlens support-report --client claude --preview\n  \
                            configlens support-report --client claude --output support.json")]
    SupportReport(SupportReportArgs),
}

#[derive(Args, Debug)]
pub struct ClientsArgs {
    #[arg(long, value_enum, default_value_t = Format::Text)]
    pub format: Format,
}

#[derive(Args, Debug, Clone)]
pub struct CommonArgs {
    /// Client adapter to use for resolution.
    #[arg(long)]
    pub client: Option<String>,
    /// Directory to treat as the working directory.
    #[arg(long, default_value = ".")]
    pub cwd: PathBuf,
    /// Claimed runtime client version (ConfigLens never reads a client binary).
    #[arg(long)]
    pub client_version: Option<String>,
    /// Output format.
    #[arg(long, value_enum, default_value_t = Format::Text)]
    pub format: Format,
    /// Fail when the context is incomplete.
    #[arg(long)]
    pub strict: bool,
    /// Do not read any process environment variable.
    #[arg(long)]
    pub no_env: bool,
    /// Supplied context document (sensitive input).
    #[arg(long)]
    pub input_context: Option<PathBuf>,
    /// Colour policy for text output.
    #[arg(long, value_enum, default_value_t = ColorChoice::Auto)]
    pub color: ColorChoice,
    /// Path rendering policy for reports.
    #[arg(long, value_enum, default_value_t = PathsChoice::Local)]
    pub paths: PathsChoice,
    /// Extra root that file references may read from.
    #[arg(long = "allow-read-root")]
    pub allow_read_root: Vec<PathBuf>,
    /// Project root to use instead of discovering it.
    #[arg(long)]
    pub project_root: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct ExplainArgs {
    /// RFC 6901 JSON Pointer, e.g. /model or /mcp/example/enabled.
    pub key: String,
    #[command(flatten)]
    pub common: CommonArgs,
}

#[derive(Args, Debug)]
pub struct DoctorArgs {
    #[command(flatten)]
    pub common: CommonArgs,
    /// Severity at which doctor exits non-zero.
    #[arg(long = "fail-on", value_enum, default_value_t = FailOnArg::Error)]
    pub fail_on: FailOnArg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum FailOnArg {
    Warning,
    Error,
}

impl From<FailOnArg> for FailOn {
    fn from(value: FailOnArg) -> Self {
        match value {
            FailOnArg::Warning => FailOn::Warning,
            FailOnArg::Error => FailOn::Error,
        }
    }
}

#[derive(Args, Debug)]
pub struct SnapshotArgs {
    #[command(flatten)]
    pub common: CommonArgs,
    /// Destination file for the redacted report.
    #[arg(long)]
    pub output: PathBuf,
    /// Replace an existing destination.
    #[arg(long)]
    pub overwrite: bool,
}

#[derive(Args, Debug)]
pub struct DiffArgs {
    pub before: PathBuf,
    pub after: PathBuf,
    #[arg(long, value_enum, default_value_t = Format::Text)]
    pub format: Format,
}

#[derive(Args, Debug)]
pub struct SchemaArgs {
    /// Which bundled schema to print.
    #[arg(long, value_enum, default_value_t = SchemaKind::Report)]
    pub kind: SchemaKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SchemaKind {
    Report,
}

#[derive(Args, Debug)]
pub struct CompletionsArgs {
    /// Shell to generate a completion script for.
    #[arg(long, value_enum)]
    pub shell: crate::completions::Shell,
}

#[derive(Args, Debug, Clone)]
pub struct SupportReportArgs {
    #[command(flatten)]
    pub common: CommonArgs,
    /// Destination file for the sanitized report (required unless --preview).
    #[arg(long)]
    pub output: Option<PathBuf>,
    /// Replace an existing destination file.
    #[arg(long)]
    pub overwrite: bool,
    /// Print the sanitized report to stdout and write nothing.
    #[arg(long)]
    pub preview: bool,
}

/// Text output is coloured only when asked for, and never when `NO_COLOR` is set or stdout is
/// not a terminal.
pub fn should_color(choice: ColorChoice, no_color: bool, is_tty: bool) -> bool {
    match choice {
        ColorChoice::Always => !no_color,
        ColorChoice::Never => false,
        ColorChoice::Auto => !no_color && is_tty,
    }
}

/// Everything a command needs after the context has been built.
pub struct Prepared {
    pub client: Client,
    pub context: ResolutionContext,
    pub policy: PathPolicy,
    pub run: RunOutput,
}

/// Build the resolution context from the command line and the allowlisted environment.
pub fn prepare(client: Client, common: &CommonArgs) -> Result<Prepared, CliError> {
    let env = capture_environment(common.no_env);
    let home = home_from_env(env.as_ref());
    let xdg = xdg_from_env(env.as_ref(), &home);

    // Everything the path policy sees must be absolute: a relative cwd would be outside every
    // authorised root and nothing would be readable.
    let cwd = absolute(&common.cwd);
    let mut context = ResolutionContext::new(
        client.id(),
        &cwd.display().to_string(),
        &home.display().to_string(),
        &xdg.display().to_string(),
    );
    context.env = env;
    context.paths = path_mode(common.paths);
    context.strict = common.strict;
    if let Some(version) = &common.client_version {
        context.client_version = VersionContext::Claimed(version.clone());
    }
    context.project_root = match &common.project_root {
        Some(root) => Some(absolute(root)),
        None => crate::adapters::find_project_root(&cwd, 64),
    };

    // Roots that may be read: the adapter's own roots plus what the user approved. The working
    // directory is always readable: it is the project the user explicitly asked about.
    let mut extra: Vec<PathBuf> = common.allow_read_root.iter().map(|p| absolute(p)).collect();
    if !extra.contains(&cwd) {
        extra.push(cwd.clone());
    }
    if let Some(path) = &common.input_context {
        let path = absolute(path);
        if let Some(parent) = path.parent() {
            extra.push(parent.to_path_buf());
        }
    }

    if let Some(path) = &common.input_context {
        let text = std::fs::read_to_string(path).map_err(|_| {
            CliError::file_problem("context_unreadable", &path.display().to_string())
        })?;
        let supplied = parse_supplied_context(&text, client.id(), &context.client_version)?;
        if let Some(version) = &supplied.client_version {
            if context.client_version.is_unknown() {
                context.client_version = VersionContext::Claimed(version.clone());
            }
        }
        context.supplied_env = supplied.environment.clone();
        context.client_arguments = supplied.client_arguments.clone();
        context.supplied_sources = supplied.sources.clone();
        if let Some(trust) = &supplied.project_trust {
            context.trust = Trust::parse(trust).unwrap_or(Trust::Unknown);
        }
        context.simulated = true;
    }
    context.approved_roots = extra.clone();

    let managed =
        crate::adapters::managed_dirs(client, context.os, context.program_data.as_deref());
    let policy = PathPolicy::new(
        &home,
        &xdg,
        context.project_root.as_deref(),
        &managed,
        &extra,
    );

    let adapter = adapter_for(client);
    let run = Engine::new(adapter.as_ref(), &context, &RealFileAccess, &policy).run()?;
    Ok(Prepared {
        client,
        context,
        policy,
        run,
    })
}

/// Execute a parsed command, returning the process exit code.
pub fn run(cli: Cli) -> i32 {
    match dispatch(cli) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("configlens: {}: {}", error.code(), error.message());
            error.exit_code()
        }
    }
}

fn dispatch(cli: Cli) -> Result<i32, CliError> {
    match cli.command {
        Command::Clients(args) => {
            if args.format == Format::Json {
                print!("{}", clients_json()?);
            } else {
                print!("{}", render_text::render_clients());
            }
            Ok(0)
        }
        Command::Schema(args) => {
            match args.kind {
                SchemaKind::Report => print!("{REPORT_SCHEMA_JSON}"),
            }
            Ok(0)
        }
        Command::Completions(args) => {
            print!("{}", crate::completions::script(args.shell));
            Ok(0)
        }
        Command::SupportReport(args) => {
            let client = resolve_client(args.common.client.as_deref())?;
            // A support report is always share-sanitized, whatever `--paths` says: paths, hosts,
            // unknown keys and secrets are all projected before anything is shown or written.
            let mut common = args.common.clone();
            common.paths = PathsChoice::Share;
            let prepared = prepare(client, &common)?;
            let report = compose_report("support-report", &prepared, Vec::new())?;
            let json = render_json::render(&report);
            if args.preview {
                // Explicit preview: the sanitized report goes to stdout, nothing is written.
                print!("{json}");
                eprintln!(
                    "configlens: preview only — nothing was written. Add --output <path> to save the sanitized report."
                );
            } else if let Some(output) = &args.output {
                // Explicit output; never an automatic upload, and never a default path.
                let outcome = crate::commands::write_snapshot(output, &json, args.overwrite)?;
                eprintln!(
                    "configlens: wrote {} bytes of share-mode support report to {} (permissions: {}). Review it before attaching it anywhere.",
                    outcome.bytes, outcome.location, outcome.permissions
                );
            } else {
                return Err(CliError::usage(
                    "support-report needs --output <path> to save the sanitized report, or --preview to print it",
                ));
            }
            Ok(exit_code_for_run(&prepared, &common, FailOn::Error, None)?.exit_code())
        }
        Command::Diff(args) => {
            let before = std::fs::read_to_string(&args.before).map_err(|_| {
                CliError::file_problem("report_unreadable", &args.before.display().to_string())
            })?;
            let after = std::fs::read_to_string(&args.after).map_err(|_| {
                CliError::file_problem("report_unreadable", &args.after.display().to_string())
            })?;
            let result = crate::commands::diff_reports(
                &crate::commands::parse_report(&before)?,
                &crate::commands::parse_report(&after)?,
            )?;
            match args.format {
                Format::Text => print!("{}", crate::commands::render_diff(&result)),
                Format::Json => print!("{}", crate::commands::render_diff_json(&result)?),
            }
            Ok(0)
        }
        Command::Scan(common) => {
            let client = resolve_client(common.client.as_deref())?;
            let prepared = prepare(client, &common)?;
            let report = compose_report("scan", &prepared, Vec::new())?;
            print_report(&report, &prepared, common.format, None);
            Ok(exit_code_for_run(&prepared, &common, FailOn::Error, None)?.exit_code())
        }
        Command::Explain(args) => {
            let client = resolve_client(args.common.client.as_deref())?;
            let mut prepared = prepare(client, &args.common)?;
            // The queried pointer must appear in the report, even when nothing defines it.
            let resolution = {
                let adapter = adapter_for(client);
                prepared.run.explain_with(&args.key, adapter.registry())
            };
            prepared.run.insert_resolution(resolution);
            let report = compose_report("explain", &prepared, Vec::new())?;
            print_report(&report, &prepared, args.common.format, Some(&args.key));
            Ok(
                exit_code_for_run(&prepared, &args.common, FailOn::Error, Some(&args.key))?
                    .exit_code(),
            )
        }
        Command::Doctor(args) => {
            let client = resolve_client(args.common.client.as_deref())?;
            let fail_on = FailOn::from(args.fail_on);
            let prepared = prepare(client, &args.common)?;
            let report = compose_report("doctor", &prepared, Vec::new())?;
            print_report(&report, &prepared, args.common.format, None);
            Ok(exit_code_for_run(&prepared, &args.common, fail_on, None)?.exit_code())
        }
        Command::Snapshot(args) => {
            let client = resolve_client(args.common.client.as_deref())?;
            let prepared = prepare(client, &args.common)?;
            let report = compose_report("snapshot", &prepared, Vec::new())?;
            let json = render_json::render(&report);
            let outcome = crate::commands::write_snapshot(&args.output, &json, args.overwrite)?;
            eprintln!(
                "configlens: wrote {} bytes to {} (permissions: {})",
                outcome.bytes, outcome.location, outcome.permissions
            );
            Ok(exit_code_for_run(&prepared, &args.common, FailOn::Error, None)?.exit_code())
        }
    }
}

fn clients_json() -> Result<String, CliError> {
    let mut implemented = Vec::new();
    for client in crate::adapters::Client::all() {
        let adapter = adapter_for(*client);
        let metadata = adapter.metadata();
        let registry = adapter.registry();
        implemented.push(serde_json::json!({
            "client": metadata.id,
            "adapter_revision": metadata.revision,
            "tested_versions": metadata.tested_versions,
            "documented_versions": metadata.documented_versions,
            "conformance_status": metadata.conformance_status,
            "pinned_commit": metadata.pinned_commit,
            "always_incomplete_layers": registry
                .coverage_gaps()
                .into_iter()
                .map(|(id, reason)| serde_json::json!({"layer": id, "reason": reason}))
                .collect::<Vec<_>>(),
        }));
    }
    let unsupported: Vec<serde_json::Value> = crate::adapters::Client::unsupported()
        .iter()
        .map(|(id, why)| serde_json::json!({"client": id, "reason": why}))
        .collect();
    let document = serde_json::json!({
        "schema_version": crate::REPORT_SCHEMA_VERSION,
        "tool_version": crate::TOOL_VERSION,
        "command": "clients",
        "clients": implemented,
        "unsupported_clients": unsupported,
        "errors": [],
    });
    let mut text = serde_json::to_string_pretty(&document)
        .map_err(|_| CliError::usage("the client matrix could not be serialized"))?;
    text.push('\n');
    Ok(text)
}

fn print_report(
    report: &Report,
    prepared: &Prepared,
    format: Format,
    explain_pointer: Option<&str>,
) {
    match format {
        Format::Json => print!("{}", render_json::render(report)),
        Format::Text => match explain_pointer {
            Some(pointer) => {
                // Share mode rewrites pointers, URL hosts and embedded path roots inside the
                // report; render the *projected* resolution so text output obeys `--paths share`
                // exactly like the JSON envelope. Index correspondence holds because
                // `project_resolutions` maps the resolutions in order without filtering.
                let index = prepared
                    .run
                    .resolutions
                    .iter()
                    .position(|r| r.pointer == pointer);
                let resolution = index
                    .and_then(|i| report.resolutions.get(i).cloned())
                    .unwrap_or_else(|| prepared.run.explain(pointer));
                let color = should_color(
                    ColorChoice::Auto,
                    std::env::var_os("NO_COLOR").is_some(),
                    false,
                );
                let _ = color; // text output is plain unless a caller asks for colour
                print!(
                    "{}",
                    render_text::render_explain(&prepared.run, &resolution, &prepared.context)
                );
                let base = resolution.pointer.clone();
                let children: Vec<&str> = report
                    .resolutions
                    .iter()
                    .filter(|r| {
                        r.pointer.starts_with(&base)
                            && r.pointer.len() > base.len()
                            && r.pointer[base.len()..].starts_with('/')
                    })
                    .map(|r| r.pointer.as_str())
                    .collect();
                if !children.is_empty() {
                    println!("Child pointers ({}):", children.len());
                    for child in children {
                        println!("  {child}");
                    }
                }
            }
            None => match report.command.as_str() {
                "doctor" => print!("{}", render_text::render_doctor(&prepared.run)),
                _ => print!("{}", render_text::render_scan(&prepared.run)),
            },
        },
    }
}

/// Compose the report envelope for a run.
pub fn compose_report(
    command: &str,
    prepared: &Prepared,
    failures: Vec<CliError>,
) -> Result<Report, CliError> {
    let adapter = adapter_for(prepared.client);
    let registry = adapter.registry();
    let metadata = registry.metadata();
    let provisional = !prepared.context.version_supported(
        registry.tested_client_versions(),
        registry.documented_client_versions(),
    );
    let compatibility = Compatibility {
        conformance_status: metadata.conformance_status,
        tested_versions: metadata.tested_versions,
        documented_versions: metadata.documented_versions,
        pinned_commit: metadata.pinned_commit,
        evidence: registry
            .evidence()
            .iter()
            .map(|entry| EvidenceReport {
                id: entry.id.clone(),
                kind: entry.kind.clone(),
                url: entry.url.clone(),
                retrieved: entry.retrieved.clone(),
                note: entry.note.clone(),
            })
            .collect(),
        provisional,
    };
    Ok(build_report(
        ReportInput {
            command,
            client: prepared.run.client.clone(),
            adapter_revision: metadata.revision,
            compatibility,
            context: prepared.context.clone(),
            sources: &prepared.run.sources,
            resolutions: &prepared.run.resolutions,
            diagnostics: &prepared.run.diagnostics,
            coverage: &prepared.run.coverage,
            paths: &prepared.policy,
            errors: errors_of(&failures),
        },
        prepared.context.paths,
    ))
}

/// Exit-code decision for a run, following the documented priority.
fn exit_code_for_run(
    prepared: &Prepared,
    common: &CommonArgs,
    fail_on: FailOn,
    explain_pointer: Option<&str>,
) -> Result<Outcome, CliError> {
    let adapter = adapter_for(prepared.client);
    let registry = adapter.registry();
    let version_untested = !prepared.context.version_supported(
        registry.tested_client_versions(),
        registry.documented_client_versions(),
    );
    // An unknown pointer (no rule matches) is unsupported semantics.
    let unknown_key = explain_pointer
        .map(|pointer| registry.rule_for(pointer).pattern.is_empty())
        .unwrap_or(false);
    Ok(exit_code_for_known(
        &[],
        &prepared.run,
        fail_on,
        common.strict,
        explain_pointer,
        version_untested,
        unknown_key,
    ))
}

/// Report + rendered text, used by every command.
pub fn render_report(report: &Report, format: Format) -> String {
    match format {
        Format::Json => render_json::render(report),
        Format::Text => render_text::render_scan(&RunOutput::default()),
    }
}

/// Exit-code decision for a run.
pub fn exit_code_for(
    failures: &[CliError],
    run: &RunOutput,
    fail_on: FailOn,
    strict: bool,
    explain_pointer: Option<&str>,
    version_untested: bool,
) -> Outcome {
    exit_code_for_known(
        failures,
        run,
        fail_on,
        strict,
        explain_pointer,
        version_untested,
        false,
    )
}

/// Exit-code decision including the "no rule matches this pointer" case.
pub fn exit_code_for_known(
    failures: &[CliError],
    run: &RunOutput,
    fail_on: FailOn,
    strict: bool,
    _explain_pointer: Option<&str>,
    version_untested: bool,
    unknown_key: bool,
) -> Outcome {
    let mut outcomes: Vec<Outcome> = failures.iter().map(CliError::outcome).collect();
    if findings_at_or_above(run, fail_on) > 0 {
        outcomes.push(Outcome::DoctorFindings);
    }
    if strict && (version_untested || has_incomplete_resolution(run) || has_blocking_gap(run)) {
        outcomes.push(Outcome::Unsupported);
    }
    if unknown_key {
        outcomes.push(Outcome::Unsupported);
    }
    // A known key whose *version* is untested stays an ordinary provisional report: the observed
    // winner is still shown and exit code 0 applies unless `--strict` was requested (spec §4:
    // exit 3 is "requested key/client semantics unsupported **or** strict mode detects incomplete
    // context"). Only a pointer no rule matches is unsupported key semantics (`unknown_key`).
    if run
        .diagnostics
        .iter()
        .any(|d| d.severity == Severity::Error)
    {
        outcomes.push(Outcome::FileProblem);
    }
    arbitrate(&outcomes)
}

/// Resolve a possibly relative path against the process working directory.
pub fn absolute(path: &std::path::Path) -> PathBuf {
    match std::fs::canonicalize(path) {
        Ok(resolved) => resolved,
        Err(_) => {
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                std::env::current_dir()
                    .map(|cwd| cwd.join(path))
                    .unwrap_or_else(|_| path.to_path_buf())
            }
        }
    }
}

/// Read the allowlisted process environment (spec §4).
pub fn capture_environment(no_env: bool) -> Option<BTreeMap<String, String>> {
    if no_env {
        return None;
    }
    let mut env = BTreeMap::new();
    for name in ENV_ALLOWLIST {
        if let Ok(value) = std::env::var(name) {
            env.insert((*name).to_string(), value);
        }
    }
    Some(env)
}

/// Home directory, from the allowlisted environment only.
pub fn home_from_env(env: Option<&BTreeMap<String, String>>) -> PathBuf {
    env.and_then(|e| e.get("HOME").or_else(|| e.get("USERPROFILE")).cloned())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// XDG config directory, from the allowlisted environment only.
pub fn xdg_from_env(env: Option<&BTreeMap<String, String>>, home: &std::path::Path) -> PathBuf {
    env.and_then(|e| e.get("XDG_CONFIG_HOME").cloned())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"))
}

/// Interpret `--client`, mapping known-but-unimplemented clients to exit code 3.
pub fn resolve_client(requested: Option<&str>) -> Result<Client, CliError> {
    let Some(text) = requested else {
        return Err(CliError::usage(
            "--client is required for resolution (one of: opencode, claude)",
        ));
    };
    if let Some(client) = Client::parse(text) {
        return Ok(client);
    }
    if Client::unsupported().iter().any(|(id, _)| *id == text) {
        return Err(CliError::unsupported(
            "this client is explicitly unsupported in this release; see `configlens clients`",
        ));
    }
    Err(CliError::usage(
        "unknown client; supported values are opencode and claude",
    ))
}

/// `--paths` selection.
pub fn path_mode(choice: PathsChoice) -> PathMode {
    match choice {
        PathsChoice::Local => PathMode::Local,
        PathsChoice::Share => PathMode::Share,
    }
}

/// `--strict` helper: true when any resolution is not fully resolved.
pub fn has_incomplete_resolution(run: &RunOutput) -> bool {
    run.resolutions
        .iter()
        .any(|r| r.status != crate::resolve::ResolutionStatus::Resolved)
}

/// Does the run have an applicable, blocking coverage gap?
pub fn has_blocking_gap(run: &RunOutput) -> bool {
    run.coverage
        .iter()
        .any(|g| g.blocks_completeness && g.kind != GapKind::TrustUnknown)
}

/// Count diagnostics at or above `fail_on`.
pub fn findings_at_or_above(run: &RunOutput, fail_on: FailOn) -> usize {
    run.diagnostics
        .iter()
        .filter(|d| match fail_on {
            FailOn::Warning => d.severity >= Severity::Warning,
            FailOn::Error => d.severity == Severity::Error,
        })
        .count()
}

/// Structured error list for a report envelope.
pub fn errors_of(failures: &[CliError]) -> Vec<ErrorDto> {
    failures.iter().map(CliError::to_dto).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Outcome;

    #[test]
    fn colour_is_disabled_for_json_and_non_tty_and_no_color() {
        assert!(!should_color(ColorChoice::Auto, false, false));
        assert!(should_color(ColorChoice::Auto, false, true));
        assert!(!should_color(ColorChoice::Auto, true, true));
        assert!(!should_color(ColorChoice::Never, false, true));
        assert!(should_color(ColorChoice::Always, false, false));
        assert!(!should_color(ColorChoice::Always, true, false));
    }

    #[test]
    fn unsupported_clients_are_distinct_from_unknown_ones() {
        assert_eq!(resolve_client(Some("opencode")), Ok(Client::OpenCode));
        assert_eq!(resolve_client(Some("claude")), Ok(Client::Claude));
        assert_eq!(resolve_client(None).unwrap_err().exit_code(), 2);
        assert_eq!(resolve_client(Some("nope")).unwrap_err().exit_code(), 2);
        let codex = resolve_client(Some("codex")).unwrap_err();
        assert_eq!(codex.exit_code(), 3);
        assert_eq!(codex.code(), "unsupported");
    }

    #[test]
    fn environment_capture_is_allowlisted_and_can_be_disabled() {
        let captured = capture_environment(true);
        assert!(captured.is_none(), "--no-env must disable capture");
        let captured = capture_environment(false).expect("allowed");
        for key in captured.keys() {
            assert!(
                ENV_ALLOWLIST.contains(&key.as_str()),
                "{key} is not allowlisted"
            );
        }
        assert!(!ENV_ALLOWLIST.contains(&"PATH"));
    }

    #[test]
    fn home_and_xdg_fall_back_without_mutating_the_environment() {
        let env = BTreeMap::new();
        assert_eq!(home_from_env(Some(&env)), PathBuf::from("/"));
        assert_eq!(
            xdg_from_env(Some(&env), std::path::Path::new("/home/x")),
            PathBuf::from("/home/x/.config")
        );
        let env = BTreeMap::from([("HOME".to_string(), "/home/y".to_string())]);
        assert_eq!(home_from_env(Some(&env)), PathBuf::from("/home/y"));
    }

    #[test]
    fn paths_choices_map_to_report_modes() {
        assert_eq!(path_mode(PathsChoice::Local), PathMode::Local);
        assert_eq!(path_mode(PathsChoice::Share), PathMode::Share);
    }

    #[test]
    fn fail_on_maps_to_the_documented_thresholds() {
        assert_eq!(FailOn::from(FailOnArg::Error), FailOn::Error);
        assert_eq!(FailOn::from(FailOnArg::Warning), FailOn::Warning);
        assert_eq!(FailOn::Error.name(), "error");
    }

    #[test]
    fn exit_code_increases_are_arbitrated_in_priority_order() {
        let outcomes = vec![
            Outcome::DoctorFindings,
            Outcome::Unsupported,
            Outcome::FileProblem,
        ];
        assert_eq!(arbitrate(&outcomes), Outcome::FileProblem);
    }
}
