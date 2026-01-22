#[cfg(test)]
mod tests {
    use crate::eval::runtime::vm_unit::WplEvaluator;
    use crate::parser::parse_code::wpl_express;
    use crate::types::AnyResult;
    use orion_error::TestAssert;
    use wp_parse_api::RawData;
    use wp_parser::Parser;

    #[test]
    fn test_err_1() {
        let rule = r#"(ip,_,_,time<[,]>)"#;
        let data = r#"192.168.1 - - [06/Aug/2019:12:12:19 +0800] "#;
        report_err(rule, data);

        let rule = r#"(ip,_,_,time<[,]>)"#;
        let data = r#"localhos - - [06/Aug/2019:12:12:19 +0800] "#;
        report_err(rule, data);

        let rule = r#"(ip,_,_,time<[,]>)"#;
        let data = r#"localhost - - [06/Ast/2019:12:12:19 +0800] "#;
        report_err(rule, data);
    }
    #[test]
    fn test_err_2() {
        let rule = r#"(ip,_,_,time<[,]>)\,"#;
        let data = r#"localhost - - [06/Ast/2019:12:12:19 +0800] "#;
        report_err(rule, data);
        let rule = r#"(ip,_,_,time<[,]>)"#;
        let data = r#"localhost - - 06/Ast/2019:12:12:19 +0800] "#;
        report_err(rule, data);

        let data = r#"localhost - - [06/Ast/2019:12:12:19 +0800 "#;
        report_err(rule, data);

        let data = r#"localhost - - "[06/Ast/2019:12:12:19 +0800] "#;
        report_err(rule, data);
    }
    #[test]
    fn test_err_3() {
        let rule = r#"(kv(time@curr<[,]>))"#;
        let data = r#"curr: [06/Ast/2019:12:12:19 +0800] "#;
        report_err(rule, data);
    }
    #[test]
    fn test_ok_1() {
        let data = r#" "聊城市", 36.4837, 115.983, 3733321295, 3733321295"#;
        let rule =
            r#"(chars:city_name",float:latitude,float:longitude,digit:ip_beg,digit:ip_end)\,"#;
        report_err(rule, data);
    }
    #[test]
    fn test_ok_2() {
        let data = r#"192.168.1.1,200"#;
        let rule = r#"opt(ip:src)\,,(digit)"#;
        report_err(rule, data);

        let data = r#"200"#;
        let rule = r#"opt(ip:src)\,,(digit)"#;
        report_err(rule, data);

        let data = r#"192.168.1.1"#;
        let rule = r#"(ip:src)\,,opt(ip)"#;
        report_err(rule, data);

        let data = r#"192.168.1.1,192.168.1.2"#;
        let rule = r#"(ip:src)\,,opt(ip)"#;
        report_err(rule, data);
    }
    #[test]
    fn test_ok_3() {
        let data = r#"{ "data": "192.168.1.1" }"#;
        let rule = r#"(json( chars@data ))"#;
        report_err(rule, data);

        let data = r#"{ "data": "192.168.1.1" }"#;
        let rule = r#"(json( opt(ip)@data  ))"#;
        report_err(rule, data);

        let data = r#"{ "data": "" }"#;
        let rule = r#"(json( opt(ip)@data  ))"#;
        report_err(rule, data);

        let data = r#"data: "192.168.1.2" "#;
        let rule = r#"(kv( opt(ip)@data  ))"#;
        report_err(rule, data);

        let data = r#"data: "192.158" "#;
        let rule = r#"(kv( opt(ip)@data  ))"#;
        report_err(rule, data);
    }

    #[test]
    fn test_case_1() -> AnyResult<()> {
        let wpl = r#"(digit:id,digit:len,time,sn,chars:dev-name,time,kv,sn,chars:dev-name,time,time,ip,kv,chars,kv,kv,chars,kv,kv,chars,chars,ip,chars,http/request<[,]>,http/agent")\,
            "#;
        let data = r#"1407,509,2021-4-20 18:10:19,WCY7-ZT-QEAK-N6PD,ByHJpEtscumFff6FNLLjoFwMsOjVRWHMxxFT56NxfmktY1ASgo,2022-4-4 21:0:13,Tv7=9WxLPktFSMRBH4WRUCiBkmh2swZLod,DQGB-NL-RY2X-0SFD,cqIZXVT8FtAYrrlKI7q2CKL0D69Cg5jgbtnzzaJnUcUusZBIF5,2020-11-8 10:58:21,2022-4-13 14:27:12,111.237.105.120,TeG=ro1WpYpimAoG0n182NqwpkRvX2Xfod,q9gZeTkIxlCoGrAEUNqHhG17CT4OKebKXC0Ze5iXiyi2JYYnwc,hnB=FEdOhmFkM6SxBwiy3ATZePyBJBK5TT,YUC=X9JVE4p4WCNRwNjIdJ8mwnjLzs9fTY,Cmvp92V96paAHM8L60NzWl93AUHSR3WdxriwHmUDDxVohd8NcI,gtd=5srrDgB8YZMipedJ60jpl99HQg2SZR,8Ju=I1C1RzlgmX3IlS9Vp2hLsQWiudvZqz,uVAx1yArjlE1suY3887oCA44dWbm2MNZykeAqCwiq2KJbZlais,3ERd33ADEIKXISZLYWJx8juR455t753fybdcypXE2akn4KqITx,83.213.168.46,tzZ6oyqEA9ffm1e1Pi96344C6HVlw9zti4LWhBd0z9gStkFDuw,[GET /index  HTTP/1.1 ],"Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/70.0.3538.77 Safari/537.36""#;
        let express = wpl_express.parse(wpl).assert();
        let lpp = WplEvaluator::from(&express, None).assert();
        let raw = RawData::from_string(data.to_string());
        let (tdc, _) = lpp.proc(raw, 0)?;
        println!("{}", tdc);
        Ok(())
    }
    #[test]
    fn test_gen_1() -> AnyResult<()> {
        let data = r#" 222.133.52.20 - - [06/Aug/2019:12:12:19 +0800] "GET /nginx-logo.png HTTP/1.1" 200 368 "http://119.122.1.4/" "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_14_5) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/75.0.3770.142 Safari/537.36" "-""#;
        let wpl = r#"(ip:sip,2*_,time:recv_time<[,]>,http/request",http/status,digit,chars",http/agent",_")"#;
        let express = wpl_express.parse(wpl).assert();
        let lpp = WplEvaluator::from(&express, None).assert();
        let raw = RawData::from_string(data.to_string());
        let (tdc, _) = lpp.proc(raw, 0)?;
        println!("{}", tdc);
        Ok(())
    }
    fn report_err(rule: &str, data: &str) {
        let express = wpl_express.parse(rule).assert();
        let ppl = WplEvaluator::from(&express, None).assert();

        let raw = RawData::from_string(data.to_string());
        let result = ppl.proc(raw, 0);

        match result {
            Err(e) => {
                println!("-----");
                println!("{}", e);
            }
            Ok(v) => {
                println!("{}", v.0);
            }
        }
    }
}
