//! RFC 6901 pointer patterns used by the compatibility registry and the redaction policy.
//!
//! Grammar: `/segment/segment/...` where `*` matches exactly one segment and a trailing `**`
//! matches zero or more remaining segments. No other wildcard syntax exists, so a pattern can never
//! accidentally match more than it says.

/// Split a pointer or pattern into its segments (no leading empty element).
fn segments(s: &str) -> Vec<&str> {
    if s.is_empty() {
        return Vec::new();
    }
    s.trim_start_matches('/').split('/').collect()
}

/// Does `pattern` match `pointer`?
pub fn matches(pattern: &str, pointer: &str) -> bool {
    let p = segments(pattern);
    let s = segments(pointer);
    if p.is_empty() {
        return s.is_empty();
    }
    let double_star = p.last() == Some(&"**");
    let head = if double_star {
        &p[..p.len() - 1]
    } else {
        &p[..]
    };
    if double_star {
        if s.len() < head.len() {
            return false;
        }
    } else if s.len() != head.len() {
        return false;
    }
    head.iter()
        .zip(s.iter())
        .all(|(pp, ss)| *pp == "*" || pp == ss)
}

/// True when the pointer itself, or any ancestor of it, matches `pattern`.
pub fn matches_self_or_ancestor(pattern: &str, pointer: &str) -> bool {
    let s = segments(pointer);
    for len in (0..=s.len()).rev() {
        let candidate = if len == 0 {
            String::new()
        } else {
            format!("/{}", s[..len].join("/"))
        };
        if matches(pattern, &candidate) {
            return true;
        }
    }
    false
}

/// How specific a pattern is: more literal segments is more specific; a trailing `**` is less
/// specific than an exact pattern of the same literal count.
pub fn specificity(pattern: &str) -> (usize, usize) {
    let p = segments(pattern);
    let literals = p.iter().filter(|s| **s != "*" && **s != "**").count();
    let exact = usize::from(p.last() != Some(&"**"));
    (literals, exact)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_patterns_match_only_themselves() {
        assert!(matches("/model", "/model"));
        assert!(!matches("/model", "/models"));
        assert!(!matches("/model", "/model/extra"));
        assert!(!matches("/model", "/other"));
    }

    #[test]
    fn single_star_matches_exactly_one_segment() {
        assert!(matches("/mcp/*/enabled", "/mcp/jira/enabled"));
        assert!(!matches("/mcp/*/enabled", "/mcp/jira/extra/enabled"));
        assert!(!matches("/mcp/*/enabled", "/mcp/enabled"));
    }

    #[test]
    fn double_star_matches_zero_or_more_segments() {
        assert!(matches("/permission/**", "/permission/bash"));
        assert!(matches("/permission/**", "/permission/bash/rm -rf *"));
        assert!(matches("/permission/**", "/permission"));
        assert!(!matches("/permission/**", "/permissions"));
        assert!(matches("**", "/anything/at/all"));
        assert!(matches("**", ""));
    }

    #[test]
    fn root_pointer_matches_only_root_patterns() {
        assert!(matches("", ""));
        assert!(!matches("", "/a"));
        assert!(!matches("/a", ""));
    }

    #[test]
    fn specificity_ranks_literal_segments_over_wildcards() {
        assert!(specificity("/mcp/*/enabled") > specificity("/mcp/**"));
        assert!(specificity("/mcp/*/enabled") > specificity("/mcp/*"));
        assert!(specificity("/mcp/*") > specificity("**"));
        assert_eq!(specificity("/a/b/c"), (3, 1));
    }
}
