use crate::language::MapOperation;
use crate::language::{NestedBinding, PreciseEvaluator};
use crate::parser::keyword::kw_object;
use crate::parser::oml_aggregate::oml_aggregate_sub;
use winnow::ascii::multispace0;
use winnow::combinator::{repeat, trace};
use wp_parser::Parser;
use wp_parser::WResult;
use wp_parser::utils::get_scope;

pub fn oml_aga_map(data: &mut &str) -> WResult<PreciseEvaluator> {
    let map = trace("gw map", oml_map).parse_next(data)?;
    Ok(PreciseEvaluator::Map(map))
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
    use wp_parser::Parser;
    use wp_parser::WResult;

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
