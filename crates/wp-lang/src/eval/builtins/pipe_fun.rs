use crate::ast::WplFun;
use crate::ast::processor::{
    Base64Decode, CharsHas, CharsIn, CharsNotHas, DigitHas, DigitIn, DigitRange, Has, IpIn,
    JsonUnescape, RegexMatch, ReplaceFunc, SelectLast, TakeField, TargetCharsHas, TargetCharsIn,
    TargetCharsNotHas, TargetDigitHas, TargetDigitIn, TargetHas, TargetIpIn,
};
use crate::eval::runtime::field_pipe::{FieldIndex, FieldPipe, FieldSelector, FieldSelectorSpec};
use base64::Engine;
use base64::engine::general_purpose;
use regex::Regex;
use winnow::combinator::fail;
use wp_model_core::model::{DataField, Value};
use wp_parser::symbol::ctx_desc;
use wp_parser::{Parser, WResult};

impl FieldSelector for TakeField {
    fn select(
        &self,
        fields: &mut Vec<DataField>,
        index: Option<&FieldIndex>,
    ) -> WResult<Option<usize>> {
        if let Some(idx) = index.and_then(|map| map.get(self.target.as_str()))
            && idx < fields.len()
        {
            return Ok(Some(idx));
        }
        if let Some(pos) = fields.iter().position(|f| f.get_name() == self.target) {
            Ok(Some(pos))
        } else {
            fail.context(ctx_desc("take | not exists"))
                .parse_next(&mut "")?;
            Ok(None)
        }
    }

    fn requires_index(&self) -> bool {
        true
    }
}

impl FieldSelector for SelectLast {
    fn select(
        &self,
        fields: &mut Vec<DataField>,
        _index: Option<&FieldIndex>,
    ) -> WResult<Option<usize>> {
        if fields.is_empty() {
            fail.context(ctx_desc("last | not exists"))
                .parse_next(&mut "")?;
            Ok(None)
        } else {
            Ok(Some(fields.len() - 1))
        }
    }
}

impl FieldPipe for TargetCharsHas {
    #[inline]
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        if let Some(item) = field
            && let Value::Chars(value) = item.get_value()
            && value.as_str() == self.value.as_str()
        {
            return Ok(());
        }
        fail.context(ctx_desc("<pipe> | not exists"))
            .parse_next(&mut "")
    }

    fn auto_select<'a>(&'a self) -> Option<FieldSelectorSpec<'a>> {
        self.target.as_deref().map(FieldSelectorSpec::Take)
    }
}

impl FieldPipe for CharsHas {
    #[inline]
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        if let Some(item) = field
            && let Value::Chars(value) = item.get_value()
            && value.as_str() == self.value.as_str()
        {
            return Ok(());
        }
        fail.context(ctx_desc("<pipe> | not exists"))
            .parse_next(&mut "")
    }
}

impl FieldPipe for TargetCharsNotHas {
    #[inline]
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        match field {
            None => Ok(()),
            Some(item) => {
                if let Value::Chars(value) = item.get_value()
                    && value.as_str() != self.value.as_str()
                {
                    return Ok(());
                }
                fail.context(ctx_desc("<pipe> | not exists"))
                    .parse_next(&mut "")
            }
        }
    }

    fn auto_select<'a>(&'a self) -> Option<FieldSelectorSpec<'a>> {
        self.target.as_deref().map(FieldSelectorSpec::Take)
    }
}

impl FieldPipe for CharsNotHas {
    #[inline]
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        match field {
            None => Ok(()),
            Some(item) => {
                if let Value::Chars(value) = item.get_value()
                    && value.as_str() != self.value.as_str()
                {
                    return Ok(());
                }
                fail.context(ctx_desc("<pipe> | not exists"))
                    .parse_next(&mut "")
            }
        }
    }
}

impl FieldPipe for TargetCharsIn {
    #[inline]
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        if let Some(item) = field
            && let Value::Chars(value) = item.get_value()
            && self.value.iter().any(|v| v.as_str() == value.as_str())
        {
            return Ok(());
        }
        fail.context(ctx_desc("<pipe> | not in"))
            .parse_next(&mut "")
    }

    fn auto_select<'a>(&'a self) -> Option<FieldSelectorSpec<'a>> {
        self.target.as_deref().map(FieldSelectorSpec::Take)
    }
}

impl FieldPipe for CharsIn {
    #[inline]
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        if let Some(item) = field
            && let Value::Chars(value) = item.get_value()
            && self.value.iter().any(|v| v.as_str() == value.as_str())
        {
            return Ok(());
        }
        fail.context(ctx_desc("<pipe> | not in"))
            .parse_next(&mut "")
    }
}

impl FieldPipe for TargetDigitHas {
    #[inline]
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        if let Some(item) = field
            && let Value::Digit(value) = item.get_value()
            && *value == self.value
        {
            return Ok(());
        }
        fail.context(ctx_desc("<pipe> | not exists"))
            .parse_next(&mut "")
    }

    fn auto_select<'a>(&'a self) -> Option<FieldSelectorSpec<'a>> {
        self.target.as_deref().map(FieldSelectorSpec::Take)
    }
}

impl FieldPipe for DigitHas {
    #[inline]
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        if let Some(item) = field
            && let Value::Digit(value) = item.get_value()
            && *value == self.value
        {
            return Ok(());
        }
        fail.context(ctx_desc("<pipe> | not exists"))
            .parse_next(&mut "")
    }
}

impl FieldPipe for TargetDigitIn {
    #[inline]
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        if let Some(item) = field
            && let Value::Digit(value) = item.get_value()
            && self.value.contains(value)
        {
            return Ok(());
        }
        fail.context(ctx_desc("<pipe> | not in"))
            .parse_next(&mut "")
    }

    fn auto_select<'a>(&'a self) -> Option<FieldSelectorSpec<'a>> {
        self.target.as_deref().map(FieldSelectorSpec::Take)
    }
}

impl FieldPipe for DigitIn {
    #[inline]
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        if let Some(item) = field
            && let Value::Digit(value) = item.get_value()
            && self.value.contains(value)
        {
            return Ok(());
        }
        fail.context(ctx_desc("<pipe> | not in"))
            .parse_next(&mut "")
    }
}

impl FieldPipe for DigitRange {
    #[inline]
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        if let Some(item) = field
            && let Value::Digit(value) = item.get_value()
            && *value >= self.begin
            && *value <= self.end
        {
            return Ok(());
        }
        fail.context(ctx_desc("<pipe> | not in range"))
            .parse_next(&mut "")
    }
}

impl FieldPipe for TargetIpIn {
    #[inline]
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        if let Some(item) = field
            && let Value::IpAddr(value) = item.get_value()
            && self.value.contains(value)
        {
            return Ok(());
        }
        fail.context(ctx_desc("<pipe> | not in"))
            .parse_next(&mut "")
    }

    fn auto_select<'a>(&'a self) -> Option<FieldSelectorSpec<'a>> {
        self.target.as_deref().map(FieldSelectorSpec::Take)
    }
}

impl FieldPipe for IpIn {
    #[inline]
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        if let Some(item) = field
            && let Value::IpAddr(value) = item.get_value()
            && self.value.contains(value)
        {
            return Ok(());
        }
        fail.context(ctx_desc("<pipe> | not in"))
            .parse_next(&mut "")
    }
}

impl FieldPipe for TargetHas {
    #[inline]
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        if field.is_some() {
            return Ok(());
        }
        fail.context(ctx_desc("json not exists sub item"))
            .parse_next(&mut "")
    }

    fn auto_select<'a>(&'a self) -> Option<FieldSelectorSpec<'a>> {
        self.target.as_deref().map(FieldSelectorSpec::Take)
    }
}

impl FieldPipe for Has {
    #[inline]
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        if field.is_some() {
            return Ok(());
        }
        fail.context(ctx_desc("json not exists sub item"))
            .parse_next(&mut "")
    }
}

impl FieldPipe for JsonUnescape {
    #[inline]
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        let Some(field) = field else {
            return fail
                .context(ctx_desc("json_unescape | no active field"))
                .parse_next(&mut "");
        };
        let value = field.get_value_mut();
        if value_json_unescape(value) {
            Ok(())
        } else {
            fail.context(ctx_desc("json_unescape")).parse_next(&mut "")
        }
    }
}

impl FieldPipe for Base64Decode {
    #[inline]
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        let Some(field) = field else {
            return fail
                .context(ctx_desc("base64_decode | no active field"))
                .parse_next(&mut "");
        };
        let value = field.get_value_mut();
        if value_base64_decode(value) {
            Ok(())
        } else {
            fail.context(ctx_desc("base64_decode")).parse_next(&mut "")
        }
    }
}

impl FieldPipe for ReplaceFunc {
    #[inline]
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        let Some(field) = field else {
            return fail
                .context(ctx_desc("chars_replace | no active field"))
                .parse_next(&mut "");
        };
        let value = field.get_value_mut();
        if value_chars_replace(value, &self.target, &self.value) {
            Ok(())
        } else {
            fail.context(ctx_desc("chars_replace")).parse_next(&mut "")
        }
    }
}

impl FieldPipe for RegexMatch {
    #[inline]
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        let Some(field) = field else {
            return fail
                .context(ctx_desc("regex_match | no active field"))
                .parse_next(&mut "");
        };

        // 只处理字符串类型的字段
        if let Value::Chars(value) = field.get_value() {
            // 编译正则表达式
            match Regex::new(self.pattern.as_str()) {
                Ok(re) => {
                    if re.is_match(value.as_str()) {
                        Ok(())
                    } else {
                        fail.context(ctx_desc("regex_match | not matched"))
                            .parse_next(&mut "")
                    }
                }
                Err(_) => fail
                    .context(ctx_desc("regex_match | invalid regex pattern"))
                    .parse_next(&mut ""),
            }
        } else {
            fail.context(ctx_desc("regex_match | field is not a string"))
                .parse_next(&mut "")
        }
    }
}

impl WplFun {
    pub fn as_field_pipe(&self) -> Option<&dyn FieldPipe> {
        match self {
            WplFun::SelectTake(_) | WplFun::SelectLast(_) => None,
            WplFun::TargetCharsHas(fun) => Some(fun),
            WplFun::CharsHas(fun) => Some(fun),
            WplFun::TargetCharsNotHas(fun) => Some(fun),
            WplFun::CharsNotHas(fun) => Some(fun),
            WplFun::TargetCharsIn(fun) => Some(fun),
            WplFun::CharsIn(fun) => Some(fun),
            WplFun::TargetDigitHas(fun) => Some(fun),
            WplFun::DigitHas(fun) => Some(fun),
            WplFun::TargetDigitIn(fun) => Some(fun),
            WplFun::DigitIn(fun) => Some(fun),
            WplFun::DigitRange(fun) => Some(fun),
            WplFun::TargetIpIn(fun) => Some(fun),
            WplFun::IpIn(fun) => Some(fun),
            WplFun::TargetHas(fun) => Some(fun),
            WplFun::Has(fun) => Some(fun),
            WplFun::TransJsonUnescape(fun) => Some(fun),
            WplFun::TransBase64Decode(fun) => Some(fun),
            WplFun::TransCharsReplace(fun) => Some(fun),
            WplFun::RegexMatch(fun) => Some(fun),
        }
    }

    pub fn as_field_selector(&self) -> Option<&dyn FieldSelector> {
        match self {
            WplFun::SelectTake(selector) => Some(selector),
            WplFun::SelectLast(selector) => Some(selector),
            _ => None,
        }
    }

    pub fn auto_selector_spec(&self) -> Option<FieldSelectorSpec<'_>> {
        match self {
            WplFun::TargetCharsHas(fun) => fun.auto_select(),
            WplFun::TargetCharsNotHas(fun) => fun.auto_select(),
            WplFun::TargetCharsIn(fun) => fun.auto_select(),
            WplFun::TargetDigitHas(fun) => fun.auto_select(),
            WplFun::TargetDigitIn(fun) => fun.auto_select(),
            WplFun::TargetIpIn(fun) => fun.auto_select(),
            WplFun::TargetHas(fun) => fun.auto_select(),
            _ => None,
        }
    }

    pub fn requires_index(&self) -> bool {
        if let Some(selector) = self.as_field_selector()
            && selector.requires_index()
        {
            return true;
        }
        if let Some(spec) = self.auto_selector_spec() {
            return spec.requires_index();
        }
        false
    }
}

// ---------------- String Mode ----------------
#[inline]
fn decode_json_escapes(raw: &str) -> Option<String> {
    let quoted = format!("\"{}\"", raw);
    serde_json::from_str::<String>(&quoted).ok()
}

#[inline]
fn value_json_unescape(v: &mut Value) -> bool {
    if let Value::Chars(s) = v {
        if !s.as_bytes().contains(&b'\\') {
            return true;
        }
        if let Some(decoded) = decode_json_escapes(s) {
            *s = decoded.into();
            return true;
        }
    }
    false
}

#[inline]
fn value_base64_decode(v: &mut Value) -> bool {
    match v {
        Value::Chars(s) => {
            if let Ok(decoded) = general_purpose::STANDARD.decode(s.as_bytes())
                && let Ok(vstring) = String::from_utf8(decoded)
            {
                *s = vstring.into();
                return true;
            }
            false
        }
        _ => false,
    }
}

#[inline]
fn value_chars_replace(v: &mut Value, target: &str, replacement: &str) -> bool {
    match v {
        Value::Chars(s) => {
            let replaced = s.replace(target, replacement);
            *s = replaced.into();
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_decode_successfully_rewrites_chars_field() {
        let encoded = general_purpose::STANDARD.encode("hello world");
        let mut fields = vec![DataField::from_chars("payload".to_string(), encoded)];
        Base64Decode {}
            .process(fields.get_mut(0))
            .expect("decode ok");
        if let Value::Chars(s) = fields[0].get_value() {
            assert_eq!(s, "hello world");
        } else {
            panic!("payload should remain chars");
        }
    }

    #[test]
    fn base64_decode_returns_err_on_invalid_payload() {
        let mut fields = vec![DataField::from_chars(
            "payload".to_string(),
            "***".to_string(),
        )];
        assert!(Base64Decode {}.process(fields.get_mut(0)).is_err());
    }

    #[test]
    fn json_unescape_successfully_decodes_chars_field() {
        let mut fields = vec![DataField::from_chars(
            "txt".to_string(),
            r"line1\nline2".to_string(),
        )];
        JsonUnescape {}
            .process(fields.get_mut(0))
            .expect("decode ok");
        if let Value::Chars(s) = fields[0].get_value() {
            assert!(s.contains('\n'));
        } else {
            panic!("txt should stay chars");
        }
    }

    #[test]
    fn json_unescape_returns_err_on_invalid_escape() {
        let mut fields = vec![DataField::from_chars(
            "txt".to_string(),
            r"line1\qline2".to_string(),
        )];
        assert!(JsonUnescape {}.process(fields.get_mut(0)).is_err());
    }

    #[test]
    fn chars_replace_successfully_replaces_substring() {
        let mut fields = vec![DataField::from_chars(
            "message".to_string(),
            "hello world, hello rust".to_string(),
        )];
        ReplaceFunc {
            target: "hello".into(),
            value: "hi".into(),
        }
        .process(fields.get_mut(0))
        .expect("replace ok");
        if let Value::Chars(s) = fields[0].get_value() {
            assert_eq!(s.as_str(), "hi world, hi rust");
        } else {
            panic!("message should remain chars");
        }
    }

    #[test]
    fn chars_replace_handles_empty_target() {
        let mut fields = vec![DataField::from_chars(
            "message".to_string(),
            "test".to_string(),
        )];
        ReplaceFunc {
            target: "".into(),
            value: "_".into(),
        }
        .process(fields.get_mut(0))
        .expect("replace ok");
        if let Value::Chars(s) = fields[0].get_value() {
            // Empty target should insert replacement between each character
            assert_eq!(s.as_str(), "_t_e_s_t_");
        } else {
            panic!("message should remain chars");
        }
    }

    #[test]
    fn chars_replace_returns_err_on_non_chars_field() {
        let mut fields = vec![DataField::from_digit("num".to_string(), 123)];
        assert!(
            ReplaceFunc {
                target: "old".into(),
                value: "new".into(),
            }
            .process(fields.get_mut(0))
            .is_err()
        );
    }

    #[test]
    fn digit_range_successfully_matches_value_in_range() {
        let mut fields = vec![DataField::from_digit("num".to_string(), 5)];
        DigitRange { begin: 1, end: 10 }
            .process(fields.get_mut(0))
            .expect("value 5 should be in range [1,10]");
    }

    #[test]
    fn digit_range_matches_boundary_values() {
        // 测试下界
        let mut fields = vec![DataField::from_digit("num".to_string(), 100)];
        DigitRange {
            begin: 100,
            end: 200,
        }
        .process(fields.get_mut(0))
        .expect("value 100 should match lower boundary");

        // 测试上界
        let mut fields = vec![DataField::from_digit("num".to_string(), 200)];
        DigitRange {
            begin: 100,
            end: 200,
        }
        .process(fields.get_mut(0))
        .expect("value 200 should match upper boundary");
    }

    #[test]
    fn digit_range_returns_err_when_value_out_of_range() {
        let mut fields = vec![DataField::from_digit("num".to_string(), 50)];
        assert!(
            DigitRange { begin: 1, end: 10 }
                .process(fields.get_mut(0))
                .is_err()
        );
    }

    #[test]
    fn digit_range_returns_err_on_non_digit_field() {
        let mut fields = vec![DataField::from_chars(
            "text".to_string(),
            "hello".to_string(),
        )];
        assert!(
            DigitRange { begin: 1, end: 10 }
                .process(fields.get_mut(0))
                .is_err()
        );
    }

    #[test]
    fn digit_range_returns_err_when_field_is_none() {
        assert!(DigitRange { begin: 1, end: 10 }.process(None).is_err());
    }

    #[test]
    fn regex_match_successfully_matches_simple_pattern() {
        let mut fields = vec![DataField::from_chars(
            "email".to_string(),
            "test@example.com".to_string(),
        )];
        RegexMatch {
            pattern: r"^\w+@\w+\.\w+$".into(),
        }
        .process(fields.get_mut(0))
        .expect("email should match pattern");
    }

    #[test]
    fn regex_match_successfully_matches_complex_pattern() {
        let mut fields = vec![DataField::from_chars(
            "log".to_string(),
            "2024-01-15 10:30:45 ERROR Something went wrong".to_string(),
        )];
        RegexMatch {
            pattern: r"\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2} \w+ .+".into(),
        }
        .process(fields.get_mut(0))
        .expect("log should match pattern");
    }

    #[test]
    fn regex_match_returns_err_when_pattern_not_matched() {
        let mut fields = vec![DataField::from_chars(
            "text".to_string(),
            "hello world".to_string(),
        )];
        assert!(
            RegexMatch {
                pattern: r"^\d+$".into(),
            }
            .process(fields.get_mut(0))
            .is_err()
        );
    }

    #[test]
    fn regex_match_returns_err_on_invalid_regex() {
        let mut fields = vec![DataField::from_chars(
            "text".to_string(),
            "hello".to_string(),
        )];
        // 无效的正则表达式：未闭合的括号
        assert!(
            RegexMatch {
                pattern: r"(unclosed".into(),
            }
            .process(fields.get_mut(0))
            .is_err()
        );
    }

    #[test]
    fn regex_match_returns_err_on_non_chars_field() {
        let mut fields = vec![DataField::from_digit("num".to_string(), 123)];
        assert!(
            RegexMatch {
                pattern: r"\d+".into(),
            }
            .process(fields.get_mut(0))
            .is_err()
        );
    }

    #[test]
    fn regex_match_returns_err_when_field_is_none() {
        assert!(
            RegexMatch {
                pattern: r"test".into(),
            }
            .process(None)
            .is_err()
        );
    }

    #[test]
    fn regex_match_works_with_anchors() {
        let mut fields = vec![DataField::from_chars(
            "code".to_string(),
            "ABC123".to_string(),
        )];

        // 完全匹配
        RegexMatch {
            pattern: r"^[A-Z]+\d+$".into(),
        }
        .process(fields.get_mut(0))
        .expect("should match with anchors");

        // 部分匹配
        RegexMatch {
            pattern: r"\d+".into(),
        }
        .process(fields.get_mut(0))
        .expect("should match partial");
    }

    #[test]
    fn regex_match_case_sensitive() {
        let mut fields = vec![DataField::from_chars(
            "text".to_string(),
            "Hello World".to_string(),
        )];

        // 大小写敏感，不匹配
        assert!(
            RegexMatch {
                pattern: r"^hello".into(),
            }
            .process(fields.get_mut(0))
            .is_err()
        );

        // 使用 (?i) 标志进行大小写不敏感匹配
        RegexMatch {
            pattern: r"(?i)^hello".into(),
        }
        .process(fields.get_mut(0))
        .expect("should match with case-insensitive flag");
    }
}
