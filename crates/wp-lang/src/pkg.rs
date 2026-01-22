use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub type PkgID = u64;

pub const DEFAULT_KEY: &str = "_";

static ORDER_COUNTER: AtomicU64 = AtomicU64::new(0);
static PKG_ID_BASE: OnceLock<u64> = OnceLock::new();

fn pkg_id_base() -> u64 {
    *PKG_ID_BASE.get_or_init(|| {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        (secs & 0xFFFF_FFFF) << 24
    })
}

pub fn gen_pkg_id() -> u64 {
    pkg_id_base() + ORDER_COUNTER.fetch_add(1, Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use crate::pkg::gen_pkg_id;

    #[test]
    fn test_gen_id() {
        let a = gen_pkg_id();
        let b = gen_pkg_id();
        assert_ne!(a, b);
        println!("{},{}", a, b);
    }
}
