//! Resolution engine (spec §5 pipeline) and the internal resolution model.

pub mod coverage;

use std::collections::BTreeMap;

use serde::Serialize;

use crate::adapters::{
    AdapterMetadata, ClientAdapter, DiscoveryPlan, Operator, Registry, SupportLevel,
};
use crate::context::ResolutionContext;
use crate::error::CliError;
use crate::filesystem::{Budget, FileAccess, FileKind};
use crate::model::definition::ValueKind;
use crate::model::event::{Confidence, Definition, DefinitionId, Event, EventKind};
use crate::model::source::{Source, SourceId, SourceStatus};
use crate::security::{PathPolicy, PublicValue};

pub use coverage::{CoverageGap, GapKind};

/// Final status of one key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionStatus {
    /// All applicable sources were read and no gap can change this key.
    Resolved,
    /// Something relevant is unknown, so only an observed winner can be reported.
    Partial,
    /// The key, the client or the version is not supported: inventory only.
    Unsupported,
    /// A required higher-priority source is malformed or blocked.
    Invalid,
    /// No source defines this key and no gap could define it either.
    Absent,
}

impl ResolutionStatus {
    pub fn name(self) -> &'static str {
        match self {
            ResolutionStatus::Resolved => "resolved",
            ResolutionStatus::Partial => "partial",
            ResolutionStatus::Unsupported => "unsupported",
            ResolutionStatus::Invalid => "invalid",
            ResolutionStatus::Absent => "absent",
        }
    }
}

/// One resolution, with its full event trace and provenance.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Resolution {
    pub pointer: String,
    pub status: ResolutionStatus,
    pub support: SupportLevel,
    pub confidence: Confidence,
    /// What ConfigLens observed as the highest-precedence applicable definition.
    pub observed_value: PublicValue,
    /// Present only when the context justifies calling this the runtime value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_value: Option<PublicValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_id: Option<String>,
    pub evidence: Vec<String>,
    pub events: Vec<Event>,
    pub coverage_gaps: Vec<String>,
    pub notes: Vec<String>,
}

impl Resolution {
    pub fn new(pointer: &str) -> Self {
        Self {
            pointer: pointer.to_string(),
            status: ResolutionStatus::Absent,
            support: SupportLevel::Unsupported,
            confidence: Confidence::Unverified,
            observed_value: PublicValue::redacted(
                "absent",
                crate::model::definition::ValueKind::Null,
            ),
            effective_value: None,
            rule_id: None,
            evidence: Vec::new(),
            events: Vec::new(),
            coverage_gaps: Vec::new(),
            notes: Vec::new(),
        }
    }

    /// The winning definition's location, when one was selected.
    pub fn selected_location(&self) -> Option<&str> {
        self.events
            .iter()
            .find(|e| e.kind == crate::model::event::EventKind::Selected)
            .and_then(|e| e.location.as_deref())
    }

    /// Reason codes of every ignored definition, in event order.
    pub fn ignored_reasons(&self) -> Vec<&str> {
        self.events
            .iter()
            .filter(|e| e.kind == crate::model::event::EventKind::Ignored)
            .filter_map(|e| e.reason_code.as_deref())
            .collect()
    }

    pub fn event_kinds(&self) -> Vec<&'static str> {
        self.events.iter().map(|e| e.kind.name()).collect()
    }

    /// The note that says a list merge is provenance only, when the rule carries one.
    pub fn authorization_note(&self) -> Option<&str> {
        self.notes
            .iter()
            .find(|n| n.contains("not an authorization"))
            .map(String::as_str)
    }

    pub fn effective_str(&self) -> Option<&str> {
        self.effective_value
            .as_ref()
            .and_then(|v| v.as_value())
            .and_then(|v| v.as_str())
    }

    pub fn observed_str(&self) -> Option<&str> {
        self.observed_value.as_value().and_then(|v| v.as_str())
    }
}

/// A diagnostic about a source or about the run. Never contains raw content.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Diagnostic {
    /// `info` | `warning` | `error`
    pub severity: Severity,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

impl Severity {
    pub fn name(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error => "error",
        }
    }
}

/// What a run produced.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RunOutput {
    pub client: String,
    pub adapter: Option<AdapterMetadata>,
    pub plan: DiscoveryPlan,
    pub sources: Vec<Source>,
    pub definitions: Vec<Definition>,
    pub resolutions: Vec<Resolution>,
    pub diagnostics: Vec<Diagnostic>,
    pub coverage: Vec<CoverageGap>,
    pub budget: Option<Budget>,
    /// Pointer → resolution for O(1) lookup by `explain` and `doctor`.
    pub by_pointer: BTreeMap<String, Resolution>,
}

impl RunOutput {
    pub fn resolution(&self, pointer: &str) -> Option<&Resolution> {
        self.by_pointer.get(pointer)
    }

    /// Resolution for a queried pointer, synthesising an `absent` result when unknown.
    pub fn explain(&self, pointer: &str) -> Resolution {
        self.by_pointer
            .get(pointer)
            .cloned()
            .unwrap_or_else(|| Resolution::new(pointer))
    }

    /// Resolution for a queried pointer, using the registry to distinguish an unknown key
    /// (unsupported semantics) from a known key that nothing defines (absent).
    pub fn explain_with(&self, pointer: &str, registry: &Registry) -> Resolution {
        if let Some(found) = self.by_pointer.get(pointer) {
            return found.clone();
        }
        let rule = registry.rule_for(pointer);
        let mut resolution = Resolution::new(pointer);
        resolution.rule_id = Some(rule.rule_id.clone());
        resolution.evidence = rule.evidence.clone();
        resolution.support = rule.support;
        resolution.confidence = Confidence::from_support(rule.support);
        resolution.status = if rule.pattern.is_empty() {
            ResolutionStatus::Unsupported
        } else {
            ResolutionStatus::Absent
        };
        if resolution.status == ResolutionStatus::Unsupported {
            resolution.notes.push(
                "no adapter rule matches this pointer: inventory only, no merge result is claimed"
                    .to_string(),
            );
        }
        resolution
    }

    /// Insert a resolution produced for a query, keeping the report deterministic.
    pub fn insert_resolution(&mut self, resolution: Resolution) {
        self.by_pointer
            .insert(resolution.pointer.clone(), resolution);
        self.resolutions = self.by_pointer.values().cloned().collect();
    }

    pub fn has_gap(&self, kind: GapKind, subject: &str) -> bool {
        self.coverage
            .iter()
            .any(|g| g.kind == kind && g.subject == subject)
    }

    pub fn ignored_definition_count(&self) -> usize {
        self.resolutions
            .iter()
            .flat_map(|r| r.events.iter())
            .filter(|e| e.kind == crate::model::event::EventKind::Ignored)
            .count()
    }
}

/// The engine: discovery → parse → applicability → reduction → policy → coverage.
pub struct Engine<'a> {
    adapter: &'a dyn ClientAdapter,
    ctx: &'a ResolutionContext,
    access: &'a dyn FileAccess,
    paths: &'a PathPolicy,
}

impl<'a> Engine<'a> {
    pub fn new(
        adapter: &'a dyn ClientAdapter,
        ctx: &'a ResolutionContext,
        access: &'a dyn FileAccess,
        paths: &'a PathPolicy,
    ) -> Self {
        Self {
            adapter,
            ctx,
            access,
            paths,
        }
    }

    /// Run the whole pipeline. Never returns raw values: everything is projected.
    pub fn run(&self) -> Result<RunOutput, CliError> {
        let registry = self.adapter.registry();
        let policy = registry.redaction_policy();
        let mode = self.ctx.paths;

        let mut out = RunOutput {
            client: self.adapter.client().id().to_string(),
            adapter: Some(self.adapter.metadata()),
            ..RunOutput::default()
        };
        let mut budget = Budget::new(self.ctx.total_bytes_limit, self.ctx.max_sources);
        let mut sources: Vec<Source> = Vec::new();
        let mut definitions: Vec<Definition> = Vec::new();
        let mut diagnostics: Vec<Diagnostic> = Vec::new();
        let mut coverage: Vec<CoverageGap> = Vec::new();
        let mut next_source = 1u32;
        let mut next_def = 1u32;
        let mut unverified_layout = false;
        // Sources whose file contains a duplicate key: the last definition wins inside the file, and
        // that file-internal decision is not verified against the client loader.
        let mut duplicate_displays: std::collections::BTreeSet<String> =
            std::collections::BTreeSet::new();

        // --- Context-level coverage (spec §5 step 1, §3 version discipline) -------------
        let version_ok = self.ctx.version_supported(
            registry.tested_client_versions(),
            registry.documented_client_versions(),
        );
        if !version_ok {
            coverage.push(CoverageGap::run(
                GapKind::VersionUntested,
                "run",
                "client_version_not_tested_or_documented",
            ));
        }
        // Workspace trust is only a concept for clients that gate project contributions on it
        // (Claude Code). OpenCode has no upstream trust concept (audit O4), so no gap is emitted.
        if self.ctx.trust == crate::context::Trust::Unknown && self.adapter.trust_applicable() {
            coverage.push(CoverageGap::run(
                GapKind::TrustUnknown,
                "run",
                "project_trust_unknown",
            ));
        }
        if !self.ctx.client_arguments.is_empty()
            || !self.ctx.supplied_env.is_empty()
            || self.ctx.simulated
        {
            coverage.push(
                CoverageGap::run(
                    GapKind::SessionStateSimulated,
                    "run",
                    "supplied_session_state_is_not_runtime_proof",
                )
                .non_blocking(),
            );
        }

        // --- Discovery (spec §5 step 2) ------------------------------------------------
        let plan = self.adapter.discover(self.ctx, self.access, self.paths);
        let supplied_roles: Vec<&str> = self
            .ctx
            .supplied_sources
            .iter()
            .filter(|s| s.content.is_some())
            .map(|s| s.role.as_str())
            .collect();
        for (layer_id, reason) in &plan.expected_unobservable {
            let rank = registry.layer(layer_id).map(|l| l.rank).unwrap_or(0);
            if supplied_roles.contains(&layer_id.as_str()) {
                coverage.push(
                    CoverageGap::layer(
                        GapKind::SessionStateSimulated,
                        layer_id,
                        "user_supplied_snapshot_is_evidence_not_runtime_proof",
                        rank,
                    )
                    .non_blocking(),
                );
            } else {
                coverage.push(CoverageGap::layer(
                    GapKind::ExpectedUnobservable,
                    layer_id,
                    reason,
                    rank,
                ));
            }
        }

        // --- Sources: bounded read + parse with spans (spec §5 step 3) ------------------
        for candidate in &plan.candidates {
            if budget.charge_source().is_err() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    code: "source_limit_exceeded".to_string(),
                    location: None,
                    line: None,
                    column: None,
                    message: "the source count limit was reached; the inventory is incomplete"
                        .to_string(),
                });
                break;
            }
            let layer_rank = registry
                .layer(&candidate.layer_id)
                .map(|l| l.rank)
                .unwrap_or(0);
            let note = candidate.note.clone().unwrap_or_default();
            let rank = if note.contains("legacy_pre_v2_1_211_path") {
                layer_rank.saturating_sub(1)
            } else {
                layer_rank
            };
            if note.contains("custom_dir_placement") {
                unverified_layout = true;
                coverage.push(CoverageGap::layer(
                    GapKind::RulePlacementUnverified,
                    &candidate.layer_id,
                    "custom_dir_placement_unverified",
                    rank,
                ));
            }
            if note.contains("project_root_uncertain") {
                coverage.push(CoverageGap::run(
                    GapKind::ProjectBoundaryUncertain,
                    "run",
                    "project_root_uncertain",
                ));
            }

            let source_id = SourceId(next_source);
            next_source += 1;
            let syntax = self.adapter.syntax_for(&candidate.layer_id);
            let applicability = self.adapter.applicability(&candidate.layer_id, self.ctx);
            let class = candidate.class;
            let mut status = SourceStatus::Missing;
            let mut bytes = 0u64;
            let mut text: Option<String> = None;
            let display = match &candidate.path {
                Some(path) => self.paths.display(path, mode),
                None => "session environment:$OPENCODE_CONFIG_CONTENT".to_string(),
            };

            if let Some(inline) = &candidate.inline {
                text = Some(inline.clone());
                status = SourceStatus::Supplied;
                bytes = inline.len() as u64;
            } else if let Some(path) = &candidate.path {
                if let Err(reason) = self.paths.check_read(path) {
                    status = SourceStatus::Blocked(reason.to_string());
                    diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        code: reason.to_string(),
                        location: Some(display.clone()),
                        line: None,
                        column: None,
                        message: "the path is outside the authorised roots and was not read"
                            .to_string(),
                    });
                } else {
                    let outcome = self.access.read_capped(
                        path,
                        self.ctx.limits.max_bytes as u64,
                        &self.paths.approved_roots,
                    );
                    bytes = outcome.bytes;
                    match outcome.kind {
                        crate::filesystem::FileKind::Missing => status = SourceStatus::Missing,
                        crate::filesystem::FileKind::Regular => {
                            if let Some(content) = outcome.content {
                                if budget.charge_bytes(bytes).is_err() {
                                    status = SourceStatus::ResourceLimit("total_bytes".to_string());
                                    diagnostics.push(Diagnostic {
                                        severity: Severity::Error,
                                        code: "total_bytes_exceeded".to_string(),
                                        location: Some(display.clone()),
                                        line: None,
                                        column: None,
                                        message:
                                            "the total byte limit was reached; input is incomplete"
                                                .to_string(),
                                    });
                                } else {
                                    text = Some(content);
                                    status = SourceStatus::Observed;
                                }
                            }
                        }
                        other => {
                            let code = other.diagnostic_code();
                            status = match other {
                                crate::filesystem::FileKind::OverLimit => {
                                    SourceStatus::ResourceLimit("per_file".to_string())
                                }
                                _ => SourceStatus::Blocked(code.to_string()),
                            };
                            diagnostics.push(Diagnostic {
                                severity: Severity::Error,
                                code: code.to_string(),
                                location: Some(display.clone()),
                                line: None,
                                column: None,
                                message: "the candidate was not read".to_string(),
                            });
                        }
                    }
                }
            }

            if status == SourceStatus::Missing && class == crate::adapters::SourceClass::ManagedFile
            {
                coverage.push(CoverageGap::layer(
                    GapKind::ManagedStateUnknown,
                    &candidate.layer_id,
                    "managed_file_absent_does_not_prove_policy_absent",
                    rank,
                ));
            }

            // A candidate whose file is absent is not part of the inventory: its absence is
            // represented by the coverage gaps recorded above (and by the adapter's
            // expected-unobservable gaps), so "no config files" is a genuinely empty `sources`
            // list rather than a list of missing locations (spec §9 example 9). The candidate was
            // still planned and cost a source slot, so limits and diagnostics stay honest.
            if status == SourceStatus::Missing {
                continue;
            }

            // Parse with spans and duplicate-key evidence.
            if let Some(text) = &text {
                match crate::parse::parse(text, syntax, &self.ctx.limits) {
                    Ok(document) => {
                        for raw in &document.definitions {
                            definitions.push(Definition {
                                id: DefinitionId(next_def),
                                source: source_id,
                                pointer: raw.pointer.clone(),
                                value: crate::model::definition::SensitiveValue::new(
                                    raw.value.clone(),
                                ),
                                span: raw.span,
                                line: raw.span.map(|s| s.line_column(text).0),
                                column: raw.span.map(|s| s.line_column(text).1),
                            });
                            next_def += 1;
                        }
                        for duplicate in &document.duplicates {
                            let (line, column) = duplicate.second.line_column(text);
                            duplicate_displays.insert(display.clone());
                            diagnostics.push(Diagnostic {
                                severity: Severity::Warning,
                                code: "duplicate_key".to_string(),
                                location: Some(display.clone()),
                                line: Some(line),
                                column: Some(column),
                                message: "the same key is defined twice in one file; the last definition wins inside the file".to_string(),
                            });
                        }
                    }
                    Err(error) => {
                        status = SourceStatus::Malformed(error.clone());
                        diagnostics.push(Diagnostic {
                            severity: Severity::Error,
                            code: "malformed".to_string(),
                            location: Some(display.clone()),
                            line: Some(error.line),
                            column: Some(error.column),
                            message: format!("the file could not be parsed ({})", error.code),
                        });
                    }
                }
            }

            if applicability.note().is_some() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    code: applicability.note().unwrap_or("inapplicable").to_string(),
                    location: Some(display.clone()),
                    line: None,
                    column: None,
                    message: "the source is not unconditionally applicable".to_string(),
                });
            }

            sources.push(Source {
                id: source_id,
                layer_id: candidate.layer_id.clone(),
                rank,
                class,
                display,
                path: candidate.path.clone(),
                status,
                syntax,
                applicability,
                bytes,
                note: candidate.note.clone(),
            });
        }

        // --- Session arguments and user-supplied sources (spec §4) ---------------------
        let supplied =
            self.ingest_supplied(registry, &mut next_source, &mut next_def, &mut diagnostics);
        for (source, defs) in supplied {
            sources.push(source);
            definitions.extend(defs);
        }

        // --- Reduction, policy and coverage per pointer (spec §5 steps 4-8) ------------
        let inputs = ResolveInputs {
            registry,
            policy: &policy,
            mode,
            ctx: self.ctx,
            access: self.access,
            paths: self.paths,
            sources: &sources,
            definitions: &definitions,
            coverage: &coverage,
            duplicate_displays: &duplicate_displays,
            version_ok,
            unverified_layout,
        };
        let mut pointers: std::collections::BTreeSet<String> = definitions
            .iter()
            .map(|d| d.pointer.clone())
            .filter(|pointer| !pointer.is_empty())
            .collect();
        for pair in registry.env_pairs() {
            if self.ctx.environment_value(&pair.env).is_some() {
                pointers.insert(pair.pointer.clone());
            }
        }
        for pointer in pointers {
            let resolution = resolve_pointer(&inputs, &pointer);
            out.by_pointer.insert(pointer, resolution);
        }
        out.resolutions = out.by_pointer.values().cloned().collect();

        out.plan = plan;
        out.sources = sources;
        out.definitions = definitions;
        out.diagnostics = diagnostics;
        out.coverage = coverage;
        out.budget = Some(budget);
        Ok(out)
    }

    /// Turn `--input-context` sources and session arguments into sources and definitions.
    fn ingest_supplied(
        &self,
        registry: &Registry,
        next_source: &mut u32,
        next_def: &mut u32,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Vec<(Source, Vec<Definition>)> {
        let mut out: Vec<(Source, Vec<Definition>)> = Vec::new();

        for supplied in &self.ctx.supplied_sources {
            let Some(content) = &supplied.content else {
                continue;
            };
            let rank = registry.layer(&supplied.role).map(|l| l.rank).unwrap_or(0);
            let id = SourceId(*next_source);
            *next_source += 1;
            let mut defs = Vec::new();
            let mut status = SourceStatus::Supplied;
            match crate::parse::parse(
                content,
                self.adapter.syntax_for(&supplied.role),
                &self.ctx.limits,
            ) {
                Ok(document) => {
                    for raw in &document.definitions {
                        defs.push(Definition {
                            id: DefinitionId(*next_def),
                            source: id,
                            pointer: raw.pointer.clone(),
                            value: crate::model::definition::SensitiveValue::new(raw.value.clone()),
                            span: raw.span,
                            line: raw.span.map(|s| s.line_column(content).0),
                            column: raw.span.map(|s| s.line_column(content).1),
                        });
                        *next_def += 1;
                    }
                }
                Err(error) => {
                    status = SourceStatus::Malformed(error.clone());
                    diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        code: "malformed".to_string(),
                        location: Some(
                            supplied
                                .path
                                .clone()
                                .unwrap_or_else(|| format!("supplied:{}", supplied.role)),
                        ),
                        line: Some(error.line),
                        column: Some(error.column),
                        message: "the supplied snapshot could not be parsed".to_string(),
                    });
                }
            }
            out.push((
                Source {
                    id,
                    layer_id: supplied.role.clone(),
                    rank,
                    class: crate::adapters::SourceClass::ManagedFile,
                    display: format!("supplied:{}", supplied.role),
                    path: None,
                    status,
                    syntax: self.adapter.syntax_for(&supplied.role),
                    applicability: crate::adapters::Applicability::Applicable,
                    bytes: content.len() as u64,
                    note: Some("user_provided_snapshot".to_string()),
                },
                defs,
            ));
        }

        if !self.ctx.client_arguments.is_empty() {
            let id = SourceId(*next_source);
            *next_source += 1;
            let rank = registry
                .layer("cc.session_args")
                .map(|l| l.rank)
                .unwrap_or(40);
            let mut defs = Vec::new();
            let mut index = 0usize;
            while index < self.ctx.client_arguments.len() {
                let flag = self.ctx.client_arguments[index].clone();
                let value = self.ctx.client_arguments.get(index + 1).cloned();
                match (flag.as_str(), value) {
                    ("--settings", Some(json_text)) if json_text.trim_start().starts_with('{') => {
                        if let Ok(document) = crate::parse::parse(
                            &json_text,
                            self.adapter.syntax_for("cc.session_args"),
                            &self.ctx.limits,
                        ) {
                            for raw in &document.definitions {
                                defs.push(Definition {
                                    id: DefinitionId(*next_def),
                                    source: id,
                                    pointer: raw.pointer.clone(),
                                    value: crate::model::definition::SensitiveValue::new(
                                        raw.value.clone(),
                                    ),
                                    span: raw.span,
                                    line: raw.span.map(|s| s.line_column(&json_text).0),
                                    column: raw.span.map(|s| s.line_column(&json_text).1),
                                });
                                *next_def += 1;
                            }
                        } else {
                            diagnostics.push(Diagnostic {
                                severity: Severity::Error,
                                code: "malformed".to_string(),
                                location: Some("supplied session arguments:--settings".to_string()),
                                line: None,
                                column: None,
                                message: "the inline --settings JSON could not be parsed"
                                    .to_string(),
                            });
                        }
                        index += 2;
                    }
                    ("--model", Some(model)) => {
                        defs.push(synthetic_definition(
                            id,
                            "/model",
                            serde_json::Value::String(model),
                            next_def,
                        ));
                        index += 2;
                    }
                    ("--effort", Some(level)) => {
                        defs.push(synthetic_definition(
                            id,
                            "/effortLevel",
                            serde_json::Value::String(level),
                            next_def,
                        ));
                        index += 2;
                    }
                    (_, _) => {
                        diagnostics.push(Diagnostic {
                            severity: Severity::Warning,
                            code: "unrecognised_session_flag".to_string(),
                            location: Some("supplied session arguments".to_string()),
                            line: None,
                            column: None,
                            // The raw flag is never echoed: a client argument can itself be a
                            // credential (`--api-key=…`), and spec §7 forbids exposing command
                            // arguments in reports. The position is enough to act on.
                            message: format!(
                                "argument #{} is not a verified option; resolution for affected keys is partial",
                                index + 1
                            ),
                        });
                        index += 1;
                    }
                }
            }
            out.push((
                Source {
                    id,
                    layer_id: "cc.session_args".to_string(),
                    rank,
                    class: crate::adapters::SourceClass::SessionArgs,
                    display: "supplied session arguments".to_string(),
                    path: None,
                    status: SourceStatus::Supplied,
                    syntax: self.adapter.syntax_for("cc.session_args"),
                    applicability: crate::adapters::Applicability::Applicable,
                    bytes: defs.len() as u64,
                    note: Some("simulated_session_arguments".to_string()),
                },
                defs,
            ));
        }

        out
    }
}

fn synthetic_definition(
    source: SourceId,
    pointer: &str,
    value: serde_json::Value,
    next_def: &mut u32,
) -> Definition {
    let id = DefinitionId(*next_def);
    *next_def += 1;
    Definition {
        id,
        source,
        pointer: pointer.to_string(),
        value: crate::model::definition::SensitiveValue::new(value),
        span: None,
        line: None,
        column: None,
    }
}

/// Everything `resolve_pointer` may read. Pure function of the context, so tests can call it.
struct ResolveInputs<'a> {
    registry: &'a crate::adapters::Registry,
    policy: &'a crate::security::RedactionPolicy,
    mode: crate::security::PathMode,
    ctx: &'a ResolutionContext,
    access: &'a dyn FileAccess,
    paths: &'a PathPolicy,
    sources: &'a [Source],
    definitions: &'a [Definition],
    coverage: &'a [CoverageGap],
    duplicate_displays: &'a std::collections::BTreeSet<String>,
    version_ok: bool,
    unverified_layout: bool,
}

/// One contribution to a key, ordered by precedence. Holds only redacted values.
#[derive(Debug, Clone)]
struct Contribution {
    rank: u32,
    source_id: Option<SourceId>,
    definition_id: Option<DefinitionId>,
    location: String,
    value: PublicValue,
    /// Set when the definition must not participate, with the reason code.
    ignored: Option<String>,
    /// Set when the value was derived from an adapter-verified interpolation.
    transformed: Option<String>,
    /// Set when a verified interpolation could not be resolved, with the reason code.
    unresolved: Option<String>,
    /// Reason code to use when this contribution is the winner.
    selected_reason: Option<String>,
    /// Raw value, used only for replacement and allowlist decisions. Never rendered.
    raw: Option<serde_json::Value>,
    value_kind: ValueKind,
    source_note: String,
    source_class: Option<crate::adapters::SourceClass>,
}

fn project_with_interpolation(
    inputs: &ResolveInputs<'_>,
    pointer: &str,
    rule: &crate::adapters::RuleMatch,
    raw: &serde_json::Value,
) -> (PublicValue, Option<String>, Option<String>) {
    if let serde_json::Value::String(text) = raw {
        if rule.interpolation.contains("env") {
            if let Some(name) = between(text, "{env:", "}") {
                return match inputs.ctx.environment_value(name) {
                    Some(_) => (
                        PublicValue::redacted("environment_value", ValueKind::String),
                        Some("environment_value".to_string()),
                        None,
                    ),
                    None => (
                        PublicValue::redacted("interpolation_source", ValueKind::String),
                        None,
                        Some("env_variable_not_supplied".to_string()),
                    ),
                };
            }
        }
        if rule.interpolation.contains("file") {
            if let Some(reference) = between(text, "{file:", "}") {
                let Some(path) = expand_reference(reference, inputs.ctx) else {
                    return (
                        PublicValue::redacted("interpolation_source", ValueKind::String),
                        None,
                        Some("file_reference_not_absolute".to_string()),
                    );
                };
                if inputs.paths.check_read(&path).is_err() {
                    return (
                        PublicValue::redacted("interpolation_source", ValueKind::String),
                        None,
                        Some("file_reference_blocked".to_string()),
                    );
                }
                let outcome = inputs.access.read_capped(
                    &path,
                    inputs.ctx.limits.max_bytes as u64,
                    &inputs.paths.approved_roots,
                );
                return match outcome.kind {
                    FileKind::Regular => (
                        PublicValue::redacted("file_content", ValueKind::String),
                        Some("file_content".to_string()),
                        None,
                    ),
                    _ => (
                        PublicValue::redacted("interpolation_source", ValueKind::String),
                        None,
                        Some("file_reference_unreadable".to_string()),
                    ),
                };
            }
        }
    }
    (
        crate::security::redact_value(pointer, raw, inputs.policy, inputs.mode),
        None,
        None,
    )
}

fn between<'a>(text: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let from = text.find(start)? + start.len();
    let rest = &text[from..];
    let to = rest.find(end)?;
    Some(&rest[..to])
}

fn expand_reference(reference: &str, ctx: &ResolutionContext) -> Option<std::path::PathBuf> {
    if let Some(rest) = reference.strip_prefix("~/") {
        return Some(ctx.home.join(rest));
    }
    let path = std::path::Path::new(reference);
    if path.is_absolute() {
        Some(path.to_path_buf())
    } else {
        None
    }
}

fn collect_contributions(
    inputs: &ResolveInputs<'_>,
    pointer: &str,
    rule: &crate::adapters::RuleMatch,
) -> Vec<Contribution> {
    let mut out: Vec<Contribution> = Vec::new();

    for definition in inputs.definitions.iter().filter(|d| d.pointer == pointer) {
        let Some(source) = inputs.sources.iter().find(|s| s.id == definition.source) else {
            continue;
        };
        let raw = definition.value.raw().clone();
        let (value, transformed, unresolved) =
            project_with_interpolation(inputs, pointer, rule, &raw);
        let note = source.note.clone().unwrap_or_default();
        let mut ignored = None;
        if !source.status.is_usable() {
            ignored = Some(format!("source_{}", source.status.code()));
        }
        if let crate::adapters::Applicability::Skipped(reason) = &source.applicability {
            ignored = Some(reason.clone());
        }
        if note.contains("legacy_pre_v2_1_211_path") {
            ignored = Some("legacy_pre_v2_1_211_path".to_string());
        }
        if rule.trust_gated
            && inputs.ctx.trust != crate::context::Trust::Trusted
            && source.class == crate::adapters::SourceClass::ProjectShared
        {
            // Key-granular trust gate (audit C4). Upstream applies only `permissions.allow`,
            // `additionalDirectories`, `extraKnownMarketplaces` and most `env` values after the
            // folder is trusted; `deny`/`ask` apply right away, and an *untracked* local file's
            // allow rules apply without the trust step. So only a trust-gated key from the *shared*
            // project file is ignored here; local and the deny/ask rules are not. The reason is the
            // adapter's own applicability note (`workspace_untrusted` vs `trust_unknown`).
            if ignored.is_none() {
                ignored = Some(
                    source
                        .applicability
                        .note()
                        .unwrap_or("trust_unknown")
                        .to_string(),
                );
            }
        }
        let excluded_value = raw
            .as_str()
            .map(|v| rule.scope_excluded_values.iter().any(|e| e == v))
            .unwrap_or(false);
        if excluded_value && rule.scope_excluded_from.contains(&source.layer_id) {
            ignored = Some("scope_excluded_value".to_string());
        }
        out.push(Contribution {
            rank: source.rank,
            source_id: Some(source.id),
            definition_id: Some(definition.id),
            location: match (definition.line, definition.column) {
                (Some(line), Some(column)) => format!("{}:{line}:{column}", source.display),
                _ => source.display.clone(),
            },
            value,
            ignored,
            transformed,
            unresolved,
            selected_reason: None,
            raw: Some(raw),
            value_kind: definition.kind(),
            source_note: note,
            source_class: Some(source.class),
        });
    }

    // Documented environment pairs (spec §3 "environment precedence is pair-specific").
    for pair in inputs
        .registry
        .env_pairs()
        .iter()
        .filter(|p| p.pointer == pointer)
    {
        let Some(value) = inputs.ctx.environment_value(&pair.env) else {
            continue;
        };
        let file_sets_key = out.iter().any(|c| {
            matches!(
                c.source_class,
                Some(crate::adapters::SourceClass::User)
                    | Some(crate::adapters::SourceClass::ProjectShared)
                    | Some(crate::adapters::SourceClass::ProjectLocal)
                    | Some(crate::adapters::SourceClass::ManagedFile)
                    | Some(crate::adapters::SourceClass::CustomPath)
                    | Some(crate::adapters::SourceClass::CustomDir)
            )
        });
        if pair.mode == "only_when_no_file_sets_key" && file_sets_key {
            continue;
        }
        let projected = crate::security::redact_value(
            pointer,
            &serde_json::Value::String(value.to_string()),
            inputs.policy,
            inputs.mode,
        );
        let reason = if pair.mode == "overrides_files" {
            "env_pair_overrides_files"
        } else {
            "env_pair_default"
        };
        out.push(Contribution {
            rank: 35,
            source_id: None,
            definition_id: None,
            location: format!("session environment:{}", pair.env),
            value: projected,
            ignored: None,
            transformed: None,
            unresolved: None,
            selected_reason: Some(reason.to_string()),
            raw: Some(serde_json::Value::String(value.to_string())),
            value_kind: ValueKind::String,
            source_note: "documented_environment_pair".to_string(),
            source_class: None,
        });
    }

    out.sort_by_key(|c| {
        (
            c.rank,
            c.source_id.map(|s| s.0).unwrap_or(0),
            c.definition_id.map(|d| d.0).unwrap_or(0),
        )
    });
    out
}

fn push_event(
    resolution: &mut Resolution,
    contribution: &Contribution,
    kind: crate::model::event::EventKind,
    rule_id: &str,
    reason: Option<&str>,
) {
    let mut event = Event::new(kind).with_location(contribution.location.clone());
    if let (Some(definition), Some(source)) = (contribution.definition_id, contribution.source_id) {
        event = event.with_definition(definition, source);
    }
    event = event.with_value(contribution.value.clone());
    event = event.with_rule(rule_id);
    if let Some(reason) = reason {
        event = event.with_reason(reason);
    }
    resolution.events.push(event);
}

fn resolve_pointer(inputs: &ResolveInputs<'_>, pointer: &str) -> Resolution {
    let rule = inputs.registry.rule_for(pointer);
    let mut resolution = Resolution::new(pointer);
    resolution.rule_id = Some(rule.rule_id.clone());
    resolution.evidence = rule.evidence.clone();
    resolution.support = if inputs.version_ok {
        rule.support
    } else {
        SupportLevel::Unsupported
    };
    resolution.confidence = Confidence::from_support(resolution.support);
    if rule.authorization.as_deref() == Some("none") {
        resolution.notes.push(
            "this is a provenance merge of list entries and not an authorization decision"
                .to_string(),
        );
    }
    if !inputs.version_ok {
        resolution.notes.push(
            "the claimed client version is not in the adapter's tested or documented set; no merge result is claimed"
                .to_string(),
        );
    }

    let contributions = collect_contributions(inputs, pointer, &rule);

    // Nothing defines this key.
    if contributions.is_empty() {
        resolution.status = ResolutionStatus::Absent;
        resolution.observed_value =
            PublicValue::redacted("absent", crate::model::definition::ValueKind::Null);
        return resolution;
    }

    // Unknown keys are inventory only: never an invented winner (spec §3 rule 6).
    if rule.operator == Operator::InventoryOnly {
        for contribution in &contributions {
            push_event(
                &mut resolution,
                contribution,
                EventKind::Ignored,
                &rule.rule_id,
                Some("inventory_only"),
            );
        }
        let unknown = rule.pattern.is_empty();
        resolution.status = if unknown {
            ResolutionStatus::Unsupported
        } else {
            ResolutionStatus::Partial
        };
        if !unknown {
            resolution.notes.push(
                "known key with inventory_only support: no merge result is claimed".to_string(),
            );
        }
        // Inventory-only means the value is not interpreted at all: describe the shape, never the
        // content (spec §3 rule 6, §7 "unknown keys themselves may contain credentials").
        if let Some(first) = contributions.first() {
            resolution.observed_value = PublicValue::redacted("inventory_only", first.value_kind);
        }
        return resolution;
    }

    // Containers: object merge is reported per child pointer.
    let object_like = contributions
        .iter()
        .filter(|c| c.value_kind == ValueKind::Object)
        .count();
    let list_like = contributions
        .iter()
        .filter(|c| c.value_kind == ValueKind::Array)
        .count();
    if rule.operator == Operator::ObjectMerge && object_like > 0 {
        for contribution in &contributions {
            push_event(
                &mut resolution,
                contribution,
                EventKind::Contributed,
                &rule.rule_id,
                Some("container_child_merge"),
            );
        }
        resolution.status = ResolutionStatus::Partial;
        resolution
            .notes
            .push("container value: each child pointer is resolved on its own".to_string());
        if let Some(first) = contributions.first() {
            resolution.observed_value = first.value.clone();
        }
        attach_blocking_gaps(
            inputs,
            &mut resolution,
            contributions.iter().map(|c| c.rank).min().unwrap_or(0),
            &rule,
        );
        if object_like != contributions.len() {
            resolution
                .notes
                .push("the same pointer holds both container and scalar values".to_string());
        }
        return resolution;
    }

    // Lists: documented stable concatenation with duplicate removal.
    if rule.operator == Operator::ConcatDedupe
        || (rule.operator == Operator::ObjectMerge && list_like > 0)
    {
        let mut combined: Vec<serde_json::Value> = Vec::new();
        let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for contribution in &contributions {
            if let Some(reason) = &contribution.ignored {
                push_event(
                    &mut resolution,
                    contribution,
                    EventKind::Ignored,
                    &rule.rule_id,
                    Some(reason),
                );
                continue;
            }
            match contribution.raw.as_ref().and_then(|v| v.as_array()) {
                Some(items) => {
                    for item in items {
                        let key = serde_json::to_string(item).unwrap_or_default();
                        if seen.insert(key) {
                            combined.push(item.clone());
                        }
                    }
                    push_event(
                        &mut resolution,
                        contribution,
                        EventKind::Contributed,
                        &rule.rule_id,
                        Some("list_contribution"),
                    );
                }
                None => push_event(
                    &mut resolution,
                    contribution,
                    EventKind::Ignored,
                    &rule.rule_id,
                    Some("not_a_list_value"),
                ),
            }
        }
        let projected = crate::security::redact_value(
            pointer,
            &serde_json::Value::Array(combined.clone()),
            inputs.policy,
            inputs.mode,
        );
        resolution.observed_value = projected.clone();
        let winner_rank = contributions
            .iter()
            .filter(|c| c.ignored.is_none())
            .map(|c| c.rank)
            .max()
            .unwrap_or(0);
        resolution.status = ResolutionStatus::Partial;
        attach_blocking_gaps(inputs, &mut resolution, winner_rank, &rule);
        if resolution.coverage_gaps.is_empty()
            && !inputs.unverified_layout
            && !has_jsonc_coexistence(&contributions)
        {
            resolution.status = ResolutionStatus::Resolved;
            resolution.effective_value = Some(projected);
        }
        return resolution;
    }

    // Scalar replacement (also used for arrays under an ordered replace).
    let winner_index = contributions
        .iter()
        .rposition(|c| c.ignored.is_none() && c.unresolved.is_none());
    for (index, contribution) in contributions.iter().enumerate() {
        if let Some(reason) = &contribution.ignored {
            push_event(
                &mut resolution,
                contribution,
                EventKind::Ignored,
                &rule.rule_id,
                Some(reason),
            );
            continue;
        }
        if let Some(reason) = &contribution.unresolved {
            push_event(
                &mut resolution,
                contribution,
                EventKind::Unresolved,
                &rule.rule_id,
                Some(reason),
            );
            continue;
        }
        if Some(index) == winner_index {
            let reason = contribution
                .selected_reason
                .clone()
                .unwrap_or_else(|| "observed_winner".to_string());
            push_event(
                &mut resolution,
                contribution,
                EventKind::Selected,
                &rule.rule_id,
                Some(&reason),
            );
        } else {
            let reason = if rule.operator == Operator::OrderedReplace {
                "replaced_by_higher_precedence_whole_value"
            } else {
                "replaced_by_later_layer"
            };
            push_event(
                &mut resolution,
                contribution,
                EventKind::Replaced,
                &rule.rule_id,
                Some(reason),
            );
        }
        if let Some(transform) = &contribution.transformed {
            let mut event = Event::new(EventKind::Transformed)
                .with_location(contribution.location.clone())
                .with_value(PublicValue::redacted(transform, ValueKind::String))
                .with_rule(&rule.rule_id)
                .with_reason(transform);
            if let (Some(definition), Some(source)) =
                (contribution.definition_id, contribution.source_id)
            {
                event = event.with_definition(definition, source);
            }
            resolution.events.push(event);
        }
    }

    let Some(winner_index) = winner_index else {
        resolution.status = ResolutionStatus::Partial;
        if let Some(first) = contributions.first() {
            resolution.observed_value = first.value.clone();
        }
        attach_blocking_gaps(inputs, &mut resolution, 0, &rule);
        return resolution;
    };
    let winner = &contributions[winner_index];
    resolution.observed_value = winner.value.clone();

    // Any already-ignored value must stay visible: contributions were never filtered out.

    // A malformed source at or above the winner prevents any effective claim. A *blocked* or
    // over-limit source is reported as a partial condition instead: it is a gap in what we could
    // read, not proof that the configuration itself is broken.
    let invalid_source = inputs.sources.iter().any(|s| {
        matches!(s.status, SourceStatus::Malformed(_))
            && s.rank >= winner.rank
            && s.applicability.is_applicable()
    });
    let unreadable_source = inputs.sources.iter().any(|s| {
        matches!(
            s.status,
            SourceStatus::Blocked(_) | SourceStatus::ResourceLimit(_) | SourceStatus::Unreadable
        ) && s.rank >= winner.rank
    });

    resolution.status = ResolutionStatus::Resolved;
    attach_blocking_gaps(inputs, &mut resolution, winner.rank, &rule);
    if let Some(reason) = &winner.unresolved {
        resolution
            .notes
            .push(format!("the value could not be resolved ({reason})"));
        resolution.status = ResolutionStatus::Partial;
    }
    if winner.transformed.is_some() {
        resolution.notes.push(
            "the value is derived from an adapter-verified interpolation; redaction taint applies"
                .to_string(),
        );
        resolution.status = ResolutionStatus::Partial;
    }
    if has_jsonc_coexistence(&contributions) {
        resolution.notes.push(
            "both opencode.json and opencode.jsonc are present (jsonc_coexistence_unverified): the preference between them is not verified"
                .to_string(),
        );
        resolution.status = ResolutionStatus::Partial;
    }
    if inputs.unverified_layout {
        resolution
            .notes
            .push("an unverified source layout is present in this context".to_string());
        resolution.status = ResolutionStatus::Partial;
    }
    if winner.source_note.contains("simulated_session_arguments") {
        resolution
            .notes
            .push("the winning value comes from supplied session state (simulated)".to_string());
        resolution.status = ResolutionStatus::Partial;
    }
    if let Some(source_id) = winner.source_id {
        if let Some(source) = inputs.sources.iter().find(|s| s.id == source_id) {
            if inputs.duplicate_displays.contains(&source.display) {
                resolution
                    .notes
                    .push("the winning file defines this key more than once; the file-internal winner is not verified".to_string());
                resolution.status = ResolutionStatus::Partial;
            }
        }
    }
    if invalid_source {
        resolution.status = ResolutionStatus::Invalid;
    } else if unreadable_source {
        resolution
            .notes
            .push("a source at or above the winner could not be read or was over a limit; the winner is observed only".to_string());
        resolution.status = ResolutionStatus::Partial;
    }
    if contributions
        .iter()
        .any(|c| c.ignored.as_deref() == Some("scope_excluded_value"))
    {
        resolution.notes.push(
            "a value of this key does not take effect from project or local settings (documented scope exclusion)"
                .to_string(),
        );
        resolution.status = ResolutionStatus::Partial;
    }

    // Managed allowlist constraint (spec §5 step 7: a distinct policy phase).
    if pointer == "/model" {
        if let Some(constraint) = inputs
            .definitions
            .iter()
            .find(|d| d.pointer == "/availableModels")
        {
            let source = inputs.sources.iter().find(|s| s.id == constraint.source);
            let mut event = Event::new(EventKind::Constrained)
                .with_rule("claude.availableModels.managed-lock")
                .with_value(crate::security::redact_value(
                    "/availableModels",
                    constraint.value.raw(),
                    inputs.policy,
                    inputs.mode,
                ))
                .with_reason("managed_allowlist");
            if let Some(source) = source {
                event = event.with_location(source.display.clone());
            }
            resolution.events.push(event);
            let allowed = constraint
                .value
                .raw()
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|v| v.as_str())
                        .any(|v| Some(v) == winner.raw.as_ref().and_then(|r| r.as_str()))
                })
                .unwrap_or(false);
            if !allowed {
                resolution.notes.push(
                    "the observed model is not in the managed allowlist; the runtime choice inside the allowlist is unknown"
                        .to_string(),
                );
                resolution.status = ResolutionStatus::Partial;
            }
        }
    }

    if resolution.status == ResolutionStatus::Resolved && resolution.coverage_gaps.is_empty() {
        resolution.effective_value = Some(winner.value.clone());
    } else if resolution.status == ResolutionStatus::Resolved {
        resolution.status = ResolutionStatus::Partial;
    }

    // An untested or unknown client version can never justify an effective value: the inventory is
    // still available, but no exact resolution claim is made (spec §3 "Compatibility registry").
    if !inputs.version_ok {
        resolution.status = ResolutionStatus::Unsupported;
        resolution.support = SupportLevel::Unsupported;
        resolution.confidence = Confidence::Unverified;
        resolution.effective_value = None;
    }
    resolution
}

fn has_jsonc_coexistence(contributions: &[Contribution]) -> bool {
    contributions
        .iter()
        .filter(|c| c.source_note.contains("jsonc_coexistence"))
        .count()
        >= 2
}

fn attach_blocking_gaps(
    inputs: &ResolveInputs<'_>,
    resolution: &mut Resolution,
    winner_rank: u32,
    rule: &crate::adapters::RuleMatch,
) {
    for gap in inputs.coverage {
        let rank_blocks = gap.rank.map(|rank| rank > winner_rank).unwrap_or(false);
        let trust_blocks = gap.kind == GapKind::TrustUnknown
            && rule.trust_gated
            && resolution
                .events
                .iter()
                .any(|e| e.reason_code.as_deref() == Some("trust_unknown"));
        let boundary_blocks = gap.kind == GapKind::ProjectBoundaryUncertain;
        if gap.blocks_completeness && (rank_blocks || trust_blocks || boundary_blocks) {
            let described = gap.describe();
            if !resolution.coverage_gaps.contains(&described) {
                resolution.coverage_gaps.push(described);
            }
        }
    }
}
