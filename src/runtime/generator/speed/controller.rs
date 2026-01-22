//! 动态速度控制器
//!
//! 根据 SpeedProfile 计算当前时刻的目标速率

use super::profile::{CombineMode, SpeedProfile};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::time::Instant;

/// 动态速度控制器
///
/// 根据配置的速度模型，计算任意时刻的目标生成速率。
pub struct DynamicSpeedController {
    profile: SpeedProfile,
    start_time: Instant,
    // Stepped 模式状态
    step_idx: usize,
    step_start_time: f64,
    // Burst 模式状态
    in_burst: bool,
    burst_end: Option<Instant>,
    // 随机数生成器 (Send + Sync)
    rng: StdRng,
    // Composite 子控制器（保持子模型状态）
    composite_children: Vec<DynamicSpeedController>,
}

impl DynamicSpeedController {
    /// 创建新的速度控制器
    pub fn new(profile: SpeedProfile) -> Self {
        let start_time = Instant::now();
        Self::with_start_time(profile, start_time)
    }

    fn with_start_time(profile: SpeedProfile, start_time: Instant) -> Self {
        let mut controller = Self {
            profile,
            start_time,
            step_idx: 0,
            step_start_time: 0.0,
            in_burst: false,
            burst_end: None,
            rng: StdRng::from_os_rng(),
            composite_children: Vec::new(),
        };

        if let SpeedProfile::Composite { profiles, .. } = &controller.profile {
            controller.composite_children = profiles
                .iter()
                .cloned()
                .map(|p| DynamicSpeedController::with_start_time(p, start_time))
                .collect();
        }

        controller
    }

    /// 重置控制器状态（重新开始计时）
    pub fn reset(&mut self) {
        let start_time = Instant::now();
        self.reset_with_start_time(start_time);
    }

    fn reset_with_start_time(&mut self, start_time: Instant) {
        self.start_time = start_time;
        self.step_idx = 0;
        self.step_start_time = 0.0;
        self.in_burst = false;
        self.burst_end = None;

        for child in &mut self.composite_children {
            child.reset_with_start_time(start_time);
        }
    }

    /// 获取当前时刻的目标速率
    pub fn current_speed(&mut self) -> usize {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        self.calculate_speed(elapsed)
    }

    /// 获取从指定时间点开始经过的秒数
    pub fn elapsed_secs(&self) -> f64 {
        self.start_time.elapsed().as_secs_f64()
    }

    /// 计算指定时间点的速率
    fn calculate_speed(&mut self, elapsed_secs: f64) -> usize {
        if let SpeedProfile::Composite { combine_mode, .. } = &self.profile {
            let mode = combine_mode.clone();
            return self.calc_composite(mode, elapsed_secs);
        }

        match &self.profile {
            SpeedProfile::Constant(rate) => *rate,

            SpeedProfile::Sinusoidal {
                base,
                amplitude,
                period_secs,
            } => self.calc_sinusoidal(*base, *amplitude, *period_secs, elapsed_secs),

            SpeedProfile::Stepped {
                steps,
                loop_forever,
            } => self.calc_stepped(steps.clone(), *loop_forever, elapsed_secs),

            SpeedProfile::Burst {
                base,
                burst_rate,
                burst_duration_ms,
                burst_probability,
            } => self.calc_burst(*base, *burst_rate, *burst_duration_ms, *burst_probability),

            SpeedProfile::Ramp {
                start,
                end,
                duration_secs,
            } => self.calc_ramp(*start, *end, *duration_secs, elapsed_secs),

            SpeedProfile::RandomWalk { base, variance } => self.calc_random_walk(*base, *variance),

            SpeedProfile::Composite { .. } => unreachable!("Composite handled above"),
        }
    }

    /// 正弦波动计算
    fn calc_sinusoidal(
        &self,
        base: usize,
        amplitude: usize,
        period_secs: f64,
        elapsed_secs: f64,
    ) -> usize {
        if period_secs <= 0.0 {
            return base;
        }
        let phase = (elapsed_secs / period_secs) * 2.0 * std::f64::consts::PI;
        let variation = (amplitude as f64) * phase.sin();
        ((base as f64) + variation).max(1.0) as usize
    }

    /// 阶梯变化计算
    fn calc_stepped(
        &mut self,
        steps: Vec<(f64, usize)>,
        loop_forever: bool,
        elapsed_secs: f64,
    ) -> usize {
        if steps.is_empty() {
            return 1000; // 默认速率
        }

        // 计算当前应该处于哪个阶梯
        let total_duration: f64 = steps.iter().map(|(d, _)| *d).sum();
        if total_duration <= 0.0 {
            return steps[0].1;
        }

        let effective_elapsed = if loop_forever {
            elapsed_secs % total_duration
        } else {
            elapsed_secs.min(total_duration)
        };

        let mut accumulated = 0.0;
        for (duration, rate) in &steps {
            accumulated += duration;
            if effective_elapsed < accumulated {
                return *rate;
            }
        }

        // 如果不循环且超过总时长，返回最后一个速率
        steps.last().map(|(_, r)| *r).unwrap_or(1000)
    }

    /// 突发模式计算
    fn calc_burst(
        &mut self,
        base: usize,
        burst_rate: usize,
        burst_duration_ms: u64,
        burst_probability: f64,
    ) -> usize {
        let now = Instant::now();

        // 检查是否仍在突发中
        if let Some(end) = self.burst_end {
            if now < end {
                return burst_rate;
            }
            // 突发结束
            self.in_burst = false;
            self.burst_end = None;
        }

        // 尝试触发新的突发（概率检查）
        // 注意：此方法可能被频繁调用，因此实际概率需要调整
        // 假设每 100ms 调用一次，probability 是每秒概率
        let check_probability = burst_probability / 10.0; // 调整为每次调用的概率
        if self.rng.random::<f64>() < check_probability {
            self.in_burst = true;
            self.burst_end = Some(now + std::time::Duration::from_millis(burst_duration_ms));
            return burst_rate;
        }

        base
    }

    /// 渐进模式计算
    fn calc_ramp(&self, start: usize, end: usize, duration_secs: f64, elapsed_secs: f64) -> usize {
        if duration_secs <= 0.0 {
            return end;
        }

        let progress = (elapsed_secs / duration_secs).clamp(0.0, 1.0);
        let rate = (start as f64) + ((end as f64) - (start as f64)) * progress;
        rate.max(1.0) as usize
    }

    /// 随机波动计算
    fn calc_random_walk(&mut self, base: usize, variance: f64) -> usize {
        let noise = self.rng.random::<f64>() * 2.0 * variance - variance;
        ((base as f64) * (1.0 + noise)).max(1.0) as usize
    }

    /// 复合模式计算
    fn calc_composite(&mut self, combine_mode: CombineMode, elapsed_secs: f64) -> usize {
        if self.composite_children.is_empty() {
            return 1000;
        }

        let rates: Vec<usize> = self
            .composite_children
            .iter_mut()
            .map(|child| child.calculate_speed(elapsed_secs))
            .collect();

        match combine_mode {
            CombineMode::Average => rates.iter().sum::<usize>() / rates.len(),
            CombineMode::Max => rates.iter().copied().max().unwrap_or(1000),
            CombineMode::Min => rates.iter().copied().min().unwrap_or(1000),
            CombineMode::Sum => rates.iter().sum(),
        }
    }
}

impl Default for DynamicSpeedController {
    fn default() -> Self {
        Self::new(SpeedProfile::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn test_default_controller() {
        let mut ctrl = DynamicSpeedController::default();
        // 默认是 Constant(1000)
        assert_eq!(ctrl.current_speed(), 1000);
    }

    #[test]
    fn test_constant_speed() {
        let mut ctrl = DynamicSpeedController::new(SpeedProfile::Constant(5000));
        assert_eq!(ctrl.current_speed(), 5000);
        assert_eq!(ctrl.current_speed(), 5000);

        // 等待后仍然是恒定值
        sleep(Duration::from_millis(100));
        assert_eq!(ctrl.current_speed(), 5000);
    }

    #[test]
    fn test_constant_zero_speed() {
        let mut ctrl = DynamicSpeedController::new(SpeedProfile::Constant(0));
        assert_eq!(ctrl.current_speed(), 0);
    }

    #[test]
    fn test_sinusoidal_speed() {
        let mut ctrl = DynamicSpeedController::new(SpeedProfile::Sinusoidal {
            base: 5000,
            amplitude: 2000,
            period_secs: 1.0, // 1秒周期便于测试
        });

        // 初始时刻接近 base
        let speed = ctrl.current_speed();
        assert!((3000..=7000).contains(&speed));
    }

    #[test]
    fn test_sinusoidal_zero_period() {
        let mut ctrl = DynamicSpeedController::new(SpeedProfile::Sinusoidal {
            base: 5000,
            amplitude: 2000,
            period_secs: 0.0, // 零周期应返回 base
        });

        assert_eq!(ctrl.current_speed(), 5000);
    }

    #[test]
    fn test_ramp_speed() {
        let mut ctrl = DynamicSpeedController::new(SpeedProfile::Ramp {
            start: 100,
            end: 1000,
            duration_secs: 1.0,
        });

        // 初始时刻接近 start
        let speed = ctrl.current_speed();
        assert!((100..=200).contains(&speed));

        // 等待后速率应增加
        sleep(Duration::from_millis(500));
        let speed = ctrl.current_speed();
        assert!((400..=700).contains(&speed));
    }

    #[test]
    fn test_ramp_zero_duration() {
        let mut ctrl = DynamicSpeedController::new(SpeedProfile::Ramp {
            start: 100,
            end: 1000,
            duration_secs: 0.0, // 零时长应直接返回 end
        });

        assert_eq!(ctrl.current_speed(), 1000);
    }

    #[test]
    fn test_ramp_reverse() {
        // 测试从高到低的渐变
        let mut ctrl = DynamicSpeedController::new(SpeedProfile::Ramp {
            start: 1000,
            end: 100,
            duration_secs: 1.0,
        });

        let speed = ctrl.current_speed();
        assert!((900..=1000).contains(&speed));

        sleep(Duration::from_millis(500));
        let speed = ctrl.current_speed();
        assert!((400..=600).contains(&speed));
    }

    #[test]
    fn test_stepped_speed() {
        let mut ctrl = DynamicSpeedController::new(SpeedProfile::Stepped {
            steps: vec![(0.5, 1000), (0.5, 5000)],
            loop_forever: false,
        });

        // 第一阶段
        assert_eq!(ctrl.current_speed(), 1000);

        // 等待进入第二阶段
        sleep(Duration::from_millis(600));
        assert_eq!(ctrl.current_speed(), 5000);
    }

    #[test]
    fn test_stepped_loop_forever() {
        let mut ctrl = DynamicSpeedController::new(SpeedProfile::Stepped {
            steps: vec![(0.3, 1000), (0.3, 2000)],
            loop_forever: true,
        });

        // 第一阶段
        assert_eq!(ctrl.current_speed(), 1000);

        // 等待进入第二阶段
        sleep(Duration::from_millis(350));
        assert_eq!(ctrl.current_speed(), 2000);

        // 循环回到第一阶段
        sleep(Duration::from_millis(350));
        assert_eq!(ctrl.current_speed(), 1000);
    }

    #[test]
    fn test_stepped_empty() {
        let mut ctrl = DynamicSpeedController::new(SpeedProfile::Stepped {
            steps: vec![],
            loop_forever: false,
        });

        // 空步骤返回默认值
        assert_eq!(ctrl.current_speed(), 1000);
    }

    #[test]
    fn test_random_walk_speed() {
        let mut ctrl = DynamicSpeedController::new(SpeedProfile::RandomWalk {
            base: 5000,
            variance: 0.3,
        });

        // 多次采样应在范围内
        for _ in 0..10 {
            let speed = ctrl.current_speed();
            assert!((3500..=6500).contains(&speed));
        }
    }

    #[test]
    fn test_random_walk_zero_variance() {
        let mut ctrl = DynamicSpeedController::new(SpeedProfile::RandomWalk {
            base: 5000,
            variance: 0.0,
        });

        // 零方差应返回恒定值
        assert_eq!(ctrl.current_speed(), 5000);
        assert_eq!(ctrl.current_speed(), 5000);
    }

    #[test]
    fn test_burst_base_rate() {
        let mut ctrl = DynamicSpeedController::new(SpeedProfile::Burst {
            base: 1000,
            burst_rate: 10000,
            burst_duration_ms: 100,
            burst_probability: 0.0, // 不触发突发
        });

        // 概率为0，应该始终返回 base
        for _ in 0..10 {
            assert_eq!(ctrl.current_speed(), 1000);
        }
    }

    #[test]
    fn test_composite_average() {
        let mut ctrl = DynamicSpeedController::new(SpeedProfile::Composite {
            profiles: vec![SpeedProfile::Constant(1000), SpeedProfile::Constant(3000)],
            combine_mode: CombineMode::Average,
        });

        assert_eq!(ctrl.current_speed(), 2000);
    }

    #[test]
    fn test_composite_max() {
        let mut ctrl = DynamicSpeedController::new(SpeedProfile::Composite {
            profiles: vec![SpeedProfile::Constant(1000), SpeedProfile::Constant(3000)],
            combine_mode: CombineMode::Max,
        });

        assert_eq!(ctrl.current_speed(), 3000);
    }

    #[test]
    fn test_composite_min() {
        let mut ctrl = DynamicSpeedController::new(SpeedProfile::Composite {
            profiles: vec![SpeedProfile::Constant(1000), SpeedProfile::Constant(3000)],
            combine_mode: CombineMode::Min,
        });

        assert_eq!(ctrl.current_speed(), 1000);
    }

    #[test]
    fn test_composite_sum() {
        let mut ctrl = DynamicSpeedController::new(SpeedProfile::Composite {
            profiles: vec![SpeedProfile::Constant(1000), SpeedProfile::Constant(3000)],
            combine_mode: CombineMode::Sum,
        });

        assert_eq!(ctrl.current_speed(), 4000);
    }

    #[test]
    fn test_composite_empty() {
        let mut ctrl = DynamicSpeedController::new(SpeedProfile::Composite {
            profiles: vec![],
            combine_mode: CombineMode::Average,
        });

        // 空列表返回默认值
        assert_eq!(ctrl.current_speed(), 1000);
    }

    #[test]
    fn test_composite_with_stateful_child() {
        let mut ctrl = DynamicSpeedController::new(SpeedProfile::Composite {
            profiles: vec![
                SpeedProfile::Stepped {
                    steps: vec![(0.2, 100), (0.2, 2000)],
                    loop_forever: false,
                },
                SpeedProfile::Constant(500),
            ],
            combine_mode: CombineMode::Max,
        });

        // 初始阶段取最大值，来自恒定 500
        assert_eq!(ctrl.current_speed(), 500);

        // 等待进入第二个阶梯
        sleep(Duration::from_millis(250));
        assert_eq!(ctrl.current_speed(), 2000);
    }

    #[test]
    fn test_reset() {
        let mut ctrl = DynamicSpeedController::new(SpeedProfile::Ramp {
            start: 100,
            end: 1000,
            duration_secs: 1.0,
        });

        // 等待一段时间让速率增加
        sleep(Duration::from_millis(500));
        let speed_before = ctrl.current_speed();
        assert!(speed_before > 200);

        // 重置后应回到初始状态
        ctrl.reset();
        let speed_after = ctrl.current_speed();
        assert!((100..=200).contains(&speed_after));
    }

    #[test]
    fn test_elapsed_secs() {
        let ctrl = DynamicSpeedController::new(SpeedProfile::Constant(1000));

        let elapsed1 = ctrl.elapsed_secs();
        sleep(Duration::from_millis(100));
        let elapsed2 = ctrl.elapsed_secs();

        assert!(elapsed2 > elapsed1);
        assert!(elapsed2 - elapsed1 >= 0.09); // 至少 90ms
    }
}
