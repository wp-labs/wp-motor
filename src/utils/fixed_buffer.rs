use std::cmp::Ordering;

pub const FIXED_CHARS_SIZE: usize = 256;

pub type FixedBuffer = [u8; FIXED_CHARS_SIZE];

pub fn cmp_fcs(a: &FixedBuffer, found: &FixedBuffer) -> Ordering {
    let mut i = 0;
    loop {
        if i >= FIXED_CHARS_SIZE {
            return Ordering::Equal;
        }
        if a[i] == 0 && found[i] == 0 {
            return Ordering::Equal;
        }
        if a[i] < found[i] {
            return Ordering::Less;
        }
        if a[i] > found[i] {
            return Ordering::Greater;
        }
        i += 1;
    }
}

pub fn fixed_to_string(value: &FixedBuffer) -> String {
    let mut name_str = String::new();
    unsafe {
        if let Some(first) = value.split(|c| *c == 0).next() {
            name_str.push_str(std::str::from_utf8_unchecked(first));
        }
    }
    name_str
}

pub fn fixed_from(s: &str) -> FixedBuffer {
    let mut item = [0u8; FIXED_CHARS_SIZE];
    item[..s.len()].copy_from_slice(s.as_bytes());
    item
}

#[cfg(test)]
mod tests {
    use crate::utils::fixed_buffer::{cmp_fcs, fixed_from};

    #[test]
    fn test_sort_search() {
        let mut items = [
            fixed_from("no_2"),
            fixed_from("no_1"),
            fixed_from("no_3"),
            fixed_from("no_5"),
            fixed_from("no_4"),
        ];
        items.sort();
        assert_eq!(items[0], fixed_from("no_1"));
        assert_eq!(items[1], fixed_from("no_2"));
        assert_eq!(items[4], fixed_from("no_5"));

        let x = items
            .binary_search_by(|x| cmp_fcs(x, &fixed_from("no_3")))
            .ok()
            .map(|i| &items[i]);
        assert_eq!(x, Some(&fixed_from("no_3")));
    }
}
