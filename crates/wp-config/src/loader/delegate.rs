use crate::structure::ConfStdOperation;
use orion_conf::error::OrionConfResult;
use orion_variate::EnvDict;
use std::marker::PhantomData;

pub struct ConfDelegate<T: ConfStdOperation> {
    pub(super) path: String,
    pub(super) _x: PhantomData<T>,
}

impl<T: ConfStdOperation> ConfDelegate<T> {
    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
            _x: PhantomData,
        }
    }
    pub fn init(&self) -> OrionConfResult<T> {
        T::init(self.path.as_str())
    }
    pub fn safe_clean(&self) -> OrionConfResult<()> {
        T::safe_clean(self.path.as_str())?;
        Ok(())
    }
    pub fn load(&self, dict: &EnvDict) -> OrionConfResult<T> {
        T::load(self.path.as_str(), dict)
    }
}
