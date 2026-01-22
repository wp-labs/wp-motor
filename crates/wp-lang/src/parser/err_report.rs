#[cfg(test)]
mod tests {
    use crate::ast::WplPackage;

    fn report_err(rule: &mut &str) {
        let rule = WplPackage::parse(rule, "from_test");
        if let Err(e) = rule {
            println!("-----");
            println!("{}", e);
        }
    }
    fn report_ok(rule: &mut &str) {
        let rule = WplPackage::parse(rule, "from_test");
        if let Err(e) = rule {
            println!("-----");
            println!("{}", e);
            panic!("oml parse failed!")
        }
    }

    #[test]
    fn test_err_1() {
        let mut rule = r#"
        packag pkg {
            rule x {
                (ip,_,_,time<[,]>)
            }
        }"#;
        report_err(&mut rule);

        let mut rule = r#"
        package pkg
            rule x {
                (ip,_,_,time<[,]>)
            }
        }"#;
        report_err(&mut rule);

        let mut rule = r#"
        package pkg {
            ru x {
                (ip,_,_,time<[,]>)
            }
        }"#;
        report_err(&mut rule);
        let mut rule = r#"
        package pkg {
            rule  {
                (ip,_,_,time<[,]>)
            }
        }"#;
        report_err(&mut rule);

        let mut rule = r#"
        package pkg {
            rule  x {
                ip,_,_,time<[,]>)
            }
        }"#;
        report_err(&mut rule);
    }
    #[test]
    fn test_err_2() {
        let mut rule = r#"
        package pkg {
            rule  x {
                (px,_,_,time<[,]>)
            }
        }"#;
        report_err(&mut rule);

        let mut rule = r#"package /example {
   rule nginx {
        (ip:sip,_^,time<[,]>,http/request",http/status,digit,chars",http/agent",_")
    }
}"#;
        report_err(&mut rule);
    }

    #[test]
    fn test_err_3() {
        let mut rule = r#"
        package pkg {
            rule  x {
                (\ip,_,_,time<[,]>)
            }
        }"#;
        report_err(&mut rule);
    }

    #[test]
    fn test_err_4() {
        let mut rule = r#"
        package pkg {
            rule  x {
                (ip\,_,_,time<[,]>)
            }
        }"#;
        report_err(&mut rule);
        let mut rule = r#"
        package pkg {
            rule  x {
                (ip,_,_,time<[,])
            }
        }"#;
        report_err(&mut rule);
    }
    #[test]
    fn test_err_5() {
        let mut rule = r#"
        package pkg {
            rule  x {
                (json(,_,_,time<[,]>)
            }
        }"#;
        report_err(&mut rule);
        let mut rule = r#"
        package pkg {
            rule  x {
                (json(),_,_,time<[,]>)
            }
        }"#;
        report_err(&mut rule);
        let mut rule = r#"
        package pkg {
            rule  x {
                (json(a),_,_,time<[,]>)
            }
        }"#;
        report_err(&mut rule);
    }
    #[test]
    fn test_err_6() {
        let mut rule = r#"
package /test_pkg {
    rule test_rule {
                (_*)
    }
 }
       "#;
        report_err(&mut rule);
    }
    #[test]
    fn test_ok_1() {
        let mut rule = r#"
package /benchmark {
    rule benchmark_2 {
                (ip:src_ip,digit:port,chars:dev_name,ip:dst_ip,digit:port,time",kv,kv,sn,kv,ip,kv,chars,kv,sn,kv,kv,time,chars,time,sn,kv,chars,chars,ip,chars,http/request",http/agent")\,
    }
    rule benchmark_1 {
                (digit:id,digit:len,time,sn,chars:dev_name,time,kv,sn,chars:dev_name,time,time,ip,kv,chars,kv,kv,chars,kv,kv,chars,chars,ip,chars,http/request<[,]>,http/agent")\,
    }
 }
       "#;
        report_ok(&mut rule);
    }

    #[test]
    fn test_ok_2() {
        let mut rule = r#"
package pkg{
    rule x {
        (digit:id<<,>>,time,sn,chars\:), opt(kv\;), (*kv\,)
    }
}
"#;
        report_ok(&mut rule);
    }

    #[test]
    fn test_ok_3() {
        let mut rule = r#"
package pkg{
    rule x {
        opt(ip), opt(kv\;), (*kv\,)
    }
}
"#;
        report_ok(&mut rule);
    }

    #[test]
    fn test_ok_4() {
        let mut rule = r#"
package pkg{
    rule x {
        (json( opt(ip)@a ) )
    }
}
"#;
        report_ok(&mut rule);
    }

    #[test]
    fn test_group_err_1() {
        let mut rule = r#"
package pkg{
    rule x {
        (digit:id<<,>>,time,sn,chars\:), pt(kv\;) , (*kv\,)
    }
}
"#;
        report_err(&mut rule);

        let mut rule = r#"
package pkg{
    rule x {
        (digit:id<<,>>,time,sn,chars\:) opt(kv\;) , (*kv\,)
    }
}
"#;
        report_err(&mut rule);
    }

    #[test]
    fn test_pkg_ok1() {
        let mut rule = CODE_1;
        report_ok(&mut rule);
    }

    const CODE_1: &str = r#"
package /qty {
     #[tag(dat_type: "webids_alert")]
     rule webids_alert {
          (symbol(webids_alert),chars:serialno,chars:rule_id,chars:rule_name,time_timestamp:write_date,chars:vuln_type,ip:sip,digit:sport,ip:dip,digit:dport,digit:severity,chars:host,chars:parameter,chars:uri,chars:filename,chars:referer,chars:method,chars:vuln_desc,time:public_date,chars:vuln_harm,chars:solution,chars:confidence,chars:victim_type,chars:attack_flag,chars:attacker,chars:victim,digit:attack_result,chars:kill_chain,chars:code_language,time:loop_public_date,chars:rule_version,chars:xff,chars:vlan_id,chars:vxlan_id)\|\!
     }
     #[tag(dat_type: "webshell_alert")]
     rule webshell_alert {
          (symbol(webshell_alert),chars:serialno,chars:rule_id,ip:host,chars:uri,chars:file_md5,ip:sip,digit:sport,ip:dip,digit:dport,chars:attack_type,time_timestamp:write_date,digit:severity,chars:attack_desc,chars:attack_harm,digit:confidence,chars:file_dir,chars:victim_type,chars:attack_flag,chars:attacker,chars:victim,digit:attack_result,chars:kill_chain,chars:rule_name,chars:vlan_id,chars:vxlan_id)\|\!
     }
}
"#;
}
