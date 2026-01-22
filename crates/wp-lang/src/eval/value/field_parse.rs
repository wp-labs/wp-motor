use crate::ast::WplField;
use winnow::ascii::multispace0;
use winnow::combinator::{delimited, fail};
use winnow::error::{StrContext, StrContextValue};
use winnow::token::{literal, take_until};
use wp_parser::Parser;
use wp_parser::WResult as ModalResult;
use wp_parser::symbol::ctx_desc;

pub trait FieldParse {
    fn scope_field<'a>(&self, data: &mut &'a str) -> ModalResult<&'a str>;
}
impl FieldParse for WplField {
    fn scope_field<'a>(&self, data: &mut &'a str) -> ModalResult<&'a str> {
        let _ = multispace0.parse_next(data)?;
        if let (Some(scope_beg), Some(scope_end)) = (
            self.fmt_conf().scope_beg.as_deref(),
            self.fmt_conf().scope_end.as_deref(),
        ) {
            self.take_scope_use(scope_beg, scope_end, data)
        } else {
            fail.context(ctx_desc("scope field error")).parse_next(data)
        }
    }
}

impl WplField {
    fn take_scope_use<'a>(
        &self,
        s_beg: &str,
        s_end: &str,
        data: &mut &'a str,
    ) -> ModalResult<&'a str> {
        let take = delimited(literal(s_beg), take_until(1.., s_end), literal(s_end))
            .context(StrContext::Label("scope"))
            .context(StrContext::Expected(StrContextValue::Description(
                "<<< scope: <beg,end>",
            )))
            .parse_next(data)?;

        //multispace0.parse_next(data)?;
        //self.sep_field(data)?;
        Ok(take)
    }
}
