//! Coverage: what ConfigLens could not observe, and why no "effective" claim is made.

use serde::Serialize;

/// Why a resolution cannot be promoted from *observed* to *effective*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GapKind {
    /// A layer exists upstream but ConfigLens cannot read it (remote, MDM, registry, plist).
    ExpectedUnobservable,
    /// A managed source is absent, which never proves enterprise policy is absent.
    ManagedStateUnknown,
    /// Workspace trust is unknown, so trust-gated contributions stay conditional.
    TrustUnknown,
    /// The supplied session state (arguments, environment) is simulated evidence.
    SessionStateSimulated,
    /// The adapter's own rules carry an unresolved placement or dialect question.
    RulePlacementUnverified,
    /// The claimed client version is not in the adapter's tested/documented set.
    VersionUntested,
    /// The project boundary could not be identified without guessing.
    ProjectBoundaryUncertain,
}

impl GapKind {
    pub fn name(self) -> &'static str {
        match self {
            GapKind::ExpectedUnobservable => "expected_unobservable",
            GapKind::ManagedStateUnknown => "managed_state_unknown",
            GapKind::TrustUnknown => "trust_unknown",
            GapKind::SessionStateSimulated => "session_state_simulated",
            GapKind::RulePlacementUnverified => "rule_placement_unverified",
            GapKind::VersionUntested => "version_untested",
            GapKind::ProjectBoundaryUncertain => "project_boundary_uncertain",
        }
    }
}

/// One concrete reason a claim was withheld.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CoverageGap {
    pub kind: GapKind,
    /// Layer id when the gap belongs to a layer, otherwise the pointer or `run`.
    pub subject: String,
    pub reason: String,
    /// Precedence rank of the layer concerned, when it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rank: Option<u32>,
    /// Whether this gap prevents an *effective* claim. Some gaps (for example "supplied session
    /// state was used") are disclosed without blocking, because the report already records that the
    /// context is simulated.
    pub blocks_completeness: bool,
}

impl CoverageGap {
    pub fn layer(kind: GapKind, layer_id: &str, reason: &str, rank: u32) -> Self {
        Self {
            kind,
            subject: layer_id.to_string(),
            reason: reason.to_string(),
            rank: Some(rank),
            blocks_completeness: true,
        }
    }

    pub fn run(kind: GapKind, subject: &str, reason: &str) -> Self {
        Self {
            kind,
            subject: subject.to_string(),
            reason: reason.to_string(),
            rank: None,
            blocks_completeness: true,
        }
    }

    /// Disclosed, but not a reason to withhold an effective value.
    pub fn non_blocking(mut self) -> Self {
        self.blocks_completeness = false;
        self
    }

    /// One-line description used by text reports.
    pub fn describe(&self) -> String {
        format!("{} — {} ({})", self.subject, self.reason, self.kind.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_gaps_carry_a_rank_and_run_gaps_do_not() {
        let layer = CoverageGap::layer(
            GapKind::ExpectedUnobservable,
            "cc.managed.mdm",
            "not_read",
            70,
        );
        assert_eq!(layer.rank, Some(70));
        assert_eq!(layer.kind.name(), "expected_unobservable");
        assert_eq!(
            layer.describe(),
            "cc.managed.mdm — not_read (expected_unobservable)"
        );

        let run = CoverageGap::run(GapKind::TrustUnknown, "run", "project_trust_unknown");
        assert_eq!(run.rank, None);
        assert_eq!(run.subject, "run");
    }

    #[test]
    fn gap_kinds_sort_deterministically() {
        let mut kinds = [
            GapKind::VersionUntested,
            GapKind::ExpectedUnobservable,
            GapKind::TrustUnknown,
        ];
        kinds.sort();
        assert_eq!(kinds[0], GapKind::ExpectedUnobservable);
    }
}
