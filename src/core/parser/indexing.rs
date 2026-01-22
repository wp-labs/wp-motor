use std::collections::HashMap;

#[derive(Default)]
pub struct ResourceIndexer {
    unused_idx: usize,
    idx_map: HashMap<String, usize>,
}

impl ResourceIndexer {
    pub fn checkin(&mut self, key: &str) -> usize {
        if let Some(idx) = self.idx_map.get(key) {
            *idx
        } else {
            self.idx_map.insert(key.to_string(), self.unused_idx);
            self.unused_idx += 1;
            self.unused_idx - 1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ResourceIndexer;

    #[test]
    fn test_idx() {
        let mut idx = ResourceIndexer::default();
        assert_eq!(idx.checkin("a"), 0);
        assert_eq!(idx.checkin("a"), 0);
        assert_eq!(idx.checkin("b"), 1);
        assert_eq!(idx.checkin("b"), 1);
    }
}
