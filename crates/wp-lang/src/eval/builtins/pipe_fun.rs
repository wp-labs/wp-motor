use crate::ast::WplFun;
use crate::ast::processor::{
    Base64Decode, CharsHas, CharsIn, CharsNotHas, DigitHas, DigitIn, ExtPassFunc, Has, IpIn,
    JsonUnescape, SelectLast, SendtoSrcFunc, SplitInnerSrcFunc, TakeField, TargetCharsHas,
    TargetCharsIn, TargetCharsNotHas, TargetDigitHas, TargetDigitIn, TargetHas, TargetIpIn,
};
use crate::eval::runtime::field_pipe::{FieldIndex, FieldPipe, FieldSelector, FieldSelectorSpec};
use base64::Engine;
use base64::engine::general_purpose;
use winnow::combinator::fail;
use wp_model_core::model::{DataField, DataType, IgnoreT, Value};
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

impl ExtPassFunc {
    fn process_with_label(
        &self,
        field: Option<&mut DataField>,
        label: &'static str,
    ) -> WResult<()> {
        if let Err(_msg) = self.processor.process(field) {
            return fail.context(ctx_desc(label)).parse_next(&mut "");
        }
        Ok(())
    }
}

impl SendtoSrcFunc {
    fn process_with_label(
        &self,
        mut field: Option<&mut DataField>,
        label: &'static str,
    ) -> WResult<()> {
        if let Some(active) = field.as_deref_mut() {
            let mut dispatched = false;
            match active.get_value_mut() {
                Value::Array(items) => {
                    for item in items.iter_mut() {
                        if let Err(_msg) = self.processor.process(Some(item)) {
                            return fail.context(ctx_desc(label)).parse_next(&mut "");
                        }
                    }
                    dispatched = true;
                }
                Value::Chars(_) => {
                    if let Err(_msg) = self.processor.process(Some(active)) {
                        return fail.context(ctx_desc(label)).parse_next(&mut "");
                    }
                    dispatched = true;
                }
                _ => {}
            }
            if dispatched {
                active.meta = DataType::Ignore;
                active.value = Value::Ignore(IgnoreT::default());
                return Ok(());
            }
        }
        if let Err(_msg) = self.processor.process(field) {
            return fail.context(ctx_desc(label)).parse_next(&mut "");
        }
        Ok(())
    }
}

impl SplitInnerSrcFunc {
    fn process_with_label(
        &self,
        mut field: Option<&mut DataField>,
        label: &'static str,
    ) -> WResult<()> {
        let proc = self.processor();
        if let Some(active) = field.as_deref_mut() {
            let field_name = active.get_name().to_string();
            if let Value::Chars(value) = active.get_value_mut() {
                let separator = self.separator();
                if !separator.is_empty() {
                    let contents = value.to_string();
                    for segment in contents.split(separator.as_str()) {
                        if segment.is_empty() {
                            continue;
                        }
                        let mut seg_field =
                            DataField::from_chars(field_name.clone(), segment.to_string());
                        if let Err(_msg) = proc.process(Some(&mut seg_field)) {
                            return fail.context(ctx_desc(label)).parse_next(&mut "");
                        }
                    }
                    return Ok(());
                }
            }
        }
        if let Err(_msg) = proc.process(field) {
            return fail.context(ctx_desc(label)).parse_next(&mut "");
        }
        Ok(())
    }
}

impl FieldPipe for ExtPassFunc {
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        self.process_with_label(field, "to_ext_pass | processor failed")
    }
}

impl FieldPipe for SendtoSrcFunc {
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        self.process_with_label(field, "send_to_src | processor failed")
    }
}

impl FieldPipe for SplitInnerSrcFunc {
    fn process(&self, field: Option<&mut DataField>) -> WResult<()> {
        self.process_with_label(field, "split_to_src | processor failed")
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
            WplFun::VecToSrc(fun) => Some(fun),
            WplFun::TransExtPass(fun) => Some(fun),
            WplFun::SplitToSrc(fun) => Some(fun),
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
    use crate::eval::runtime::field_pipe::FieldPipe;
    use crate::traits::{
        FieldProcessor, FiledExtendType, clear_field_processors, register_field_processor,
    };
    use once_cell::sync::Lazy;
    use std::sync::Mutex;

    static REG_GUARD: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
    static INNER_BUFFER: Lazy<Mutex<Vec<String>>> = Lazy::new(|| Mutex::new(Vec::new()));

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

    struct AppendExtra;

    impl FieldProcessor for AppendExtra {
        fn name(&self) -> &'static str {
            "append_extra"
        }

        fn process(&self, field: Option<&mut DataField>) -> Result<(), String> {
            if let Some(field) = field {
                if let Some(chars) = field.get_chars() {
                    let updated = format!("{}|processed", chars);
                    field.value = Value::from(updated.as_str());
                }
            }
            Ok(())
        }
    }

    struct InnerAppend;

    impl FieldProcessor for InnerAppend {
        fn name(&self) -> &'static str {
            "inner_append"
        }

        fn process(&self, field: Option<&mut DataField>) -> Result<(), String> {
            let mut buf = INNER_BUFFER.lock().expect("inner buffer");
            if let Some(field) = field {
                if let Some(chars) = field.get_chars() {
                    buf.push(chars.to_string());
                    return Ok(());
                }
            }
            buf.push("payload".to_string());
            Ok(())
        }
    }

    #[test]
    fn ext_pass_executes_registered_processor_and_collects_extra_fields() {
        let _lock = REG_GUARD.lock().unwrap();
        clear_field_processors();
        register_field_processor(FiledExtendType::MemChannel, AppendExtra);

        let func =
            ExtPassFunc::from_registry(FiledExtendType::MemChannel).expect("processor registered");
        let mut field = DataField::from_chars("msg".to_string(), "body".to_string());
        FieldPipe::process(&func, Some(&mut field)).expect("processor runs");
        assert_eq!(field.get_chars(), Some("body|processed"));

        clear_field_processors();
    }

    #[test]
    fn ext_pass_is_none_if_processor_missing() {
        let _lock = REG_GUARD.lock().unwrap();
        clear_field_processors();
        assert!(ExtPassFunc::from_registry(FiledExtendType::MemChannel).is_none());
    }

    #[test]
    fn split_to_src_executes_registered_processor() {
        let _lock = REG_GUARD.lock().unwrap();
        clear_field_processors();
        register_field_processor(FiledExtendType::InnerSource, InnerAppend);
        INNER_BUFFER.lock().unwrap().clear();

        let func = SplitInnerSrcFunc::from_registry("|".into()).expect("inner processor");
        let mut field = DataField::from_chars("msg".to_string(), "alpha|beta|".to_string());
        FieldPipe::process(&func, Some(&mut field)).expect("processor runs");
        let buf = INNER_BUFFER.lock().unwrap().clone();
        assert_eq!(buf, vec!["alpha".to_string(), "beta".to_string()]);

        INNER_BUFFER.lock().unwrap().clear();
        FieldPipe::process(&func, None).expect("processor runs without field");
        let buf = INNER_BUFFER.lock().unwrap().clone();
        assert_eq!(buf, vec!["payload".to_string()]);

        clear_field_processors();
    }

    #[test]
    fn send_to_src_executes_registered_processor() {
        let _lock = REG_GUARD.lock().unwrap();
        clear_field_processors();
        register_field_processor(FiledExtendType::InnerSource, InnerAppend);
        INNER_BUFFER.lock().unwrap().clear();

        let func = SendtoSrcFunc::from_registry(FiledExtendType::InnerSource)
            .expect("processor registered");
        FieldPipe::process(&func, None).expect("processor runs");
        let buf = INNER_BUFFER.lock().unwrap().clone();
        assert_eq!(buf, vec!["payload".to_string()]);

        clear_field_processors();
    }

    #[test]
    fn send_to_src_marks_chars_field_as_ignore_after_dispatch() {
        let _lock = REG_GUARD.lock().unwrap();
        clear_field_processors();
        register_field_processor(FiledExtendType::InnerSource, InnerAppend);
        INNER_BUFFER.lock().unwrap().clear();

        let func = SendtoSrcFunc::from_registry(FiledExtendType::InnerSource)
            .expect("processor registered");
        let mut field = DataField::from_chars("msg".to_string(), "alpha".to_string());
        FieldPipe::process(&func, Some(&mut field)).expect("processor runs");
        let buf = INNER_BUFFER.lock().unwrap().clone();
        assert_eq!(buf, vec!["alpha".to_string()]);
        assert_eq!(field.get_meta(), &DataType::Ignore);
        assert!(matches!(field.get_value(), Value::Ignore(_)));

        clear_field_processors();
    }

    struct CollectChars;

    impl FieldProcessor for CollectChars {
        fn name(&self) -> &'static str {
            "collect_chars"
        }

        fn process(&self, field: Option<&mut DataField>) -> Result<(), String> {
            if let Some(field) = field {
                if let Some(chars) = field.get_chars() {
                    INNER_BUFFER.lock().unwrap().push(chars.to_string());
                }
            }
            Ok(())
        }
    }

    #[test]
    fn send_to_src_processes_each_array_element() {
        let _lock = REG_GUARD.lock().unwrap();
        clear_field_processors();
        register_field_processor(FiledExtendType::InnerSource, CollectChars);
        INNER_BUFFER.lock().unwrap().clear();

        let func = SendtoSrcFunc::from_registry(FiledExtendType::InnerSource)
            .expect("processor registered");
        let sources = vec![
            DataField::from_chars("msg", "one"),
            DataField::from_chars("msg", "two"),
        ];
        let mut field = DataField::from_arr("msgs", sources);
        FieldPipe::process(&func, Some(&mut field)).expect("processor runs");
        let buf = INNER_BUFFER.lock().unwrap().clone();
        assert_eq!(buf, vec!["one".to_string(), "two".to_string()]);
        assert_eq!(field.get_meta(), &DataType::Ignore);
        assert!(matches!(field.get_value(), Value::Ignore(_)));

        clear_field_processors();
    }
}
