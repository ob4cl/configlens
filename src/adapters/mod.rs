//! Client adapters and the bundled compatibility registry (spec §3, §6).
//!
//! The registry JSON in `compatibility/<client>/registry.json` is compiled into the binary and is
//! the single source of truth for layers, per-key rules, support levels and redaction allowlists.
//! Documentation in `compatibility/<client>/registry.md` describes the same data for reviewers.

pub mod claude;
pub mod opencode;

use serde::Deserialize;
use std::path::PathBuf;

use crate::context::{Os, ResolutionContext, Trust};
use crate::filesystem::FileAccess;
use crate::parse::Syntax;
use crate::pattern::{matches, specificity};
use crate::security::{PathPolicy, RedactionPolicy};

/// Adapters that exist in this release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Client {
    OpenCode,
    Claude,
}

impl Client {
    pub fn id(self) -> &'static str {
        match self {
            Client::OpenCode => "opencode",
            Client::Claude => "claude",
        }
    }

    pub fn display(self) -> &'static str {
        match self {
            Client::OpenCode => "OpenCode",
            Client::Claude => "Claude Code",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "opencode" => Some(Client::OpenCode),
            "claude" | "claude-code" => Some(Client::Claude),
            _ => None,
        }
    }

    pub fn all() -> &'static [Client] {
        &[Client::OpenCode, Client::Claude]
    }

    /// Clients explicitly named in the specification as *not* implemented.
    pub fn unsupported() -> &'static [(&'static str, &'static str)] {
        &[
            (
                "codex",
                "0.2 backlog — TOML, trust gating and requirements model not implemented",
            ),
            (
                "gemini",
                "0.2 backlog — settings/trust behaviour not verified",
            ),
            ("aider", "backlog — loader semantics not inspected"),
            ("cursor", "not investigated for this specification"),
            ("copilot-cli", "not investigated for this specification"),
            ("cline", "not investigated for this specification"),
            ("roo", "not investigated for this specification"),
            ("hermes", "not investigated for this specification"),
        ]
    }
}

/// Support state of an advertised rule (spec §3 "Required support states").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportLevel {
    Unsupported,
    InventoryOnly,
    Documented,
    FixtureVerified,
    UpstreamConformanceVerified,
}

impl SupportLevel {
    pub fn name(self) -> &'static str {
        match self {
            SupportLevel::Unsupported => "unsupported",
            SupportLevel::InventoryOnly => "inventory_only",
            SupportLevel::Documented => "documented",
            SupportLevel::FixtureVerified => "fixture_verified",
            SupportLevel::UpstreamConformanceVerified => "upstream_conformance_verified",
        }
    }

    pub fn parse(text: &str) -> Self {
        match text {
            "upstream_conformance_verified" => SupportLevel::UpstreamConformanceVerified,
            "fixture_verified" => SupportLevel::FixtureVerified,
            "documented" => SupportLevel::Documented,
            "inventory_only" => SupportLevel::InventoryOnly,
            _ => SupportLevel::Unsupported,
        }
    }
}

/// Reduction operator. Only these exist; an unknown key never gets one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    Replace,
    ObjectMerge,
    OrderedReplace,
    ConcatDedupe,
    InventoryOnly,
    Constraint,
}

impl Operator {
    pub fn name(self) -> &'static str {
        match self {
            Operator::Replace => "replace",
            Operator::ObjectMerge => "object-merge",
            Operator::OrderedReplace => "ordered-replace",
            Operator::ConcatDedupe => "concat-dedupe",
            Operator::InventoryOnly => "inventory-only",
            Operator::Constraint => "constraint",
        }
    }

    pub fn parse(text: &str) -> Self {
        match text {
            "replace" => Operator::Replace,
            "object-merge" => Operator::ObjectMerge,
            "ordered-replace" => Operator::OrderedReplace,
            "concat-dedupe" => Operator::ConcatDedupe,
            "constraint" => Operator::Constraint,
            _ => Operator::InventoryOnly,
        }
    }
}

/// How a source may be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Applicability {
    Applicable,
    Skipped(String),
    Conditional(String),
}

impl Applicability {
    pub fn is_applicable(&self) -> bool {
        matches!(self, Applicability::Applicable)
    }

    pub fn note(&self) -> Option<&str> {
        match self {
            Applicability::Applicable => None,
            Applicability::Skipped(r) | Applicability::Conditional(r) => Some(r),
        }
    }
}

/// Broad class of a discovered source, used for grouping and trust rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceClass {
    RemoteDefaults,
    User,
    ProjectShared,
    ProjectLocal,
    CustomPath,
    CustomDir,
    Inline,
    ManagedFile,
    PlatformManaged,
    SessionArgs,
    SessionEnv,
    NotASettingsDocument,
}

impl SourceClass {
    pub fn name(self) -> &'static str {
        match self {
            SourceClass::RemoteDefaults => "remote_defaults",
            SourceClass::User => "user",
            SourceClass::ProjectShared => "project_shared",
            SourceClass::ProjectLocal => "project_local",
            SourceClass::CustomPath => "custom_path",
            SourceClass::CustomDir => "custom_dir",
            SourceClass::Inline => "inline",
            SourceClass::ManagedFile => "managed_file",
            SourceClass::PlatformManaged => "platform_managed",
            SourceClass::SessionArgs => "session_arguments",
            SourceClass::SessionEnv => "session_environment",
            SourceClass::NotASettingsDocument => "not_a_settings_document",
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct EvidenceEntry {
    pub id: String,
    pub kind: String,
    pub url: String,
    pub retrieved: String,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct LayerEntry {
    pub id: String,
    pub rank: u32,
    pub observable: bool,
    pub location: String,
    #[serde(default)]
    pub unobservable_reason: Option<String>,
    #[serde(default)]
    pub placement_verified: Option<bool>,
    #[serde(default)]
    pub parse_supported: Option<bool>,
    #[serde(default)]
    pub supplied_only: Option<bool>,
}

impl LayerEntry {
    /// `Some(reason)` when this layer can never be fully observed (spec §7: never assume absent).
    pub fn gap_reason(&self) -> Option<String> {
        if !self.observable {
            return Some(
                self.unobservable_reason
                    .clone()
                    .unwrap_or_else(|| "unobservable_layer".to_string()),
            );
        }
        if self.parse_supported == Some(false) {
            return Some("platform_prefs_not_parsed".to_string());
        }
        None
    }

    pub fn is_supplied_only(&self) -> bool {
        self.supplied_only.unwrap_or(false)
    }

    pub fn placement_verified(&self) -> bool {
        self.placement_verified.unwrap_or(true)
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct RuleEntry {
    pub pointer: String,
    pub operator: String,
    pub rule_id: String,
    pub support: String,
    #[serde(default)]
    pub evidence: Vec<String>,
    #[serde(default)]
    pub interpolation: Option<String>,
    #[serde(default)]
    pub public_enums: Option<Vec<String>>,
    #[serde(default)]
    pub public_strings: Option<bool>,
    #[serde(default)]
    pub model_name: Option<bool>,
    #[serde(default)]
    pub public_urls: Option<bool>,
    #[serde(default)]
    pub public_host_list: Option<bool>,
    #[serde(default)]
    pub trust_gated: Option<bool>,
    #[serde(default)]
    pub authorization: Option<String>,
    #[serde(default)]
    pub redact_subtree: Option<String>,
    #[serde(default)]
    pub scope_excluded_values: Option<Vec<String>>,
    #[serde(default)]
    pub scope_excluded_from: Option<Vec<String>>,
    #[serde(default)]
    pub tests: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct EnvPairEntry {
    pub env: String,
    pub pointer: String,
    pub mode: String,
    pub evidence: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct RegistryFile {
    pub registry_version: String,
    pub client: String,
    pub adapter_revision: String,
    pub evidence: Vec<EvidenceEntry>,
    pub tested_client_versions: Vec<String>,
    pub documented_client_versions: Vec<String>,
    #[serde(default)]
    pub pinned_commit: Option<String>,
    /// How upstream combines multiple managed sources. Upstream default is `first-wins` (take the
    /// highest-ranked source that delivers any policy key and ignore the rest); `merge` is opt-in
    /// from Claude Code v2.1.242. ConfigLens models single-source managed resolution only.
    #[serde(default)]
    pub managed_sources_behavior: Option<String>,
    pub conformance_status: String,
    pub layers: Vec<LayerEntry>,
    pub rules: Vec<RuleEntry>,
    #[serde(default)]
    pub env_pairs: Vec<EnvPairEntry>,
    #[serde(default)]
    pub sensitive_field_patterns: Vec<String>,
}

/// The registry as the engine uses it.
#[derive(Debug, Clone)]
pub struct Registry {
    file: RegistryFile,
}

/// The winning rule for one pointer.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleMatch {
    pub rule_id: String,
    pub pattern: String,
    pub operator: Operator,
    pub support: SupportLevel,
    pub evidence: Vec<String>,
    pub interpolation: String,
    pub authorization: Option<String>,
    pub trust_gated: bool,
    pub scope_excluded_values: Vec<String>,
    pub scope_excluded_from: Vec<String>,
    pub tests: Vec<String>,
}

impl Registry {
    /// Load the registry bundled for `client`.
    pub fn load(client: Client) -> Result<Registry, String> {
        let text = match client {
            Client::OpenCode => include_str!("../../compatibility/opencode/registry.json"),
            Client::Claude => include_str!("../../compatibility/claude/registry.json"),
        };
        let file: RegistryFile =
            serde_json::from_str(text).map_err(|e| format!("bundled registry is invalid: {e}"))?;
        if file.client != client.id() {
            return Err("bundled registry client does not match the adapter".to_string());
        }
        Ok(Registry { file })
    }

    /// A registry with no layers and no rules, used only if the bundled data were unreadable.
    /// Every lookup then degrades to `inventory_only` instead of guessing.
    pub fn unavailable(client: &str) -> Registry {
        Registry {
            file: RegistryFile {
                registry_version: "0".to_string(),
                client: client.to_string(),
                adapter_revision: "unavailable".to_string(),
                evidence: Vec::new(),
                tested_client_versions: Vec::new(),
                documented_client_versions: Vec::new(),
                pinned_commit: None,
                managed_sources_behavior: None,
                conformance_status: "unavailable".to_string(),
                layers: Vec::new(),
                rules: Vec::new(),
                env_pairs: Vec::new(),
                sensitive_field_patterns: Vec::new(),
            },
        }
    }

    pub fn client(&self) -> &str {
        &self.file.client
    }

    pub fn adapter_revision(&self) -> &str {
        &self.file.adapter_revision
    }

    pub fn registry_version(&self) -> &str {
        &self.file.registry_version
    }

    pub fn evidence(&self) -> &[EvidenceEntry] {
        &self.file.evidence
    }

    pub fn tested_client_versions(&self) -> &[String] {
        &self.file.tested_client_versions
    }

    pub fn documented_client_versions(&self) -> &[String] {
        &self.file.documented_client_versions
    }

    pub fn pinned_commit(&self) -> Option<&str> {
        self.file.pinned_commit.as_deref()
    }

    /// How upstream combines multiple managed sources (`first-wins` by default). `None` when the
    /// registry does not declare it; ConfigLens only claims single-source managed resolution.
    pub fn managed_sources_behavior(&self) -> Option<&str> {
        self.file.managed_sources_behavior.as_deref()
    }

    pub fn conformance_status(&self) -> &str {
        &self.file.conformance_status
    }

    pub fn rules(&self) -> &[RuleEntry] {
        &self.file.rules
    }

    pub fn env_pairs(&self) -> &[EnvPairEntry] {
        &self.file.env_pairs
    }

    pub fn sensitive_field_patterns(&self) -> &[String] {
        &self.file.sensitive_field_patterns
    }

    /// Layers ordered lowest precedence first.
    pub fn layers(&self) -> Vec<&LayerEntry> {
        let mut layers: Vec<&LayerEntry> = self.file.layers.iter().collect();
        layers.sort_by_key(|l| l.rank);
        layers
    }

    pub fn layer(&self, id: &str) -> Option<&LayerEntry> {
        self.file.layers.iter().find(|l| l.id == id)
    }

    /// Layers that can never be fully observed, with the reason to report.
    pub fn coverage_gaps(&self) -> Vec<(&str, String)> {
        self.layers()
            .into_iter()
            .filter_map(|l| l.gap_reason().map(|r| (l.id.as_str(), r)))
            .collect()
    }

    /// The most specific rule for `pointer`, or the client's inventory-only default.
    pub fn rule_for(&self, pointer: &str) -> RuleMatch {
        let mut best: Option<&RuleEntry> = None;
        let mut best_key = (0usize, 0usize, 0usize);
        for rule in &self.file.rules {
            if !matches(&rule.pointer, pointer) {
                continue;
            }
            let (literals, exact) = specificity(&rule.pointer);
            let key = (literals, exact, rule.pointer.len());
            if best.is_none() || key > best_key {
                best = Some(rule);
                best_key = key;
            }
        }
        match best {
            Some(rule) => RuleMatch {
                rule_id: rule.rule_id.clone(),
                pattern: rule.pointer.clone(),
                operator: Operator::parse(&rule.operator),
                support: SupportLevel::parse(&rule.support),
                evidence: rule.evidence.clone(),
                interpolation: rule
                    .interpolation
                    .clone()
                    .unwrap_or_else(|| "none".to_string()),
                authorization: rule.authorization.clone(),
                trust_gated: rule.trust_gated.unwrap_or(false),
                scope_excluded_values: rule.scope_excluded_values.clone().unwrap_or_default(),
                scope_excluded_from: rule.scope_excluded_from.clone().unwrap_or_default(),
                tests: rule.tests.clone().unwrap_or_default(),
            },
            None => RuleMatch {
                rule_id: format!("{}.unknown.inventory-only", self.client()),
                pattern: String::new(),
                operator: Operator::InventoryOnly,
                support: SupportLevel::InventoryOnly,
                evidence: Vec::new(),
                interpolation: "none".to_string(),
                authorization: None,
                trust_gated: false,
                scope_excluded_values: Vec::new(),
                scope_excluded_from: Vec::new(),
                tests: Vec::new(),
            },
        }
    }

    /// Build the redaction allowlist this registry advertises.
    pub fn redaction_policy(&self) -> RedactionPolicy {
        let mut policy = RedactionPolicy {
            sensitive_field_patterns: self.file.sensitive_field_patterns.clone(),
            ..RedactionPolicy::default()
        };
        for rule in &self.file.rules {
            let operator = Operator::parse(&rule.operator);
            if let Some(why) = &rule.redact_subtree {
                policy
                    .redact_subtrees
                    .push((rule.pointer.clone(), why.clone()));
            }
            match operator {
                Operator::InventoryOnly => {
                    policy.inventory_only_pointers.push(rule.pointer.clone())
                }
                Operator::ConcatDedupe => policy.tool_rule_list_pointers.push(rule.pointer.clone()),
                _ => {}
            }
            if let Some(values) = &rule.public_enums {
                policy
                    .public_enums
                    .push((rule.pointer.clone(), values.clone()));
            }
            if rule.model_name == Some(true) {
                policy.model_name_pointers.push(rule.pointer.clone());
            }
            if rule.public_strings == Some(true) {
                policy.public_string_pointers.push(rule.pointer.clone());
            }
            if rule.public_urls == Some(true) {
                policy.public_url_pointers.push(rule.pointer.clone());
            }
            if rule.public_host_list == Some(true) {
                policy.public_host_list_pointers.push(rule.pointer.clone());
            }
        }
        policy
    }

    /// Adapter metadata for `clients` and reports.
    pub fn metadata(&self) -> AdapterMetadata {
        AdapterMetadata {
            id: self.file.client.clone(),
            revision: self.file.adapter_revision.clone(),
            tested_versions: self.file.tested_client_versions.clone(),
            documented_versions: self.file.documented_client_versions.clone(),
            pinned_commit: self.file.pinned_commit.clone(),
            conformance_status: self.file.conformance_status.clone(),
            evidence_count: self.file.evidence.len(),
        }
    }
}

/// What `clients` reports and what the compatibility section of every report carries.
#[derive(Debug, Clone, PartialEq)]
pub struct AdapterMetadata {
    pub id: String,
    pub revision: String,
    pub tested_versions: Vec<String>,
    pub documented_versions: Vec<String>,
    pub pinned_commit: Option<String>,
    pub conformance_status: String,
    pub evidence_count: usize,
}

/// A source the adapter wants to inspect.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub layer_id: String,
    pub class: SourceClass,
    pub path: Option<PathBuf>,
    /// Raw inline content for `$OPENCODE_CONFIG_CONTENT` style sources.
    pub inline: Option<String>,
    pub note: Option<String>,
}

/// What discovery found, including layers it could not observe at all.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DiscoveryPlan {
    pub candidates: Vec<Candidate>,
    pub expected_unobservable: Vec<(String, String)>,
}

/// Interface every client adapter implements (spec §6).
pub trait ClientAdapter {
    fn client(&self) -> Client;
    fn registry(&self) -> &Registry;
    fn metadata(&self) -> AdapterMetadata {
        self.registry().metadata()
    }
    /// Discover adapter-known sources only. No recursive repository crawl.
    fn discover(
        &self,
        ctx: &ResolutionContext,
        access: &dyn FileAccess,
        paths: &PathPolicy,
    ) -> DiscoveryPlan;
    /// Parser dialect for a layer: Claude Code settings are strict JSON, OpenCode accepts JSONC.
    fn syntax_for(&self, layer_id: &str) -> Syntax;
    /// Whether a source applies in this context.
    fn applicability(&self, layer_id: &str, ctx: &ResolutionContext) -> Applicability;
    /// Whether workspace trust is a concept for this client at all. Claude Code gates project
    /// contributions on trust; OpenCode has no upstream trust concept (audit finding O4), so its
    /// adapter never reports a trust gap and its project config is always applicable.
    fn trust_applicable(&self) -> bool {
        true
    }
}

/// Construct the adapter for a client.
pub fn adapter_for(client: Client) -> Box<dyn ClientAdapter> {
    match client {
        Client::OpenCode => Box::new(opencode::OpenCodeAdapter::new()),
        Client::Claude => Box::new(claude::ClaudeAdapter::new()),
    }
}

/// Default applicability helper shared by adapters.
///
/// A project layer is never *fully* dropped by an untrusted workspace: upstream only gates a fixed
/// set of keys (`permissions.allow`, `additionalDirectories`, `extraKnownMarketplaces`, most `env`)
/// on trust, while `deny`/`ask` apply right away. So an untrusted/unknown workspace yields a
/// `Conditional` layer; the per-key trust gate is applied during reduction (`resolve`), where the
/// rule's `trust_gated` flag and the source class decide whether a *contribution* is ignored.
pub(crate) fn default_applicability(class: SourceClass, ctx: &ResolutionContext) -> Applicability {
    match class {
        SourceClass::ProjectShared | SourceClass::ProjectLocal => match ctx.trust {
            Trust::Trusted => Applicability::Applicable,
            Trust::Untrusted => Applicability::Conditional("workspace_untrusted".to_string()),
            Trust::Unknown => Applicability::Conditional("trust_unknown".to_string()),
        },
        _ => Applicability::Applicable,
    }
}

/// The managed system directories for a client, per platform (verified against the docs).
pub(crate) fn managed_dirs(
    client: Client,
    os: Os,
    program_data: Option<&std::path::Path>,
) -> Vec<PathBuf> {
    match (client, os) {
        (Client::Claude, Os::MacOs) => {
            vec![PathBuf::from("/Library/Application Support/ClaudeCode")]
        }
        (Client::Claude, Os::Linux) | (Client::Claude, Os::Wsl) => {
            vec![PathBuf::from("/etc/claude-code")]
        }
        (Client::Claude, Os::Windows) => {
            vec![PathBuf::from(r"C:\Program Files\ClaudeCode")]
        }
        (Client::OpenCode, Os::MacOs) => {
            vec![PathBuf::from("/Library/Application Support/opencode")]
        }
        (Client::OpenCode, Os::Linux) | (Client::OpenCode, Os::Wsl) => {
            vec![PathBuf::from("/etc/opencode")]
        }
        (Client::OpenCode, Os::Windows) => program_data
            .map(|p| vec![p.join("opencode")])
            .unwrap_or_else(|| vec![PathBuf::from(r"C:\ProgramData\opencode")]),
    }
}

/// Find the project root by walking up to the nearest `.git` entry (directory or worktree file),
/// without invoking Git or reading hooks (spec §6).
pub(crate) fn find_project_root(cwd: &std::path::Path, max_depth: usize) -> Option<PathBuf> {
    let mut current = Some(cwd.to_path_buf());
    let mut steps = 0usize;
    while let Some(dir) = current {
        if steps > max_depth {
            return None;
        }
        if dir.join(".git").exists() {
            return Some(dir);
        }
        current = dir.parent().map(|p| p.to_path_buf());
        steps += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opencode() -> Registry {
        Registry::load(Client::OpenCode).expect("bundled registry must load")
    }

    fn claude() -> Registry {
        Registry::load(Client::Claude).expect("bundled registry must load")
    }

    #[test]
    fn both_bundled_registries_load_and_are_pinned_to_a_revision() {
        for registry in [opencode(), claude()] {
            assert_eq!(registry.registry_version(), "1.0");
            assert_eq!(registry.adapter_revision(), "0.1.0");
            assert!(!registry.tested_client_versions().is_empty());
            assert!(registry.evidence().iter().all(|e| !e.url.is_empty()));
        }
        // Claude Code is closed source: no loader SHA exists.
        assert_eq!(claude().pinned_commit(), None);
        // OpenCode is pinned to the v1 line that the registry documents (audit O1).
        assert_eq!(
            opencode().pinned_commit(),
            Some("53d1eabb61e21162157817bf677da0a4ad3332e3")
        );
        assert_eq!(opencode().client(), "opencode");
        assert_eq!(claude().client(), "claude");
    }

    #[test]
    fn opencode_registry_matches_its_pinned_v1_line() {
        // O1/O2/O3: the tested version and the pinned SHA must belong to the same product line.
        let oc = opencode();
        assert_eq!(oc.tested_client_versions(), ["1.18.35"]);
        assert_eq!(oc.documented_client_versions(), ["1.18.35"]);
        assert_eq!(
            oc.pinned_commit(),
            Some("53d1eabb61e21162157817bf677da0a4ad3332e3")
        );
        let note = oc
            .evidence()
            .iter()
            .find(|e| e.id == "OC-REPO")
            .and_then(|e| e.note.clone())
            .unwrap_or_default();
        assert!(note.contains("53d1eabb"), "v1 tag must be cited: {note}");
        assert!(
            !note.contains("no replacement loader"),
            "the stale 'no loader' wording must not survive: {note}"
        );
    }

    #[test]
    fn claude_hkcu_is_the_lowest_managed_layer() {
        // C1: upstream ranks HKCU as the bottom managed fallback, so its rank must sit *below*
        // cc.managed.file. A regression to the inverted rank makes this fail.
        let cc = claude();
        let hkcu = cc.layer("cc.managed.hkcu").expect("hkcu layer");
        let file = cc.layer("cc.managed.file").expect("managed file layer");
        assert!(
            hkcu.rank < file.rank,
            "HKCU ({}) must rank below the managed file tier ({})",
            hkcu.rank,
            file.rank
        );
        let managed: Vec<(&str, u32)> = cc
            .layers()
            .into_iter()
            .filter(|l| l.id.starts_with("cc.managed."))
            .map(|l| (l.id.as_str(), l.rank))
            .collect();
        assert_eq!(
            managed,
            vec![
                ("cc.managed.hkcu", 50),
                ("cc.managed.file", 60),
                ("cc.managed.mdm", 70),
                ("cc.managed.server", 80),
            ],
            "server > MDM > file > HKCU, lowest precedence first"
        );
    }

    #[test]
    fn claude_project_shared_location_names_the_primary_working_directory() {
        // C2: the shared file is read from the session's primary working directory, not the repo
        // root (registry.md already said so; registry.json must agree).
        let cc = claude();
        let shared = cc.layer("cc.project_shared").expect("shared layer");
        assert_eq!(
            shared.location,
            "the session primary working directory/.claude/settings.json"
        );
    }

    #[test]
    fn claude_managed_sources_behavior_is_first_wins() {
        // C3: managed sources combine first-wins by default; `merge` is opt-in (v2.1.242+).
        // ConfigLens only claims single-source managed resolution.
        assert_eq!(claude().managed_sources_behavior(), Some("first-wins"));
        assert_eq!(opencode().managed_sources_behavior(), None);
    }

    #[test]
    fn claude_model_picker_is_inventory_only() {
        // C5: upstream ignores modelPicker in project/local, so ConfigLens must not claim an
        // ordered-replace merge for it. It is inventory-only, matching registry.md.
        let rule = claude().rule_for("/modelPicker");
        assert_eq!(rule.operator, Operator::InventoryOnly);
        assert_eq!(rule.support, SupportLevel::InventoryOnly);
        assert_eq!(rule.rule_id, "claude.unknown.inventory-only");
    }

    #[test]
    fn trust_is_a_claude_only_concept() {
        // O4: OpenCode has no workspace-trust concept; only the Claude adapter reports trust gaps.
        assert!(adapter_for(Client::Claude).trust_applicable());
        assert!(!adapter_for(Client::OpenCode).trust_applicable());
    }

    #[test]
    fn layers_are_ordered_lowest_precedence_first_and_are_unique() {
        let registry = opencode();
        let ranks: Vec<u32> = registry.layers().iter().map(|l| l.rank).collect();
        let mut sorted = ranks.clone();
        sorted.sort_unstable();
        assert_eq!(ranks, sorted);
        let ids: Vec<&str> = registry.layers().iter().map(|l| l.id.as_str()).collect();
        let mut unique = ids.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(ids.len(), unique.len(), "layer ids must be unique");

        // The documented OpenCode order.
        assert_eq!(
            ids,
            vec![
                "oc.remote.wellknown",
                "oc.global",
                "oc.custom_path",
                "oc.project",
                "oc.opencode_dirs",
                "oc.custom_dir",
                "oc.inline",
                "oc.managed_file",
                "oc.managed_macos_prefs",
            ]
        );
        // The documented Claude order, lowest first. HKCU is the lowest *managed* layer (C1).
        assert_eq!(
            claude()
                .layers()
                .iter()
                .map(|l| l.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "cc.user",
                "cc.project_shared",
                "cc.project_local",
                "cc.session_args",
                "cc.managed.hkcu",
                "cc.managed.file",
                "cc.managed.mdm",
                "cc.managed.server",
            ]
        );
    }

    #[test]
    fn unobservable_layers_are_always_reported_as_coverage_gaps() {
        let oc = opencode();
        let gaps = oc.coverage_gaps();
        assert!(gaps
            .iter()
            .any(|(id, why)| *id == "oc.remote.wellknown" && why == "remote_source_not_fetched"));
        assert!(gaps
            .iter()
            .any(|(id, why)| *id == "oc.opencode_dirs" && why == "not_a_settings_document"));
        assert!(gaps.iter().any(|(id, _)| *id == "oc.managed_macos_prefs"));

        let cc = claude();
        let gaps = cc.coverage_gaps();
        for layer in ["cc.managed.mdm", "cc.managed.server", "cc.managed.hkcu"] {
            assert!(gaps.iter().any(|(id, _)| *id == layer), "{layer} must gap");
        }
    }

    #[test]
    fn rule_lookup_prefers_the_most_specific_pattern() {
        let registry = opencode();
        assert_eq!(
            registry.rule_for("/model").rule_id,
            "opencode.scalar.layer-override"
        );
        assert_eq!(registry.rule_for("/model").operator, Operator::Replace);
        assert_eq!(
            registry.rule_for("/mcp/jira/enabled").rule_id,
            "opencode.object.recursive-merge"
        );
        assert_eq!(
            registry.rule_for("/mcp/jira/extra/enabled").rule_id,
            "opencode.unknown.inventory-only"
        );
        assert_eq!(
            registry.rule_for("/permission/bash/rm -rf *").rule_id,
            "opencode.object.recursive-merge"
        );
    }

    #[test]
    fn unknown_pointers_get_inventory_only_and_never_an_operator() {
        let registry = claude();
        let rule = registry.rule_for("/totally/unknown/key");
        assert_eq!(rule.operator, Operator::InventoryOnly);
        assert_eq!(rule.support, SupportLevel::InventoryOnly);
        assert_eq!(rule.rule_id, "claude.unknown.inventory-only");
        assert!(rule.evidence.is_empty());
    }

    #[test]
    fn advertised_support_levels_never_exceed_the_conformance_status() {
        for registry in [opencode(), claude()] {
            assert_eq!(registry.conformance_status(), "fixture_verified_max");
            for rule in registry.rules() {
                let level = SupportLevel::parse(&rule.support);
                assert!(
                    level <= SupportLevel::FixtureVerified,
                    "rule {} claims {:?} without an upstream conformance run",
                    rule.rule_id,
                    level
                );
                if level >= SupportLevel::Documented {
                    assert!(
                        !rule.evidence.is_empty(),
                        "rule {} must cite evidence",
                        rule.rule_id
                    );
                }
            }
        }
    }

    #[test]
    fn every_advertised_rule_names_its_tests() {
        for registry in [opencode(), claude()] {
            for rule in registry.rules() {
                if rule.operator == "inventory-only" {
                    continue;
                }
                let tests = rule.tests.clone().unwrap_or_default();
                assert!(
                    !tests.is_empty(),
                    "rule {} advertises an operator but lists no test ids",
                    rule.rule_id
                );
            }
        }
    }

    #[test]
    fn redaction_policy_is_derived_from_the_registry() {
        let cc = claude().redaction_policy();
        assert!(cc
            .tool_rule_list_pointers
            .contains(&"/permissions/allow".to_string()));
        assert!(cc
            .redact_subtrees
            .iter()
            .any(|(p, why)| p == "/env/*" && why == "environment_value"));
        assert!(cc
            .redact_subtrees
            .iter()
            .any(|(p, _)| p == "/mcpServers/**"));
        assert!(cc.model_name_pointers.contains(&"/model".to_string()));
        assert!(cc
            .inventory_only_pointers
            .contains(&"/permissions/additionalDirectories".to_string()));
        assert!(cc.sensitive_field_patterns.contains(&"token".to_string()));

        let oc = opencode().redaction_policy();
        assert!(oc
            .public_enums
            .iter()
            .any(|(p, values)| p == "/share" && values.contains(&"disabled".to_string())));
        assert!(oc
            .redact_subtrees
            .iter()
            .any(|(p, why)| p == "/mcp/*/environment" && why == "mcp_environment_map"));
    }

    #[test]
    fn client_parsing_and_the_unsupported_list_are_explicit() {
        assert_eq!(Client::parse("opencode"), Some(Client::OpenCode));
        assert_eq!(Client::parse("claude"), Some(Client::Claude));
        assert_eq!(Client::parse("codex"), None);
        assert_eq!(Client::all().len(), 2);
        let unsupported = Client::unsupported();
        assert!(unsupported.iter().any(|(id, _)| *id == "codex"));
        assert!(unsupported.iter().any(|(id, _)| *id == "gemini"));
        assert!(unsupported.iter().all(|(_, why)| !why.is_empty()));
    }

    #[test]
    fn support_levels_are_ordered_by_strength() {
        assert!(SupportLevel::Unsupported < SupportLevel::InventoryOnly);
        assert!(SupportLevel::InventoryOnly < SupportLevel::Documented);
        assert!(SupportLevel::Documented < SupportLevel::FixtureVerified);
        assert!(SupportLevel::FixtureVerified < SupportLevel::UpstreamConformanceVerified);
    }

    #[test]
    fn managed_directories_follow_the_documented_platform_paths() {
        assert_eq!(
            managed_dirs(Client::Claude, Os::Linux, None),
            vec![PathBuf::from("/etc/claude-code")]
        );
        assert_eq!(
            managed_dirs(Client::Claude, Os::Wsl, None),
            vec![PathBuf::from("/etc/claude-code")]
        );
        assert_eq!(
            managed_dirs(Client::Claude, Os::MacOs, None),
            vec![PathBuf::from("/Library/Application Support/ClaudeCode")]
        );
        assert_eq!(
            managed_dirs(Client::Claude, Os::Windows, None),
            vec![PathBuf::from(r"C:\Program Files\ClaudeCode")]
        );
        assert_eq!(
            managed_dirs(Client::OpenCode, Os::Linux, None),
            vec![PathBuf::from("/etc/opencode")]
        );
        assert_eq!(
            managed_dirs(
                Client::OpenCode,
                Os::Windows,
                Some(std::path::Path::new(r"C:\ProgramData"))
            ),
            vec![PathBuf::from(r"C:\ProgramData").join("opencode")]
        );
    }

    #[test]
    fn project_root_discovery_accepts_a_git_directory_and_a_worktree_file() {
        let dir = tempfile::tempdir().expect("temp");
        let root = dir.path();
        let nested = root.join("a/b/c");
        std::fs::create_dir_all(&nested).expect("mkdir");
        std::fs::create_dir_all(root.join(".git")).expect("mkdir .git");
        assert_eq!(find_project_root(&nested, 64), Some(root.to_path_buf()));

        std::fs::remove_dir_all(root.join(".git")).expect("rmdir");
        std::fs::write(root.join(".git"), "gitdir: /elsewhere\n").expect("write");
        assert_eq!(find_project_root(&nested, 64), Some(root.to_path_buf()));
    }

    #[test]
    fn project_root_discovery_stops_at_the_depth_limit() {
        let dir = tempfile::tempdir().expect("temp");
        let root = dir.path();
        let nested = root.join("a/b/c/d/e");
        std::fs::create_dir_all(&nested).expect("mkdir");
        std::fs::create_dir_all(root.join(".git")).expect("mkdir .git");
        assert_eq!(find_project_root(&nested, 2), None);
    }

    #[test]
    fn source_classes_and_applicability_are_explicit() {
        assert_eq!(SourceClass::ManagedFile.name(), "managed_file");
        assert!(Applicability::Applicable.is_applicable());
        assert_eq!(
            Applicability::Conditional("trust_unknown".to_string()).note(),
            Some("trust_unknown")
        );
        assert_eq!(Operator::parse("concat-dedupe"), Operator::ConcatDedupe);
        assert_eq!(Operator::parse("something-new"), Operator::InventoryOnly);
    }
}
