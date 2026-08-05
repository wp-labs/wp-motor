//! issue #346：嵌套对象 + 对象数组 解析与求值验证
use oml::core::AsyncDataTransformer;
use oml::parser::oml_parse_raw;
use wp_knowledge::cache::FieldQueryCache;
use wp_model_core::model::{DataField, DataRecord, FieldStorage, Value};

const MODEL: &str = r#"
name : nested_test
---
roles_obj = object {
    source = object {
        entity_type = chars(host);
        host = object {
            id = read(asset_id);
            name = read(computer_name);
            ip = read(ip);
        };
    };
};

signatures = array {
    object {
        signer = read(process_sign);
    };
    object {
        signer = read(process_parent_sign);
    };
};
"#;

fn sample_record() -> DataRecord {
    DataRecord::from(vec![
        FieldStorage::from_owned(DataField::from_chars("asset_id", "2868257359929541780")),
        FieldStorage::from_owned(DataField::from_chars("computer_name", "DESKTOP-NGMF7JI")),
        FieldStorage::from_owned(DataField::from_ip("ip", "10.95.209.76".parse().unwrap())),
        FieldStorage::from_owned(DataField::from_chars("process_sign", "Microsoft Windows")),
        FieldStorage::from_owned(DataField::from_chars(
            "process_parent_sign",
            "Microsoft Windows Publisher",
        )),
    ])
}

#[tokio::test]
async fn nested_object_and_array_parse() {
    let mut code: &str = MODEL;
    let model = oml_parse_raw(&mut code).await.expect("parse oml");
    assert_eq!(model.items().len(), 2);
}

#[tokio::test]
async fn nested_object_evaluates_to_nested_json_shape() {
    let mut code: &str = MODEL;
    let model = oml_parse_raw(&mut code).await.expect("parse oml");
    let cache = &mut FieldQueryCache::default();
    let src = sample_record();

    let out = model.transform_async(src, cache).await;

    // roles_obj: Obj{ source: Obj{ entity_type, host: Obj{ id, name, ip } } }
    let roles = out.field("roles_obj").expect("roles_obj").as_field();
    let Value::Obj(source) = roles.get_value() else {
        panic!("roles_obj 应为 Obj，实际 {:?}", roles.get_value());
    };
    let source_field = source.get("source").expect("source key");
    let Value::Obj(source_obj) = source_field.get_value() else {
        panic!("source 应为 Obj");
    };
    assert!(matches!(
        source_obj.get("entity_type").unwrap().get_value(),
        Value::Chars(v) if v == "host"
    ));
    let Value::Obj(host_obj) = source_obj.get("host").unwrap().get_value() else {
        panic!("host 应为 Obj");
    };
    assert!(matches!(
        host_obj.get("id").unwrap().get_value(),
        Value::Chars(v) if v == "2868257359929541780"
    ));
    assert!(matches!(
        host_obj.get("name").unwrap().get_value(),
        Value::Chars(v) if v == "DESKTOP-NGMF7JI"
    ));
    assert!(matches!(
        host_obj.get("ip").unwrap().get_value(),
        Value::IpAddr(ip) if ip.to_string() == "10.95.209.76"
    ));

    // signatures: Array[ Obj{ signer }, Obj{ signer } ]
    let sigs = out.field("signatures").expect("signatures").as_field();
    let Value::Array(items) = sigs.get_value() else {
        panic!("signatures 应为 Array，实际 {:?}", sigs.get_value());
    };
    assert_eq!(items.len(), 2);
    let Value::Obj(first) = items[0].get_value() else {
        panic!("signatures[0] 应为 Obj");
    };
    assert!(matches!(
        first.get("signer").unwrap().get_value(),
        Value::Chars(v) if v == "Microsoft Windows"
    ));
    let Value::Obj(second) = items[1].get_value() else {
        panic!("signatures[1] 应为 Obj");
    };
    assert!(matches!(
        second.get("signer").unwrap().get_value(),
        Value::Chars(v) if v == "Microsoft Windows Publisher"
    ));
}

#[tokio::test]
async fn nested_object_inside_object_and_array_inside_object() {
    let mut code: &str = r#"
name : nested_inside
---
payload = object {
    source = object {
        entity_type = chars(host);
    };
    signatures = array {
        object {
            signer = read(process_sign);
        };
    };
};
"#;
    let model = oml_parse_raw(&mut code).await.expect("parse oml");
    let cache = &mut FieldQueryCache::default();
    let src = DataRecord::from(vec![FieldStorage::from_owned(DataField::from_chars(
        "process_sign",
        "MS",
    ))]);
    let out = model.transform_async(src, cache).await;
    let payload = out.field("payload").expect("payload").as_field();
    let Value::Obj(obj) = payload.get_value() else {
        panic!("payload 应为 Obj");
    };
    assert!(obj.get("source").is_some());
    assert!(obj.get("signatures").is_some());
}

// ======================= 补充用例 =======================

use wp_data_fmt::Json;
use wp_data_fmt::ValueFormatter;

/// 端到端：按 issue #346 期望输出核对 JSON 序列化结果
#[tokio::test]
async fn end_to_end_json_matches_issue_expected_output() {
    let mut code: &str = MODEL;
    let model = oml_parse_raw(&mut code).await.expect("parse oml");
    let cache = &mut FieldQueryCache::default();
    let out = model.transform_async(sample_record(), cache).await;

    let json = Json;
    let roles = out.field("roles_obj").expect("roles_obj").as_field();
    let roles_value: serde_json::Value =
        serde_json::from_str(&json.format_value(roles.get_value())).expect("roles json");
    let expect_roles = serde_json::json!({
        "source": {
            "entity_type": "host",
            "host": {
                "id": "2868257359929541780",
                "name": "DESKTOP-NGMF7JI",
                "ip": "10.95.209.76"
            }
        }
    });
    assert_eq!(roles_value, expect_roles);

    let sigs = out.field("signatures").expect("signatures").as_field();
    let sigs_value: serde_json::Value =
        serde_json::from_str(&json.format_value(sigs.get_value())).expect("sigs json");
    let expect_sigs = serde_json::json!([
        { "signer": "Microsoft Windows" },
        { "signer": "Microsoft Windows Publisher" }
    ]);
    assert_eq!(sigs_value, expect_sigs);
}

/// 深层混合嵌套：object → array → object → array
#[tokio::test]
async fn deep_mixed_nesting() {
    let mut code: &str = r#"
name : deep_mix
---
groups = array {
    object {
        name = chars(g1);
        members = array {
            object { uid = chars(u1); roles = array { chars(r1); chars(r2); }; };
            object { uid = chars(u2); };
        };
    };
    object {
        name = chars(g2);
    };
};
"#;
    let model = oml_parse_raw(&mut code).await.expect("parse oml");
    let cache = &mut FieldQueryCache::default();
    let out = model.transform_async(DataRecord::default(), cache).await;

    let groups = out.field("groups").expect("groups").as_field();
    let Value::Array(items) = groups.get_value() else {
        panic!("groups 应为 Array");
    };
    assert_eq!(items.len(), 2);
    let Value::Obj(g1) = items[0].get_value() else {
        panic!("groups[0] 应为 Obj");
    };
    let Value::Array(members) = g1.get("members").unwrap().get_value() else {
        panic!("members 应为 Array");
    };
    assert_eq!(members.len(), 2);
    let Value::Obj(m1) = members[0].get_value() else {
        panic!("members[0] 应为 Obj");
    };
    let Value::Array(roles) = m1.get("roles").unwrap().get_value() else {
        panic!("roles 应为 Array");
    };
    assert_eq!(roles.len(), 2);
}

/// 数组元素可以是值 / 函数 / read，且混排
#[tokio::test]
async fn array_items_mixed_kinds() {
    let mut code: &str = r#"
name : mixed_items
---
mix = array {
    chars(alpha);
    read(code);
    chars(omega);
};
"#;
    let model = oml_parse_raw(&mut code).await.expect("parse oml");
    let cache = &mut FieldQueryCache::default();
    let src = DataRecord::from(vec![FieldStorage::from_owned(DataField::from_digit(
        "code", 42,
    ))]);
    let out = model.transform_async(src, cache).await;
    let mix = out.field("mix").expect("mix").as_field();
    let Value::Array(items) = mix.get_value() else {
        panic!("mix 应为 Array");
    };
    assert_eq!(items.len(), 3);
    assert!(matches!(
        items[0].get_value(),
        Value::Chars(v) if v == "alpha"
    ));
    assert!(matches!(items[1].get_value(), Value::Digit(v) if *v == 42));
    assert!(matches!(
        items[2].get_value(),
        Value::Chars(v) if v == "omega"
    ));
}

/// 数组元素缺失（read 不到）→ 跳过；全部缺失 → 字段省略
#[tokio::test]
async fn array_missing_items_are_skipped() {
    let mut code: &str = r#"
name : skip_missing
---
all_missing = array {
    read(no_such_a);
    read(no_such_b);
};
part_missing = array {
    read(no_such_a);
    read(exist);
};
"#;
    let model = oml_parse_raw(&mut code).await.expect("parse oml");
    let cache = &mut FieldQueryCache::default();
    let src = DataRecord::from(vec![FieldStorage::from_owned(DataField::from_chars(
        "exist", "ok",
    ))]);
    let out = model.transform_async(src, cache).await;
    // 全部缺失 → 不产出字段
    assert!(out.field("all_missing").is_none());
    // 部分缺失 → 只保留存在的元素
    let part = out.field("part_missing").expect("part_missing").as_field();
    let Value::Array(items) = part.get_value() else {
        panic!("part_missing 应为 Array");
    };
    assert_eq!(items.len(), 1);
    assert!(matches!(items[0].get_value(), Value::Chars(v) if v == "ok"));
}

/// 数组的数组（嵌套 array 字面量）
#[tokio::test]
async fn array_of_arrays() {
    let mut code: &str = r#"
name : arr_of_arr
---
matrix = array {
    array { chars(a1); chars(a2); };
    array { chars(b1); };
};
"#;
    let model = oml_parse_raw(&mut code).await.expect("parse oml");
    let cache = &mut FieldQueryCache::default();
    let out = model.transform_async(DataRecord::default(), cache).await;
    let matrix = out.field("matrix").expect("matrix").as_field();
    let Value::Array(rows) = matrix.get_value() else {
        panic!("matrix 应为 Array");
    };
    assert_eq!(rows.len(), 2);
    let Value::Array(row0) = rows[0].get_value() else {
        panic!("matrix[0] 应为 Array");
    };
    assert_eq!(row0.len(), 2);
    let Value::Array(row1) = rows[1].get_value() else {
        panic!("matrix[1] 应为 Array");
    };
    assert_eq!(row1.len(), 1);
}

/// 显式类型声明 `: obj` 与 `: array` 不丢失字段
#[tokio::test]
async fn typed_object_and_array_declarations() {
    let mut code: &str = r#"
name : typed
---
roles : obj = object {
    source = object { entity_type = chars(host); };
};
sigs : array = array {
    object { signer = read(process_sign); };
};
"#;
    let model = oml_parse_raw(&mut code).await.expect("parse oml");
    let cache = &mut FieldQueryCache::default();
    let src = DataRecord::from(vec![FieldStorage::from_owned(DataField::from_chars(
        "process_sign",
        "MS",
    ))]);
    let out = model.transform_async(src, cache).await;
    let roles = out.field("roles").expect("roles").as_field();
    assert!(matches!(roles.get_value(), Value::Obj(_)));
    let sigs = out.field("sigs").expect("sigs").as_field();
    assert!(matches!(sigs.get_value(), Value::Array(_)));
}

/// 同一模型内后续语句可 read 前面的对象输出，并用 get 管道取嵌套字段
#[tokio::test]
async fn get_pipe_on_nested_object_output() {
    let mut code: &str = r#"
name : get_nested
---
roles_obj = object {
    source = object {
        entity_type = chars(host);
        host = object {
            id = read(asset_id);
            name = read(computer_name);
        };
    };
};
role_id = pipe read(roles_obj) | get(source/host/id) ;
"#;
    let model = oml_parse_raw(&mut code).await.expect("parse oml");
    let cache = &mut FieldQueryCache::default();
    let out = model.transform_async(sample_record(), cache).await;
    let role_id = out.field("role_id").expect("role_id").as_field();
    assert!(matches!(
        role_id.get_value(),
        Value::Chars(v) if v == "2868257359929541780"
    ));
}

/// static 块中的嵌套对象/数组：物化后可作为字段引用
#[tokio::test]
async fn static_block_nested_object_and_array() {
    let mut code: &str = r#"
name : static_nested
---
static {
    tpl = object {
        nested = object { id = chars(E1); };
        list = array {
            object { v = chars(a); };
            object { v = chars(b); };
        };
    };
}
result = object {
    clone = tpl;
};
"#;
    let model = oml_parse_raw(&mut code).await.expect("parse oml");
    let cache = &mut FieldQueryCache::default();
    let out = model.transform_async(DataRecord::default(), cache).await;
    let result = out.field("result").expect("result").as_field();
    let Value::Obj(res_obj) = result.get_value() else {
        panic!("result 应为 Obj");
    };
    let Value::Obj(clone) = res_obj.get("clone").unwrap().get_value() else {
        panic!("clone 应为 Obj");
    };
    let Value::Obj(nested) = clone.get("nested").unwrap().get_value() else {
        panic!("nested 应为 Obj");
    };
    assert!(matches!(
        nested.get("id").unwrap().get_value(),
        Value::Chars(v) if v == "E1"
    ));
    let Value::Array(list) = clone.get("list").unwrap().get_value() else {
        panic!("list 应为 Array");
    };
    assert_eq!(list.len(), 2);
}

/// Display 输出可再次解析（往返一致）
#[tokio::test]
async fn display_round_trip_reparse() {
    let mut code: &str = MODEL;
    let model = oml_parse_raw(&mut code).await.expect("parse oml");
    let rendered = format!("{}", model);
    let mut code2 = rendered.as_str();
    let model2 = oml_parse_raw(&mut code2).await.expect("re-parse oml");
    assert_eq!(format!("{}", model2), rendered);
}

/// 空数组 `array { }`：解析通过，运行期无元素则不产出字段
#[tokio::test]
async fn empty_array_is_omitted() {
    let mut code: &str = r#"
name : empty_arr
---
empty = array { };
"#;
    let model = oml_parse_raw(&mut code).await.expect("parse oml");
    let cache = &mut FieldQueryCache::default();
    let out = model.transform_async(DataRecord::default(), cache).await;
    assert!(out.field("empty").is_none());
}

/// 数组元素之间省略分号也可解析
#[tokio::test]
async fn array_items_without_semicolons() {
    let mut code: &str = r#"
name : no_semi
---
list = array {
    object { v = chars(a); }
    object { v = chars(b); }
};
"#;
    let model = oml_parse_raw(&mut code).await.expect("parse oml");
    let cache = &mut FieldQueryCache::default();
    let out = model.transform_async(DataRecord::default(), cache).await;
    let list = out.field("list").expect("list").as_field();
    let Value::Array(items) = list.get_value() else {
        panic!("list 应为 Array");
    };
    assert_eq!(items.len(), 2);
}

/// 文档示例（core-concepts / complete-example 8.3+8.4）可解析并求值
#[tokio::test]
async fn doc_examples_parse_and_evaluate() {
    // 文档：07-complete-example 8.3/8.4
    let mut code: &str = r#"
name : doc_example
---
signatures = array {
    object { signer = read(process_sign); };
    object { signer = read(process_parent_sign); };
};
roles_obj = object {
    source = object {
        entity_type = chars(host);
        host = object {
            id = read(asset_id);
            name = read(computer_name);
        };
    };
    tags = array {
        chars(web);
        chars(proxy);
    };
};
"#;
    let model = oml_parse_raw(&mut code).await.expect("parse oml");
    let cache = &mut FieldQueryCache::default();
    let out = model.transform_async(sample_record(), cache).await;

    let sigs = out.field("signatures").expect("signatures").as_field();
    assert!(matches!(sigs.get_value(), Value::Array(_)));

    let roles = out.field("roles_obj").expect("roles_obj").as_field();
    let Value::Obj(roles_obj) = roles.get_value() else {
        panic!("roles_obj 应为 Obj");
    };
    let Value::Obj(source) = roles_obj.get("source").unwrap().get_value() else {
        panic!("source 应为 Obj");
    };
    let Value::Obj(host) = source.get("host").unwrap().get_value() else {
        panic!("host 应为 Obj");
    };
    assert!(matches!(
        host.get("id").unwrap().get_value(),
        Value::Chars(v) if v == "2868257359929541780"
    ));
    let Value::Array(tags) = roles_obj.get("tags").unwrap().get_value() else {
        panic!("tags 应为 Array");
    };
    assert_eq!(tags.len(), 2);
    assert!(matches!(tags[0].get_value(), Value::Chars(v) if v == "web"));
    assert!(matches!(tags[1].get_value(), Value::Chars(v) if v == "proxy"));
}
