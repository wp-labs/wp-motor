extern crate wp_knowledge as wp_know;
use oml::core::DataTransformer;
use oml::parser::oml_parse_raw;
use oml::types::AnyResult;
use orion_error::TestAssert;
use std::net::{IpAddr, Ipv4Addr};
use wp_data_fmt::DataFormat;
use wp_data_fmt::Json;
use wp_data_fmt::KeyValue;
use wp_data_fmt::ProtoTxt;
use wp_data_fmt::StaticDataFormatter;
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
    let src = DataRecord { items: data };

    let mut conf = r#"
        name : test
        ---
        A10  = take() { _ : chars(hello1) };
        "#;
    let model = oml_parse_raw(&mut conf).assert();

    let _expect = src.clone();
    let target = model.transform(src.clone(), cache);

    assert_eq!(
        target.field("A10"),
        Some(&DataField::from_chars("A10", "hello1"))
    );

    let mut conf = r#"
        name : test
        ---
        A1 : chars = take(B2);
        "#;
    let model = oml_parse_raw(&mut conf).assert();
    let target = model.transform(src.clone(), cache);
    let expect = DataField::from_chars("A1", "hello2");
    assert_eq!(target.field("A1"), Some(&expect));

    let mut conf = r#"
        name : test
        ---
        A3 : chars = take(option : [B3,C3]);
        "#;
    let model = oml_parse_raw(&mut conf).assert();
    let target = model.transform(src.clone(), cache);
    let expect = DataField::from_chars("A3", "hello3");
    assert_eq!(target.field("A3"), Some(&expect));
}

#[test]
fn test_take_fun() {
    let cache = &mut FieldQueryCache::default();

    let data = vec![
        DataField::from_chars("A1", "hello1"),
        DataField::from_chars("B2", "hello2"),
        DataField::from_chars("C3", "hello3"),
    ];
    let src = DataRecord { items: data };
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
    let src = DataRecord { items: data };
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
    let src = DataRecord { items: data };

    let mut conf = r#"
        name : test
        ---
        * = take();
        "#;
    let model = oml_parse_raw(&mut conf).assert();

    let expect = src.clone();
    let target = model.transform(src.clone(), cache);

    assert_eq!(target.items.len(), 5);
    assert_eq!(target.field("A1/path"), expect.field("A1/path"));
    assert_eq!(target.field("B2/path"), expect.field("B2/path"));

    let mut conf = r#"
        name : test
        ---
        */path = take();
        "#;
    let model = oml_parse_raw(&mut conf).assert();

    let expect = src.clone();
    let target = model.transform(src.clone(), cache);

    assert_eq!(target.items.len(), 2);
    assert_eq!(target.field("A1/path"), expect.field("A1/path"));
    assert_eq!(target.field("B2/path"), expect.field("B2/path"));

    let mut conf = r#"
        name : test
        ---
        A*/path = take();
        "#;
    let model = oml_parse_raw(&mut conf).assert();

    let expect = src.clone();
    let target = model.transform(src.clone(), cache);

    assert_eq!(target.items.len(), 1);
    assert_eq!(target.field("A1/path"), expect.field("A1/path"));

    let mut conf = r#"
        name : test
        ---
        */name= take();
        "#;
    let model = oml_parse_raw(&mut conf).assert();

    let expect = src.clone();
    let target = model.transform(src.clone(), cache);

    assert_eq!(target.items.len(), 3);
    assert_eq!(target.field("A2/name"), expect.field("A2/name"));
}

#[test]
fn test_crate_move() {
    let cache = &mut FieldQueryCache::default();
    let data = vec![
        DataField::from_chars("A1", "hello1"),
        DataField::from_chars("B2", "hello2"),
        DataField::from_chars("C3", "hello3"),
    ];
    let src = DataRecord { items: data };

    let mut conf = r#"
        name : test
        ---
        A1 : chars = take(A1);
        A2 : chars = take(A1);
        "#;
    let model = oml_parse_raw(&mut conf).assert();

    let expect = src.clone();
    let target = model.transform(src, cache);

    assert_eq!(target.field("A1"), expect.field("A1"));
    assert!(target.field("A2").is_none())
}

#[test]
fn test_value_get() {
    let cache = &mut FieldQueryCache::default();
    let data = vec![
        DataField::from_chars("A1", "hello1"),
        DataField::from_chars("B2", "hello2"),
        DataField::from_chars("C3", "hello3"),
    ];
    let src = DataRecord { items: data };

    let mut conf = r#"
        name : test
        ---
        A4 : chars = chars(hello4);
        "#;
    let model = oml_parse_raw(&mut conf).assert();

    let target = model.transform(src, cache);

    let expect = DataField::from_chars("A4", "hello4");
    assert_eq!(target.field("A4"), Some(&expect));
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
    let src = DataRecord { items: data };

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
        target.field("X"),
        Some(&DataField::from_obj("X", expect_obj))
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
    let src = DataRecord { items: data };

    let target = model.transform(src, cache);
    let one = target.field("X");

    assert_eq!(one, Some(&DataField::from_chars("X", "cs")));

    let data = vec![
        DataField::from_ip("ip", IpAddr::V4(Ipv4Addr::new(10, 0, 10, 1))),
        DataField::from_chars("city1", "cs"),
        DataField::from_chars("city2", "hk"),
    ];
    let src = DataRecord { items: data };

    let target = model.transform(src, cache);
    let one = target.field("X");

    assert_eq!(one, Some(&DataField::from_chars("X", "hk")));

    let data = vec![
        DataField::from_ip("ip", IpAddr::V4(Ipv4Addr::new(10, 0, 10, 2))),
        DataField::from_chars("city1", "cs"),
        DataField::from_chars("city2", "hk"),
    ];
    let src = DataRecord { items: data };

    let target = model.transform(src, cache);
    let one = target.field("X");

    assert_eq!(one, Some(&DataField::from_chars("X", "bj")));
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
    let src = DataRecord { items: data };

    let target = model.transform(src, cache);
    let one = target.field("X");

    assert_eq!(one, Some(&DataField::from_chars("X", "cs")));

    let data = vec![
        DataField::from_ip("ip", IpAddr::V4(Ipv4Addr::new(10, 0, 0, 3))),
        DataField::from_chars("key1", "B"),
        DataField::from_chars("city1", "cs"),
        DataField::from_chars("city2", "hk"),
    ];
    let src = DataRecord { items: data };

    let target = model.transform(src, cache);
    let one = target.field("X");

    assert_eq!(one, Some(&DataField::from_chars("X", "bj")));

    let data = vec![
        DataField::from_ip("ip", IpAddr::V4(Ipv4Addr::new(10, 0, 10, 1))),
        DataField::from_chars("key1", "B"),
        DataField::from_chars("city1", "cs"),
        DataField::from_chars("city2", "hk"),
    ];
    let src = DataRecord { items: data };

    let target = model.transform(src, cache);
    let one = target.field("X");

    assert_eq!(one, Some(&DataField::from_chars("X", "hk")));

    let data = vec![
        DataField::from_ip("ip", IpAddr::V4(Ipv4Addr::new(10, 0, 10, 2))),
        DataField::from_chars("city1", "cs"),
        DataField::from_chars("city2", "hk"),
    ];
    let src = DataRecord { items: data };

    let target = model.transform(src, cache);
    let one = target.field("X");

    assert_eq!(one, Some(&DataField::from_chars("X", "bj")));
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
    let src = DataRecord { items: data };
    let target = model.transform(src, cache);
    let one = target.field("X");
    assert_eq!(one, Some(&DataField::from_digit("X", 1)));

    let data = vec![DataField::from_bool("key1", false)];
    let src = DataRecord { items: data };
    let target = model.transform(src, cache);
    let one = target.field("X");
    assert_eq!(one, Some(&DataField::from_digit("X", 2)));
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
    let src = DataRecord { items: data };

    let target = model.transform(src, cache);
    let one = target.field("X");
    assert_eq!(one, Some(&DataField::from_chars("X", "Q1")));

    let data = vec![DataField::from_digit("month", 6)];
    let src = DataRecord { items: data };

    let target = model.transform(src, cache);
    let one = target.field("X");
    assert_eq!(one, Some(&DataField::from_chars("X", "Q2")));

    let data = vec![DataField::from_digit("month", 10)];
    let src = DataRecord { items: data };

    let target = model.transform(src, cache);
    let one = target.field("X");
    assert_eq!(one, Some(&DataField::from_chars("X", "Q4")));
    println!("{}", target);
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
    let src = DataRecord {
        items: data.clone(),
    };

    let mut conf = r#"
        name : test
        ---
        X1 : array = collect take(keys : [A1, B2,C*]);
        X2  =  pipe read(X1) | to_json ;
        "#;
    let model = oml_parse_raw(&mut conf).assert();

    let target = model.transform(src, cache);

    let expect = DataField::from_arr("X1".to_string(), data);
    assert_eq!(target.field("X1"), Some(&expect));
    let json_out = Json::stdfmt_record(&target).to_string();
    println!("{}", json_out);
    println!("{}", ProtoTxt.format_record(&target));
    println!("{}", KeyValue::default().format_record(&target));
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
    let src = DataRecord {
        items: data.clone(),
    };

    let mut conf = r#"
        name : test
        ---
        A2,B2  = select name,pinying from example where pinying = read(py) ;
        _,_  = select name,pinying from example where pinying = "xiaolongnu" ;
        "#;
    let model = oml_parse_raw(&mut conf).assert();
    let target = model.transform(src, cache);
    let result = Json::stdfmt_record(&target).to_string();
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
    let src = DataRecord {
        items: data.clone(),
    };

    let mut conf = r#"
        name : test
        ---
        _,_  = select name,pinying from example where pinying = 'xiaolongnu' ;
        "#;
    let model = oml_parse_raw(&mut conf).assert();
    let target = model.transform(src, cache);
    let result = Json::stdfmt_record(&target).to_string();
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
    let src = DataRecord {
        items: data.clone(),
    };

    let mut conf = r#"
        name : test
        ---
        X1 : array = collect take(keys :[details[*]/process_name]);
        X2  = pipe read(X1) | nth(0) ;
        X3  = pipe read(X1) | nth(2) ;
        "#;
    let model = oml_parse_raw(&mut conf).assert();

    let target = model.transform(src, cache);

    println!("{}", Json::stdfmt_record(&target));
    let expect = DataField::from_arr("X1".to_string(), data);
    assert_eq!(target.field("X1"), Some(&expect));
    assert_eq!(
        target.field("X2"),
        Some(&DataField::from_chars("X2", "hello1"))
    );
    assert_eq!(
        target.field("X3"),
        Some(&DataField::from_chars("X3", "hello3"))
    );
}
//}
