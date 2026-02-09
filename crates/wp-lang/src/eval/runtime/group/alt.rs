use crate::WplSep;
use crate::ast::group::GroupAlt;
use crate::eval::runtime::group::{LogicProc, WplEvalGroup};
use winnow::stream::Stream;
use wp_log::trace_edata;
use wp_model_core::model::DataField;
use wp_parser::WResult as ModalResult;

impl LogicProc for GroupAlt {
    fn process(
        &self,
        e_id: u64,
        group: &WplEvalGroup,
        ups_sep: &WplSep,
        data: &mut &str,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()> {
        alt_proc(e_id, group, ups_sep, data, out)
    }
}

pub fn alt_proc(
    e_id: u64,
    group: &WplEvalGroup,
    ups_sep: &WplSep,
    data: &mut &str,
    out: &mut Vec<DataField>,
) -> ModalResult<()> {
    let mut last_err = None;
    let mut min_left_len = data.len();
    let cur_sep = group.combo_sep(ups_sep);
    for fpu in group.field_units.iter() {
        if data.is_empty() {
            break;
        }
        let ck_point = data.checkpoint();
        match fpu.parse(e_id, &cur_sep, data, Some(fpu.conf().safe_name()), out) {
            Ok(_) => {
                return Ok(());
            }
            Err(e) => {
                trace_edata!(e_id, "fpu parse error {} {} \n{}", fpu.conf(), e, data);
                let cur_pos = data.len();
                if cur_pos < min_left_len || last_err.is_none() {
                    last_err = Some(e);
                    min_left_len = cur_pos;
                }
                data.reset(&ck_point);
                continue;
            }
        }
    }
    match last_err {
        None => Ok(()),
        Some(e) => Err(e),
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
    fn test_alt_group() -> AnyResult<()> {
        let express = wpl_express
            .parse(r#"alt(ip:sip,digit:id),(2*_,time<[,]>)"#)
            .assert();
        let mut data = r#"192.168.1.2 - - [06/Aug/2019:12:12:19 +0800] "#;
        let ppl = WplEvaluator::from(&express, None)?;

        let result = ppl.parse_groups(0, &mut data).assert();
        assert_eq!(data, "");
        println!("{}", result);
        assert_eq!(
            result.get_field_owned("sip"),
            Some(DataField::from_ip(
                "sip",
                IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2))
            ))
        );

        let mut data = r#"2002 - - [06/Aug/2019:12:12:19 +0800] "#;
        let result = ppl.parse_groups(0, &mut data).assert();
        assert_eq!(data, "");
        println!("{}", result);
        assert_eq!(
            result.get_field_owned("id"),
            Some(DataField::from_digit("id", 2002))
        );

        let mut data = r#"bad - - [06/Aug/2019:12:12:19 +0800] "#;
        let result = ppl.parse_groups(0, &mut data);
        assert!(result.is_err());
        Ok(())
    }
}
