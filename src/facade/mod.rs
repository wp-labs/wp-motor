//! Facade 模块：为 CLI 应用提供稳定、简洁的一站式入口。
//! - 隐藏内部装配细节（资源管理、任务组、控制面、PID 管理等）
//! - 暴露少量稳定 API，便于 apps/wparse 与 apps/wprescue 直接调用

pub mod args;
pub mod cli;
pub mod config;
pub mod diagnostics;
pub mod engine;
pub mod enrich;
pub mod generator;
pub mod kit;
pub mod rescue;
pub mod runtime_ctrl;
pub mod test_helpers;
pub mod usecases;

// 常用导出，便于应用端按需使用
pub use engine::WpApp;
pub use rescue::WpRescueApp;
pub use runtime_ctrl::{
    RuntimeCommandResp, RuntimeCommandResult, RuntimeCommandSendError, RuntimeCommandSender,
    RuntimeControlHandle, RuntimeStatusSnapshot,
};

// 可选便捷导出：wp_enrich_api 依赖已注释，暂不可用
// pub use enrich::{EnrichLibAble, EnrichingAble};
