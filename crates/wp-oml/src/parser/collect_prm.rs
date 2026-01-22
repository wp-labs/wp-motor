use crate::language::ArrOperation;
use crate::language::PreciseEvaluator;
use crate::parser::keyword::kw_gw_collect;
use crate::parser::oml_aggregate::oml_var_get;
use winnow::error::{StrContext, StrContextValue};
use wp_parser::Parser;
use wp_parser::WResult;

pub fn oml_aga_collect(data: &mut &str) -> WResult<PreciseEvaluator> {
    Ok(PreciseEvaluator::Collect(
        oml_collect
            .context(StrContext::Label("GW"))
            .context(StrContext::Expected(StrContextValue::Description(
                ">> collect  <crate>",
            )))
            .parse_next(data)?,
    ))
}

pub fn oml_collect(data: &mut &str) -> WResult<ArrOperation> {
    kw_gw_collect.parse_next(data)?;
    let from = oml_var_get.parse_next(data)?;
    //dat_crate::oml_crate.parse_next(data).map(GetWay::Direct)
    Ok(ArrOperation::new(from))
}

#[cfg(test)]
mod tests {
    use crate::core::DataTransformer;
    use crate::parser::collect_prm::oml_aga_collect;
    use crate::parser::oml_parse_raw;
    use orion_error::TestAssert;
    use wp_data_model::cache::FieldQueryCache;
    use wp_model_core::model::{DataField, DataRecord};
    use wp_parser::WResult as ModalResult;

    #[test]
    fn test_oml_collect() -> ModalResult<()> {
        let mut code = r#" collect take( keys: [a,b,c*]) "#;
        let expect = r#" collect take( in: [a,b,c*]) "#;
        crate::parser::utils::for_test::assert_oml_parse_ext(&mut code, oml_aga_collect, expect);
        Ok(())
    }

    #[test]
    fn test_collect_array() {
        let cache = &mut FieldQueryCache::default();

        let data = vec![
            DataField::from_chars("sport", "514"),
            DataField::from_chars("dport", "22"),
        ];
        let src = DataRecord { items: data };

        let mut conf = r#"
        name : das_apt_alert_log
        ---
        sport:digit = read(sport);
        dport:digit = read(dport);
        port_list = collect read(keys:[sport,dport]);
         "#;
        let model = oml_parse_raw(&mut conf).assert();
        let target = model.transform(src, cache);

        let expect = DataField::from_arr(
            "port_list".to_string(),
            vec![
                DataField::from_digit("sport", 514),
                DataField::from_digit("dport", 22),
            ],
        );
        assert_eq!(target.field("port_list"), Some(&expect));
    }
}
