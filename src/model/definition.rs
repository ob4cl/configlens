//! Definitions, spans and the non-serializable sensitive value type.

use serde_json::Value;

use crate::security::sanitize;

/// Half-open byte range inside one source document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Span {
    pub start_byte: usize,
    pub end_byte: usize,
}

impl Span {
    pub fn new(start_byte: usize, end_byte: usize) -> Self {
        let (start_byte, end_byte) = if start_byte <= end_byte {
            (start_byte, end_byte)
        } else {
            (end_byte, start_byte)
        };
        Self {
            start_byte,
            end_byte,
        }
    }

    /// 1-based (line, column) of the span start, in **characters** for the column.
    pub fn line_column(&self, source: &str) -> (u32, u32) {
        let mut line = 1u32;
        let mut column = 1u32;
        for (idx, ch) in source.char_indices() {
            if idx >= self.start_byte {
                break;
            }
            if ch == '\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }
        (line, column)
    }
}

/// A raw configuration value.
///
/// Deliberately implements **no** `Serialize`, no `Display`, and a `Debug` that cannot leak the
/// value (spec §6). Only [`crate::security::redact`] may convert this into a report value.
#[derive(Clone, PartialEq)]
pub struct SensitiveValue(Value);

impl SensitiveValue {
    pub fn new(value: Value) -> Self {
        Self(value)
    }

    /// Structural access for reducers. Reducers are library-internal and never render.
    pub fn raw(&self) -> &Value {
        &self.0
    }

    pub fn into_raw(self) -> Value {
        self.0
    }

    pub fn kind(&self) -> ValueKind {
        ValueKind::of(&self.0)
    }
}

impl std::fmt::Debug for SensitiveValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SensitiveValue(<redacted>)")
    }
}

/// The shape of a value, for reports that must describe without disclosing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueKind {
    Null,
    Bool,
    Integer,
    Number,
    String,
    Array,
    Object,
}

impl ValueKind {
    pub fn of(value: &Value) -> Self {
        match value {
            Value::Null => ValueKind::Null,
            Value::Bool(_) => ValueKind::Bool,
            Value::Number(n) if n.is_i64() || n.is_u64() => ValueKind::Integer,
            Value::Number(_) => ValueKind::Number,
            Value::String(_) => ValueKind::String,
            Value::Array(_) => ValueKind::Array,
            Value::Object(_) => ValueKind::Object,
        }
    }
}

/// Sanitized text helper used whenever a string from a source becomes part of a report.
pub fn safe_text(value: &str) -> String {
    sanitize(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn span_normalizes_reversed_bounds() {
        let s = Span::new(9, 3);
        assert_eq!((s.start_byte, s.end_byte), (3, 9));
    }

    #[test]
    fn span_line_column_is_one_based() {
        let src = "{\n  \"a\": 1\n}";
        let start = src.find("\"a\"").expect("present");
        let span = Span::new(start, start + 3);
        assert_eq!(span.line_column(src), (2, 3));
        assert_eq!(Span::new(0, 1).line_column(src), (1, 1));
    }

    #[test]
    fn span_line_column_counts_characters_not_bytes() {
        let src = "{\"é\": 1}";
        let start = src.find('1').expect("present");
        let (line, column) = Span::new(start, start + 1).line_column(src);
        assert_eq!(line, 1);
        // 1-based character columns: { " é " : space 1  ->  the '1' is column 7
        assert_eq!(column, 7);
    }

    #[test]
    fn sensitive_value_debug_does_not_leak_the_value() {
        let v = SensitiveValue::new(json!("super-secret-token"));
        let dbg = format!("{v:?}");
        assert!(!dbg.contains("super-secret-token"));
        assert_eq!(dbg, "SensitiveValue(<redacted>)");
    }

    #[test]
    fn value_kind_distinguishes_shapes() {
        assert_eq!(ValueKind::of(&json!(null)), ValueKind::Null);
        assert_eq!(ValueKind::of(&json!(true)), ValueKind::Bool);
        assert_eq!(ValueKind::of(&json!(3)), ValueKind::Integer);
        assert_eq!(ValueKind::of(&json!(3.5)), ValueKind::Number);
        assert_eq!(ValueKind::of(&json!("x")), ValueKind::String);
        assert_eq!(ValueKind::of(&json!([])), ValueKind::Array);
        assert_eq!(ValueKind::of(&json!({})), ValueKind::Object);
    }
}
