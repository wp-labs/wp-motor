use crate::language::PreciseEvaluator;
use crate::language::{FmtOperation, RecordOperation};
use crate::parser::keyword::kw_fmt;
use crate::parser::oml_aggregate::oml_var_get;
use winnow::ascii::multispace0;
use winnow::combinator::repeat;
use wp_parser::Parser;
use wp_parser::WResult as ModalResult;
use wp_parser::symbol::symbol_comma;
use wp_parser::utils::get_scope;

pub fn oml_aga_fmt(data: &mut &str) -> ModalResult<PreciseEvaluator> {
    let fmt = oml_fmt_item.parse_next(data)?;
    Ok(PreciseEvaluator::Fmt(fmt))
}

pub fn oml_fmt_item(data: &mut &str) -> ModalResult<FmtOperation> {
    multispace0.parse_next(data)?;
    kw_fmt.parse_next(data)?;
    (multispace0, "(", multispace0).parse_next(data)?;
    let fmt_str = get_scope(data, '"', '"')?;
    let args = repeat(1.., oml_arg_item).parse_next(data)?;
    (multispace0, ")", multispace0).parse_next(data)?;
    let _ = get_scope(data, '(', ')');
    let get = FmtOperation::new(fmt_str.to_string(), args);
    Ok(get)
}
pub fn oml_arg_item(data: &mut &str) -> ModalResult<RecordOperation> {
    let (_, x) = (symbol_comma, oml_var_get).parse_next(data)?;
    Ok(RecordOperation::new(x))
}

#[cfg(test)]
mod tests {
    use crate::parser::fmt_prm::oml_aga_fmt;
    use crate::parser::utils::for_test::assert_oml_parse;
    use wp_parser::WResult as ModalResult;

    #[test]
    fn test_oml_crate_lib() -> ModalResult<()> {
        let mut code = r#" fmt("{a}-{b}" , read(a), read(b) )
     "#;
        assert_oml_parse(&mut code, oml_aga_fmt);
        Ok(())
    }
}
