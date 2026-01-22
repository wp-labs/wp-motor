use crate::language::BatchEvalExpBuilder;
use crate::language::EvalExp;
use crate::language::EvaluationTargetBuilder;
use crate::language::FieldRead;
use crate::language::FieldTakeBuilder;
use crate::language::GenericBinding;
use crate::language::NestedBinding;
use crate::language::ReadOptionBuilder;
use crate::language::RecordOperationBuilder;
use crate::language::SingleEvalExpBuilder;
use crate::language::{MatchSource, RecordOperation};

use crate::language::DirectAccessor;
use crate::language::{BatchEvalTarget, EvaluationTarget};
use crate::parser::collect_prm::oml_aga_collect;
use crate::parser::fmt_prm::oml_aga_fmt;
use crate::parser::fun_prm::oml_gw_fun;
use crate::parser::keyword::{kw_crate_symbol, kw_in, kw_keys, kw_option, kw_read, kw_take};
use crate::parser::map_prm::oml_aga_map;
use crate::parser::match_prm::oml_aga_match;
use crate::parser::pipe_prm; // for oml_aga_pipe_noprefix
use crate::parser::pipe_prm::oml_aga_pipe;
use crate::parser::sql_prm::oml_aga_sql;
use crate::parser::syntax::oml_default;
use crate::parser::tdc_prm::{oml_aga_tdc, oml_aga_value, oml_batch_gw_get};
use crate::parser::{oml_acq, syntax};
use winnow::ascii::multispace0;
use winnow::combinator::{alt, fail, peek, repeat, separated, trace};
use winnow::error::StrContext;
use winnow::error::StrContextValue;
use winnow::stream::Stream;
use wp_model_core::model::DataType;
use wp_parser::Parser;
use wp_parser::WResult;
use wp_parser::atom::{take_var_name, take_wild_key};
use wp_parser::symbol::ctx_desc;
use wp_parser::symbol::ctx_label;
use wp_parser::symbol::ctx_literal;
use wp_parser::symbol::{
    symbol_assign, symbol_brace_beg, symbol_brace_end, symbol_colon, symbol_comma, symbol_semicolon,
};
use wp_parser::utils::{RestAble, err_convert, get_scope};
use wpl::parser::datatype::take_datatype;
use wpl::parser::utils::{peek_str, take_key};

pub fn oml_target(data: &mut &str) -> WResult<EvaluationTarget> {
    let _ = multispace0.parse_next(data)?;
    let name_str = take_wild_key.parse_next(data)?;
    let _ = multispace0.parse_next(data)?;
    let meta = if peek_str(":", data).is_ok() {
        symbol_colon.parse_next(data)?;
        take_datatype.parse_next(data)?
    } else {
        DataType::Auto
    };
    let target_name = if name_str == "_" {
        None
    } else {
        Some(name_str.to_string())
    };
    err_convert(
        EvaluationTargetBuilder::default()
            .name(target_name)
            .data_type(meta)
            .build(),
        "EvaluationTarget build failed",
    )
}
pub fn oml_target_vec_same_meta(data: &mut &str) -> WResult<Vec<EvaluationTarget>> {
    let _ = multispace0.parse_next(data)?;
    let names: Vec<&str> = separated(1.., take_var_name, ",").parse_next(data)?;
    let _ = multispace0.parse_next(data)?;
    //symbol_colon.parse_next(data)?;
    //let meta = tdm_meta.parse_next(data)?;
    let meta = if peek_str(":", data).is_ok() {
        symbol_colon.parse_next(data)?;
        take_datatype.parse_next(data)?
    } else {
        DataType::Auto
    };
    let mut targets = Vec::new();
    for name_str in names {
        let target_name = if name_str == "_" {
            None
        } else {
            Some(name_str.to_string())
        };
        targets.push(EvaluationTarget::from((target_name, meta.clone())));
    }
    Ok(targets)
}

pub fn oml_target_vec(data: &mut &str) -> WResult<Vec<EvaluationTarget>> {
    let _ = multispace0.parse_next(data)?;
    let targets: Vec<EvaluationTarget> = separated(1.., oml_target, ",").parse_next(data)?;
    Ok(targets)
}

pub fn oml_aggregate(data: &mut &str) -> WResult<EvalExp> {
    let target_vec = oml_target_vec
        .context(StrContext::Label("oml target"))
        .context(StrContext::Expected(StrContextValue::Description(
            ">> <name> : <meta>",
        )))
        .parse_next(data)?;
    symbol_assign.parse_next(data)?;
    multispace0.parse_next(data)?;
    let key = peek(take_key).parse_next(data)?;

    let first_target = target_vec.first().expect("no target define");
    let unit = if first_target.safe_name().contains('*') {
        let gw = match key {
            "take" => oml_batch_gw_get
                .context(ctx_label("take"))
                .parse_next(data)?,
            "read" => oml_batch_gw_get
                .context(ctx_label("read"))
                .parse_next(data)?,
            _ => fail
                .context(ctx_label("method"))
                .context(ctx_literal("take ()"))
                .context(ctx_literal("only support take"))
                .parse_next(data)?,
        };
        let mut builder = BatchEvalExpBuilder::default();
        builder.target(BatchEvalTarget::new(first_target.clone()));
        builder.eval_way(gw);
        EvalExp::Batch(err_convert(builder.build(), "BatchEvalExp Build failed")?)
    } else {
        let gw = match key {
            "match" => oml_aga_match.parse_next(data)?,
            "object" => oml_aga_map.parse_next(data)?,
            "pipe" => oml_aga_pipe.parse_next(data)?,
            "collect" => oml_aga_collect.parse_next(data)?,
            //"query" => oml_aga_shmlib.parse_next(data)?,
            "select" => oml_aga_sql.parse_next(data)?,
            "fmt" => oml_aga_fmt.parse_next(data)?,
            "take" => alt((pipe_prm::oml_aga_pipe_noprefix, oml_aga_tdc)).parse_next(data)?,
            "read" => alt((pipe_prm::oml_aga_pipe_noprefix, oml_aga_tdc)).parse_next(data)?,
            _ => alt((
                trace("get value:", oml_aga_value),
                trace("fun  struct:", oml_gw_fun),
                fail.context(StrContext::Label("method"))
                    .context(StrContext::Expected(StrContextValue::StringLiteral(
                        "<meta>(...)",
                    )))
                    .context(StrContext::Expected(StrContextValue::StringLiteral(
                        "inner fun",
                    ))),
            ))
            .parse_next(data)?,
        };
        let mut builder = SingleEvalExpBuilder::default();
        builder.target(target_vec);
        builder.eval_way(gw);
        EvalExp::Single(err_convert(builder.build(), "SingleEvalExp Build Failed")?)
    };

    symbol_semicolon
        .context(StrContext::Label("oml semicolon"))
        .context(StrContext::Expected(StrContextValue::Description(
            ">> <item> ;",
        )))
        .parse_next(data)?;
    Ok(unit)
}

pub fn oml_aggregate_sub(data: &mut &str) -> WResult<Vec<NestedBinding>> {
    let targets = oml_target_vec_same_meta.parse_next(data)?;

    symbol_assign.parse_next(data)?;
    let sub_gw = oml_acq::oml_sub_acq.parse_next(data)?;

    let mut subs = Vec::new();
    for target in targets {
        subs.push(NestedBinding::new(target, sub_gw.clone()))
    }
    Ok(subs)
}

pub fn oml_var_get(data: &mut &str) -> WResult<DirectAccessor> {
    alt((
        oml_var_get_ref,
        oml_var_get_std
            .context(StrContext::Label("var"))
            .context(StrContext::Expected(StrContextValue::Description(
                ">> read ( <args>  ) ",
            ))),
    ))
    .parse_next(data)
}

pub fn oml_crate_tuple(data: &mut &str) -> WResult<MatchSource> {
    multispace0.parse_next(data)?;
    let cp = data.checkpoint();
    let code = get_scope(data, '(', ')').err_reset(data, &cp)?;
    let mut code_data: &str = code;
    let fst = oml_var_get.parse_next(&mut code_data)?;
    symbol_comma.parse_next(&mut code_data)?;
    let sec = oml_var_get.parse_next(&mut code_data)?;
    Ok(MatchSource::Double(fst, sec))
}

pub fn oml_crate_calc_ref(data: &mut &str) -> WResult<MatchSource> {
    multispace0.parse_next(data)?;
    if peek_str("(", data).is_ok() {
        oml_crate_tuple
            .context(ctx_desc(">> (<crate> , <crate>)"))
            .parse_next(data)
    } else {
        let obj = oml_var_get
            .context(ctx_desc(">> <crate>"))
            .parse_next(data)?;
        Ok(MatchSource::Single(obj))
    }
}

//#[allow(clippy::manual_inspect)]
pub fn oml_read(data: &mut &str) -> WResult<DirectAccessor> {
    kw_read.parse_next(data)?;
    let cp = data.checkpoint();

    let code = get_scope(data, '(', ')').inspect_err(|_e| {
        data.reset(&cp);
    })?;
    let args: Vec<(String, String)> = repeat(0.., syntax::oml_args).parse_next(&mut &code[..])?;

    let mut builder = ReadOptionBuilder::default();
    for (k, v) in args {
        match k.as_str() {
            "option" => {
                let keys = v.split(',').map(|x| x.trim().to_string()).collect();
                builder.option(keys);
            }
            "in" | "keys" => {
                let keys = v.split(',').map(|x| x.trim().to_string()).collect();
                builder.collect(keys);
            }
            "get" => {
                builder.get(Some(v));
            }
            _ => {
                fail.context(ctx_desc(
                    "unknown arg key. Expected: get | keys | option | <json_path>",
                ))
                .parse_next(data)?;
            }
        }
    }

    match builder.build() {
        Ok(obj) => Ok(DirectAccessor::Read(FieldRead::from(obj))),
        Err(_e) => {
            data.reset(&cp);
            fail.context(ctx_desc("read builder failed!"))
                .parse_next(data)
        }
    }
}
pub fn oml_var_get_std(data: &mut &str) -> WResult<DirectAccessor> {
    alt((oml_take, oml_read)).parse_next(data)
}

//#[allow(clippy::manual_inspect)]
pub fn oml_take(data: &mut &str) -> WResult<DirectAccessor> {
    kw_take.parse_next(data)?;
    let cp = data.checkpoint();

    let code = get_scope(data, '(', ')').inspect_err(|_e| {
        data.reset(&cp);
    })?;
    let args: Vec<(String, String)> = repeat(0.., syntax::oml_args).parse_next(&mut &code[..])?;

    let mut builder = FieldTakeBuilder::default();
    for (k, v) in args {
        match k.as_str() {
            "option" => {
                let keys = v.split(',').map(|x| x.trim().to_string()).collect();
                builder.option(keys);
            }
            "in" | "keys" => {
                let keys = v.split(',').map(|x| x.trim().to_string()).collect();
                builder.collect(keys);
            }
            "get" => {
                builder.get(Some(v));
            }
            _ => {
                fail.context(ctx_desc(
                    "unknown arg key. Expected: get | keys | option | <json_path>",
                ))
                .parse_next(data)?;
            }
        }
    }

    match builder.build() {
        Ok(obj) => Ok(DirectAccessor::Take(obj)),
        Err(_e) => {
            data.reset(&cp);
            fail.context(ctx_desc("take builder failed!"))
                .parse_next(data)
        }
    }
}
pub fn oml_var_get_ref(data: &mut &str) -> WResult<DirectAccessor> {
    kw_crate_symbol.parse_next(data)?;
    multispace0.parse_next(data)?;
    let key = take_var_name.parse_next(data)?;

    let mut builder = ReadOptionBuilder::default();
    builder.get(Some(key.to_string()));
    err_convert(
        builder
            .build()
            .map(|x| DirectAccessor::Read(FieldRead::from(x))),
        "ReadOption build Failed",
    )
}

pub fn oml_tdo_get(data: &mut &str) -> WResult<RecordOperation> {
    let mut builder = RecordOperationBuilder::default();
    let x = oml_var_get.parse_next(data)?;
    builder.dat_get(x);
    multispace0.parse_next(data)?;
    if let Ok(c) = wp_parser::utils::peek_one.parse_next(data) {
        if c == "{" {
            let o = oml_default_body.parse_next(data)?;
            builder.default_val(Some(o));
        } else {
            builder.default_val(None);
        }
    }
    err_convert(builder.build(), "RecordOperation Build Failed")
}
pub fn oml_default_body(data: &mut &str) -> WResult<GenericBinding> {
    symbol_brace_beg.parse_next(data)?;
    let df = oml_default
        .context(StrContext::Label("body"))
        .context(StrContext::Expected(StrContextValue::Description(
            ">> { <default> }",
        )))
        .parse_next(data)?;
    symbol_brace_end.parse_next(data)?;
    Ok(df)
}

pub fn oml_args_option(data: &mut &str) -> WResult<String> {
    (kw_option, symbol_colon).parse_next(data)?;
    let mapping = get_scope(data, '[', ']')?;
    Ok(mapping.to_string())
}
pub fn oml_args_in(data: &mut &str) -> WResult<String> {
    // 支持 in: 与 keys: 两种写法，语义等价
    alt((kw_in, kw_keys)).parse_next(data)?;
    symbol_colon.parse_next(data)?;
    let mapping = get_scope(data, '[', ']')?;
    Ok(mapping.to_string())
}

#[cfg(test)]
mod tests {

    use winnow::{ModalResult, Parser};

    use crate::language::PreciseEvaluator;
    use crate::parser::oml_aggregate::oml_target;
    use crate::parser::tdc_prm::oml_aga_tdc;
    use crate::parser::utils::for_test::fmt_assert_eq;

    #[test]
    fn test_oml_crate_removed() -> ModalResult<()> {
        Ok(())
    }

    #[test]
    fn test_oml_take() -> ModalResult<()> {
        let mut code =
            r#"take ( option : [ src_ip, s_ip, source_ip] ) { _   :  ip ( 127.10.10.10 ) } "#;
        let expect = code;

        if let PreciseEvaluator::Tdc(o) = oml_aga_tdc.parse_next(&mut code)? {
            let real = format!("{}", o);
            fmt_assert_eq(real.as_str(), expect);
            println!("{}", o);
        }
        Ok(())
    }

    #[test]
    fn test_oml_target_item() -> ModalResult<()> {
        let mut code = r#" src : ip "#;
        let x = oml_target.parse_next(&mut code)?;
        println!("{:?}", x);
        Ok(())
    }

    #[test]
    fn test_oml_target_2() -> ModalResult<()> {
        let mut code = r#" * : ip "#;
        let x = oml_target.parse_next(&mut code)?;
        println!("{:?}", x);
        Ok(())
    }
    #[test]
    fn test_oml_target_3() -> ModalResult<()> {
        let mut code = r#" * : auto "#;
        let x = oml_target.parse_next(&mut code)?;
        println!("{:?}", x);
        Ok(())
    }
    #[test]
    fn test_oml_target() -> ModalResult<()> {
        let mut code = r#" src : ip  = query lib(geo) where take ( src_ip)  {
            idx : geo , col : domain , _ : chars(office)
        };
        "#;
        let x = oml_target.parse_next(&mut code)?;
        println!("{:?}", x);

        let mut code = r#"
        begin : time =
        take (
            keys : [ src_ip, s_ip, source_ip]
             _   :  "2020-10-1:27:30:30"
        );
       "#;
        let x = oml_target.parse_next(&mut code)?;
        println!("{:?}", x);
        Ok(())
    }
}
