use std::{collections::HashMap, net::IpAddr, num::NonZeroUsize};

use derive_builder::Builder;

use wp_model_core::model::{DataField, FValueStr, Value};

#[derive(Builder, Debug, Clone)]
pub struct FieldQueryCache {
    str_idx: HashMap<FValueStr, usize>,
    i64_idx: HashMap<i64, usize>,
    ip_idx: HashMap<IpAddr, usize>,
    cache_data: lru::LruCache<EnumSizeIndex, Vec<DataField>>,
    idx_num: usize,
}
impl Default for FieldQueryCache {
    fn default() -> Self {
        Self {
            str_idx: HashMap::new(),
            i64_idx: HashMap::new(),
            ip_idx: HashMap::new(),
            idx_num: 0,
            cache_data: lru::LruCache::new(NonZeroUsize::new(100).unwrap()),
        }
    }
}
impl FieldQueryCache {
    pub fn with_capacity(size: usize) -> Self {
        let size = size.max(1); // LruCache requires non-zero capacity
        Self {
            str_idx: HashMap::new(),
            i64_idx: HashMap::new(),
            ip_idx: HashMap::new(),
            idx_num: 0,
            cache_data: lru::LruCache::new(NonZeroUsize::new(size).unwrap()),
        }
    }
}
#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub enum EnumSizeIndex {
    Idx1(usize),
    Idx2(usize, usize),
    Idx3(usize, usize, usize),
    Idx4(usize, usize, usize, usize),
    Idx5(usize, usize, usize, usize, usize),
    Idx6(usize, usize, usize, usize, usize, usize),
}
pub trait CacheAble<P, T, const N: usize> {
    fn save(&mut self, params: &[P; N], result: T);
    fn fetch(&self, params: &[P; N]) -> Option<&T>;
}

impl CacheAble<DataField, Vec<DataField>, 1> for FieldQueryCache {
    fn save(&mut self, params: &[DataField; 1], result: Vec<DataField>) {
        if let Some(i0) = self.try_up_idx(&params[0]) {
            let idxs = EnumSizeIndex::Idx1(i0);
            self.cache_data.put(idxs, result);
        }
    }
    fn fetch(&self, params: &[DataField; 1]) -> Option<&Vec<DataField>> {
        if let Some(idx1) = self.get_idx(&params[0]) {
            let idxs = EnumSizeIndex::Idx1(idx1);
            return self.cache_data.peek(&idxs);
        }
        None
    }
}

impl CacheAble<DataField, Vec<DataField>, 2> for FieldQueryCache {
    fn save(&mut self, params: &[DataField; 2], result: Vec<DataField>) {
        if let (Some(i0), Some(i1)) = (self.try_up_idx(&params[0]), self.try_up_idx(&params[1])) {
            let idxs = EnumSizeIndex::Idx2(i0, i1);
            self.cache_data.put(idxs, result);
        }
    }
    fn fetch(&self, params: &[DataField; 2]) -> Option<&Vec<DataField>> {
        if let (Some(idx1), Some(idx2)) = (self.get_idx(&params[0]), self.get_idx(&params[1])) {
            let idxs = EnumSizeIndex::Idx2(idx1, idx2);
            return self.cache_data.peek(&idxs);
        }
        None
    }
}

impl CacheAble<DataField, Vec<DataField>, 3> for FieldQueryCache {
    fn save(&mut self, params: &[DataField; 3], result: Vec<DataField>) {
        if let (Some(i0), Some(i1), Some(i2)) = (
            self.try_up_idx(&params[0]),
            self.try_up_idx(&params[1]),
            self.try_up_idx(&params[2]),
        ) {
            let idxs = EnumSizeIndex::Idx3(i0, i1, i2);
            self.cache_data.put(idxs, result);
        }
    }
    fn fetch(&self, params: &[DataField; 3]) -> Option<&Vec<DataField>> {
        if let (Some(idx1), Some(idx2), Some(idx3)) = (
            self.get_idx(&params[0]),
            self.get_idx(&params[1]),
            self.get_idx(&params[2]),
        ) {
            let idxs = EnumSizeIndex::Idx3(idx1, idx2, idx3);
            return self.cache_data.peek(&idxs);
        }
        None
    }
}

impl CacheAble<DataField, Vec<DataField>, 4> for FieldQueryCache {
    fn save(&mut self, params: &[DataField; 4], result: Vec<DataField>) {
        if let (Some(i0), Some(i1), Some(i2), Some(i3)) = (
            self.try_up_idx(&params[0]),
            self.try_up_idx(&params[1]),
            self.try_up_idx(&params[2]),
            self.try_up_idx(&params[3]),
        ) {
            let idxs = EnumSizeIndex::Idx4(i0, i1, i2, i3);
            self.cache_data.put(idxs, result);
        }
    }
    fn fetch(&self, params: &[DataField; 4]) -> Option<&Vec<DataField>> {
        if let (Some(idx1), Some(idx2), Some(idx3), Some(idx4)) = (
            self.get_idx(&params[0]),
            self.get_idx(&params[1]),
            self.get_idx(&params[2]),
            self.get_idx(&params[3]),
        ) {
            let idxs = EnumSizeIndex::Idx4(idx1, idx2, idx3, idx4);
            return self.cache_data.peek(&idxs);
        }
        None
    }
}

impl CacheAble<DataField, Vec<DataField>, 5> for FieldQueryCache {
    fn save(&mut self, params: &[DataField; 5], result: Vec<DataField>) {
        if let (Some(i0), Some(i1), Some(i2), Some(i3), Some(i4)) = (
            self.try_up_idx(&params[0]),
            self.try_up_idx(&params[1]),
            self.try_up_idx(&params[2]),
            self.try_up_idx(&params[3]),
            self.try_up_idx(&params[4]),
        ) {
            let idxs = EnumSizeIndex::Idx5(i0, i1, i2, i3, i4);
            self.cache_data.put(idxs, result);
        }
    }
    fn fetch(&self, params: &[DataField; 5]) -> Option<&Vec<DataField>> {
        if let (Some(idx1), Some(idx2), Some(idx3), Some(idx4), Some(idx5)) = (
            self.get_idx(&params[0]),
            self.get_idx(&params[1]),
            self.get_idx(&params[2]),
            self.get_idx(&params[3]),
            self.get_idx(&params[4]),
        ) {
            let idxs = EnumSizeIndex::Idx5(idx1, idx2, idx3, idx4, idx5);
            return self.cache_data.peek(&idxs);
        }
        None
    }
}

impl CacheAble<DataField, Vec<DataField>, 6> for FieldQueryCache {
    fn save(&mut self, params: &[DataField; 6], result: Vec<DataField>) {
        if let (Some(i0), Some(i1), Some(i2), Some(i3), Some(i4), Some(i5)) = (
            self.try_up_idx(&params[0]),
            self.try_up_idx(&params[1]),
            self.try_up_idx(&params[2]),
            self.try_up_idx(&params[3]),
            self.try_up_idx(&params[4]),
            self.try_up_idx(&params[5]),
        ) {
            let idxs = EnumSizeIndex::Idx6(i0, i1, i2, i3, i4, i5);
            self.cache_data.put(idxs, result);
        }
    }
    fn fetch(&self, params: &[DataField; 6]) -> Option<&Vec<DataField>> {
        if let (Some(idx1), Some(idx2), Some(idx3), Some(idx4), Some(idx5), Some(idx6)) = (
            self.get_idx(&params[0]),
            self.get_idx(&params[1]),
            self.get_idx(&params[2]),
            self.get_idx(&params[3]),
            self.get_idx(&params[4]),
            self.get_idx(&params[5]),
        ) {
            let idxs = EnumSizeIndex::Idx6(idx1, idx2, idx3, idx4, idx5, idx6);
            return self.cache_data.peek(&idxs);
        }
        None
    }
}

impl FieldQueryCache {
    fn get_idx(&self, param: &DataField) -> Option<usize> {
        match param.get_value() {
            Value::Chars(v) => self.str_idx.get(v).copied(),
            Value::Digit(v) => self.i64_idx.get(v).copied(),
            Value::IpAddr(v) => self.ip_idx.get(v).copied(),
            _ => None,
        }
    }

    // Try to assign or retrieve an index for supported value types; returns None for unsupported.
    fn try_up_idx(&mut self, param: &DataField) -> Option<usize> {
        match param.get_value() {
            Value::Chars(v) => {
                if let Some(idx_val) = self.str_idx.get(v) {
                    Some(*idx_val)
                } else {
                    self.idx_num += 1;
                    self.str_idx.insert(v.clone(), self.idx_num);
                    Some(self.idx_num)
                }
            }
            Value::Digit(v) => {
                if let Some(idx_val) = self.i64_idx.get(v) {
                    Some(*idx_val)
                } else {
                    self.idx_num += 1;
                    self.i64_idx.insert(*v, self.idx_num);
                    Some(self.idx_num)
                }
            }
            Value::IpAddr(v) => {
                if let Some(idx_val) = self.ip_idx.get(v) {
                    Some(*idx_val)
                } else {
                    self.idx_num += 1;
                    self.ip_idx.insert(*v, self.idx_num);
                    Some(self.idx_num)
                }
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod test {

    use std::collections::HashMap;

    use wp_model_core::model::DataField;

    use crate::cache::{CacheAble, EnumSizeIndex, FieldQueryCache};

    #[test]
    fn test_idx() {
        let i1 = EnumSizeIndex::Idx2(0, 1);
        let i2 = EnumSizeIndex::Idx2(0, 1);
        assert_eq!(i1, i2);

        let i3 = EnumSizeIndex::Idx2(0, 2);
        let mut map: HashMap<EnumSizeIndex, usize> = HashMap::new();
        map.insert(i1.clone(), 1);
        map.insert(i3.clone(), 2);
        assert_eq!(map.get(&i1), Some(1).as_ref())
    }
    #[test]
    fn test_cache() {
        let data = [
            DataField::from_chars("A", "chars-1"),
            DataField::from_chars("B", "chars-2"),
        ];
        let out = vec![
            DataField::from_chars("A1", "chars-11"),
            DataField::from_chars("B1", "chars-21"),
        ];

        let mut cache = FieldQueryCache::with_capacity(3);
        let cache_ret = cache.fetch(&data);
        assert!(cache_ret.is_none());
        cache.save(&data, out.clone());
        let cache_ret = cache.fetch(&data);
        assert_eq!(cache_ret, Some(&out));

        let data1 = [DataField::from_chars("A", "chars-1")];

        let cache_ret = cache.fetch(&data1);
        assert!(cache_ret.is_none());
        cache.save(&data1, out.clone());
        let cache_ret = cache.fetch(&data1);
        assert_eq!(cache_ret, Some(&out));

        let data2 = [DataField::from_chars("B", "chars-2")];

        let cache_ret = cache.fetch(&data2);
        assert!(cache_ret.is_none());
        cache.save(&data2, out.clone());
        let cache_ret = cache.fetch(&data2);
        assert_eq!(cache_ret, Some(&out));
    }
}
