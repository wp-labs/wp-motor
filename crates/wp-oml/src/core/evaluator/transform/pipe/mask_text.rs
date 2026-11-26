//! Text-scanning `mask_text` operator: `mask_text(kind, [params])`.
//!
//! Unlike `mask(kind)`, which treats the whole input as a single value and
//! fails closed on a type mismatch, `mask_text` locates occurrences of `kind`
//! inside the text, masks them, and leaves the rest untouched. This is a
//! *pattern-based masking* contract (best effort): it does not claim that
//! the whole field was sanitized, so callers that need the fail-closed
//! guarantee should keep using `mask`.

use super::privacy::{
    is_missing, map_ipv4, map_ipv6, mask_domain, mask_keep, mask_mail, select_pool_v4,
    select_pool_v6, value_text,
};
use crate::core::prelude::*;
use crate::language::{DEFAULT_SALT_NAME, MaskText, MaskTextKind};
use regex::Regex;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Once, OnceLock};
use wp_model_core::model::DataField;

static UNSALTED_WARNED: Once = Once::new();
static SALT_MISSING_LOGGED: Once = Once::new();

macro_rules! lazy_re {
    ($name:ident, $pattern:expr) => {
        fn $name() -> &'static Regex {
            static RE: OnceLock<Regex> = OnceLock::new();
            RE.get_or_init(|| Regex::new($pattern).expect("built-in privacy regex must compile"))
        }
    };
}

// E-mail: conservative local part, at least one dot in the domain, alphabetic
// TLD. Avoids matching `password=Dayu@1234` (no TLD).
lazy_re!(
    email_re,
    r"[A-Za-z0-9._%+\-]+@(?:[A-Za-z0-9](?:[A-Za-z0-9\-]{0,61}[A-Za-z0-9])?\.)+[A-Za-z]{2,11}"
);
lazy_re!(mobile_re, r"(?-u)\b1[3-9]\d{9}\b");
lazy_re!(id_card_re, r"(?-u)\b\d{17}[\dXx]\b");
lazy_re!(ipv4_re, r"(?-u)\b(?:\d{1,3}\.){3}\d{1,3}\b");
lazy_re!(
    domain_re,
    r"(?-u)\b(?:[A-Za-z0-9](?:[A-Za-z0-9\-]{0,61}[A-Za-z0-9])?\.)+[A-Za-z]{2,11}\b"
);
// Credential-ish `key=value` / `key: value` pairs. `authorization` is handled
// by `http_auth_re` (it needs to keep the scheme), so it is not listed here.
// The left boundary accepts `_` / `-` so keys like `db_password` and
// `client_secret` are covered as well.
lazy_re!(
    secret_kv_re,
    r#"(?i)(?:^|[^A-Za-z0-9])(?:password|passwd|pwd|pass|secret|token|api[_-]?key|access[_-]?key|secret[_-]?key|auth[_-]?token)\s*[=:]\s*(?P<value>"[^"]*"|'[^']*'|[^\s,;]+)"#
);
// One HTTP header per line. The header name and the auth scheme are kept; only
// the credential value is replaced.
lazy_re!(
    http_auth_re,
    r"(?im)^(?P<head>[ \t]*(?:proxy-authorization|authorization|x-(?:[a-z0-9]+-)*auth-token|cookie|set-cookie)[ \t]*:[ \t]*)(?P<scheme>(?:basic|bearer|digest|negotiate|ntlm|token)[ \t]+)?(?P<value>[^\r\n]+)"
);
// Fallback for auth headers embedded in a single-line value: the line-anchored
// pattern above cannot see them. Values are token-shaped here, so stop at the
// usual separators instead of consuming the rest of the line.
lazy_re!(
    auth_token_inline_re,
    r"(?i)(?P<head>(?:(?:proxy-)?authorization|x-(?:[a-z0-9]+-)*auth-token)[ \t]*:[ \t]*)(?P<scheme>(?:basic|bearer|digest|negotiate|ntlm|token)[ \t]+)?(?P<value>[^\s,;]+)"
);

impl ValueProcessor for MaskText {
    fn value_cacu(&self, in_val: DataField) -> DataField {
        let name = in_val.get_name().to_string();
        let Some(text) = value_text(in_val.get_value()) else {
            if is_missing(in_val.get_value()) {
                return in_val;
            }
            // Composite values (object/array) cannot be scanned here. Failing
            // closed is safer than letting an unexamined structure through.
            return DataField::from_chars(name, self.placeholder().to_string());
        };
        if text.is_empty() {
            return in_val;
        }
        DataField::from_chars(name, mask_text(self, &text))
    }
}

/// Apply the scan contract for this operator.
fn mask_text(op: &MaskText, text: &str) -> String {
    match op.kind {
        MaskTextKind::Pii => mask_text_pii(op, text),
        MaskTextKind::HttpAuth => mask_text_http_auth(op, text),
        MaskTextKind::Ip => mask_text_ip(op, text),
        MaskTextKind::Mobile => mask_text_mobile(op, text),
        MaskTextKind::IdCard => mask_text_id_card(op, text),
        MaskTextKind::Mail => mask_text_mail(op, text),
        MaskTextKind::Domain => mask_text_domain(op, text),
    }
}

/// Replace every regex match for which `map` returns `Some`.
fn replace_matches<F>(text: &str, re: &Regex, mut map: F) -> String
where
    F: FnMut(&str) -> Option<String>,
{
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for matched in re.find_iter(text) {
        let Some(replacement) = map(matched.as_str()) else {
            continue;
        };
        out.push_str(&text[last..matched.start()]);
        out.push_str(&replacement);
        last = matched.end();
    }
    out.push_str(&text[last..]);
    out
}

fn mask_text_mobile(op: &MaskText, text: &str) -> String {
    let ch = op.mask_char();
    let placeholder = op.placeholder();
    let keep_last = op.keep_last.unwrap_or(4);
    replace_matches(text, mobile_re(), |matched| {
        Some(mask_keep(matched, 0, keep_last, ch, placeholder))
    })
}

fn mask_text_id_card(op: &MaskText, text: &str) -> String {
    let ch = op.mask_char();
    let placeholder = op.placeholder();
    let keep_first = op.keep_first.unwrap_or(6);
    let keep_last = op.keep_last.unwrap_or(4);
    replace_matches(text, id_card_re(), |matched| {
        is_valid_id_card(matched)
            .then(|| mask_keep(matched, keep_first, keep_last, ch, placeholder))
    })
}

fn mask_text_mail(op: &MaskText, text: &str) -> String {
    let ch = op.mask_char();
    let placeholder = op.placeholder();
    let keep_first = op.keep_first.unwrap_or(1);
    let keep_domain = op.keep_domain.unwrap_or(true);
    replace_matches(text, email_re(), |matched| {
        Some(mask_mail(matched, keep_first, keep_domain, ch, placeholder))
    })
}

fn mask_text_domain(op: &MaskText, text: &str) -> String {
    let ch = op.mask_char();
    let placeholder = op.placeholder();
    let keep_levels = op.keep_levels.unwrap_or(2);
    replace_matches(text, domain_re(), |matched| {
        Some(mask_domain(matched, keep_levels, ch, placeholder))
    })
}

fn mask_text_secret_kv(op: &MaskText, text: &str) -> String {
    let placeholder = op.placeholder();
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for caps in secret_kv_re().captures_iter(text) {
        let value = caps.name("value").expect("pattern has a value group");
        if value.as_str() == placeholder {
            continue;
        }
        out.push_str(&text[last..value.start()]);
        out.push_str(placeholder);
        last = value.end();
    }
    out.push_str(&text[last..]);
    out
}

fn mask_text_http_auth(op: &MaskText, text: &str) -> String {
    let text = replace_header_values(op, text, http_auth_re());
    replace_header_values(op, &text, auth_token_inline_re())
}

fn replace_header_values(op: &MaskText, text: &str, re: &Regex) -> String {
    let placeholder = op.placeholder();
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for caps in re.captures_iter(text) {
        let whole = caps.get(0).expect("whole match is always present");
        out.push_str(&text[last..whole.start()]);
        if let Some(head) = caps.name("head") {
            out.push_str(head.as_str());
        }
        if let Some(scheme) = caps.name("scheme") {
            out.push_str(scheme.as_str());
        }
        out.push_str(placeholder);
        last = whole.end();
    }
    out.push_str(&text[last..]);
    out
}

fn mask_text_ip(op: &MaskText, text: &str) -> String {
    let salt = ip_salt(op);
    let placeholder = op.placeholder();
    let after_v4 = replace_matches(text, ipv4_re(), |matched| {
        let ip = matched.parse::<Ipv4Addr>().ok()?;
        Some(match &salt {
            Some(salt) => map_ipv4(ip, salt, select_pool_v4(op.pool)).to_string(),
            None => placeholder.to_string(),
        })
    });
    mask_text_ipv6_tokens(op, &after_v4, salt.as_deref())
}

/// IPv6 literals are hard to pin down with a single regex (compressed forms,
/// scoped addresses), so scan maximal IP-character runs and let `IpAddr`
/// decide. Only successfully parsed IPv6 values are replaced.
fn mask_text_ipv6_tokens(op: &MaskText, text: &str, salt: Option<&[u8]>) -> String {
    let placeholder = op.placeholder();
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut emitted = 0;
    let mut i = 0;
    while i < bytes.len() {
        if !is_ip_token_byte(bytes[i]) {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && is_ip_token_byte(bytes[i]) {
            i += 1;
        }
        let token = &text[start..i];
        // A bare `A::B` / `d::` is a syntactically valid IPv6 literal but almost
        // always a symbol (`std::vector`). Require a decimal digit so scanning
        // does not rewrite such tokens.
        if !token.bytes().any(|b| b.is_ascii_digit()) {
            continue;
        }
        let (core, suffix) = split_trailing_dots(token);
        let Ok(IpAddr::V6(v6)) = core.parse::<IpAddr>() else {
            continue;
        };
        let mapped = match salt {
            Some(salt) => map_ipv6(v6, salt, select_pool_v6(op.pool)).to_string(),
            None => placeholder.to_string(),
        };
        out.push_str(&text[emitted..start]);
        out.push_str(&mapped);
        out.push_str(suffix);
        emitted = i;
    }
    out.push_str(&text[emitted..]);
    out
}

fn is_ip_token_byte(b: u8) -> bool {
    b.is_ascii_hexdigit() || b == b':' || b == b'.'
}

/// Sentence punctuation can stick to an address (`2001:db8::1.`). Keep the
/// suffix so the replacement does not swallow it.
fn split_trailing_dots(token: &str) -> (&str, &str) {
    let core = token.trim_end_matches('.');
    (core, &token[core.len()..])
}

fn ip_salt(op: &MaskText) -> Option<Vec<u8>> {
    match crate::privacy::get_secret(DEFAULT_SALT_NAME) {
        Some(salt) => Some(salt),
        None if op.allow_unsalted => {
            UNSALTED_WARNED.call_once(|| {
                log::warn!(
                    "privacy: mask_text(ip) is running without a salt (allow_unsalted:true)"
                );
            });
            Some(Vec::new())
        }
        None => {
            // Log once: per-record error logs would flood high-volume sinks.
            SALT_MISSING_LOGGED.call_once(|| {
                log::error!(
                    "privacy: mask_text(ip) requires secret '{DEFAULT_SALT_NAME}'; matched addresses are replaced with the placeholder"
                );
            });
            None
        }
    }
}

fn mask_text_pii(op: &MaskText, text: &str) -> String {
    // HTTP headers first: `authorization` needs its scheme preserved, and the
    // credential key/value pass must not re-handle them.
    let text = mask_text_http_auth(op, text);
    let text = mask_text_secret_kv(op, &text);
    let text = mask_text_mail(op, &text);
    let text = mask_text_mobile(op, &text);
    mask_text_id_card(op, &text)
}

/// GB 11643-1999 checksum. Used to avoid treating arbitrary 18-digit numbers
/// (timestamps, hashes) as ID cards during scanning.
fn is_valid_id_card(candidate: &str) -> bool {
    let bytes = candidate.as_bytes();
    if bytes.len() != 18 {
        return false;
    }
    const WEIGHTS: [u32; 17] = [7, 9, 10, 5, 8, 4, 2, 1, 6, 3, 7, 9, 10, 5, 8, 4, 2];
    const CHECK: [u8; 11] = *b"10X98765432";
    let mut sum = 0u32;
    for (i, weight) in WEIGHTS.iter().enumerate() {
        let digit = match bytes[i] {
            b'0'..=b'9' => (bytes[i] - b'0') as u32,
            _ => return false,
        };
        sum += digit * weight;
    }
    let expected = CHECK[(sum % 11) as usize];
    bytes[17].to_ascii_uppercase() == expected
}

#[cfg(test)]
mod tests {
    use super::*;
    use wp_model_core::model::types::value::ObjectValue;

    fn op(kind: MaskTextKind) -> MaskText {
        MaskText::new(kind)
    }

    #[test]
    fn scans_and_keeps_surrounding_text() {
        let text = "user=huangxingbo password=Dayu@1234 mobile=13800138000 \
                    email=zhangsan@example.com from 10.168.0.123";
        let out = mask_text(&op(MaskTextKind::Pii), text);
        assert_eq!(
            out,
            "user=huangxingbo password=[REDACTED] mobile=*******8000 \
             email=z***@example.com from 10.168.0.123"
        );
    }

    #[test]
    fn pii_leaves_text_without_matches_untouched() {
        let text = "2026-10-20 15:26:46 DEBUG MsgUserContext - login user=huangxingbo";
        assert_eq!(mask_text(&op(MaskTextKind::Pii), text), text);
    }

    #[test]
    fn masks_http_credential_headers() {
        let text = "POST /mgmt/tm/util/bash HTTP/1.1\n\
                    Authorization: Basic YWRtaW46aG9yaXpvbjM=\n\
                    X-F5-Auth-Token: a\n\
                    Content-Type: application/json\n\
                    Cookie: session=abc; theme=dark\n";
        assert_eq!(
            mask_text(&op(MaskTextKind::HttpAuth), text),
            "POST /mgmt/tm/util/bash HTTP/1.1\n\
             Authorization: Basic [REDACTED]\n\
             X-F5-Auth-Token: [REDACTED]\n\
             Content-Type: application/json\n\
             Cookie: [REDACTED]\n"
        );
    }

    #[test]
    fn scan_mobile_only_touches_the_number() {
        assert_eq!(
            mask_text(&op(MaskTextKind::Mobile), "call 13800138000 now"),
            "call *******8000 now"
        );
    }

    #[test]
    fn scan_mail_keeps_domain_and_other_text() {
        assert_eq!(
            mask_text(&op(MaskTextKind::Mail), "mail=alice@example.com."),
            "mail=a***@example.com."
        );
    }

    #[test]
    fn scan_does_not_treat_numbers_inside_id_cards_as_mobiles() {
        let text = "id 110101199003078515 ok";
        assert_eq!(
            mask_text(&op(MaskTextKind::IdCard), text),
            "id 110101********8515 ok"
        );
        assert_eq!(mask_text(&op(MaskTextKind::Mobile), text), text);
    }

    #[test]
    fn scan_ip_maps_each_address() {
        let text = "from 203.0.113.5 and 2001:4860:4860::8888";
        let out = mask_text(
            &{
                let mut o = op(MaskTextKind::Ip);
                o.allow_unsalted = true;
                o
            },
            text,
        );
        assert!(out.contains(" and 2001:db8:"), "got {out}");
        let first: u8 = out
            .trim_start_matches("from ")
            .split('.')
            .next()
            .and_then(|octet| octet.parse().ok())
            .expect("ipv4 first octet");
        assert!(first >= 240, "default IPv4 pool is 240.0.0.0/4, got {out}");
        assert!(!out.contains("203.0.113.5"));
        assert!(!out.contains("4860:4860"));
    }

    #[test]
    fn scan_ip_does_not_touch_symbol_like_tokens() {
        let mut o = op(MaskTextKind::Ip);
        o.allow_unsalted = true;
        assert_eq!(
            mask_text(&o, "std::vector and d:: inside code"),
            "std::vector and d:: inside code"
        );
    }

    #[test]
    fn scan_ip_never_leaks_the_original_address() {
        // Whether `pii_salt` happens to be set by a parallel test or not, the
        // original address must never survive the scan.
        let out = mask_text(&op(MaskTextKind::Ip), "from 203.0.113.5");
        assert!(!out.contains("203.0.113.5"), "got {out}");
    }

    #[test]
    fn invalid_id_card_is_left_alone() {
        assert!(!is_valid_id_card("110101199003078516"));
        assert!(is_valid_id_card("110101199003078515"));
    }

    #[test]
    fn secret_value_placeholder_is_idempotent() {
        assert_eq!(
            mask_text(&op(MaskTextKind::Pii), "password=[REDACTED]"),
            "password=[REDACTED]"
        );
    }

    #[test]
    fn secret_kv_matches_prefixed_keys() {
        assert_eq!(
            mask_text(
                &op(MaskTextKind::Pii),
                "db_password=Hunter2 client_secret=xyz"
            ),
            "db_password=[REDACTED] client_secret=[REDACTED]"
        );
        // A key embedded in a longer word is not a credential key.
        assert_eq!(
            mask_text(&op(MaskTextKind::Pii), "mypassword=Hunter2"),
            "mypassword=Hunter2"
        );
    }

    #[test]
    fn secret_kv_masks_quoted_values() {
        assert_eq!(
            mask_text(&op(MaskTextKind::Pii), "password=\"a b\" token='x y'"),
            "password=[REDACTED] token=[REDACTED]"
        );
    }

    #[test]
    fn http_auth_handles_single_line_payloads() {
        assert_eq!(
            mask_text(
                &op(MaskTextKind::HttpAuth),
                "request Authorization: Basic dXNlcjpwdw== status=200"
            ),
            "request Authorization: Basic [REDACTED] status=200"
        );
    }

    #[test]
    fn pii_handles_embedded_single_line_authorization() {
        // A header embedded in a longer single-line value is picked up by the
        // inline fallback, which stops at the next whitespace.
        assert_eq!(
            mask_text(
                &op(MaskTextKind::Pii),
                "req Authorization: Basic dXNlcjpwdw== status=200"
            ),
            "req Authorization: Basic [REDACTED] status=200"
        );
    }

    #[test]
    fn http_auth_covers_proxy_authorization_and_set_cookie() {
        let text = "Proxy-Authorization: Basic abc\nSet-Cookie: sid=1; HttpOnly\nOther: x";
        assert_eq!(
            mask_text(&op(MaskTextKind::HttpAuth), text),
            "Proxy-Authorization: Basic [REDACTED]\nSet-Cookie: [REDACTED]\nOther: x"
        );
    }

    #[test]
    fn http_auth_preserves_crlf() {
        let text = "GET / HTTP/1.1\r\nAuthorization: Bearer abc\r\nHost: h\r\n";
        assert_eq!(
            mask_text(&op(MaskTextKind::HttpAuth), text),
            "GET / HTTP/1.1\r\nAuthorization: Bearer [REDACTED]\r\nHost: h\r\n"
        );
    }

    #[test]
    fn ascii_boundaries_match_next_to_han_text() {
        assert_eq!(
            mask_text(&op(MaskTextKind::Mobile), "手机13800138000结束"),
            "手机*******8000结束"
        );
        assert_eq!(
            mask_text(&op(MaskTextKind::IdCard), "证件110101199003078515号"),
            "证件110101********8515号"
        );
    }

    #[test]
    fn pii_is_idempotent() {
        let once = mask_text(&op(MaskTextKind::Pii), PII_LOG);
        let twice = mask_text(&op(MaskTextKind::Pii), &once);
        assert_eq!(once, twice);
    }

    #[test]
    fn http_auth_is_idempotent() {
        let text = "Authorization: Basic dXNlcjpwdw==\nX-Auth-Token: tok";
        let once = mask_text(&op(MaskTextKind::HttpAuth), text);
        let twice = mask_text(&op(MaskTextKind::HttpAuth), &once);
        assert_eq!(once, twice);
    }

    #[test]
    fn chained_mail_and_mobile_do_not_interfere() {
        let text = "mail=alice@example.com mobile=13800138000";
        let mail = mask_text(&op(MaskTextKind::Mail), text);
        let both = mask_text(&op(MaskTextKind::Mobile), &mail);
        assert_eq!(both, "mail=a***@example.com mobile=*******8000");
    }

    #[test]
    fn domain_scan_leaves_the_rest_alone() {
        assert_eq!(
            mask_text(&op(MaskTextKind::Domain), "host api.example.com port 443"),
            "host ***.example.com port 443"
        );
    }

    #[test]
    fn secret_kv_covers_all_key_variants() {
        let text = "pass=a pwd=b api_key=c access_key=d secret_key=e auth_token=f";
        assert_eq!(
            mask_text(&op(MaskTextKind::Pii), text),
            "pass=[REDACTED] pwd=[REDACTED] api_key=[REDACTED] access_key=[REDACTED] secret_key=[REDACTED] auth_token=[REDACTED]"
        );
    }

    #[test]
    fn secret_kv_allows_spaces_around_the_separator() {
        assert_eq!(
            mask_text(&op(MaskTextKind::Pii), "password = hunter2 ;"),
            "password = [REDACTED] ;"
        );
    }

    #[test]
    fn composite_values_fail_closed() {
        let field = DataField::from_obj("payload", ObjectValue::default());
        let out = op(MaskTextKind::Pii).value_cacu(field);
        assert_eq!(format!("{}", out.get_value()), "[REDACTED]");
    }

    const PII_LOG: &str = "user=huangxingbo password=Dayu@1234 mobile=13800138000 \
                           email=zhangsan@example.com";
}
