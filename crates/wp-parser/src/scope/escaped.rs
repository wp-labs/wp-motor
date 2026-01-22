/// Evaluator for scopes with escape sequences (e.g., quoted strings)
#[derive(Default)]
pub struct EscapedScopeEval {
    beg: char,
    end: char,
    esc_beg: char,
    esc_end: char,
}

/// State machine for parsing scoped content with escape sequences
enum ScopeParseState {
    /// Waiting for the opening delimiter
    Initial,
    /// Parsing content inside the scope
    Parsing,
    /// Inside an escaped section (e.g., within quotes)
    Escaped,
}
impl EscapedScopeEval {
    #[inline(always)]
    pub fn new(beg: char, end: char, esc_beg: char, esc_end: char) -> Self {
        Self {
            beg,
            end,
            esc_beg,
            esc_end,
        }
    }
    #[inline(always)]
    pub fn len(&self, data: &str) -> usize {
        let mut take_len = 0;
        let mut mode = ScopeParseState::Initial;
        let mut work_level = 0;
        for c in data.chars() {
            match mode {
                ScopeParseState::Initial => {
                    if c == self.beg {
                        mode = ScopeParseState::Parsing;
                        work_level += 1;
                        take_len += 1;
                        continue;
                    } else {
                        break;
                    }
                }
                ScopeParseState::Parsing => {
                    if c == self.end {
                        take_len += 1;
                        work_level -= 1;
                        if work_level == 0 {
                            break;
                        }
                        continue;
                    }
                    if c == self.beg {
                        take_len += 1;
                        work_level += 1;
                        continue;
                    }
                    if c == self.esc_beg {
                        mode = ScopeParseState::Escaped;
                        take_len += 1;
                        continue;
                    }
                    take_len += 1;
                }
                ScopeParseState::Escaped => {
                    if c == self.esc_end {
                        mode = ScopeParseState::Parsing;
                        take_len += 1;
                        continue;
                    }
                    take_len += 1;
                }
            }
        }
        take_len
    }
}

#[cfg(test)]
mod tests {
    use crate::scope::EscapedScopeEval;

    mod escaped_scope {
        use super::*;

        #[test]
        fn valid_escaped_content() {
            let scope_rule = EscapedScopeEval::new('{', '}', '"', '"');
            let data = r#"{ "a" : "} hello {" }"#;
            let size = scope_rule.len(data);
            assert_eq!(size, 21);
        }

        #[test]
        fn valid_simple_scope() {
            let scope_rule = EscapedScopeEval::new('{', '}', '"', '"');
            let data = r#"{ "a" : 123 }"#;
            let size = scope_rule.len(data);
            assert_eq!(size, 13);
        }

        #[test]
        fn valid_first_scope_only() {
            let scope_rule = EscapedScopeEval::new('{', '}', '"', '"');
            let data = r#"{ "a" : 123 } {"b" : 234 }"#;
            let size = scope_rule.len(data);
            assert_eq!(size, 13);
        }

        #[test]
        fn invalid_leading_space() {
            let scope_rule = EscapedScopeEval::new('{', '}', '"', '"');
            let data = r#" { "a" : 123 } {"b" : 234 }"#;
            let size = scope_rule.len(data);
            assert_eq!(size, 0);
        }

        #[test]
        fn valid_deeply_nested() {
            let scope_rule = EscapedScopeEval::new('{', '}', '"', '"');
            let data = r#"{ "a" : 123 , "b": { "x" : { "y" :1 }} }"#;
            let size = scope_rule.len(data);
            assert_eq!(size, 40);
        }
    }

    mod edge_cases {
        use super::*;

        #[test]
        fn empty_scope() {
            let scope_rule = EscapedScopeEval::new('{', '}', '"', '"');
            let data = "{}";
            let size = scope_rule.len(data);
            assert_eq!(size, 2);
        }

        #[test]
        fn empty_input() {
            let scope_rule = EscapedScopeEval::new('{', '}', '"', '"');
            let data = "";
            let size = scope_rule.len(data);
            assert_eq!(size, 0);
        }

        #[test]
        fn escaped_opening_delimiter() {
            let scope_rule = EscapedScopeEval::new('{', '}', '"', '"');
            let data = r#"{ "key": "{value}" }"#;
            let size = scope_rule.len(data);
            assert_eq!(size, 20);
        }

        #[test]
        fn escaped_closing_delimiter() {
            let scope_rule = EscapedScopeEval::new('{', '}', '"', '"');
            let data = r#"{ "key": "val}ue" }"#;
            let size = scope_rule.len(data);
            assert_eq!(size, 19);
        }

        #[test]
        fn multiple_escaped_sections() {
            let scope_rule = EscapedScopeEval::new('{', '}', '"', '"');
            let data = r#"{ "a": "}" , "b": "{" , "c": "}" }"#;
            let size = scope_rule.len(data);
            assert_eq!(size, 34);
        }

        #[test]
        fn unmatched_opening() {
            let scope_rule = EscapedScopeEval::new('{', '}', '"', '"');
            let data = r#"{ "key": "value" "#;
            let size = scope_rule.len(data);
            // Current implementation continues until end of input
            assert_eq!(
                size, 17,
                "Continues to end without finding closing delimiter"
            );
        }

        #[test]
        fn nested_with_escape() {
            let scope_rule = EscapedScopeEval::new('{', '}', '"', '"');
            let data = r#"{ "outer": { "inner": "}" } }"#;
            let size = scope_rule.len(data);
            assert_eq!(size, 29);
        }
    }

    mod different_escape_delimiters {
        use super::*;

        #[test]
        fn single_quote_escape() {
            let scope_rule = EscapedScopeEval::new('{', '}', '\'', '\'');
            let data = r#"{ 'key': 'val}ue' }"#;
            let size = scope_rule.len(data);
            assert_eq!(size, 19);
        }

        #[test]
        fn parentheses_with_string_escape() {
            let scope_rule = EscapedScopeEval::new('(', ')', '"', '"');
            let data = r#"(a ")" b)"#;
            let size = scope_rule.len(data);
            assert_eq!(size, 9);
        }

        #[test]
        fn brackets_with_escape() {
            let scope_rule = EscapedScopeEval::new('[', ']', '"', '"');
            let data = r#"["item]", "other"]"#;
            let size = scope_rule.len(data);
            assert_eq!(size, 18);
        }
    }

    mod boundary_conditions {
        use super::*;

        #[test]
        fn only_escape_chars() {
            let scope_rule = EscapedScopeEval::new('{', '}', '"', '"');
            let data = r#"{ "" }"#;
            let size = scope_rule.len(data);
            assert_eq!(size, 6);
        }

        #[test]
        fn nested_escape_sections() {
            let scope_rule = EscapedScopeEval::new('{', '}', '"', '"');
            let data = r#"{ "a": "\"nested\"" }"#;
            let size = scope_rule.len(data);
            // Note: This tests current behavior, might need different escape handling
            assert_eq!(size, 21);
        }

        #[test]
        fn whitespace_in_escaped() {
            let scope_rule = EscapedScopeEval::new('{', '}', '"', '"');
            let data = r#"{ "  spaces  " }"#;
            let size = scope_rule.len(data);
            assert_eq!(size, 16);
        }

        #[test]
        fn newlines_in_escaped() {
            let scope_rule = EscapedScopeEval::new('{', '}', '"', '"');
            let data = "{ \"line1\nline2\" }";
            let size = scope_rule.len(data);
            assert_eq!(size, 17);
        }
    }
}
