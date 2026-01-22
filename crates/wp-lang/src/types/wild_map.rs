use std::slice::{Iter, IterMut};
use wildmatch::WildMatch;

#[derive(Default, PartialEq, Clone, Debug)]
pub struct WildMap<T> {
    exact_items: Vec<(String, T)>,
    wild_items: Vec<(String, WildMatch, T)>,
}

impl<T> WildMap<T>
where
    T: Clone,
{
    pub fn new() -> Self {
        Self {
            exact_items: Vec::new(),
            wild_items: Vec::new(),
        }
    }
    pub fn insert(&mut self, key: String, value: T) {
        if key.contains("*") {
            self.wild_items
                .push((key.clone(), WildMatch::new(key.as_str()), value));
        } else {
            self.exact_items.push((key.clone(), value));
        }
    }
    fn get_impl(&self, key: &str) -> Option<(&str, &T)> {
        for (store_key, v) in &self.exact_items {
            if store_key == key {
                return Some((store_key.as_str(), v));
            }
        }
        for (store_key, wild, v) in &self.wild_items {
            if wild.matches(key) {
                return Some((store_key.as_str(), v));
            }
        }
        None
    }
    pub fn get(&self, key: &str) -> Option<&T> {
        self.get_impl(key).map(|x| x.1)
    }
    pub fn get_more(&self, key: &str) -> Option<(&str, &T)> {
        self.get_impl(key)
    }
    pub fn len(&self) -> usize {
        self.exact_items.len() + self.wild_items.len()
    }
    pub fn is_empty(&self) -> bool {
        self.exact_items.is_empty() && self.wild_items.is_empty()
    }

    pub fn exact_iter(&self) -> Iter<'_, (String, T)> {
        self.exact_items.iter()
    }
    pub fn wild_iter(&self) -> Iter<'_, (String, WildMatch, T)> {
        self.wild_items.iter()
    }
    pub fn exact_iter_mut(&mut self) -> IterMut<'_, (String, T)> {
        self.exact_items.iter_mut()
    }
    pub fn wild_iter_mut(&mut self) -> IterMut<'_, (String, WildMatch, T)> {
        self.wild_items.iter_mut()
    }
}
