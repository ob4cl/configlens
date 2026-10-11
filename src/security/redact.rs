//! Redaction projection: the only way a raw value becomes reportable.
//!
//! Defaults to *redact unless allowlisted* (spec §7). A value is public only when the pointer is
//! allowlisted by the adapter **and** the value shape passes that allowlist.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use crate::model::definition::ValueKind;
use crate::pattern::{matches, matches_self_or_ancestor};
use crate::security::sanitize_bounded;

/// Whether paths/keys are shown (`Local`) or pseudonymized (`Share`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PathMode {
    #[default]
    Local,
    Share,
}

/// A reportable value. This is the only value type that implements `Serialize`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PublicValue {
    /// Present only for allowlisted values.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    /// Present only for redacted values.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redacted: Option<bool>,
    /// Why the value is not public, or how it was allowlisted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Always present: the shape of the underlying value.
    pub kind: ValueKind,
}

impl PublicValue {
    pub fn revealed(value: Value) -> Self {
        let kind = ValueKind::of(&value);
        Self {
            value: Some(value),
            redacted: None,
            reason: None,
            kind,
        }
    }

    pub fn revealed_with_reason(value: Value, reason: &str) -> Self {
        let kind = ValueKind::of(&value);
        Self {
            value: Some(value),
            redacted: None,
            reason: Some(reason.to_string()),
            kind,
        }
    }

    pub fn redacted(reason: &str, kind: ValueKind) -> Self {
        Self {
            value: None,
            redacted: Some(true),
            reason: Some(reason.to_string()),
            kind,
        }
    }

    pub fn is_redacted(&self) -> bool {
        self.redacted.unwrap_or(false)
    }

    pub fn reason_str(&self) -> &str {
        self.reason.as_deref().unwrap_or("")
    }

    pub fn as_value(&self) -> Option<&Value> {
        self.value.as_ref()
    }
}

/// Per-adapter allowlist, loaded from `compatibility/<client>/registry.json`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RedactionPolicy {
    /// Pointers whose whole subtree is redacted, with the reason to report.
    pub redact_subtrees: Vec<(String, String)>,
    /// Pointer patterns whose string value is a known enum.
    pub public_enums: Vec<(String, Vec<String>)>,
    /// Pointer patterns whose string value is a model name.
    pub model_name_pointers: Vec<String>,
    /// Pointer patterns whose string value is public text.
    pub public_string_pointers: Vec<String>,
    /// Pointer patterns whose string value is a URL that gets userinfo/query/fragment stripping.
    pub public_url_pointers: Vec<String>,
    /// Pointer patterns that hold arrays of tool rules (shown locally, pseudonymized in share).
    pub tool_rule_list_pointers: Vec<String>,
    /// Pointer patterns that hold arrays of host names (shown locally, pseudonymized in share).
    pub public_host_list_pointers: Vec<String>,
    /// Field-name fragments that mark a value as sensitive.
    pub sensitive_field_patterns: Vec<String>,
    /// Pointers that hold inventory-only values: never revealed.
    pub inventory_only_pointers: Vec<String>,
}

/// Reasons emitted by the projection. Kept as constants so tests and docs cannot drift.
pub mod reason {
    pub const SENSITIVE_FIELD: &str = "sensitive_field";
    pub const MCP_ENVIRONMENT: &str = "mcp_environment_map";
    pub const ENV_VALUE: &str = "environment_value";
    pub const UNCLASSIFIED: &str = "unclassified_value";
    pub const INVENTORY_ONLY: &str = "inventory_only";
    pub const INTERPOLATION_SOURCE: &str = "interpolation_source";
    pub const TOOL_RULE_LOCAL_ONLY: &str = "tool_rule_share_mode";
    pub const PUBLIC_ENUM: &str = "public_enum";
    pub const PUBLIC_BOOL_OR_NUMBER: &str = "public_primitive";
    pub const PUBLIC_NULL: &str = "null_literal";
    pub const MODEL_NAME: &str = "verified_model_name";
    pub const PUBLIC_STRING: &str = "allowlisted_string";
    pub const PUBLIC_URL: &str = "allowlisted_url_sanitized";
    pub const PUBLIC_HOST: &str = "allowlisted_host_list";
    pub const PUBLIC_HOST_SHARE: &str = "host_list_share_mode";
    pub const TOOL_RULE: &str = "tool_rule";
}

/// Is `value` a plain host name (letters, digits, dot, hyphen)? Used for host allowlists.
pub fn is_host_shape(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 253
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

/// Normalize a pointer segment for sensitive-name comparison: lowercase, drop `-` and `_`.
pub fn normalize_field(name: &str) -> String {
    name.chars()
        .filter(|c| !matches!(c, '-' | '_'))
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Is the final segment of `pointer` a credential-looking field name?
pub fn is_sensitive_field(pointer: &str, patterns: &[String]) -> bool {
    let last = pointer.rsplit('/').next().unwrap_or(pointer);
    let norm = normalize_field(last);
    patterns.iter().any(|p| {
        let p = normalize_field(p);
        !p.is_empty() && (norm == p || norm.starts_with(&p) || norm.ends_with(&p))
    })
}

/// Does `value` look like a model identifier (no whitespace, bounded charset and length)?
///
/// Provider model identifiers are lower-case (`claude-sonnet-4-5`, `anthropic/claude-3-5-sonnet`,
/// `gpt-4o`). Upper-case letters are not part of any verified model-name shape, so a token like
/// `SEEDED-VALUE-9f3c1a` must not be promoted to a public "verified model name" merely because it
/// sits under `/model` (spec §7 "public values are allowlisted per adapter ... verified model
/// names"; §8 "seed values absent from ... stdout, stderr, snapshot, diff").
pub fn is_model_name_shape(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.chars().all(|c| {
            c.is_ascii_lowercase()
                || c.is_ascii_digit()
                || matches!(c, '.' | '-' | '_' | '/' | ':' | '@')
        })
}

/// Is `value` a safe public string (no interpolation, bounded length)?
pub fn is_plain_string(value: &str) -> bool {
    !value.is_empty()
        && value.chars().count() <= 256
        && !value.contains('{')
        && !value.contains('}')
}

/// Strip URL userinfo, query and fragment. Returns `None` when the value is not a URL.
pub fn sanitize_url(value: &str) -> Option<String> {
    let (scheme, rest) = value.split_once("://")?;
    if !matches!(scheme, "http" | "https" | "ws" | "wss") {
        return None;
    }
    let (authority, tail) = match rest.find(['/', '?', '#']) {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, ""),
    };
    let host = match authority.rsplit_once('@') {
        Some((_, host)) => host,
        None => authority,
    };
    let path = match tail.find(['?', '#']) {
        Some(idx) => &tail[..idx],
        None => tail,
    };
    let mut out = format!("{scheme}://{host}{path}");
    if tail.contains('?') || tail.contains('#') {
        out.push_str("?[redacted]");
    }
    Some(out)
}

/// The host (authority without userinfo) of an absolute `http`/`https`/`ws`/`wss` URL.
///
/// Used by the share-mode projection to collect the private hosts that must be pseudonymized
/// (spec §7 "In share mode pseudonymize private hosts").
pub fn url_host(value: &str) -> Option<String> {
    let (scheme, rest) = value.split_once("://")?;
    if !matches!(scheme, "http" | "https" | "ws" | "wss") {
        return None;
    }
    let authority = match rest.find(['/', '?', '#']) {
        Some(idx) => &rest[..idx],
        None => rest,
    };
    if authority.is_empty() {
        return None;
    }
    Some(match authority.rsplit_once('@') {
        Some((_, host)) => host.to_string(),
        None => authority.to_string(),
    })
}

/// Rewrite the host of a URL to its share-mode pseudonym.
///
/// The most specific public form of the URL (`sanitize_url`) is used as the base, so userinfo,
/// query and fragment are already gone and the replacement only touches the authority.
pub fn pseudonymize_url_host(value: &str, hosts: &Pseudonymizer) -> Option<String> {
    let host = url_host(value)?;
    let clean = sanitize_url(value)?;
    let replacement = hosts.host_or_identity(&host);
    Some(clean.replacen(&host, &replacement, 1))
}

/// Does the string contain a verified adapter interpolation directive?
pub fn has_interpolation(value: &str) -> bool {
    value.contains("{env:") || value.contains("{file:")
}

/// Does the string look like a Claude Code permission rule (`Tool(spec)`)?
pub fn is_tool_rule(value: &str) -> bool {
    if value.chars().any(char::is_control) {
        return false;
    }
    let Some(open) = value.find('(') else {
        return false;
    };
    if !value.ends_with(')') || open == 0 {
        return false;
    }
    let head = &value[..open];
    let mut chars = head.chars();
    let first_ok = chars
        .next()
        .map(|c| c.is_ascii_alphabetic() || c == '_')
        .unwrap_or(false);
    first_ok && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Convert one raw value at `pointer` into a reportable value.
///
/// Order of decisions: subtree redaction → inventory-only → sensitive field name → value-shape
/// allowlist. The allowlist is per adapter and is the *only* way a string or container becomes
/// public.
pub fn redact_value(
    pointer: &str,
    value: &Value,
    policy: &RedactionPolicy,
    mode: PathMode,
) -> PublicValue {
    let kind = ValueKind::of(value);

    for (pattern, why) in &policy.redact_subtrees {
        if matches_self_or_ancestor(pattern, pointer) {
            return PublicValue::redacted(why, kind);
        }
    }
    for pattern in &policy.inventory_only_pointers {
        if matches_self_or_ancestor(pattern, pointer) {
            return PublicValue::redacted(reason::INVENTORY_ONLY, kind);
        }
    }
    if is_sensitive_field(pointer, &policy.sensitive_field_patterns) {
        return PublicValue::redacted(reason::SENSITIVE_FIELD, kind);
    }

    match value {
        Value::Null => PublicValue::revealed_with_reason(Value::Null, reason::PUBLIC_NULL),
        Value::Bool(_) | Value::Number(_) => {
            PublicValue::revealed_with_reason(value.clone(), reason::PUBLIC_BOOL_OR_NUMBER)
        }
        Value::String(s) => redact_string(pointer, s, policy),
        Value::Array(items) => {
            if pattern_list_matches(&policy.tool_rule_list_pointers, pointer) {
                let projected: Vec<Value> = items
                    .iter()
                    .enumerate()
                    .map(|(idx, item)| match item.as_str() {
                        Some(rule) if is_tool_rule(rule) => match mode {
                            PathMode::Local => Value::String(sanitize_bounded(rule, 256)),
                            PathMode::Share => Value::String(format!("rule_{}", idx + 1)),
                        },
                        _ => serde_json::json!({
                            "redacted": true,
                            "reason": reason::UNCLASSIFIED,
                        }),
                    })
                    .collect();
                let why = match mode {
                    PathMode::Local => reason::TOOL_RULE,
                    PathMode::Share => reason::TOOL_RULE_LOCAL_ONLY,
                };
                PublicValue::revealed_with_reason(Value::Array(projected), why)
            } else if pattern_list_matches(&policy.model_name_pointers, pointer)
                && items
                    .iter()
                    .all(|item| item.as_str().map(is_model_name_shape).unwrap_or(false))
            {
                PublicValue::revealed_with_reason(value.clone(), reason::MODEL_NAME)
            } else if pattern_list_matches(&policy.public_host_list_pointers, pointer)
                && items
                    .iter()
                    .all(|item| item.as_str().map(is_host_shape).unwrap_or(false))
            {
                match mode {
                    PathMode::Local => PublicValue::revealed_with_reason(
                        Value::Array(
                            items
                                .iter()
                                .map(|item| {
                                    Value::String(sanitize_bounded(
                                        item.as_str().unwrap_or_default(),
                                        256,
                                    ))
                                })
                                .collect(),
                        ),
                        reason::PUBLIC_HOST,
                    ),
                    PathMode::Share => PublicValue::revealed_with_reason(
                        Value::Array(
                            (0..items.len())
                                .map(|index| Value::String(format!("host_{}", index + 1)))
                                .collect(),
                        ),
                        reason::PUBLIC_HOST_SHARE,
                    ),
                }
            } else {
                PublicValue::redacted(reason::UNCLASSIFIED, kind)
            }
        }
        Value::Object(_) => PublicValue::redacted(reason::UNCLASSIFIED, kind),
    }
}

fn pattern_list_matches(patterns: &[String], pointer: &str) -> bool {
    patterns.iter().any(|p| matches(p, pointer))
}

fn redact_string(pointer: &str, value: &str, policy: &RedactionPolicy) -> PublicValue {
    if let Some((_, allowed)) = policy
        .public_enums
        .iter()
        .find(|(pattern, _)| matches(pattern, pointer))
    {
        if allowed.iter().any(|a| a == value) {
            return PublicValue::revealed_with_reason(
                Value::String(sanitize_bounded(value, 256)),
                reason::PUBLIC_ENUM,
            );
        }
    }
    if pattern_list_matches(&policy.model_name_pointers, pointer) && is_model_name_shape(value) {
        return PublicValue::revealed_with_reason(
            Value::String(sanitize_bounded(value, 256)),
            reason::MODEL_NAME,
        );
    }
    if pattern_list_matches(&policy.public_url_pointers, pointer) {
        if let Some(clean) = sanitize_url(value) {
            return PublicValue::revealed_with_reason(
                Value::String(sanitize_bounded(&clean, 256)),
                reason::PUBLIC_URL,
            );
        }
    }
    if pattern_list_matches(&policy.public_string_pointers, pointer) && is_plain_string(value) {
        return PublicValue::revealed_with_reason(
            Value::String(sanitize_bounded(value, 256)),
            reason::PUBLIC_STRING,
        );
    }
    if has_interpolation(value) {
        return PublicValue::redacted(reason::INTERPOLATION_SOURCE, ValueKind::String);
    }
    PublicValue::redacted(reason::UNCLASSIFIED, ValueKind::String)
}

/// Deterministic pseudonyms for share mode: assigned from sorted order, never from content.
#[derive(Debug, Clone, Default)]
pub struct Pseudonymizer {
    fields: BTreeMap<String, String>,
    hosts: BTreeMap<String, String>,
}

impl Pseudonymizer {
    /// Build from every opaque name that must be pseudonymized, sorted and deduplicated.
    pub fn new(
        names: impl IntoIterator<Item = String>,
        hosts: impl IntoIterator<Item = String>,
    ) -> Self {
        let mut uniq: Vec<String> = names.into_iter().collect();
        uniq.sort();
        uniq.dedup();
        let fields = uniq
            .into_iter()
            .enumerate()
            .map(|(i, name)| (name, format!("field_{}", i + 1)))
            .collect();
        let mut uniq_hosts: Vec<String> = hosts.into_iter().collect();
        uniq_hosts.sort();
        uniq_hosts.dedup();
        let hosts = uniq_hosts
            .into_iter()
            .enumerate()
            .map(|(i, name)| (name, format!("host_{}", i + 1)))
            .collect();
        Self { fields, hosts }
    }

    pub fn field(&self, name: &str) -> String {
        self.fields
            .get(name)
            .cloned()
            .unwrap_or_else(|| "field_?".to_string())
    }

    pub fn host(&self, name: &str) -> String {
        self.hosts
            .get(name)
            .cloned()
            .unwrap_or_else(|| "host_?".to_string())
    }

    /// Pseudonym for a host that was collected during projection. Falls back to a stable literal
    /// `host_opaque` when the host was not part of the collected set, so no private host string can
    /// ever survive un-pseudonymized (spec §7).
    pub fn host_or_identity(&self, name: &str) -> String {
        self.hosts
            .get(name)
            .cloned()
            .unwrap_or_else(|| "host_opaque".to_string())
    }
}

/// Sanitize a string that is about to become public.
pub fn public_text(value: &str) -> String {
    sanitize_bounded(value, 256)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn policy() -> RedactionPolicy {
        RedactionPolicy {
            redact_subtrees: vec![
                (
                    "/mcp/*/environment".to_string(),
                    reason::MCP_ENVIRONMENT.to_string(),
                ),
                ("/env/**".to_string(), reason::ENV_VALUE.to_string()),
                (
                    "/mcpServers/**".to_string(),
                    reason::MCP_ENVIRONMENT.to_string(),
                ),
            ],
            public_enums: vec![
                (
                    "/share".to_string(),
                    vec!["manual".into(), "auto".into(), "disabled".into()],
                ),
                (
                    "/permissions/defaultMode".to_string(),
                    vec!["default".into(), "plan".into(), "acceptEdits".into()],
                ),
            ],
            model_name_pointers: vec!["/model".to_string(), "/small_model".to_string()],
            public_string_pointers: vec!["/theme".to_string(), "/shell".to_string()],
            public_url_pointers: vec!["/mcp/*/url".to_string()],
            tool_rule_list_pointers: vec![
                "/permissions/allow".to_string(),
                "/permissions/deny".to_string(),
                "/permissions/ask".to_string(),
            ],
            public_host_list_pointers: vec!["/sandbox/**".to_string()],
            sensitive_field_patterns: vec![
                "password".into(),
                "passwd".into(),
                "passphrase".into(),
                "secret".into(),
                "token".into(),
                "credential".into(),
                "credentials".into(),
                "authorization".into(),
                "auth".into(),
                "apikey".into(),
                "api_key".into(),
                "api-key".into(),
                "access_key".into(),
                "private_key".into(),
                "client_secret".into(),
                "cookie".into(),
                "bearer".into(),
                "session".into(),
                "key".into(),
            ],
            inventory_only_pointers: vec!["/provider/**".to_string()],
        }
    }

    #[test]
    fn sensitive_field_names_are_redacted() {
        let p = policy();
        for pointer in [
            "/password",
            "/apiKey",
            "/api_key",
            "/api-key",
            "/auth/token",
            "/provider/anthropic/options/apiKey",
            "/headers/authorization",
            "/client_secret",
            "/private_key",
            "/cookies/session",
            "/bearerToken",
        ] {
            let out = redact_value(pointer, &json!("SEEDED-SECRET-VALUE"), &p, PathMode::Local);
            assert!(out.is_redacted(), "{pointer} must be redacted");
            assert!(!serde_json::to_string(&out).unwrap().contains("SEEDED"));
        }
    }

    #[test]
    fn primitive_values_are_public() {
        let p = policy();
        assert_eq!(
            redact_value("/snapshot", &json!(true), &p, PathMode::Local),
            PublicValue::revealed_with_reason(json!(true), reason::PUBLIC_BOOL_OR_NUMBER)
        );
        assert_eq!(
            redact_value("/compaction/reserved", &json!(10000), &p, PathMode::Local).as_value(),
            Some(&json!(10000))
        );
        assert!(redact_value("/nothing", &json!(null), &p, PathMode::Local)
            .reason_str()
            .eq(reason::PUBLIC_NULL));
    }

    #[test]
    fn allowlisted_enums_are_public_and_other_values_are_not() {
        let p = policy();
        let ok = redact_value("/share", &json!("auto"), &p, PathMode::Local);
        assert_eq!(ok.as_value(), Some(&json!("auto")));
        assert_eq!(ok.reason_str(), reason::PUBLIC_ENUM);

        let bad = redact_value("/share", &json!("something-else"), &p, PathMode::Local);
        assert!(bad.is_redacted());
        assert_eq!(bad.reason_str(), reason::UNCLASSIFIED);
    }

    #[test]
    fn model_names_are_public_when_they_match_the_verified_shape() {
        let p = policy();
        let ok = redact_value(
            "/model",
            &json!("anthropic/claude-sonnet-4-5"),
            &p,
            PathMode::Local,
        );
        assert_eq!(ok.as_value(), Some(&json!("anthropic/claude-sonnet-4-5")));
        assert_eq!(ok.reason_str(), reason::MODEL_NAME);

        let bad = redact_value("/model", &json!("a model with spaces"), &p, PathMode::Local);
        assert!(bad.is_redacted());

        // Regression: an upper-case seeded token under /model is not a verified model name and
        // must be redacted, never published as `verified_model_name` (spec §7, §8).
        let seeded = redact_value("/model", &json!("SEEDED-VALUE-9f3c1a"), &p, PathMode::Local);
        assert!(
            seeded.is_redacted(),
            "an upper-case token is not a model name"
        );
        assert_eq!(seeded.reason_str(), reason::UNCLASSIFIED);
    }

    #[test]
    fn allowlisted_strings_are_public_but_interpolation_is_not() {
        let p = policy();
        assert_eq!(
            redact_value("/theme", &json!("tokyonight"), &p, PathMode::Local).as_value(),
            Some(&json!("tokyonight"))
        );
        let interp = redact_value("/theme", &json!("{env:THEME}"), &p, PathMode::Local);
        assert!(interp.is_redacted());
        assert_eq!(interp.reason_str(), reason::INTERPOLATION_SOURCE);
    }

    #[test]
    fn mcp_environment_and_env_maps_are_redacted_wholesale() {
        let p = policy();
        let mcp = redact_value(
            "/mcp/jira/environment",
            &json!({"JIRA_TOKEN": "SEEDED"}),
            &p,
            PathMode::Local,
        );
        assert!(mcp.is_redacted());
        assert_eq!(mcp.reason_str(), reason::MCP_ENVIRONMENT);

        let env = redact_value(
            "/env/ANTHROPIC_API_KEY",
            &json!("SEEDED"),
            &p,
            PathMode::Local,
        );
        assert!(env.is_redacted());
        assert_eq!(env.reason_str(), reason::ENV_VALUE);
    }

    #[test]
    fn urls_have_userinfo_query_and_fragment_removed() {
        let p = policy();
        let out = redact_value(
            "/mcp/jira/url",
            &json!("https://user:pass@jira.internal.example/mcp?token=SEEDED#frag"),
            &p,
            PathMode::Local,
        );
        let shown = out.as_value().and_then(|v| v.as_str()).unwrap_or_default();
        assert_eq!(shown, "https://jira.internal.example/mcp?[redacted]");
        assert!(!shown.contains("SEEDED"));
        assert!(!shown.contains("pass"));
    }

    #[test]
    fn inventory_only_pointers_are_never_revealed() {
        let p = policy();
        let out = redact_value(
            "/provider/anthropic/name",
            &json!("anthropic"),
            &p,
            PathMode::Local,
        );
        assert!(out.is_redacted());
    }

    #[test]
    fn descendants_of_a_redacted_subtree_are_redacted_too() {
        let p = policy();
        for pointer in [
            "/mcp/jira/environment/JIRA_TOKEN",
            "/mcp/jira/environment/nested/deep",
            "/env/ANTHROPIC_API_KEY/extra",
        ] {
            let out = redact_value(pointer, &json!("SEEDED"), &p, PathMode::Local);
            assert!(out.is_redacted(), "{pointer} must stay redacted");
            assert!(!serde_json::to_string(&out).unwrap().contains("SEEDED"));
        }
    }

    #[test]
    fn tool_rule_lists_are_shown_locally_and_pseudonymized_in_share_mode() {
        let p = policy();
        let rules = json!(["Bash(npm run test *)", "Read(./.env)", "not a rule"]);
        let local = redact_value("/permissions/allow", &rules, &p, PathMode::Local);
        let local_rules = local
            .as_value()
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        assert_eq!(local_rules.len(), 3);
        assert_eq!(local_rules[0], json!("Bash(npm run test *)"));
        assert_eq!(
            local_rules[2],
            json!({"redacted": true, "reason": reason::UNCLASSIFIED})
        );

        let share = redact_value("/permissions/allow", &rules, &p, PathMode::Share);
        let share_rules = share
            .as_value()
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        assert_eq!(share_rules.len(), 3);
        let first = share_rules[0].as_str().unwrap_or_default().to_string();
        assert!(first.starts_with("rule_"), "got {first}");
        let serialized = serde_json::to_string(&share).unwrap();
        assert!(!serialized.contains("npm run test"));
    }

    #[test]
    fn unclassified_strings_and_objects_are_redacted() {
        let p = policy();
        assert_eq!(
            redact_value("/unknown/thing", &json!("free text"), &p, PathMode::Local).reason_str(),
            reason::UNCLASSIFIED
        );
        let obj = redact_value("/unknown/thing", &json!({"a": 1}), &p, PathMode::Local);
        assert!(obj.is_redacted());
        assert_eq!(obj.kind, ValueKind::Object);
    }

    #[test]
    fn revealed_strings_are_sanitized() {
        let p = policy();
        let out = redact_value(
            "/theme",
            &json!("dark\u{1b}]8;;http://evil\u{7}\n!"),
            &p,
            PathMode::Local,
        );
        let shown = out.as_value().and_then(|v| v.as_str()).unwrap_or_default();
        assert_eq!(shown, "dark\\n!");
    }

    #[test]
    fn helper_shape_checks_are_bounded() {
        assert!(is_model_name_shape("anthropic/claude-sonnet-4-5"));
        assert!(!is_model_name_shape(""));
        assert!(!is_model_name_shape("has space"));
        assert!(!is_model_name_shape(&"a".repeat(129)));
        assert!(
            !is_model_name_shape("SEEDED-VALUE-9f3c1a"),
            "upper-case is not a model name"
        );
        assert!(is_plain_string("tokyonight"));
        assert!(!is_plain_string("{env:X}"));
    }

    #[test]
    fn normalize_field_ignores_separators_and_case() {
        assert_eq!(normalize_field("API_KEY"), "apikey");
        assert_eq!(normalize_field("api-key"), "apikey");
        assert_eq!(normalize_field("apiKey"), "apikey");
    }

    #[test]
    fn sensitive_matching_is_prefix_or_suffix_bounded() {
        let pats = vec!["token".to_string(), "key".to_string()];
        assert!(is_sensitive_field("/auth/token", &pats));
        assert!(is_sensitive_field("/x/refreshToken", &pats));
        assert!(is_sensitive_field("/x/tokenValue", &pats));
        assert!(!is_sensitive_field("/x/monkeybusiness", &pats));
        assert!(!is_sensitive_field("/x/name", &pats));
    }

    #[test]
    fn sanitize_url_rejects_non_urls() {
        assert_eq!(sanitize_url("not a url"), None);
        assert_eq!(sanitize_url("file:///etc/passwd"), None);
        assert_eq!(
            sanitize_url("https://host.example/path"),
            Some("https://host.example/path".to_string())
        );
    }

    #[test]
    fn url_host_extracts_the_authority_without_userinfo() {
        assert_eq!(
            url_host("https://user:pass@jira.internal.example/mcp?x=1"),
            Some("jira.internal.example".to_string())
        );
        assert_eq!(url_host("wss://h.example"), Some("h.example".to_string()));
        assert_eq!(url_host("file:///etc/passwd"), None);
        assert_eq!(url_host("not a url"), None);
    }

    #[test]
    fn pseudonymizer_is_deterministic_from_sorted_names() {
        let a = Pseudonymizer::new(
            vec!["zeta".to_string(), "alpha".to_string(), "alpha".to_string()],
            vec!["b.example".to_string(), "a.example".to_string()],
        );
        let b = Pseudonymizer::new(
            vec!["alpha".to_string(), "zeta".to_string()],
            vec!["a.example".to_string(), "b.example".to_string()],
        );
        assert_eq!(a.field("alpha"), "field_1");
        assert_eq!(a.field("zeta"), "field_2");
        assert_eq!(a.field("alpha"), b.field("alpha"));
        assert_eq!(a.host("a.example"), "host_1");
        assert_eq!(a.field("unknown"), "field_?");
    }
}
