use std::net::Ipv4Addr;

use crate::cond::parse::WarpConditionParser;
use wp_data_model::conditions::evaluate_expression;
use wp_model_core::model::{DataField, DataRecord};
use wp_parser::WResult as ModalResult;

#[test]
pub fn test_express_exec_simple() -> ModalResult<()> {
    let tdc = test_crate();

    let mut data = r#"$IP == ip(127.0.0.1) "#;
    let exp = WarpConditionParser::exp(&mut data)?;
    assert!(evaluate_expression(&exp, &tdc));

    let mut data = r#"$IP > ip(127.0.0.0) "#;
    let exp = WarpConditionParser::exp(&mut data)?;
    assert!(evaluate_expression(&exp, &tdc));

    let mut data = r#"$IP < ip(127.0.0.2) "#;
    let exp = WarpConditionParser::exp(&mut data)?;
    assert!(evaluate_expression(&exp, &tdc));

    let mut data = r#"$IP =* ip(127.0.0.1) "#;
    let exp = WarpConditionParser::exp(&mut data)?;
    assert!(evaluate_expression(&exp, &tdc));

    let mut data = r#"$city =* chars(c*a) "#;
    let exp = WarpConditionParser::exp(&mut data)?;
    assert!(evaluate_expression(&exp, &tdc));

    let mut data = r#"$city =* chars(b*) "#;
    let exp = WarpConditionParser::exp(&mut data)?;
    assert!(!evaluate_expression(&exp, &tdc));

    let mut data = r#"$score =* digit(90) "#;
    let exp = WarpConditionParser::exp(&mut data)?;
    assert!(evaluate_expression(&exp, &tdc));

    let mut data = r#"$score >= digit(90) "#;
    let exp = WarpConditionParser::exp(&mut data)?;
    assert!(evaluate_expression(&exp, &tdc));

    let mut data = r#"$score <= digit(90) "#;
    let exp = WarpConditionParser::exp(&mut data)?;
    assert!(evaluate_expression(&exp, &tdc));

    Ok(())
}

fn test_crate() -> DataRecord {
    let tdo = vec![
        DataField::from_ip("IP", std::net::IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))),
        DataField::from_chars("city", "changsha"),
        DataField::from_digit("score", 90),
    ];
    DataRecord::from(tdo)
}

#[test]
pub fn test_express_exec_logic() -> ModalResult<()> {
    let tdc = test_crate();
    let mut data = r#"$IP > ip(127.0.0.0) && $IP < ip(127.0.0.2)"#;
    let exp = WarpConditionParser::exp(&mut data)?;
    assert!(evaluate_expression(&exp, &tdc));

    let mut data = r#"$IP > ip(127.0.0.2) || $CITY  == chars(changsha) "#;
    let exp = WarpConditionParser::exp(&mut data)?;
    assert!(!evaluate_expression(&exp, &tdc));

    let mut data = r#"( $IP > ip(127.0.0.0) && $IP < ip(127.0.0.2) && $city == chars(bj) ) || $city =* chars(c*a)"#;
    let exp = WarpConditionParser::exp(&mut data)?;
    println!("{}", exp);
    assert!(evaluate_expression(&exp, &tdc));

    Ok(())
}
#[test]
pub fn debug_test() -> ModalResult<()> {
    let tdc = test_crate();
    let mut data = r#"( $IP > ip(127.0.0.0) && $IP < ip(127.0.0.2) && $city == chars(bj) ) || $city =* chars(c*a)"#;
    let exp = WarpConditionParser::exp(&mut data)?;
    println!("{}", exp);
    assert!(evaluate_expression(&exp, &tdc));

    Ok(())
}

#[test]
pub fn test_isset_with_spaces_eval() -> ModalResult<()> {
    // isset($var) with flexible spaces
    let mut code = r#"isset ( $FLAG )"#;
    let expr = WarpConditionParser::exp(&mut code)?;

    let mut rec = DataRecord::default();
    assert!(!evaluate_expression(&expr, &rec)); // missing

    rec.items.push(DataField::from_chars("FLAG", "yes").into());
    assert!(evaluate_expression(&expr, &rec)); // exists
    Ok(())
}
