use std::net::IpAddr;

use arcstr::ArcStr;
use chrono::NaiveDateTime;
use ipnet::IpNet;
use smol_str::SmolStr;

use crate::WildcardMatcher;
#[cfg(feature = "we_precompile")]
use std::{cell::RefCell, collections::HashMap, env, sync::Arc};
#[cfg(feature = "we_precompile")]
struct Lru {
    map: HashMap<String, Arc<wildmatch::WildMatch>>, // pattern -> compiled
    order: Vec<String>,                              // recency: oldest at 0
    cap: usize,
}
#[cfg(feature = "we_precompile")]
impl Lru {
    fn new(cap: usize) -> Self {
        Self {
            map: HashMap::new(),
            order: Vec::new(),
            cap,
        }
    }
    fn touch(&mut self, key: &str) {
        if let Some(pos) = self.order.iter().position(|k| k == key) {
            let k = self.order.remove(pos);
            self.order.push(k);
        }
    }
    fn get_or_put(&mut self, pat: &str) -> Arc<wildmatch::WildMatch> {
        if self.map.contains_key(pat) {
            self.touch(pat);
            return self.map.get(pat).unwrap().clone();
        }
        // compile new
        let compiled = Arc::new(wildmatch::WildMatch::new(pat));
        if self.order.len() >= self.cap {
            if let Some(old) = self.order.first() {
                self.map.remove(old);
            }
            if !self.order.is_empty() {
                self.order.remove(0);
            }
        }
        self.map.insert(pat.to_string(), compiled.clone());
        self.order.push(pat.to_string());
        compiled
    }
}
#[cfg(feature = "we_precompile")]
fn lru_capacity_from_env() -> usize {
    const DEF: usize = 256;
    match env::var("ORION_EXP_WE_LRU_CAP")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
    {
        Some(v) if v >= 8 => v.min(4096),
        _ => DEF,
    }
}
#[cfg(feature = "we_precompile")]
thread_local! {
    static WE_CACHE: RefCell<Option<Lru>> = const { RefCell::new(None) };
}

impl WildcardMatcher for String {
    fn matches(&self, other: &Self) -> bool {
        #[cfg(feature = "we_precompile")]
        {
            let pat = self.as_str();
            let wm = WE_CACHE.with(|cell| {
                let mut slot = cell.borrow_mut();
                if slot.is_none() {
                    *slot = Some(Lru::new(lru_capacity_from_env()));
                }
                let lru = slot.as_mut().unwrap();
                lru.get_or_put(pat)
            });
            wm.matches(other.as_str())
        }
        #[cfg(not(feature = "we_precompile"))]
        {
            wildmatch::WildMatch::new(self.as_str()).matches(other.as_str())
        }
    }
}

impl WildcardMatcher for ArcStr {
    fn matches(&self, other: &Self) -> bool {
        #[cfg(feature = "we_precompile")]
        {
            let pat = self.as_str();
            let wm = WE_CACHE.with(|cell| {
                let mut slot = cell.borrow_mut();
                if slot.is_none() {
                    *slot = Some(Lru::new(lru_capacity_from_env()));
                }
                let lru = slot.as_mut().unwrap();
                lru.get_or_put(pat)
            });
            wm.matches(other.as_str())
        }
        #[cfg(not(feature = "we_precompile"))]
        {
            wildmatch::WildMatch::new(self.as_str()).matches(other.as_str())
        }
    }
}

impl WildcardMatcher for SmolStr {
    fn matches(&self, other: &Self) -> bool {
        #[cfg(feature = "we_precompile")]
        {
            let pat = self.as_str();
            let wm = WE_CACHE.with(|cell| {
                let mut slot = cell.borrow_mut();
                if slot.is_none() {
                    *slot = Some(Lru::new(lru_capacity_from_env()));
                }
                let lru = slot.as_mut().unwrap();
                lru.get_or_put(pat)
            });
            wm.matches(other.as_str())
        }
        #[cfg(not(feature = "we_precompile"))]
        {
            wildmatch::WildMatch::new(self.as_str()).matches(other.as_str())
        }
    }
}

impl WildcardMatcher for i64 {
    fn matches(&self, other: &Self) -> bool {
        *self == *other
    }
}
impl WildcardMatcher for bool {
    fn matches(&self, other: &Self) -> bool {
        *self == *other
    }
}

impl WildcardMatcher for u32 {
    fn matches(&self, other: &Self) -> bool {
        *self == *other
    }
}
impl WildcardMatcher for u128 {
    fn matches(&self, other: &Self) -> bool {
        *self == *other
    }
}
impl WildcardMatcher for u64 {
    fn matches(&self, other: &Self) -> bool {
        *self == *other
    }
}

impl WildcardMatcher for f64 {
    fn matches(&self, other: &Self) -> bool {
        if *self > *other {
            (*self - *other) < 0.0001
        } else {
            (*other - *self) < 0.0001
        }
    }
}

impl WildcardMatcher for IpAddr {
    fn matches(&self, other: &Self) -> bool {
        *self == *other
    }
}

impl WildcardMatcher for IpNet {
    fn matches(&self, other: &Self) -> bool {
        *self == *other
    }
}

impl WildcardMatcher for NaiveDateTime {
    fn matches(&self, other: &Self) -> bool {
        *self == *other
    }
}
