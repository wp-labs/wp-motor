/// Evaluator for basic nested scopes with balanced delimiters
#[derive(Default)]
pub struct ScopeEval {
    count: i32,
    beg: char,
    end: char,
    in_scope: bool,
    end_last: bool,
}
impl ScopeEval {
    #[inline(always)]
    pub fn new(beg: char, end: char) -> Self {
        ScopeEval {
            count: 0,
            beg,
            end,
            in_scope: false,
            end_last: false,
        }
    }
    #[inline(always)]
    pub fn in_scope(&mut self, i: char) -> bool {
        if self.end_last {
            self.end_last = false;
            self.in_scope = false;
        }
        if self.in_scope {
            if i == self.end {
                self.count -= 1;
                if self.count == 0 {
                    self.end_last = true;
                }
            } else if i == self.beg {
                self.count += 1;
            }
        } else if i == self.beg {
            self.count += 1;
            self.in_scope = true;
        }
        self.in_scope
    }
    #[inline(always)]
    pub fn len(data: &str, beg: char, end: char) -> usize {
        let mut op = ScopeEval::new(beg, end);
        let mut len_size: usize = 0;
        for x in data.chars() {
            if op.in_scope(x) {
                len_size += 1;
            } else {
                break;
            }
        }
        len_size
    }
}

#[cfg(test)]
mod tests {
    use super::ScopeEval;

    mod basic_scope {
        use super::*;

        #[test]
        fn valid_simple_scope() {
            let data = r#"(hello)"#;
            let size = ScopeEval::len(data, '(', ')');
            assert_eq!(size, 7);
        }

        #[test]
        fn invalid_no_opening() {
            let data = r#"what(hello)"#;
            let size = ScopeEval::len(data, '(', ')');
            assert_eq!(size, 0);
        }

        #[test]
        fn valid_nested_scope() {
            let data = r#"(what(hello))"#;
            let size = ScopeEval::len(data, '(', ')');
            assert_eq!(size, 13);
        }

        #[test]
        fn valid_complex_nested() {
            let data = r#"(ip(10.0.0.1), ip(10.0.0.10)) => crate(city1) ;
ip(10.0.10.1)  => crate(city2) ;
_  => chars(bj) ;
"#;
            let size = ScopeEval::len(data, '(', ')');
            assert_eq!(size, 29);
        }
    }

    mod edge_cases {
        use super::*;

        #[test]
        fn empty_scope() {
            let data = "()";
            let size = ScopeEval::len(data, '(', ')');
            assert_eq!(size, 2);
        }

        #[test]
        fn empty_input() {
            let data = "";
            let size = ScopeEval::len(data, '(', ')');
            assert_eq!(size, 0);
        }

        #[test]
        fn only_opening() {
            let data = "(((";
            let size = ScopeEval::len(data, '(', ')');
            // Current implementation doesn't match all closings, returns partial count
            assert_eq!(size, 3, "Counts opening but no closing found");
        }

        #[test]
        fn only_closing() {
            let data = ")))";
            let size = ScopeEval::len(data, '(', ')');
            assert_eq!(size, 0);
        }

        #[test]
        fn partial_match() {
            let data = "(hello)(world)";
            let size = ScopeEval::len(data, '(', ')');
            // Current implementation matches both scopes consecutively
            assert_eq!(size, 14, "Matches both adjacent scopes");
        }

        #[test]
        fn mismatched_nesting() {
            let data = "((hello)";
            let size = ScopeEval::len(data, '(', ')');
            // Current implementation returns partial match
            assert_eq!(size, 8, "Returns length including unmatched bracket");
        }

        #[test]
        fn deeply_nested() {
            let data = "((((inner))))";
            let size = ScopeEval::len(data, '(', ')');
            assert_eq!(size, 13);
        }
    }

    mod different_delimiters {
        use super::*;

        #[test]
        fn curly_braces() {
            let data = "{key: value}";
            let size = ScopeEval::len(data, '{', '}');
            assert_eq!(size, 12);
        }

        #[test]
        fn square_brackets() {
            let data = "[1, 2, 3]";
            let size = ScopeEval::len(data, '[', ']');
            assert_eq!(size, 9);
        }

        #[test]
        fn angle_brackets() {
            let data = "<generic>";
            let size = ScopeEval::len(data, '<', '>');
            assert_eq!(size, 9);
        }

        #[test]
        fn nested_curly_braces() {
            let data = "{outer: {inner: value}}";
            let size = ScopeEval::len(data, '{', '}');
            assert_eq!(size, 23);
        }
    }

    mod boundary_conditions {
        use super::*;

        #[test]
        fn single_char_content() {
            let data = "(x)";
            let size = ScopeEval::len(data, '(', ')');
            assert_eq!(size, 3);
        }

        #[test]
        fn whitespace_only_content() {
            let data = "(   )";
            let size = ScopeEval::len(data, '(', ')');
            assert_eq!(size, 5);
        }

        #[test]
        fn newlines_in_content() {
            let data = "(line1\nline2\nline3)";
            let size = ScopeEval::len(data, '(', ')');
            assert_eq!(size, 19);
        }

        #[test]
        fn special_chars_in_content() {
            let data = "(!@#$%^&*)";
            let size = ScopeEval::len(data, '(', ')');
            assert_eq!(size, 10);
        }
    }
}
