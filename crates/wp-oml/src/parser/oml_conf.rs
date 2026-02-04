use crate::language::{EvalExp, ObjModel};
use crate::parser::error::OMLCodeErrorTait;
use crate::parser::keyword::{kw_head_sep_line, kw_oml_name};
use crate::parser::oml_aggregate::oml_aggregate;
use winnow::ascii::multispace0;
use winnow::combinator::{opt, repeat};
use winnow::error::StrContext;
use wp_error::{OMLCodeError, OMLCodeResult};
use wp_parser::Parser;
use wp_parser::WResult;
use wp_parser::atom::{take_obj_path, take_obj_wild_path};
use wp_parser::symbol::symbol_colon;
use wpl::parser::utils::peek_str;

use super::keyword::kw_oml_rule;

pub fn oml_parse_raw(data: &mut &str) -> WResult<ObjModel> {
    oml_conf_code.parse_next(data)
}
pub fn oml_parse(data: &mut &str, tag: &str) -> OMLCodeResult<ObjModel> {
    match oml_conf_code.parse_next(data) {
        Ok(o) => Ok(o),
        Err(e) => Err(OMLCodeError::from_syntax(e, data, tag)),
    }
}

pub fn oml_conf_code(data: &mut &str) -> WResult<ObjModel> {
    let name = oml_conf_head.parse_next(data)?;
    debug_rule!("obj model: {} begin ", name);
    let mut a_items = ObjModel::new(name);
    let rules = opt(oml_conf_rules).parse_next(data)?;
    debug_rule!("obj model: rules loaded!");
    a_items.bind_rules(rules);
    kw_head_sep_line.parse_next(data)?;
    let mut items: Vec<EvalExp> = repeat(1.., oml_aggregate).parse_next(data)?;
    debug_rule!("obj model: aggregate item  loaded!");
    //repeat(1.., terminated(oml_aggregate, symbol_semicolon)).parse_next(data)?;
    a_items.items.append(&mut items);

    // Check if any field name starts with "__" (temporary field marker)
    let has_temp = check_temp_fields(&a_items.items);
    a_items.set_has_temp_fields(has_temp);

    multispace0.parse_next(data)?;
    if !data.is_empty() {
        if peek_str("---", data).is_ok() {
            kw_head_sep_line.parse_next(data)?;
        } else {
            //探测错误;
            oml_aggregate.parse_next(data)?;
        }
    }
    Ok(a_items)
}

/// Check if any evaluation expression has a target field starting with "__"
fn check_temp_fields(items: &[EvalExp]) -> bool {
    for item in items {
        match item {
            EvalExp::Single(single) => {
                if check_targets_temp(single.target()) {
                    return true;
                }
            }
            EvalExp::Batch(batch) => {
                if check_batch_target_temp(batch.target()) {
                    return true;
                }
            }
        }
    }
    false
}

fn check_targets_temp(targets: &[crate::language::EvaluationTarget]) -> bool {
    targets.iter().any(|t| {
        t.name()
            .as_ref()
            .map(|n| n.starts_with("__"))
            .unwrap_or(false)
    })
}

fn check_batch_target_temp(target: &crate::language::BatchEvalTarget) -> bool {
    target
        .origin()
        .name()
        .as_ref()
        .map(|n| n.starts_with("__"))
        .unwrap_or(false)
}

pub fn oml_conf_head(data: &mut &str) -> WResult<String> {
    multispace0.parse_next(data)?;
    let (_, _, name) = (
        kw_oml_name,
        symbol_colon,
        take_obj_path.context(StrContext::Label("oml name")),
    )
        .parse_next(data)?;
    Ok(name.to_string())
}
pub fn oml_conf_rules(data: &mut &str) -> WResult<Vec<String>> {
    multispace0.parse_next(data)?;
    let (_, _) = (kw_oml_rule, symbol_colon).parse_next(data)?;
    let rules: Vec<&str> = repeat(0.., take_obj_wild_path).parse_next(data)?;
    Ok(rules.into_iter().map(|s| s.to_string()).collect())
}

#[cfg(test)]
mod tests {
    use crate::parser::oml_conf::oml_parse_raw;
    use crate::parser::utils::for_test::{assert_oml_parse, assert_oml_parse_ext};
    use wp_parser::Parser;
    use wp_parser::WResult as ModalResult;
    use wp_parser::comment::CommentParser;

    #[test]
    fn test_conf_sample() -> ModalResult<()> {
        let mut code = r#"
name : test
rule :
    wpx/abc
    wpx/efg
---
version      :chars   = chars(1.0.0) ;
pos_sn       :chars   = take() ;
aler*        :auto   = take() ;
src_ip       :auto   = take();
update_time  :time    = take() { _ :  time(2020-10-01 12:30:30) };

        "#;
        assert_oml_parse(&mut code, oml_parse_raw);
        let mut code = r#"
name : test
rule :
    wpx/abc   wpx/efg
---
version      :chars   = chars(1.0.0) ;
pos_sn       :chars   = take() ;
aler*        : auto   = take() ;
update_time  :time    = take() { _ :  time(2020-10-01 12:30:30) };
        "#;
        assert_oml_parse(&mut code, oml_parse_raw);
        Ok(())
    }

    #[test]
    fn test_conf_fun() -> ModalResult<()> {
        let mut code = r#"
name : test
---
version      : chars   = Now::time() ;
version      : chars   = Now::time() ;
        "#;
        assert_oml_parse(&mut code, oml_parse_raw);
        Ok(())
    }

    #[test]
    fn test_conf_pipe() -> ModalResult<()> {
        let mut code = r#"
name : test
---
version      : chars   = pipe take() | base64_encode  ;
version      : chars   = pipe take(ip) | to_str |  base64_encode ;
        "#;
        assert_oml_parse(&mut code, oml_parse_raw);
        Ok(())
    }

    #[test]
    fn test_conf_pipe_optional_keyword() -> ModalResult<()> {
        use orion_error::TestAssert;

        // Test pipe without 'pipe' keyword - should parse successfully
        let mut code = r#"
name : test
---
url_secure = take(url) | starts_with('https://') | map_to(true) ;
encoded = read(data) | base64_encode ;
        "#;
        let model = oml_parse_raw(&mut code).assert();
        assert_eq!(model.name(), "test");
        assert_eq!(model.items.len(), 2);

        // Test mixed usage: with and without 'pipe' keyword
        let mut code = r#"
name : test
---
version1 = pipe take(ip) | to_str | base64_encode ;
version2 = take(ip) | to_str | base64_encode ;
        "#;
        let model = oml_parse_raw(&mut code).assert();
        assert_eq!(model.name(), "test");
        assert_eq!(model.items.len(), 2);

        Ok(())
    }
    #[test]
    fn test_conf_fmt() -> ModalResult<()> {
        let mut code = r#"
name : test
---
version      :chars   = fmt("_{}*{}",@ip,@sys)  ;
        "#;
        oml_parse_raw.parse_next(&mut code)?;
        //assert_oml_parse(&mut code, oml_conf);
        Ok(())
    }
    #[test]
    fn test_conf2() -> ModalResult<()> {
        let mut code = r#"
name : test
---
values : obj = object {
    cpu_free, memory_free, cpu_used_by_one_min, cpu_used_by_fifty_min             : digit  = take() ;
    process,disk_free, disk_used ,disk_used_by_fifty_min, disk_used_by_one_min    : digit  = take() ;
};
citys : array = collect take( keys : [ a,b,c* ] ) ;
        "#;
        let model = oml_parse_raw.parse_next(&mut code)?;
        assert_eq!(model.items.len(), 2);
        println!("{}", model);
        Ok(())
    }
    #[test]
    fn test_conf3() -> ModalResult<()> {
        let mut code = r#"
name : test
---
src_city: chars = match take( x_type ) {
            chars(A) => chars(bj),
            chars(B) => chars(cs),
            _ => take(src_city)
};
values : obj = object {
    cpu_free, memory_free, cpu_used_by_one_min, cpu_used_by_fifty_min             : digit  = take() ;
    process,disk_free, disk_used ,disk_used_by_fifty_min, disk_used_by_one_min    : digit  = take() ;
};
        "#;
        let model = oml_parse_raw.parse_next(&mut code)?;
        assert_eq!(model.items.len(), 2);
        println!("{}", model);
        Ok(())
    }

    #[test]
    fn test_conf4() -> ModalResult<()> {
        let mut code = r#"
name : test
---

src_city  = match take( x_type ) {
            chars(A) => chars(bj),
            chars(B) => chars(cs),
            _ => take(src_city)
};
values  = object {
    cpu_free, memory_free, cpu_used_by_one_min, cpu_used_by_fifty_min             : digit  = take() ;
    process,disk_free, disk_used ,disk_used_by_fifty_min, disk_used_by_one_min    : digit  = take() ;
};
"#;
        let model = oml_parse_raw.parse_next(&mut code)?;
        assert_eq!(model.items.len(), 2);
        println!("{}", model);
        Ok(())
    }
    #[test]
    fn test_conf_comment() -> ModalResult<()> {
        let mut raw_code = r#"
name : test
---
// this is ok;
version      = chars(1.0.0) ;
pos_sn       = take () ;
update_time  = take () { _ :  time(2020-10-01 12:30:30) };
        "#;

        let expect = r#"
name : test
---
version      : auto = chars(1.0.0) ;
pos_sn       : auto = take () ;
update_time  : auto = take () { _ :  time(2020-10-01 12:30:30) };
        "#;

        let code = CommentParser::ignore_comment(&mut raw_code)?;
        let mut pure_code = code.as_str();
        assert_oml_parse_ext(&mut pure_code, oml_parse_raw, expect);
        Ok(())
    }

    #[test]
    fn test_conf_quoted_chars() -> ModalResult<()> {
        use orion_error::TestAssert;

        // Test that chars() supports both quoted and unquoted strings
        let mut code1 = r#"
name : test
---
msg1 = chars('hello world');
msg2 = chars("goodbye");
msg3 = chars(unquoted);
        "#;
        let model = oml_parse_raw(&mut code1).assert();
        assert_eq!(model.name(), "test");

        // Test with special characters
        let mut code2 = r#"
name : test
---
msg = chars('hello\nworld');
        "#;
        let model2 = oml_parse_raw(&mut code2).assert();
        assert_eq!(model2.name(), "test");

        Ok(())
    }

    #[test]
    fn test_temp_field_filter() -> ModalResult<()> {
        use crate::core::DataTransformer;
        use orion_error::TestAssert;
        use wp_data_model::cache::FieldQueryCache;
        use wp_model_core::model::{DataRecord, DataType};

        // Test that fields starting with "__" are converted to ignore type
        let mut code = r#"
name : test
---
__temp = chars(temporary);
result = chars(final);
__another_temp = chars(also_temp);
        "#;
        let model = oml_parse_raw(&mut code).assert();
        assert_eq!(model.name(), "test");

        // Transform with empty input
        let cache = &mut FieldQueryCache::default();
        let input = DataRecord::default();
        let output = model.transform(input, cache);

        // Check that normal fields are preserved
        let result_field = output.field("result");
        assert!(result_field.is_some(), "Normal field 'result' should exist");
        assert_eq!(result_field.unwrap().get_meta(), &DataType::Chars);
        assert_eq!(result_field.unwrap().get_value().to_string(), "final");

        // Check that temporary fields are converted to ignore type
        let temp_field = output.field("__temp");
        assert!(
            temp_field.is_some(),
            "Temporary field '__temp' should exist"
        );
        assert_eq!(
            temp_field.unwrap().get_meta(),
            &DataType::Ignore,
            "Temporary field should be Ignore type"
        );

        let another_temp_field = output.field("__another_temp");
        assert!(
            another_temp_field.is_some(),
            "Temporary field '__another_temp' should exist"
        );
        assert_eq!(
            another_temp_field.unwrap().get_meta(),
            &DataType::Ignore,
            "Temporary field should be Ignore type"
        );

        Ok(())
    }

    #[test]
    fn test_temp_field_in_computation() -> ModalResult<()> {
        use crate::core::DataTransformer;
        use orion_error::TestAssert;
        use wp_data_model::cache::FieldQueryCache;
        use wp_model_core::model::{DataRecord, DataType};

        // Test that temporary fields can be used in intermediate computation
        // Simple test: use temp field in match expression
        let mut code = r#"
name : test
---
__temp_type = chars(error);
result = match read(__temp_type) {
    chars(error) => chars(failed),
    _ => chars(ok),
};
        "#;
        let model = oml_parse_raw(&mut code).assert();

        let cache = &mut FieldQueryCache::default();
        let input = DataRecord::default();
        let output = model.transform(input, cache);

        // Debug: print all fields
        println!("Output fields:");
        for field in &output.items {
            println!(
                "  {}: {} = {:?}",
                field.get_name(),
                field.get_meta(),
                field.get_value()
            );
        }

        // Check that the final result field exists
        let result = output.field("result");
        assert!(result.is_some(), "Result field should exist");

        // Check that temporary field is converted to ignore type
        let temp_field = output.field("__temp_type");
        assert!(temp_field.is_some());
        assert_eq!(
            temp_field.unwrap().get_meta(),
            &DataType::Ignore,
            "__temp_type should be Ignore type"
        );

        Ok(())
    }

    #[test]
    fn test_temp_field_flag() -> ModalResult<()> {
        use orion_error::TestAssert;

        // Test case 1: Model with no temporary fields
        let mut code_no_temp = r#"
name : test
---
normal1 = chars(value1);
normal2 = chars(value2);
        "#;
        let model_no_temp = oml_parse_raw(&mut code_no_temp).assert();
        assert_eq!(
            model_no_temp.has_temp_fields(),
            false,
            "Should not have temp fields flag"
        );

        // Test case 2: Model with temporary fields
        let mut code_with_temp = r#"
name : test
---
__temp = chars(temp_value);
normal = chars(normal_value);
        "#;
        let model_with_temp = oml_parse_raw(&mut code_with_temp).assert();
        assert_eq!(
            model_with_temp.has_temp_fields(),
            true,
            "Should have temp fields flag"
        );

        // Test case 3: Multiple temporary fields
        let mut code_multi_temp = r#"
name : test
---
__temp1 = chars(value1);
normal = chars(value2);
__temp2 = chars(value3);
        "#;
        let model_multi_temp = oml_parse_raw(&mut code_multi_temp).assert();
        assert_eq!(
            model_multi_temp.has_temp_fields(),
            true,
            "Should have temp fields flag"
        );

        Ok(())
    }
}
