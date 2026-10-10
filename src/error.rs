//! Error boundary and exit-code arbitration (spec §4 "Exit codes").
//!
//! Priority when several conditions apply: `2 → 4 → 3 → 1 → 0`.
//!
//! | code | meaning |
//! |---|---|
//! | 0 | successful report, ordinary non-fatal findings permitted |
//! | 1 | doctor findings meet `--fail-on warning|error` (default error) |
//! | 2 | usage or incompatible input/report |
//! | 3 | requested key/client semantics unsupported, or strict mode detects incomplete context |
//! | 4 | required file inaccessible, malformed or over resource limit |
//!
//! Messages are sanitized at construction: a raw value never reaches `Display`, `Debug` or JSON.

use serde::Serialize;

use crate::security::sanitize;

/// The class of a finished run, before arbitration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Outcome {
    Success,
    DoctorFindings,
    Unsupported,
    FileProblem,
    Usage,
}

impl Outcome {
    /// Exit code of this outcome in isolation.
    pub fn exit_code(self) -> i32 {
        match self {
            Outcome::Success => 0,
            Outcome::DoctorFindings => 1,
            Outcome::Usage => 2,
            Outcome::Unsupported => 3,
            Outcome::FileProblem => 4,
        }
    }

    /// Arbitration rank: lower wins when several outcomes apply (`2 → 4 → 3 → 1 → 0`).
    pub fn rank(self) -> u8 {
        match self {
            Outcome::Usage => 0,
            Outcome::FileProblem => 1,
            Outcome::Unsupported => 2,
            Outcome::DoctorFindings => 3,
            Outcome::Success => 4,
        }
    }
}

/// Arbitrate several outcomes into the single exit code the process must use.
pub fn arbitrate(outcomes: &[Outcome]) -> Outcome {
    outcomes
        .iter()
        .copied()
        .min_by_key(|o| o.rank())
        .unwrap_or(Outcome::Success)
}

/// A fatal condition. Construct through the helper functions so messages are always sanitized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError {
    code: &'static str,
    message: String,
    location: Option<String>,
    outcome: Outcome,
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for CliError {}

/// Structured error inside a JSON report envelope. Contains no raw configuration content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ErrorDto {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
}

impl CliError {
    fn new(code: &'static str, message: &str, location: Option<&str>, outcome: Outcome) -> Self {
        Self {
            code,
            message: sanitize(message),
            location: location.map(sanitize),
            outcome,
        }
    }

    /// Exit code 2 — usage or incompatible input/report.
    pub fn usage(message: &str) -> Self {
        Self::new("usage", message, None, Outcome::Usage)
    }

    /// Exit code 2 — input or report is structurally incompatible.
    pub fn incompatible_input(message: &str) -> Self {
        Self::new("incompatible_input", message, None, Outcome::Usage)
    }

    /// Exit code 3 — the requested key or client semantics are unsupported.
    pub fn unsupported(message: &str) -> Self {
        Self::new("unsupported", message, None, Outcome::Unsupported)
    }

    /// Exit code 3 — strict mode found incomplete context.
    pub fn strict_incomplete(message: &str) -> Self {
        Self::new("strict_incomplete", message, None, Outcome::Unsupported)
    }

    /// Exit code 4 — a required file is inaccessible, malformed or over a resource limit.
    pub fn file_problem(code: &'static str, location: &str) -> Self {
        Self::new(code, code, Some(location), Outcome::FileProblem)
    }

    /// Exit code 1 — doctor findings meet the configured threshold.
    pub fn doctor_findings(count: usize, threshold: &str) -> Self {
        Self::new(
            "doctor_findings",
            &format!("{count} finding(s) at or above {threshold}"),
            None,
            Outcome::DoctorFindings,
        )
    }

    /// Machine-readable code, e.g. `usage`, `malformed`, `not_a_regular_file`.
    pub fn code(&self) -> &str {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn location(&self) -> Option<&str> {
        self.location.as_deref()
    }

    pub fn outcome(&self) -> Outcome {
        self.outcome
    }

    pub fn exit_code(&self) -> i32 {
        self.outcome.exit_code()
    }

    pub fn to_dto(&self) -> ErrorDto {
        ErrorDto {
            code: self.code.to_string(),
            message: self.message.clone(),
            location: self.location.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_match_the_specification() {
        assert_eq!(Outcome::Success.exit_code(), 0);
        assert_eq!(Outcome::DoctorFindings.exit_code(), 1);
        assert_eq!(Outcome::Usage.exit_code(), 2);
        assert_eq!(Outcome::Unsupported.exit_code(), 3);
        assert_eq!(Outcome::FileProblem.exit_code(), 4);
    }

    #[test]
    fn arbitration_follows_the_documented_priority() {
        use Outcome::*;
        assert_eq!(arbitrate(&[]), Success);
        assert_eq!(arbitrate(&[Success, DoctorFindings]), DoctorFindings);
        assert_eq!(arbitrate(&[DoctorFindings, Unsupported]), Unsupported);
        assert_eq!(arbitrate(&[Unsupported, FileProblem]), FileProblem);
        assert_eq!(arbitrate(&[FileProblem, Usage]), Usage);
        assert_eq!(
            arbitrate(&[Success, DoctorFindings, Unsupported, FileProblem, Usage]),
            Usage
        );
    }

    #[test]
    fn error_constructors_map_to_the_right_exit_code() {
        assert_eq!(CliError::usage("bad flag").exit_code(), 2);
        assert_eq!(CliError::incompatible_input("schema 2.0").exit_code(), 2);
        assert_eq!(CliError::unsupported("/mcp/nope").exit_code(), 3);
        assert_eq!(
            CliError::strict_incomplete("version unknown").exit_code(),
            3
        );
        assert_eq!(
            CliError::file_problem("malformed", "$HOME/.claude/settings.json").exit_code(),
            4
        );
        assert_eq!(CliError::doctor_findings(2, "warning").exit_code(), 1);
    }

    #[test]
    fn error_messages_are_sanitized_at_construction() {
        let err = CliError::usage("bad\u{1b}]8;;https://evil.example\u{7}flag\nend");
        assert_eq!(err.message(), "badflag\\nend");
        assert!(!err.to_string().contains('\u{1b}'));
        assert!(!err.to_string().contains("evil.example"));
    }

    #[test]
    fn error_locations_are_sanitized_and_reported_as_dto() {
        let err = CliError::file_problem("malformed", "$HOME/.claude/se\u{202e}ttings.json");
        let dto = err.to_dto();
        assert_eq!(dto.code, "malformed");
        assert_eq!(dto.location.as_deref(), Some("$HOME/.claude/settings.json"));
        assert_eq!(dto.message, "malformed");
    }

    #[test]
    fn doctor_error_code_is_stable_for_json_envelopes() {
        let err = CliError::doctor_findings(3, "error");
        assert_eq!(err.code(), "doctor_findings");
        assert_eq!(err.message(), "3 finding(s) at or above error");
    }
}
