//! Privacy secret registry.
//!
//! Privacy operators use fixed names (`pii_salt` for `mask`/`hash`, `pii_key`
//! for `tokenize`); models never carry key names. The key material is loaded
//! once during startup from the `SEC_XXX` mechanism (`.warp_parse/sec_key.toml`
//! via `orion-sec`) and registered here.
//!
//! Runtime lookup must never fall back to an empty key silently: operators use
//! [`get_secret`] and apply their own fail-closed behavior when the key is
//! missing.

use base64::Engine;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

static SECRETS: OnceLock<RwLock<HashMap<String, Vec<u8>>>> = OnceLock::new();

fn store() -> &'static RwLock<HashMap<String, Vec<u8>>> {
    SECRETS.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Normalize a logical secret name (`pii_salt`) or a `SEC_` variable name
/// (`SEC_PII_SALT`) to the same registry key (`PII_SALT`).
pub fn normalize_secret_name(name: &str) -> String {
    let upper = name.trim().to_ascii_uppercase();
    upper.strip_prefix("SEC_").unwrap_or(&upper).to_string()
}

/// Register raw secret material under a logical name.
///
/// Empty material is treated as missing: the entry is removed instead of being
/// stored, so operators fail closed rather than hashing with an empty key.
pub fn set_secret<N, V>(name: N, value: V)
where
    N: AsRef<str>,
    V: Into<Vec<u8>>,
{
    if let Ok(mut map) = store().write() {
        upsert(&mut map, name.as_ref(), value.into());
    }
}

fn upsert(map: &mut HashMap<String, Vec<u8>>, name: &str, value: Vec<u8>) {
    let name = normalize_secret_name(name);
    if value.is_empty() {
        map.remove(&name);
    } else {
        map.insert(name, value);
    }
}

/// Decode a text secret when it is a 32-byte hex/base64 value; otherwise use
/// the raw UTF-8 bytes. This matches the documented `sec_key.toml` format.
fn decode_key_material(value: &str) -> Vec<u8> {
    let value = value.trim();
    if value.len() == 64
        && value.bytes().all(|b| b.is_ascii_hexdigit())
        && let Ok(bytes) = hex::decode(value)
    {
        return bytes;
    }
    for engine in [&STANDARD, &STANDARD_NO_PAD, &URL_SAFE, &URL_SAFE_NO_PAD] {
        if let Ok(bytes) = engine.decode(value)
            && bytes.len() == 32
        {
            return bytes;
        }
    }
    value.as_bytes().to_vec()
}

/// Register a text secret, trimming surrounding whitespace and decoding a
/// 32-byte hex/base64 value when present.
pub fn set_secret_str<N, V>(name: N, value: V)
where
    N: AsRef<str>,
    V: AsRef<str>,
{
    set_secret(name, decode_key_material(value.as_ref()));
}

/// Look up secret material by logical name or `SEC_` variable name.
pub fn get_secret(name: &str) -> Option<Vec<u8>> {
    store()
        .read()
        .ok()
        .and_then(|map| map.get(&normalize_secret_name(name)).cloned())
}

/// Remove all registered secrets. Primarily useful in tests.
pub fn clear_secrets() {
    if let Ok(mut map) = store().write() {
        map.clear();
    }
}

/// Register secrets from `(name, value)` pairs.
pub fn set_secrets_from_pairs<I, K, V>(pairs: I)
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<str>,
    V: AsRef<str>,
{
    if let Ok(mut map) = store().write() {
        for (name, value) in pairs {
            upsert(&mut map, name.as_ref(), decode_key_material(value.as_ref()));
        }
    }
}

/// Register all values from an `orion-variate` environment dictionary.
///
/// This is the integration point for the `SEC_XXX` mechanism: the engine calls
/// it with the dictionary returned by `warp_parse::load_sec_dict()`.
pub fn set_secrets_from_env_dict(dict: &orion_variate::vars::EnvDict) {
    if let Ok(mut map) = store().write() {
        for (name, value) in dict.iter() {
            upsert(
                &mut map,
                name.as_str(),
                decode_key_material(&value.to_string()),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_material_is_decoded_when_encoded() {
        let hex_key = "00".repeat(32);
        set_secret_str("unit_hex_key", &hex_key);
        assert_eq!(get_secret("unit_hex_key"), Some(vec![0u8; 32]));

        let b64 = STANDARD.encode([7u8; 32]);
        set_secret_str("unit_b64_key", &b64);
        assert_eq!(get_secret("unit_b64_key"), Some(vec![7u8; 32]));

        set_secret_str("unit_raw_key", "plain-secret");
        assert_eq!(get_secret("unit_raw_key"), Some(b"plain-secret".to_vec()));
    }

    #[test]
    fn env_dict_registration_works() {
        use orion_variate::vars::{EnvDict, ValueType};
        let mut dict = EnvDict::default();
        dict.insert("SEC_UNIT_ENV_SALT", ValueType::from("env-secret"));
        dict.insert("SEC_UNIT_ENV_HEX", ValueType::from("00".repeat(32)));
        dict.insert("SEC_UNIT_ENV_EMPTY", ValueType::from(""));
        dict.insert("SEC_UNIT_ENV_BLANK", ValueType::from("   "));
        set_secrets_from_env_dict(&dict);
        assert_eq!(get_secret("unit_env_salt"), Some(b"env-secret".to_vec()));
        assert_eq!(get_secret("unit_env_hex"), Some(vec![0u8; 32]));
        assert_eq!(get_secret("unit_env_empty"), None);
        assert_eq!(get_secret("unit_env_blank"), None);
    }

    #[test]
    fn empty_secret_material_is_treated_as_missing() {
        set_secret_str("unit_empty_secret", "");
        assert_eq!(get_secret("unit_empty_secret"), None);

        set_secret_str("unit_blank_secret", "   ");
        assert_eq!(get_secret("unit_blank_secret"), None);

        set_secret("unit_empty_bytes", Vec::<u8>::new());
        assert_eq!(get_secret("unit_empty_bytes"), None);

        // Re-registering an empty value clears the previous material.
        set_secret_str("unit_reset_secret", "abc");
        assert_eq!(get_secret("unit_reset_secret"), Some(b"abc".to_vec()));
        set_secret_str("unit_reset_secret", "");
        assert_eq!(get_secret("unit_reset_secret"), None);
    }

    #[test]
    fn secret_names_are_normalized() {
        set_secret_str("unit_test_secret", "abc");
        assert_eq!(get_secret("unit_test_secret"), Some(b"abc".to_vec()));
        assert_eq!(get_secret("UNIT_TEST_SECRET"), Some(b"abc".to_vec()));
        assert_eq!(get_secret("SEC_UNIT_TEST_SECRET"), Some(b"abc".to_vec()));
    }
}
