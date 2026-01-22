#[macro_use]
extern crate wp_log;
#[macro_use]
extern crate serde_derive;

// Avoid re-exporting third-party/internal crates at root; apps should depend directly.

shadow_rs::shadow!(build);

#[allow(dead_code)]
#[cfg(any(test, feature = "dev-tools"))]
mod debug;
#[macro_use]
pub mod utils;
// Legacy alias modules can be added here if needed.
pub mod core;
pub(crate) mod orchestrator;
//pub mod dm_run;
pub mod knowledge;
mod protocol;
pub mod resources;
pub mod runtime;
pub mod sinks;
pub mod sources;
pub mod stat;
#[cfg(test)]
pub mod test_support;
pub mod types;
// Facade: 对外提供稳定且收敛的应用入口，隐藏内部实现细节
pub mod facade;
// Engine-side registries and composition helpers for connectors
pub mod connectors;
// 运行期开关（跳过 parse/sink 阶段）
pub mod engine_flags;

// ---------- Logging Sampling Macros ----------
// 使用示例：
// sample_log!(MY_COUNTER, STRIDE, warn_mtrc, "message {}", arg);
// sample_log_with_hits!(MY_COUNTER, STRIDE, info_mtrc, "message {}", arg);

#[macro_export]
macro_rules! sample_log {
    // 基本版：不注入 hits 到日志，使用全局步长 $crate::LOG_SAMPLE_STRIDE
    ($counter:ident, $log_macro:ident, $($args:tt)*) => {{
        static $counter: std::sync::atomic::AtomicU64 =
            std::sync::atomic::AtomicU64::new(0);
        let hits = $counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        let stride: u64 = $crate::LOG_SAMPLE_STRIDE as u64;
        if stride == 0 || hits % stride == 0 {
            $log_macro!($($args)*);
        }
    }};
}

#[macro_export]
macro_rules! sample_log_with_hits {
    // 带 hits 版：自动在消息末尾拼接 ", hits={}"（用于高频抽样情况下标记命中次数），使用全局步长
    ($counter:ident, $log_macro:ident, $fmt:expr $(, $args:expr)*) => {{
        static $counter: std::sync::atomic::AtomicU64 =
            std::sync::atomic::AtomicU64::new(0);
        let hits = $counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        let stride: u64 = $crate::LOG_SAMPLE_STRIDE as u64;
        if stride == 0 || hits % stride == 0 {
            $log_macro!(concat!($fmt, ", hits={}") $(, $args)*, hits);
        }
    }};
}

/// Global sampling stride for logging (0 = always log)
pub const LOG_SAMPLE_STRIDE: u64 = 256;

// Note: plg_pipe implementations are now external to wp-engine core.
// Applications using wp-engine should register extensions dynamically using the register_plg_pipe! macro.
// Example: register_plg_pipe!("MY-PIPE", my_pipe::build_parser);

// Optional enterprise control-plane shim (no-op in community builds)
// Only declare the module when the feature is enabled to avoid unused-module noise.
#[cfg(feature = "enterprise-backend")]
mod wp_ctrl_enterprise;
