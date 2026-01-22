use crate::ast::WplExpress;
use crate::parser::utils::is_sep_next;
use crate::parser::wpl_group::wpl_group;
use crate::parser::wpl_rule;
use crate::types::AnyResult;
use anyhow::anyhow;
use winnow::ascii::multispace0;
use winnow::combinator::{cut_err, delimited, opt};
use winnow::token::literal;
use wp_parser::Parser;
use wp_parser::WResult;
use wp_parser::symbol::ctx_desc;

use super::wpl_anno::ann_fun;
//parentheses

pub fn wpl_express(input: &mut &str) -> WResult<WplExpress> {
    let mut rule = WplExpress::default();
    if let Some(mut pipe) = opt(wpl_rule::pip_proc).parse_next(input)? {
        rule.pipe_process.append(&mut pipe);
    }
    loop {
        wpl_group
            .context(ctx_desc("group"))
            .map(|x| rule.group.push(x))
            .parse_next(input)?;
        if !is_sep_next(input) {
            break;
        }
    }
    Ok(rule)
}

pub(crate) fn segment(input: &mut &str) -> WResult<WplExpress> {
    let tags = opt(ann_fun).parse_next(input)?;
    let mut define = delimited(
        (multispace0, literal('{'), multispace0),
        cut_err(wpl_express),
        (multispace0, literal('}'), multispace0),
    )
    .parse_next(input)?;
    define.tags = tags;
    Ok(define)
}

pub fn source_segment(code: &str) -> AnyResult<WplExpress> {
    segment
        .parse(code)
        .map_err(|e| anyhow!("parse source_prefix error: {:?}", e))
}

/*
fn wpl_codes(input: &mut &str) -> ModalResult<Vec<WPLPackage>> {
    let multi_package = opt(repeat(1.., wpl_package)).parse_next(input)?;
    if let Some(packages) = multi_package {
        return Ok(packages);
    }
    let mut default_package = WPLPackage::new("/", vec![]);
    wpl_pkg_body(&mut default_package).parse_next(input)?;
    Ok(vec![default_package])
}

 */

#[cfg(test)]
mod tests {
    use smol_str::SmolStr;
    use winnow::LocatingSlice;

    use super::*;
    use crate::ast::fld_fmt::WplFieldFmt;
    use crate::ast::{WplField, WplPackage};
    use crate::parser::error::WplCodeError;
    use crate::parser::wpl_pkg::{wpl_package, wpl_pkg_body};
    use crate::parser::wpl_rule::pip_proc;
    use crate::types::AnyResult;
    use orion_error::{ErrorOwe, TestAssert};
    use wp_model_core::model::DataType;

    #[test]
    fn test_package() -> Result<(), WplCodeError> {
        let input = r#"    package test {
                rule test { (digit<<,>>,digit,time_3339:recv_time,5*_) }
        }
    "#;

        assert_eq!(
            wpl_package
                .parse(&LocatingSlice::new(input))
                .owe_conf()?
                .to_string(),
            r#"package test {
  rule test {
    (
      digit<<,>>,
      digit,
      time_3339:recv_time,
      5*_
    )
  }
}
"#
        );

        let data = r#"
    package test {
            rule /service/for_test/wplab_1 {
                (digit<<,>>,digit,time_3339:recv_time,5*_),
                (digit:id,digit:len,time,sn,chars:dev_name,time,kv,sn,chars:dev_name,time,time,ip,kv,chars,kv,kv,chars,kv,kv,chars,chars,ip,chars,http/request<[,]>,http/agent")
            }
    }
        "#;

        assert_eq!(
            wpl_package.parse(data).assert().to_string(),
            r#"package test {
  rule /service/for_test/wplab_1 {
    (
      digit<<,>>,
      digit,
      time_3339:recv_time,
      5*_
    ),
    (
      digit:id,
      digit:len,
      time,
      sn,
      chars:dev_name,
      time,
      kv,
      sn,
      chars:dev_name,
      time,
      time,
      ip,
      kv,
      chars,
      kv,
      kv,
      chars,
      kv,
      kv,
      chars,
      chars,
      ip,
      chars,
      http/request<[,]>,
      http/agent"
    )
  }
}
"#
        );

        let data = r#"
    package test {
            rule /service/for_test/wplab_1 {
                (time_3339:recv_time,5*_)\!\|
            }
            rule /service/for_test/wplab_2 {
                (time_3339:recv_time,5*_)
            }
    }
        "#;

        let result = wpl_package.parse(data).assert();
        assert_eq!(
            result.to_string(),
            r#"package test {
  rule /service/for_test/wplab_1 {
    (
      time_3339:recv_time,
      5*_
    )\!\|
  }
  rule /service/for_test/wplab_2 {
    (
      time_3339:recv_time,
      5*_
    )
  }
}
"#
        );
        Ok(())
    }
    #[test]
    fn test_parse_block2() {
        let data = r#"(kv(digit@message_type),kv(chars@serial_num))\!\|"#;
        let result = wpl_express.parse(data).assert();
        assert_eq!(
            result.to_string(),
            r#"  (
    kv(digit@message_type),
    kv(@serial_num)
  )\!\|"#,
        );
    }

    #[test]
    fn test_parse_block() {
        let data = "(kv(digit@message_type),chars<skyeye_abnormal {,|>,kv(chars@serial_num),kv(time@access_time),kv(@type),kv(ip@sip),kv(digit@sport),kv(ip@dip),kv(digit@dport),kv(chars@data),kv(digit@datalen),kv(chars@info),kv(chars@vendor_id),kv(ip@device_ip),chars<},|>)";
        assert_eq!(
            wpl_express.parse(data).assert().to_string(),
            r#"  (
    kv(digit@message_type),
    chars<skyeye_abnormal {,|>,
    kv(@serial_num),
    kv(time@access_time),
    kv(@type),
    kv(ip@sip),
    kv(digit@sport),
    kv(ip@dip),
    kv(digit@dport),
    kv(@data),
    kv(digit@datalen),
    kv(@info),
    kv(@vendor_id),
    kv(ip@device_ip),
    chars<},|>
  )"#,
        );

        let data = r#"(json(_@_origin,_@payload/packet_data))"#;
        assert_eq!(
            wpl_express.parse(data).assert().to_string(),
            "  (
    json(_@_origin,_@payload/packet_data,)
  )"
        );
    }

    #[test]
    fn test_pip_proc() {
        assert_eq!(
            pip_proc.parse_peek("|decode/base64|"),
            Ok(("", vec![SmolStr::from("decode/base64")]))
        );

        assert_eq!(
            pip_proc.parse_peek("|decode/hex|"),
            Ok(("", vec![SmolStr::from("decode/hex")]))
        );

        assert_eq!(
            pip_proc.parse_peek("|unquote/unescape|"),
            Ok(("", vec![SmolStr::from("unquote/unescape")]))
        );
        assert_eq!(
            pip_proc.parse_peek("|decode/base64|zip|"),
            Ok((
                "",
                vec![SmolStr::from("decode/base64"), SmolStr::from("zip")]
            ))
        );
        assert_eq!(
            pip_proc.parse_peek("|decode/base64|zip |"),
            Ok((
                "",
                vec![SmolStr::from("decode/base64"), SmolStr::from("zip")]
            ))
        );
        assert_eq!(
            pip_proc.parse_peek("|   base64  |zip |"),
            Ok(("", vec![SmolStr::from("base64"), SmolStr::from("zip")]))
        );
        assert_eq!(
            pip_proc.parse_peek("|   base      |"),
            Ok(("", vec![SmolStr::from("base")]))
        );
        assert_eq!(
            pip_proc
                .parse(&LocatingSlice::new("| !!!|"))
                .err()
                .unwrap()
                .offset(),
            2
        );
        assert_eq!(
            pip_proc
                .parse(&LocatingSlice::new("|"))
                .err()
                .unwrap()
                .offset(),
            1
        );
        assert_eq!(
            pip_proc
                .parse(&LocatingSlice::new("|2222 |34333| 444   "))
                .err()
                .unwrap()
                .offset(),
            20
        );
    }

    #[test]
    fn test_conf_map() -> AnyResult<()> {
        let data = r#"(json(base64@a:x,@b:y))"#;
        let conf = wpl_express.parse(data).assert();
        let map = conf.group[0].fields[0].sub_fields.as_ref().unwrap();
        let expect = WplField {
            name: Some("x".into()),
            meta_name: "base64".into(),
            meta_type: DataType::Base64,
            desc: "base64:x".to_string(),
            fmt_conf: WplFieldFmt {
                //separator: PrioSep::infer_low("0"),
                //patten_first: Some(true),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(map.get("a"), Some(&expect));
        Ok(())
    }

    #[test]
    fn test_conf_vec() {
        let data = "(ip,ip)";
        wpl_group.parse(data).assert();
        wpl_group.parse("(http/method,ip)").assert();
        wpl_group.parse("(*ip,ip:src)").assert();

        let group = wpl_group.parse("(*ip,ip:src)[100]\\,").assert();
        group
            .fields
            .iter()
            .for_each(|x| assert_eq!(x.separator, None));
        assert!(group.base_group_sep.is_some());

        let data = "(chars<-[,]*>)";
        let group = wpl_group.parse(data).assert();
        assert_eq!(group.fields[0].fmt_conf.scope_beg, Some("-[".to_string()));
        assert_eq!(group.fields[0].fmt_conf.scope_end, Some("]*".to_string()));

        wpl_group.parse("(chars<http://,/>)").assert();
        wpl_group.parse("(chars<http://,/>)").assert();
        wpl_group.parse("\n(\nip,\nip\n)").assert();
    }

    #[test]
    fn test_rules() -> AnyResult<()> {
        let data = r#" rule wparse_1 { |decode/base64|zip|unquote/unescape|(digit,time) }"#;
        wpl_pkg_body(&mut WplPackage::default())
            .parse(data)
            .assert();

        let data = r#"
         rule wparse_1 { |base64|zip|(digit,time) }

        rule wparse_2 { |base64|zip|(digit,time) } "#;
        let mut package = WplPackage::default();
        wpl_pkg_body(&mut package).parse(data).assert();
        assert_eq!(package.rules.len(), 2);
        Ok(())
    }

    /*
        #[test]
        fn test_muti_package() {
            let data = r#"
    package test {
            rule /service/for_test/wplab_1 {
                (digit<<,>>,digit,time_3339:recv_time2,5*_),
                (digit:id,digit:len,time,sn,chars:dev_name,time,kv,sn,chars:dev_name,time,time,ip,kv,chars,kv,kv,chars,kv,kv,chars,chars,ip,chars,http/request<[,]>,http/agent")\,
            }
    }

    package test1 {
            rule /service/for_test/wplab_1 {
                (digit<<,>>,digit,time_3339:recv_time2,5*_),
                (digit:id,digit:len,time,sn,chars:dev_name,time,kv,sn,chars:dev_name,time,time,ip,kv,chars,kv,kv,chars,kv,kv,chars,chars,ip,chars,http/request<[,]>,http/agent")\,
            }
    }
        "#;

            let packages = wpl_codes.parse(data).assert();
            assert_eq!(packages.len(), 2);
        }
    */
    #[test]
    fn test_package_annotation1() {
        let data = r#"
#[tag(t1:"id",t2:"sn"),copy_raw(hello:"ll")]
package test {
        rule /service/for_test/wplab_1 {
            (digit<<,>>,digit,time_3339:recv_time2,5*_),
            (digit:id,digit:len,time,sn,chars:dev_name,time,kv,sn,chars:dev_name,time,time,ip,kv,chars,kv,kv,chars,kv,kv,chars,chars,ip,chars,http/request<[,]>,http/agent")\,
        }
}
    "#;

        let expect = r#"#[tag(t1:"id", t2:"sn"), copy_raw(hello:"ll")]
package test {
  #[tag(t1:"id", t2:"sn"), copy_raw(hello:"ll")]
  rule /service/for_test/wplab_1 {
    (
      digit<<,>>,
      digit,
      time_3339:recv_time2,
      5*_
    ),
    (
      digit:id,
      digit:len,
      time,
      sn,
      chars:dev_name,
      time,
      kv,
      sn,
      chars:dev_name,
      time,
      time,
      ip,
      kv,
      chars,
      kv,
      kv,
      chars,
      kv,
      kv,
      chars,
      chars,
      ip,
      chars,
      http/request<[,]>,
      http/agent"
    )\,
  }
}
"#;

        let packages = wpl_package.parse(data).assert();
        assert_eq!(packages.to_string(), expect);
    }

    #[test]
    fn test_annotation2() {
        let data = r#"
#[tag(t1:"id",t2:"sn"),copy_raw(name:"ok")]
package test {
        #[tag(t1:"id",t3:"sn2"),copy_raw(name:"yes")]
        rule /service/for_test/wplab_1 {
            (digit<<,>>,digit,time_3339:recv_time2,5*_),
            (digit:id,digit:len,time,sn,chars:dev_name,time,kv,sn,chars:dev_name,time,time,ip,kv,chars,kv,kv,chars,kv,kv,chars,chars,ip,chars,http/request<[,]>,http/agent")\,
        }
}
    "#;

        let expect = r#"#[tag(t1:"id", t2:"sn"), copy_raw(name:"ok")]
package test {
  #[tag(t1:"id", t2:"sn", t3:"sn2"), copy_raw(name:"yes")]
  rule /service/for_test/wplab_1 {
    (
      digit<<,>>,
      digit,
      time_3339:recv_time2,
      5*_
    ),
    (
      digit:id,
      digit:len,
      time,
      sn,
      chars:dev_name,
      time,
      kv,
      sn,
      chars:dev_name,
      time,
      time,
      ip,
      kv,
      chars,
      kv,
      kv,
      chars,
      kv,
      kv,
      chars,
      chars,
      ip,
      chars,
      http/request<[,]>,
      http/agent"
    )\,
  }
}
"#;

        let packages = wpl_package.parse(data).assert();
        assert_eq!(packages.to_string(), expect);
    }

    #[test]
    fn test_annotation3() {
        let data = r#"
#[tag(t1:"id")]
package test {
        #[tag(t1:"hello",t3:"sn2"),copy_raw(hello:"ll")]
        rule /service/for_test/wplab_1 {
            (digit<<,>>,digit,time_3339:recv_time2,5*_),
            (digit:id,digit:len,time,sn,chars:dev_name,time,kv,sn,chars:dev_name,time,time,ip,kv,chars,kv,kv,chars,kv,kv,chars,chars,ip,chars,http/request<[,]>,http/agent")\,
        }
}
    "#;

        let expect = r#"#[tag(t1:"id")]
package test {
  #[tag(t1:"hello", t3:"sn2"), copy_raw(hello:"ll")]
  rule /service/for_test/wplab_1 {
    (
      digit<<,>>,
      digit,
      time_3339:recv_time2,
      5*_
    ),
    (
      digit:id,
      digit:len,
      time,
      sn,
      chars:dev_name,
      time,
      kv,
      sn,
      chars:dev_name,
      time,
      time,
      ip,
      kv,
      chars,
      kv,
      kv,
      chars,
      kv,
      kv,
      chars,
      chars,
      ip,
      chars,
      http/request<[,]>,
      http/agent"
    )\,
  }
}
"#;

        let packages = wpl_package.parse(data).assert();
        assert_eq!(packages.to_string(), expect);
    }
}
