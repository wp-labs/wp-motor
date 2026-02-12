extern crate wp_knowledge as wp_know;
use oml::core::DataTransformer;
use oml::parser::oml_parse_raw;
use oml::types::AnyResult;
use orion_error::TestAssert;
use std::net::{IpAddr, Ipv4Addr};
use wp_data_fmt::Json;
use wp_data_fmt::KeyValue;
use wp_data_fmt::ProtoTxt;
use wp_data_fmt::RecordFormatter;
use wp_data_model::cache::FieldQueryCache;
use wp_know::mem::memdb::MemDB;
use wp_log::conf::log_for_test;
use wp_model_core::model::DataField;
use wp_model_core::model::DataRecord;
use wp_model_core::model::Value;
use wp_model_core::model::types::value::ObjectValue;
use wp_parser::WResult as ModalResult;
#[test]
fn test_crate_get() {
    let cache = &mut FieldQueryCache::default();

    let data = vec![
        DataField::from_chars("A1", "hello1"),
        DataField::from_chars("B2", "hello2"),
        DataField::from_chars("C3", "hello3"),
    ];
    let src = DataRecord::from(data);

    let mut conf = r#"
        name : test
        ---
        A10  = take() { _ : chars(hello1) };
        "#;
    let model = oml_parse_raw(&mut conf).assert();

    let _expect = src.clone();
    let target = model.transform(src.clone(), cache);

    assert_eq!(
        target.get_field_owned("A10"),
        Some(DataField::from_chars("A10", "hello1"))
    );

    let mut conf = r#"
        name : test
        ---
        A1 : chars = take(B2);
        "#;
    let model = oml_parse_raw(&mut conf).assert();
    let target = model.transform(src.clone(), cache);
    let expect = DataField::from_chars("A1", "hello2");
    assert_eq!(target.get_field_owned("A1"), Some(expect));

    let mut conf = r#"
        name : test
        ---
        A3 : chars = take(option : [B3,C3]);
        "#;
    let model = oml_parse_raw(&mut conf).assert();
    let target = model.transform(src.clone(), cache);
    let expect = DataField::from_chars("A3", "hello3");
    assert_eq!(target.get_field_owned("A3"), Some(expect));
}

#[test]
fn test_take_fun() {
    let cache = &mut FieldQueryCache::default();

    let data = vec![
        DataField::from_chars("A1", "hello1"),
        DataField::from_chars("B2", "hello2"),
        DataField::from_chars("C3", "hello3"),
    ];
    let src = DataRecord::from(data);
    let mut conf = r#"
        name : test
        ---
        A10  = read() { _ : Now::date() };
        A20  = read() { _ : Now::date() };
        A30  = read() { _ : Now::hour() };
        A40  = read() { _ : Now::hour() };
        "#;
    let model = oml_parse_raw(&mut conf).assert();

    let target = model.transform(src.clone(), cache);

    assert_eq!(target.get_value("A10"), target.get_value("A20"));
    assert_eq!(target.get_value("A30"), target.get_value("A40"));
    println!("{:?}", target.get_value("A10"));
    println!("{:?}", target.get_value("A30"));
}

#[test]
fn test_take_conv() {
    let cache = &mut FieldQueryCache::default();

    let data = vec![
        DataField::from_chars("A1", "192.168.0.1"),
        DataField::from_chars("B2", "100"),
        DataField::from_chars("C3", "100.1"),
    ];
    let src = DataRecord::from(data);
    let mut conf = r#"
        name : test
        ---
        A1 : ip = read();
        B2 : digit = read();
        C3 : float = read();
        D4 : chars = ip(192.168.1.1);
        "#;
    let model = oml_parse_raw(&mut conf).assert();
    let target = model.transform(src.clone(), cache);

    println!("{}", target);
    assert_eq!(
        target.get_value("A1"),
        Some(&Value::IpAddr(IpAddr::V4(Ipv4Addr::new(192, 168, 0, 1))))
    );
    assert_eq!(target.get_value("B2"), Some(&Value::Digit(100)));
    assert_eq!(
        target.get_value("D4"),
        Some(&Value::Chars("192.168.1.1".into()))
    );
}
#[test]
fn test_wild_get() {
    let cache = &mut FieldQueryCache::default();

    let data = vec![
        DataField::from_chars("A1/path", "hello1"),
        DataField::from_chars("A2/name", "hello1"),
        DataField::from_chars("B2/path", "hello2"),
        DataField::from_chars("C3/name", "hello3"),
        DataField::from_chars("C4/name ", "hello3"),
    ];
    let src = DataRecord::from(data);

    let mut conf = r#"
        name : test
        ---
        * = take();
        "#;
    let model = oml_parse_raw(&mut conf).assert();

    let expect = src.clone();
    let target = model.transform(src.clone(), cache);

    assert_eq!(target.items.len(), 5);
    assert_eq!(
        target.get_field_owned("A1/path"),
        expect.get_field_owned("A1/path")
    );
    assert_eq!(
        target.get_field_owned("B2/path"),
        expect.get_field_owned("B2/path")
    );

    let mut conf = r#"
        name : test
        ---
        */path = take();
        "#;
    let model = oml_parse_raw(&mut conf).assert();

    let expect = src.clone();
    let target = model.transform(src.clone(), cache);

    assert_eq!(target.items.len(), 2);
    assert_eq!(
        target.get_field_owned("A1/path"),
        expect.get_field_owned("A1/path")
    );
    assert_eq!(
        target.get_field_owned("B2/path"),
        expect.get_field_owned("B2/path")
    );

    let mut conf = r#"
        name : test
        ---
        A*/path = take();
        "#;
    let model = oml_parse_raw(&mut conf).assert();

    let expect = src.clone();
    let target = model.transform(src.clone(), cache);

    assert_eq!(target.items.len(), 1);
    assert_eq!(
        target.get_field_owned("A1/path"),
        expect.get_field_owned("A1/path")
    );

    let mut conf = r#"
        name : test
        ---
        */name= take();
        "#;
    let model = oml_parse_raw(&mut conf).assert();

    let expect = src.clone();
    let target = model.transform(src.clone(), cache);

    assert_eq!(target.items.len(), 3);
    assert_eq!(
        target.get_field_owned("A2/name"),
        expect.get_field_owned("A2/name")
    );
}

#[test]
fn test_crate_move() {
    let cache = &mut FieldQueryCache::default();
    let data = vec![
        DataField::from_chars("A1", "hello1"),
        DataField::from_chars("B2", "hello2"),
        DataField::from_chars("C3", "hello3"),
    ];
    let src = DataRecord::from(data);

    let mut conf = r#"
        name : test
        ---
        A1 : chars = take(A1);
        A2 : chars = take(A1);
        "#;
    let model = oml_parse_raw(&mut conf).assert();

    let expect = src.clone();
    let target = model.transform(src, cache);

    assert_eq!(target.get_field_owned("A1"), expect.get_field_owned("A1"));
    assert!(target.get_field_owned("A2").is_none())
}

#[test]
fn test_value_get() {
    let cache = &mut FieldQueryCache::default();
    let data = vec![
        DataField::from_chars("A1", "hello1"),
        DataField::from_chars("B2", "hello2"),
        DataField::from_chars("C3", "hello3"),
    ];
    let src = DataRecord::from(data);

    let mut conf = r#"
        name : test
        ---
        A4 : chars = chars(hello4);
        "#;
    let model = oml_parse_raw(&mut conf).assert();

    let target = model.transform(src, cache);

    let expect = DataField::from_chars("A4", "hello4");
    assert_eq!(target.get_field_owned("A4"), Some(expect));
}
#[test]
fn test_map_get() {
    let cache = &mut FieldQueryCache::default();
    let data = vec![
        DataField::from_chars("A1", "hello1"),
        DataField::from_chars("B2", "hello2"),
        DataField::from_chars("C3", "hello3"),
    ];
    let expect = data.clone();
    let src = DataRecord::from(data);

    let mut conf = r#"
        name : test
        ---

        X : obj =  object {
            A1 : chars = take();
            B2 : chars = take();
            C3 : chars = chars(hello3);
        };
        "#;
    let model = oml_parse_raw(&mut conf).assert();

    let target = model.transform(src, cache);

    println!("{}", target);
    let mut expect_obj = ObjectValue::default();
    for i in expect {
        expect_obj.insert(i.get_name().to_string(), DataField::from(i));
    }
    assert_eq!(
        target.get_field_owned("X"),
        Some(DataField::from_obj("X", expect_obj))
    );
}

#[test]
fn test_match_get() {
    let cache = &mut FieldQueryCache::default();
    let mut conf = r#"
        name : test
        ---
        X : chars =  match take(ip) {
                in (ip(10.0.0.1), ip(10.0.0.10)) => take(city1) ;
                ip(10.0.10.1)  => take(city2) ;
                _  => chars(bj) ;
        };
        "#;
    let model = oml_parse_raw(&mut conf).assert();

    let data = vec![
        DataField::from_ip("ip", IpAddr::V4(Ipv4Addr::new(10, 0, 0, 3))),
        DataField::from_chars("city1", "cs"),
        DataField::from_chars("city2", "hk"),
    ];
    let src = DataRecord::from(data);

    let target = model.transform(src, cache);
    let one = target.get_field_owned("X");

    assert_eq!(one, Some(DataField::from_chars("X", "cs")));

    let data = vec![
        DataField::from_ip("ip", IpAddr::V4(Ipv4Addr::new(10, 0, 10, 1))),
        DataField::from_chars("city1", "cs"),
        DataField::from_chars("city2", "hk"),
    ];
    let src = DataRecord::from(data);

    let target = model.transform(src, cache);
    let one = target.get_field_owned("X");

    assert_eq!(one, Some(DataField::from_chars("X", "hk")));

    let data = vec![
        DataField::from_ip("ip", IpAddr::V4(Ipv4Addr::new(10, 0, 10, 2))),
        DataField::from_chars("city1", "cs"),
        DataField::from_chars("city2", "hk"),
    ];
    let src = DataRecord::from(data);

    let target = model.transform(src, cache);
    let one = target.get_field_owned("X");

    assert_eq!(one, Some(DataField::from_chars("X", "bj")));
}

#[test]
fn test_match2_get() -> ModalResult<()> {
    let cache = &mut FieldQueryCache::default();
    let mut conf = r#"
        name : test
        ---
        X : chars =  match (take(ip),read(key1) ) {
                (in (ip(10.0.0.1), ip(10.0.0.10)), chars(A) ) => take(city1) ;
                ( ip(10.0.10.1), chars(B) )  => take(city2) ;
                _  => chars(bj) ;
        };
        "#;
    let model = oml_parse_raw(&mut conf).assert();

    let data = vec![
        DataField::from_ip("ip", IpAddr::V4(Ipv4Addr::new(10, 0, 0, 3))),
        DataField::from_chars("key1", "A"),
        DataField::from_chars("city1", "cs"),
        DataField::from_chars("city2", "hk"),
    ];
    let src = DataRecord::from(data);

    let target = model.transform(src, cache);
    let one = target.get_field_owned("X");

    assert_eq!(one, Some(DataField::from_chars("X", "cs")));

    let data = vec![
        DataField::from_ip("ip", IpAddr::V4(Ipv4Addr::new(10, 0, 0, 3))),
        DataField::from_chars("key1", "B"),
        DataField::from_chars("city1", "cs"),
        DataField::from_chars("city2", "hk"),
    ];
    let src = DataRecord::from(data);

    let target = model.transform(src, cache);
    let one = target.get_field_owned("X");

    assert_eq!(one, Some(DataField::from_chars("X", "bj")));

    let data = vec![
        DataField::from_ip("ip", IpAddr::V4(Ipv4Addr::new(10, 0, 10, 1))),
        DataField::from_chars("key1", "B"),
        DataField::from_chars("city1", "cs"),
        DataField::from_chars("city2", "hk"),
    ];
    let src = DataRecord::from(data);

    let target = model.transform(src, cache);
    let one = target.get_field_owned("X");

    assert_eq!(one, Some(DataField::from_chars("X", "hk")));

    let data = vec![
        DataField::from_ip("ip", IpAddr::V4(Ipv4Addr::new(10, 0, 10, 2))),
        DataField::from_chars("city1", "cs"),
        DataField::from_chars("city2", "hk"),
    ];
    let src = DataRecord::from(data);

    let target = model.transform(src, cache);
    let one = target.get_field_owned("X");

    assert_eq!(one, Some(DataField::from_chars("X", "bj")));
    Ok(())
}

#[test]
fn test_match3_get() -> ModalResult<()> {
    let cache = &mut FieldQueryCache::default();
    let mut conf = r#"
        name : test
        ---
        X : digit =  match take(key1) {
                bool(true)  => digit(1) ;
                bool(false) => digit(2) ;
                _  => digit(3) ;
        };
        "#;
    let model = oml_parse_raw(&mut conf).assert();

    let data = vec![DataField::from_bool("key1", true)];
    let src = DataRecord::from(data);
    let target = model.transform(src, cache);
    let one = target.get_field_owned("X");
    assert_eq!(one, Some(DataField::from_digit("X", 1)));

    let data = vec![DataField::from_bool("key1", false)];
    let src = DataRecord::from(data);
    let target = model.transform(src, cache);
    let one = target.get_field_owned("X");
    assert_eq!(one, Some(DataField::from_digit("X", 2)));
    Ok(())
}

#[test]
fn test_match4_get() -> ModalResult<()> {
    let cache = &mut FieldQueryCache::default();

    let mut conf = r#"
name: csv_example
---
occur_time : time =  Now::time()  ;
occur_ss =  pipe read(occur_time)  | Time::to_ts_zone(0,ss);
occur_ms =  pipe read(occur_time)  | Time::to_ts_zone(0,ms);
occur_us =  pipe read(occur_time)  | Time::to_ts_zone(0,us);

occur_ss1  =  pipe read(occur_time)  | Time::to_ts_zone(8,s);
X: chars = match  read(month) {
    in ( digit(1) , digit(3) ) => chars(Q1);
    in ( digit(4) , digit(6) ) => chars(Q2);
    in ( digit(7) , digit(9) ) => chars(Q3);
    in ( digit(10) , digit(12) ) => chars(Q4);
    _ => chars(Q5);
};
        "#;
    let model = oml_parse_raw(&mut conf).assert();

    let data = vec![DataField::from_digit("month", 3)];
    let src = DataRecord::from(data);

    let target = model.transform(src, cache);
    let one = target.get_field_owned("X");
    assert_eq!(one, Some(DataField::from_chars("X", "Q1")));

    let data = vec![DataField::from_digit("month", 6)];
    let src = DataRecord::from(data);

    let target = model.transform(src, cache);
    let one = target.get_field_owned("X");
    assert_eq!(one, Some(DataField::from_chars("X", "Q2")));

    let data = vec![DataField::from_digit("month", 10)];
    let src = DataRecord::from(data);

    let target = model.transform(src, cache);
    let one = target.get_field_owned("X");
    assert_eq!(one, Some(DataField::from_chars("X", "Q4")));
    println!("{}", target);
    Ok(())
}

#[test]
fn test_match_triple_get() -> ModalResult<()> {
    let cache = &mut FieldQueryCache::default();
    let mut conf = r#"
        name : test
        ---
        X : chars = match (take(city), take(level), take(zone)) {
                (chars(bj), chars(high), chars(north)) => chars(result_A) ;
                (chars(sh), chars(low), chars(east)) => chars(result_B) ;
                _  => chars(default) ;
        };
        "#;
    let model = oml_parse_raw(&mut conf).assert();

    // Test case 1: first arm matches
    let data = vec![
        DataField::from_chars("city", "bj"),
        DataField::from_chars("level", "high"),
        DataField::from_chars("zone", "north"),
    ];
    let src = DataRecord::from(data);
    let target = model.transform(src, cache);
    let one = target.get_field_owned("X");
    assert_eq!(one, Some(DataField::from_chars("X", "result_A")));

    // Test case 2: second arm matches
    let data = vec![
        DataField::from_chars("city", "sh"),
        DataField::from_chars("level", "low"),
        DataField::from_chars("zone", "east"),
    ];
    let src = DataRecord::from(data);
    let target = model.transform(src, cache);
    let one = target.get_field_owned("X");
    assert_eq!(one, Some(DataField::from_chars("X", "result_B")));

    // Test case 3: partial mismatch falls to default
    let data = vec![
        DataField::from_chars("city", "bj"),
        DataField::from_chars("level", "low"),
        DataField::from_chars("zone", "north"),
    ];
    let src = DataRecord::from(data);
    let target = model.transform(src, cache);
    let one = target.get_field_owned("X");
    assert_eq!(one, Some(DataField::from_chars("X", "default")));

    Ok(())
}

#[test]
fn test_match_triple_with_mixed_cond() -> ModalResult<()> {
    let cache = &mut FieldQueryCache::default();
    let mut conf = r#"
        name : test
        ---
        X : chars = match (take(ip), take(level), take(zone)) {
                (in (ip(10.0.0.1), ip(10.0.0.100)), chars(high), chars(north)) => chars(block) ;
                (ip(192.168.0.1), chars(low), chars(south)) => chars(allow) ;
                _  => chars(unknown) ;
        };
        "#;
    let model = oml_parse_raw(&mut conf).assert();

    // Test case 1: ip in range + exact match
    let data = vec![
        DataField::from_ip("ip", IpAddr::V4(Ipv4Addr::new(10, 0, 0, 50))),
        DataField::from_chars("level", "high"),
        DataField::from_chars("zone", "north"),
    ];
    let src = DataRecord::from(data);
    let target = model.transform(src, cache);
    let one = target.get_field_owned("X");
    assert_eq!(one, Some(DataField::from_chars("X", "block")));

    // Test case 2: exact ip match
    let data = vec![
        DataField::from_ip("ip", IpAddr::V4(Ipv4Addr::new(192, 168, 0, 1))),
        DataField::from_chars("level", "low"),
        DataField::from_chars("zone", "south"),
    ];
    let src = DataRecord::from(data);
    let target = model.transform(src, cache);
    let one = target.get_field_owned("X");
    assert_eq!(one, Some(DataField::from_chars("X", "allow")));

    // Test case 3: no match
    let data = vec![
        DataField::from_ip("ip", IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1))),
        DataField::from_chars("level", "mid"),
        DataField::from_chars("zone", "west"),
    ];
    let src = DataRecord::from(data);
    let target = model.transform(src, cache);
    let one = target.get_field_owned("X");
    assert_eq!(one, Some(DataField::from_chars("X", "unknown")));

    Ok(())
}

#[test]
fn test_match_quadruple_get() -> ModalResult<()> {
    let cache = &mut FieldQueryCache::default();
    let mut conf = r#"
        name : test
        ---
        X : chars = match (take(src_ip), take(dst_ip), take(proto), take(action)) {
                (ip(10.0.0.1), ip(192.168.1.1), chars(tcp), chars(allow)) => chars(rule_A) ;
                (ip(10.0.0.2), ip(192.168.1.2), chars(udp), chars(deny)) => chars(rule_B) ;
                _  => chars(default_rule) ;
        };
        "#;
    let model = oml_parse_raw(&mut conf).assert();

    // Test case 1: first arm matches
    let data = vec![
        DataField::from_ip("src_ip", IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))),
        DataField::from_ip("dst_ip", IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))),
        DataField::from_chars("proto", "tcp"),
        DataField::from_chars("action", "allow"),
    ];
    let src = DataRecord::from(data);
    let target = model.transform(src, cache);
    let one = target.get_field_owned("X");
    assert_eq!(one, Some(DataField::from_chars("X", "rule_A")));

    // Test case 2: second arm matches
    let data = vec![
        DataField::from_ip("src_ip", IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2))),
        DataField::from_ip("dst_ip", IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2))),
        DataField::from_chars("proto", "udp"),
        DataField::from_chars("action", "deny"),
    ];
    let src = DataRecord::from(data);
    let target = model.transform(src, cache);
    let one = target.get_field_owned("X");
    assert_eq!(one, Some(DataField::from_chars("X", "rule_B")));

    // Test case 3: no match falls to default
    let data = vec![
        DataField::from_ip("src_ip", IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1))),
        DataField::from_ip("dst_ip", IpAddr::V4(Ipv4Addr::new(172, 16, 0, 2))),
        DataField::from_chars("proto", "icmp"),
        DataField::from_chars("action", "log"),
    ];
    let src = DataRecord::from(data);
    let target = model.transform(src, cache);
    let one = target.get_field_owned("X");
    assert_eq!(one, Some(DataField::from_chars("X", "default_rule")));

    Ok(())
}

#[test]
fn test_match_quadruple_with_range() -> ModalResult<()> {
    let cache = &mut FieldQueryCache::default();
    let mut conf = r#"
        name : test
        ---
        X : chars = match (take(src_ip), take(dst_ip), take(level), take(zone)) {
                (in (ip(10.0.0.1), ip(10.0.0.100)), in (ip(192.168.0.1), ip(192.168.0.100)), chars(high), chars(east)) => chars(critical) ;
                (ip(172.16.0.1), ip(172.16.0.2), chars(low), chars(west)) => chars(minor) ;
                _  => chars(normal) ;
        };
        "#;
    let model = oml_parse_raw(&mut conf).assert();

    // Test case 1: both ip ranges match + exact fields
    let data = vec![
        DataField::from_ip("src_ip", IpAddr::V4(Ipv4Addr::new(10, 0, 0, 50))),
        DataField::from_ip("dst_ip", IpAddr::V4(Ipv4Addr::new(192, 168, 0, 50))),
        DataField::from_chars("level", "high"),
        DataField::from_chars("zone", "east"),
    ];
    let src = DataRecord::from(data);
    let target = model.transform(src, cache);
    let one = target.get_field_owned("X");
    assert_eq!(one, Some(DataField::from_chars("X", "critical")));

    // Test case 2: default
    let data = vec![
        DataField::from_ip("src_ip", IpAddr::V4(Ipv4Addr::new(10, 0, 0, 50))),
        DataField::from_ip("dst_ip", IpAddr::V4(Ipv4Addr::new(192, 168, 0, 50))),
        DataField::from_chars("level", "low"),
        DataField::from_chars("zone", "east"),
    ];
    let src = DataRecord::from(data);
    let target = model.transform(src, cache);
    let one = target.get_field_owned("X");
    assert_eq!(one, Some(DataField::from_chars("X", "normal")));

    Ok(())
}

#[test]
fn test_value_arr() {
    let cache = &mut FieldQueryCache::default();
    let data = vec![
        DataField::from_chars("A1", "hello1"),
        DataField::from_chars("B2", "hello2"),
        DataField::from_chars("C3", "hello3"),
        DataField::from_chars("C4", "hello4"),
    ];
    let src = DataRecord::from(data.clone());

    let mut conf = r#"
        name : test
        ---
        X1 : array = collect take(keys : [A1, B2,C*]);
        X2  =  pipe read(X1) | to_json ;
        "#;
    let model = oml_parse_raw(&mut conf).assert();

    let target = model.transform(src, cache);

    let expect = DataField::from_arr("X1".to_string(), data);
    assert_eq!(target.get_field_owned("X1"), Some(expect));
    let json_out = Json.fmt_record(&target).to_string();
    println!("{}", json_out);
    println!("{}", ProtoTxt.fmt_record(&target));
    println!("{}", KeyValue::default().fmt_record(&target));
    assert_eq!(
        json_out,
        r#"{"X1":["hello1","hello2","hello3","hello4"],"X2":"[\"hello1\",\"hello2\",\"hello3\",\"hello4\"]"}"#
    );
    //println!("{}", target.get("X2"));
}

#[test]
fn test_sql_1() -> AnyResult<()> {
    let cache = &mut FieldQueryCache::default();
    // 绑定门面到全局内存库并装载 example 表
    let _ = wp_knowledge::facade::init_mem_provider(MemDB::global());
    MemDB::load_test()?;
    let data = vec![DataField::from_chars("py", "xiaolongnu")];
    let src = DataRecord::from(data.clone());

    let mut conf = r#"
        name : test
        ---
        A2,B2  = select name,pinying from example where pinying = read(py) ;
        _,_  = select name,pinying from example where pinying = "xiaolongnu" ;
        "#;
    let model = oml_parse_raw(&mut conf).assert();
    let target = model.transform(src, cache);
    let result = Json.fmt_record(&target).to_string();
    let expect = r#"{"A2":"小龙女","B2":"xiaolongnu","name":"小龙女","pinying":"xiaolongnu"}"#;
    assert_eq!(result, expect);
    Ok(())
}

#[test]
fn test_sql_debug() -> AnyResult<()> {
    log_for_test()?;
    let cache = &mut FieldQueryCache::default();
    let _ = wp_knowledge::facade::init_mem_provider(MemDB::global());
    MemDB::load_test()?;
    let data = vec![DataField::from_chars("X", "xiaolongnu")];
    let src = DataRecord::from(data.clone());

    let mut conf = r#"
        name : test
        ---
        _,_  = select name,pinying from example where pinying = 'xiaolongnu' ;
        "#;
    let model = oml_parse_raw(&mut conf).assert();
    let target = model.transform(src, cache);
    let result = Json.fmt_record(&target).to_string();
    let expect = r#"{"name":"小龙女","pinying":"xiaolongnu"}"#;
    assert_eq!(result, expect);
    Ok(())
}

#[test]
fn test_value_arr1() {
    let cache = &mut FieldQueryCache::default();

    let data = vec![
        DataField::from_chars("details[0]/process_name", "hello1"),
        DataField::from_chars("details[1]/process_name", "hello2"),
        DataField::from_chars("details[2]/process_name", "hello3"),
        DataField::from_chars("details[3]/process_name", "hello4"),
    ];
    let src = DataRecord::from(data.clone());

    let mut conf = r#"
        name : test
        ---
        X1 : array = collect take(keys :[details[*]/process_name]);
        X2  = pipe read(X1) | nth(0) ;
        X3  = pipe read(X1) | nth(2) ;
        "#;
    let model = oml_parse_raw(&mut conf).assert();

    let target = model.transform(src, cache);

    println!("{}", Json.fmt_record(&target));
    let expect = DataField::from_arr("X1".to_string(), data);
    assert_eq!(target.get_field_owned("X1"), Some(expect));
    assert_eq!(
        target.get_field_owned("X2"),
        Some(DataField::from_chars("X2", "hello1"))
    );
    assert_eq!(
        target.get_field_owned("X3"),
        Some(DataField::from_chars("X3", "hello3"))
    );
}
//}

// ==================== Enable Configuration Tests ====================

#[test]
fn test_enable_default_true() {
    // Test that enable defaults to true when not specified
    let mut conf = r#"
        name : test
        ---
        A1 = chars(hello);
        "#;
    let model = oml_parse_raw(&mut conf).assert();
    assert_eq!(*model.enable(), true, "Default enable should be true");
}

#[test]
fn test_enable_explicit_true() {
    // Test explicit enable: true
    let mut conf = r#"
        name : test
        enable : true
        ---
        A1 = chars(hello);
        "#;
    let model = oml_parse_raw(&mut conf).assert();
    assert_eq!(*model.enable(), true, "Explicit enable true");
}

#[test]
fn test_enable_explicit_false() {
    // Test explicit enable: false
    let mut conf = r#"
        name : test
        enable : false
        ---
        A1 = chars(hello);
        "#;
    let model = oml_parse_raw(&mut conf).assert();
    assert_eq!(*model.enable(), false, "Explicit enable false");
}

#[test]
fn test_enable_with_rule() {
    // Test enable with rule configuration
    let mut conf = r#"
        name : test
        rule : /nginx/*
        enable : false
        ---
        A1 = chars(hello);
        "#;
    let model = oml_parse_raw(&mut conf).assert();
    assert_eq!(*model.enable(), false, "Enable should be false");
    assert!(!model.rules().is_empty(), "Rules should be set");
}

#[test]
fn test_enable_before_rule() {
    // Test enable before rule (order independence)
    let mut conf = r#"
        name : test
        enable : false
        rule : /path/*
        ---
        A1 = chars(hello);
        "#;
    let model = oml_parse_raw(&mut conf).assert();
    assert_eq!(*model.enable(), false, "Enable should be false");
    assert!(!model.rules().is_empty(), "Rules should be set");
}

#[test]
fn test_enabled_model_transforms_data() {
    // Test that enabled model transforms data correctly
    let cache = &mut FieldQueryCache::default();
    let mut conf = r#"
        name : test
        enable : true
        ---
        result = chars(transformed);
        "#;
    let model = oml_parse_raw(&mut conf).assert();
    assert_eq!(*model.enable(), true);

    let src = DataRecord::default();
    let target = model.transform(src, cache);
    assert_eq!(
        target.get_field_owned("result"),
        Some(DataField::from_chars("result", "transformed"))
    );
}

#[test]
fn test_disabled_model_still_parses() {
    // Test that disabled model can still be parsed and used if needed
    // (the filtering happens at load time, not at parse time)
    let cache = &mut FieldQueryCache::default();
    let mut conf = r#"
        name : disabled_model
        enable : false
        ---
        result = chars(should_not_run);
        "#;
    let model = oml_parse_raw(&mut conf).assert();
    assert_eq!(*model.enable(), false);
    assert_eq!(model.name(), "disabled_model");

    // Model can still transform if called directly (filtering is at load time)
    let src = DataRecord::default();
    let target = model.transform(src, cache);
    assert_eq!(
        target.get_field_owned("result"),
        Some(DataField::from_chars("result", "should_not_run"))
    );
}

#[test]
fn test_enable_with_complex_config() {
    // Test enable with complex configuration including static blocks
    let cache = &mut FieldQueryCache::default();
    let mut conf = r#"
        name : complex_model
        rule : /api/* /web/*
        enable : true
        ---
        static {
            default_val = chars(default);
        }
        result = default_val;
        field1, field2 = take();
        "#;
    let model = oml_parse_raw(&mut conf).assert();
    assert_eq!(*model.enable(), true);
    assert_eq!(model.rules().as_ref().len(), 2);

    let data = vec![
        DataField::from_chars("field1", "v1"),
        DataField::from_chars("field2", "v2"),
    ];
    let src = DataRecord::from(data);
    let target = model.transform(src, cache);

    assert_eq!(
        target.get_field_owned("result"),
        Some(DataField::from_chars("result", "default"))
    );
    assert_eq!(
        target.get_field_owned("field1"),
        Some(DataField::from_chars("field1", "v1"))
    );
}

#[test]
fn test_enable_preserves_model_name() {
    // Ensure enable config doesn't affect model name parsing
    let mut conf = r#"
        name : my_special_model
        enable : false
        ---
        x = chars(y);
        "#;
    let model = oml_parse_raw(&mut conf).assert();
    assert_eq!(model.name(), "my_special_model");
    assert_eq!(*model.enable(), false);
}

#[test]
fn test_multiple_rules_with_enable() {
    // Test multiple rules with enable configuration
    let mut conf = r#"
        name : multi_rule_model
        rule : /path/a/* /path/b/* /path/c/*
        enable : true
        ---
        * = take();
        "#;
    let model = oml_parse_raw(&mut conf).assert();
    assert_eq!(*model.enable(), true);
    assert_eq!(model.rules().as_ref().len(), 3);
}

// ==================== Static Symbol Match Tests ====================

#[test]
fn test_static_symbol_eq_match() {
    // Test static symbols in equality match conditions
    let cache = &mut FieldQueryCache::default();
    let mut conf = r#"
        name : test_static_eq
        ---
        static {
            ip_local = ip(127.0.0.1);
            status_ok = chars(success);
        }

        result = match read(src_ip) {
            ip_local => chars(localhost);
            _ => chars(remote);
        };
        "#;
    let model = oml_parse_raw(&mut conf).assert();

    // Test matching case
    let data = vec![DataField::from_ip(
        "src_ip",
        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
    )];
    let src = DataRecord::from(data);
    let target = model.transform(src, cache);
    assert_eq!(
        target.get_field_owned("result"),
        Some(DataField::from_chars("result", "localhost"))
    );

    // Test non-matching case (default)
    let data = vec![DataField::from_ip(
        "src_ip",
        IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),
    )];
    let src = DataRecord::from(data);
    let target = model.transform(src, cache);
    assert_eq!(
        target.get_field_owned("result"),
        Some(DataField::from_chars("result", "remote"))
    );
}

#[test]
fn test_static_symbol_neq_match() {
    // Test static symbols in negation match conditions
    let cache = &mut FieldQueryCache::default();
    let mut conf = r#"
        name : test_static_neq
        ---
        static {
            ip_127 = ip(127.0.0.1);
        }

        result = match read(src_ip) {
            !ip_127 => chars(external);
            _ => chars(internal);
        };
        "#;
    let model = oml_parse_raw(&mut conf).assert();

    // Test negation match (not 127.0.0.1)
    let data = vec![DataField::from_ip(
        "src_ip",
        IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
    )];
    let src = DataRecord::from(data);
    let target = model.transform(src, cache);
    assert_eq!(
        target.get_field_owned("result"),
        Some(DataField::from_chars("result", "external"))
    );

    // Test negation non-match (is 127.0.0.1)
    let data = vec![DataField::from_ip(
        "src_ip",
        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
    )];
    let src = DataRecord::from(data);
    let target = model.transform(src, cache);
    assert_eq!(
        target.get_field_owned("result"),
        Some(DataField::from_chars("result", "internal"))
    );
}

#[test]
fn test_static_symbol_in_range_match() {
    // Test static symbols in range match conditions
    let cache = &mut FieldQueryCache::default();
    let mut conf = r#"
        name : test_static_in_range
        ---
        static {
            status_200 = digit(200);
            status_299 = digit(299);
            status_400 = digit(400);
            status_499 = digit(499);
        }

        level = match read(http_status) {
            in (status_200, status_299) => chars(success);
            in (status_400, status_499) => chars(client_error);
            _ => chars(other);
        };
        "#;
    let model = oml_parse_raw(&mut conf).assert();

    // Test in first range
    let data = vec![DataField::from_digit("http_status", 200)];
    let src = DataRecord::from(data);
    let target = model.transform(src, cache);
    assert_eq!(
        target.get_field_owned("level"),
        Some(DataField::from_chars("level", "success"))
    );

    let data = vec![DataField::from_digit("http_status", 250)];
    let src = DataRecord::from(data);
    let target = model.transform(src, cache);
    assert_eq!(
        target.get_field_owned("level"),
        Some(DataField::from_chars("level", "success"))
    );

    // Test in second range
    let data = vec![DataField::from_digit("http_status", 404)];
    let src = DataRecord::from(data);
    let target = model.transform(src, cache);
    assert_eq!(
        target.get_field_owned("level"),
        Some(DataField::from_chars("level", "client_error"))
    );

    // Test outside ranges (default)
    let data = vec![DataField::from_digit("http_status", 500)];
    let src = DataRecord::from(data);
    let target = model.transform(src, cache);
    assert_eq!(
        target.get_field_owned("level"),
        Some(DataField::from_chars("level", "other"))
    );
}

#[test]
fn test_static_symbol_chars_match() {
    // Test static symbols with chars data type
    let cache = &mut FieldQueryCache::default();
    let mut conf = r#"
        name : test_static_chars
        ---
        static {
            env_prod = chars(production);
            env_dev = chars(development);
        }

        priority = match read(environment) {
            env_prod => digit(1);
            env_dev => digit(3);
            _ => digit(5);
        };
        "#;
    let model = oml_parse_raw(&mut conf).assert();

    // Test production environment
    let data = vec![DataField::from_chars("environment", "production")];
    let src = DataRecord::from(data);
    let target = model.transform(src, cache);
    assert_eq!(
        target.get_field_owned("priority"),
        Some(DataField::from_digit("priority", 1))
    );

    // Test development environment
    let data = vec![DataField::from_chars("environment", "development")];
    let src = DataRecord::from(data);
    let target = model.transform(src, cache);
    assert_eq!(
        target.get_field_owned("priority"),
        Some(DataField::from_digit("priority", 3))
    );

    // Test other environment
    let data = vec![DataField::from_chars("environment", "staging")];
    let src = DataRecord::from(data);
    let target = model.transform(src, cache);
    assert_eq!(
        target.get_field_owned("priority"),
        Some(DataField::from_digit("priority", 5))
    );
}

#[test]
fn test_static_symbol_multiple_match_cases() {
    // Test multiple match expressions using static symbols
    let cache = &mut FieldQueryCache::default();
    let mut conf = r#"
        name : test_multiple_static_match
        ---
        static {
            localhost = chars(localhost);
            attack_ip = chars(attack);
            ip_127 = ip(127.0.0.1);
            dgt_200 = digit(200);
            dgt_400 = digit(400);
        }

        ip_type = match read(src_ip) {
            ip_127 => localhost;
            !ip_127 => attack_ip;
        };

        status_level = match read(status_code) {
            in (dgt_200, dgt_400) => chars(normal);
            _ => chars(other);
        };
        "#;
    let model = oml_parse_raw(&mut conf).assert();

    // Test both matches
    let data = vec![
        DataField::from_ip("src_ip", IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))),
        DataField::from_digit("status_code", 300),
    ];
    let src = DataRecord::from(data);
    let target = model.transform(src, cache);

    assert_eq!(
        target.get_field_owned("ip_type"),
        Some(DataField::from_chars("ip_type", "localhost"))
    );
    assert_eq!(
        target.get_field_owned("status_level"),
        Some(DataField::from_chars("status_level", "normal"))
    );

    // Test with different values
    let data = vec![
        DataField::from_ip("src_ip", IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))),
        DataField::from_digit("status_code", 500),
    ];
    let src = DataRecord::from(data);
    let target = model.transform(src, cache);

    assert_eq!(
        target.get_field_owned("ip_type"),
        Some(DataField::from_chars("ip_type", "attack"))
    );
    assert_eq!(
        target.get_field_owned("status_level"),
        Some(DataField::from_chars("status_level", "other"))
    );
}

#[test]
fn test_static_symbol_with_result_reference() {
    // Test static symbols in both condition and result parts
    let cache = &mut FieldQueryCache::default();
    let mut conf = r#"
        name : test_static_cond_and_result
        ---
        static {
            min_threshold = digit(100);
            max_threshold = digit(200);
            high_label = chars(high);
            low_label = chars(low);
        }

        level = match read(value) {
            in (min_threshold, max_threshold) => high_label;
            _ => low_label;
        };
        "#;
    let model = oml_parse_raw(&mut conf).assert();

    // Test in range
    let data = vec![DataField::from_digit("value", 150)];
    let src = DataRecord::from(data);
    let target = model.transform(src, cache);
    assert_eq!(
        target.get_field_owned("level"),
        Some(DataField::from_chars("level", "high"))
    );

    // Test below range
    let data = vec![DataField::from_digit("value", 50)];
    let src = DataRecord::from(data);
    let target = model.transform(src, cache);
    assert_eq!(
        target.get_field_owned("level"),
        Some(DataField::from_chars("level", "low"))
    );
}

// ==================== Arc Performance Tests ====================

#[test]
fn test_arc_optimization_parsing_performance() {
    use std::time::Instant;

    // Test with static symbols (should use Arc for zero-copy)
    let mut conf_with_static = r#"
        name : test_with_static
        ---
        static {
            ip_127 = ip(127.0.0.1);
            localhost = chars(localhost_value);
            attack_ip = chars(attack_ip_value);
            status_200 = digit(200);
            status_400 = digit(400);
            ok_msg = chars(ok_message);
            err_msg = chars(error_message);
        }

        ip_type = match read(src_ip) {
            ip_127 => localhost;
            !ip_127 => attack_ip;
        };

        status_level = match read(status) {
            in (status_200, status_400) => ok_msg;
            _ => err_msg;
        };
    "#;

    let start = Instant::now();
    let model_with_static = oml_parse_raw(&mut conf_with_static).assert();
    let parse_time_static = start.elapsed();

    // Test without static symbols (inline values, multiple DataField clones)
    let mut conf_without_static = r#"
        name : test_without_static
        ---
        ip_type = match read(src_ip) {
            ip(127.0.0.1) => chars(localhost_value);
            !ip(127.0.0.1) => chars(attack_ip_value);
        };

        status_level = match read(status) {
            in (digit(200), digit(400)) => chars(ok_message);
            _ => chars(error_message);
        };
    "#;

    let start = Instant::now();
    let model_without_static = oml_parse_raw(&mut conf_without_static).assert();
    let parse_time_no_static = start.elapsed();

    println!("\n=== Parsing Performance (Arc Optimization) ===");
    println!("With static (Arc):    {:?}", parse_time_static);
    println!("Without static:       {:?}", parse_time_no_static);

    // Arc optimization should make parsing faster or equal
    // (Arc::clone is much cheaper than DataField clone)

    // Runtime performance test - should be identical
    let cache = &mut FieldQueryCache::default();
    let data = vec![
        DataField::from_ip("src_ip", IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))),
        DataField::from_digit("status", 300),
    ];
    let src = DataRecord::from(data);

    // Verify both produce correct results
    let result_static = model_with_static.transform(src.clone(), cache);
    let result_no_static = model_without_static.transform(src.clone(), cache);

    assert_eq!(
        result_static.get_field_owned("ip_type"),
        result_no_static.get_field_owned("ip_type")
    );
    assert_eq!(
        result_static.get_field_owned("status_level"),
        result_no_static.get_field_owned("status_level")
    );
}

#[test]
fn test_arc_optimization_with_many_references() {
    use std::time::Instant;

    // Test with many static symbol references
    // Each static value is used multiple times - Arc shines here
    let mut conf_with_static = r#"
        name : test_many_refs_static
        ---
        static {
            val_100 = digit(100);
            val_200 = digit(200);
            val_300 = digit(300);
            val_400 = digit(400);
            val_500 = digit(500);
            msg_low = chars(low_priority);
            msg_medium = chars(medium_priority);
            msg_high = chars(high_priority);
            msg_critical = chars(critical_priority);
        }

        level1 = match read(score1) {
            val_100 => msg_low;
            val_200 => msg_medium;
            val_300 => msg_high;
            val_400 => msg_critical;
            _ => msg_low;
        };

        level2 = match read(score2) {
            val_100 => msg_low;
            val_200 => msg_medium;
            val_300 => msg_high;
            val_400 => msg_critical;
            _ => msg_low;
        };

        level3 = match read(score3) {
            val_100 => msg_low;
            val_200 => msg_medium;
            val_300 => msg_high;
            val_400 => msg_critical;
            _ => msg_low;
        };

        level4 = match read(score4) {
            val_100 => msg_low;
            val_200 => msg_medium;
            val_300 => msg_high;
            val_400 => msg_critical;
            _ => msg_low;
        };

        level5 = match read(score5) {
            in (val_100, val_200) => msg_low;
            in (val_300, val_400) => msg_high;
            _ => msg_medium;
        };
    "#;

    let start = Instant::now();
    let _model_with_static = oml_parse_raw(&mut conf_with_static).assert();
    let parse_time_static = start.elapsed();

    // Same logic without static - each value is duplicated many times
    let mut conf_without_static = r#"
        name : test_many_refs_no_static
        ---
        level1 = match read(score1) {
            digit(100) => chars(low_priority);
            digit(200) => chars(medium_priority);
            digit(300) => chars(high_priority);
            digit(400) => chars(critical_priority);
            _ => chars(low_priority);
        };

        level2 = match read(score2) {
            digit(100) => chars(low_priority);
            digit(200) => chars(medium_priority);
            digit(300) => chars(high_priority);
            digit(400) => chars(critical_priority);
            _ => chars(low_priority);
        };

        level3 = match read(score3) {
            digit(100) => chars(low_priority);
            digit(200) => chars(medium_priority);
            digit(300) => chars(high_priority);
            digit(400) => chars(critical_priority);
            _ => chars(low_priority);
        };

        level4 = match read(score4) {
            digit(100) => chars(low_priority);
            digit(200) => chars(medium_priority);
            digit(300) => chars(high_priority);
            digit(400) => chars(critical_priority);
            _ => chars(low_priority);
        };

        level5 = match read(score5) {
            in (digit(100), digit(200)) => chars(low_priority);
            in (digit(300), digit(400)) => chars(high_priority);
            _ => chars(medium_priority);
        };
    "#;

    let start = Instant::now();
    let _model_without_static = oml_parse_raw(&mut conf_without_static).assert();
    let parse_time_no_static = start.elapsed();

    println!("\n=== Parsing Performance (Many References) ===");
    println!("With static (Arc):    {:?}", parse_time_static);
    println!("Without static:       {:?}", parse_time_no_static);
    println!(
        "Speedup:              {:.2}x",
        parse_time_no_static.as_nanos() as f64 / parse_time_static.as_nanos() as f64
    );

    // Arc optimization shows clear benefit when values are reused
    // Without Arc: each reference creates a new DataField (expensive)
    // With Arc: each reference just clones Arc pointer (cheap)
}
