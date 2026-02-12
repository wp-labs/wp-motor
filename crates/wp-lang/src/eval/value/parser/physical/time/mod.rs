// time module split: CLF (Common Log Format), RFC (3339/2822/ISO), and TIMESTAMP
// This module orchestrates submodules and exposes the public surface.

mod clf;
mod common; // shared helpers (parse_fixed, fast_apache_dt)
mod rfc; // RFC3339 / RFC2822 / flexible time parser
mod timestamp; // unix timestamp family (s/ms/us) // Common Log Format fast path

pub use clf::TimeCLF;
pub use rfc::{TimeISOP, TimeP, TimeRFC2822, TimeRFC3339, parse_time};
pub use timestamp::TimeStampPSR;
use wp_model_core::model::DataField;

// Shared generator used by all time parsers when synthesizing test/bench data
pub fn gen_time(
    gnc: &mut crate::generator::GenChannel,
    f_conf: &crate::ast::WplField,
    g_conf: Option<&crate::generator::FieldGenConf>,
) -> crate::types::AnyResult<DataField> {
    use chrono::TimeZone; // bring with_ymd_and_hms into scope
    use rand::RngExt as _;
    let y = gnc.rng.random_range(2020..2023);
    let mon = gnc.rng.random_range(1..12);
    let d = gnc.rng.random_range(1..29);
    let h = gnc.rng.random_range(0..23);
    let min = gnc.rng.random_range(0..59);
    let s = gnc.rng.random_range(0..59);

    let time: chrono::DateTime<chrono::FixedOffset> = chrono::FixedOffset::east_opt(0)
        .unwrap()
        .with_ymd_and_hms(y, mon, d, h, min, s)
        .unwrap();
    if let Some(conf) = g_conf
        && let Some(fmt) = &conf.gen_fmt
    {
        let mut my_vars: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        my_vars.insert(
            "val".to_string(),
            time.format("%Y-%m-%d %H:%M:%S").to_string(),
        );
        match strfmt::strfmt(fmt, &my_vars) {
            Ok(dat) => {
                return Ok(DataField::from_chars(f_conf.safe_name().to_string(), dat));
            }
            Err(e) => {
                log::error!("gen fmt error: {}", e);
            }
        }
    }
    Ok(DataField::from_time(
        f_conf.safe_name().to_string(),
        time.naive_local(),
    ))
}
