use crate::ast::syntax::sep_pattern::SepPattern;
use crate::ast::{GenFmt, WplFmt};
use crate::parser::utils::{quot_r_str, quot_str, take_to_end};
use derive_getters::Getters;
use smol_str::SmolStr;
use std::fmt::{Display, Formatter};
use std::marker::PhantomData;
use winnow::combinator::{alt, opt, separated};
use winnow::stream::Range;
use winnow::token::{literal, take_until, take_while};
use wp_parser::Parser;
use wp_parser::WResult;
use wp_parser::symbol::ctx_desc;

const DEFAULT_SEP: &str = " ";
pub trait DefaultSep {
    fn sep_str() -> &'static str;
}
impl DefaultSep for () {
    fn sep_str() -> &'static str {
        DEFAULT_SEP
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Getters)]
pub struct WplSepT<T> {
    prio: usize,
    cur_val: Option<SepEnum>,
    ups_val: Option<SmolStr>,
    infer: bool,
    is_take: bool,
    #[serde(skip)]
    _phant: PhantomData<T>,
}
pub type WplSep = WplSepT<()>;

impl<T> WplSepT<T> {
    pub fn from(value: &WplSep) -> Self {
        WplSepT {
            prio: value.prio,
            cur_val: value.cur_val.clone(),
            ups_val: value.ups_val.clone(),
            infer: value.infer,
            is_take: value.is_take,
            _phant: PhantomData,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SepEnum {
    Str(SmolStr),
    End,
    Whitespace, // Matches space or tab
    Pattern(SepPattern),
}
impl From<&str> for SepEnum {
    fn from(value: &str) -> Self {
        if value == "\\0" || value == "0" {
            SepEnum::End
        } else if value == "\\s" || value == "s" {
            SepEnum::Str(" ".into())
        } else if value == "\\t" || value == "t" {
            SepEnum::Str("\t".into())
        } else if value == "\\S" || value == "S" {
            SepEnum::Whitespace
        } else {
            SepEnum::Str(value.into())
        }
    }
}

impl From<String> for SepEnum {
    fn from(value: String) -> Self {
        Self::from(value.as_str())
    }
}

impl From<SmolStr> for SepEnum {
    fn from(value: SmolStr) -> Self {
        Self::from(value.as_str())
    }
}
impl<T> Default for WplSepT<T> {
    fn default() -> Self {
        Self {
            prio: 1,
            cur_val: None,
            ups_val: None,
            infer: false,
            is_take: true,
            _phant: PhantomData,
        }
    }
}

impl<T: DefaultSep + Clone> WplSepT<T> {
    /// 字段级分隔符（优先级 3），覆盖组级与上游
    pub fn field_sep<S: Into<SmolStr>>(val: S) -> Self {
        Self {
            prio: 3,
            cur_val: Some(SepEnum::from(val.into())),
            ups_val: None,
            infer: false,
            is_take: true,
            _phant: PhantomData,
        }
    }

    pub fn apply_default(&mut self, other: WplSep) {
        if other.prio > self.prio || self.cur_val.is_none() {
            self.prio = other.prio;
            self.cur_val = other.cur_val;
        }
    }
    pub fn set_current<S: Into<SmolStr>>(&mut self, sep: S) {
        self.cur_val = Some(SepEnum::from(sep.into()))
    }
    pub fn is_unset(&self) -> bool {
        self.cur_val().is_none()
    }
    pub fn is_to_end(&self) -> bool {
        if let Some(x) = &self.cur_val {
            *x == SepEnum::End
        } else {
            false
        }
    }
    pub fn override_with(&mut self, other: &WplSep) {
        if other.prio > self.prio {
            self.prio = other.prio;
            self.cur_val = other.cur_val.clone();
        }
    }
    pub fn sep_str(&self) -> &str {
        if let Some(val) = &self.cur_val {
            match val {
                SepEnum::Str(str) => str.as_str(),
                SepEnum::End => "\n",
                SepEnum::Whitespace => " ", // Default to space for display
                SepEnum::Pattern(p) => p.raw(),
            }
        } else {
            T::sep_str()
            //DEFAULT_SEP
        }
    }
    pub fn inherited_sep<S: Into<SmolStr>>(val: S) -> Self {
        Self {
            prio: 1,
            cur_val: Some(SepEnum::from(val.into())),
            ups_val: None,
            infer: false,
            is_take: true,
            ..Default::default()
        }
    }
    pub fn infer_inherited_sep<S: Into<SmolStr>>(val: S) -> Self {
        Self {
            prio: 1,
            cur_val: Some(SepEnum::from(val.into())),
            ups_val: None,
            infer: true,
            is_take: true,
            ..Default::default()
        }
    }
    pub fn infer_group_sep<S: Into<SmolStr>>(val: S) -> Self {
        Self {
            prio: 2,
            cur_val: Some(SepEnum::from(val.into())),
            ups_val: None,
            infer: true,
            is_take: true,
            ..Default::default()
        }
    }
    pub fn infer_clone(&self) -> Self {
        let mut c = self.clone();
        c.infer = true;
        c
    }
    pub fn group_sep<S: Into<SmolStr>>(val: S) -> Self {
        Self {
            prio: 2,
            cur_val: Some(SepEnum::from(val.into())),
            ups_val: None,
            infer: false,
            is_take: true,
            ..Default::default()
        }
    }
    pub fn field_sep_until<S: Into<SmolStr>>(val: S, sec: S, is_take: bool) -> Self {
        Self {
            prio: 3,
            cur_val: Some(SepEnum::from(val.into())),
            ups_val: Some(sec.into()),
            infer: false,
            is_take,
            ..Default::default()
        }
    }
    pub fn infer_field_sep<S: Into<SmolStr>>(val: S) -> Self {
        Self {
            prio: 3,
            cur_val: Some(SepEnum::from(val.into())),
            ups_val: None,
            infer: true,
            is_take: true,
            ..Default::default()
        }
    }
    pub fn field_sep_pattern(pattern: SepPattern) -> Self {
        Self {
            prio: 3,
            cur_val: Some(SepEnum::Pattern(pattern)),
            ups_val: None,
            infer: false,
            is_take: true,
            _phant: PhantomData,
        }
    }
    pub fn is_pattern(&self) -> bool {
        matches!(&self.cur_val, Some(SepEnum::Pattern(_)))
    }

    pub fn consume_sep(&self, input: &mut &str) -> WResult<()> {
        if self.is_take {
            if let Some(SepEnum::Whitespace) = &self.cur_val {
                // For Whitespace, accept either space or tab
                alt((literal(" "), literal("\t")))
                    .context(ctx_desc("take <whitespace>"))
                    .parse_next(input)?;
            } else if let Some(SepEnum::Pattern(pattern)) = &self.cur_val {
                match pattern.match_at_start(input) {
                    Some(m) => {
                        *input = &input[m.consumed..];
                    }
                    None => {
                        winnow::combinator::fail
                            .context(ctx_desc("take <sep pattern>"))
                            .parse_next(input)?;
                    }
                }
            } else {
                literal(self.sep_str())
                    .context(ctx_desc("take <sep>"))
                    .parse_next(input)?;
            }
        }
        Ok(())
    }
    pub fn try_consume_sep(&self, input: &mut &str) -> WResult<()> {
        if self.is_take {
            if let Some(SepEnum::Whitespace) = &self.cur_val {
                // For Whitespace, optionally accept either space or tab
                opt(alt((literal(" "), literal("\t")))).parse_next(input)?;
            } else if let Some(SepEnum::Pattern(pattern)) = &self.cur_val {
                if let Some(m) = pattern.match_at_start(input) {
                    *input = &input[m.consumed..];
                }
            } else {
                opt(literal(self.sep_str())).parse_next(input)?;
            }
        }
        Ok(())
    }
    pub fn is_space_sep(&self) -> bool {
        !self.is_pattern() && self.sep_str() == " "
    }

    pub fn need_take_sep(&self) -> bool {
        !(self.is_to_end() || self.is_space_sep())
    }

    pub fn read_until_any_char<'a>(end1: &str, end2: &str, data: &mut &'a str) -> WResult<&'a str> {
        let ends1 = end1.as_bytes();
        let ends2 = end2.as_bytes();
        alt((
            quot_r_str,
            quot_str,
            take_while(0.., |c: char| {
                !(ends1.contains(&(c as u8)) || ends2.contains(&(c as u8)))
            }),
            take_to_end,
        ))
        .parse_next(data)
    }

    pub fn read_until_sep(&self, data: &mut &str) -> WResult<String> {
        // 读到当前分隔符，若存在"次级结束符"（ups_val），应以"最近结束优先"裁剪。
        // 特殊值：\0 由 is_to_end() 覆盖；单字符对使用 read_until_any_char 快路径。
        if self.is_to_end() {
            let buf = take_to_end.parse_next(data)?;
            return Ok(buf.to_string());
        }

        // Handle Whitespace separator specially
        if let Some(SepEnum::Whitespace) = &self.cur_val {
            // Take until space or tab
            let buf = take_while(0.., |c: char| c != ' ' && c != '\t').parse_next(data)?;
            return Ok(buf.to_string());
        }

        // Handle Pattern separator
        if let Some(SepEnum::Pattern(pattern)) = &self.cur_val {
            let s = *data;
            // Exclude quoted segments (consistent with existing logic)
            if s.starts_with('"') || s.starts_with("r#\"") || s.starts_with("r\"") {
                let buf = alt((quot_r_str, quot_str)).parse_next(data)?;
                return Ok(buf.to_string());
            }
            return match pattern.find(s) {
                Some((offset, sep_match)) => {
                    let content = &s[..offset];
                    *data = &s[offset + sep_match.consumed..];
                    Ok(content.to_string())
                }
                None => Ok(take_to_end.parse_next(data)?.to_string()),
            };
        }

        if let Some(ups) = &self.ups_val {
            // 快路径：单字符对，使用按字符扫描，天然最近结束优先
            if self.sep_str().len() == 1 && ups.len() == 1 {
                let buf = Self::read_until_any_char(self.sep_str(), ups.as_str(), data)?;
                return Ok(buf.to_string());
            }
            // 常规：对多字符分隔的最近结束优先实现
            let s = *data;
            // 若下一个是引号，优先让上层调用流按引号解析；保持与既有行为一致
            // （复杂场景建议使用 json/kv 等协议解析器避免干扰）。
            if s.starts_with('"') || s.starts_with("r#\"") || s.starts_with("r\"") {
                // 引号或原始字符串优先整体解析，避免被错误切分
                let buf = alt((quot_r_str, quot_str)).parse_next(data)?;
                return Ok(buf.to_string());
            }
            let p = s.find(self.sep_str());
            let q = s.find(ups.as_str());
            let idx = match (p, q) {
                (Some(i), Some(j)) => Some(i.min(j)),
                (Some(i), None) => Some(i),
                (None, Some(j)) => Some(j),
                (None, None) => None,
            };
            if let Some(i) = idx {
                let (left, right) = s.split_at(i);
                *data = right; // 保持与 take_until 一致：不消费结束符本身
                return Ok(left.to_string());
            }
            let buf = take_to_end.parse_next(data)?;
            return Ok(buf.to_string());
        }
        // 无次级结束符：原有语义
        let buf = alt((
            quot_r_str,
            quot_str,
            take_until(0.., self.sep_str()),
            take_to_end,
        ))
        .parse_next(data)?;
        Ok(buf.to_string())
    }
    pub fn read_until_sep_repeat(&self, num: usize, data: &mut &str) -> WResult<String> {
        let buffer: Vec<&str> = separated(
            Range::from(num),
            take_until(1.., self.sep_str()),
            self.sep_str(),
        )
        .parse_next(data)?;

        let msg = buffer.join(self.sep_str());
        Ok(msg)
    }
}

impl Display for WplFmt<&WplSep> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if !self.0.infer {
            if let Some(SepEnum::Pattern(p)) = &self.0.cur_val {
                write!(f, "{{{}}}", p.raw())?;
            } else {
                for c in self.0.sep_str().chars() {
                    if c != ' ' {
                        write!(f, "\\{}", c)?;
                    }
                }
            }
        }
        Ok(())
    }
}

impl Display for GenFmt<&WplSep> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(SepEnum::Pattern(p)) = &self.0.cur_val {
            write!(f, "{{{}}}", p.raw())?;
        } else {
            write!(f, "{}", self.0.sep_str())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sep_enum_from_str() {
        // Test \s -> space
        assert_eq!(SepEnum::from("\\s"), SepEnum::Str(" ".into()));
        assert_eq!(SepEnum::from("s"), SepEnum::Str(" ".into()));

        // Test \t -> tab
        assert_eq!(SepEnum::from("\\t"), SepEnum::Str("\t".into()));
        assert_eq!(SepEnum::from("t"), SepEnum::Str("\t".into()));

        // Test \S -> Whitespace
        assert_eq!(SepEnum::from("\\S"), SepEnum::Whitespace);
        assert_eq!(SepEnum::from("S"), SepEnum::Whitespace);

        // Test \0 -> End
        assert_eq!(SepEnum::from("\\0"), SepEnum::End);
        assert_eq!(SepEnum::from("0"), SepEnum::End);

        // Test regular string
        assert_eq!(SepEnum::from(","), SepEnum::Str(",".into()));
    }

    #[test]
    fn test_whitespace_sep_read_until() {
        // Test reading until space
        let mut data = "hello world";
        let sep = WplSep::field_sep("\\S");
        let result = sep.read_until_sep(&mut data).unwrap();
        assert_eq!(result, "hello");
        assert_eq!(data, " world");

        // Test reading until tab
        let mut data = "hello\tworld";
        let sep = WplSep::field_sep("\\S");
        let result = sep.read_until_sep(&mut data).unwrap();
        assert_eq!(result, "hello");
        assert_eq!(data, "\tworld");
    }

    #[test]
    fn test_tab_sep_read_until() {
        // Test reading until tab
        let mut data = "field1\tfield2\tfield3";
        let sep = WplSep::field_sep("\\t");
        let result = sep.read_until_sep(&mut data).unwrap();
        assert_eq!(result, "field1");
        assert_eq!(data, "\tfield2\tfield3");
    }

    #[test]
    fn test_whitespace_consume_sep() {
        // Test consuming space with Whitespace separator
        let mut data = " world";
        let sep = WplSep::field_sep("\\S");
        sep.consume_sep(&mut data).unwrap();
        assert_eq!(data, "world");

        // Test consuming tab with Whitespace separator
        let mut data = "\tworld";
        let sep = WplSep::field_sep("\\S");
        sep.consume_sep(&mut data).unwrap();
        assert_eq!(data, "world");
    }

    #[test]
    fn test_tab_consume_sep() {
        // Test consuming tab
        let mut data = "\tfield2";
        let sep = WplSep::field_sep("\\t");
        sep.consume_sep(&mut data).unwrap();
        assert_eq!(data, "field2");
    }

    // ── Pattern integration tests ────────────────────────────────────

    #[test]
    fn test_pattern_read_until_sep_literal() {
        use crate::ast::syntax::sep_pattern::build_pattern;
        let pat = build_pattern("abc").unwrap();
        let sep = WplSep::field_sep_pattern(pat);
        let mut data = "xyzabcdef";
        let result = sep.read_until_sep(&mut data).unwrap();
        assert_eq!(result, "xyz");
        assert_eq!(data, "def");
    }

    #[test]
    fn test_pattern_read_until_sep_glob() {
        use crate::ast::syntax::sep_pattern::build_pattern;
        let pat = build_pattern("*=").unwrap();
        let sep = WplSep::field_sep_pattern(pat);
        let mut data = "key=value";
        let result = sep.read_until_sep(&mut data).unwrap();
        // Star non-greedy: "*=" at offset 0 matches "k" + "=", so consumed="k=", offset=0
        // Actually: star matches "", then "=" matches "=" at pos 0? No.
        // Pattern "*=" means: Star(non-greedy) + Literal("=")
        // find scans from pos 0: star tries 0 chars → remaining "=" tries to match "key=value" starting with "=" → no
        // star tries 1 char "k" → remaining "=" tries to match "ey=value" starting with "=" → no
        // star tries 2 chars "ke" → remaining "=" tries to match "y=value" starting with "=" → no
        // star tries 3 chars "key" → remaining "=" tries to match "=value" starting with "=" → yes!
        // So consumed = 3 + 1 = 4 ("key="), offset = 0
        assert_eq!(result, "");
        assert_eq!(data, "value");
    }

    #[test]
    fn test_pattern_read_until_sep_no_match() {
        use crate::ast::syntax::sep_pattern::build_pattern;
        let pat = build_pattern("xyz").unwrap();
        let sep = WplSep::field_sep_pattern(pat);
        let mut data = "abcdef";
        let result = sep.read_until_sep(&mut data).unwrap();
        assert_eq!(result, "abcdef");
        assert_eq!(data, "");
    }

    #[test]
    fn test_pattern_consume_sep() {
        use crate::ast::syntax::sep_pattern::build_pattern;
        let pat = build_pattern("\\s=").unwrap();
        let sep = WplSep::field_sep_pattern(pat);
        let mut data = "  =value";
        sep.consume_sep(&mut data).unwrap();
        assert_eq!(data, "value");
    }

    #[test]
    fn test_pattern_try_consume_sep() {
        use crate::ast::syntax::sep_pattern::build_pattern;
        let pat = build_pattern("\\s=").unwrap();
        let sep = WplSep::field_sep_pattern(pat);
        // When it matches
        let mut data = " =value";
        sep.try_consume_sep(&mut data).unwrap();
        assert_eq!(data, "value");
        // When it doesn't match — input unchanged
        let mut data = "value";
        sep.try_consume_sep(&mut data).unwrap();
        assert_eq!(data, "value");
    }

    #[test]
    fn test_pattern_is_pattern() {
        use crate::ast::syntax::sep_pattern::build_pattern;
        let pat = build_pattern("abc").unwrap();
        let sep = WplSep::field_sep_pattern(pat);
        assert!(sep.is_pattern());
        assert!(!sep.is_space_sep());

        let sep2 = WplSep::field_sep(",");
        assert!(!sep2.is_pattern());
    }

    #[test]
    fn test_pattern_display_wpl_fmt() {
        use crate::ast::syntax::sep_pattern::build_pattern;
        let pat = build_pattern("*\\s(key=)").unwrap();
        let sep = WplSep::field_sep_pattern(pat);
        let display = format!("{}", WplFmt(&sep));
        assert_eq!(display, "{*\\s(key=)}");
    }

    #[test]
    fn test_pattern_display_gen_fmt() {
        use crate::ast::syntax::sep_pattern::build_pattern;
        let pat = build_pattern("abc").unwrap();
        let sep = WplSep::field_sep_pattern(pat);
        let display = format!("{}", GenFmt(&sep));
        assert_eq!(display, "{abc}");
    }

    #[test]
    fn test_pattern_serde_roundtrip() {
        use crate::ast::syntax::sep_pattern::build_pattern;
        let pat = build_pattern("*=").unwrap();
        let sep = WplSep::field_sep_pattern(pat);
        let json = serde_json::to_string(&sep).unwrap();
        let sep2: WplSep = serde_json::from_str(&json).unwrap();
        assert_eq!(sep, sep2);
    }

    #[test]
    fn test_pattern_preserve_read_until() {
        use crate::ast::syntax::sep_pattern::build_pattern;
        let pat = build_pattern("*\\s(key=)").unwrap();
        let sep = WplSep::field_sep_pattern(pat);
        let mut data = "hello  key=value";
        let result = sep.read_until_sep(&mut data).unwrap();
        // Star consumes "hello", \s consumes "  ", preserve "key=" not consumed
        assert_eq!(result, "");
        assert_eq!(data, "key=value");
    }
}
