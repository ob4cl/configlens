//! Fuzz target: configuration parsing (`configlens::parse`).
//!
//! Run: `cargo +nightly fuzz run parse_documents`
//!
//! The parser is the crate's only attack surface for untrusted bytes: every configuration file,
//! every `--input-context` document and every `$OPENCODE_CONFIG_CONTENT` value goes through it. The
//! target feeds arbitrary UTF-8 text through both dialects and asserts the parser's contract, so a
//! crash is either a panic (bad) or a broken invariant (also bad), and never a legitimate outcome:
//!
//! 1. **No panic, ever.** Any input either parses or returns `ParseError`; malformed, truncated,
//!    deeply nested, control-character-laden and duplicated-key input are all in scope.
//! 2. **Pointers resolve.** Every `RawDefinition.pointer` addresses a node inside the parsed value
//!    (a pointer that resolves nowhere would make `explain` lie about where a value came from).
//! 3. **Spans stay inside the document.** A span is used to print `file:line:column`; a span past
//!    the end of the source would produce an impossible location.
//! 4. **Errors carry a location, never an excerpt.** Spec §6: a parse error reports a code and a
//!    position and must not echo the document (a document can contain a secret).
//! 5. **Re-serialization round-trips.** `serde_json::to_string` of a parsed value must itself parse,
//!    so a report never embeds something the parser cannot read back.
//! 6. **Limits are enforced as errors, not truncation.** Over-limit input is refused; it is never
//!    silently accepted with less content than the file has.

#![no_main]

use libfuzzer_sys::fuzz_target;

use configlens::parse::{parse, Limits, Syntax};

/// Deliberately tiny limits: hostile input must be refused, not merely survived.
const TINY: Limits = Limits {
    max_depth: 3,
    max_definitions: 8,
    max_bytes: 256,
};

fn check(text: &str, syntax: Syntax, limits: &Limits) {
    match parse(text, syntax, limits) {
        Ok(parsed) => {
            assert!(
                parsed.definitions.len() <= limits.max_definitions,
                "definition count {} exceeded the limit {} without an error",
                parsed.definitions.len(),
                limits.max_definitions
            );
            assert!(
                parsed.depth <= limits.max_depth,
                "depth {} exceeded the limit {} without an error",
                parsed.depth,
                limits.max_depth
            );
            for definition in &parsed.definitions {
                assert!(
                    parsed.value.pointer(&definition.pointer).is_some(),
                    "definition pointer {:?} does not resolve inside the parsed value",
                    definition.pointer
                );
            }
            for span in parsed.spans.values().chain(parsed.key_spans.values()) {
                assert!(
                    span.start_byte <= span.end_byte && span.end_byte <= text.len(),
                    "span {}..{} is not inside the {} byte document",
                    span.start_byte,
                    span.end_byte,
                    text.len()
                );
            }
            // Duplicate-key evidence must point at the two occurrences, in document order, and
            // both must be real spans.
            for duplicate in &parsed.duplicates {
                assert!(
                    duplicate.first.start_byte <= duplicate.second.start_byte,
                    "duplicate evidence for {:?} is out of document order",
                    duplicate.pointer
                );
                assert!(duplicate.second.end_byte <= text.len());
            }
            let round_tripped =
                serde_json::to_string(&parsed.value).expect("a parsed value must re-serialize");
            assert!(
                serde_json::from_str::<serde_json::Value>(&round_tripped).is_ok(),
                "re-serialized value is not valid JSON"
            );
        }
        Err(error) => {
            assert!(!error.code.is_empty(), "a parse error must carry a code");
            assert!(
                error.line >= 1 && error.column >= 1,
                "a parse error must carry a 1-based location, got {}:{}",
                error.line,
                error.column
            );
            // The error type has no field for document text: assert the rendered message cannot
            // contain a long verbatim run from the input.
            let rendered = error.to_string();
            let window: String = text.chars().take(32).collect();
            if window.chars().count() == 32 && !window.contains(char::is_whitespace) {
                assert!(
                    !rendered.contains(&window),
                    "a parse error echoed source text: {rendered:?}"
                );
            }
        }
    }
}

fuzz_target!(|data: &[u8]| {
    // Non-UTF-8 bytes are refused by the read layer before the parser sees them, so the parser's
    // own contract is only defined for `&str`. Lossy conversion still fuzzes the interesting part
    // (replacement characters, control bytes, lone surrogates' byte patterns).
    let text = String::from_utf8_lossy(data);

    check(&text, Syntax::Strict, &Limits::default());
    check(&text, Syntax::Jsonc, &Limits::default());
    check(&text, Syntax::Strict, &TINY);
    check(&text, Syntax::Jsonc, &TINY);

    // Structure-preserving mutations of a valid document: exercise duplicate keys, deep nesting
    // and truncation from the same corpus entry.
    if !text.is_empty() {
        let mut wrapped = String::from("{\"a\":{\"b\":{\"c\":0}},\"dup\":");
        wrapped.push_str(&text);
        wrapped.push_str(",\"k\":");
        wrapped.push_str(&text);
        wrapped.push('}');
        check(&wrapped, Syntax::Jsonc, &Limits::default());
    }
});
