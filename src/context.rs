//! Explicit resolution context (spec §5 step 1) and the `--input-context` file format (spec §4).

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Deserialize;

use crate::error::CliError;
use crate::parse::Limits;
use crate::security::PathMode;

/// The operating environment. WSL is distinct from Linux: managed paths and Windows policy stores
/// behave differently (spec §6 "Paths and platforms").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Linux,
    MacOs,
    Windows,
    Wsl,
}

impl Os {
    pub fn name(self) -> &'static str {
        match self {
            Os::Linux => "linux",
            Os::MacOs => "macos",
            Os::Windows => "windows",
            Os::Wsl => "wsl",
        }
    }

    /// Detect from explicit hints so tests never depend on the host.
    pub fn detect(is_windows: bool, wsl_marker: bool) -> Self {
        if is_windows {
            Os::Windows
        } else if wsl_marker {
            Os::Wsl
        } else {
            Os::Linux
        }
    }
}

/// Workspace trust, tri-state: `Unknown` must never be resolved to a decision (spec §2 acceptance 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Trust {
    #[default]
    Unknown,
    Trusted,
    Untrusted,
}

impl Trust {
    pub fn name(self) -> &'static str {
        match self {
            Trust::Unknown => "unknown",
            Trust::Trusted => "trusted",
            Trust::Untrusted => "untrusted",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "trusted" => Some(Trust::Trusted),
            "untrusted" => Some(Trust::Untrusted),
            "unknown" | "" => Some(Trust::Unknown),
            _ => None,
        }
    }
}

/// A client version claim. ConfigLens never reads a version from a client binary.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum VersionContext {
    #[default]
    Unknown,
    Claimed(String),
}

impl VersionContext {
    pub fn as_str(&self) -> &str {
        match self {
            VersionContext::Unknown => "unknown",
            VersionContext::Claimed(v) => v,
        }
    }

    pub fn is_unknown(&self) -> bool {
        matches!(self, VersionContext::Unknown)
    }
}

/// A source supplied by the user through `--input-context`.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct SuppliedSource {
    pub role: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub producer: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
}

/// The `--input-context` document (spec §4 "Input context").
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct SuppliedContext {
    #[serde(default)]
    pub schema_version: Option<String>,
    #[serde(default)]
    pub client: Option<String>,
    #[serde(default)]
    pub client_version: Option<String>,
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
    #[serde(default)]
    pub client_arguments: Vec<String>,
    #[serde(default)]
    pub project_trust: Option<String>,
    #[serde(default)]
    pub sources: Vec<SuppliedSource>,
}

/// The fully resolved context every stage receives. Reducers never read the process environment.
#[derive(Debug, Clone)]
pub struct ResolutionContext {
    pub client: String,
    pub os: Os,
    pub cwd: PathBuf,
    pub project_root: Option<PathBuf>,
    pub home: PathBuf,
    pub xdg_config: PathBuf,
    pub program_data: Option<PathBuf>,
    pub approved_roots: Vec<PathBuf>,
    pub client_version: VersionContext,
    pub trust: Trust,
    /// Captured process environment, restricted to the adapter allowlist. `None` with `--no-env`.
    pub env: Option<BTreeMap<String, String>>,
    /// Environment supplied through the context file (overrides captured values, marked simulated).
    pub supplied_env: BTreeMap<String, String>,
    pub client_arguments: Vec<String>,
    pub supplied_sources: Vec<SuppliedSource>,
    pub strict: bool,
    pub simulated: bool,
    pub paths: PathMode,
    pub limits: Limits,
    pub total_bytes_limit: u64,
    pub max_sources: usize,
    pub fail_on: FailOn,
    pub color: bool,
}

/// `--fail-on` threshold for `doctor`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FailOn {
    Warning,
    #[default]
    Error,
}

impl FailOn {
    pub fn name(self) -> &'static str {
        match self {
            FailOn::Warning => "warning",
            FailOn::Error => "error",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "warning" => Some(FailOn::Warning),
            "error" => Some(FailOn::Error),
            _ => None,
        }
    }
}

impl ResolutionContext {
    /// A fully populated context with conservative defaults and the host OS detected.
    /// Every field stays public so callers (CLI, tests, fixtures) can tighten it.
    pub fn new(client: &str, cwd: &str, home: &str, xdg_config: &str) -> Self {
        let os = if cfg!(target_os = "windows") {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::MacOs
        } else if cfg!(target_os = "linux")
            && std::path::Path::new("/proc/sys/kernel/osrelease").exists()
            && std::fs::read_to_string("/proc/sys/kernel/osrelease")
                .map(|text| text.to_lowercase().contains("microsoft"))
                .unwrap_or(false)
        {
            Os::Wsl
        } else {
            Os::Linux
        };
        Self {
            client: client.to_string(),
            os,
            cwd: PathBuf::from(cwd),
            project_root: None,
            home: PathBuf::from(home),
            xdg_config: PathBuf::from(xdg_config),
            // Environment policy belongs to the caller, not this constructor.
            program_data: None,
            approved_roots: Vec::new(),
            client_version: VersionContext::Unknown,
            trust: Trust::Unknown,
            env: None,
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

    /// The environment value for a captured/supplied variable, if any (supplied wins).
    pub fn environment_value(&self, name: &str) -> Option<&str> {
        if let Some(v) = self.supplied_env.get(name) {
            return Some(v.as_str());
        }
        self.env
            .as_ref()
            .and_then(|env| env.get(name))
            .map(String::as_str)
    }

    /// Is the claimed version one the registry actually tested or documented?
    pub fn version_supported(&self, tested: &[String], documented: &[String]) -> bool {
        match &self.client_version {
            VersionContext::Unknown => false,
            VersionContext::Claimed(v) => tested.contains(v) || documented.contains(v),
        }
    }
}

/// Parse an `--input-context` document, rejecting incompatible or ambiguous input.
pub fn parse_supplied_context(
    text: &str,
    expected_client: &str,
    claimed_version: &VersionContext,
) -> Result<SuppliedContext, CliError> {
    // Serde type errors may include arbitrary input strings. Terminal sanitization
    // is not secret redaction, so diagnostics must never contain its raw message.
    let merged: SuppliedContext = serde_json::from_str(text)
        .map_err(|_| CliError::incompatible_input("context file is not valid context JSON"))?;

    if let Some(schema) = &merged.schema_version {
        if schema != "1.0" {
            return Err(CliError::incompatible_input(
                "unsupported context schema_version",
            ));
        }
    }
    if let Some(client) = &merged.client {
        if client != expected_client {
            return Err(CliError::incompatible_input(
                "context file client does not match the selected client",
            ));
        }
    }
    if let (Some(file_version), VersionContext::Claimed(flag)) =
        (&merged.client_version, claimed_version)
    {
        if file_version != flag {
            return Err(CliError::incompatible_input(
                "context file client_version does not match --client-version",
            ));
        }
    }
    if let Some(trust) = &merged.project_trust {
        if Trust::parse(trust).is_none() {
            return Err(CliError::incompatible_input(
                "context file project_trust must be trusted, untrusted or unknown",
            ));
        }
    }
    let mut seen: Vec<(String, Option<String>)> = Vec::new();
    for source in &merged.sources {
        let identity = (source.role.clone(), source.path.clone());
        if seen.contains(&identity) {
            return Err(CliError::incompatible_input(
                "context file contains a duplicate source identity",
            ));
        }
        seen.push(identity);
    }
    Ok(merged)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> ResolutionContext {
        ResolutionContext {
            client: "claude".into(),
            os: Os::Linux,
            cwd: PathBuf::from("/home/dev/work/project"),
            project_root: Some(PathBuf::from("/home/dev/work/project")),
            home: PathBuf::from("/home/dev"),
            xdg_config: PathBuf::from("/home/dev/.config"),
            program_data: None,
            approved_roots: Vec::new(),
            client_version: VersionContext::Claimed("2.1.296".into()),
            trust: Trust::Unknown,
            env: None,
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

    #[test]
    fn trust_parses_only_the_three_documented_values() {
        assert_eq!(Trust::parse("trusted"), Some(Trust::Trusted));
        assert_eq!(Trust::parse("untrusted"), Some(Trust::Untrusted));
        assert_eq!(Trust::parse("unknown"), Some(Trust::Unknown));
        assert_eq!(Trust::parse("maybe"), None);
    }

    #[test]
    fn version_context_is_a_claim_not_a_lookup() {
        let c = ctx();
        assert!(c.version_supported(&["2.1.296".into()], &[]));
        assert!(!c.version_supported(&["2.1.200".into()], &[]));
        let mut unknown = ctx();
        unknown.client_version = VersionContext::Unknown;
        assert!(!unknown.version_supported(&["2.1.296".into()], &[]));
        assert!(unknown.client_version.is_unknown());
    }

    #[test]
    fn supplied_environment_overrides_captured_environment() {
        let mut c = ctx();
        c.env = Some(BTreeMap::from([(
            "ANTHROPIC_MODEL".to_string(),
            "from-env".to_string(),
        )]));
        c.supplied_env =
            BTreeMap::from([("ANTHROPIC_MODEL".to_string(), "from-context".to_string())]);
        assert_eq!(c.environment_value("ANTHROPIC_MODEL"), Some("from-context"));
        assert_eq!(c.environment_value("MISSING"), None);
    }

    #[test]
    fn no_env_means_no_captured_values_at_all() {
        let mut c = ctx();
        c.env = None;
        assert_eq!(c.environment_value("ANTHROPIC_MODEL"), None);
    }

    #[test]
    fn os_detection_is_injectable_for_tests() {
        assert_eq!(Os::detect(false, false), Os::Linux);
        assert_eq!(Os::detect(false, true), Os::Wsl);
        assert_eq!(Os::detect(true, true), Os::Windows);
        assert_eq!(Os::Linux.name(), "linux");
    }

    #[test]
    fn parses_a_minimal_context_document() {
        let text = r#"{"schema_version":"1.0","client":"claude","client_version":"2.1.296",
            "environment":{"ANTHROPIC_MODEL":"m"},"client_arguments":["--settings","{\"model\":\"x\"}"],
            "project_trust":"untrusted","sources":[{"role":"managed","path":"/etc/claude-code/managed-settings.json"}]}"#;
        let parsed =
            parse_supplied_context(text, "claude", &VersionContext::Claimed("2.1.296".into()))
                .expect("valid");
        assert_eq!(parsed.project_trust.as_deref(), Some("untrusted"));
        assert_eq!(parsed.client_arguments.len(), 2);
        // argv semantics: the JSON blob stays one argument, it is not shell-split.
        assert_eq!(parsed.client_arguments[1], "{\"model\":\"x\"}");
        assert_eq!(parsed.sources.len(), 1);
    }

    #[test]
    fn rejects_client_and_version_mismatches() {
        let text = r#"{"client":"opencode","client_version":"9.9.9"}"#;
        let err =
            parse_supplied_context(text, "claude", &VersionContext::Claimed("2.1.296".into()))
                .unwrap_err();
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn rejects_duplicate_source_identity() {
        let text = r#"{"sources":[{"role":"managed","path":"/a"},{"role":"managed","path":"/a"}]}"#;
        let err = parse_supplied_context(text, "claude", &VersionContext::Unknown).unwrap_err();
        assert_eq!(err.code(), "incompatible_input");
    }

    #[test]
    fn rejects_unsupported_schema_version_and_bad_trust() {
        let bad_schema = r#"{"schema_version":"2.0"}"#;
        assert!(parse_supplied_context(bad_schema, "claude", &VersionContext::Unknown).is_err());
        let bad_trust = r#"{"project_trust":"probably"}"#;
        assert!(parse_supplied_context(bad_trust, "claude", &VersionContext::Unknown).is_err());
        let bad_json = "{not json";
        assert!(parse_supplied_context(bad_json, "claude", &VersionContext::Unknown).is_err());
    }

    #[test]
    fn allows_a_context_without_version_claims() {
        let text = r#"{"environment":{}}"#;
        assert!(parse_supplied_context(text, "opencode", &VersionContext::Unknown).is_ok());
    }
}
