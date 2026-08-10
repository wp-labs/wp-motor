use crate::language::MapOperation;
use crate::language::{NestedBinding, ObjArrayOperation, PreciseEvaluator};
use crate::parser::fun_prm::oml_gw_fun;
use crate::parser::keyword::{kw_array, kw_object};
use crate::parser::oml_aggregate::oml_aggregate_sub;
use crate::parser::static_ctx::parse_static_value;
use crate::parser::tdc_prm::{oml_aga_tdc, oml_aga_value};
use winnow::ascii::multispace0;
use winnow::combinator::{alt, cut_err, fail, opt, repeat, trace};
use winnow::error::{StrContext, StrContextValue};
use wp_primitives::Parser;
use wp_primitives::WResult;
use wp_primitives::symbol::{ctx_desc, symbol_semicolon};
use wp_primitives::utils::get_scope;

pub fn oml_aga_map(data: &mut &str) -> WResult<PreciseEvaluator> {
    let map = trace("gw map", oml_map).parse_next(data)?;
    Ok(PreciseEvaluator::Map(map))
}

pub fn oml_aga_obj_array(data: &mut &str) -> WResult<PreciseEvaluator> {
    let arr = trace("gw array", oml_obj_array).parse_next(data)?;
    Ok(PreciseEvaluator::ObjArray(arr))
}

/// `array { <item> ; <item> ; ... }`：元素为 object 字面量或值表达式
pub fn oml_obj_array(data: &mut &str) -> WResult<ObjArrayOperation> {
    kw_array.parse_next(data)?;
    multispace0.parse_next(data)?;
    let body = get_scope(data, '{', '}')?;
    let mut items: Vec<PreciseEvaluator> = Vec::new();
    let mut body_data: &str = body;
    loop {
        multispace0.parse_next(&mut body_data)?;
        if body_data.is_empty() {
            break;
        }
        let item = trace("array item", oml_array_item).parse_next(&mut body_data)?;
        items.push(item);
    }
    Ok(ObjArrayOperation::new(items))
}

/// 数组元素：object 字面量 / 嵌套 array / take / read / 值 / 函数 / 静态符号
fn oml_array_item(data: &mut &str) -> WResult<PreciseEvaluator> {
    let gw = alt((
        trace("array item map:", oml_aga_map),
        trace("array item array:", oml_aga_obj_array),
        trace("array item take:", oml_aga_tdc),
        trace("array item fun:", oml_gw_fun),
        trace("array item value:", oml_aga_value),
        trace("array item static:", parse_static_value),
    ))
    .parse_next(data)?;
    opt(symbol_semicolon).parse_next(data)?;
    Ok(gw)
}

pub fn oml_map_item(data: &mut &str) -> WResult<Vec<NestedBinding>> {
    let subs: Vec<NestedBinding> = oml_aggregate_sub.parse_next(data)?;
    //opt(symbol_semicolon).parse_next(data)?;
    Ok(subs)
}
pub fn oml_map(data: &mut &str) -> WResult<MapOperation> {
    kw_object.parse_next(data)?;
    multispace0.parse_next(data)?;
    let body = get_scope(data, '{', '}')?;
    let mut body_data: &str = body;
    let subs_list: Vec<Vec<NestedBinding>> =
        trace(" repeat map item :", repeat(1.., oml_map_item)).parse_next(&mut body_data)?;
    multispace0.parse_next(&mut body_data)?;
    if !body_data.is_empty() {
        // `repeat` 在某个成员无法继续解析时会静默结束，这里必须确认 body 已被完整消费，
        // 否则非法/未解析成员会被悄悄丢弃，导致部分对象成功加载。
        return cut_err(
            fail.context(ctx_desc("unexpected content in object member"))
                .context(StrContext::Expected(StrContextValue::StringLiteral(
                    "<name> = <value>;",
                ))),
        )
        .parse_next(&mut body_data);
    }
    let mut map_get = MapOperation::new();
    for subs in subs_list {
        map_get.append(subs);
    }
    Ok(map_get)
}

#[cfg(test)]
mod tests {
    use crate::language::NestedAccessor;
    use wp_primitives::Parser;
    use wp_primitives::WResult;

    use crate::parser::map_prm::oml_map;
    use crate::parser::utils::for_test::{assert_oml_parse, err_of_oml};

    #[test]
    fn test_oml_map() -> WResult<()> {
        let mut code = r#"
    object {
        cpu_free : digit  = take() ;
        process : digit  = take()  ;
    }
     "#;
        assert_oml_parse(&mut code, oml_map);
        Ok(())
    }

    #[test]
    fn test_oml_map1() -> WResult<()> {
        let mut code = r#"
object {
    cpu_free, memory_free, cpu_used_by_one_min, cpu_used_by_fifty_min             : digit  = take();
    process,disk_free, disk_used ,disk_used_by_fifty_min, disk_used_by_one_min    : digit  = take();
}
     "#;
        let x = oml_map.parse_next(&mut code)?;
        println!("{}", x);
        Ok(())
    }

    #[test]
    fn test_oml_map2() -> WResult<()> {
        let mut code = r#"
            object {
                cpu_free = take();
            }
     "#;
        let x = oml_map.parse_next(&mut code)?;
        println!("{}", x);
        Ok(())
    }

    #[test]
    fn test_oml_map3() -> WResult<()> {
        let mut code = r#"
            object {
                cpu_free, cpu_free2 = take();
                cpu_free3, cpu_free4 : digit = take();
            }
     "#;
        let x = oml_map.parse_next(&mut code)?;
        println!("{}", x);
        Ok(())
    }

    // issue #348: 嵌套 object 成员含 pipe 时，后续兄弟字段必须全部保留
    #[test]
    fn test_oml_map_nested_pipe_tail() -> WResult<()> {
        let mut code = r#"
object {
    before = chars(before);
    rule = object {
        signature_id = pipe read(missing_rule_ids) | nth(0);
    };
    after = chars(after);
    nested_after = object {
        value = chars(also_after);
    };
}
"#;
        let map = oml_map.parse_next(&mut code)?;
        assert!(
            code.trim().is_empty(),
            "object body must be fully consumed, remain: {:?}",
            code
        );
        let names: Vec<String> = map.subs().iter().map(|b| b.target().safe_name()).collect();
        assert_eq!(names, vec!["before", "rule", "after", "nested_after"]);
        match map.subs()[1].acquirer() {
            NestedAccessor::Map(rule) => {
                let inner: Vec<String> =
                    rule.subs().iter().map(|b| b.target().safe_name()).collect();
                assert_eq!(inner, vec!["signature_id"]);
                assert!(
                    matches!(rule.subs()[0].acquirer(), NestedAccessor::Pipe(_)),
                    "nested pipe member must be a Pipe accessor"
                );
            }
            other => panic!("expected nested Map, got {:?}", other),
        }
        Ok(())
    }

    // issue #348: 非法 object 成员必须使整个 object 校验失败，而不是静默丢弃
    #[test]
    fn test_oml_map_invalid_member_rejected() {
        let mut code = r#"
object {
    before = chars(before);
    after = ;
}
"#;
        use wp_error::parse_error::OMLCodeReason;

        let e = err_of_oml(&mut code, oml_map);
        println!("err: {}", e);
        assert!(
            matches!(e.reason(), OMLCodeReason::Syntax(s) if s.contains("unexpected content in object member")),
            "error should point at the invalid member, got: {}",
            e
        );
    }

    // issue #348: 非法成员位于有效成员之后（中间位置），同样必须整体失败
    #[test]
    fn test_oml_map_invalid_member_at_middle_rejected() {
        use wp_error::parse_error::OMLCodeReason;

        let mut code = r#"
object {
    before = chars(before);
    bad = ;
    after = chars(after);
}
"#;
        let e = err_of_oml(&mut code, oml_map);
        assert!(
            matches!(e.reason(), OMLCodeReason::Syntax(s) if s.contains("unexpected content in object member")),
            "error should point at the invalid member, got: {}",
            e
        );
    }

    // issue #348: 首成员非法走 repeat(1..) 首项失败路径，同样不得成功
    #[test]
    fn test_oml_map_invalid_first_member_rejected() {
        use wp_error::parse_error::OMLCodeReason;

        let mut code = r#"
object {
    @@bad@@ = ;
    before = chars(before);
}
"#;
        let e = err_of_oml(&mut code, oml_map);
        assert!(matches!(e.reason(), OMLCodeReason::Syntax(_)));
    }

    // issue #348: 嵌套 object 中 pipe 成员出现在首/中/末位置，兄弟字段均保留
    #[test]
    fn test_oml_map_pipe_member_any_position() -> WResult<()> {
        let mut code = r#"
object {
    first = pipe read(a) | to_str;
    mid = chars(m);
    last = pipe read(b) | to_str;
}
"#;
        let map = oml_map.parse_next(&mut code)?;
        assert!(
            code.trim().is_empty(),
            "object body must be fully consumed, remain: {:?}",
            code
        );
        let names: Vec<String> = map.subs().iter().map(|b| b.target().safe_name()).collect();
        assert_eq!(names, vec!["first", "mid", "last"]);
        assert!(matches!(map.subs()[0].acquirer(), NestedAccessor::Pipe(_)));
        assert!(matches!(map.subs()[2].acquirer(), NestedAccessor::Pipe(_)));
        Ok(())
    }

    // issue #348: 省略 pipe 前缀（read(...) | fun）作为 object 成员值
    #[test]
    fn test_oml_map_pipe_no_prefix_member() -> WResult<()> {
        let mut code = r#"
object {
    before = chars(before);
    sig = read(rule_id) | to_str;
    after = chars(after);
}
"#;
        let map = oml_map.parse_next(&mut code)?;
        assert!(code.trim().is_empty());
        let names: Vec<String> = map.subs().iter().map(|b| b.target().safe_name()).collect();
        assert_eq!(names, vec!["before", "sig", "after"]);
        assert!(matches!(map.subs()[1].acquirer(), NestedAccessor::Pipe(_)));
        Ok(())
    }
}
