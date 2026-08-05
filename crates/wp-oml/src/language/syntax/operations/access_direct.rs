use crate::language::DirectAccessor;
use crate::language::prelude::*;
use derive_getters::Getters;

/// 访问方向操作：`access_direct(src, dst)`
///
/// 判断 src/dst 两个 IP 地址的内外归属后，组合出访问方向：
/// `内到内` / `内到外` / `外到内` / `外到外`。
/// src/dst 缺失或非法（无法判断）时输出 `Ignore`。
#[derive(Clone, Debug, Getters, PartialEq)]
pub struct AccessDirectOperation {
    src: DirectAccessor,
    dst: DirectAccessor,
}

impl AccessDirectOperation {
    pub fn new(src: DirectAccessor, dst: DirectAccessor) -> Self {
        Self { src, dst }
    }

    pub fn src_mut(&mut self) -> &mut DirectAccessor {
        &mut self.src
    }

    pub fn dst_mut(&mut self) -> &mut DirectAccessor {
        &mut self.dst
    }
}

impl Display for AccessDirectOperation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "access_direct({}, {}) ", self.src, self.dst)
    }
}
