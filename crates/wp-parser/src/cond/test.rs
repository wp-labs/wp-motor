use crate::atom::take_var_name;
use crate::cond::SymbolFrom;
use crate::cond::{CmpParser, ConditionParser};
use crate::symbol::{symbol_cmp, symbol_dollar};
use orion_exp::{CmpOperator, Comparison, ConditionEvaluator, ValueGetter};
use std::collections::HashMap;
use winnow::ascii::{digit1, multispace0};
use winnow::{ModalResult as WResult, Parser};

use orion_exp::RustSymbol;

pub struct ObjGet {}
impl CmpParser<u32, RustSymbol> for ObjGet {
    fn cmp_exp(data: &mut &str) -> WResult<Comparison<u32, RustSymbol>> {
        symbol_dollar.parse_next(data)?;
        let var_name = take_var_name(data)?;
        let op = symbol_cmp.parse_next(data)?;
        multispace0.parse_next(data)?;
        let target = digit1.parse_next(data)?;
        let ins = Comparison::new(
            CmpOperator::op_from(op),
            var_name.to_string(),
            target.parse::<u32>().unwrap(),
        );
        Ok(ins)
    }
}

type SVMap = HashMap<&'static str, u32>;
struct VMap(HashMap<&'static str, u32>);
impl ValueGetter<u32> for VMap {
    fn get_value(&self, var: &str) -> Option<&u32> {
        self.0.get(var)
    }
}

/*
impl ConditionEvaluator<VMap> for LogicalExpress<u32, RustSymbol> {
    fn evaluate(&self, data: &VMap) -> bool {
        cmp_is_true(&self.op, self.left.as_ref(), &self.right, data)
    }
}
*/

type CondParser = ConditionParser<u32, ObjGet, RustSymbol>;
#[test]
pub fn test_express_exec_simple() -> WResult<()> {
    let data = SVMap::from([("A", 100), ("B", 200)]);

    let mut code = r#"$A == 100"#;
    let exp = CondParser::exp(&mut code)?;
    assert!(exp.evaluate(&VMap(data.clone())));

    let mut code = r#"$A =* 100"#;
    let exp = CondParser::exp(&mut code)?;
    assert!(exp.evaluate(&VMap(data.clone())));

    let mut code = r#"$A >= 100"#;
    let exp = CondParser::exp(&mut code)?;
    assert!(exp.evaluate(&VMap(data.clone())));

    let mut code = r#"$A <= 100"#;
    let exp = CondParser::exp(&mut code)?;
    assert!(exp.evaluate(&VMap(data.clone())));

    let mut code = r#"$A != 100"#;
    let exp = CondParser::exp(&mut code)?;
    assert!(!exp.evaluate(&VMap(data.clone())));

    let mut code = r#"$A > 90 && $B > 150"#;
    let exp = CondParser::exp(&mut code)?;
    assert!(exp.evaluate(&VMap(data.clone())));

    let mut code = r#"$A > 100 && $B > 150"#;
    let exp = CondParser::exp(&mut code)?;
    assert!(!exp.evaluate(&VMap(data.clone())));

    let mut code = r#"$A > 100 || $B > 150"#;
    let exp = CondParser::exp(&mut code)?;
    assert!(exp.evaluate(&VMap(data.clone())));

    let mut code = r#"$A < 10 || ($A >= 100 && $B > 150)"#;
    let exp = CondParser::exp(&mut code)?;
    assert!(exp.evaluate(&VMap(data.clone())));

    Ok(())
}

/*
 */
