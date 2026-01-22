use std::cell::RefCell;

use wp_model_core::model::DataField;

thread_local! {
    static FIELD_BUF: RefCell<Vec<DataField>> = RefCell::new(Vec::with_capacity(256));
}

pub fn with_buffer<F, R>(f: F) -> R
where
    F: FnOnce(&mut Vec<DataField>) -> R,
{
    FIELD_BUF.with(|buf| {
        let mut vec = buf.borrow_mut();
        vec.clear();
        f(&mut vec)
    })
}
