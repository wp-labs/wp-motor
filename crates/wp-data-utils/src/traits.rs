pub trait AppendAble1<T1> {
    fn append(&mut self, first: T1);
}
pub trait AppendAble2<T1, T2> {
    fn append(&mut self, first: T1, second: T2);
}
pub use wp_model_core::traits::AsValueRef;
