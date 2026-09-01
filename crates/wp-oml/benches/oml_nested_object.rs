mod support;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use oml::language::ObjModel;
use support::{BenchTransformExt, parse_model};
use wp_knowledge::cache::FieldQueryCache;
use wp_model_core::model::DataRecord;

/// issue #352 的嵌套 `roles_obj` 映射形状。
///
/// 与 ngsoc 真实负载一致：`__sip` / `__dip` / `devIp` 等是「前面已求值的输出字段」（落在 dst），
/// `roles_obj` 里的 `read(...)` 读的是这些 dst 字段。因此热点是 `read` 对 dst 的按名查找
/// （`find_tdc_target_storage` 线性扫描），而不是对输入记录（src）的查找。
const PREDEFINED_FIELDS: &str = r#"
__sip = chars(10.0.0.1);
__sport = chars(443);
__sip_continent_name = chars(Asia);
__sip_country_code = chars(CN);
__sip_country_name = chars(China);
__sip_province_name = chars(Beijing);
__sip_city_name = chars(Beijing);
__sip_latitude = chars(39.9042);
__sip_longitude = chars(116.4074);
__sip_asset_id = chars(sip-asset-1);
__sip_asset_name = chars(sip-host);
__sip_asset_type = chars(host);
__sip_system_id = chars(sys-1);
__sip_system_name = chars(linux);
__sip_company_name = chars(acme);
__dip = chars(10.0.0.2);
__dport = chars(80);
__target_domain = chars(example.com);
__dip_continent_name = chars(Asia);
__dip_country_code = chars(CN);
__dip_country_name = chars(China);
__dip_province_name = chars(Shanghai);
__dip_city_name = chars(Shanghai);
__dip_latitude = chars(31.2304);
__dip_longitude = chars(121.4737);
__dip_asset_id = chars(dip-asset-1);
__dip_asset_name = chars(dip-host);
__dip_asset_type = chars(host);
__dip_system_id = chars(sys-2);
__dip_system_name = chars(linux);
__dip_company_name = chars(acme);
devIp = chars(192.168.1.1);
"#;

const ROLES_OBJ: &str = r#"
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

fn build_model(padding: usize) -> ObjModel {
    let mut oml = String::from("name : nested_roles_obj\n---\n");
    // padding 字段模拟 dst 里排在 `__sip` 之前的大量前置输出字段。
    for i in 0..padding {
        oml.push_str(&format!("pad_{} = chars(x);\n", i));
    }
    oml.push_str(PREDEFINED_FIELDS);
    oml.push_str(ROLES_OBJ);
    parse_model(&oml)
}

fn bench_nested_roles_obj(c: &mut Criterion) {
    let mut group = c.benchmark_group("nested_roles_obj");
    for padding in [0usize, 64, 256, 1024] {
        let model = build_model(padding);
        let input = DataRecord::default();
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
