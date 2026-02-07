use crate::WplSep;
use crate::ast::group::GroupOpt;
use crate::eval::runtime::group::{LogicProc, WplEvalGroup};
use winnow::stream::Stream;
use wp_model_core::model::DataField;
use wp_parser::WResult as ModalResult;

impl LogicProc for GroupOpt {
    fn process(
        &self,
        e_id: u64,
        group: &WplEvalGroup,
        ups_sep: &WplSep,
        data: &mut &str,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()> {
        if let Some(fpu) = group.field_units.first() {
            let cur_sep = group.combo_sep(ups_sep);
            let ck_point = data.checkpoint();
            match fpu.parse(e_id, &cur_sep, data, None, out) {
                //match fpu.exec( data) {
                Ok(_) => return Ok(()),
                Err(_e) => data.reset(&ck_point),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::types::AnyResult;
    use crate::{WplEvaluator, wpl_express};
    use orion_error::TestAssert;
    use std::net::{IpAddr, Ipv4Addr};
    use wp_model_core::model::DataField;
    use wp_parser::Parser;
    #[test]
    fn test_opt_group_1() -> AnyResult<()> {
        let express = wpl_express.parse(r#"opt(ip:sip),(2*_,time<[,]>)"#).assert();
        let mut data = r#"192.168.1.2 - - [06/Aug/2019:12:12:19 +0800] "#;
        let ppl = WplEvaluator::from(&express, None)?;
        let result = ppl.parse_groups(0, &mut data).assert();
        assert_eq!(data, "");
        println!("{}", result);
        assert_eq!(
            result.field("sip"),
            Some(&DataField::from_ip(
                "sip",
                IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2))
            ))
        );

        let mut data = r#"- - [06/Aug/2019:12:12:19 +0800] "#;
        let result = ppl.parse_groups(0, &mut data).assert();
        assert_eq!(data, "");
        println!("{}", result);
        Ok(())
    }

    #[test]
    fn test_opt_group_2() -> AnyResult<()> {
        let express = wpl_express.parse(r#"(ip:sip) ,opt(ip:sip)"#).assert();
        let mut data = r#"192.168.1.2"#;
        let ppl = WplEvaluator::from(&express, None)?;
        let result = ppl.parse_groups(0, &mut data).assert();
        assert_eq!(data, "");
        println!("{}", result);
        assert_eq!(
            result.field("sip"),
            Some(&DataField::from_ip(
                "sip",
                IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2))
            ))
        );
        Ok(())
    }
}
