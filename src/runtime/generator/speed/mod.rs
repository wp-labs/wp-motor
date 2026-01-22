//! 动态速度控制模块
//!
//! 提供多种速度变化模型，模拟真实环境下的流量波动。
//!
//! # 模型类型
//! - `Constant` - 恒定速率
//! - `Sinusoidal` - 正弦波动（日夜周期）
//! - `Stepped` - 阶梯变化（业务高峰/低谷）
//! - `Burst` - 突发模式（流量尖峰）
//! - `Ramp` - 渐进模式（压测梯度）
//! - `RandomWalk` - 随机波动（自然抖动）
//! - `Composite` - 复合模式

mod controller;
mod limiter;
pub mod profile;

pub use controller::DynamicSpeedController;
pub use limiter::DynamicRateLimiter;
pub use profile::{CombineMode, SpeedProfile};
