use crate::language::MapOperation;
use crate::language::{NestedBinding, ObjArrayOperation, PreciseEvaluator};
use crate::parser::fun_prm::oml_gw_fun;
use crate::parser::keyword::{kw_array, kw_object};
use crate::parser::oml_aggregate::oml_aggregate_sub;
use crate::parser::static_ctx::parse_static_value;
use crate::parser::tdc_prm::{oml_aga_tdc, oml_aga_value};
use winnow::ascii::multispace0;
use winnow::combinator::{alt, opt, repeat, trace};
use wp_primitives::Parser;
use wp_primitives::WResult;
use wp_primitives::symbol::symbol_semicolon;
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
    let subs_list: Vec<Vec<NestedBinding>> =
        trace(" repeat map item :", repeat(1.., oml_map_item)).parse_next(&mut &body[..])?;
    let mut map_get = MapOperation::new();
    for subs in subs_list {
        map_get.append(subs);
    }
    Ok(map_get)
}

#[cfg(test)]
mod tests {
    use wp_primitives::Parser;
    use wp_primitives::WResult;

    use crate::parser::map_prm::oml_map;
    use crate::parser::utils::for_test::assert_oml_parse;

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
}
