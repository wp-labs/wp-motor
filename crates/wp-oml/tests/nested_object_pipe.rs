//! issue #348：嵌套 object 成员含 pipe 时，后续兄弟字段必须全部保留，
//! 且非法 object 成员必须使整个 OML 校验失败（不得静默丢弃）。
use oml::core::AsyncDataTransformer;
use oml::language::{EvalExp, NestedAccessor, PreciseEvaluator};
use oml::parser::oml_parse_raw;
use wp_knowledge::cache::FieldQueryCache;
use wp_model_core::model::{DataField, DataRecord, FieldStorage, Value};

/// 与 issue #348 最小复现一致：rule 子对象成员是 pipe，其后还有标量与另一个 object
const MODEL: &str = r#"
name : nested_object_tail
---
parser_bug_probe = object {
    before = chars(before);
    rule = object {
        signature_id = pipe read(missing_rule_ids) | nth(0);
    };
    after = chars(after);
    nested_after = object {
        value = chars(also_after);
    };
};
"#;

/// 解析层面：嵌套 pipe 成员及其后兄弟字段全部保留
#[tokio::test]
async fn nested_pipe_object_parse_preserves_tail_members() {
    let mut code: &str = MODEL;
    let model = oml_parse_raw(&mut code).await.expect("parse oml");
    assert_eq!(model.items().len(), 1);

    let EvalExp::Single(single) = &model.items()[0] else {
        panic!("expected Single");
    };
    let PreciseEvaluator::Map(map) = single.eval_way() else {
        panic!("expected Map");
    };
    let names: Vec<String> = map.subs().iter().map(|b| b.target().safe_name()).collect();
    assert_eq!(names, vec!["before", "rule", "after", "nested_after"]);

    let NestedAccessor::Map(rule) = map.subs()[1].acquirer() else {
        panic!("rule 应为 Map");
    };
    assert!(matches!(rule.subs()[0].acquirer(), NestedAccessor::Pipe(_)));
}

/// 求值层面：pipe 源字段缺失时，仅缺失子字段被跳过，结构与其后兄弟字段全部保留
#[tokio::test]
async fn missing_pipe_source_keeps_structure_and_siblings() {
    let mut code: &str = MODEL;
    let model = oml_parse_raw(&mut code).await.expect("parse oml");
    let cache = &mut FieldQueryCache::default();
    // 只提供 name，故意缺失 missing_rule_ids
    let src = DataRecord::from(vec![FieldStorage::from_owned(DataField::from_chars(
        "name", "demo",
    ))]);

    let out = model.transform_async(src, cache).await;
    let probe = out
        .field("parser_bug_probe")
        .expect("parser_bug_probe 不应被丢弃")
        .as_field();
    let Value::Obj(obj) = probe.get_value() else {
        panic!("parser_bug_probe 应为 Obj，实际 {:?}", probe.get_value());
    };
    assert!(matches!(
        obj.get("before").unwrap().get_value(),
        Value::Chars(v) if v == "before"
    ));
    assert!(matches!(
        obj.get("after").unwrap().get_value(),
        Value::Chars(v) if v == "after"
    ));
    let Value::Obj(nested) = obj.get("nested_after").unwrap().get_value() else {
        panic!("nested_after 应为 Obj");
    };
    assert!(matches!(
        nested.get("value").unwrap().get_value(),
        Value::Chars(v) if v == "also_after"
    ));
    // rule 子对象结构保留（源字段缺失 → 空对象）
    assert!(
        matches!(obj.get("rule").unwrap().get_value(), Value::Obj(_)),
        "rule 应保留为 Obj，实际 {:?}",
        obj.get("rule").unwrap().get_value()
    );
}

/// 求值层面：pipe 源字段存在时，嵌套 pipe 成员正常求值
#[tokio::test]
async fn present_pipe_source_evaluates_nested_member() {
    let mut code: &str = r#"
name : pipe_present
---
parser_bug_probe = object {
    before = chars(before);
    rule = object {
        signature_id = pipe read(missing_rule_ids) | map_to('found');
    };
    after = chars(after);
};
"#;
    let model = oml_parse_raw(&mut code).await.expect("parse oml");
    let cache = &mut FieldQueryCache::default();
    let src = DataRecord::from(vec![FieldStorage::from_owned(DataField::from_chars(
        "missing_rule_ids",
        "abc",
    ))]);

    let out = model.transform_async(src, cache).await;
    let probe = out
        .field("parser_bug_probe")
        .expect("parser_bug_probe")
        .as_field();
    let Value::Obj(obj) = probe.get_value() else {
        panic!("parser_bug_probe 应为 Obj");
    };
    let Value::Obj(rule) = obj.get("rule").unwrap().get_value() else {
        panic!("rule 应为 Obj");
    };
    assert!(matches!(
        rule.get("signature_id").unwrap().get_value(),
        Value::Chars(v) if v == "found"
    ));
    assert!(matches!(
        obj.get("after").unwrap().get_value(),
        Value::Chars(v) if v == "after"
    ));
}

/// 非法 object 成员必须使整个 OML 校验失败，而不是静默丢弃
#[tokio::test]
async fn invalid_object_member_fails_whole_oml() {
    let mut code: &str = r#"
name : bad
---
probe = object {
    before = chars(before);
    after = ;
};
"#;
    let result = oml_parse_raw(&mut code).await;
    assert!(result.is_err(), "非法 object 成员必须使整个 OML 校验失败");
}

/// 复现 issue 中的真实结构：嵌套 `rule = object {...}`（成员全为 pipe）
/// 位于 `category` 之后、`behavior/confidence/attacker` 之前，后者必须全部保留
const REPORTED_SHAPE: &str = r#"
name : source_finding
---
source_finding_obj = object {
    title = read(name);
    category = object {
        original = object { name = read(ruleCategoryName); };
    };
    rule = object {
        signature_id = pipe read(ruleId) | nth(0);
        name = pipe read(ruleName) | nth(0);
    };
    behavior = read(ruleCategoryName);
    confidence = read(confidence);
    attacker = object {
        endpoint = object { ip = read(__attacker_ip); };
    };
};
"#;

fn reported_record() -> DataRecord {
    DataRecord::from(vec![
        FieldStorage::from_owned(DataField::from_chars("name", "demo")),
        FieldStorage::from_owned(DataField::from_chars("ruleCategoryName", "弱口令")),
        FieldStorage::from_owned(DataField::from_chars("confidence", "高")),
        FieldStorage::from_owned(DataField::from_ip(
            "__attacker_ip",
            "10.86.10.191".parse().unwrap(),
        )),
    ])
}

/// 真实结构：解析层面 7 个成员全部保留
#[tokio::test]
async fn reported_shape_parse_keeps_all_siblings() {
    let mut code: &str = REPORTED_SHAPE;
    let model = oml_parse_raw(&mut code).await.expect("parse oml");
    let EvalExp::Single(single) = &model.items()[0] else {
        panic!("expected Single");
    };
    let PreciseEvaluator::Map(map) = single.eval_way() else {
        panic!("expected Map");
    };
    let names: Vec<String> = map.subs().iter().map(|b| b.target().safe_name()).collect();
    assert_eq!(
        names,
        vec![
            "title",
            "category",
            "rule",
            "behavior",
            "confidence",
            "attacker"
        ]
    );
    let NestedAccessor::Map(rule) = map.subs()[2].acquirer() else {
        panic!("rule 应为 Map");
    };
    let inner: Vec<String> = rule.subs().iter().map(|b| b.target().safe_name()).collect();
    assert_eq!(inner, vec!["signature_id", "name"]);
    assert!(matches!(rule.subs()[0].acquirer(), NestedAccessor::Pipe(_)));
    assert!(matches!(rule.subs()[1].acquirer(), NestedAccessor::Pipe(_)));
}

/// 真实结构：ruleId/ruleName 缺失时，rule 为空对象，其后 behavior/confidence/attacker 全部保留
#[tokio::test]
async fn reported_shape_missing_rule_sources_keeps_rest() {
    let mut code: &str = REPORTED_SHAPE;
    let model = oml_parse_raw(&mut code).await.expect("parse oml");
    let cache = &mut FieldQueryCache::default();
    // 故意不提供 ruleId/ruleName
    let out = model.transform_async(reported_record(), cache).await;

    let obj = out
        .field("source_finding_obj")
        .expect("source_finding_obj 不应被丢弃")
        .as_field();
    let Value::Obj(v) = obj.get_value() else {
        panic!("source_finding_obj 应为 Obj");
    };
    assert!(matches!(
        v.get("title").unwrap().get_value(),
        Value::Chars(s) if s == "demo"
    ));
    assert!(matches!(
        v.get("behavior").unwrap().get_value(),
        Value::Chars(s) if s == "弱口令"
    ));
    assert!(matches!(
        v.get("confidence").unwrap().get_value(),
        Value::Chars(s) if s == "高"
    ));
    let Value::Obj(category) = v.get("category").unwrap().get_value() else {
        panic!("category 应为 Obj");
    };
    let Value::Obj(original) = category.get("original").unwrap().get_value() else {
        panic!("original 应为 Obj");
    };
    assert!(matches!(
        original.get("name").unwrap().get_value(),
        Value::Chars(s) if s == "弱口令"
    ));
    let Value::Obj(attacker) = v.get("attacker").unwrap().get_value() else {
        panic!("attacker 应为 Obj");
    };
    let Value::Obj(endpoint) = attacker.get("endpoint").unwrap().get_value() else {
        panic!("endpoint 应为 Obj");
    };
    assert!(matches!(
        endpoint.get("ip").unwrap().get_value(),
        Value::IpAddr(ip) if ip.to_string() == "10.86.10.191"
    ));
    // rule 子对象保留结构（源缺失 → 空对象）
    assert!(matches!(v.get("rule").unwrap().get_value(), Value::Obj(_)));
}

/// pipe 成员在 object 内位置不限：开头 / 中间 / 末尾均不丢失兄弟字段
#[tokio::test]
async fn pipe_member_at_any_position() {
    let mut code: &str = r#"
name : positions
---
probe = object {
    first = pipe read(a) | to_str;
    mid = chars(m);
    last = pipe read(b) | to_str;
};
"#;
    let model = oml_parse_raw(&mut code).await.expect("parse oml");
    let EvalExp::Single(single) = &model.items()[0] else {
        panic!("expected Single");
    };
    let PreciseEvaluator::Map(map) = single.eval_way() else {
        panic!("expected Map");
    };
    let names: Vec<String> = map.subs().iter().map(|b| b.target().safe_name()).collect();
    assert_eq!(names, vec!["first", "mid", "last"]);
    let cache = &mut FieldQueryCache::default();
    let src = DataRecord::from(vec![
        FieldStorage::from_owned(DataField::from_chars("a", "A")),
        FieldStorage::from_owned(DataField::from_chars("b", "B")),
    ]);
    let out = model.transform_async(src, cache).await;
    let obj = out.field("probe").expect("probe").as_field();
    let Value::Obj(v) = obj.get_value() else {
        panic!("probe 应为 Obj");
    };
    assert!(matches!(
        v.get("first").unwrap().get_value(),
        Value::Chars(s) if s == "A"
    ));
    assert!(matches!(
        v.get("mid").unwrap().get_value(),
        Value::Chars(s) if s == "m"
    ));
    assert!(matches!(
        v.get("last").unwrap().get_value(),
        Value::Chars(s) if s == "B"
    ));
}

/// 省略 pipe 前缀（`read(x) | to_str`）在 object 成员中同样可用
#[tokio::test]
async fn omitted_pipe_prefix_in_object_member() {
    let mut code: &str = r#"
name : no_prefix
---
probe = object {
    before = chars(before);
    sig = read(rule_id) | to_str;
    after = chars(after);
};
"#;
    let model = oml_parse_raw(&mut code).await.expect("parse oml");
    let cache = &mut FieldQueryCache::default();
    let src = DataRecord::from(vec![FieldStorage::from_owned(DataField::from_chars(
        "rule_id", "abc",
    ))]);
    let out = model.transform_async(src, cache).await;
    let obj = out.field("probe").expect("probe").as_field();
    let Value::Obj(v) = obj.get_value() else {
        panic!("probe 应为 Obj");
    };
    assert!(matches!(
        v.get("before").unwrap().get_value(),
        Value::Chars(s) if s == "before"
    ));
    assert!(matches!(
        v.get("sig").unwrap().get_value(),
        Value::Chars(s) if s == "abc"
    ));
    assert!(matches!(
        v.get("after").unwrap().get_value(),
        Value::Chars(s) if s == "after"
    ));
}

/// take 源 pipe（`pipe take(x) | to_str`）在 object 成员中可用
#[tokio::test]
async fn take_source_pipe_in_object_member() {
    let mut code: &str = r#"
name : take_pipe
---
probe = object {
    s = pipe take(src_field) | to_str;
    tail = chars(done);
};
"#;
    let model = oml_parse_raw(&mut code).await.expect("parse oml");
    let cache = &mut FieldQueryCache::default();
    let src = DataRecord::from(vec![FieldStorage::from_owned(DataField::from_chars(
        "src_field",
        "v1",
    ))]);
    let out = model.transform_async(src, cache).await;
    let obj = out.field("probe").expect("probe").as_field();
    let Value::Obj(v) = obj.get_value() else {
        panic!("probe 应为 Obj");
    };
    assert!(matches!(
        v.get("s").unwrap().get_value(),
        Value::Chars(s) if s == "v1"
    ));
    assert!(matches!(
        v.get("tail").unwrap().get_value(),
        Value::Chars(s) if s == "done"
    ));
}

/// 同一嵌套 object 内多个 pipe 成员 + 其后标量
#[tokio::test]
async fn multiple_pipe_members_in_nested_object() {
    let mut code: &str = r#"
name : multi_pipe
---
probe = object {
    rule = object {
        a = pipe read(x) | to_str;
        b = pipe read(y) | to_str;
        tail = chars(z);
    };
    after = chars(ok);
};
"#;
    let model = oml_parse_raw(&mut code).await.expect("parse oml");
    let cache = &mut FieldQueryCache::default();
    let src = DataRecord::from(vec![
        FieldStorage::from_owned(DataField::from_chars("x", "1")),
        FieldStorage::from_owned(DataField::from_chars("y", "2")),
    ]);
    let out = model.transform_async(src, cache).await;
    let obj = out.field("probe").expect("probe").as_field();
    let Value::Obj(v) = obj.get_value() else {
        panic!("probe 应为 Obj");
    };
    let Value::Obj(rule) = v.get("rule").unwrap().get_value() else {
        panic!("rule 应为 Obj");
    };
    assert!(matches!(
        rule.get("a").unwrap().get_value(),
        Value::Chars(s) if s == "1"
    ));
    assert!(matches!(
        rule.get("b").unwrap().get_value(),
        Value::Chars(s) if s == "2"
    ));
    assert!(matches!(
        rule.get("tail").unwrap().get_value(),
        Value::Chars(s) if s == "z"
    ));
    assert!(matches!(
        v.get("after").unwrap().get_value(),
        Value::Chars(s) if s == "ok"
    ));
}

/// 数组元素内的嵌套 object 支持 pipe 成员，且不破坏数组其余元素
#[tokio::test]
async fn pipe_member_inside_array_item() {
    let mut code: &str = r#"
name : arr_pipe
---
items = array {
    object {
        signer = pipe read(process_sign) | to_str;
        tail = chars(t1);
    };
    object {
        signer = read(process_parent_sign);
    };
};
"#;
    let model = oml_parse_raw(&mut code).await.expect("parse oml");
    let cache = &mut FieldQueryCache::default();
    let src = DataRecord::from(vec![
        FieldStorage::from_owned(DataField::from_chars("process_sign", "MS")),
        FieldStorage::from_owned(DataField::from_chars(
            "process_parent_sign",
            "Windows Publisher",
        )),
    ]);
    let out = model.transform_async(src, cache).await;
    let items = out.field("items").expect("items").as_field();
    let Value::Array(arr) = items.get_value() else {
        panic!("items 应为 Array");
    };
    assert_eq!(arr.len(), 2);
    let Value::Obj(first) = arr[0].get_value() else {
        panic!("items[0] 应为 Obj");
    };
    assert!(matches!(
        first.get("signer").unwrap().get_value(),
        Value::Chars(s) if s == "MS"
    ));
    assert!(matches!(
        first.get("tail").unwrap().get_value(),
        Value::Chars(s) if s == "t1"
    ));
    let Value::Obj(second) = arr[1].get_value() else {
        panic!("items[1] 应为 Obj");
    };
    assert!(matches!(
        second.get("signer").unwrap().get_value(),
        Value::Chars(s) if s == "Windows Publisher"
    ));
}

/// 深层混合嵌套：object → array → object（pipe 成员）→ 标量/object
#[tokio::test]
async fn deep_nesting_object_array_pipe() {
    let mut code: &str = r#"
name : deep_pipe
---
payload = object {
    groups = array {
        object {
            gid = pipe read(group_id) | to_str;
            members = object { leader = read(leader); };
        };
    };
    after = chars(done);
};
"#;
    let model = oml_parse_raw(&mut code).await.expect("parse oml");
    let cache = &mut FieldQueryCache::default();
    let src = DataRecord::from(vec![
        FieldStorage::from_owned(DataField::from_chars("group_id", "g1")),
        FieldStorage::from_owned(DataField::from_chars("leader", "l1")),
    ]);
    let out = model.transform_async(src, cache).await;
    let obj = out.field("payload").expect("payload").as_field();
    let Value::Obj(v) = obj.get_value() else {
        panic!("payload 应为 Obj");
    };
    let Value::Array(groups) = v.get("groups").unwrap().get_value() else {
        panic!("groups 应为 Array");
    };
    let Value::Obj(g0) = groups[0].get_value() else {
        panic!("groups[0] 应为 Obj");
    };
    assert!(matches!(
        g0.get("gid").unwrap().get_value(),
        Value::Chars(s) if s == "g1"
    ));
    let Value::Obj(members) = g0.get("members").unwrap().get_value() else {
        panic!("members 应为 Obj");
    };
    assert!(matches!(
        members.get("leader").unwrap().get_value(),
        Value::Chars(s) if s == "l1"
    ));
    assert!(matches!(
        v.get("after").unwrap().get_value(),
        Value::Chars(s) if s == "done"
    ));
}

/// object 成员内 pipe 对前序语句输出的嵌套对象取路径（get 管道）
#[tokio::test]
async fn get_pipe_on_earlier_object_output_in_member() {
    let mut code: &str = r#"
name : get_member
---
roles_obj = object {
    source = object { host = object { id = read(asset_id); }; };
};
role_id = object {
    sig = pipe read(roles_obj) | get(source/host/id);
    tail = chars(done);
};
"#;
    let model = oml_parse_raw(&mut code).await.expect("parse oml");
    let cache = &mut FieldQueryCache::default();
    let src = DataRecord::from(vec![FieldStorage::from_owned(DataField::from_chars(
        "asset_id",
        "2868257359929541780",
    ))]);
    let out = model.transform_async(src, cache).await;
    let obj = out.field("role_id").expect("role_id").as_field();
    let Value::Obj(v) = obj.get_value() else {
        panic!("role_id 应为 Obj");
    };
    assert!(matches!(
        v.get("sig").unwrap().get_value(),
        Value::Chars(s) if s == "2868257359929541780"
    ));
    assert!(matches!(
        v.get("tail").unwrap().get_value(),
        Value::Chars(s) if s == "done"
    ));
}

/// Display 往返：含 pipe 成员的嵌套 object 模型可被 Display 后重新解析
#[tokio::test]
async fn display_round_trip_pipe_in_object() {
    let mut code: &str = MODEL;
    let model = oml_parse_raw(&mut code).await.expect("parse oml");
    let rendered = format!("{}", model);
    let mut code2 = rendered.as_str();
    let model2 = oml_parse_raw(&mut code2).await.expect("re-parse oml");
    assert_eq!(format!("{}", model2), rendered);
}
