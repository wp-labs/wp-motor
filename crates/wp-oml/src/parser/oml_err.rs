#[cfg(test)]
mod tests {
    use crate::parser::oml_parse_raw;
    use wp_parser::Parser;
    use wp_parser::WResult as ModalResult;
    use wpl::parser::error::WplCodeError;
    use wpl::parser::error::WplCodeReason;

    #[test]
    fn test_report_err() -> ModalResult<()> {
        let mut code = r#"
name : test
---
version      :chrs   = chars(1.0.0) ;
pos_sn       :chars   = take () ;
update_time  :time    = take () { _ :  time(2020-10-01 12:30:30) };
    "#;
        report_err(&mut code, "chrs");
        let mut code = r#"
name : test
---
version      :chars   = char(1.0.0) ;
pos_sn       :chars   = take () ;
update_time  :time    = take () { _ :  time(2020-10-01 12:30:30) };
    "#;
        report_err(&mut code, "char(1.0.0)");
        let mut code = r#"
name : test
---
version      :chars   = chars(1.0.0) ;
pos_sn       chars   = take () ;
update_time  :time    = take () { _ :  time(2020-10-01 12:30:30) };
    "#;
        report_err(&mut code, "chars   = ");

        let mut code = r#"
name : test
---
version      :chars   = chars(1.0.0) ;
pos_sn       : chars   = take ( hello: ;
update_time  :time    = take () { _ :  time(2020-10-01 12:30:30) };
    "#;
        report_err(&mut code, "( hello:");

        let mut code = r#"
name : test
---
pos_sn    : chars   = take () ;
version   : chars   = chars(1.0.0) ;
update_time  :time    = take () { :  time(2020-10-01 12:30:30) };
    "#;
        report_err(&mut code, ":  time");
        Ok(())
    }
    #[test]
    fn test_match_err() {
        let mut code = r#"
name : test
---
pos_sn    : chars   = take () ;
x   : chars = match  take() {
    chars(bj) => take(a) ;
    chars(cs) => take(b) ;
}
    "#;
        report_err(&mut code, "");

        let mut code = r#"
name : test
---
pos_sn    : chars   = take () ;
x   : chars = match  take() {
    cha(bj) => take(a) ;
    chars(cs) => take(b) ;
} ;
    "#;
        report_err(&mut code, "cha(bj)");

        let mut code = r#"
name : test
---
x   : chars = match  take() {
    chars(bj) => chars2(a) ;
    chars(cs) => take(b) ;
} ;
    "#;
        report_err(&mut code, "chars2");
    }

    #[test]
    fn test_match_wild() {
        let mut code = r#"
name : test
---
pos_sn    : chars   = take () ;
aler*     : auto    = take () ;
time1     : auto    = Time::now() ;
time*     : auto    = Time::now() ;
    "#;
        report_err(&mut code, "Time::now()");
    }

    #[test]
    fn test_match2_err() -> ModalResult<()> {
        let mut code = r#"
        name : test
        ---
        x : auto =  match ( take(city1), take(city2) ) {
        in (ip(127.0.0.1),   ip(127.0.0.100)) => chars(bj),
        (ip(127.0.0.100), ip(127.0.0.200))    => chars(bj),
        (ip(127.0.0.200),  ip(127.0.0.255))   => chars(cs),
        _ => chars(sz),
        };
       "#;
        report_err(&mut code, "in (");

        let mut code = r#"
        name : test
        ---
        x : auto =  match ( take(city1), take(city2)  {
        (ip(127.0.0.1),   ip(127.0.0.100)) => chars(bj),
        (ip(127.0.0.100), ip(127.0.0.200))    => chars(bj),
        (ip(127.0.0.200),  ip(127.0.0.255))   => chars(cs),
        _ => chars(sz),
        };
       "#;
        report_err(&mut code, "( take(city1)");

        let mut code = r#"
                name : test
                ---
                x : auto =  match ( take(city1), take(city2))
                (ip(127.0.0.1),   ip(127.0.0.100)) => chars(bj),
                (ip(127.0.0.100), ip(127.0.0.200))    => chars(bj),
                (ip(127.0.0.200),  ip(127.0.0.255))   => chars(cs),
                _ => chars(sz),
                };
               "#;
        report_err(&mut code, "(ip(127.0.0.1)");

        let mut code = r#"
                name : test
                ---
                X : chars =  match (take(ip),take(key1) ) {
                        (in (ip(10.0.0.1), ip(10.0.0.10)), chars(A) ) => take(city1) ;
                        ( ip(10.0.10.1), chars(B) )  => take(city2) ;
                        ( _ ) => chars(bj) ;
                };
                "#;
        report_err(&mut code, "( _ )");
        let mut code = r#"
                name : example/simple
                ---
                recv_time   = read() ;
                occur_time =  Time::now() ;
                from_ip    =  take(option:[from-ip]) ;
                src_ip     =  rad(option:[src-ip,sip,source-ip] );
                src_city   = query lib(geo) where read(src_ip) {
                        idx : src_ip,
                        col : city_name,
                        _  : chars(unknow)
                };

                from_zone =  query lib(zone) where read(src_ip) {
                        idx : src_ip,
                        col : zone,
                        _  : chars(unknow)
                };
        "#;

        report_err(&mut code, "rad");

        Ok(())
    }
    fn report_err(code: &mut &str, pos: &str) {
        match oml_parse_raw.parse_next(code).map_err(|e| {
            WplCodeError::from(WplCodeReason::Syntax("".into()))
                .with_detail(e.to_string())
                .with_position(code.to_string())
        }) {
            Err(e) => {
                println!("-----");
                println!("{}", e);

                if let Some(x) = e.position().clone() {
                    assert!(x.contains(pos), "position error: {}", pos);
                }
                //e.reason.
            }
            Ok(obj) => {
                println!("~~~~~~~~expect error, but not report ~~~~~~~~~~");
                println!("{}", obj);
                panic!("not found error");
            }
        }
    }
}
