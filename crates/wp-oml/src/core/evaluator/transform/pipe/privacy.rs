use crate::core::prelude::*;
use crate::language::{
    DEFAULT_IPV4_POOL, DEFAULT_IPV6_POOL, DEFAULT_KEY_NAME, DEFAULT_SALT_NAME, Encoding, Hash,
    HashAlgo, Mask, MaskKind, NameStyle, Tokenize,
};
use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::{Hmac, Mac};
use ipnet::{IpNet, Ipv4Net, Ipv6Net};
use sha1::Sha1;
use sha2::Sha256;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::{Once, OnceLock};
use wp_model_core::model::{DataField, DataType, Value};

const AES_GCM_NONCE_LEN: usize = 12;

static UNSALTED_WARNED: Once = Once::new();
static V4_POOL_MISMATCH_WARNED: Once = Once::new();
static V6_POOL_MISMATCH_WARNED: Once = Once::new();

fn value_text(value: &Value) -> Option<String> {
    match value {
        Value::Chars(v) => Some(v.to_string()),
        Value::Symbol(v) => Some(v.to_string()),
        Value::Domain(v) => Some(v.0.to_string()),
        Value::Url(v) => Some(v.0.to_string()),
        Value::Email(v) => Some(v.0.to_string()),
        Value::IdCard(v) => Some(v.0.to_string()),
        Value::MobilePhone(v) => Some(v.0.to_string()),
        Value::IpAddr(v) => Some(v.to_string()),
        Value::IpNet(v) => Some(v.to_string()),
        Value::Int(v) => Some(v.to_string()),
        Value::Float(v) => Some(v.to_string()),
        Value::Bool(v) => Some(v.to_string()),
        Value::Hex(v) => Some(v.to_string()),
        Value::BigUint(v) => Some(v.to_string()),
        Value::Null | Value::Ignore(_) => None,
        _ => None,
    }
}

fn is_missing(value: &Value) -> bool {
    matches!(value, Value::Null | Value::Ignore(_))
}

fn mask_keep(
    text: &str,
    keep_first: usize,
    keep_last: usize,
    ch: char,
    placeholder: &str,
) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return String::new();
    }
    if keep_first.saturating_add(keep_last) >= chars.len() {
        return placeholder.to_string();
    }
    let mut out = String::with_capacity(text.len());
    for (i, c) in chars.iter().enumerate() {
        if i < keep_first || i >= chars.len() - keep_last {
            out.push(*c);
        } else {
            out.push(ch);
        }
    }
    out
}

fn mask_name(text: &str, style: NameStyle, ch: char) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return String::new();
    }
    if chars.len() == 1 {
        return ch.to_string();
    }
    match style {
        NameStyle::Head => {
            let mut out = String::new();
            out.push(chars[0]);
            out.extend(std::iter::repeat_n(ch, chars.len() - 1));
            out
        }
        NameStyle::Tail => {
            let mut out: String = std::iter::repeat_n(ch, chars.len() - 1).collect();
            out.push(*chars.last().expect("non-empty"));
            out
        }
        NameStyle::Full => std::iter::repeat_n(ch, chars.len()).collect(),
    }
}

fn mask_mail(
    text: &str,
    keep_first: usize,
    keep_domain: bool,
    ch: char,
    placeholder: &str,
) -> String {
    let Some((local, domain)) = text.rsplit_once('@') else {
        return placeholder.to_string();
    };
    let local_len = local.chars().count();
    if local_len == 0 || keep_first >= local_len {
        return placeholder.to_string();
    }
    let mut out: String = local.chars().take(keep_first).collect();
    out.extend(std::iter::repeat_n(ch, 3));
    if keep_domain {
        out.push('@');
        out.push_str(domain);
    }
    out
}

fn mask_domain(text: &str, keep_levels: usize, ch: char, placeholder: &str) -> String {
    let levels: Vec<&str> = text.split('.').filter(|part| !part.is_empty()).collect();
    if levels.len() < keep_levels || keep_levels == 0 {
        return placeholder.to_string();
    }
    let mut out: String = std::iter::repeat_n(ch, 3).collect();
    out.push('.');
    out.push_str(&levels[levels.len() - keep_levels..].join("."));
    out
}

fn parse_ip(text: &str) -> Option<IpAddr> {
    text.trim().parse::<IpAddr>().ok()
}

fn default_pool_v4() -> Ipv4Net {
    static POOL: OnceLock<Ipv4Net> = OnceLock::new();
    *POOL.get_or_init(|| {
        DEFAULT_IPV4_POOL
            .parse()
            .expect("built-in IPv4 privacy pool must be valid")
    })
}

fn default_pool_v6() -> Ipv6Net {
    static POOL: OnceLock<Ipv6Net> = OnceLock::new();
    *POOL.get_or_init(|| {
        DEFAULT_IPV6_POOL
            .parse()
            .expect("built-in IPv6 privacy pool must be valid")
    })
}

fn map_ipv4(ip: Ipv4Addr, salt: &[u8], pool: Ipv4Net) -> Ipv4Addr {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(salt).expect("HMAC accepts any key");
    mac.update(&[4u8]);
    mac.update(&ip.octets());
    let tag = mac.finalize().into_bytes();
    let host_bits = 32 - pool.prefix_len() as u32;
    let host_mask = match host_bits {
        0 => 0,
        32 => u32::MAX,
        n => u32::MAX >> (32 - n),
    };
    let host = u32::from_be_bytes([tag[0], tag[1], tag[2], tag[3]]) & host_mask;
    Ipv4Addr::from(u32::from(pool.network()) | host)
}

fn map_ipv6(ip: Ipv6Addr, salt: &[u8], pool: Ipv6Net) -> Ipv6Addr {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(salt).expect("HMAC accepts any key");
    mac.update(&[6u8]);
    mac.update(&ip.octets());
    let tag = mac.finalize().into_bytes();
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&tag[..16]);
    let host_bits = 128 - pool.prefix_len() as u32;
    let host_mask = match host_bits {
        0 => 0,
        128 => u128::MAX,
        n => u128::MAX >> (128 - n),
    };
    let host = u128::from_be_bytes(bytes) & host_mask;
    Ipv6Addr::from(u128::from(pool.network()) | host)
}

fn select_pool_v4(pool: Option<IpNet>) -> Ipv4Net {
    match pool {
        Some(IpNet::V4(v4)) => v4,
        Some(IpNet::V6(_)) => {
            V4_POOL_MISMATCH_WARNED.call_once(|| {
                log::warn!("privacy: an IPv6 pool is ignored for an IPv4 address");
            });
            default_pool_v4()
        }
        None => default_pool_v4(),
    }
}

fn select_pool_v6(pool: Option<IpNet>) -> Ipv6Net {
    match pool {
        Some(IpNet::V6(v6)) => v6,
        Some(IpNet::V4(_)) => {
            V6_POOL_MISMATCH_WARNED.call_once(|| {
                log::warn!("privacy: an IPv4 pool is ignored for an IPv6 address");
            });
            default_pool_v6()
        }
        None => default_pool_v6(),
    }
}

impl ValueProcessor for Mask {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        let name = in_val.get_name().to_string();
        let Some(text) = value_text(in_val.get_value()) else {
            if is_missing(in_val.get_value()) {
                return in_val;
            }
            return self.fallback(name);
        };
        if text.is_empty() {
            return in_val;
        }

        let ch = self.mask_char();
        let placeholder = self.placeholder();

        if self.kind == MaskKind::Ip {
            let Some(ip) = parse_ip(&text) else {
                return self.fallback(name);
            };
            let salt = match crate::privacy::get_secret(DEFAULT_SALT_NAME) {
                Some(salt) => salt,
                None if self.allow_unsalted => {
                    UNSALTED_WARNED.call_once(|| {
                        log::warn!(
                            "privacy: mask(ip) is running without a salt (allow_unsalted:true)"
                        );
                    });
                    Vec::new()
                }
                None => {
                    log::error!(
                        "privacy: mask(ip) requires secret '{DEFAULT_SALT_NAME}'; refusing to emit a reversible value"
                    );
                    return self.fallback(name);
                }
            };
            let mapped = match ip {
                IpAddr::V4(v4) => IpAddr::V4(map_ipv4(v4, &salt, select_pool_v4(self.pool))),
                IpAddr::V6(v6) => IpAddr::V6(map_ipv6(v6, &salt, select_pool_v6(self.pool))),
            };
            return DataField::from_ip(name, mapped);
        }

        let masked = match self.kind {
            MaskKind::Mobile => mask_keep(&text, 0, self.keep_last.unwrap_or(4), ch, placeholder),
            MaskKind::IdCard => mask_keep(
                &text,
                self.keep_first.unwrap_or(6),
                self.keep_last.unwrap_or(4),
                ch,
                placeholder,
            ),
            MaskKind::Mail => mask_mail(
                &text,
                self.keep_first.unwrap_or(1),
                self.keep_domain.unwrap_or(true),
                ch,
                placeholder,
            ),
            MaskKind::Domain => mask_domain(&text, self.keep_levels.unwrap_or(2), ch, placeholder),
            MaskKind::Name => mask_name(&text, self.name_style.unwrap_or_default(), ch),
            MaskKind::Address => mask_keep(
                &text,
                self.keep_first.unwrap_or(6),
                self.keep_last.unwrap_or(0),
                ch,
                placeholder,
            ),
            MaskKind::Company => mask_keep(
                &text,
                self.keep_first.unwrap_or(4),
                self.keep_last.unwrap_or(0),
                ch,
                placeholder,
            ),
            MaskKind::KeyMsg => placeholder.to_string(),
            MaskKind::Text => {
                if self.keep_first.is_none() && self.keep_last.is_none() {
                    placeholder.to_string()
                } else {
                    mask_keep(
                        &text,
                        self.keep_first.unwrap_or(0),
                        self.keep_last.unwrap_or(0),
                        ch,
                        placeholder,
                    )
                }
            }
            MaskKind::Ip => unreachable!("handled above"),
        };
        DataField::from_chars(name, masked)
    }
}

impl Mask {
    fn fallback(&self, name: String) -> DataField {
        if self.on_error == crate::language::OnError::Fail {
            log::error!(
                "privacy: mask({}) input does not match the selected kind",
                self.kind
            );
        }
        DataField::from_chars(name, self.placeholder().to_string())
    }
}

fn hmac_sha256(key: &[u8], msg: &[u8]) -> Vec<u8> {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key).expect("HMAC accepts any key");
    mac.update(msg);
    mac.finalize().into_bytes().to_vec()
}

fn hmac_sha1(key: &[u8], msg: &[u8]) -> Vec<u8> {
    let mut mac = <Hmac<Sha1> as Mac>::new_from_slice(key).expect("HMAC accepts any key");
    mac.update(msg);
    mac.finalize().into_bytes().to_vec()
}

fn hmac_md5(key: &[u8], msg: &[u8]) -> Vec<u8> {
    const BLOCK: usize = 64;
    let mut key_block = if key.len() > BLOCK {
        md5::compute(key).0.to_vec()
    } else {
        key.to_vec()
    };
    key_block.resize(BLOCK, 0);

    let mut inner = Vec::with_capacity(BLOCK + msg.len());
    inner.extend(key_block.iter().map(|b| b ^ 0x36));
    inner.extend_from_slice(msg);
    let inner_digest = md5::compute(&inner).0;

    let mut outer = Vec::with_capacity(BLOCK + inner_digest.len());
    outer.extend(key_block.iter().map(|b| b ^ 0x5c));
    outer.extend_from_slice(&inner_digest);
    md5::compute(&outer).0.to_vec()
}

fn digest(algo: HashAlgo, key: &[u8], msg: &[u8]) -> Vec<u8> {
    match algo {
        HashAlgo::Sha256 => hmac_sha256(key, msg),
        HashAlgo::Sha1 => hmac_sha1(key, msg),
        HashAlgo::Md5 => hmac_md5(key, msg),
    }
}

impl ValueProcessor for Hash {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        let name = in_val.get_name().to_string();
        let Some(text) = value_text(in_val.get_value()) else {
            if is_missing(in_val.get_value()) {
                return in_val;
            }
            return DataField::from_chars(name, "[REDACTED]");
        };
        if text.is_empty() {
            return in_val;
        }
        let Some(salt) = crate::privacy::get_secret(DEFAULT_SALT_NAME) else {
            log::error!(
                "privacy: hash({}) requires secret '{DEFAULT_SALT_NAME}'; refusing to emit a keyed hash",
                self.algo
            );
            return DataField::from_chars(name, "[REDACTED]");
        };
        let raw = digest(self.algo, &salt, text.as_bytes());
        let mut encoded = match self.encoding {
            Encoding::Hex => hex::encode(raw),
            Encoding::Base64Url => URL_SAFE_NO_PAD.encode(raw),
        };
        if let Some(limit) = self.truncate {
            encoded = encoded.chars().take(limit).collect();
        }
        DataField::from_chars(name, encoded)
    }
}

fn encrypt_token(key: &[u8], plain: &[u8]) -> Option<Vec<u8>> {
    let cipher = Aes256Gcm::new_from_slice(key).ok()?;
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher.encrypt(&nonce, plain).ok()?;
    let mut payload = nonce.to_vec();
    payload.extend_from_slice(&ciphertext);
    Some(payload)
}

/// Decrypt a `tokenize` value. Restoration is intentionally kept outside OML;
/// this helper is the reference implementation for the independent tooling.
pub fn detokenize(token: &str, key: &[u8], prefix: &str) -> Option<String> {
    let payload = token.strip_prefix(prefix)?;
    let raw = URL_SAFE_NO_PAD.decode(payload).ok()?;
    if raw.len() <= AES_GCM_NONCE_LEN {
        return None;
    }
    let (nonce, ciphertext) = raw.split_at(AES_GCM_NONCE_LEN);
    let cipher = Aes256Gcm::new_from_slice(key).ok()?;
    let plain = cipher.decrypt(Nonce::from_slice(nonce), ciphertext).ok()?;
    String::from_utf8(plain).ok()
}

impl ValueProcessor for Tokenize {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        let name = in_val.get_name().to_string();
        let Some(text) = value_text(in_val.get_value()) else {
            if is_missing(in_val.get_value()) {
                return in_val;
            }
            return DataField::from_chars(name, "[REDACTED]");
        };
        if text.is_empty() {
            return in_val;
        }
        let Some(key) = crate::privacy::get_secret(DEFAULT_KEY_NAME) else {
            log::error!(
                "privacy: tokenize requires secret '{DEFAULT_KEY_NAME}'; refusing to emit a plaintext value"
            );
            return DataField::from_chars(name, "[REDACTED]");
        };
        let Some(payload) = encrypt_token(&key, text.as_bytes()) else {
            log::error!(
                "privacy: tokenize failed (key length must be 32 bytes for {})",
                self.algo.name()
            );
            return DataField::from_chars(name, "[REDACTED]");
        };
        DataField::from_chars(
            name,
            format!("{}{}", self.prefix, URL_SAFE_NO_PAD.encode(payload)),
        )
    }
}

impl ValueProcessor for crate::language::Remove {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        // A Null value is skipped by the aggregation layer; the Ignore meta is
        // the marker that lets it also delete an already-emitted target.
        DataField::new(DataType::Ignore, in_val.get_name().to_string(), Value::Null)
    }
}

#[cfg(test)]
mod tests {
    use super::detokenize;
    use crate::core::AsyncDataTransformer;
    use crate::parser::oml_parse_raw;
    use crate::privacy::{set_secret, set_secret_str};
    use wp_knowledge::cache::FieldQueryCache;
    use wp_model_core::model::{DataField, DataRecord, FieldStorage};

    /// Tests that touch the process-wide default secrets (`pii_salt` /
    /// `pii_key`) must not run concurrently.
    static DEFAULT_SECRET_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

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

    fn text_field(dst: &DataRecord, name: &str) -> String {
        dst.field(name)
            .map(|f| format!("{}", f.as_field().get_value()))
            .unwrap_or_else(|| panic!("field {name} missing"))
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_mask_mobile_id_card_mail_domain() {
        let dst = run(
            r#"
name : t
---
mobile  : chars = read(mobile) | mask(mobile) ;
id_card : chars = read(id_card) | mask(id_card) ;
mail    : chars = read(mail) | mask(mail) ;
domain  : chars = read(domain) | mask(domain) ;
"#,
            record(&[
                ("mobile", "13800138000"),
                ("id_card", "110101199003071234"),
                ("mail", "alice@example.com"),
                ("domain", "api.example.com"),
            ]),
        )
        .await;

        assert_eq!(text_field(&dst, "mobile"), "*******8000");
        assert_eq!(text_field(&dst, "id_card"), "110101********1234");
        assert_eq!(text_field(&dst, "mail"), "a***@example.com");
        assert_eq!(text_field(&dst, "domain"), "***.example.com");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_mask_ip_is_deterministic_and_reserved() {
        let _guard = DEFAULT_SECRET_LOCK.lock().await;
        set_secret_str("pii_salt", "unit-ip-salt");
        let oml = r#"
name : t
---
s_ip : ip = read(s_ip) | mask(ip, allow_unsalted:true) ;
"#;
        let src = record(&[("s_ip", "203.0.113.5")]);
        let dst1 = run(oml, src.clone()).await;
        let dst2 = run(oml, src).await;
        let ip1 = text_field(&dst1, "s_ip");
        let ip2 = text_field(&dst2, "s_ip");
        assert_eq!(
            ip1, ip2,
            "same input + same salt must map deterministically"
        );
        let first_octet: u8 = ip1.split('.').next().expect("ipv4").parse().expect("octet");
        assert!(
            first_octet >= 240,
            "default IPv4 pool is 240.0.0.0/4, got {ip1}"
        );
        assert!(ip1.parse::<std::net::IpAddr>().is_ok());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_mask_type_mismatch_fails_closed() {
        let dst = run(
            r#"
name : t
---
s_ip : chars = read(s_ip) | mask(ip, allow_unsalted:true) ;
"#,
            record(&[("s_ip", "not-an-ip")]),
        )
        .await;
        assert_eq!(text_field(&dst, "s_ip"), "[REDACTED]");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_hash_and_remove() {
        let _guard = DEFAULT_SECRET_LOCK.lock().await;
        set_secret_str("pii_salt", "unit-test-salt");
        let dst = run(
            r#"
name : t
---
uid   : chars = read(uid) | hash(sha256) ;
secret: chars = read(secret) | remove ;
"#,
            record(&[("uid", "u-1001"), ("secret", "password")]),
        )
        .await;

        let uid = text_field(&dst, "uid");
        assert_eq!(uid.len(), 64, "sha256 hex is 64 characters");
        assert_ne!(uid, "u-1001");
        assert!(
            dst.field("secret").is_none(),
            "remove must drop the target field"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_tokenize_round_trip() {
        let _guard = DEFAULT_SECRET_LOCK.lock().await;
        let key = b"0123456789abcdef0123456789abcdef".to_vec();
        set_secret("pii_key", key.clone());
        let dst = run(
            r#"
name : t
---
uid : chars = read(uid) | tokenize() ;
"#,
            record(&[("uid", "user-42")]),
        )
        .await;
        let token = text_field(&dst, "uid");
        assert!(token.starts_with("tok:v1:"), "got {token}");
        let plain = detokenize(&token, &key, "tok:v1:").expect("detokenize");
        assert_eq!(plain, "user-42");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_mask_text_name_keymsg_and_options() {
        let dst = run(
            r#"
name : t
---
name_v   : chars = read(name_a) | mask(name) ;
name_t   : chars = read(name_b) | mask(name, style:tail) ;
text_v   : chars = read(text_v) | mask(text, keep_first:2, keep_last:2) ;
keymsg_v : chars = read(keymsg_v) | mask(keymsg) ;
"#,
            record(&[
                ("name_a", "张三丰"),
                ("name_b", "张三丰"),
                ("text_v", "abcdef"),
                ("keymsg_v", "p@ss"),
            ]),
        )
        .await;
        assert_eq!(text_field(&dst, "name_v"), "张**");
        assert_eq!(text_field(&dst, "name_t"), "**丰");
        assert_eq!(text_field(&dst, "text_v"), "ab**ef");
        assert_eq!(text_field(&dst, "keymsg_v"), "[REDACTED]");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_hash_accepts_named_param_first() {
        let _guard = DEFAULT_SECRET_LOCK.lock().await;
        set_secret_str("pii_salt", "named-salt");
        let dst = run(
            r#"
name : t
---
uid : chars = read(uid) | hash(encoding:base64url, truncate:16) ;
"#,
            record(&[("uid", "u-1001")]),
        )
        .await;
        let uid = text_field(&dst, "uid");
        assert_eq!(uid.len(), 16);
        assert_ne!(uid, "u-1001");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_remove_must_be_terminal() {
        let mut conf = r#"
name : t
---
x : chars = read(x) | remove | to_str ;
"#;
        assert!(oml_parse_raw(&mut conf).await.is_err());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_secret_missing_fails_closed() {
        let _guard = DEFAULT_SECRET_LOCK.lock().await;
        set_secret_str("pii_salt", "");
        set_secret_str("pii_key", "");
        let dst = run(
            r#"
name : t
---
h : chars = read(h) | hash(sha256) ;
t : chars = read(t) | tokenize() ;
"#,
            record(&[("h", "value"), ("t", "value")]),
        )
        .await;
        assert_eq!(text_field(&dst, "h"), "[REDACTED]");
        assert_eq!(text_field(&dst, "t"), "[REDACTED]");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_pipe_left_side_generalized() {
        let dst = run(
            r#"
name : t
---
X : chars = fmt("{a}-{b}", @a, read(b)) | mask(text, keep_first:2) ;
Y : chars = match read(level) {
  chars(high) => chars(HELLO),
  _ => chars(LO) ;
} | mask(text, keep_first:1) ;
"#,
            record(&[("a", "ab"), ("b", "cd"), ("level", "high")]),
        )
        .await;
        assert_eq!(text_field(&dst, "X"), "ab***");
        assert_eq!(text_field(&dst, "Y"), "H****");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_object_inner_pipe_parses() {
        let mut conf = r#"
name : t
---
summary : obj = object {
  s_ip : ip = read(src_ip) | mask(ip, allow_unsalted:true) ;
};
"#;
        assert!(oml_parse_raw(&mut conf).await.is_ok());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_batch_target_with_pipe() {
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
        assert_eq!(text_field(&dst, "a_mail"), "a***@example.com");
        assert_eq!(text_field(&dst, "b_mail"), "b***@example.org");
        assert!(
            dst.field("other").is_none(),
            "unmatched fields are not emitted"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_mask_requires_kind() {
        let mut conf = r#"
name : t
---
x : chars = read(x) | mask() ;
"#;
        assert!(oml_parse_raw(&mut conf).await.is_err());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_mask_rejects_text_params_for_ip() {
        let mut conf = r#"
name : t
---
x : ip = read(x) | mask(ip, keep_last:2) ;
"#;
        assert!(oml_parse_raw(&mut conf).await.is_err());
    }

    #[test]
    fn test_hmac_standard_vectors() {
        use super::{hmac_md5, hmac_sha1, hmac_sha256};
        let key = b"Jefe";
        let msg = b"what do ya want for nothing?";
        assert_eq!(
            hex::encode(hmac_sha256(key, msg)),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
        assert_eq!(
            hex::encode(hmac_sha1(key, msg)),
            "effcdf6ae5eb2fa2d27416d5f184df9c259a7c79"
        );
        assert_eq!(
            hex::encode(hmac_md5(key, msg)),
            "750c783e6ab0b503eaa86e310a5db738"
        );
    }

    #[test]
    fn test_hmac_md5_long_key_vector() {
        use super::hmac_md5;
        let key = [0xaau8; 80];
        let msg = b"Test Using Larger Than Block-Size Key - Hash Key First";
        assert_eq!(
            hex::encode(hmac_md5(&key, msg)),
            "6b1ab7fe4bd7bf8f0b62e6ce61b9d0cd"
        );
    }

    #[test]
    fn test_mask_helpers_fall_back_on_short_values() {
        use super::{mask_keep, mask_mail, mask_name};
        use crate::language::NameStyle;
        assert_eq!(mask_keep("1234", 0, 4, '*', "[X]"), "[X]");
        assert_eq!(mask_mail("a@b.com", 1, true, '*', "[X]"), "[X]");
        assert_eq!(mask_name("张", NameStyle::Head, '*'), "*");
        assert_eq!(mask_name("张三", NameStyle::Tail, '*'), "*三");
        assert_eq!(mask_name("张三", NameStyle::Full, '*'), "**");
    }

    #[test]
    fn test_ip_mapping_vectors() {
        use super::{map_ipv4, map_ipv6};
        use ipnet::{Ipv4Net, Ipv6Net};
        let salt = b"test-salt";

        let v4: Ipv4Net = "240.0.0.0/4".parse().expect("v4 pool");
        let out = map_ipv4("203.0.113.5".parse().expect("v4"), salt, v4);
        assert_eq!(out.to_string(), "246.44.48.7");

        let v6: Ipv6Net = "2001:db8::/32".parse().expect("v6 pool");
        let out = map_ipv6("2001:4860:4860::8888".parse().expect("v6"), salt, v6);
        assert_eq!(out.to_string(), "2001:db8:c6e1:cdef:74f:e78:ca8:b9b6");
    }
}
