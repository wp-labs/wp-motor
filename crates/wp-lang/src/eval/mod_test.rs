#[cfg(test)]
mod tests {
    use crate::eval::runtime::vm_unit::WplEvaluator;
    use crate::parser::parse_code::wpl_express;
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
    fn test_alt_err() {
        let rule = r#"alt(ip,digit)"#;
        let data = r#"hello"#;
        report_err(rule, data);
    }

    #[test]
    fn test_suc_1() {
        let rule = r#"(kv(time<[,]>@curr))"#;
        let data = r#"curr: [06/Ast/2019:12:12:19 +0800] "#;
        assert_suc(rule, data);
    }
    #[test]
    fn test_json_symbol_miss() {
        let rule = r#"(json(symbol(boy2)@key)) "#;
        let data = r#"{"key":  "boy"}"#;
        report_err(rule, data);
    }
    #[test]
    fn test_suc_2() {
        let data = r#" "聊城市", 36.4837, 115.983, 3733321295, 3733321295"#;
        let rule =
            r#"(chars:city_name",float:latitude,float:longitude,digit:ip_beg,digit:ip_end)\,"#;
        assert_suc(rule, data);
    }

    #[test]
    fn test_peek_symbol_suc_1() {
        let rule = r#"(peek_symbol(curr),kv(time<[,]>@curr))"#;
        let data = r#"curr: [06/Ast/2019:12:12:19 +0800] "#;
        assert_suc(rule, data);
    }

    #[test]
    fn test_peek_symbol_suc_2() {
        let rule = r#"(peek_symbol({"sys": "unix"),json)"#;
        let data = r#"{"sys": "unix" ,"key":  "hello boy"}"#;
        assert_suc(rule, data);
    }

    #[test]
    fn test_exact_json_suc_1() {
        let rule = r#"(exact_json(@sys,@key))"#;
        let data = r#"{"sys": "unix" ,"key":  "hello boy"}"#;
        assert_suc(rule, data);
    }
    #[test]
    fn test_exact_json_err() {
        let rule = r#"(exact_json(@sys))"#;
        let data = r#"{"sys": "unix" ,"key":  "hello boy"}"#;
        report_err(rule, data);
    }

    #[test]
    fn test_suc_5() {
        let rule = r#"(kv(time@fist_time),kv(time@last_time),kv)"#;
        let data = r#"fist_time=2023-10-11 11:30:26 last_time=2023-10-11 11:30:26 tally=1"#;
        assert_suc(rule, data);
    }
    #[test]
    fn test_err_peek_symbol() {
        let rule = r#"(peek_symbol({"sys":"unix"),json)"#;
        let data = r#"{"sys": "unix" ,"key":  "hello boy"}"#;
        report_err(rule, data);
    }
    fn report_err(rule: &str, data: &str) {
        let express = wpl_express.parse(rule).assert();
        let ppl = WplEvaluator::from(&express, None).assert();
        let raw = RawData::from_string(data.to_string());
        let result = ppl.proc(0, raw, 0);

        if let Err(e) = result {
            println!("-----");
            println!("{}", e);
        }
    }

    fn assert_suc(rule: &str, data: &str) {
        let express = wpl_express.parse(rule).assert();
        let ppl = WplEvaluator::from(&express, None).assert();
        let raw = RawData::from_string(data.to_string());
        let result = ppl.proc(0, raw, 0);

        match result {
            Ok((o, _)) => {
                println!("{}", o);
            }
            Err(e) => {
                println!("-----");
                println!("{}", e);
                panic!("proc fail!");
            }
        }
    }
}
