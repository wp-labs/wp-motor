use super::super::prelude::*;
use smol_str::SmolStr;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use wp_model_core::model::FNameStr;

use crate::derive_base_prs;
use crate::eval::runtime::field::FieldEvalUnit;
use crate::eval::value::parse_def::*;
use crate::generator::ParserValue;
use crate::generator::{FieldGenConf, GenScopeEnum};
use rand::Rng;
use winnow::ascii::digit1;
use winnow::combinator::separated_pair;
use wp_model_core::model::IpNetValue;
use wp_parser::net::ip;

derive_base_prs!(IpPSR);

impl ParserValue<IpAddr> for IpPSR {
    fn parse_value<'a>(data: &mut &str) -> ModalResult<IpAddr> {
        ip.context(ctx_desc("<ip>")).parse_next(data)
    }
}

impl PatternParser for IpPSR {
    fn pattern_parse(
        &self,
        _fpu: &FieldEvalUnit,
        _ups_sep: &WplSep,
        data: &mut &str,
        name: FNameStr,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()> {
        let ip = IpPSR::parse_value(data)?;
        out.push(DataField::from_ip(name, ip));
        Ok(())
    }

    fn patten_gen(
        &self,
        gnc: &mut GenChannel,
        f_conf: &WplField,
        g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<DataField> {
        let range = if let Some(Some(GenScopeEnum::Ip(conf))) = g_conf.map(|c| &c.scope) {
            let beg: u32 = conf.beg.into();
            let end: u32 = conf.end.into();
            beg..end
        } else {
            1000000000..2000000000
        };
        let digit = gnc.rng.random_range(range);
        let ip = Ipv4Addr::from(digit);

        if let Some(Some(fmt)) = g_conf.map(|c| &c.gen_fmt) {
            let mut vals = HashMap::new();
            vals.insert("val".to_string(), ip.to_string());
            match strfmt::strfmt(fmt, &vals) {
                Ok(dat) => {
                    return Ok(DataField::from_chars(
                        f_conf.safe_name(),
                        SmolStr::from(dat),
                    ));
                }
                Err(e) => {
                    error!("gen fmt error: {}", e);
                }
            }
        }

        Ok(DataField::from_ip(f_conf.safe_name(), IpAddr::V4(ip)))
    }
}

derive_base_prs!(IpNetP);
impl ParserValue<IpNetValue> for IpNetP {
    fn parse_value<'a>(data: &mut &str) -> ModalResult<IpNetValue> {
        separated_pair(ip, literal("/"), digit1.try_map(str::parse::<u8>))
            .map(|(ip, mask)| IpNetValue::new(ip, mask).unwrap())
            .parse_next(data)
    }
}

impl PatternParser for IpNetP {
    fn pattern_parse(
        &self,
        _fpu: &FieldEvalUnit,
        _ups_sep: &WplSep,
        data: &mut &str,
        name: FNameStr,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()> {
        let ipnet = IpNetP::parse_value(data)?;
        out.push(DataField::new_opt(
            DataType::IpNet,
            Some(name),
            ipnet.into(),
        ));
        Ok(())
    }

    fn patten_gen(
        &self,
        _gen: &mut GenChannel,
        _f_conf: &WplField,
        _g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<DataField> {
        unimplemented!("ip_net gen");
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    use wp_parser::Parser;

    use crate::ast::WplField;
    use crate::eval::value::parser::base::digit::DigitP;
    use crate::eval::value::parser::network::net::{IpNetP, IpPSR, ip};
    use crate::eval::value::test_utils::{ParserTUnit, verify_parse_v_suc_end};
    use crate::types::AnyResult;
    use orion_error::TestAssert;
    use wp_model_core::model::{DataField, IpNetValue};

    #[test]
    fn test_ip_net() {
        let mut data = "172.0.0.1/24";
        let field = verify_parse_v_suc_end::<IpNetP, IpNetValue>(&mut data);
        assert_eq!(
            field,
            IpNetValue::new("172.0.0.1".parse().assert(), 24).assert()
        )
    }

    #[test]
    fn test_ip_partial() -> AnyResult<()> {
        assert_eq!(
            ip.parse_peek("172.0.0.9"),
            Ok(("", "172.0.0.9".parse().assert()))
        );
        assert_eq!(
            ip.parse_peek("172.0.0.9:80"),
            Ok((":80", "172.0.0.9".parse().assert()))
        );
        assert_eq!(
            ip.parse_peek("2001:db8::1:80"),
            Ok(("", "2001:db8::1:80".parse().assert()))
        );
        assert_eq!(
            ip.parse_peek("2001:db8::1.80"),
            Ok((".80", "2001:db8::1".parse().assert()))
        );

        let mut data = "172.0.0.1:80";
        let conf_ip = WplField::try_parse("ip\\:").assert();
        let field = ParserTUnit::new(IpPSR::default(), conf_ip.clone())
            .verify_parse_suc(&mut data)
            .assert();
        assert_eq!(
            field[0],
            DataField::from_ip("ip", "172.0.0.1".parse().assert())
        );
        assert_eq!(data, "80");

        let conf_port = WplField::try_parse("digit").assert();
        let field = ParserTUnit::new(DigitP::default(), conf_port.clone())
            .verify_parse_suc(&mut data)
            .assert();
        assert_eq!(field[0], DataField::from_digit("digit", 80));
        assert_eq!(data, "");

        let mut data = "[172.0.0.1] [80]";
        let conf_ip = WplField::try_parse("ip<[,]>").assert();
        let field = ParserTUnit::new(IpPSR::default(), conf_ip.clone())
            .verify_parse_suc(&mut data)
            .assert();
        assert_eq!(
            field[0],
            DataField::from_ip("ip", "172.0.0.1".parse().assert())
        );
        assert_eq!(data, "[80]");

        let conf_port = WplField::try_parse("digit<[,]>").assert();
        let field = ParserTUnit::new(DigitP::default(), conf_port.clone())
            .verify_parse_suc(&mut data)
            .assert();
        assert_eq!(field[0], DataField::from_digit("digit", 80));
        assert_eq!(data, "");

        let mut data = "192.168.1.2 - -";
        let conf_ip = WplField::try_parse("ip").assert();
        let field = ParserTUnit::new(IpPSR::default(), conf_ip.clone())
            .verify_parse_suc(&mut data)
            .assert();
        assert_eq!(
            field[0],
            DataField::from_ip("ip", "192.168.1.2".parse().assert())
        );
        assert_eq!(data, "- -");
        Ok(())
    }

    #[test]
    fn test_ip() -> AnyResult<()> {
        let conf_ip = WplField::try_parse("ip").assert();
        let mut data = "192.168.1.2";
        let x = ParserTUnit::new(IpPSR::default(), conf_ip.clone())
            .verify_parse_suc(&mut data)
            .assert();
        assert_eq!(
            x,
            vec![DataField::from_ip(
                "ip",
                IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2)),
            )]
        );

        let mut data = " localhost ";
        let x = ParserTUnit::new(IpPSR::default(), conf_ip.clone())
            .verify_parse_suc(&mut data)
            .assert();
        assert_eq!(
            x,
            vec![DataField::from_ip(
                "ip",
                IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            )]
        );

        Ok(())
    }

    #[test]
    fn test_ip_gen() -> AnyResult<()> {
        let conf_ip = WplField::try_parse("ip").assert();
        ParserTUnit::new(IpPSR::default(), conf_ip).verify_gen_parse_suc();

        let conf_ip = WplField::try_parse("ip<[,]>").assert();
        ParserTUnit::new(IpPSR::default(), conf_ip).verify_gen_parse_suc();

        let conf_ip = WplField::try_parse("ip\"").assert();
        ParserTUnit::new(IpPSR::default(), conf_ip).verify_gen_parse_suc();

        Ok(())
    }

    #[test]
    fn test_ip_2() {
        assert_eq!(
            Ok(("", IpAddr::V6(Ipv6Addr::new(0xff00, 0, 0, 0, 0, 0, 0, 0)))),
            ip.parse_peek("ff00::")
        );
        assert_eq!(
            ip.parse_peek("127.0.0.1"),
            Ok(("", IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)))),
        );
        assert_eq!(
            ip.parse_peek("::1"),
            Ok(("", IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)))),
        );
    }
}
