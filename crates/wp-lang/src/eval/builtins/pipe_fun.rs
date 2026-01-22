use crate::ast::WplFun;
use crate::ast::processor::{
    Base64Decode, CharsHas, CharsIn, CharsNotHas, DigitHas, DigitIn, Has, IpIn, JsonUnescape,
    SelectLast, TakeField, TargetCharsHas, TargetCharsIn, TargetCharsNotHas, TargetDigitHas,
    TargetDigitIn, TargetHas, TargetIpIn,
};
use crate::eval::runtime::field_pipe::{FieldIndex, FieldPipe, FieldSelector, FieldSelectorSpec};
use base64::Engine;
use base64::engine::general_purpose;
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
            WplFun::TargetIpIn(fun) => Some(fun),
            WplFun::IpIn(fun) => Some(fun),
            WplFun::TargetHas(fun) => Some(fun),
            WplFun::Has(fun) => Some(fun),
            WplFun::TransJsonUnescape(fun) => Some(fun),
            WplFun::TransBase64Decode(fun) => Some(fun),
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
}
