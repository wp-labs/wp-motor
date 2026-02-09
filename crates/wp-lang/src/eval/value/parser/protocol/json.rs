use super::super::prelude::*;
use std::io::Cursor;
use wp_model_core::model::FNameStr;

use crate::eval::runtime::field::FieldEvalUnit;
use crate::eval::value::parse_def::PatternParser;
use crate::eval::value::parser::protocol::json_impl::JsonProc;
use serde::Deserialize;
use serde_json::{Deserializer, Value};

//derive_base_prs!(JsonP);
#[derive(Default)]
pub struct JsonP {}

impl PatternParser for JsonP {
    fn pattern_parse<'a>(
        &self,
        e_id: u64,
        fpu: &FieldEvalUnit,
        ups_sep: &WplSep,
        data: &mut &str,
        name: FNameStr,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()> {
        multispace0.parse_next(data)?;
        let mut cursor = Cursor::new(data.as_bytes());
        let mut deserializer = Deserializer::from_reader(&mut cursor);
        if let Ok(value) = Value::deserialize(&mut deserializer) {
            let json_end = cursor.position() as usize;
            JsonProc::proc_value(e_id, fpu, ups_sep, "", &value, name.as_str(), false, out)?;
            let (_, remaining_text) = data.split_at(json_end);
            *data = remaining_text;
            Ok(())
        } else {
            fail.parse_next(data)
        }
    }

    fn patten_gen(
        &self,
        _gen: &mut GenChannel,
        _f_conf: &WplField,
        _g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<DataField> {
        unimplemented!("json generate")
    }
}

#[cfg(test)]
mod tests {
    use smol_str::SmolStr;
    use wp_data_fmt::{DataFormat, KeyValue, Raw};
    use wp_model_core::model::types::value::ObjectValue;
    use wp_parser::Parser;

    use super::*;
    use crate::eval::value::parser::protocol::json::JsonP;

    use crate::ast::WplSep;
    use crate::eval::runtime::vm_unit::WplEvaluator;
    use crate::eval::value::test_utils::ParserTUnit;
    use crate::parser::wpl_rule::wpl_rule;
    use crate::types::AnyResult;
    use orion_error::TestAssert;

    use orion_overload::conv::*;
    use wp_model_core::model::{DataRecord, DataType};

    use once_cell::sync::Lazy;

    static KV_FMT: Lazy<KeyValue> = Lazy::new(KeyValue::default);
    static RAW_FMT: Lazy<Raw> = Lazy::new(Raw::new);

    #[test]
    fn test_json_std() -> AnyResult<()> {
        let mut data = r#"{"a":1,"b":2}"#;
        let conf = WplField::try_parse("json").assert();
        ParserTUnit::new(JsonP::default(), conf.clone())
            .verify_parse_suc(&mut data)
            .assert();

        let mut data = r#"{"a":1,"b":2,"c": { "a" : 1 } }"#;
        ParserTUnit::new(JsonP::default(), conf.clone())
            .verify_parse_suc(&mut data)
            .assert();
        let mut data = r#"{"a":1,"b":2,"c":  "a" : 1 } }"#;
        ParserTUnit::new(JsonP::default(), conf.clone()).verify_parse_fail(&mut data);
        Ok(())
    }

    #[test]
    fn test_json_base64() -> AnyResult<()> {
        let conf = wpl_rule
            .parse("rule test {(json(base64@a:_a, _@c))}")
            .assert();

        let f_conf = conf.statement.first_field().no_less("first field")?;
        let mut data = r#"{"a":"aGVsbG8=","b":2,"c": "gogogo"}"#;
        let mut out = Vec::new();
        let ups_sep = WplSep::default();
        let fpu = FieldEvalUnit::for_test(JsonP::default(), f_conf.clone());
        fpu.parse(0, &ups_sep, &mut data, None, &mut out).assert();

        Ok(())
    }

    #[test]
    fn test_json_long() -> AnyResult<()> {
        let mut data = r#"{"_origin": {"rsp_status": 200, "sip": "10.180.17.50", "public_date": "2023-02-08 17:11:04", "vuln_name": "疑似访问/下载脚本文件", "detail_info": "发现疑似脚本文件下载行为，该类文件可直接在操作系统上执行，可能存在风险。", "solution": "确认该文件访问/下载行为是否为合规行为；如下载成功，检查所下载文件是否为恶意文件；如是，及时删除该文件并检查其是否被执行，杀死异常进程。", "uri": "/download/distribution/1_20230515/6461b42f12f17.bat", "xff": "", "vuln_harm": "该类文件可直接在操作系统上执行，若下载的是恶意脚本，可能存在较高风险。", "vuln_type": "文件下载", "vuln_desc": "发现疑似脚本文件下载行为，该类文件可直接在操作系统上执行，可能存在风险。", "write_date": 1684127059, "dport": 80, "code_language": "", "sport": 62551, "dip": "10.111.48.17", "site_app": ""}, "alarm_sample": 1, "alarm_sip": "10.180.17.50", "alarm_source": 1, "attack_chain": "0x02050000", "attack_org": "", "attack_sip": "10.111.48.17", "attack_type": "文件下载", "branch_id": "QtAVdJgqi", "file_name": "6461b42f12f17.bat", "first_access_time": "2023-05-15T13:04:19.000+0800", "hazard_level": 4, "hazard_rating": "中危", "host_state": "攻击成功", "ioc": "268572087-疑似访问/下载脚本文件", "is_delete": 0, "is_web_attack": "1", "is_white": 0, "nid": "", "repeat_count": 1, "rule_desc": "网页漏洞利用", "rule_key": "webids", "rule_state": "green", "sip_ioc_dip": "7873ecac0a91ce2b9f96b9f955e82065", "skyeye_id": "", "skyeye_index": "", "skyeye_type": "webids-webattack_dolog", "super_attack_chain": "0x02000000", "super_type": "攻击利用", "type": "文件下载", "type_chain": "16120000", "update_time": 1684127441, "vuln_type": "疑似访问/下载脚本文件", "white_id": null, "x_forwarded_for": "", "rule_labels": "{\"0x110A02\": {\"parent_name\": \"资产识别\", \"name\": \"UA指纹识别\", \"os\": \"Windows/XP\", \"parent_id\": \"0x110A00\", \"role\": \"C\", \"software\": \"IE/6.0\", \"type\": \"模块名称\", \"hw_type\": \"PC\"}}", "att_ck": "初始访问:TA0001|利用面向公众的应用程序:T1190", "serial_num": "QbJK/tb6A", "access_time": "2023-05-15T13:04:19.000+0800", "file_md5": "64f9bfa67c0b89fdb82b0673c1a96964", "host": "10.111.48.17", "rule_id": "0x100215b7", "rsp_status": "200", "skyeye_serial_num": "QbJK/tb6A", "host_md5": "6470cf2c183004f8ead3e150277b4d01", "alarm_id": "20230515_d3b0cca9ea8e9cca348f08ee036103dc", "payload": {"req_header": "R0VUIC9kb3dubG9hZC9kaXN0cmlidXRpb24vMV8yMDIzMDUxNS82NDYxYjQyZjEyZjE3LmJhdCBIVFRQLzEuMQ0KQWNjZXB0OiAqLyoNClVzZXItQWdlbnQ6IE1vemlsbGEvNC4wIChjb21wYXRpYmxlOyBNU0lFIDYuMDsgV2luZG93cyBOVCA1LjE7IFNWMSkNCkhvc3Q6IDEwLjExMS40OC4xNw0KQ29ubmVjdGlvbjogQ2xvc2UNCkNhY2hlLUNvbnRyb2w6IG5vLWNhY2hlDQoNCg==", "rsp_header": "SFRUUC8xLjEgMjAwIE9LDQpEYXRlOiBNb24sIDE1IE1heSAyMDIzIDA1OjExOjE2IEdNVA0KQ29udGVudC1UeXBlOiBhcHBsaWNhdGlvbi9vY3RldC1zdHJlYW0NCkNvbnRlbnQtTGVuZ3RoOiAzNDANCkNvbm5lY3Rpb246IGNsb3NlDQpMYXN0LU1vZGlmaWVkOiBNb24sIDE1IE1heSAyMDIzIDA0OjI1OjE5IEdNVA0KRVRhZzogIjY0NjFiNDJmLTE1NCINClNlcnZlcjogUWlBblhpbiB3ZWIgc2VydmVyDQpBY2NlcHQtUmFuZ2VzOiBieXRlcw0KDQo=", "rsp_body": "QGVjaG8gb2ZmDQpyZW0g6I635Y+W5qGM6Z2i6Lev5b6EDQpzZXQgZGVzayA9IiINCkZvciAvRiAlJWkgaW4gKCdwb3dlcnNoZWxsIC1jb20gIltlbnZpcm9ubWVudF06OmdldGZvbGRlcnBhdGgoW2Vudmlyb25tZW50K3NwZWNpYWxmb2xkZXJdOjpkZXNrdG9wKSInKSBkbyAoc2V0IGRlc2s9JSVpKQ0KZWNobyAlZGVzayUNCmRlbCAiJWRlc2slXOaWsOWNj+WQjOWKnuWFrOS4k+eUqC5sbmsiDQpTZXQgU2hlbGw9V1NjcmlwdC5DcmVhdGVPYmplY3QoIldTY3JpcHQuU2hlbGwiKSANCldTY3JpcHQuU2xlZXAgMTAwMA0KU2hlbGwuU2VuZEtleXMgIntGNX0iDQppZTR1aW5pdC5leGUgLUNsZWFySWNvbkNhY2hlDQpkZWwgJTA=", "req_body": ""}, "proto": "http", "sip": "10.180.17.50", "dip": "10.111.48.17", "src_mac": "00:15:c7:f5:90:00", "dst_mac": "00:21:a0:0c:cc:40", "vlan_id": "", "sport": 62551, "dport": 80, "asset_group": "未分配资产组", "dip_group": "未分配资产组", "sip_group": "未分配资产组", "sip_addr": "局域网", "dip_addr": "局域网", "confidence": "高"}"#;
        let conf = WplField::try_parse("json").assert();
        let obj = ParserTUnit::new(JsonP::default(), conf.clone())
            .verify_parse_suc(&mut data)
            .assert();
        let tdc = DataRecord::from(obj);
        //assert_eq!(86, obj.len());
        println!("{}", tdc);
        assert!(tdc.field("rsp_status").is_some());
        assert!(tdc.field("dip_addr").is_some());
        assert!(tdc.field("_origin/vuln_name").is_some());
        assert!(tdc.field("_origin/detail_info").is_some());
        assert!(tdc.field("_origin/rsp_status").is_some());
        Ok(())
    }

    #[test]
    fn test_json_rename() -> AnyResult<()> {
        //let mut data = r#"{"_origin": {"rsp_status": 200, "sip": "10.180.17.50", "public_date": "2023-02-08 17:11:04", "vuln_name": "疑似访问/下载脚本文件", "detail_info": "发现疑似脚本文件下载行为，该类文件可直接在操作系统上执行，可能存在风险。", "solution": "确认该文件访问/下载行为是否为合规行为；如下载成功，检查所下载文件是否为恶意文件；如是，及时删除该文件并检查其是否被执行，杀死异常进程。", "uri": "/download/distribution/1_20230515/6461b42f12f17.bat", "xff": "", "vuln_harm": "该类文件可直接在操作系统上执行，若下载的是恶意脚本，可能存在较高风险。", "vuln_type": "文件下载", "vuln_desc": "发现疑似脚本文件下载行为，该类文件可直接在操作系统上执行，可能存在风险。", "write_date": 1684127059, "dport": 80, "code_language": "", "sport": 62551, "dip": "10.111.48.17", "site_app": ""}, "alarm_sample": 1, "alarm_sip": "10.180.17.50", "alarm_source": 1, "attack_chain": "0x02050000", "attack_org": "", "attack_sip": "10.111.48.17", "attack_type": "文件下载", "branch_id": "QtAVdJgqi", "file_name": "6461b42f12f17.bat", "first_access_time": "2023-05-15T13:04:19.000+0800", "hazard_level": 4, "hazard_rating": "中危", "host_state": "攻击成功", "ioc": "268572087-疑似访问/下载脚本文件", "is_delete": 0, "is_web_attack": "1", "is_white": 0, "nid": "", "repeat_count": 1, "rule_desc": "网页漏洞利用", "rule_key": "webids", "rule_state": "green", "sip_ioc_dip": "7873ecac0a91ce2b9f96b9f955e82065", "skyeye_id": "", "skyeye_index": "", "skyeye_type": "webids-webattack_dolog", "super_attack_chain": "0x02000000", "super_type": "攻击利用", "type": "文件下载", "type_chain": "16120000", "update_time": 1684127441, "vuln_type": "疑似访问/下载脚本文件", "white_id": null, "x_forwarded_for": "", "rule_labels": "{\"0x110A02\": {\"parent_name\": \"资产识别\", \"name\": \"UA指纹识别\", \"os\": \"Windows/XP\", \"parent_id\": \"0x110A00\", \"role\": \"C\", \"software\": \"IE/6.0\", \"type\": \"模块名称\", \"hw_type\": \"PC\"}}", "att_ck": "初始访问:TA0001|利用面向公众的应用程序:T1190", "serial_num": "QbJK/tb6A", "access_time": "2023-05-15T13:04:19.000+0800", "file_md5": "64f9bfa67c0b89fdb82b0673c1a96964", "host": "10.111.48.17", "rule_id": "0x100215b7", "rsp_status": "200", "skyeye_serial_num": "QbJK/tb6A", "host_md5": "6470cf2c183004f8ead3e150277b4d01", "alarm_id": "20230515_d3b0cca9ea8e9cca348f08ee036103dc", "payload": {"req_header": "R0VUIC9kb3dubG9hZC9kaXN0cmlidXRpb24vMV8yMDIzMDUxNS82NDYxYjQyZjEyZjE3LmJhdCBIVFRQLzEuMQ0KQWNjZXB0OiAqLyoNClVzZXItQWdlbnQ6IE1vemlsbGEvNC4wIChjb21wYXRpYmxlOyBNU0lFIDYuMDsgV2luZG93cyBOVCA1LjE7IFNWMSkNCkhvc3Q6IDEwLjExMS40OC4xNw0KQ29ubmVjdGlvbjogQ2xvc2UNCkNhY2hlLUNvbnRyb2w6IG5vLWNhY2hlDQoNCg==", "rsp_header": "SFRUUC8xLjEgMjAwIE9LDQpEYXRlOiBNb24sIDE1IE1heSAyMDIzIDA1OjExOjE2IEdNVA0KQ29udGVudC1UeXBlOiBhcHBsaWNhdGlvbi9vY3RldC1zdHJlYW0NCkNvbnRlbnQtTGVuZ3RoOiAzNDANCkNvbm5lY3Rpb246IGNsb3NlDQpMYXN0LU1vZGlmaWVkOiBNb24sIDE1IE1heSAyMDIzIDA0OjI1OjE5IEdNVA0KRVRhZzogIjY0NjFiNDJmLTE1NCINClNlcnZlcjogUWlBblhpbiB3ZWIgc2VydmVyDQpBY2NlcHQtUmFuZ2VzOiBieXRlcw0KDQo=", "rsp_body": "QGVjaG8gb2ZmDQpyZW0g6I635Y+W5qGM6Z2i6Lev5b6EDQpzZXQgZGVzayA9IiINCkZvciAvRiAlJWkgaW4gKCdwb3dlcnNoZWxsIC1jb20gIltlbnZpcm9ubWVudF06OmdldGZvbGRlcnBhdGgoW2Vudmlyb25tZW50K3NwZWNpYWxmb2xkZXJdOjpkZXNrdG9wKSInKSBkbyAoc2V0IGRlc2s9JSVpKQ0KZWNobyAlZGVzayUNCmRlbCAiJWRlc2slXOaWsOWNj+WQjOWKnuWFrOS4k+eUqC5sbmsiDQpTZXQgU2hlbGw9V1NjcmlwdC5DcmVhdGVPYmplY3QoIldTY3JpcHQuU2hlbGwiKSANCldTY3JpcHQuU2xlZXAgMTAwMA0KU2hlbGwuU2VuZEtleXMgIntGNX0iDQppZTR1aW5pdC5leGUgLUNsZWFySWNvbkNhY2hlDQpkZWwgJTA=", "req_body": ""}, "proto": "http", "sip": "10.180.17.50", "dip": "10.111.48.17", "src_mac": "00:15:c7:f5:90:00", "dst_mac": "00:21:a0:0c:cc:40", "vlan_id": "", "sport": 62551, "dport": 80, "asset_group": "未分配资产组", "dip_group": "未分配资产组", "sip_group": "未分配资产组", "sip_addr": "局域网", "dip_addr": "局域网", "confidence": "高"}"#;
        let mut data =
            r#"{"_origin": {"rsp_status": 200, "sip": "10.180.17.50" } ,"rsp_status": "200" }"#;
        let conf = WplField::try_parse(
            "json( @rsp_status:x_status, @_origin/rsp_status:_origin/x_status) ",
        )
        .assert();
        //let conf = WPLField::parse("json( @_origin/rsp_status:_origin/x_status) ");
        let obj = ParserTUnit::new(JsonP::default(), conf.clone())
            .verify_parse_suc(&mut data)
            .assert();
        let tdc = DataRecord::from(obj);
        //assert_eq!(86, obj.len());
        println!("{}", tdc);
        assert!(tdc.field("x_status").is_some());
        assert!(tdc.field("_origin/x_status").is_some());
        Ok(())
    }

    #[test]
    fn test_json_tianyan() -> AnyResult<()> {
        let conf = wpl_rule
            .parse("rule test {(json(_@_origin*,_@payload/packet_data))}")
            .assert();
        let f_conf = conf.statement.first_field().no_less("first field")?;
        let mut data = LONG_DATA;
        let fpu = FieldEvalUnit::from_auto(f_conf.clone());
        let ups_sep = WplSep::default();
        let mut out = Vec::new();
        fpu.parse(0, &ups_sep, &mut data, None, &mut out).assert();

        let mut no_ignore = 0;
        for i in &out {
            println!("{:?}", i);
        }

        for i in &out {
            println!("{:?}", i);
            if i.get_name().starts_with("_origin") || i.get_name().eq("payload/packet_data") {
                assert_eq!(i.get_meta(), &DataType::Ignore);
                continue;
            }

            no_ignore += 1;
            assert_ne!(i.get_meta(), &DataType::Ignore);
        }
        assert_eq!(no_ignore, 57);
        Ok(())
    }

    #[test]
    fn test_json_tianyan2() -> AnyResult<()> {
        let conf = wpl_rule.parse("rule test {(json(_@*))}").assert();
        let f_conf = conf.statement.first_field().no_less("first field")?;
        let mut data = LONG_DATA;
        let fpu = FieldEvalUnit::from_auto(f_conf.clone());
        let ups_sep = WplSep::default();
        let mut out = Vec::new();
        fpu.parse(0, &ups_sep, &mut data, None, &mut out).assert();

        let mut no_ignore = 0;
        for i in out {
            if *i.get_meta() != DataType::Ignore {
                no_ignore += 1;
            }
        }
        assert!(no_ignore < 1);
        Ok(())
    }

    #[test]
    fn test_json_tianyan3() -> AnyResult<()> {
        let conf = wpl_rule
            .parse(
                "rule test {(json(_@*,ip@alarm_sip , json@_origin, _@_origin/*,ip@_origin/sip)) }",
            )
            .assert();
        let f_conf = conf.statement.first_field().no_less("first field")?;
        let mut data = LONG_DATA;
        let fpu = FieldEvalUnit::from_auto(f_conf.clone());
        let mut out = Vec::new();
        let ups_sep = WplSep::default();
        fpu.parse(0, &ups_sep, &mut data, None, &mut out).assert();
        let mut no_ignore = 0;
        println!("TDC:{}", DataRecord::from(out.clone()));
        for i in out {
            if i.get_name().eq("alarm_sip") {
                assert_eq!(
                    KV_FMT.format_field(&i.into()),
                    "alarm_sip: 10.111.6.136".to_string()
                );
                no_ignore += 1;
                continue;
            }

            if i.get_name().eq("_origin/sip") {
                assert_eq!(
                    KV_FMT.format_field(&i.into()),
                    "_origin/sip: 10.111.134.201".to_string()
                );
                no_ignore += 1;
                continue;
            }
            assert_eq!(i.get_meta(), &DataType::Ignore);
        }
        assert!(no_ignore < 3);
        Ok(())
    }

    #[test]
    fn test_json_jt1() -> AnyResult<()> {
        let conf = wpl_rule
            .parse(r#"rule test {(json(chars@action/text)\\\0)}"#)
            .assert();
        let f_conf = conf.statement.first_field().no_less("first field")?;
        let mut data = JSON_DATA1;
        let fpu = FieldEvalUnit::from_auto(f_conf.clone());
        let mut out = Vec::new();
        let ups_sep = WplSep::default();
        fpu.parse(0, &ups_sep, &mut data, None, &mut out).assert();
        for i in out {
            if i.get_name().eq("action/text") {
                assert_eq!(
                    KV_FMT.format_field(&i.into()),
                    r#"action/text: "父进程 /bin/bash（pid：105123）创建进程 /usr/bin/curl（pid：105129）启动参数：-fsL http://localhost:8080/api/health/ 。来源：进程创建监控""#.to_string());
                continue;
            }

            assert_eq!(i.get_meta(), &DataType::Ignore);
        }
        Ok(())
    }

    #[test]
    fn test_json_jt2() -> AnyResult<()> {
        let conf = wpl_rule.parse(r#"rule test {(json\0)}"#).assert();
        let f_conf = conf.statement.first_field().no_less("first field")?;
        let mut data = JSON_DATA2;
        let fpu = FieldEvalUnit::for_test(JsonP::default(), f_conf.clone());
        let mut out = Vec::new();
        let ups_sep = WplSep::default();
        fpu.parse(0, &ups_sep, &mut data, None, &mut out).assert();
        for i in out {
            if i.get_name().eq("action/text") {
                assert_eq!(
                    KV_FMT.format_field(&i.into()),
                    r#"action/text: "父进程 /bin/bash（pid：105123）创建进程 /usr/bin/curl（pid：105129）启动参数：-fsL http://localhost:8080/api/health/ 。来源：进程创建监控""#
                );
                continue;
            }
        }
        Ok(())
    }
    #[test]
    fn test_json_3() -> AnyResult<()> {
        let rule = r#"rule test { (json(digit@value:cpu),json)\, }"#;
        let data =
            r#"{"name": "空闲CPU百分比", "value": 96}, {"name": "空闲内存(kB)", "value": 10243}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        let (tdc, _) = pipe.proc(0, data, 0)?;
        if let Some(i) = tdc.field("cpu") {
            let expected = DataField::from_digit("cpu", 96);
            assert_eq!(i.as_field(), &expected);
        }
        Ok(())
    }
    //{"value": 96.8}, abc

    #[test]
    fn test_json_4() -> AnyResult<()> {
        let rule = r#"rule test { (json(symbol(CPU)@name,digit@value:cpu),json)\, }"#;
        let data = r#"{"name": "CPU", "value": 96}, {"name": "空闲内存(kB)", "value": 10243}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        let (tdc, _) = pipe.proc(0, data, 0)?;
        if let Some(i) = tdc.field("cpu") {
            let expected = DataField::from_digit("cpu", 96);
            assert_eq!(i.as_field(), &expected);
        }
        Ok(())
    }
    #[test]
    fn test_json_5() -> AnyResult<()> {
        let rule = r#"rule test { (json(symbol(中国)@name,digit@value:cpu),json)\, }"#;
        let data = r#"{"name": "中国", "value": 96}, {"name": "空闲内存(kB)", "value": 10243}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        let (tdc, _) = pipe.proc(0, data, 0)?;
        if let Some(i) = tdc.field("cpu") {
            let expected = DataField::from_digit("cpu", 96);
            assert_eq!(i.as_field(), &expected);
        }
        Ok(())
    }

    #[test]
    fn test_json_6() -> AnyResult<()> {
        let rule = r#"rule test { (json)\, }"#;
        let data = r#"{"name": "中国", "value": 96, "key" : ["a","b","c"] }"#;
        let pipe = WplEvaluator::from_code(rule)?;
        let (tdc, _) = pipe.proc(0, data, 0)?;
        if let Some(i) = tdc.field("key[0]") {
            println!("{}", i);
            //assert_eq!(*i, TDOEnum::from_digit("cpu", 96));
        } else {
            panic!("json parse error");
        }
        Ok(())
    }

    #[test]
    fn test_json_long_keys_no_panic() -> AnyResult<()> {
        // 构造超长 key，验证路径拼接不因固定容量溢出而 panic
        let long_key_a = "a".repeat(300);
        let long_key_b = "b".repeat(300);
        let data = format!("{{\"{}\": {{ \"{}\": 123 }} }}", long_key_a, long_key_b);

        let conf = WplField::try_parse("json").assert();
        let _obj = ParserTUnit::new(JsonP::default(), conf.clone())
            .verify_parse_suc(&mut data.as_str())
            .assert();
        Ok(())
    }

    #[test]
    fn test_json_long_parent_with_array_no_panic() -> AnyResult<()> {
        // 父级为超长 key，子级为数组，验证数组元素名构造不 panic
        let long_key = "parent_".to_string() + &"x".repeat(260);
        // 使用较小数组规模，保证测试快速
        let data = format!(
            "{{\"{}\": {{ \"arr\": [1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16] }} }}",
            long_key
        );

        let conf = WplField::try_parse("json").assert();
        let _obj = ParserTUnit::new(JsonP::default(), conf.clone())
            .verify_parse_suc(&mut data.as_str())
            .assert();
        Ok(())
    }

    #[test]
    fn test_json_7() -> AnyResult<()> {
        let rule = r#"rule test { (json(time_timestamp@access_time)) }"#;
        let data = r#"{ "access_time": 1652174567000 }"#;
        let pipe = WplEvaluator::from_code(rule)?;
        let (tdc, _) = pipe.proc(0, data, 0)?;
        println!("{}", tdc);
        if let Some(i) = tdc.field("access_time") {
            assert_eq!(RAW_FMT.format_field(i), "2022-05-10 09:22:47".to_string());
        } else {
            panic!("json parse error");
        }
        Ok(())
    }

    #[test]
    fn test_json_logs_unescape_rule() -> AnyResult<()> {
        let rule = r#"rule nginx { (json( chars@logs | json_unescape() )) }"#;
        let data = r#"{"age": 10, "logs": "[10]:\"sys\""}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        let (tdc, _) = pipe.proc(0, data, 0)?;
        if let Some(field) = tdc.field("logs") {
            let expected = DataField::from_chars("logs".to_string(), "[10]:\"sys\"".to_string());
            assert_eq!(field.as_field(), &expected);
        } else {
            panic!("logs field missing");
        }
        Ok(())
    }

    #[test]
    fn test_json_pipe_auto_last_behavior() -> AnyResult<()> {
        // 未显式 take 时，last() 行为仍应作用于末尾字段
        let rule = r#"rule nginx { (json(chars@a, chars@b) | json_unescape()) }"#;
        let data = r#"{"a":"noop","b":"line1\nline2"}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        let (tdc, _) = pipe.proc(0, data, 0)?;
        assert_eq!(
            tdc.field("b").map(|s| s.as_field()),
            Some(&DataField::from_chars(
                "b".to_string(),
                "line1\nline2".to_string()
            ))
        );

        // take + auto selector 组合：第一次 take(name) 之后调用 f_chars_has 仍能针对 name
        let rule = r#"rule nginx { (json(chars@name, chars@code) | take(name) | chars_has( -99) | take(code) | chars_has( aaa)) }"#;
        let data = r#"{"name": -99, "code": "aaa"}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        assert!(pipe.proc(0, data, 0).is_ok());

        let rule = r#"rule nginx { (json(chars@code) | take(code) | chars_has(aaa)) }"#;
        let pipe = WplEvaluator::from_code(rule)?;
        assert!(pipe.proc(0, data, 0).is_ok());
        Ok(())
    }

    #[test]
    fn test_json_big_integer_downgrade_to_string() -> AnyResult<()> {
        // 大于 i64::MAX 的无符号整数应降级为字符串，避免静默丢失
        let conf = WplField::try_parse("json").assert();
        let data = format!("{{\"big\": {}}}", (i64::MAX as u128 + 1));
        let mut s = data.as_str();
        let obj = ParserTUnit::new(JsonP::default(), conf)
            .verify_parse_suc(&mut s)
            .assert();
        let tdc = DataRecord::from(obj);
        let expect = DataField::from_chars("big".to_string(), (i64::MAX as u128 + 1).to_string());
        assert_eq!(tdc.field("big").map(|s| s.as_field()), Some(&expect));
        Ok(())
    }

    #[test]
    fn test_json_i64_and_float_preserve() -> AnyResult<()> {
        let conf = WplField::try_parse("json").assert();
        let mut data = r#"{"i": -42, "f": 3.1415}"#;
        let obj = ParserTUnit::new(JsonP::default(), conf)
            .verify_parse_suc(&mut data)
            .assert();
        let tdc = DataRecord::from(obj);
        let expected_i = DataField::from_digit("i", -42);
        assert_eq!(tdc.field("i").map(|s| s.as_field()), Some(&expected_i));
        if let Some(storage) = tdc.field("f") {
            let _field = storage.as_field();
            // 不强约束小数舍入,仅校验存在
            // presence assertion only
        } else {
            panic!("float field missing");
        }
        Ok(())
    }

    #[test]
    fn test_json_str_mode_decoded_pipe() -> AnyResult<()> {
        let mut data = r#"{"path":"c:\\users\\fc\\file","txt":"line1\nline2"}"#;
        let conf = wpl_rule
            .parse(
                "rule test {(json(chars@path,chars@txt) | take(path) | json_unescape() | take(txt) | json_unescape())}"
            )
            .assert();
        let f_conf = conf.statement.first_field().no_less("first field")?;
        let fpu = FieldEvalUnit::from_auto(f_conf.clone());
        let ups_sep = WplSep::default();
        let mut out = Vec::new();
        fpu.parse(0, &ups_sep, &mut data, None, &mut out).assert();
        let dr = DataRecord::from(out);
        // 路径中的反斜杠数量符合预期；txt 中包含实际换行
        let expected_path = DataField::from_chars("path", "c:\\users\\fc\\file");
        assert_eq!(
            dr.field("path").map(|s| s.as_field()),
            Some(&expected_path)
        );
        if let Some(v) = dr.field("txt") {
            if let wp_model_core::model::Value::Chars(s) = v.as_field().get_value() {
                assert!(s.contains('\n'));
            } else {
                panic!("txt not chars")
            }
        }
        Ok(())
    }
    #[test]
    fn test_json_8() -> AnyResult<()> {
        let rule = r#"rule test { (json) }"#;
        let data = r#"{ "age": 18}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        let (tdc, _) = pipe.proc(0, data, 0)?;
        if let Some(i) = tdc.field("age") {
            assert_eq!(RAW_FMT.format_field(i), "18".to_string());
        } else {
            panic!("json parse error");
        }
        Ok(())
    }
    #[test]
    fn test_json_8_1() -> AnyResult<()> {
        let rule = r#"rule test { (json | f_has( age ) ) }"#;
        let data = r#"{ "age": 18}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        let (tdc, _) = pipe.proc(0, data, 0)?;
        if let Some(i) = tdc.field("age") {
            assert_eq!(RAW_FMT.format_field(i), "18".to_string());
        } else {
            panic!("json parse error");
        }

        let rule = r#"rule test { (json | f_has( age1 ) ) }"#;
        let data = r#"{ "age": 18}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        assert!(pipe.proc(0, data, 0).is_err());
        Ok(())
    }
    #[test]
    fn test_json_8_2_0() -> AnyResult<()> {
        let rule = r#"rule test { (json | f_digit_has( age,18 ) ) }"#;
        let data = r#"{  "name": "china","age": 18}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        let (tdc, _) = pipe.proc(0, data, 0)?;
        if let Some(i) = tdc.field("age") {
            assert_eq!(RAW_FMT.format_field(i), "18".to_string());
        } else {
            panic!("json parse error");
        }

        let rule = r#"rule test { (json | f_digit_has( age,19) ) }"#;
        let data = r#"{ "name": "china", "age": 18}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        assert!(pipe.proc(0, data, 0).is_err());
        Ok(())
    }
    #[test]
    fn test_json_8_2_1() -> AnyResult<()> {
        let rule = r#"rule test { (json | f_digit_in( age, [18,19] ) ) }"#;
        let data = r#"{  "name": "china","age": 18}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        let (tdc, _) = pipe.proc(0, data, 0)?;
        if let Some(i) = tdc.field("age") {
            assert_eq!(RAW_FMT.format_field(i), "18".to_string());
        } else {
            panic!("json parse error");
        }

        let rule = r#"rule test { (json | f_digit_in( age, [18,19] ) ) }"#;
        let data = r#"{ "name": "china", "age": 17}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        assert!(pipe.proc(0, data, 0).is_err());
        Ok(())
    }
    #[test]
    fn test_json_8_3() -> AnyResult<()> {
        let rule = r#"rule test { (json | f_chars_has( name,china ) ) }"#;
        let data = r#"{ "name": "china"}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        let (tdc, _) = pipe.proc(0, data, 0)?;
        if let Some(i) = tdc.field("name") {
            assert_eq!(RAW_FMT.format_field(i), "china".to_string());
        } else {
            panic!("json parse error");
        }

        let rule = r#"rule test { (json | f_chars_has( name,chinx) ) }"#;
        let data = r#"{ "name": "china"}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        assert!(pipe.proc(0, data, 0).is_err());

        let rule = r#"rule test { (json(chars@name) | f_chars_has(name, -99) | f_chars_has(code, aaa) ) }"#;
        let data = r#"{ "name": -99, "code": "aaa"}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        assert!(pipe.proc(0, data, 0).is_ok());
        Ok(())
    }
    #[test]
    fn test_json_8_3_1() -> AnyResult<()> {
        let rule = r#"rule test { (json | f_chars_in( name, [china,japan]) ) }"#;
        let data = r#"{ "name": "china"}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        assert!(pipe.proc(0, data, 0).is_ok());
        Ok(())
    }
    #[test]
    fn test_json_8_4() -> AnyResult<()> {
        let rule = r#"rule test { (json | f_chars_not_has(name, chinx) ) }"#;
        let data = r#"{ "name": "china"}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        assert!(pipe.proc(0, data, 0).is_ok());

        let rule = r#"rule test { (json(chars@name, chars@code) | f_chars_not_has(name, 1) | f_chars_has(code, aaa) ) }"#;
        let data = r#"{ "name": -99, "code": "aaa"}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        assert!(pipe.proc(0, data, 0).is_ok());
        Ok(())
    }
    #[test]
    fn test_json_8_5() -> AnyResult<()> {
        let rule = r#"rule test { (json(ip@addr) | f_ip_in(addr, [1.1.1.1,2.2.2.2]) ) }"#;
        let data = r#"{ "addr": "1.1.1.1"}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        assert!(pipe.proc(0, data, 0).is_ok());
        Ok(())
    }
    #[test]
    fn test_json_9() -> AnyResult<()> {
        let rule = r#"rule test { (json(time_timestamp@found_time:occur_time,@virus_name:alert_name,@virus_type:origin_alert_cat_name,@risk_level:severity,@iplist:terminal_ip,@host_name:terminal_name,@virus_name:malware_name,@file_md5,chars@file_path,@file_size:file_bytes,@state:protect_action,@agent_id,_@*)) }"#;
        let data = r#"{"_id":"6C941E33DDA773F19AEF2F21203863542E053D94","file_md5":"7e5432f32a3b6f25666e0cc9acff00bf","virus_name":"Suspicious.Win32.Save.a","risk_level":0,"create_time":1671693072,"state":"已处理","time":1671695066,"found_time":1671695066,"agent_id":"3358992609","file_path":"c:\\users\\fc\\desktop\\tr-shopbot\\7e5432f32a3b6f25666e0cc9acff00bf","virus_type":"其他病毒","threat_file":"Suspicious.Win32.Save.a","host_name":"DESKTOP-ARRA948","iplist":"10.122.163.99"}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        let (tdc, _) = pipe.proc(0, data, 0)?;
        if let Some(i) = tdc.field("file_path") {
            let expected = DataField::from_chars(
                "file_path",
                r#"c:\\users\\fc\\desktop\\tr-shopbot\\7e5432f32a3b6f25666e0cc9acff00bf"#
            );
            assert_eq!(i.as_field(), &expected);
        } else {
            panic!("json parse error");
        }
        Ok(())
    }

    #[test]
    fn test_json_11() -> AnyResult<()> {
        let rule = r#"rule test { (json(array/json@details:event_detail)) }"#;
        //let rule = r#"rule test { (json(@details:event_detail)) }"#;
        let data = r#"{"details":[{"relation":1,"alert_id":"94882787-9505-49d4-9024-20DC93AF579B","action_time":1676304603062,"rule_name":"访问 lemonduck 挖矿的通信域名","rule_desc":"进程 powershell.exe 访问 lemonduck 挖矿的通信域名","attck_id":"TA0011.T1071.004","process_mame":"powershell.exe","process_path":"C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe","command":"C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.EXE -ep bypass -eSQuAGIAZQA="}]}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        let (tdc, _) = pipe.proc(0, data, 0)?;

        println!("{}", tdc);

        let mut expected = ObjectValue::default();
        expected.insert(
            SmolStr::from("relation"),
            DataField::from_digit("relation".to_string(), 1),
        );
        expected.insert(
            SmolStr::from("alert_id"),
            DataField::from_chars(
                "alert_id".to_string(),
                r#"94882787-9505-49d4-9024-20DC93AF579B"#.to_string(),
            ),
        );
        expected.insert(
            SmolStr::from("action_time"),
            DataField::from_digit("action_time".to_string(), 1676304603062),
        );
        expected.insert(
            SmolStr::from("rule_name"),
            DataField::from_chars(
                "rule_name".to_string(),
                r#"访问 lemonduck 挖矿的通信域名"#.to_string(),
            ),
        );
        expected.insert(
            SmolStr::from("rule_desc"),
            DataField::from_chars(
                "rule_desc".to_string(),
                r#"进程 powershell.exe 访问 lemonduck 挖矿的通信域名"#.to_string(),
            ),
        );
        expected.insert(
            SmolStr::from("attck_id"),
            DataField::from_chars("attck_id".to_string(), r#"TA0011.T1071.004"#.to_string()),
        );
        expected.insert(
            SmolStr::from("process_mame"),
            DataField::from_chars("process_mame".to_string(), r#"powershell.exe"#.to_string()),
        );
        expected.insert(
            SmolStr::from("process_path"),
            DataField::from_chars(
                "process_path".to_string(),
                r#"C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe"#.to_string(),
            ),
        );
        expected.insert(SmolStr::from("command"),
                        DataField::from_chars( "command".to_string(),
                                             r#"C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.EXE -ep bypass -eSQuAGIAZQA="#.to_string()));
        if let Some(i) = tdc.field("event_detail") {
            let expected_field = DataField::from_arr(
                "event_detail".to_string(),
                vec![DataField::new_opt(DataType::Obj, None, expected.into())]
            );
            assert_eq!(i.as_field(), &expected_field);
        } else {
            panic!("json parse error");
        }

        let rule = r#"rule test { (json(@details:event_detail)) }"#;
        let data = r#"{"details":[{"relation":1,"alert_id":"94882787-9505-49d4-9024-20DC93AF579B","action_time":1676304603062,"rule_name":"访问 lemonduck 挖矿的通信域名","rule_desc":"进程 powershell.exe 访问 lemonduck 挖矿的通信域名","attck_id":"TA0011.T1071.004","process_mame":"powershell.exe","process_path":"C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe","command":"C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.EXE -ep bypass -eSQuAGIAZQA="}]}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        let (tdc, _) = pipe.proc(0, data, 0)?;

        let _expected = vec![
            DataField::from_digit("event_detail/relation".to_string(), 1),
            DataField::from_chars(
                "event_detail/alert_id".to_string(),
                r#"94882787-9505-49d4-9024-20DC93AF579B"#.to_string(),
            ),
            DataField::from_digit(
                "event_detail/action_time".to_string(),
                1676304603062,
            ),
            DataField::from_chars(
                "event_detail/rule_name".to_string(),
                r#"访问 lemonduck 挖矿的通信域名"#.to_string(),
            ),
            DataField::from_chars(
                "event_detail/rule_desc".to_string(),
                r#"进程 powershell.exe 访问 lemonduck 挖矿的通信域名"#.to_string(),
            ),
            DataField::from_chars(
                "event_detail/attck_id".to_string(),
                r#"TA0011.T1071.004"#.to_string(),
            ),
            DataField::from_chars(
                "event_detail/process_mame".to_string(),
                r#"powershell.exe"#.to_string(),
            ),
            DataField::from_chars(
                "event_detail/process_path".to_string(),
                r#"C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe"#.to_string(),
            ),
            DataField::from_chars(
                "event_detail/command".to_string(),
                r#"C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.EXE -ep bypass -eSQuAGIAZQA="#.to_string(),
            ),
        ];

        if let Some(i) = tdc.field("event_detail[0]/alert_id") {
            let expected = DataField::from_chars(
                "event_detail[0]/alert_id".to_string(),
                "94882787-9505-49d4-9024-20DC93AF579B".to_string()
            );
            assert_eq!(i.as_field(), &expected);
        } else {
            panic!("json parse error");
        }
        Ok(())
    }
    #[test]
    fn test_json_bug1() -> AnyResult<()> {
        let data = r#"{"http_req_header":"GET /?n=%0A&cmd=ipconfig+/all&search=%25xxx%25url%25:%password%}{.exec|{.?cmd.}|timeout=15|out=abc.}{.?n.}{.?n.}RESULT:{.?n.}{.^abc.}===={.?n.} HTTP/1.1\r\nAccept-Encoding: identity\r\nHost: 221.182.184.6:8081\r\nUser-Agent: Mozilla/5.0 (Windows NT 6.3; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/47.0.2526.106 Safari/537.36\r\nConnection: close\r\n\r\n"}"#;
        let rule = r#"rule test { (json(chars@http_req_header)) }"#;
        let pipe = WplEvaluator::from_code(rule)?;
        let (tdc, _) = pipe.proc(0, data, 0)?;
        println!("{}", tdc);
        Ok(())
    }

    #[test]
    fn test_json_take_chars() -> AnyResult<()> {
        let rule = r#"rule test { (json(chars@key)) }"#;
        let data = r#"{"key":  "hello boy"}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        let (tdc, _) = pipe.proc(0, data, 0)?;
        if let Some(i) = tdc.field("key") {
            let expected = DataField::from_chars("key", "hello boy");
            assert_eq!(i.as_field(), &expected);
        } else {
            panic!("json parse error");
        }
        Ok(())
    }

    #[test]
    fn test_json_take_chars2() -> AnyResult<()> {
        let rule = r#"rule test { (json(@action,_@*))}"#;
        let data = r#"{"action": "{\"text\": \"10.91.7.38(局域网) 访问 已知Webshell 10.48.116.32:8080/newShell/hello.jsp（物理路径：/usr/local/apache-tomcat-8.0.23/webapps/newShell/hello.jsp）。来源：网页浏览实时防护\", \"html\": \"<span class='ip'>10.91.7.38</span><span class='ipAddr'>(局域网)</span> 访问 <span class='type'>已知Webshell</span> <span class='url'>10.48.116.32:8080/newShell/hello.jsp</span><span class='webPagePhysicalPath'>（物理路径：/usr/local/apache-tomcat-8.0.23/webapps/newShell/hello.jsp）</span>。来源：<span class='source'> 网页浏览实时防护</span>\"}"}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        let (tdc, _) = pipe.proc(0, data, 0)?;
        if let Some(i) = tdc.field("action") {
            let expected = DataField::from_chars(
                "action",
                r#"{\"text\": \"10.91.7.38(局域网) 访问 已知Webshell 10.48.116.32:8080/newShell/hello.jsp（物理路径：/usr/local/apache-tomcat-8.0.23/webapps/newShell/hello.jsp）。来源：网页浏览实时防护\", \"html\": \"<span class='ip'>10.91.7.38</span><span class='ipAddr'>(局域网)</span> 访问 <span class='type'>已知Webshell</span> <span class='url'>10.48.116.32:8080/newShell/hello.jsp</span><span class='webPagePhysicalPath'>（物理路径：/usr/local/apache-tomcat-8.0.23/webapps/newShell/hello.jsp）</span>。来源：<span class='source'> 网页浏览实时防护</span>\"}"#,
            );
            assert_eq!(i.as_field(), &expected);
        } else {
            panic!("json parse error");
        }
        Ok(())
    }
    #[test]
    fn test_json_symbol() -> AnyResult<()> {
        let rule = r#"rule test { (json(symbol(boy)@key)) }"#;
        let data = r#"{"key":  "boy"}"#;
        let pipe = WplEvaluator::from_code(rule)?;
        let (tdc, _) = pipe.proc(0, data, 0)?;
        if let Some(i) = tdc.field("key") {
            let expected = DataField::from_symbol("key", "boy");
            assert_eq!(i.as_field(), &expected);
        } else {
            panic!("json parse error");
        }
        Ok(())
    }

    const LONG_DATA: &str = r#"{
	"_origin": {
		"ids_rule_version": "1.0",
		"sip": "10.111.134.201",
		"description": "1",
		"vuln_type": "暴力猜解",
		"attack_method": "远程",
		"cnnvd_id": "",
		"uri": "",
		"detail_info": "SSH 是 Secure Shell 的缩写，由 IETF 的网络小组（Network Working Group）所制定；SSH  为建立在应用层基础上的安全协议。  SSH暴力破解是指攻击者通过密码字典或随机组合密码的方式尝试登陆服务器（针对的是全网机器），这种攻击行为一般不会有明确攻击目标，多数是通过扫描软件直接扫描整个广播域或网段。",
		"protocol_id": 6,
		"rule_name": "疑似SSH账号暴力猜解",
		"sport": 20983,
		"write_date": 1684218967,
		"appid": 146,
		"dport": 2222,
		"xff": "",
		"dip": "10.111.6.136",
		"bulletin": "为了您的帐户安全，请尽量设置复杂密码，不要有规律。您容易记忆的密码，同时也很可能被轻易猜出来。请参考以下建议： 1、密码长度为6到16个字符； 2、密码安全性级别说明： a.当您仅使用英文字母、数字、特殊字符中的其中一种来设置密码时，如sqpofeHWESIS、54894565、%$#!%@等，系统会提示您密码的安全性级别为“不安全”； b.当您使用英文字母、数字、特殊字符的任意两种组合时，如uTEh47dy61、dg%ah$aj、25$2*04!63等，系统会提示您密码的安全性级别为“普通”； c.当您使用英文字母+数字+特殊字符的组合时，如sd8bjh*dh、sge352%ds等，系统会提示您密码的安全性级别为“安全”。",
		"affected_system": ""
	},
	"alarm_sample": 1,
	"alarm_sip": "10.111.6.136",
	"alarm_source": 1,
	"attack_chain": "0x02040000",
	"attack_org": "",
	"attack_sip": "10.111.134.201",
	"attack_type": "暴力猜解",
	"branch_id": "QtAVdJgqi",
	"file_name": null,
	"first_access_time": "2023-05-16T14:36:07.000+0800",
	"hazard_level": 4,
	"hazard_rating": "中危",
	"host_state": "企图",
	"ioc": "22951-疑似SSH账号暴力猜解",
	"is_delete": 0,
	"is_web_attack": "0",
	"is_white": 0,
	"nid": "",
	"repeat_count": 1,
	"rule_desc": "网络攻击",
	"rule_key": "webids",
	"rule_state": "green",
	"sip_ioc_dip": "0e096fe1f2fb6e51dbc7fdb5d1e4f06e",
	"skyeye_id": "",
	"skyeye_index": "",
	"skyeye_type": "webids-ids_dolog",
	"super_attack_chain": "0x02000000",
	"super_type": "攻击利用",
	"type": "暴力猜解",
	"type_chain": "16180000",
	"update_time": 1684219356,
	"vuln_type": "疑似SSH账号暴力猜解",
	"white_id": null,
	"x_forwarded_for": "",
	"rule_labels": "{}",
	"att_ck": "凭据访问:TA0006|暴力猜解:T1110",
	"serial_num": "QbJK/tb6A",
	"access_time": "2023-05-16T14:36:07.000+0800",
	"file_md5": "",
	"host": "",
	"rule_id": "0x59a7",
	"rsp_status": "",
	"skyeye_serial_num": "QbJK/tb6A",
	"host_md5": "",
	"alarm_id": "20230516_54bf47cc1f6591e18b5ab0785f910296",
	"payload": {
		"packet_data": "0ND9K06AEPMRBOFAiEcACiE9RQAChPvnQAA9Bp1dCm8GiApvhskIrlH3HwxviTRePZ+AGABy8p8AAAEBCAr6wpSWow8dyQAAAkwOFLfXvERV1A+DRd3WJ6Tgvy4AAACQY3VydmUyNTUxOS1zaGEyNTYsY3VydmUyNTUxOS1zaGEyNTZAbGlic3NoLm9yZyxlY2RoLXNoYTItbmlzdHAyNTYsZWNkaC1zaGEyLW5pc3RwMzg0LGVjZGgtc2hhMi1uaXN0cDUyMSxkaWZmaWUtaGVsbG1hbi1ncm91cDE0LXNoYTI1NixleHQtaW5mby1zAAAAOXJzYS1zaGEyLTI1Nixyc2Etc2hhMi01MTIsZWNkc2Etc2hhMi1uaXN0cDI1Nixzc2gtZWQyNTUxOQAAAGxhZXMxMjgtZ2NtQG9wZW5zc2guY29tLGFlczI1Ni1nY21Ab3BlbnNzaC5jb20sY2hhY2hhMjAtcG9seTEzMDVAb3BlbnNzaC5jb20sYWVzMTI4LWN0cixhZXMxOTItY3RyLGFlczI1Ni1jdHIAAABsYWVzMTI4LWdjbUBvcGVuc3NoLmNvbSxhZXMyNTYtZ2NtQG9wZW5zc2guY29tLGNoYWNoYTIwLXBvbHkxMzA1QG9wZW5zc2guY29tLGFlczEyOC1jdHIsYWVzMTkyLWN0cixhZXMyNTYtY3RyAAAAK2htYWMtc2hhMi0yNTYtZXRtQG9wZW5zc2guY29tLGhtYWMtc2hhMi0yNTYAAAAraG1hYy1zaGEyLTI1Ni1ldG1Ab3BlbnNzaC5jb20saG1hYy1zaGEyLTI1NgAAAARub25lAAAABG5vbmUAAAAAAAAAAAAAAAAAjXI7M1jzP7C5pmMtMhA="
	},
	"proto": "ssh",
	"sip": "10.111.134.201",
	"dip": "10.111.6.136",
	"src_mac": "00:0c:31:0b:30:80",
	"dst_mac": "d0:d0:fd:2b:4e:80",
	"vlan_id": "",
	"sport": 20983,
	"dport": 2222,
	"asset_group": "未分配资产组",
	"dip_group": "未分配资产组",
	"sip_group": "未分配资产组",
	"sip_addr": "局域网",
	"dip_addr": "局域网",
	"confidence": "中"
}
"#;

    const JSON_DATA1: &str = r#"
{"action":{"text":"父进程 /bin/bash（pid：105123）创建进程 /usr/bin/curl（pid：105129）启动参数：-fsL http://localhost:8080/api/health/ 。来源：进程创建监控"} }
"#;
    const JSON_DATA2: &str = r#"
{"action":{"text":"父进程 /bin/bash（pid：105123）创建进程 /usr/bin/curl（pid：105129）启动参数：-fsL http://localhost:8080/api/health/ 。来源：进程创建监控"},"alarmMsg":"","bannedStatus":0,"categoryName":"进程操作日志","categoryUuid":2,"day":"2024-06-17","dealStatus":0,"dealTime":0,"eventId":0,"eventUuid":"00000000","fullTree":"[{\"cmd\":\"\",\"fname\":\"\",\"name\":\"\",\"perm\":\"\",\"pid\":-72515583,\"ppid\":32672,\"svcname\":\"\",\"svctype\":\"\",\"user\":\"\"},{\"cmd\":\"--root /var/run/docker/runtime-runc/moby --log /var/run/docker/containerd/daemon/io.containerd.runtime.v2.task/moby/f585d4e717b36d73e0b342acb2705e042c9186acf4eaf739f8e0c399c3dcead3/log.json --log-format json exec --process /tmp/runc-process1432386409 --detach --pid-file /var/run/docker/containerd/daemon/io.containerd.runtime.v2.task/moby/f585d4e717b36d73e0b342acb2705e042c9186acf4eaf739f8e0c399c3dcead3/d48d6cb28d10e44c301df02aee8bc45568b6bdc1d6f9f6ec1e631255bb230d7d.pid f585d4e717b36d73e0b342acb2705e042c9186acf4eaf739f8e0c399c3dcead3 \",\"fname\":\"/usr/local/bin/runc\",\"name\":\"runc\",\"perm\":\"0755\",\"pid\":105105,\"ppid\":13033,\"svcname\":\"\",\"svctype\":\"\",\"user\":\"\"},{\"cmd\":\"-c curl -fsL http://localhost:8080/api/health/ > /dev/null \",\"fname\":\"/bin/bash\",\"name\":\"bash\",\"perm\":\"0755\",\"pid\":105123,\"ppid\":105105,\"svcname\":\"\",\"svctype\":\"\",\"user\":\"\"},{\"cmd\":\"-fsL http://localhost:8080/api/health/ \",\"fname\":\"/usr/bin/curl\",\"name\":\"curl\",\"perm\":\"0755\",\"pid\":105129,\"ppid\":105123,\"svcname\":\"\",\"svctype\":\"\",\"user\":\"\"}]\n","groupName":"创建进程","hour":19,"innerIp":"192.168.40.100","levelDesc":"低危","localTimestamp":1718623813000,"logType":0,"logo":"yunsuo","machine":{"assetsLevel":0,"currentPage":1,"extranetIp":"10.95.209.107","ifDelete":0,"installTime":1709880605000,"intranetIp":"192.168.40.100","ipv4":"192.168.250.1,192.168.40.100","ipv6":"fe80::42:51ff:fe36:8573,fe80::8268:5155:b100:c7c5,fe80::6045:86ff:fec3:1557,fe80::b853:9fff:fe36:6286,fe80::4c7c:8ff:fe82:be48,fe80::e8f7:c1ff:feb9:b9ea,fe80::fc0c:bbff:fee3:6def,fe80::24fe:25ff:fe18:7bfc,fe80::1818:85ff:fe6e:c51e,fe80::b0cf:6dff:fe39:ce7a","machineName":"jumpserver","maxResults":10,"nickname":"","onlineStatus":1,"operatingSystem":"CentOS Linux 7 (Core)","osType":1,"softwareVersion":"linux_8.0.6.2044","uuid":"2c5c146dfbbfc21c06c3cb0d54c98d82"},"machineUuid":"2c5c146dfbbfc21c06c3cb0d54c98d82","minute":1172,"object":{"cmdline":"-fsL http://localhost:8080/api/health/ ","pid":"105129","proc":"/usr/bin/curl"},"operation":"create_proc","operationDesc":"创建进程","outerIp":"10.95.209.107","phase":0,"phaseDesc":"其他","result":1,"risk":0,"score":0,"searchStatus":0,"sn":"CA720142ADB3F17827163179903CB1E1","source":1,"sourceDesc":"内置规则","standardTimestamp":1718623975000,"subject":{"pid":"105123","procHash":"cfd65bed18a1fae631091c3a4c4dd533","process":"/bin/bash","type":"kernel","user":"root"},"treePath":"systemd(1)>containerd-shim-runc-v2(13033)>runc(105105)>bash(105123)","typeName":"创建进程","ucrc":3732834598,"userAndSettings":[{"machineTags":["场景19"],"status":0,"userUuid":"68375c3f443e4bca9b1d2a0c1b6c7c1a","username":"yanshaopeng"},{"machineTags":["场景19"],"status":0,"userUuid":"8fd56137ea6b4945a4ac3c3ed2944988","username":"pangshibo"},{"machineTags":["场景19"],"status":0,"userUuid":"e64df71c4b304a9aa7453cbd2b68fa96","username":"sxbchj"}],"victimIpFlag":1}
"#;
}
