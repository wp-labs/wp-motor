use orion_exp::{CmpOperator, Comparison, RustSymbol};
use winnow::combinator::fail;
use wp_model_core::model::{DataField, DataType};
use wp_parser::Parser;
use wp_parser::WResult;
use wp_parser::atom::take_var_name;
use wp_parser::cond::{CmpParser, ConditionParser, SymbolFrom};
use wp_parser::symbol::{ctx_desc, symbol_cmp, symbol_dollar};
use wpl::parser::datatype;

pub struct FieldDataOperator {}
impl CmpParser<DataField, RustSymbol> for FieldDataOperator {
    fn cmp_exp(data: &mut &str) -> WResult<Comparison<DataField, RustSymbol>> {
        // 1) try parse `isset($var)`
        {
            // 支持宽松空格：isset ( $var )
            let mut probe = *data;
            // 前导空白
            probe = probe.trim_start();
            if probe.starts_with("isset") {
                probe = &probe["isset".len()..];
                probe = probe.trim_start();
                if probe.starts_with('(') {
                    probe = &probe[1..];
                    probe = probe.trim_start();
                    if probe.starts_with('$') {
                        probe = &probe[1..];
                        // 读取变量名
                        let mut var_probe = probe;
                        let var_name = take_var_name(&mut var_probe)?;
                        probe = var_probe;
                        probe = probe.trim_start();
                        if probe.starts_with(')') {
                            probe = &probe[1..];
                            // commit：推进主输入
                            *data = probe;
                            let target = DataField::from_ignore("");
                            return Ok(Comparison::new(
                                CmpOperator::Eq,
                                var_name.to_string(),
                                target,
                            ));
                        }
                    }
                }
            }
        }
        // 2) default: `$var <op> <const>`
        symbol_dollar.parse_next(data)?;
        let var_name = take_var_name(data)?;
        let op = symbol_cmp.parse_next(data)?;
        let target = datatype::take_field(data)?;
        // Minimal validation: IpNet only supports Eq/Ne at parse-time
        let cop = CmpOperator::op_from(op);
        if *target.get_meta() == DataType::IpNet
            && !matches!(cop, CmpOperator::Eq | CmpOperator::Ne)
        {
            return fail
                .context(ctx_desc("cmp op not support for ipnet (only Eq/Ne)"))
                .parse_next(data);
        }
        let ins = Comparison::new(
            CmpOperator::op_from(op),
            var_name.to_string(),
            target.clone(),
        );
        Ok(ins)
    }
}
pub type WarpConditionParser = ConditionParser<DataField, FieldDataOperator, RustSymbol>;

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use wp_parser::WResult as ModalResult;
    use wp_parser::{Parser, WResult};

    use wp_model_core::model::DataField;

    use crate::cond::parse::{FieldDataOperator, WarpConditionParser};
    use orion_exp::{CmpOperator, ExpressionBuilder, LogicalBuilder, LogicalTrait, RustSymbol};
    use orion_exp::{Comparison, Expression};
    use wp_parser::cond::CmpParser;
    use wpl::parser::datatype;

    #[test]
    pub fn test_parse_value() -> WResult<()> {
        let mut data = r#"ip(127.0.0.1) "#;
        let x = datatype::take_field.parse_next(&mut data)?;
        let expect = DataField::from_ip("".to_string(), IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
        assert_eq!(x, expect);

        let mut data = r#"digit(10) "#;
        let x = datatype::take_field.parse_next(&mut data)?;
        let expect = DataField::from_digit("".to_string(), 10);
        assert_eq!(x, expect);

        let mut data = r#"chars(xyz) "#;
        let x = datatype::take_field.parse_next(&mut data)?;
        let expect = DataField::from_chars("".to_string(), "xyz".to_string());
        assert_eq!(x, expect);

        Ok(())
    }
    #[test]
    pub fn test_parse_express() -> WResult<()> {
        let mut data = r#"$IP == ip(127.0.0.1) "#;
        let x = FieldDataOperator::cmp_exp.parse_next(&mut data)?;
        let expect = Comparison::new(
            CmpOperator::Eq,
            "IP".to_string(),
            DataField::from_ip("", IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))),
        );
        assert_eq!(x, expect);

        let mut data = r#"$IP != ip(127.0.0.1) "#;
        let x = FieldDataOperator::cmp_exp.parse_next(&mut data)?;
        let expect = Comparison::new(
            CmpOperator::Ne,
            "IP".to_string(),
            DataField::from_ip("", IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))),
        );
        assert_eq!(x, expect);

        let mut data = r#"$len == digit(10) "#;
        let x = FieldDataOperator::cmp_exp(&mut data)?;
        let expect = Comparison::new(
            CmpOperator::Eq,
            "len".to_string(),
            DataField::from_digit("", 10),
        );
        assert_eq!(x, expect);

        let mut data = r#"$len =* digit(10) "#;
        let x = FieldDataOperator::cmp_exp.parse_next(&mut data)?;
        let expect = Comparison::new(
            CmpOperator::We,
            "len".to_string(),
            DataField::from_digit("", 10),
        );
        assert_eq!(x, expect);

        let mut data = r#"$len =* digit(10) "#;
        let y = WarpConditionParser::exp(&mut data)?;
        assert_eq!(y, Expression::Compare(expect));
        Ok(())
    }
    #[test]
    pub fn test_logic_express_or() -> WResult<()> {
        let mut data = r#"$IP == ip(127.0.0.1)  &&  $IP == ip(192.168.0.1) "#;
        let x = WarpConditionParser::exp(&mut data)?;
        let left = Comparison::new(
            CmpOperator::Eq,
            "IP",
            DataField::from_ip("", IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))),
        );
        let right = Comparison::new(
            CmpOperator::Eq,
            "IP",
            DataField::from_ip("", IpAddr::V4(Ipv4Addr::new(192, 168, 0, 1))),
        );
        let expect = LogicalBuilder::and(left, right).build();
        assert_eq!(x, expect);

        let mut data =
            r#"$IP == ip(127.0.0.1)  &&  $IP == ip(192.168.0.1)  || $IP == ip(192.168.0.2)"#;
        let x = WarpConditionParser::exp(&mut data)?;
        let right = Comparison::new(
            CmpOperator::Eq,
            "IP",
            DataField::from_ip("IP", IpAddr::V4(Ipv4Addr::new(192, 168, 0, 2))),
        );
        let expect = LogicalBuilder::or(expect, Expression::Compare(right)).build();
        //let expect = LogicalExpression::from_or(right, expect);
        assert_eq_exp(x, expect);
        Ok(())
    }
    #[test]
    pub fn test_logic_express_1() -> ModalResult<()> {
        let mut data =
            r#"($IP == ip(127.0.0.1) || $IP == ip(192.168.0.1))  &&  $IP == ip(192.168.0.2)"#;
        let expect = data;
        let x = WarpConditionParser::exp(&mut data)?;
        assert_eq_exp_str(x, expect);

        let mut data =
            r#"$IP == ip(127.0.0.1) || ( $IP == ip(192.168.0.1)  &&  $IP == ip(192.168.0.2) )"#;
        let expect = data;
        let x = WarpConditionParser::exp(&mut data)?;
        assert_eq_exp_str(x, expect);

        let mut data = r#"($IP > ip(127.0.0.1) && $IP < ip(127.0.0.2)) || ( $IP == ip(192.168.0.1)  &&  $IP == ip(192.168.0.2) )"#;
        let expect = data;
        let x = WarpConditionParser::exp(&mut data)?;
        assert_eq_exp_str(x, expect);

        Ok(())
    }

    #[test]
    pub fn test_logic_express_not() -> ModalResult<()> {
        let mut data = r#"! $IP == ip(192.168.0.1) "#;
        let x = WarpConditionParser::exp(&mut data)?;
        let right = Comparison::new(
            CmpOperator::Eq,
            "IP",
            DataField::from_ip("", IpAddr::V4(Ipv4Addr::new(192, 168, 0, 1))),
        );
        let expect = LogicalBuilder::not(right).build();
        assert_eq!(x, expect);
        Ok(())
    }

    #[test]
    pub fn test_express_x1() -> ModalResult<()> {
        let mut data =
            r#" $len == digit(10) &&  (! $IP == ip (192.168.0.1)  || $IP == ip(127.0.0.1)) "#;
        let expect = data;
        let obj = WarpConditionParser::exp(&mut data)?;
        assert_eq_exp_str(obj, expect);

        let mut data =
            " $len == digit(10)  \n && (! $IP == ip (192.168.0.1)  \n || $IP == ip(127.0.0.1)) ";
        let expect = data;
        let obj = WarpConditionParser::exp(&mut data)?;
        assert_eq_exp_str(obj, expect);

        Ok(())
    }

    #[test]
    pub fn test_express_x2() -> ModalResult<()> {
        let mut data = r#"$access_ip =* chars(10.48.95.78) || $access_ip =* chars(10.48.32.25)"#;
        let expect = data;
        let obj = WarpConditionParser::exp(&mut data)?;
        assert_eq_exp_str(obj, expect);

        let mut data = r#"$dat_type =* chars(告警日志) || $dat_type =* chars(安全日志) || $attacker_ip =* ip(10.91.7.38) || $attacker_ip =* ip(10.91.7.39) || $test =* chars(test16) || $test =* chars(test17)"#;
        let expect = r#"$dat_type =* chars(告警日志) || ( $dat_type =* chars(安全日志) || ( $attacker_ip =* ip(10.91.7.38) || ($attacker_ip =* ip(10.91.7.39) || ( $test =* chars(test16) || $test =* chars(test17) ))))"#;
        let obj = WarpConditionParser::exp(&mut data)?;
        assert_eq_exp_str(obj, expect);

        let mut data = r#"($access_ip =* chars(10.48.95.78) || $access_ip =* chars(10.48.32.25))"#;
        let expect = r#"$access_ip =* chars(10.48.95.78) || $access_ip =* chars(10.48.32.25)"#;
        let obj = WarpConditionParser::exp(&mut data)?;
        assert_eq_exp_str(obj, expect);

        let mut data = r#"(($access_ip =* chars(10.48.95.78) || $access_ip =* chars(10.48.32.25)) && ($dat_type =* chars(告警日志) || $dat_type =* chars(安全日志))) || (($attacker_ip =* ip(10.91.7.38) || $attacker_ip =* ip(10.91.7.39)) || ($test =* chars(test16) || $test =* chars(test17)))"#;
        let expect = r#"(($access_ip =* chars(10.48.95.78) || $access_ip =* chars(10.48.32.25)) && ($dat_type =* chars(告警日志) || $dat_type =* chars(安全日志))) || (($attacker_ip =* ip(10.91.7.38) || $attacker_ip =* ip(10.91.7.39)) || ($test =* chars(test16) || $test =* chars(test17)))"#;
        let obj = WarpConditionParser::exp(&mut data)?;
        assert_eq_exp_str(obj, expect);
        //express_assert(&mut data)?;

        Ok(())
    }
    fn assert_eq_exp(
        left: Expression<DataField, RustSymbol>,
        right: Expression<DataField, RustSymbol>,
    ) {
        let l_str = left.to_string();
        let r_str = right.to_string();
        print!("{}\n{}\n", l_str, r_str);
        assert_eq!(l_str, r_str)
    }
    fn assert_eq_exp_str(left: Expression<DataField, RustSymbol>, r_str: &str) {
        let l_str = left.to_string();
        print!("{}\n{}\n", l_str, r_str);
        assert_eq!(ignore_space(l_str.as_str()), ignore_space(r_str));
    }
    fn ignore_space(str: &str) -> String {
        str.replace(" ", "").replace("\n", "").replace("\t", "")
    }
}
