use std::collections::VecDeque;

pub struct RollingQueue<T> {
    item_queue: VecDeque<T>,
    current_item: Option<T>,
    count: usize,
}

impl<T> Default for RollingQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> RollingQueue<T> {
    pub fn new() -> Self {
        Self {
            current_item: None,
            item_queue: VecDeque::new(),
            count: 0,
        }
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.item_queue.len() + if self.current_item.is_some() { 1 } else { 0 }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.current_item.is_none() && self.item_queue.is_empty()
    }
    pub fn append(&mut self, one: T) {
        if self.current_item.is_some() {
            self.item_queue.push_back(one);
        } else {
            self.current_item = Some(one);
        }
    }

    #[inline]
    pub fn cur(&self) -> &Option<T> {
        &self.current_item
    }

    pub fn roll(&mut self) {
        if self.item_queue.is_empty() {
            return;
        }
        let top = self.item_queue.pop_front();
        let old = self.current_item.take();
        self.current_item = top;
        self.item_queue.push_back(old.expect("cur_s is empty"));
        debug_assert!(self.current_item.is_some());
    }
    //#[allow(clippy::manual_is_multiple_of)]
    pub fn auto_roll(&mut self, roll_times: usize) {
        self.count += 1;
        if self.count.is_multiple_of(roll_times) {
            self.roll();
            if self.count > 100000000 {
                self.count = 0;
            }
        }
    }
}
