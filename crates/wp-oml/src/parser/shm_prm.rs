use winnow::ascii::multispace0;
use winnow::combinator::repeat;
use winnow::{ModalResult as WResult, Parser};

use crate::language::DirectAccessor;
use crate::language::{PreciseEvaluator, QueryPrimitiveBuilder};
use crate::oml_parser::keyword::{kw_gw_query, kw_lib, kw_where};
use crate::oml_parser::oml_aggregate::oml_var_get;
use crate::oml_parser::syntax;
use crate::oml_parser::syntax::oml_value;
use wp_parser::utils::get_scope;

pub fn oml_aga_shmlib(data: &mut &str) -> WResult<PreciseEvaluator> {
    kw_gw_query.parse_next(data)?;
    let lib = oml_query_lib.parse_next(data)?;
    kw_where.parse_next(data)?;
    let oct = oml_var_get.parse_next(data)?;
    let code = get_scope(data, '{', '}')?;
    let args: Vec<(String, String)> =
        repeat(0.., syntax::oml_properties).parse_next(&mut code.as_str())?;

    let builder = build_query(lib, oct, args)?;
    wp_parser::utils::err_convert(builder.build()).map(PreciseEvaluator::Query)
}

pub fn oml_query_lib(data: &mut &str) -> WResult<String> {
    kw_lib.parse_next(data)?;
    multispace0.parse_next(data)?;
    let lib = get_scope(data, '(', ')')?;
    Ok(lib.trim().to_string())
}

fn build_query(
    lib: String,
    oct: DirectAccessor,
    args: Vec<(String, String)>,
) -> ModalResult<QueryPrimitiveBuilder> {
    let mut builder = QueryPrimitiveBuilder::default();
    builder.lib(lib);
    builder.cond(oct);
    for (k, v) in args {
        match k.as_str() {
            "idx" => {
                builder.idx(v);
            }
            "_" => {
                let obj = oml_value.parse_next(&mut v.as_str())?;
                builder.default_val(Some(obj));
            }
            "col" => {
                builder.col(v);
            }
            _ => {
                unimplemented!()
            }
        }
    }
    Ok(builder)
}

#[cfg(test)]
mod tests {
    use wp_parser::WResult as ModalResult;

    use crate::oml_parser::shm_prm::oml_aga_shmlib;
    use crate::oml_parser::utils::for_test::assert_oml_parse;

    #[test]
    fn test_oml_crate_lib() -> ModalResult<()> {
        let mut code = r#" query lib(geo) where read( src_ip) {
     idx  : geo ,
     col  : domain ,
     _ : chars(office)
     }"#;
        assert_oml_parse(&mut code, oml_aga_shmlib);
        Ok(())
    }
}
