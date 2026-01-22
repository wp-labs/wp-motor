use crate::language::{
    BuiltinFunction, FUN_NOW_DATE, FUN_NOW_HOUR, FUN_NOW_TIME, FunOperation, NowDate, NowHour,
    NowTime, PreciseEvaluator,
};
use winnow::ascii::multispace0;
use winnow::combinator::alt;
use wp_parser::Parser;
use wp_parser::WResult;
use wp_parser::utils::get_scope;

pub fn oml_gw_fun(data: &mut &str) -> WResult<PreciseEvaluator> {
    let fun = oml_fun_item.parse_next(data)?;
    Ok(PreciseEvaluator::Fun(FunOperation::new(fun)))
}

pub fn oml_fun_item(data: &mut &str) -> WResult<BuiltinFunction> {
    multispace0.parse_next(data)?;
    let fun = alt((
        FUN_NOW_DATE.map(|_| BuiltinFunction::NowDate(NowDate::default())),
        FUN_NOW_HOUR.map(|_| BuiltinFunction::NowHour(NowHour::default())),
        FUN_NOW_TIME.map(|_| BuiltinFunction::NowTime(NowTime::default())),
    ))
    .parse_next(data)?;
    let _ = get_scope(data, '(', ')');
    Ok(fun)
}

#[cfg(test)]
mod tests {
    use crate::parser::fun_prm::oml_gw_fun;
    use crate::parser::utils::for_test::assert_oml_parse;
    use wp_parser::WResult as ModalResult;

    #[test]
    fn test_oml_crate_lib() -> ModalResult<()> {
        let mut code = r#" Now::time()
     "#;
        assert_oml_parse(&mut code, oml_gw_fun);

        let mut code = r#" Now::hour()
     "#;
        assert_oml_parse(&mut code, oml_gw_fun);

        let mut code = r#" Now::date()
     "#;
        assert_oml_parse(&mut code, oml_gw_fun);

        Ok(())
    }
}
