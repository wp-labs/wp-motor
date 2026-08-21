mod support;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use oml::language::ObjModel;
use support::{BenchTransformExt, parse_model};
use wp_knowledge::cache::FieldQueryCache;
use wp_model_core::model::{DataField, DataRecord, FieldStorage};

/// issue #352 的嵌套 `roles_obj` 映射形状。
///
/// 关键特征：单个顶层 `object { ... }`，内部包含约 30 个 `read(...)`，
/// 每个 `read` 都会对输入记录做按名字查找。输入记录越大，线性扫描成本越高。
const NESTED_OML: &str = r#"
name : nested_roles_obj
---
roles_obj = object {
  carriers = array {};
  related = array {};
  source = object {
    endpoint = object {
      ip = read(__sip);
      ipv4 = read(__sip);
      port = read(__sport);
    };
    geo = object {
      continent = object { name = read(__sip_continent_name); };
      country = object { code = read(__sip_country_code); name = read(__sip_country_name); };
      region = object { name = read(__sip_province_name); };
      city = object { name = read(__sip_city_name); };
      coordinates = object { latitude = read(__sip_latitude); longitude = read(__sip_longitude); };
    };
    resource = object {
      id = read(__sip_asset_id);
      name = read(__sip_asset_name);
      type = read(__sip_asset_type);
      system = object { id = read(__sip_system_id); name = read(__sip_system_name); };
      organization = object { name = read(__sip_company_name); };
    };
  };
  target = object {
    endpoint = object {
      ip = read(__dip);
      ipv4 = read(__dip);
      port = read(__dport);
    };
    domain = object { name = read(__target_domain); };
    geo = object {
      continent = object { name = read(__dip_continent_name); };
      country = object { code = read(__dip_country_code); name = read(__dip_country_name); };
      region = object { name = read(__dip_province_name); };
      city = object { name = read(__dip_city_name); };
      coordinates = object { latitude = read(__dip_latitude); longitude = read(__dip_longitude); };
    };
    resource = object {
      id = read(__dip_asset_id);
      name = read(__dip_asset_name);
      type = read(__dip_asset_type);
      system = object { id = read(__dip_system_id); name = read(__dip_system_name); };
      organization = object { name = read(__dip_company_name); };
    };
  };
  observer = object {
    product = object { name = chars(ngsoc); };
    device = object {
      vendor = chars(qax);
      ip = read(devIp);
      ipv4 = read(devIp);
      ip_addresses = array {
        object { address = read(devIp); };
      };
    };
  };
};
"#;

fn build_model() -> ObjModel {
    parse_model(NESTED_OML)
}

fn push(fields: &mut Vec<FieldStorage>, name: &str, value: &str) {
    fields.push(FieldStorage::from_owned(DataField::from_chars(name, value)));
}

fn sample_record(padding: usize) -> DataRecord {
    let mut fields: Vec<FieldStorage> = Vec::with_capacity(padding + 32);
    // 用 padding 模拟告警富化后输入记录里的大量 `__*` 字段。
    for i in 0..padding {
        push(&mut fields, &format!("pad_field_{}", i), "x");
    }

    push(&mut fields, "__sip", "10.0.0.1");
    push(&mut fields, "__sport", "443");
    push(&mut fields, "__sip_continent_name", "Asia");
    push(&mut fields, "__sip_country_code", "CN");
    push(&mut fields, "__sip_country_name", "China");
    push(&mut fields, "__sip_province_name", "Beijing");
    push(&mut fields, "__sip_city_name", "Beijing");
    push(&mut fields, "__sip_latitude", "39.9042");
    push(&mut fields, "__sip_longitude", "116.4074");
    push(&mut fields, "__sip_asset_id", "sip-asset-1");
    push(&mut fields, "__sip_asset_name", "sip-host");
    push(&mut fields, "__sip_asset_type", "host");
    push(&mut fields, "__sip_system_id", "sys-1");
    push(&mut fields, "__sip_system_name", "linux");
    push(&mut fields, "__sip_company_name", "acme");

    push(&mut fields, "__dip", "10.0.0.2");
    push(&mut fields, "__dport", "80");
    push(&mut fields, "__target_domain", "example.com");
    push(&mut fields, "__dip_continent_name", "Asia");
    push(&mut fields, "__dip_country_code", "CN");
    push(&mut fields, "__dip_country_name", "China");
    push(&mut fields, "__dip_province_name", "Shanghai");
    push(&mut fields, "__dip_city_name", "Shanghai");
    push(&mut fields, "__dip_latitude", "31.2304");
    push(&mut fields, "__dip_longitude", "121.4737");
    push(&mut fields, "__dip_asset_id", "dip-asset-1");
    push(&mut fields, "__dip_asset_name", "dip-host");
    push(&mut fields, "__dip_asset_type", "host");
    push(&mut fields, "__dip_system_id", "sys-2");
    push(&mut fields, "__dip_system_name", "linux");
    push(&mut fields, "__dip_company_name", "acme");

    push(&mut fields, "devIp", "192.168.1.1");

    DataRecord::from(fields)
}

fn bench_nested_roles_obj(c: &mut Criterion) {
    let model = build_model();

    let mut group = c.benchmark_group("nested_roles_obj");
    for padding in [0usize, 64, 256, 1024] {
        let input = sample_record(padding);
        group.bench_function(format!("padding_{}", padding), |b| {
            b.iter_batched(
                || input.clone(),
                |data| {
                    let mut cache = FieldQueryCache::default();
                    model.transform(data, &mut cache)
                },
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

criterion_group!(benches, bench_nested_roles_obj);
criterion_main!(benches);
