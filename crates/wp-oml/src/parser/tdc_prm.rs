use crate::language::{BatchEvaluation, PreciseEvaluator};
use crate::parser::oml_aggregate;
use crate::parser::syntax::oml_value;
use winnow::ascii::multispace0;
use wp_parser::Parser;
use wp_parser::WResult;
use wpl::parser::utils::take_sql_tval;

pub fn oml_aga_value(data: &mut &str) -> WResult<PreciseEvaluator> {
    let v = oml_value.parse_next(data)?;
    Ok(PreciseEvaluator::Obj(v))
}

pub fn oml_aga_tdc(data: &mut &str) -> WResult<PreciseEvaluator> {
    //kw_gw_get.parse_next(data)?;
    oml_aggregate::oml_tdo_get
        .parse_next(data)
        .map(PreciseEvaluator::Tdc)
}

pub fn oml_sql_raw(data: &mut &str) -> WResult<PreciseEvaluator> {
    multispace0.parse_next(data)?;
    let val = take_sql_tval.parse_next(data)?;
    Ok(PreciseEvaluator::Val(val))
}
pub fn oml_batch_gw_get(data: &mut &str) -> WResult<BatchEvaluation> {
    oml_aggregate::oml_tdo_get
        .parse_next(data)
        .map(BatchEvaluation::Get)
}

#[cfg(test)]
mod tests {
    use crate::parser::tdc_prm::{oml_aga_tdc, oml_aga_value};
    use crate::parser::utils::for_test::assert_oml_parse;
    use orion_error::{ToStructError, UvsConfFrom};
    use wp_error::OMLCodeReason;
    use wp_error::parse_error::OMLCodeResult;
    use wp_parser::Parser;
    use wp_parser::WResult as ModalResult;

    #[test]
    fn test_oml_take() -> OMLCodeResult<()> {
        let mut code = r#"read(src){ _ : Now::date() }"#;
        assert_oml_parse(&mut code, oml_aga_tdc);

        let mut code = r#"read(src){ _ : Now::time() }"#;
        assert_oml_parse(&mut code, oml_aga_tdc);
        let mut code = r#"read(src){ _ : chars(hello)}"#;
        assert_oml_parse(&mut code, oml_aga_tdc);

        let mut code = r#"take() "#;
        let x = oml_aga_tdc.parse_next(&mut code).map_err(|e| {
            OMLCodeReason::from_conf(code.to_string())
                .to_err()
                .with_detail(e.to_string())
        })?;
        println!("{:?}", x);

        let mut code = r#"take(src_ip) "#;
        assert_oml_parse(&mut code, oml_aga_tdc);

        let mut code = r#"read(src_ip) "#;
        assert_oml_parse(&mut code, oml_aga_tdc);

        let mut code = r#"read(/a/b/c) "#;
        assert_oml_parse(&mut code, oml_aga_tdc);
        let mut code = r#"read(/a/b/[0]/1) "#;
        assert_oml_parse(&mut code, oml_aga_tdc);
        Ok(())
    }
    #[test]
    fn test_oml_value() -> ModalResult<()> {
        let mut code = r#" ip(127.0.0.1) "#;
        assert_oml_parse(&mut code, oml_aga_value);
        Ok(())
    }
}
