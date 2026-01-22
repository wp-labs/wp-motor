pub trait Appendable<T> {
    fn append(&mut self, now: T);
}
pub trait Appendable2<T1, T2> {
    fn append(&mut self, first: T1, second: T2);
}
pub trait Appendable3<T1, T2, T3> {
    fn append(&mut self, first: T1, second: T2, third: T3);
}

pub trait Insertable<K, V> {
    fn insert(&mut self, key: K, value: V);
}
