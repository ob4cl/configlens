//! Definitions and the ordered resolution event trace (spec §5).

use serde::Serialize;

use crate::adapters::SupportLevel;
use crate::model::definition::{SensitiveValue, Span, ValueKind};
use crate::model::source::SourceId;
use crate::security::PublicValue;

/// Stable, content-free definition identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct DefinitionId(pub u32);

/// One observed value for one pointer in one source.
#[derive(Debug, Clone, PartialEq)]
pub struct Definition {
    pub id: DefinitionId,
    pub source: SourceId,
    pub pointer: String,
    pub value: SensitiveValue,
    pub span: Option<Span>,
    /// Line/column of the value, computed from the source text (safe to report).
    pub line: Option<u32>,
    pub column: Option<u32>,
}

impl Definition {
    pub fn kind(&self) -> ValueKind {
        self.value.kind()
    }
}

/// Kind of reduction event. Every contribution survives in the trace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    /// This definition supplies the value that wins (or that is observed to win).
    Selected,
    /// An earlier definition was replaced by a later one.
    Replaced,
    /// A definition contributed part of a combined value (list concatenation, object merge).
    Contributed,
    /// A definition was not usable, with a reason code.
    Ignored,
    /// A policy source constrains this key (e.g. a managed allowlist).
    Constrained,
    /// A verified interpolation transformed a literal into a derived value.
    Transformed,
    /// Nothing could be resolved for this pointer.
    Unresolved,
}

impl EventKind {
    pub fn name(self) -> &'static str {
        match self {
            EventKind::Selected => "selected",
            EventKind::Replaced => "replaced",
            EventKind::Contributed => "contributed",
            EventKind::Ignored => "ignored",
            EventKind::Constrained => "constrained",
            EventKind::Transformed => "transformed",
            EventKind::Unresolved => "unresolved",
        }
    }
}

/// One entry of the ordered event trace. Carries redacted values only.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Event {
    pub kind: EventKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub definition_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_id: Option<u32>,
    /// Display location of the contributing source (`$HOME/.claude/settings.json:2:12`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<PublicValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason_code: Option<String>,
}

impl Event {
    pub fn new(kind: EventKind) -> Self {
        Self {
            kind,
            definition_id: None,
            source_id: None,
            location: None,
            value: None,
            rule_id: None,
            reason_code: None,
        }
    }

    pub fn with_definition(mut self, id: DefinitionId, source: SourceId) -> Self {
        self.definition_id = Some(id.0);
        self.source_id = Some(source.0);
        self
    }

    pub fn with_location(mut self, location: String) -> Self {
        self.location = Some(location);
        self
    }

    pub fn with_value(mut self, value: PublicValue) -> Self {
        self.value = Some(value);
        self
    }

    pub fn with_rule(mut self, rule_id: &str) -> Self {
        self.rule_id = Some(rule_id.to_string());
        self
    }

    pub fn with_reason(mut self, reason: &str) -> Self {
        self.reason_code = Some(reason.to_string());
        self
    }
}

/// Where a resolution's confidence comes from: the evidence class, never a probability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    /// Adapter behaviour verified by fixtures against documentation.
    FixtureVerified,
    /// Documented upstream behaviour, not fixture-tested here.
    Documented,
    /// Present in the inventory only; no merge semantics claimed.
    InventoryOnly,
    /// The version or the semantics could not be verified at all.
    Unverified,
}

impl Confidence {
    pub fn from_support(support: SupportLevel) -> Self {
        match support {
            SupportLevel::UpstreamConformanceVerified | SupportLevel::FixtureVerified => {
                Confidence::FixtureVerified
            }
            SupportLevel::Documented => Confidence::Documented,
            SupportLevel::InventoryOnly => Confidence::InventoryOnly,
            SupportLevel::Unsupported => Confidence::Unverified,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Confidence::FixtureVerified => "evidence_class:fixture_verified",
            Confidence::Documented => "evidence_class:documented",
            Confidence::InventoryOnly => "evidence_class:inventory_only",
            Confidence::Unverified => "evidence_class:unverified",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn events_default_to_empty_and_chain() {
        let event = Event::new(EventKind::Selected)
            .with_definition(DefinitionId(7), SourceId(2))
            .with_location("$HOME/.claude/settings.json:2:12".to_string())
            .with_value(PublicValue::revealed(json!("model-b")))
            .with_rule("claude.scalar.layer-override")
            .with_reason("observed_winner");
        assert_eq!(event.kind, EventKind::Selected);
        assert_eq!(event.definition_id, Some(7));
        assert_eq!(event.source_id, Some(2));
        assert_eq!(
            event.rule_id.as_deref(),
            Some("claude.scalar.layer-override")
        );
        assert_eq!(event.reason_code.as_deref(), Some("observed_winner"));
        assert_eq!(event.kind.name(), "selected");
    }

    #[test]
    fn confidence_is_an_evidence_class_not_a_probability() {
        assert_eq!(
            Confidence::from_support(SupportLevel::FixtureVerified),
            Confidence::FixtureVerified
        );
        assert_eq!(
            Confidence::from_support(SupportLevel::Documented),
            Confidence::Documented
        );
        assert_eq!(
            Confidence::from_support(SupportLevel::Unsupported),
            Confidence::Unverified
        );
        assert!(Confidence::Documented.name().starts_with("evidence_class:"));
    }

    #[test]
    fn event_serialization_is_snake_case_and_drops_absent_fields() {
        let event = Event::new(EventKind::Ignored).with_reason("scope_excluded_value");
        let text = serde_json::to_string(&event).expect("serialize");
        assert_eq!(
            text,
            r#"{"kind":"ignored","reason_code":"scope_excluded_value"}"#
        );
    }
}
