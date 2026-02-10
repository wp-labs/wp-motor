use serde::{Deserialize, Deserializer, Serialize, Serializer};
use smol_str::SmolStr;

// ── Error constants ──────────────────────────────────────────────────
const ERR_MULTI_STAR: &str = "sep pattern: at most one * allowed";
const ERR_PRESERVE_NOT_END: &str = "sep pattern: (...) must be at the end";
const ERR_STAR_IN_PRESERVE: &str = "sep pattern: * not allowed inside (...)";
const ERR_EMPTY_PATTERN: &str = "sep pattern: empty pattern";

// ── Data structures ──────────────────────────────────────────────────

/// Result of a successful pattern match.
#[derive(Debug, Clone, PartialEq)]
pub struct SepMatch {
    /// Bytes consumed (not including preserve portion).
    pub consumed: usize,
    /// Total bytes matched (including preserve, for debugging).
    pub matched: usize,
}

/// A single segment inside a glob pattern.
#[derive(Debug, Clone, PartialEq)]
pub enum GlobSegment {
    /// Contiguous literal characters.
    Literal(SmolStr),
    /// `*` — zero or more arbitrary characters (non-greedy).
    Star,
    /// `?` — exactly one arbitrary character.
    Any,
    /// `\s` — one or more whitespace characters `[ \t\r\n]+`.
    Whitespace,
    /// `\h` — one or more horizontal whitespace `[ \t]+`.
    HorizontalWhitespace,
}

/// A compiled glob pattern with optional preserve tail.
#[derive(Debug, Clone, PartialEq)]
pub struct GlobPattern {
    pub segments: Vec<GlobSegment>,
    pub preserve: Option<Vec<GlobSegment>>,
}

/// Compiled matcher – either a plain literal or a glob.
#[derive(Debug, Clone, PartialEq)]
pub enum SepMatcher {
    /// Pure literal, use `str::find` (internally memchr / two-way).
    Literal(SmolStr),
    /// Contains wildcards / whitespace macros.
    Glob(GlobPattern),
}

/// A compiled separator pattern built from `{…}` syntax.
#[derive(Debug, Clone, PartialEq)]
pub struct SepPattern {
    pub(crate) raw: SmolStr,
    pub(crate) compiled: SepMatcher,
}

// ── build_pattern parser ─────────────────────────────────────────────

/// Build a `SepPattern` from the raw content inside `{…}`.
pub fn build_pattern(raw: &str) -> Result<SepPattern, String> {
    if raw.is_empty() {
        return Err(ERR_EMPTY_PATTERN.to_string());
    }

    // 1. Separate preserve portion: find un-escaped `(` … `)` at the very end.
    let (main_raw, preserve_raw) = split_preserve(raw)?;

    // 2. Parse main body segments.
    let (segments, star_count) = parse_segments(main_raw, false)?;

    // 3. Parse preserve segments (if any).
    let preserve = if let Some(pr) = preserve_raw {
        let (psegs, pstar) = parse_segments(pr, true)?;
        if pstar > 0 {
            return Err(ERR_STAR_IN_PRESERVE.to_string());
        }
        Some(psegs)
    } else {
        None
    };

    // 4. Validate star count.
    if star_count > 1 {
        return Err(ERR_MULTI_STAR.to_string());
    }

    // 5. Ensure non-empty after parsing.
    if segments.is_empty() && preserve.as_ref().is_none_or(|p| p.is_empty()) {
        return Err(ERR_EMPTY_PATTERN.to_string());
    }

    // 6. Choose matcher.
    let has_wildcard = segments.iter().any(|s| {
        matches!(
            s,
            GlobSegment::Star
                | GlobSegment::Any
                | GlobSegment::Whitespace
                | GlobSegment::HorizontalWhitespace
        )
    });
    let compiled = if !has_wildcard && preserve.is_none() {
        // Pure literal – collapse all Literal segments into one string.
        let lit: String = segments
            .iter()
            .map(|s| match s {
                GlobSegment::Literal(l) => l.as_str(),
                _ => unreachable!(),
            })
            .collect();
        SepMatcher::Literal(SmolStr::from(lit))
    } else {
        SepMatcher::Glob(GlobPattern {
            segments,
            preserve,
        })
    };

    Ok(SepPattern {
        raw: SmolStr::from(raw),
        compiled,
    })
}

/// Split raw pattern into (main, Option<preserve>).
/// `(…)` must be at the very end of the string and un-escaped.
fn split_preserve(raw: &str) -> Result<(&str, Option<&str>), String> {
    let bytes = raw.as_bytes();
    let len = bytes.len();
    if len == 0 || bytes[len - 1] != b')' {
        return Ok((raw, None));
    }
    // Check the `)` is not escaped.
    if is_escaped(bytes, len - 1) {
        return Ok((raw, None));
    }
    // Walk backwards to find matching un-escaped `(`.
    let mut depth = 0i32;
    let mut open_pos = None;
    let mut i = len;
    while i > 0 {
        i -= 1;
        if bytes[i] == b')' && !is_escaped(bytes, i) {
            depth += 1;
        } else if bytes[i] == b'(' && !is_escaped(bytes, i) {
            depth -= 1;
            if depth == 0 {
                open_pos = Some(i);
                break;
            }
        }
    }
    let open = match open_pos {
        Some(p) => p,
        None => return Ok((raw, None)), // unbalanced – treat as literal
    };

    // Validate that `(` is at a valid position (nothing after `)` except end).
    // The `)` is already the last byte, so we only need to check that nothing
    // between the closing `)` position and end is unexpected. Since we matched
    // the *last* `)`, this is already guaranteed.

    // Also validate that there's no un-escaped `(` before `open` that also has
    // a `)` – this would mean `()` is not at the end. Actually, the simplest
    // check: there must be no un-escaped `(` in the main portion.
    let main_part = &raw[..open];
    {
        let mb = main_part.as_bytes();
        for j in 0..mb.len() {
            if mb[j] == b'(' && !is_escaped(mb, j) {
                return Err(ERR_PRESERVE_NOT_END.to_string());
            }
        }
    }

    let preserve_content = &raw[open + 1..len - 1];
    Ok((main_part, Some(preserve_content)))
}

/// Check if byte at `pos` is preceded by an odd number of backslashes.
fn is_escaped(bytes: &[u8], pos: usize) -> bool {
    let mut count = 0usize;
    let mut p = pos;
    while p > 0 {
        p -= 1;
        if bytes[p] == b'\\' {
            count += 1;
        } else {
            break;
        }
    }
    count % 2 == 1
}

/// Parse a segment string into `Vec<GlobSegment>` and count of `*`.
fn parse_segments(s: &str, forbid_star: bool) -> Result<(Vec<GlobSegment>, usize), String> {
    let mut segs = Vec::new();
    let mut lit_buf = String::new();
    let mut star_count = 0usize;
    let bytes = s.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        let b = bytes[i];
        if b == b'\\' && i + 1 < len {
            let next = bytes[i + 1];
            match next {
                b'\\' | b'*' | b'?' | b'{' | b'}' | b'(' | b')' => {
                    lit_buf.push(next as char);
                    i += 2;
                }
                b'0' => {
                    lit_buf.push('\0');
                    i += 2;
                }
                b'n' => {
                    lit_buf.push('\n');
                    i += 2;
                }
                b't' => {
                    lit_buf.push('\t');
                    i += 2;
                }
                b'r' => {
                    lit_buf.push('\r');
                    i += 2;
                }
                b's' => {
                    flush_literal(&mut lit_buf, &mut segs);
                    segs.push(GlobSegment::Whitespace);
                    i += 2;
                }
                b'h' => {
                    flush_literal(&mut lit_buf, &mut segs);
                    segs.push(GlobSegment::HorizontalWhitespace);
                    i += 2;
                }
                _ => {
                    // Unknown escape – treat backslash + char as literal
                    lit_buf.push(b'\\' as char);
                    lit_buf.push(next as char);
                    i += 2;
                }
            }
        } else if b == b'*' {
            if forbid_star {
                return Err(ERR_STAR_IN_PRESERVE.to_string());
            }
            flush_literal(&mut lit_buf, &mut segs);
            segs.push(GlobSegment::Star);
            star_count += 1;
            if star_count > 1 {
                return Err(ERR_MULTI_STAR.to_string());
            }
            i += 1;
        } else if b == b'?' {
            flush_literal(&mut lit_buf, &mut segs);
            segs.push(GlobSegment::Any);
            i += 1;
        } else if b == b'(' || b == b')' {
            // Un-escaped parentheses in main body indicate misplaced preserve
            return Err(ERR_PRESERVE_NOT_END.to_string());
        } else {
            // Regular character – but must handle UTF-8 properly.
            let ch = s[i..].chars().next().unwrap();
            lit_buf.push(ch);
            i += ch.len_utf8();
        }
    }
    flush_literal(&mut lit_buf, &mut segs);
    Ok((segs, star_count))
}

fn flush_literal(buf: &mut String, segs: &mut Vec<GlobSegment>) {
    if !buf.is_empty() {
        segs.push(GlobSegment::Literal(SmolStr::from(buf.as_str())));
        buf.clear();
    }
}

// ── Matching engine ──────────────────────────────────────────────────

impl SepPattern {
    /// Find the first match in `haystack`. Returns `(offset, SepMatch)` where
    /// `offset` is the byte position where the match starts (= field content length).
    pub fn find(&self, haystack: &str) -> Option<(usize, SepMatch)> {
        match &self.compiled {
            SepMatcher::Literal(lit) => {
                let pos = haystack.find(lit.as_str())?;
                Some((
                    pos,
                    SepMatch {
                        consumed: lit.len(),
                        matched: lit.len(),
                    },
                ))
            }
            SepMatcher::Glob(glob) => glob_find(glob, haystack),
        }
    }

    /// Match only at the start of `haystack` (for `consume_sep`).
    pub fn match_at_start(&self, haystack: &str) -> Option<SepMatch> {
        match &self.compiled {
            SepMatcher::Literal(lit) => {
                if haystack.starts_with(lit.as_str()) {
                    Some(SepMatch {
                        consumed: lit.len(),
                        matched: lit.len(),
                    })
                } else {
                    None
                }
            }
            SepMatcher::Glob(glob) => glob_match_at(glob, haystack, 0).map(|total| {
                let main_len = try_match_segments(&glob.segments, haystack).unwrap_or(0);
                let consumed = main_len;
                SepMatch {
                    consumed,
                    matched: total,
                }
            }),
        }
    }

    /// Return the raw pattern string.
    pub fn raw(&self) -> &str {
        self.raw.as_str()
    }
}

/// Find first occurrence of glob pattern in haystack.
fn glob_find(glob: &GlobPattern, haystack: &str) -> Option<(usize, SepMatch)> {
    let segs = &glob.segments;
    if segs.is_empty() {
        // Only preserve – match at position 0 if preserve matches.
        if let Some(preserve) = &glob.preserve {
            let plen = try_match_segments(preserve, haystack)?;
            return Some((
                0,
                SepMatch {
                    consumed: 0,
                    matched: plen,
                },
            ));
        }
        return None;
    }

    // Optimization: if first segment is Star, match at pos 0 (Star expands to scan).
    if matches!(segs.first(), Some(GlobSegment::Star)) {
        if let Some(total) = glob_match_at(glob, haystack, 0) {
            let main_len = try_match_segments(segs, haystack)?;
            return Some((
                0,
                SepMatch {
                    consumed: main_len,
                    matched: total,
                },
            ));
        }
        return None;
    }

    // Optimization: if first segment is Literal, use str::find for fast skip.
    if let Some(GlobSegment::Literal(first_lit)) = segs.first() {
        let lit = first_lit.as_str();
        let mut search_start = 0;
        while search_start <= haystack.len() {
            if let Some(pos) = haystack[search_start..].find(lit) {
                let abs_pos = search_start + pos;
                if let Some(total) = glob_match_at(glob, haystack, abs_pos) {
                    let main_len =
                        try_match_segments(segs, &haystack[abs_pos..]).unwrap_or(0);
                    return Some((
                        abs_pos,
                        SepMatch {
                            consumed: main_len,
                            matched: total,
                        },
                    ));
                }
                // Advance past this position.
                search_start = abs_pos + lit.len();
            } else {
                break;
            }
        }
        return None;
    }

    // General case: scan char by char.
    for (pos, _) in haystack.char_indices() {
        if let Some(total) = glob_match_at(glob, haystack, pos) {
            let main_len =
                try_match_segments(segs, &haystack[pos..]).unwrap_or(0);
            return Some((
                pos,
                SepMatch {
                    consumed: main_len,
                    matched: total,
                },
            ));
        }
    }
    None
}

/// Attempt full match of glob pattern (main + preserve) starting at byte offset `start`.
/// Returns total matched length (main + preserve) or None.
fn glob_match_at(glob: &GlobPattern, haystack: &str, start: usize) -> Option<usize> {
    let s = &haystack[start..];
    let main_len = try_match_segments(&glob.segments, s)?;
    if let Some(preserve) = &glob.preserve {
        let rest = &s[main_len..];
        let plen = try_match_segments(preserve, rest)?;
        Some(main_len + plen)
    } else {
        Some(main_len)
    }
}

/// Try to match segments against the start of `s`. Returns consumed byte count.
fn try_match_segments(segments: &[GlobSegment], s: &str) -> Option<usize> {
    if segments.is_empty() {
        return Some(0);
    }
    match &segments[0] {
        GlobSegment::Literal(lit) => {
            if s.starts_with(lit.as_str()) {
                let rest = &s[lit.len()..];
                let tail = try_match_segments(&segments[1..], rest)?;
                Some(lit.len() + tail)
            } else {
                None
            }
        }
        GlobSegment::Any => {
            let ch = s.chars().next()?;
            let clen = ch.len_utf8();
            let rest = &s[clen..];
            let tail = try_match_segments(&segments[1..], rest)?;
            Some(clen + tail)
        }
        GlobSegment::Whitespace => {
            // Consume 1+ whitespace characters.
            let consumed = consume_whitespace(s);
            if consumed == 0 {
                return None;
            }
            let rest = &s[consumed..];
            let tail = try_match_segments(&segments[1..], rest)?;
            Some(consumed + tail)
        }
        GlobSegment::HorizontalWhitespace => {
            let consumed = consume_horizontal_whitespace(s);
            if consumed == 0 {
                return None;
            }
            let rest = &s[consumed..];
            let tail = try_match_segments(&segments[1..], rest)?;
            Some(consumed + tail)
        }
        GlobSegment::Star => {
            // Non-greedy: try expanding from 0 chars upwards.
            let remaining = &segments[1..];
            let mut char_iter = s.char_indices();
            // Try matching 0 chars consumed by Star.
            if let Some(tail) = try_match_segments(remaining, s) {
                return Some(tail);
            }
            // Expand one char at a time.
            while let Some((_, ch)) = char_iter.next() {
                let byte_pos = char_iter
                    .clone()
                    .next()
                    .map(|(p, _)| p)
                    .unwrap_or(s.len());
                // byte_pos points to start of next char (or end).
                // But we need to account for the current char's UTF-8 length:
                let after = &s[byte_pos..];
                if let Some(tail) = try_match_segments(remaining, after) {
                    return Some(byte_pos + tail);
                }
                // Don't expand past string.
                let _ = ch;
            }
            None
        }
    }
}

fn consume_whitespace(s: &str) -> usize {
    let mut n = 0;
    for ch in s.chars() {
        if ch == ' ' || ch == '\t' || ch == '\r' || ch == '\n' {
            n += ch.len_utf8();
        } else {
            break;
        }
    }
    n
}

fn consume_horizontal_whitespace(s: &str) -> usize {
    let mut n = 0;
    for ch in s.chars() {
        if ch == ' ' || ch == '\t' {
            n += ch.len_utf8();
        } else {
            break;
        }
    }
    n
}

// ── Serde ────────────────────────────────────────────────────────────

impl Serialize for SepPattern {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.raw.as_str())
    }
}

impl<'de> Deserialize<'de> for SepPattern {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        build_pattern(&s).map_err(serde::de::Error::custom)
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── build_pattern parsing ────────────────────────────────────────

    #[test]
    fn test_parse_literal() {
        let p = build_pattern("abc").unwrap();
        assert_eq!(p.compiled, SepMatcher::Literal("abc".into()));
    }

    #[test]
    fn test_parse_literal_with_newline() {
        let p = build_pattern("ab\\n").unwrap();
        assert_eq!(p.compiled, SepMatcher::Literal("ab\n".into()));
    }

    #[test]
    fn test_parse_literal_with_null() {
        let p = build_pattern("ab\\0").unwrap();
        assert_eq!(p.compiled, SepMatcher::Literal("ab\0".into()));
    }

    #[test]
    fn test_parse_literal_with_tab() {
        let p = build_pattern("ab\\t").unwrap();
        assert_eq!(p.compiled, SepMatcher::Literal("ab\t".into()));
    }

    #[test]
    fn test_parse_literal_with_cr() {
        let p = build_pattern("ab\\r").unwrap();
        assert_eq!(p.compiled, SepMatcher::Literal("ab\r".into()));
    }

    #[test]
    fn test_parse_escaped_chars() {
        let p = build_pattern("a\\*b\\?c").unwrap();
        assert_eq!(p.compiled, SepMatcher::Literal("a*b?c".into()));
    }

    #[test]
    fn test_parse_escaped_braces() {
        let p = build_pattern("a\\{b\\}c").unwrap();
        assert_eq!(p.compiled, SepMatcher::Literal("a{b}c".into()));
    }

    #[test]
    fn test_parse_escaped_parens() {
        let p = build_pattern("a\\(b\\)").unwrap();
        assert_eq!(p.compiled, SepMatcher::Literal("a(b)".into()));
    }

    #[test]
    fn test_parse_glob_star_eq() {
        let p = build_pattern("*=").unwrap();
        match &p.compiled {
            SepMatcher::Glob(g) => {
                assert_eq!(g.segments.len(), 2);
                assert_eq!(g.segments[0], GlobSegment::Star);
                assert_eq!(g.segments[1], GlobSegment::Literal("=".into()));
                assert!(g.preserve.is_none());
            }
            _ => panic!("expected Glob"),
        }
    }

    #[test]
    fn test_parse_glob_key_star() {
        let p = build_pattern("key=*").unwrap();
        match &p.compiled {
            SepMatcher::Glob(g) => {
                assert_eq!(g.segments.len(), 2);
                assert_eq!(g.segments[0], GlobSegment::Literal("key=".into()));
                assert_eq!(g.segments[1], GlobSegment::Star);
            }
            _ => panic!("expected Glob"),
        }
    }

    #[test]
    fn test_parse_glob_field_any() {
        let p = build_pattern("field?:").unwrap();
        match &p.compiled {
            SepMatcher::Glob(g) => {
                assert_eq!(g.segments.len(), 3);
                assert_eq!(g.segments[0], GlobSegment::Literal("field".into()));
                assert_eq!(g.segments[1], GlobSegment::Any);
                assert_eq!(g.segments[2], GlobSegment::Literal(":".into()));
            }
            _ => panic!("expected Glob"),
        }
    }

    #[test]
    fn test_parse_whitespace() {
        let p = build_pattern("\\s=").unwrap();
        match &p.compiled {
            SepMatcher::Glob(g) => {
                assert_eq!(g.segments.len(), 2);
                assert_eq!(g.segments[0], GlobSegment::Whitespace);
                assert_eq!(g.segments[1], GlobSegment::Literal("=".into()));
            }
            _ => panic!("expected Glob"),
        }
    }

    #[test]
    fn test_parse_horizontal_whitespace() {
        let p = build_pattern("\\h:\\h").unwrap();
        match &p.compiled {
            SepMatcher::Glob(g) => {
                assert_eq!(g.segments.len(), 3);
                assert_eq!(g.segments[0], GlobSegment::HorizontalWhitespace);
                assert_eq!(g.segments[1], GlobSegment::Literal(":".into()));
                assert_eq!(g.segments[2], GlobSegment::HorizontalWhitespace);
            }
            _ => panic!("expected Glob"),
        }
    }

    #[test]
    fn test_parse_preserve() {
        let p = build_pattern("*(key=)").unwrap();
        match &p.compiled {
            SepMatcher::Glob(g) => {
                assert_eq!(g.segments, vec![GlobSegment::Star]);
                let preserve = g.preserve.as_ref().unwrap();
                assert_eq!(preserve.len(), 1);
                assert_eq!(preserve[0], GlobSegment::Literal("key=".into()));
            }
            _ => panic!("expected Glob"),
        }
    }

    #[test]
    fn test_parse_preserve_with_whitespace() {
        let p = build_pattern("*\\s(next)").unwrap();
        match &p.compiled {
            SepMatcher::Glob(g) => {
                assert_eq!(
                    g.segments,
                    vec![GlobSegment::Star, GlobSegment::Whitespace]
                );
                let preserve = g.preserve.as_ref().unwrap();
                assert_eq!(preserve.len(), 1);
                assert_eq!(preserve[0], GlobSegment::Literal("next".into()));
            }
            _ => panic!("expected Glob"),
        }
    }

    // ── Constraint violations ────────────────────────────────────────

    #[test]
    fn test_err_multi_star() {
        let e = build_pattern("*a*").unwrap_err();
        assert_eq!(e, ERR_MULTI_STAR);
    }

    #[test]
    fn test_err_preserve_not_end() {
        let e = build_pattern("(key)*=").unwrap_err();
        assert_eq!(e, ERR_PRESERVE_NOT_END);
    }

    #[test]
    fn test_err_star_in_preserve() {
        let e = build_pattern("*(key*)").unwrap_err();
        assert_eq!(e, ERR_STAR_IN_PRESERVE);
    }

    #[test]
    fn test_err_empty() {
        let e = build_pattern("").unwrap_err();
        assert_eq!(e, ERR_EMPTY_PATTERN);
    }

    // ── Matching ─────────────────────────────────────────────────────

    #[test]
    fn test_match_literal() {
        let p = build_pattern("abc").unwrap();
        let (off, m) = p.find("xyzabcdef").unwrap();
        assert_eq!(off, 3);
        assert_eq!(m.consumed, 3);
        assert_eq!(m.matched, 3);
    }

    #[test]
    fn test_match_literal_no_match() {
        let p = build_pattern("abc").unwrap();
        assert!(p.find("xyzdef").is_none());
    }

    #[test]
    fn test_match_star_eq_non_greedy() {
        // `{*=}` on "a=b=c" → non-greedy: offset=0, match "a=", consumed=2
        let p = build_pattern("*=").unwrap();
        let (off, m) = p.find("a=b=c").unwrap();
        assert_eq!(off, 0);
        assert_eq!(m.consumed, 2);
        assert_eq!(m.matched, 2);
    }

    #[test]
    fn test_match_whitespace_eq() {
        // `{\s=}` on "key  =val" → offset=3, consumed=3 (" " " " "=")
        let p = build_pattern("\\s=").unwrap();
        let (off, m) = p.find("key  =val").unwrap();
        assert_eq!(off, 3);
        assert_eq!(m.consumed, 3);
        assert_eq!(m.matched, 3);
    }

    #[test]
    fn test_match_preserve() {
        // `{*\s(key=)}` on "hello  key=value"
        // Star matches "hello", \s matches "  ", preserve matches "key="
        // consumed = 5 + 2 = 7 ("hello  ")
        // matched = 7 + 4 = 11 ("hello  key=")
        let p = build_pattern("*\\s(key=)").unwrap();
        let (off, m) = p.find("hello  key=value").unwrap();
        assert_eq!(off, 0);
        assert_eq!(m.consumed, 7);
        assert_eq!(m.matched, 11);
    }

    #[test]
    fn test_match_field_any() {
        // `{field?:}` on "fieldA:value" → offset=0, consumed=7
        let p = build_pattern("field?:").unwrap();
        let (off, m) = p.find("fieldA:value").unwrap();
        assert_eq!(off, 0);
        assert_eq!(m.consumed, 7);
        assert_eq!(m.matched, 7);
    }

    #[test]
    fn test_match_horizontal_whitespace() {
        // `{\h:\h}` on "key\t:\tval" → offset=3, consumed=3
        let p = build_pattern("\\h:\\h").unwrap();
        let (off, m) = p.find("key\t:\tval").unwrap();
        assert_eq!(off, 3);
        assert_eq!(m.consumed, 3);
        assert_eq!(m.matched, 3);
    }

    #[test]
    fn test_match_no_match() {
        let p = build_pattern("\\s=").unwrap();
        assert!(p.find("key=val").is_none());
    }

    #[test]
    fn test_match_at_start_literal() {
        let p = build_pattern("abc").unwrap();
        let m = p.match_at_start("abcdef").unwrap();
        assert_eq!(m.consumed, 3);
        assert!(p.match_at_start("xabc").is_none());
    }

    #[test]
    fn test_match_at_start_glob() {
        let p = build_pattern("\\s=").unwrap();
        let m = p.match_at_start("  =val").unwrap();
        assert_eq!(m.consumed, 3);
        assert!(p.match_at_start("val  =").is_none());
    }

    #[test]
    fn test_match_star_at_end() {
        // `{key=*}` on "key=value" → offset=0, consumed=9
        let p = build_pattern("key=*").unwrap();
        let (off, m) = p.find("key=value").unwrap();
        assert_eq!(off, 0);
        // Star matches "value" (all remaining since no following segment)
        // But non-greedy star with no remaining segments matches 0 chars
        // Actually, non-greedy star with no remaining segments: try 0 first → succeeds
        assert_eq!(m.consumed, 4); // "key=" + 0 chars from Star
        assert_eq!(m.matched, 4);
    }

    #[test]
    fn test_match_star_newline() {
        // `{\s=*\n}` on "  =hello\n"
        let p = build_pattern("\\s=*\\n").unwrap();
        let (off, m) = p.find("  =hello\n").unwrap();
        assert_eq!(off, 0);
        assert_eq!(m.consumed, 9);
    }

    #[test]
    fn test_match_preserve_only() {
        // Pattern with only preserve: `(abc)` applied to "abcdef"
        let p = build_pattern("(abc)").unwrap();
        match &p.compiled {
            SepMatcher::Glob(g) => {
                assert!(g.segments.is_empty());
                assert!(g.preserve.is_some());
            }
            _ => panic!("expected Glob"),
        }
        let (off, m) = p.find("abcdef").unwrap();
        assert_eq!(off, 0);
        assert_eq!(m.consumed, 0);
        assert_eq!(m.matched, 3);
    }

    // ── Serde round-trip ─────────────────────────────────────────────

    #[test]
    fn test_serde_roundtrip() {
        let p = build_pattern("*\\s(key=)").unwrap();
        let json = serde_json::to_string(&p).unwrap();
        // JSON escapes the backslash: raw `*\s(key=)` → JSON `"*\\s(key=)"`
        assert_eq!(json, r#""*\\s(key=)""#);
        let p2: SepPattern = serde_json::from_str(&json).unwrap();
        assert_eq!(p.raw, p2.raw);
        assert_eq!(p.compiled, p2.compiled);
    }

    #[test]
    fn test_serde_roundtrip_literal() {
        let p = build_pattern("abc").unwrap();
        let json = serde_json::to_string(&p).unwrap();
        let p2: SepPattern = serde_json::from_str(&json).unwrap();
        assert_eq!(p, p2);
    }
}
