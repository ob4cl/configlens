//! Public report schema 1.0 (spec §5). This module is the only place where run output becomes
//! serializable, and it only ever consumes redacted public DTOs.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::context::ResolutionContext;
use crate::error::ErrorDto;
use crate::model::event::Event;
use crate::model::source::Source;
use crate::resolve::{CoverageGap, Diagnostic, Resolution};
use crate::security::{PathMode, PathPolicy, Pseudonymizer, PublicValue};
use crate::{REPORT_SCHEMA_VERSION, TOOL_VERSION};

/// The bundled JSON Schema for `schema --kind report` (no network access).
pub const REPORT_SCHEMA_JSON: &str = include_str!("../schema/report-1.0.json");

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Compatibility {
    pub conformance_status: String,
    pub tested_versions: Vec<String>,
    pub documented_versions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned_commit: Option<String>,
    /// Evidence entries the adapter cites, sorted by id.
    pub evidence: Vec<EvidenceReport>,
    /// True when the claimed client version is not among the tested/documented versions.
    pub provisional: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EvidenceReport {
    pub id: String,
    pub kind: String,
    pub url: String,
    pub retrieved: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ContextReport {
    pub os: String,
    pub cwd: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_root: Option<String>,
    pub client_version: String,
    pub trust: String,
    pub paths: String,
    /// True when any input came from `--input-context`.
    pub simulated: bool,
    pub strict: bool,
    /// Names (never values) of the environment variables that were consulted.
    pub environment_keys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SourceReport {
    pub id: u32,
    pub layer: String,
    pub rank: u32,
    pub class: String,
    pub location: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_detail: Option<String>,
    pub applicability: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub applicability_note: Option<String>,
    pub bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RedactionReport {
    pub mode: String,
    /// Rule names that were applied while projecting, sorted.
    pub rules_applied: Vec<String>,
    /// Snapshot file-permission support on this platform.
    pub snapshot_acl: String,
    /// How many values were withheld.
    pub values_redacted: usize,
}

/// The complete report. Field order follows the specification's list.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    pub schema_version: String,
    pub tool_version: String,
    pub command: String,
    pub client: String,
    pub adapter_revision: String,
    pub compatibility: Compatibility,
    pub context: ContextReport,
    pub sources: Vec<SourceReport>,
    pub resolutions: Vec<Resolution>,
    pub diagnostics: Vec<Diagnostic>,
    pub coverage: Vec<CoverageGap>,
    pub redaction: RedactionReport,
    pub errors: Vec<ErrorDto>,
}

/// Everything a report needs, already redacted.
pub struct ReportInput<'a> {
    pub command: &'a str,
    pub client: String,
    pub adapter_revision: String,
    pub compatibility: Compatibility,
    pub context: ResolutionContext,
    pub sources: &'a [Source],
    pub resolutions: &'a [Resolution],
    pub diagnostics: &'a [Diagnostic],
    pub coverage: &'a [CoverageGap],
    pub paths: &'a PathPolicy,
    pub errors: Vec<ErrorDto>,
}

/// Build the report. `mode` selects local or share rendering.
pub fn build(input: ReportInput<'_>, mode: PathMode) -> Report {
    let values_redacted = input
        .resolutions
        .iter()
        .flat_map(|r| {
            std::iter::once(&r.observed_value)
                .chain(r.effective_value.iter())
                .chain(r.events.iter().filter_map(|e: &Event| e.value.as_ref()))
        })
        .filter(|v: &&PublicValue| v.is_redacted())
        .count();

    let os = input.context.os;
    let cwd = input.paths.display(&input.context.cwd, mode);
    let project_root = input
        .context
        .project_root
        .as_ref()
        .map(|root| input.paths.display(root, mode));

    let mut environment_keys: Vec<String> = input
        .context
        .env
        .as_ref()
        .map(|env| env.keys().cloned().collect())
        .unwrap_or_default();
    environment_keys.extend(input.context.supplied_env.keys().cloned());
    environment_keys.sort();
    environment_keys.dedup();

    // Project pointers, values and hosts per mode before the client string is moved into the report.
    let resolutions = project_resolutions(input.resolutions, mode, &input.client, input.paths);
    Report {
        schema_version: REPORT_SCHEMA_VERSION.to_string(),
        tool_version: TOOL_VERSION.to_string(),
        command: input.command.to_string(),
        client: input.client,
        adapter_revision: input.adapter_revision,
        compatibility: input.compatibility,
        context: ContextReport {
            os: os.name().to_string(),
            cwd,
            project_root,
            client_version: input.context.client_version.as_str().to_string(),
            trust: input.context.trust.name().to_string(),
            paths: match mode {
                PathMode::Local => "local".to_string(),
                PathMode::Share => "share".to_string(),
            },
            simulated: input.context.simulated,
            strict: input.context.strict,
            environment_keys,
        },
        sources: input
            .sources
            .iter()
            .map(|s| project_source(s, mode, input.paths))
            .collect(),
        resolutions,
        diagnostics: input.diagnostics.to_vec(),
        coverage: input.coverage.to_vec(),
        redaction: RedactionReport {
            mode: match mode {
                PathMode::Local => "local".to_string(),
                PathMode::Share => "share".to_string(),
            },
            rules_applied: {
                let mut rules = vec![
                    "sensitive_field_names".to_string(),
                    "redacted_subtrees".to_string(),
                    "value_allowlist_per_adapter".to_string(),
                    "url_userinfo_query_fragment".to_string(),
                    "terminal_control_sanitization".to_string(),
                ];
                if mode == PathMode::Share {
                    rules.push("share_mode_path_and_host_pseudonymization".to_string());
                }
                rules
            },
            snapshot_acl: snapshot_acl_support().to_string(),
            values_redacted,
        },
        errors: input.errors,
    }
}

/// Windows ACL restriction on snapshot output is not implemented; say so instead of pretending.
pub fn snapshot_acl_support() -> &'static str {
    if cfg!(unix) {
        "owner_only_0600"
    } else {
        "not_implemented"
    }
}

/// Pointer segments that name a documented adapter key. Anything else is an opaque, untrusted
/// key name that could itself be a credential (spec §7: "Unknown keys themselves may contain
/// credentials: substitute opaque field IDs in share reports; do not expose arbitrary untrusted
/// object-key strings").
fn known_key_segments(client: &str) -> BTreeSet<String> {
    let mut known = BTreeSet::new();
    if let Some(registry) = crate::adapters::Client::parse(client)
        .and_then(|client| crate::adapters::Registry::load(client).ok())
    {
        for rule in registry.rules() {
            for segment in rule.pointer.split('/') {
                if !segment.is_empty() && segment != "*" && segment != "**" {
                    known.insert(segment.to_string());
                }
            }
        }
    }
    known
}

/// Rewrite resolution pointers *and* revealed values for the requested path mode.
///
/// In share mode every pointer segment that is not a documented key (an unknown key, an instance
/// name such as an MCP server, an environment variable) is replaced by a deterministic opaque
/// field ID, every private URL host is replaced by a deterministic host ID, and every revealed
/// string has known path roots aliased. Local mode keeps the native names and hosts because it is
/// the diagnostic view (spec §7).
fn project_resolutions(
    resolutions: &[Resolution],
    mode: PathMode,
    client: &str,
    paths: &PathPolicy,
) -> Vec<Resolution> {
    if mode == PathMode::Local {
        return resolutions.to_vec();
    }
    let known = known_key_segments(client);
    let mut opaque: Vec<String> = Vec::new();
    let mut hosts: Vec<String> = Vec::new();
    for resolution in resolutions {
        for segment in resolution.pointer.split('/') {
            if !segment.is_empty() && !known.contains(segment) {
                opaque.push(segment.to_string());
            }
        }
        for value in resolution_values(resolution) {
            if let Some(value) = value.as_value() {
                collect_url_hosts(value, &mut hosts);
            }
        }
    }
    let pseudonymizer = Pseudonymizer::new(opaque, hosts);
    resolutions
        .iter()
        .map(|resolution| {
            let mut projected = resolution.clone();
            projected.pointer = pseudonymize_pointer(&resolution.pointer, &known, &pseudonymizer);
            projected.observed_value =
                share_project_value(&resolution.observed_value, &pseudonymizer, paths);
            projected.effective_value = resolution
                .effective_value
                .as_ref()
                .map(|value| share_project_value(value, &pseudonymizer, paths));
            projected.events = resolution
                .events
                .iter()
                .map(|event| {
                    let mut event = event.clone();
                    event.value = event
                        .value
                        .as_ref()
                        .map(|value| share_project_value(value, &pseudonymizer, paths));
                    event
                })
                .collect();
            projected
        })
        .collect()
}

/// Every value a resolution carries, in a stable order.
fn resolution_values(resolution: &Resolution) -> Vec<&PublicValue> {
    let mut values: Vec<&PublicValue> = vec![&resolution.observed_value];
    values.extend(resolution.effective_value.iter());
    values.extend(
        resolution
            .events
            .iter()
            .filter_map(|event| event.value.as_ref()),
    );
    values
}

/// Collect the URL hosts that appear in a revealed value (string or array of strings).
fn collect_url_hosts(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::String(text) => {
            if let Some(host) = crate::security::redact::url_host(text) {
                out.push(host);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_url_hosts(item, out);
            }
        }
        _ => {}
    }
}

/// Rewrite a revealed public value for share mode: pseudonymize URL hosts and alias embedded
/// path roots. Redacted values and non-string shapes are returned unchanged.
fn share_project_value(
    value: &PublicValue,
    hosts: &Pseudonymizer,
    paths: &PathPolicy,
) -> PublicValue {
    if value.is_redacted() {
        return value.clone();
    }
    let Some(raw) = value.as_value() else {
        return value.clone();
    };
    let projected = match raw {
        serde_json::Value::String(text) => {
            serde_json::Value::String(share_project_string(text, hosts, paths))
        }
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items
                .iter()
                .map(|item| match item {
                    serde_json::Value::String(text) => {
                        serde_json::Value::String(share_project_string(text, hosts, paths))
                    }
                    other => other.clone(),
                })
                .collect(),
        ),
        other => other.clone(),
    };
    if &projected == raw {
        return value.clone();
    }
    let reason = value.reason_str();
    if reason.is_empty() {
        PublicValue::revealed(projected)
    } else {
        PublicValue::revealed_with_reason(projected, reason)
    }
}

/// Convert one revealed string into its share-mode form.
fn share_project_string(text: &str, hosts: &Pseudonymizer, paths: &PathPolicy) -> String {
    if let Some(rewritten) = crate::security::redact::pseudonymize_url_host(text, hosts) {
        return rewritten;
    }
    paths.alias_embedded(text, PathMode::Share)
}

fn pseudonymize_pointer(
    pointer: &str,
    known: &BTreeSet<String>,
    pseudonymizer: &Pseudonymizer,
) -> String {
    if pointer.is_empty() {
        return String::new();
    }
    pointer
        .split('/')
        .map(|segment| {
            if segment.is_empty() || known.contains(segment) {
                segment.to_string()
            } else {
                pseudonymizer.field(segment)
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn project_source(source: &Source, mode: PathMode, paths: &PathPolicy) -> SourceReport {
    let location = match (&source.path, mode) {
        (Some(path), _) => paths.display(path, mode),
        (None, _) => source.display.clone(),
    };
    SourceReport {
        id: source.id.0,
        layer: source.layer_id.clone(),
        rank: source.rank,
        class: source.class.name().to_string(),
        location,
        status: source.status.code().to_string(),
        status_detail: source.status.detail(),
        applicability: match &source.applicability {
            crate::adapters::Applicability::Applicable => "applicable".to_string(),
            crate::adapters::Applicability::Skipped(_) => "skipped".to_string(),
            crate::adapters::Applicability::Conditional(_) => "conditional".to_string(),
        },
        applicability_note: source.applicability.note().map(str::to_string),
        bytes: source.bytes,
        note: source.note.clone(),
    }
}

/// Deterministic JSON serialization of a report: sorted keys, trailing newline, no ANSI.
pub fn to_json(report: &Report) -> Result<String, serde_json::Error> {
    let value = serde_json::to_value(report)?;
    let sorted = sort_objects(value);
    let mut text = serde_json::to_string_pretty(&sorted)?;
    text.push('\n');
    Ok(text)
}

fn sort_objects(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut sorted = BTreeMap::new();
            for (key, item) in map {
                sorted.insert(key, sort_objects(item));
            }
            let mut out = serde_json::Map::new();
            for (key, item) in sorted {
                out.insert(key, item);
            }
            serde_json::Value::Object(out)
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(sort_objects).collect())
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{FailOn, Os, Trust, VersionContext};
    use crate::model::definition::ValueKind;
    use crate::parse::Limits;
    use std::path::PathBuf;

    fn ctx() -> ResolutionContext {
        ResolutionContext {
            client: "claude".into(),
            os: Os::Linux,
            cwd: PathBuf::from("/home/dev/p"),
            project_root: None,
            home: PathBuf::from("/home/dev"),
            xdg_config: PathBuf::from("/home/dev/.config"),
            program_data: None,
            approved_roots: Vec::new(),
            client_version: VersionContext::Claimed("2.1.296".into()),
            trust: Trust::Unknown,
            env: Some(BTreeMap::from([(
                "ANTHROPIC_MODEL".to_string(),
                "SEEDED-ENV-VALUE".to_string(),
            )])),
            supplied_env: BTreeMap::new(),
            client_arguments: Vec::new(),
            supplied_sources: Vec::new(),
            strict: false,
            simulated: false,
            paths: PathMode::Local,
            limits: Limits::default(),
            total_bytes_limit: 16 * 1024 * 1024,
            max_sources: 128,
            fail_on: FailOn::Error,
            color: false,
        }
    }

    fn compatibility() -> Compatibility {
        Compatibility {
            conformance_status: "fixture_verified_max".into(),
            tested_versions: vec!["2.1.296".into()],
            documented_versions: vec!["2.1.296".into()],
            pinned_commit: None,
            evidence: vec![EvidenceReport {
                id: "S3".into(),
                kind: "documentation".into(),
                url: "https://code.claude.com/docs/en/settings".into(),
                retrieved: "2026-10-10".into(),
                note: None,
            }],
            provisional: false,
        }
    }

    fn input<'a>(
        ctx: &'a ResolutionContext,
        sources: &'a [Source],
        resolutions: &'a [Resolution],
        paths: &'a PathPolicy,
    ) -> ReportInput<'a> {
        ReportInput {
            command: "explain",
            client: "claude".into(),
            adapter_revision: "0.1.0".into(),
            compatibility: compatibility(),
            context: ctx.clone(),
            sources,
            resolutions,
            diagnostics: &[],
            coverage: &[],
            paths,
            errors: Vec::new(),
        }
    }

    #[test]
    fn report_carries_every_required_top_level_field() {
        let ctx = ctx();
        let paths = PathPolicy::new(&ctx.home, &ctx.xdg_config, None, &[], &[]);
        let report = build(input(&ctx, &[], &[], &paths), PathMode::Local);
        let value = serde_json::to_value(&report).expect("serialize");
        for field in [
            "schema_version",
            "tool_version",
            "client",
            "adapter_revision",
            "compatibility",
            "context",
            "sources",
            "resolutions",
            "diagnostics",
            "coverage",
            "redaction",
            "errors",
        ] {
            assert!(value.get(field).is_some(), "missing required field {field}");
        }
        assert_eq!(value["schema_version"], REPORT_SCHEMA_VERSION);
    }

    #[test]
    fn the_bundled_schema_is_valid_json_and_matches_the_schema_version() {
        let schema: serde_json::Value =
            serde_json::from_str(REPORT_SCHEMA_JSON).expect("bundled schema must parse");
        assert_eq!(
            schema["properties"]["schema_version"]["const"],
            REPORT_SCHEMA_VERSION
        );
        for field in [
            "schema_version",
            "tool_version",
            "client",
            "adapter_revision",
            "compatibility",
            "context",
            "sources",
            "resolutions",
            "diagnostics",
            "coverage",
            "redaction",
            "errors",
        ] {
            let required = schema["required"].as_array().expect("required array");
            assert!(
                required.iter().any(|v| v == field),
                "schema must require {field}"
            );
        }
    }

    #[test]
    fn context_never_reports_environment_values() {
        let mut ctx = ctx();
        ctx.env = Some(BTreeMap::from([(
            "ANTHROPIC_MODEL".to_string(),
            "SEEDED-ENV-VALUE".to_string(),
        )]));
        let paths = PathPolicy::new(&ctx.home, &ctx.xdg_config, None, &[], &[]);
        let report = build(input(&ctx, &[], &[], &paths), PathMode::Local);
        assert_eq!(report.context.environment_keys, vec!["ANTHROPIC_MODEL"]);
        let text = to_json(&report).expect("json");
        assert!(
            !text.contains("SEEDED-ENV-VALUE"),
            "env values must never be reported"
        );
    }

    #[test]
    fn share_mode_pseudonymizes_paths_and_keeps_output_deterministic() {
        let mut ctx = ctx();
        ctx.cwd = PathBuf::from("/opt/private/repo");
        let paths = PathPolicy::new(&ctx.home, &ctx.xdg_config, None, &[], &[]);
        let local = build(input(&ctx, &[], &[], &paths), PathMode::Local);
        let share = build(input(&ctx, &[], &[], &paths), PathMode::Share);
        assert_eq!(local.context.cwd, "/opt/private/repo");
        assert_eq!(share.context.cwd, "$EXTERNAL/repo");
        let a = to_json(&share).expect("json");
        let b = to_json(&share).expect("json");
        assert_eq!(a, b, "identical input must give byte-identical output");
    }

    #[test]
    fn share_mode_makes_unknown_key_names_opaque() {
        let ctx = ctx();
        let paths = PathPolicy::new(&ctx.home, &ctx.xdg_config, None, &[], &[]);
        let secret = "SYNTHETIC_LEAK_CANARY_7e19";
        let mut resolution = Resolution::new(&format!("/{secret}"));
        resolution.status = crate::resolve::ResolutionStatus::Unsupported;
        let resolutions = vec![resolution];

        // An unknown object key can itself be a credential: in share mode it is replaced by an
        // opaque field ID (spec §7). Regression for the seeded key/value leak.
        let share = build(input(&ctx, &[], &resolutions, &paths), PathMode::Share);
        let text = to_json(&share).expect("json");
        assert!(
            !text.contains(secret),
            "unknown key names must be opaque in share mode: {text}"
        );
        assert!(
            share.resolutions[0].pointer.starts_with("/field_"),
            "got {}",
            share.resolutions[0].pointer
        );

        // Local mode keeps the native key name: it is the diagnostic view (spec §7).
        let local = build(input(&ctx, &[], &resolutions, &paths), PathMode::Local);
        assert!(to_json(&local).expect("json").contains(secret));
    }

    #[test]
    fn share_mode_pseudonymizes_url_hosts_and_aliases_embedded_paths() {
        let ctx = ctx();
        let paths = PathPolicy::new(&ctx.home, &ctx.xdg_config, None, &[], &[]);
        let mut url = Resolution::new("/mcp/jira/url");
        url.status = crate::resolve::ResolutionStatus::Resolved;
        url.observed_value = PublicValue::revealed_with_reason(
            // This is the *already projected* form the report carries (redact_value strips
            // userinfo/query/fragment before a URL becomes public); share mode must additionally
            // pseudonymize the host.
            serde_json::json!("https://jira.internal.example/mcp?[redacted]"),
            "allowlisted_url_sanitized",
        );
        let mut shell = Resolution::new("/shell");
        shell.status = crate::resolve::ResolutionStatus::Resolved;
        shell.observed_value = PublicValue::revealed_with_reason(
            serde_json::json!("/home/dev/.local/bin/zsh"),
            "allowlisted_string",
        );
        let resolutions = vec![url, shell];

        // Local mode: the private host and path stay visible for diagnosis, userinfo does not.
        let local = to_json(&build(
            input(&ctx, &[], &resolutions, &paths),
            PathMode::Local,
        ))
        .expect("json");
        assert!(
            local.contains("jira.internal.example"),
            "local keeps the host"
        );
        assert!(!local.contains("user:pass"));
        assert!(local.contains("/home/dev/.local/bin/zsh"));

        // Share mode: the host becomes a deterministic ID and the path is aliased.
        let share = to_json(&build(
            input(&ctx, &[], &resolutions, &paths),
            PathMode::Share,
        ))
        .expect("json");
        assert!(!share.contains("jira.internal.example"), "{share}");
        assert!(share.contains("host_1"), "{share}");
        assert!(!share.contains("/home/dev/.local/bin"), "{share}");
        assert!(share.contains("$HOME/.local/bin/zsh"), "{share}");
    }

    #[test]
    fn counts_of_redacted_values_are_reported() {
        let ctx = ctx();
        let paths = PathPolicy::new(&ctx.home, &ctx.xdg_config, None, &[], &[]);
        let mut resolution = Resolution::new("/model");
        resolution.observed_value = PublicValue::redacted("sensitive_field", ValueKind::String);
        let resolutions = vec![resolution];
        let report = build(input(&ctx, &[], &resolutions, &paths), PathMode::Local);
        assert_eq!(report.redaction.values_redacted, 1);
    }

    #[test]
    fn snapshot_acl_support_is_honest_about_the_platform() {
        if cfg!(unix) {
            assert_eq!(snapshot_acl_support(), "owner_only_0600");
        } else {
            assert_eq!(snapshot_acl_support(), "not_implemented");
        }
    }
}
