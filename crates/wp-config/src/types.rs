use orion_variate::EnvDict;

pub trait SafeDefault<T> {
    fn safe_default() -> T;
}

impl SafeDefault<EnvDict> for EnvDict {
    fn safe_default() -> Self {
        EnvDict::default()
    }
}
