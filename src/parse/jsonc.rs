//! JSONC comment removal without regexes (spec §6: "Avoid regex comment stripping for JSONC").
//!
//! [`strip_jsonc`] replaces comment bytes with ASCII spaces, so the returned text is **byte-for-byte
//! the same length** as the input and every remaining byte keeps its original offset. That makes
//! every span and line/column number computed from the stripped text identical to the original
//! document, with no offset map to get wrong.

/// Replace JSONC comments with spaces, preserving byte length and every newline.
pub fn strip_jsonc(source: &str) -> String {
    let bytes = source.as_bytes();
    let n = bytes.len();
    let mut out = bytes.to_vec();
    let mut i = 0usize;
    while i < n {
        match bytes[i] {
            b'"' => {
                // Copy the string literal verbatim, honouring backslash escapes.
                i += 1;
                while i < n {
                    match bytes[i] {
                        b'\\' => i += 2,
                        b'"' => {
                            i += 1;
                            break;
                        }
                        _ => i += 1,
                    }
                }
            }
            b'/' if i + 1 < n && bytes[i + 1] == b'/' => {
                while i < n && bytes[i] != b'\n' {
                    out[i] = b' ';
                    i += 1;
                }
            }
            b'/' if i + 1 < n && bytes[i + 1] == b'*' => {
                out[i] = b' ';
                out[i + 1] = b' ';
                i += 2;
                while i < n {
                    if bytes[i] == b'*' && i + 1 < n && bytes[i + 1] == b'/' {
                        out[i] = b' ';
                        out[i + 1] = b' ';
                        i += 2;
                        break;
                    }
                    if bytes[i] != b'\n' {
                        out[i] = b' ';
                    }
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }
    // Replacing whole byte runs with ASCII spaces keeps the result valid UTF-8.
    String::from_utf8(out).unwrap_or_else(|_| source.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_identity_for_comment_free_input() {
        let src = "{\"a\": 1}";
        assert_eq!(strip_jsonc(src), src);
    }

    #[test]
    fn preserves_byte_length_and_newlines() {
        let src = "{\n  // comment here\n  \"a\": 1\n}";
        let out = strip_jsonc(src);
        assert_eq!(out.len(), src.len(), "length must be preserved");
        assert_eq!(out.matches('\n').count(), src.matches('\n').count());
        assert_eq!(
            out.lines().nth(1).map(str::trim_end),
            Some(""),
            "the comment line must become blank, not disappear"
        );
    }

    #[test]
    fn removes_line_comments() {
        let src = "{\n  \"model\": \"x\" // trailing\n}";
        let out = strip_jsonc(src);
        assert!(!out.contains("trailing"));
        assert!(out.contains("\"model\": \"x\""));
    }

    #[test]
    fn removes_block_comments() {
        let src = "{\n  /* a\n     b /* not nested */ c */\n  \"a\": 1\n}";
        let out = strip_jsonc(src);
        assert!(!out.contains("nested"));
        assert!(out.contains("\"a\": 1"));
    }

    #[test]
    fn unterminated_block_comment_runs_to_end() {
        let src = "{\"a\": 1}\n/* never closed";
        let out = strip_jsonc(src);
        assert!(out.starts_with("{\"a\": 1}"));
        assert!(!out.contains("never"));
        assert_eq!(out.len(), src.len());
    }

    #[test]
    fn comment_markers_inside_strings_are_not_comments() {
        let src = "{\"url\": \"https://example.test/a//b\", \"glob\": \"**/*.rs\"}";
        let out = strip_jsonc(src);
        assert_eq!(out, src, "// and /* inside strings must survive untouched");
    }

    #[test]
    fn escaped_quote_does_not_end_a_string() {
        let src = r#"{"a": "say \"// not a comment\" ok", "b": 2}"#;
        let out = strip_jsonc(src);
        assert_eq!(out, src);
    }

    #[test]
    fn preserves_multibyte_strings() {
        let src = "{\"note\": \"naïve 日本語 // x\"}";
        let out = strip_jsonc(src);
        assert_eq!(out, src);
        assert!(std::str::from_utf8(out.as_bytes()).is_ok());
    }

    #[test]
    fn multibyte_comment_content_is_replaced_by_ascii_spaces() {
        let src = "{\"a\": 1} // 日本語のコメント";
        let out = strip_jsonc(src);
        assert_eq!(out.len(), src.len());
        assert!(!out.contains('日'));
        assert!(out.is_char_boundary(out.len()));
    }
}
