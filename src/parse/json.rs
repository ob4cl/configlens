//! Span-preserving JSON / JSONC parsing with duplicate-key evidence.
//!
//! Spec §6: "Do not use a JSON parser that silently loses duplicate-key evidence; write a detection
//! pass if necessary." This is that pass, fused with the parse so spans and duplicates come from a
//! single traversal of one document.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::model::definition::Span;

/// Which dialect to accept. Claude Code settings files are *strict* JSON (a `//` comment or a
/// trailing comma is a syntax error); OpenCode accepts JSON and JSONC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Syntax {
    Strict,
    Jsonc,
}

/// Parsing resource limits (spec §7 defaults).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub max_depth: usize,
    pub max_definitions: usize,
    pub max_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_depth: 64,
            max_definitions: 8_192,
            max_bytes: 2 * 1024 * 1024,
        }
    }
}

/// A parse failure with a safe location and no source excerpt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub code: &'static str,
    pub offset: usize,
    pub line: u32,
    pub column: u32,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} at line {}, column {}",
            self.code, self.line, self.column
        )
    }
}

impl std::error::Error for ParseError {}

/// A duplicate object key observed in one document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateKey {
    pub pointer: String,
    pub first: Span,
    pub second: Span,
}

/// One value found at a JSON Pointer, with its byte span in the original document.
#[derive(Debug, Clone, PartialEq)]
pub struct RawDefinition {
    pub pointer: String,
    pub value: Value,
    pub span: Option<Span>,
}

/// Everything a later stage needs from one parsed document.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedSource {
    pub value: Value,
    /// Pointer → span of the *value* token.
    pub spans: BTreeMap<String, Span>,
    /// Pointer → span of the *key* token (absent for the document root).
    pub key_spans: BTreeMap<String, Span>,
    pub duplicates: Vec<DuplicateKey>,
    /// Every pointer in the document, sorted, including container nodes.
    pub definitions: Vec<RawDefinition>,
    pub depth: usize,
}

/// Parse `source` with the given dialect and limits.
pub fn parse(source: &str, syntax: Syntax, limits: &Limits) -> Result<ParsedSource, ParseError> {
    if source.len() > limits.max_bytes {
        return Err(ParseError {
            code: "resource_limit_exceeded",
            offset: 0,
            line: 1,
            column: 1,
        });
    }
    // JSONC comments are blanked in place, so every byte offset below still refers to `source`.
    let text = match syntax {
        Syntax::Jsonc => crate::parse::jsonc::strip_jsonc(source),
        Syntax::Strict => source.to_string(),
    };
    let mut p = Parser {
        src: text.as_bytes(),
        original: source,
        pos: 0,
        syntax,
        limits,
        depth: 0,
        count: 0,
        spans: BTreeMap::new(),
        key_spans: BTreeMap::new(),
        duplicates: Vec::new(),
    };
    if p.src.starts_with(&[0xEF, 0xBB, 0xBF]) {
        p.pos = 3; // UTF-8 BOM: skip it, keep absolute offsets.
    }
    p.skip_ws();
    if p.pos >= p.src.len() {
        return Err(p.error("empty_document", p.pos));
    }
    let value = p.parse_value("")?;
    p.skip_ws();
    if p.pos != p.src.len() {
        return Err(p.error("unexpected_token", p.pos));
    }
    let mut definitions = Vec::new();
    collect_definitions(&value, "", &p.spans, &mut definitions);
    definitions.sort_by(|a, b| a.pointer.cmp(&b.pointer));
    let depth = definitions
        .iter()
        .map(|d| d.pointer.matches('/').count())
        .max()
        .unwrap_or(0);
    Ok(ParsedSource {
        value,
        spans: p.spans,
        key_spans: p.key_spans,
        duplicates: p.duplicates,
        definitions,
        depth,
    })
}

fn collect_definitions(
    value: &Value,
    pointer: &str,
    spans: &BTreeMap<String, Span>,
    out: &mut Vec<RawDefinition>,
) {
    out.push(RawDefinition {
        pointer: pointer.to_string(),
        value: value.clone(),
        span: spans.get(pointer).copied(),
    });
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                collect_definitions(child, &child_pointer(pointer, key), spans, out);
            }
        }
        Value::Array(items) => {
            for (idx, child) in items.iter().enumerate() {
                collect_definitions(child, &child_pointer(pointer, &idx.to_string()), spans, out);
            }
        }
        _ => {}
    }
}

/// RFC 6901 pointer join.
pub fn child_pointer(parent: &str, token: &str) -> String {
    let escaped = token.replace('~', "~0").replace('/', "~1");
    format!("{parent}/{escaped}")
}

struct Parser<'a> {
    src: &'a [u8],
    original: &'a str,
    pos: usize,
    syntax: Syntax,
    limits: &'a Limits,
    depth: usize,
    count: usize,
    spans: BTreeMap<String, Span>,
    key_spans: BTreeMap<String, Span>,
    duplicates: Vec<DuplicateKey>,
}

impl Parser<'_> {
    fn error(&self, code: &'static str, offset: usize) -> ParseError {
        let (line, column) = line_column(self.original, offset);
        ParseError {
            code,
            offset,
            line,
            column,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while let Some(b) = self.peek() {
            if matches!(b, b' ' | b'\t' | b'\n' | b'\r') {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn bump_definition(&mut self) -> Result<(), ParseError> {
        self.count += 1;
        if self.count > self.limits.max_definitions {
            let at = self.pos;
            return Err(self.error("definition_limit_exceeded", at));
        }
        Ok(())
    }

    fn parse_value(&mut self, pointer: &str) -> Result<Value, ParseError> {
        self.skip_ws();
        let start = self.pos;
        let value = match self.peek() {
            None => return Err(self.error("empty_document", start)),
            Some(b'{') => self.parse_object(pointer)?,
            Some(b'[') => self.parse_array(pointer)?,
            Some(b'"') => Value::String(self.parse_string_token()?),
            Some(b't') => {
                self.expect_literal("true")?;
                Value::Bool(true)
            }
            Some(b'f') => {
                self.expect_literal("false")?;
                Value::Bool(false)
            }
            Some(b'n') => {
                self.expect_literal("null")?;
                Value::Null
            }
            Some(b'-') | Some(b'0'..=b'9') => self.parse_number_token()?,
            Some(_) => return Err(self.error("unexpected_token", start)),
        };
        self.spans
            .insert(pointer.to_string(), Span::new(start, self.pos));
        self.bump_definition()?;
        Ok(value)
    }

    fn enter_container(&mut self, start: usize) -> Result<(), ParseError> {
        self.depth += 1;
        if self.depth > self.limits.max_depth {
            return Err(self.error("depth_limit_exceeded", start));
        }
        Ok(())
    }

    fn parse_object(&mut self, pointer: &str) -> Result<Value, ParseError> {
        let start = self.pos;
        self.enter_container(start)?;
        self.pos += 1; // '{'
        let mut map = serde_json::Map::new();
        loop {
            self.skip_ws();
            if self.peek() == Some(b'}') {
                self.pos += 1;
                break;
            }
            if self.peek() != Some(b'"') {
                return Err(self.error("unexpected_token", self.pos));
            }
            let key_start = self.pos;
            let key = self.parse_string_token()?;
            let key_span = Span::new(key_start, self.pos);
            self.skip_ws();
            if self.peek() != Some(b':') {
                return Err(self.error("unexpected_token", self.pos));
            }
            self.pos += 1;
            let child = child_pointer(pointer, &key);
            self.key_spans.insert(child.clone(), key_span);
            // Capture any earlier occurrence before the value overwrites the span table.
            let prior = self.spans.get(&child).copied();
            let value = self.parse_value(&child)?;
            if let (Some(first), Some(second)) = (prior, self.spans.get(&child).copied()) {
                self.duplicates.push(DuplicateKey {
                    pointer: child.clone(),
                    first,
                    second,
                });
            }
            map.insert(key, value);
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                    self.skip_ws();
                    if matches!(self.peek(), Some(b'}')) {
                        if self.syntax == Syntax::Strict {
                            return Err(self.error("trailing_comma", self.pos));
                        }
                        self.pos += 1;
                        break;
                    }
                }
                Some(b'}') => {
                    self.pos += 1;
                    break;
                }
                _ => return Err(self.error("unexpected_token", self.pos)),
            }
        }
        self.depth -= 1;
        Ok(Value::Object(map))
    }

    fn parse_array(&mut self, pointer: &str) -> Result<Value, ParseError> {
        let start = self.pos;
        self.enter_container(start)?;
        self.pos += 1; // '['
        let mut items = Vec::new();
        loop {
            self.skip_ws();
            match self.peek() {
                Some(b']') => {
                    self.pos += 1;
                    break;
                }
                None => return Err(self.error("unexpected_token", self.pos)),
                _ => {}
            }
            let child = child_pointer(pointer, &items.len().to_string());
            let value = self.parse_value(&child)?;
            items.push(value);
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                    self.skip_ws();
                    if matches!(self.peek(), Some(b']')) {
                        if self.syntax == Syntax::Strict {
                            return Err(self.error("trailing_comma", self.pos));
                        }
                        self.pos += 1;
                        break;
                    }
                }
                Some(b']') => {
                    self.pos += 1;
                    break;
                }
                _ => return Err(self.error("unexpected_token", self.pos)),
            }
        }
        self.depth -= 1;
        Ok(Value::Array(items))
    }

    /// Read one `"..."` token and decode it with serde_json (single well-tested unescaper).
    fn parse_string_token(&mut self) -> Result<String, ParseError> {
        let start = self.pos;
        debug_assert_eq!(self.peek(), Some(b'"'));
        self.pos += 1;
        loop {
            match self.peek() {
                None => return Err(self.error("unterminated_string", start)),
                Some(b'\\') => self.pos += 2,
                Some(b'"') => {
                    self.pos += 1;
                    break;
                }
                Some(_) => self.pos += 1,
            }
        }
        match serde_json::from_slice::<String>(&self.src[start..self.pos]) {
            Ok(s) => Ok(s),
            Err(_) => Err(self.error("invalid_string", start)),
        }
    }

    fn parse_number_token(&mut self) -> Result<Value, ParseError> {
        let start = self.pos;
        while let Some(b) = self.peek() {
            if matches!(b, b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                self.pos += 1;
            } else {
                break;
            }
        }
        match serde_json::from_slice::<Value>(&self.src[start..self.pos]) {
            Ok(v) => Ok(v),
            Err(_) => Err(self.error("invalid_number", start)),
        }
    }

    fn expect_literal(&mut self, literal: &str) -> Result<(), ParseError> {
        let start = self.pos;
        let end = start + literal.len();
        if end <= self.src.len() && &self.src[start..end] == literal.as_bytes() {
            self.pos = end;
            Ok(())
        } else {
            Err(self.error("unexpected_token", start))
        }
    }
}

/// 1-based line and character column of a byte offset.
fn line_column(source: &str, offset: usize) -> (u32, u32) {
    let mut line = 1u32;
    let mut column = 1u32;
    for (idx, ch) in source.char_indices() {
        if idx >= offset {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn obj(src: &str) -> ParsedSource {
        parse(src, Syntax::Jsonc, &Limits::default()).expect("parse should succeed")
    }

    #[test]
    fn parses_a_flat_object_and_flattens_pointers() {
        let doc = obj("{\"model\": \"a\", \"share\": \"auto\"}");
        assert_eq!(doc.value["model"], json!("a"));
        let pointers: Vec<&str> = doc.definitions.iter().map(|d| d.pointer.as_str()).collect();
        // The document root (pointer "") is itself addressable, so it is included.
        assert_eq!(pointers, vec!["", "/model", "/share"]);
    }

    #[test]
    fn flattens_nested_containers_and_array_indices() {
        let doc = obj("{\"a\": {\"b\": [10, {\"c\": true}]}}");
        let pointers: Vec<&str> = doc.definitions.iter().map(|d| d.pointer.as_str()).collect();
        assert_eq!(
            pointers,
            vec!["", "/a", "/a/b", "/a/b/0", "/a/b/1", "/a/b/1/c"]
        );
    }

    #[test]
    fn escapes_pointer_tokens_per_rfc6901() {
        let doc = obj("{\"a/b\": 1, \"c~d\": 2}");
        let pointers: Vec<&str> = doc.definitions.iter().map(|d| d.pointer.as_str()).collect();
        assert_eq!(pointers, vec!["", "/a~1b", "/c~0d"]);
    }

    #[test]
    fn spans_point_at_the_value_token() {
        let src = "{\"model\": \"model-a\"}";
        let doc = obj(src);
        let span = doc.spans["/model"];
        assert_eq!(&src[span.start_byte..span.end_byte], "\"model-a\"");
        let key = doc.key_spans["/model"];
        assert_eq!(&src[key.start_byte..key.end_byte], "\"model\"");
    }

    #[test]
    fn spans_survive_jsonc_comments() {
        let src = "{\n  // why\n  \"model\": \"b\"\n}";
        let doc = obj(src);
        let span = doc.spans["/model"];
        assert_eq!(&src[span.start_byte..span.end_byte], "\"b\"");
    }

    #[test]
    fn duplicate_keys_are_recorded_and_the_last_value_wins() {
        let src = "{\"model\": \"first\", \"model\": \"second\"}";
        let doc = obj(src);
        assert_eq!(doc.value["model"], json!("second"));
        assert_eq!(doc.duplicates.len(), 1);
        let dup = &doc.duplicates[0];
        assert_eq!(dup.pointer, "/model");
        assert_eq!(&src[dup.first.start_byte..dup.first.end_byte], "\"first\"");
        assert_eq!(
            &src[dup.second.start_byte..dup.second.end_byte],
            "\"second\""
        );
    }

    #[test]
    fn null_empty_array_and_empty_object_are_distinct() {
        let doc = obj("{\"a\": null, \"b\": [], \"c\": {}}");
        assert!(doc.value["a"].is_null());
        assert_eq!(doc.value["b"], json!([]));
        assert_eq!(doc.value["c"], json!({}));
        let pointers: Vec<&str> = doc.definitions.iter().map(|d| d.pointer.as_str()).collect();
        assert_eq!(pointers, vec!["", "/a", "/b", "/c"]);
    }

    #[test]
    fn parses_escapes_and_unicode() {
        let doc = obj(r#"{"a": "line\nbreak", "b": "\u00e9", "c": "日本"}"#);
        assert_eq!(doc.value["a"], json!("line\nbreak"));
        assert_eq!(doc.value["b"], json!("é"));
        assert_eq!(doc.value["c"], json!("日本"));
    }

    #[test]
    fn parses_numbers_booleans_and_exponents() {
        let doc =
            obj("{\"i\": -7, \"f\": 1.5e3, \"t\": true, \"n\": false, \"big\": 9007199254740993}");
        assert_eq!(doc.value["i"], json!(-7));
        assert_eq!(doc.value["f"], json!(1500.0));
        assert_eq!(doc.value["t"], json!(true));
        assert_eq!(doc.value["n"], json!(false));
        assert_eq!(doc.value["big"].as_u64(), Some(9007199254740993));
    }

    #[test]
    fn strict_syntax_rejects_comments_and_trailing_commas() {
        let commented = "{\"a\": 1 // no\n}";
        let err = parse(commented, Syntax::Strict, &Limits::default()).unwrap_err();
        assert_eq!(err.code, "unexpected_token");
        assert_eq!(err.line, 1);

        let trailing = "{\"a\": 1,}";
        let err = parse(trailing, Syntax::Strict, &Limits::default()).unwrap_err();
        assert_eq!(err.code, "trailing_comma");

        assert!(parse("{\"a\": 1,}", Syntax::Jsonc, &Limits::default()).is_ok());
        assert!(parse(commented, Syntax::Jsonc, &Limits::default()).is_ok());
    }

    #[test]
    fn jsonc_trailing_comma_inside_array_is_accepted() {
        let doc = obj("[1, 2,]");
        assert_eq!(doc.value, json!([1, 2]));
    }

    #[test]
    fn malformed_input_reports_a_safe_location() {
        let err = parse("{\"a\": }", Syntax::Jsonc, &Limits::default()).unwrap_err();
        assert_eq!(err.code, "unexpected_token");
        assert_eq!(err.line, 1);
        assert_eq!(err.column, 7);
        // No excerpt and no raw content in Display.
        assert_eq!(err.to_string(), "unexpected_token at line 1, column 7");
        assert!(!err.to_string().contains('}'));
    }

    #[test]
    fn unterminated_string_is_reported() {
        let err = parse("{\"a\": \"oops}", Syntax::Jsonc, &Limits::default()).unwrap_err();
        assert_eq!(err.code, "unterminated_string");
    }

    #[test]
    fn empty_document_is_an_error() {
        let err = parse("   \n ", Syntax::Jsonc, &Limits::default()).unwrap_err();
        assert_eq!(err.code, "empty_document");
    }

    #[test]
    fn depth_limit_is_enforced() {
        let mut src = String::new();
        for _ in 0..8 {
            src.push('[');
        }
        src.push('1');
        for _ in 0..8 {
            src.push(']');
        }
        let limits = Limits {
            max_depth: 4,
            ..Limits::default()
        };
        let err = parse(&src, Syntax::Jsonc, &limits).unwrap_err();
        assert_eq!(err.code, "depth_limit_exceeded");
        assert!(parse(&src, Syntax::Jsonc, &Limits::default()).is_ok());
    }

    #[test]
    fn definition_limit_is_enforced() {
        let src = "[\"a\",\"b\",\"c\",\"d\"]";
        let limits = Limits {
            max_definitions: 2,
            ..Limits::default()
        };
        let err = parse(src, Syntax::Jsonc, &limits).unwrap_err();
        assert_eq!(err.code, "definition_limit_exceeded");
    }

    #[test]
    fn byte_limit_is_enforced_before_parsing() {
        let limits = Limits {
            max_bytes: 4,
            ..Limits::default()
        };
        let err = parse("{\"a\": 1}", Syntax::Jsonc, &limits).unwrap_err();
        assert_eq!(err.code, "resource_limit_exceeded");
    }

    #[test]
    fn utf8_bom_is_skipped_without_shifting_spans() {
        let src = "\u{feff}{\"a\": 1}";
        let doc = obj(src);
        let span = doc.spans["/a"];
        assert_eq!(&src[span.start_byte..span.end_byte], "1");
    }

    #[test]
    fn top_level_scalars_are_allowed() {
        let doc = obj("\"just a string\"");
        assert_eq!(doc.value, json!("just a string"));
        assert_eq!(doc.definitions.len(), 1);
        assert_eq!(doc.definitions[0].pointer, "");
    }

    #[test]
    fn depth_is_reported() {
        let doc = obj("{\"a\": {\"b\": 1}}");
        assert_eq!(doc.depth, 2);
    }
}
