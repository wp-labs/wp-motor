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
    debug_data!("obj model: {} begin ", name);
    let mut a_items = ObjModel::new(name);
    let rules = opt(oml_conf_rules).parse_next(data)?;
    debug_data!("obj model: rules loaded!");
    a_items.bind_rules(rules);
    kw_head_sep_line.parse_next(data)?;
    let mut items: Vec<EvalExp> = repeat(1.., oml_aggregate).parse_next(data)?;
    debug_data!("obj model: aggregate item  loaded!");
    //repeat(1.., terminated(oml_aggregate, symbol_semicolon)).parse_next(data)?;
    a_items.items.append(&mut items);
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
}
