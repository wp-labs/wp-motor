use orion_exp::CmpOperator;
use wp_parser::atom::take_var_name;
use wp_parser::cond::{CmpParser, ConditionParser};
use wp_parser::sql_symbol::symbol_sql_cmp; // for operator flip when normal form not matched

use crate::language::CompareExpress;
use crate::language::CondAccessor;
use crate::language::PreciseEvaluator;
use crate::parser::oml_acq::oml_cond_acq;
use orion_exp::SQLSymbol;
use winnow::stream::Stream;
use wp_parser::Parser;
use wp_parser::WResult; // for checkpoint/reset on &str parser input

#[cfg(test)]
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct CondParser {}

pub type SCondParser = ConditionParser<CondAccessor, SQLGet, SQLSymbol>;

pub struct SQLGet {}

impl CmpParser<CondAccessor, SQLSymbol> for SQLGet {
    fn cmp_exp(data: &mut &str) -> WResult<CompareExpress> {
        // 1) Prefer LHS function: support `sql_fn(...) <op> <literal>` by flipping
        let cp_fn = data.checkpoint();
        if let Ok(left_acc) = oml_cond_acq.parse_next(data)
            && let Ok(op) = symbol_sql_cmp.parse_next(data)
        {
            if let CondAccessor::SqlFn(_) = &left_acc
                && let Ok((_, PreciseEvaluator::Val(v))) =
                    super::tdc_prm::oml_sql_raw.parse_peek(data)
            {
                let _ = super::tdc_prm::oml_sql_raw.parse_next(data)?; // consume
                let lit = render_sql_literal_for_value(&v);
                let flipped = flip_op(op);
                return Ok(CompareExpress::new(flipped, lit, left_acc));
            }
            // Also allow generic accessor vs column: `<accessor> <op> <column>` -> flip
            if let Ok(var_name) = take_var_name.parse_next(data) {
                // If what looks like a column is actually a function name followed by '(',
                // bail out and let canonical branch handle it (to consume the whole fn call).
                let rest = data.trim_start();
                if rest.starts_with('(') {
                    // rollback the entire function-first path
                    data.reset(&cp_fn);
                } else {
                    let flipped = flip_op(op);
                    return Ok(CompareExpress::new(flipped, var_name.to_string(), left_acc));
                }
            }
        }
        data.reset(&cp_fn);

        // 2) Canonical form: <column> <op> <accessor>
        let cp0 = data.checkpoint();
        if let Ok(var_name) = take_var_name.parse_next(data) {
            if let Ok(op) = symbol_sql_cmp.parse_next(data)
                && let Ok(target) = oml_cond_acq.parse_next(data)
            {
                return Ok(CompareExpress::new(op, var_name.to_string(), target));
            }
            data.reset(&cp0);
        }

        // 3) Final attempt to attach contexts
        let var_name = take_var_name(data)?;
        let op = symbol_sql_cmp.parse_next(data)?;
        let target = oml_cond_acq.parse_next(data)?;
        Ok(CompareExpress::new(op, var_name.to_string(), target))
    }
}

// Flip comparison operator when swapping LHS/RHS
fn flip_op(co: CmpOperator) -> CmpOperator {
    match co {
        CmpOperator::Gt => CmpOperator::Lt,
        CmpOperator::Ge => CmpOperator::Le,
        CmpOperator::Lt => CmpOperator::Gt,
        CmpOperator::Le => CmpOperator::Ge,
        CmpOperator::Eq => CmpOperator::Eq,
        CmpOperator::Ne => CmpOperator::Ne,
        CmpOperator::We => CmpOperator::We,
    }
}

// Minimal SQL literal renderer for Value to place on left side (used only on fallback Case B)
// Keep in sync with accessors::render_sql_literal for basic types.
fn render_sql_literal_for_value(v: &wp_model_core::model::Value) -> String {
    use wp_model_core::model::Value as V;
    match v {
        V::Digit(d) => d.to_string(),
        V::Float(f) => {
            if f.fract() == 0.0 {
                format!("{:.0}", f)
            } else {
                f.to_string()
            }
        }
        V::Bool(b) => {
            if *b {
                "1".to_string()
            } else {
                "0".to_string()
            }
        }
        V::Chars(s) => {
            let esc = s.replace('\'', "''");
            format!("'{}'", esc)
        }
        _ => format!("'{}'", v),
    }
}

#[cfg(test)]
mod tests {

    use wp_parser::WResult as ModalResult; // test helper

    //use orion_overload::cond::LogicalExpression;
    //
    use crate::language::LogicalExpression;
    use crate::parser::cond::SCondParser;

    #[test]
    pub fn test_parse_express() -> ModalResult<()> {
        let mut data = r#"IP = 100;"#;
        let expect = r#"IP = 100"#;
        let x = SCondParser::end_exp(&mut data, ";")?;
        assert_eq_sql(x, expect);

        let mut data = r#"IP = read(ip);"#;
        let expect = r#"IP = read(ip)"#;
        let x = SCondParser::end_exp(&mut data, ";")?;
        assert_eq_sql(x, expect);

        let mut data = r#"IP > read(ip) "#;
        let expect = data;
        let x = SCondParser::exp(&mut data)?;
        assert_eq_sql(x, expect);
        Ok(())
    }
    #[test]
    pub fn test_logic_express() -> ModalResult<()> {
        let mut data = r#"IP = read (ip)  or IP = read(ip) "#;
        let expect = data;
        let x = SCondParser::exp(&mut data)?;
        assert_eq_sql(x, expect);
        let mut data = r#"IP = read (ip)  or IP = read(ip) "#;
        let expect = data;
        let x = SCondParser::exp(&mut data)?;
        assert_eq_sql(x, expect);

        let mut data = r#"IP = read (ip)  or  IP = read(ip)  or IP = read(ip)"#;
        let expect = r#"IP = read (ip)  or  (IP = read(ip)  or IP = read(ip))"#;
        let x = SCondParser::exp(&mut data)?;
        assert_eq_sql(x, expect);

        Ok(())
    }

    #[test]
    pub fn test_logic_express_not() -> ModalResult<()> {
        let mut data = r#"not IP = read(ip) "#;
        let expect = data;
        let x = SCondParser::exp(&mut data)?;
        assert_eq_sql(x, expect);
        let mut data = r#"(not  IP = read (ip)) "#;
        let expect = r#"not  IP = read (ip)"#;
        let x = SCondParser::exp(&mut data)?;
        assert_eq_sql(x, expect);
        let mut data = r#" len = read (ip) and  (not IP = read(ip)  or IP = read(ip)) "#;
        let expect = data;
        let x = SCondParser::exp(&mut data)?;
        assert_eq_sql(x, expect);

        Ok(())
    }

    fn assert_eq_sql(left: LogicalExpression, r_str: &str) {
        let l_str = left.to_string();
        print!("{}\n{}\n", l_str, r_str);
        assert_eq!(ignore_space(l_str.as_str()), ignore_space(r_str));
    }
    fn ignore_space(str: &str) -> String {
        str.replace(" ", "").replace("\n", "").replace("\t", "")
    }
}
