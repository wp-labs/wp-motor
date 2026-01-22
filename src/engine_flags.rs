//! 全局引擎阶段开关（轻量 Runtime 开关），用于在运行期跳过某些阶段。
use std::sync::atomic::{AtomicBool, Ordering};

static SKIP_PARSE: AtomicBool = AtomicBool::new(false);
static SKIP_SINK: AtomicBool = AtomicBool::new(false);

#[inline]
pub fn set_skip_parse(v: bool) {
    SKIP_PARSE.store(v, Ordering::Relaxed);
}
#[inline]
pub fn set_skip_sink(v: bool) {
    SKIP_SINK.store(v, Ordering::Relaxed);
}

#[inline]
pub fn skip_parse() -> bool {
    SKIP_PARSE.load(Ordering::Relaxed)
}
#[inline]
pub fn skip_sink() -> bool {
    SKIP_SINK.load(Ordering::Relaxed)
}
