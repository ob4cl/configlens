//! Output sanitization for every user-derived string.
//!
//! Spec §7: "Sanitize terminal control sequences (including OSC hyperlinks), carriage returns,
//! bidi controls and untrusted newlines. Apply to errors, paths, pointer displays, help-context
//! errors and all user-derived strings."

/// Maximum length of a sanitized value rendered into a report.
///
/// Contract for [`sanitize_bounded`]: at most `max_chars` characters of the *sanitized* string are
/// kept, and `…` is appended when truncation happened (so the result may be `max_chars + 1` chars).
pub const MAX_SANITIZED_CHARS: usize = 200;

/// True for characters that must never reach a terminal or a log line: bidi controls, zero-width
/// and other invisible format characters, and Unicode line/paragraph separators.
fn is_invisible(c: char) -> bool {
    matches!(c,
        '\u{061c}'
        | '\u{200b}'..='\u{200f}'
        | '\u{2028}'..='\u{202e}'
        | '\u{2060}'..='\u{2064}'
        | '\u{2066}'..='\u{2069}'
        | '\u{feff}'
        | '\u{fff9}'..='\u{fffb}'
        | '\u{e0000}'..='\u{e007f}'
    )
}

/// Drop a whole escape sequence starting at `chars[i]` (which must be ESC) and return the index of
/// the first character after it. OSC/DCS/APC-style payloads are removed *including* their payload,
/// so a hyperlink target never survives as plain text.
fn skip_escape(chars: &[char], i: usize) -> usize {
    let n = chars.len();
    let mut j = i + 1; // skip ESC
    if j >= n {
        return n;
    }
    match chars[j] {
        ']' => {
            // OSC: terminated by BEL or ST (ESC \) or end of input.
            j += 1;
            while j < n {
                if chars[j] == '\u{7}' {
                    return j + 1;
                }
                if chars[j] == '\u{1b}' {
                    return (j + 2).min(n);
                }
                j += 1;
            }
            n
        }
        '[' => {
            // CSI: terminated by a byte in 0x40..=0x7E.
            j += 1;
            while j < n {
                let c = chars[j];
                if ('\u{40}'..='\u{7e}').contains(&c) {
                    return j + 1;
                }
                j += 1;
            }
            n
        }
        'P' | 'X' | '^' | '_' => {
            j += 1;
            while j < n {
                if chars[j] == '\u{1b}' {
                    return (j + 2).min(n);
                }
                j += 1;
            }
            n
        }
        _ => j + 1,
    }
}

/// Neutralise terminal control sequences and untrusted line breaks in `input`.
pub fn sanitize(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\u{1b}' {
            i = skip_escape(&chars, i);
            continue;
        }
        match c {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => {
                if !c.is_control() && !is_invisible(c) {
                    out.push(c);
                }
            }
        }
        i += 1;
    }
    out
}

/// Sanitize and truncate to `max_chars` characters of sanitized output, appending `…` on truncation.
pub fn sanitize_bounded(input: &str, max_chars: usize) -> String {
    let clean = sanitize(input);
    if clean.chars().count() <= max_chars {
        return clean;
    }
    let mut out: String = clean.chars().take(max_chars).collect();
    out.push('…');
    out
}

/// Sanitize a JSON Pointer for display (spec §7: "Apply to errors, paths, *pointer displays*, …
/// and all user-derived strings").
///
/// The pointer an engine carries is exact: its segments are object keys taken verbatim from the
/// configuration document, and that exact form is what `explain` matches on. But a key name is
/// attacker-controlled text, so any pointer that reaches a rendering channel — the JSON report, the
/// text report, a diff — must have terminal escape sequences, control characters, bidi controls and
/// untrusted line breaks removed first. Otherwise a configuration file could paint the terminal of
/// whoever reads the report.
pub fn display_pointer(pointer: &str) -> String {
    sanitize(pointer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_drops_control_characters() {
        // The CSI colour sequence is consumed whole, so only the visible text survives.
        let input = "mo\u{1b}[31mdel\u{7}\u{8}\u{0}\u{1f}";
        let out = sanitize(input);
        assert_eq!(out, "model", "controls must be dropped, got {out:?}");
    }

    #[test]
    fn sanitize_drops_bare_c0_and_c1_controls() {
        let out = sanitize("a\u{1}b\u{9c}c");
        assert_eq!(out, "abc");
    }

    #[test]
    fn sanitize_escapes_newlines_and_carriage_returns() {
        let out = sanitize("a\nb\r\nc\td");
        assert_eq!(out, "a\\nb\\r\\nc\\td");
    }

    #[test]
    fn sanitize_removes_osc_hyperlink() {
        // OSC 8 hyperlink: ESC ] 8 ; ; https://evil.example BEL  click text  ESC ] 8 ; ; BEL
        let input = "\u{1b}]8;;https://evil.example\u{7}click\u{1b}]8;;\u{7}";
        let out = sanitize(input);
        assert_eq!(out, "click");
        assert!(!out.contains('\u{1b}'));
        assert!(!out.contains("evil.example"));
    }

    #[test]
    fn sanitize_removes_bidi_and_invisible_format_characters() {
        let input = "safe\u{202e}txt\u{200b}\u{2066}x\u{feff}";
        assert_eq!(sanitize(input), "safetxtx");
    }

    #[test]
    fn sanitize_drops_unicode_line_separators() {
        assert_eq!(sanitize("a\u{2028}b\u{2029}c"), "abc");
    }

    #[test]
    fn sanitize_keeps_ordinary_unicode() {
        let out = sanitize("naïve — 日本語 ✓");
        assert_eq!(out, "naïve — 日本語 ✓");
    }

    #[test]
    fn sanitize_is_idempotent() {
        let once = sanitize("a\n\u{1b}]8;;http://x\u{7}b\u{202e}c\td");
        let twice = sanitize(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn sanitize_bounded_truncates_and_marks() {
        let out = sanitize_bounded("abcdefghij", 4);
        assert_eq!(out, "abcd…");
        let exact = sanitize_bounded("abcd", 4);
        assert_eq!(exact, "abcd");
    }

    #[test]
    fn sanitize_bounded_truncates_after_sanitizing() {
        // '\n' is escaped to the two characters '\' and 'n', so the bound counts escaped output.
        let out = sanitize_bounded("\nabcdefghij", 3);
        assert_eq!(out, "\\na…");
    }
}
