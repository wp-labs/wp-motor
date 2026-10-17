//! End-to-end tests for the OML privacy operators.
//!
//! Run with:
//! `cargo test -p wp-oml --test privacy_operators`

use oml::{AsyncDataTransformer, detokenize, oml_parse_raw, set_secret, set_secret_str};
use wp_knowledge::cache::FieldQueryCache;
use wp_model_core::model::{DataField, DataRecord, FieldStorage, Value};

fn record(fields: &[(&str, &str)]) -> DataRecord {
    DataRecord::from(
        fields
            .iter()
            .map(|(k, v)| FieldStorage::from_owned(DataField::from_chars(*k, *v)))
            .collect::<Vec<_>>(),
    )
}

async fn run(oml: &str, src: DataRecord) -> DataRecord {
    let mut conf: &str = oml;
    let model = oml_parse_raw(&mut conf).await.expect("oml parse");
    let mut cache = FieldQueryCache::default();
    model.transform_async(src, &mut cache).await
}

fn text(dst: &DataRecord, name: &str) -> String {
    dst.field(name)
        .map(|field| format!("{}", field.as_field().get_value()))
        .unwrap_or_else(|| panic!("field {name} missing"))
}

fn obj_text(dst: &DataRecord, obj: &str, key: &str) -> Option<String> {
    match dst.field(obj)?.as_field().get_value() {
        Value::Obj(value) => value
            .get(key)
            .map(|field| format!("{}", field.as_field().get_value())),
        _ => None,
    }
}

async fn parses(oml: &str) -> bool {
    let mut conf: &str = oml;
    oml_parse_raw(&mut conf).await.is_ok()
}

/// Tests that touch the process-wide *default* secret names (`pii_salt` /
/// `pii_key`) must not run concurrently, because they overwrite the same
/// registry entries.
static DEFAULT_SECRET_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

// ---------------------------------------------------------------------------
// mask
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "current_thread")]
async fn mask_defaults_for_all_kinds() {
    let dst = run(
        r#"
name : t
---
mobile  : chars = read(mobile) | mask(mobile) ;
id_card : chars = read(id_card) | mask(id_card) ;
mail    : chars = read(mail) | mask(mail) ;
domain  : chars = read(domain) | mask(domain) ;
name_v  : chars = read(name_v) | mask(name) ;
address : chars = read(address) | mask(address) ;
company : chars = read(company) | mask(company) ;
keymsg  : chars = read(keymsg) | mask(keymsg) ;
text_v  : chars = read(text_v) | mask(text) ;
"#,
        record(&[
            ("mobile", "13800138000"),
            ("id_card", "110101199003071234"),
            ("mail", "alice@example.com"),
            ("domain", "api.example.com"),
            ("name_v", "张三丰"),
            ("address", "1234567890"),
            ("company", "AcmeCorp"),
            ("keymsg", "p@ss"),
            ("text_v", "hello"),
        ]),
    )
    .await;

    assert_eq!(text(&dst, "mobile"), "*******8000");
    assert_eq!(text(&dst, "id_card"), "110101********1234");
    assert_eq!(text(&dst, "mail"), "a***@example.com");
    assert_eq!(text(&dst, "domain"), "***.example.com");
    assert_eq!(text(&dst, "name_v"), "张**");
    assert_eq!(text(&dst, "address"), "123456****");
    assert_eq!(text(&dst, "company"), "Acme****");
    assert_eq!(text(&dst, "keymsg"), "[REDACTED]");
    assert_eq!(text(&dst, "text_v"), "[REDACTED]");
}

#[tokio::test(flavor = "current_thread")]
async fn mask_custom_char_placeholder_and_levels() {
    let dst = run(
        r#"
name : t
---
mobile   : chars = read(mobile) | mask(mobile, keep_last:2, char:'#') ;
too_long : chars = read(mobile) | mask(mobile, keep_last:99, placeholder:'<hidden>') ;
mail     : chars = read(mail) | mask(mail, keep_domain:false) ;
domain   : chars = read(domain) | mask(domain, keep_levels:1) ;
name     : chars = read(name) | mask(name, style:full) ;
"#,
        record(&[
            ("mobile", "13800138000"),
            ("mail", "alice@example.com"),
            ("domain", "api.example.com"),
            ("name", "张三"),
        ]),
    )
    .await;

    assert_eq!(text(&dst, "mobile"), "#########00");
    assert_eq!(text(&dst, "too_long"), "<hidden>");
    assert_eq!(text(&dst, "mail"), "a***");
    assert_eq!(text(&dst, "domain"), "***.com");
    assert_eq!(text(&dst, "name"), "**");
}

#[tokio::test(flavor = "current_thread")]
async fn mask_short_and_empty_values() {
    let dst = run(
        r#"
name : t
---
mobile : chars = read(mobile) | mask(mobile) ;
mail   : chars = read(mail) | mask(mail) ;
empty  : chars = read(empty) | mask(mobile) ;
"#,
        record(&[("mobile", "1234"), ("mail", "a@b.com"), ("empty", "")]),
    )
    .await;

    // Keeping more characters than the value contains must fall back to the
    // placeholder rather than reveal the whole value.
    assert_eq!(text(&dst, "mobile"), "[REDACTED]");
    assert_eq!(text(&dst, "mail"), "[REDACTED]");
    assert_eq!(text(&dst, "empty"), "");
}

#[tokio::test(flavor = "current_thread")]
async fn mask_ip_default_pools_and_custom_pools() {
    let dst = run(
        r#"
name : t
---
v4       : ip = read(v4a) | mask(ip, allow_unsalted:true) ;
v6       : ip = read(v6a) | mask(ip, allow_unsalted:true) ;
v4_32    : ip = read(v4b) | mask(ip, allow_unsalted:true, pool:203.0.113.1/32) ;
v4_bench : ip = read(v4c) | mask(ip, allow_unsalted:true, pool:198.18.0.0/15) ;
v6_128   : ip = read(v6b) | mask(ip, allow_unsalted:true, pool:2001:db8::1/128) ;
"#,
        record(&[
            ("v4a", "198.51.100.7"),
            ("v4b", "198.51.100.7"),
            ("v4c", "198.51.100.7"),
            ("v6a", "2001:4860:4860::8888"),
            ("v6b", "2001:4860:4860::8888"),
        ]),
    )
    .await;

    let v4: std::net::IpAddr = text(&dst, "v4").parse().expect("v4");
    assert!(v4.is_ipv4());
    assert!(
        text(&dst, "v4").starts_with("240.") || {
            let first: u8 = text(&dst, "v4").split('.').next().unwrap().parse().unwrap();
            first >= 240
        }
    );

    let v6 = text(&dst, "v6");
    assert!(v6.starts_with("2001:db8:"), "default IPv6 pool, got {v6}");
    assert!(v6.parse::<std::net::IpAddr>().is_ok());

    // Regression: /32 and /128 pools used to shift by the full width.
    assert_eq!(text(&dst, "v4_32"), "203.0.113.1");
    assert_eq!(text(&dst, "v6_128"), "2001:db8::1");

    // `ip` output converted to a `chars` target must keep the mapped value.
    // Note: `read(v4)` is resolved against `dst` first, so use distinct sources
    // in `mask_ip_to_chars_target_keeps_mapped_value` instead of re-reading `v4`.
    let bench = text(&dst, "v4_bench");
    let octets: Vec<u8> = bench
        .split('.')
        .map(|part| part.parse::<u8>().expect("octet"))
        .collect();
    assert!(
        octets.len() == 4 && octets[0] == 198 && matches!(octets[1], 18 | 19),
        "198.18.0.0/15 custom pool must be honoured, got {bench}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn mask_ip_to_chars_target_keeps_mapped_value() {
    let _guard = DEFAULT_SECRET_LOCK.lock().await;
    let dst = run(
        r#"
name : t
---
as_ip   : ip    = read(ip1) | mask(ip, allow_unsalted:true) ;
as_text : chars = read(ip2) | mask(ip, allow_unsalted:true) ;
"#,
        record(&[("ip1", "198.51.100.7"), ("ip2", "198.51.100.7")]),
    )
    .await;
    assert_eq!(text(&dst, "as_ip"), text(&dst, "as_text"));
}

#[tokio::test(flavor = "current_thread")]
async fn mask_ip_is_salt_dependent_and_fails_closed_without_salt() {
    let _guard = DEFAULT_SECRET_LOCK.lock().await;

    let oml = r#"
name : t
---
ip : ip = read(ip) | mask(ip) ;
"#;

    set_secret_str("pii_salt", "salt-a");
    let a = text(&run(oml, record(&[("ip", "203.0.113.5")])).await, "ip");

    set_secret_str("pii_salt", "salt-b");
    let b = text(&run(oml, record(&[("ip", "203.0.113.5")])).await, "ip");
    assert_ne!(a, b, "changing pii_salt must change the mapping");

    // Missing salt fails closed. The target is `ip`, so the text placeholder
    // cannot be converted and the field is dropped instead of leaking the
    // original input.
    set_secret_str("pii_salt", "");
    let dst = run(oml, record(&[("ip", "203.0.113.5")])).await;
    assert!(dst.field("ip").is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn mask_ip_type_mismatch_is_fail_closed() {
    let dst = run(
        r#"
name : t
---
ip : chars = read(ip) | mask(ip, allow_unsalted:true, on_error:fail) ;
"#,
        record(&[("ip", "not-an-ip")]),
    )
    .await;
    assert_eq!(text(&dst, "ip"), "[REDACTED]");
}

#[tokio::test(flavor = "current_thread")]
async fn mask_ip_pool_family_mismatch_uses_default_family_pool() {
    let dst = run(
        r#"
name : t
---
v4 : ip = read(v4) | mask(ip, allow_unsalted:true, pool:2001:db8:1::/48) ;
v6 : ip = read(v6) | mask(ip, allow_unsalted:true, pool:198.18.0.0/15) ;
"#,
        record(&[("v4", "203.0.113.5"), ("v6", "2001:4860:4860::8888")]),
    )
    .await;

    let v4 = text(&dst, "v4");
    assert!(
        v4.split('.')
            .next()
            .and_then(|first| first.parse::<u8>().ok())
            .is_some_and(|first| first >= 240),
        "a v6 pool must fall back to the default v4 pool, got {v4}"
    );
    let v6 = text(&dst, "v6");
    assert!(
        v6.starts_with("2001:db8:"),
        "a v4 pool must fall back to the default v6 pool, got {v6}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn mask_edge_cases_for_name_and_domain() {
    let dst = run(
        r#"
name : t
---
n1 : chars = read(n1) | mask(name) ;
n3 : chars = read(n3) | mask(name, style:tail) ;
d0 : chars = read(d0) | mask(domain, keep_levels:0) ;
d1 : chars = read(d1) | mask(domain) ;
"#,
        record(&[
            ("n1", "张"),
            ("n3", "张三丰"),
            ("d0", "api.example.com"),
            ("d1", "localhost"),
        ]),
    )
    .await;

    assert_eq!(text(&dst, "n1"), "*");
    assert_eq!(text(&dst, "n3"), "**丰");
    assert_eq!(text(&dst, "d0"), "[REDACTED]");
    assert_eq!(text(&dst, "d1"), "[REDACTED]");
}

#[tokio::test(flavor = "current_thread")]
async fn mask_huge_keep_values_do_not_panic() {
    let dst = run(
        r#"
name : t
---
m : chars = read(v) | mask(text, keep_first:18446744073709551615, keep_last:18446744073709551615) ;
"#,
        record(&[("v", "abc")]),
    )
    .await;
    assert_eq!(text(&dst, "m"), "[REDACTED]");
}

// ---------------------------------------------------------------------------
// zero-config defaults (`SEC_PII_SALT` / `SEC_PII_KEY`)
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "current_thread")]
async fn default_operators_use_sec_pii_salt_from_env_dict() {
    use base64::Engine as _;
    use orion_variate::vars::{EnvDict, ValueType};

    let _guard = DEFAULT_SECRET_LOCK.lock().await;

    let salt = b"00112233445566778899aabbccddeeff";
    let key = b"0123456789abcdef0123456789abcdef";

    let mut dict = EnvDict::default();
    dict.insert(
        "SEC_PII_SALT",
        ValueType::from(base64::engine::general_purpose::STANDARD.encode(salt)),
    );
    dict.insert(
        "SEC_PII_KEY",
        ValueType::from(base64::engine::general_purpose::STANDARD.encode(key)),
    );
    oml::set_secrets_from_env_dict(&dict);

    let dst = run(
        r#"
name : t
---
ip : ip    = read(ip) | mask(ip) ;
h  : chars = read(v)  | hash(sha256) ;
h2 : chars = read(v)  | hash() ;
h3 : chars = read(v)  | hash(sha256) ;
tok: chars = read(v)  | tokenize() ;
"#,
        record(&[("ip", "203.0.113.5"), ("v", "user-42")]),
    )
    .await;

    let mapped: std::net::IpAddr = text(&dst, "ip").parse().expect("mapped IP");
    assert!(mapped.is_ipv4());
    assert!(
        text(&dst, "ip")
            .split('.')
            .next()
            .and_then(|first| first.parse::<u8>().ok())
            .is_some_and(|first| first >= 240),
        "default IPv4 pool is 240.0.0.0/4, got {}",
        text(&dst, "ip")
    );
    assert_eq!(text(&dst, "h").len(), 64);
    assert_eq!(
        text(&dst, "h"),
        text(&dst, "h2"),
        "hash() must default to sha256"
    );
    assert_eq!(
        text(&dst, "h"),
        text(&dst, "h3"),
        "the default salt must be deterministic"
    );
    assert_eq!(
        detokenize(&text(&dst, "tok"), key, "tok:v1:").as_deref(),
        Some("user-42")
    );

    // A second run must map the same input to the same output.
    let again = run(
        r#"
name : t
---
ip : ip = read(ip) | mask(ip) ;
"#,
        record(&[("ip", "203.0.113.5")]),
    )
    .await;
    assert_eq!(text(&again, "ip"), text(&dst, "ip"));
}

// ---------------------------------------------------------------------------
// hash
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "current_thread")]
async fn hash_algos_encodings_and_truncate() {
    let _guard = DEFAULT_SECRET_LOCK.lock().await;
    set_secret_str("pii_salt", "hash-salt");

    let dst = run(
        r#"
name : t
---
sha256_hex : chars = read(v) | hash(sha256) ;
sha1_hex   : chars = read(v) | hash(sha1) ;
md5_hex    : chars = read(v) | hash(md5) ;
b64        : chars = read(v) | hash(sha256, encoding:base64url) ;
short      : chars = read(v) | hash(sha256, truncate:8) ;
same       : chars = read(v) | hash(sha256) ;
empty      : chars = read(empty) | hash(sha256) ;
"#,
        record(&[("v", "value"), ("empty", "")]),
    )
    .await;

    assert_eq!(text(&dst, "sha256_hex").len(), 64);
    assert_eq!(text(&dst, "sha1_hex").len(), 40);
    assert_eq!(text(&dst, "md5_hex").len(), 32);
    assert_eq!(text(&dst, "b64").len(), 43); // 32 bytes base64url without padding
    assert_eq!(text(&dst, "short").len(), 8);
    assert_eq!(text(&dst, "sha256_hex"), text(&dst, "same"));
    assert_eq!(text(&dst, "empty"), "");
    let first_hash = text(&dst, "sha256_hex");

    set_secret_str("pii_salt", "other-salt");
    let dst = run(
        r#"
name : t
---
h : chars = read(v) | hash(sha256) ;
"#,
        record(&[("v", "value")]),
    )
    .await;
    assert_ne!(text(&dst, "h"), first_hash);
}

#[tokio::test(flavor = "current_thread")]
async fn hash_missing_salt_fails_closed() {
    let _guard = DEFAULT_SECRET_LOCK.lock().await;
    set_secret_str("pii_salt", "");
    let dst = run(
        r#"
name : t
---
h : chars = read(h) | hash(sha256) ;
"#,
        record(&[("h", "value")]),
    )
    .await;
    assert_eq!(text(&dst, "h"), "[REDACTED]");
}

#[tokio::test(flavor = "current_thread")]
async fn hash_truncate_longer_than_digest_is_noop() {
    let _guard = DEFAULT_SECRET_LOCK.lock().await;
    set_secret_str("pii_salt", "hash-trunc-salt");
    let dst = run(
        r#"
name : t
---
h : chars = read(v) | hash(sha256, truncate:100) ;
b : chars = read(v) | hash(sha256, encoding:base64url, truncate:100) ;
"#,
        record(&[("v", "value")]),
    )
    .await;
    assert_eq!(text(&dst, "h").len(), 64);
    assert_eq!(text(&dst, "b").len(), 43);
}

// ---------------------------------------------------------------------------
// tokenize
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "current_thread")]
async fn tokenize_custom_prefix_and_round_trip() {
    let _guard = DEFAULT_SECRET_LOCK.lock().await;
    let key = b"0123456789abcdef0123456789abcdef".to_vec();
    set_secret("pii_key", key.clone());

    let oml = r#"
name : t
---
a : chars = read(v) | tokenize(prefix:'tok:v2:') ;
b : chars = read(v) | tokenize(prefix:'tok:v2:') ;
empty : chars = read(empty) | tokenize(prefix:'tok:v2:') ;
"#;
    let dst = run(oml, record(&[("v", "user-42"), ("empty", "")])).await;
    let a = text(&dst, "a");
    let b = text(&dst, "b");
    assert!(a.starts_with("tok:v2:"), "got {a}");
    assert!(b.starts_with("tok:v2:"), "got {b}");
    assert_ne!(a, b, "tokenize must use a fresh nonce");
    assert_eq!(detokenize(&a, &key, "tok:v2:").as_deref(), Some("user-42"));
    assert_eq!(detokenize(&b, &key, "tok:v2:").as_deref(), Some("user-42"));
    assert_eq!(text(&dst, "empty"), "");
    assert!(detokenize(&a, b"wrong-key", "tok:v2:").is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn tokenize_hex_key_is_decoded() {
    let _guard = DEFAULT_SECRET_LOCK.lock().await;
    let key_hex = "00".repeat(32);
    set_secret_str("pii_key", &key_hex);
    let dst = run(
        r#"
name : t
---
t : chars = read(v) | tokenize() ;
"#,
        record(&[("v", "user-42")]),
    )
    .await;
    let token = text(&dst, "t");
    assert_eq!(
        detokenize(&token, &[0u8; 32], "tok:v1:").as_deref(),
        Some("user-42")
    );
}

#[tokio::test(flavor = "current_thread")]
async fn tokenize_invalid_key_length_fails_closed() {
    let _guard = DEFAULT_SECRET_LOCK.lock().await;
    set_secret("pii_key", b"too-short".to_vec());
    let dst = run(
        r#"
name : t
---
t : chars = read(v) | tokenize() ;
"#,
        record(&[("v", "user-42")]),
    )
    .await;
    assert_eq!(text(&dst, "t"), "[REDACTED]");
}

// ---------------------------------------------------------------------------
// remove
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "current_thread")]
async fn remove_drops_target_and_existing_target() {
    let dst = run(
        r#"
name : t
---
keep : chars = read(keep) ;
drop : chars = read(drop) | remove ;
"#,
        record(&[("keep", "v"), ("drop", "secret")]),
    )
    .await;
    assert_eq!(text(&dst, "keep"), "v");
    assert!(dst.field("drop").is_none());

    // A later remove must also delete an already-emitted target.
    let dst = run(
        r#"
name : t
---
X : chars = read(a) ;
X : chars = read(b) | remove ;
"#,
        record(&[("a", "first"), ("b", "second")]),
    )
    .await;
    assert!(
        dst.field("X").is_none(),
        "remove must delete existing target"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn remove_inside_object_and_batch() {
    let dst = run(
        r#"
name : t
---
summary : obj = object {
  s_ip : ip    = read(src_ip) | mask(ip, allow_unsalted:true) ;
  drop : chars = read(drop) | remove ;
  tag  : chars = read(tag) ;
};
"#,
        record(&[("src_ip", "203.0.113.5"), ("drop", "secret"), ("tag", "ok")]),
    )
    .await;
    assert!(obj_text(&dst, "summary", "s_ip").is_some());
    assert!(obj_text(&dst, "summary", "drop").is_none());
    assert_eq!(obj_text(&dst, "summary", "tag").as_deref(), Some("ok"));

    // A duplicate key followed by remove must delete the earlier value.
    let dst = run(
        r#"
name : t
---
summary : obj = object {
  x : chars = read(a) ;
  x : chars = read(b) | remove ;
  y : chars = read(c) ;
};
"#,
        record(&[("a", "first"), ("b", "second"), ("c", "ok")]),
    )
    .await;
    assert!(obj_text(&dst, "summary", "x").is_none());
    assert_eq!(obj_text(&dst, "summary", "y").as_deref(), Some("ok"));

    let dst = run(
        r#"
name : t
---
*_tmp : chars = read() | remove ;
"#,
        record(&[("a_tmp", "1"), ("b_tmp", "2"), ("keep", "3")]),
    )
    .await;
    assert!(dst.field("a_tmp").is_none());
    assert!(dst.field("b_tmp").is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn remove_with_missing_source_deletes_existing_target() {
    let dst = run(
        r#"
name : t
---
X : chars = read(a) ;
X : chars = read(missing) | remove ;
keep : chars = read(b) ;
"#,
        record(&[("a", "leak"), ("b", "ok")]),
    )
    .await;
    assert!(dst.field("X").is_none());
    assert_eq!(text(&dst, "keep"), "ok");
}

#[tokio::test(flavor = "current_thread")]
async fn batch_remove_deletes_existing_target() {
    let dst = run(
        r#"
name : t
---
a_tmp : chars = chars(leak) ;
*_tmp : chars = read() | remove ;
keep  : chars = read(keep) ;
"#,
        record(&[("a_tmp", "src1"), ("b_tmp", "src2"), ("keep", "ok")]),
    )
    .await;
    assert!(
        dst.field("a_tmp").is_none(),
        "batch remove must delete an already-emitted target"
    );
    assert!(dst.field("b_tmp").is_none());
    assert_eq!(text(&dst, "keep"), "ok");
}

// ---------------------------------------------------------------------------
// secret hygiene
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "current_thread")]
async fn empty_or_blank_secrets_are_treated_as_missing() {
    let _guard = DEFAULT_SECRET_LOCK.lock().await;
    set_secret_str("pii_salt", "   ");
    set_secret("pii_key", Vec::<u8>::new());

    let dst = run(
        r#"
name : t
---
h  : chars = read(v)  | hash(sha256) ;
tok: chars = read(v)  | tokenize() ;
ip : ip    = read(ip) | mask(ip) ;
"#,
        record(&[("v", "value"), ("ip", "203.0.113.5")]),
    )
    .await;

    assert_eq!(text(&dst, "h"), "[REDACTED]");
    assert_eq!(text(&dst, "tok"), "[REDACTED]");
    assert!(
        dst.field("ip").is_none(),
        "an empty salt must never be used as a key"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn allow_unsalted_permits_missing_default_salt() {
    let _guard = DEFAULT_SECRET_LOCK.lock().await;
    set_secret_str("pii_salt", "");

    let dst = run(
        r#"
name : t
---
ip  : ip = read(ip1) | mask(ip) ;
ipt : ip = read(ip2) | mask(ip, allow_unsalted:true) ;
"#,
        record(&[("ip1", "203.0.113.5"), ("ip2", "203.0.113.6")]),
    )
    .await;

    // Default path fails closed; the explicit development escape hatch maps.
    assert!(dst.field("ip").is_none());
    assert!(dst.field("ipt").is_some());
}

// ---------------------------------------------------------------------------
// parser validation
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "current_thread")]
async fn parser_rejects_invalid_privacy_syntax() {
    let cases = [
        // missing / unknown kind
        "x : chars = read(x) | mask() ;",
        "x : chars = read(x) | mask(unknown) ;",
        // parameters that do not belong to the kind
        "x : chars = read(x) | mask(mobile, keep_levels:2) ;",
        "x : chars = read(x) | mask(mail, pool:198.18.0.0/15) ;",
        "x : ip    = read(x) | mask(ip, keep_last:2) ;",
        "x : chars = read(x) | mask(text, style:head) ;",
        "x : chars = read(x) | mask(text, unknown:1) ;",
        "x : chars = read(x) | mask(mobile, char:'ab') ;",
        "x : chars = read(x) | mask(mobile, char:'') ;",
        // hash / tokenize errors
        "x : chars = read(x) | hash(sha999) ;",
        "x : chars = read(x) | hash(sha256, encoding:nope) ;",
        "x : chars = read(x) | hash(sha256, unknown:1) ;",
        "x : chars = read(x) | tokenize(unknown:1) ;",
        // `salt:` / `key:` overrides were removed
        "x : chars = read(x) | hash(sha256, salt:pii_salt) ;",
        "x : ip    = read(x) | mask(ip, salt:pii_salt) ;",
        "x : chars = read(x) | tokenize(key:pii_key) ;",
        // remove must be terminal
        "x : chars = read(x) | remove | to_str ;",
    ];
    for case in cases {
        let oml = format!("name : t\n---\n{case}\n");
        assert!(!parses(&oml).await, "expected parse error for: {case}");
    }
}

#[tokio::test(flavor = "current_thread")]
async fn parser_accepts_valid_parameter_combinations() {
    let cases = [
        "x : chars = read(x) | mask(text, keep_first:2, keep_last:2, char:'#', placeholder:'P', on_error:fail) ;",
        "x : chars = read(x) | mask(mobile, keep_last:2, char:*) ;",
        "x : chars = read(x) | mask(mail, keep_first:2, keep_domain:false) ;",
        "x : chars = read(x) | mask(domain, keep_levels:1) ;",
        "x : chars = read(x) | mask(name, style:tail) ;",
        "x : ip    = read(x) | mask(ip, pool:198.18.0.0/15, allow_unsalted:true, placeholder:'P') ;",
        "x : chars = read(x) | hash() ;",
        "x : chars = read(x) | hash(md5, encoding:base64url, truncate:8) ;",
        "x : chars = read(x) | hash(algo:sha1) ;",
        "x : chars = read(x) | tokenize(algo:aes_gcm, prefix:'tok:v2:') ;",
        "x : chars = read(x) | tokenize() ;",
        "x : chars = read(x) | tokenize(prefix:'tok:v2:') ;",
    ];
    for case in cases {
        let oml = format!("name : t\n---\n{case}\n");
        assert!(parses(&oml).await, "expected successful parse for: {case}");
    }
}

// ---------------------------------------------------------------------------
// DSL generalization
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "current_thread")]
async fn generalized_pipe_sources() {
    let dst = run(
        r#"
name : t
---
fmt_mask : chars = fmt("{a}-{b}", @a, read(b)) | mask(text, keep_first:2) ;
match_mask : chars = match read(level) {
  chars(high) => chars(HELLO),
  _ => chars(LO) ;
} | mask(text, keep_first:1) ;
default_body : chars = read(missing) { _ : chars(Fallback) } | mask(text, keep_first:1) ;
keyword_form : chars = pipe read(a) | mask(text, keep_first:1) ;
constant_form : chars = chars(hello) | mask(text, keep_first:1) ;
"#,
        record(&[("a", "ab"), ("b", "cd"), ("level", "high")]),
    )
    .await;

    assert_eq!(text(&dst, "fmt_mask"), "ab***");
    assert_eq!(text(&dst, "match_mask"), "H****");
    assert_eq!(text(&dst, "default_body"), "F*******");
    assert_eq!(text(&dst, "keyword_form"), "a*");
    assert_eq!(text(&dst, "constant_form"), "h****");
}

#[tokio::test(flavor = "current_thread")]
async fn batch_pipe_respects_declared_target_type() {
    // Batch matching selects IP fields; mask(mobile) produces text, which must
    // not be emitted under an `ip` target.
    let src = DataRecord::from(vec![FieldStorage::from_owned(DataField::from_ip(
        "a_ip",
        "203.0.113.5".parse().unwrap(),
    ))]);
    let dst = run(
        r#"
name : t
---
*_ip : ip = read() | mask(mobile) ;
"#,
        src,
    )
    .await;
    assert!(dst.field("a_ip").is_none(), "type mismatch must be dropped");
}

#[tokio::test(flavor = "current_thread")]
async fn object_inner_pipe_and_batch_pipe() {
    let dst = run(
        r#"
name : t
---
summary : obj = object {
  s_ip : ip = read(src_ip) | mask(ip, allow_unsalted:true) ;
};
"#,
        record(&[("src_ip", "203.0.113.5")]),
    )
    .await;
    let masked = obj_text(&dst, "summary", "s_ip").expect("nested s_ip");
    assert!(masked.parse::<std::net::IpAddr>().is_ok());

    let dst = run(
        r#"
name : t
---
*_mail : chars = read() | mask(mail) ;
"#,
        record(&[
            ("a_mail", "alice@example.com"),
            ("b_mail", "bob@example.org"),
            ("other", "keep"),
        ]),
    )
    .await;
    assert_eq!(text(&dst, "a_mail"), "a***@example.com");
    assert_eq!(text(&dst, "b_mail"), "b***@example.org");
    assert!(dst.field("other").is_none());
}
