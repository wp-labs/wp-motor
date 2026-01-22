//! 速度变化模型定义

/// 速度变化模型
///
/// 定义数据生成速率随时间变化的模式，用于模拟真实环境下的流量特征。
#[derive(Clone, Debug)]
pub enum SpeedProfile {
    /// 恒定速率（当前默认行为）
    ///
    /// # 参数
    /// - `rate` - 每秒生成行数
    Constant(usize),

    /// 正弦波动模式
    ///
    /// 速率公式: `base + amplitude * sin(2π * t / period_secs)`
    ///
    /// 适用场景：模拟日夜流量周期、工作日/周末波动等周期性变化
    ///
    /// # 示例
    /// ```ignore
    /// // 模拟24小时周期，基准5000/s，波动±3000
    /// SpeedProfile::Sinusoidal {
    ///     base: 5000,
    ///     amplitude: 3000,
    ///     period_secs: 86400.0,  // 24小时
    /// }
    /// ```
    Sinusoidal {
        /// 基准速率 (行/秒)
        base: usize,
        /// 波动幅度 (行/秒)，实际速率在 [base-amplitude, base+amplitude] 范围内
        amplitude: usize,
        /// 周期长度 (秒)
        period_secs: f64,
    },

    /// 阶梯变化模式
    ///
    /// 按预定义的时间段切换不同速率
    ///
    /// 适用场景：模拟业务高峰/低谷时段、分阶段压测
    ///
    /// # 示例
    /// ```ignore
    /// // 模拟：低峰30s -> 中峰30s -> 高峰30s -> 回落30s
    /// SpeedProfile::Stepped {
    ///     steps: vec![
    ///         (30.0, 1000),   // 30秒 1000/s
    ///         (30.0, 5000),   // 30秒 5000/s
    ///         (30.0, 10000),  // 30秒 10000/s
    ///         (30.0, 2000),   // 30秒 2000/s
    ///     ],
    ///     loop_forever: true,
    /// }
    /// ```
    Stepped {
        /// 阶梯配置：(持续时间秒, 目标速率)
        steps: Vec<(f64, usize)>,
        /// 是否循环执行，false 时执行完毕后保持最后速率
        loop_forever: bool,
    },

    /// 突发模式
    ///
    /// 在基准速率基础上随机触发高速突发
    ///
    /// 适用场景：模拟流量尖峰、热点事件、DDoS 攻击等
    ///
    /// # 示例
    /// ```ignore
    /// // 基准1000/s，随机突发到10000/s，持续500ms
    /// SpeedProfile::Burst {
    ///     base: 1000,
    ///     burst_rate: 10000,
    ///     burst_duration_ms: 500,
    ///     burst_probability: 0.05,  // 每秒5%概率触发
    /// }
    /// ```
    Burst {
        /// 基准速率 (行/秒)
        base: usize,
        /// 突发时速率 (行/秒)
        burst_rate: usize,
        /// 突发持续时间 (毫秒)
        burst_duration_ms: u64,
        /// 每秒触发突发的概率 (0.0-1.0)
        burst_probability: f64,
    },

    /// 渐进模式（斜坡）
    ///
    /// 速率从 start 线性变化到 end
    ///
    /// 适用场景：系统预热、压力测试梯度、平滑扩缩容
    ///
    /// # 示例
    /// ```ignore
    /// // 5分钟内从100/s升到10000/s
    /// SpeedProfile::Ramp {
    ///     start: 100,
    ///     end: 10000,
    ///     duration_secs: 300.0,
    /// }
    /// ```
    Ramp {
        /// 起始速率 (行/秒)
        start: usize,
        /// 目标速率 (行/秒)
        end: usize,
        /// 变化持续时间 (秒)，到达后保持 end 速率
        duration_secs: f64,
    },

    /// 随机波动模式
    ///
    /// 速率公式: `base * (1 + random(-variance, +variance))`
    ///
    /// 适用场景：模拟自然流量的随机抖动
    ///
    /// # 示例
    /// ```ignore
    /// // 基准5000/s，随机波动±30%
    /// SpeedProfile::RandomWalk {
    ///     base: 5000,
    ///     variance: 0.3,
    /// }
    /// ```
    RandomWalk {
        /// 基准速率 (行/秒)
        base: usize,
        /// 波动范围 (0.0-1.0)，如 0.3 表示 ±30%
        variance: f64,
    },

    /// 复合模式
    ///
    /// 将多个模型的速率取平均值（或可配置为叠加/取最大等）
    ///
    /// 适用场景：模拟复杂的真实流量特征
    ///
    /// # 示例
    /// ```ignore
    /// // 周期波动 + 随机抖动
    /// SpeedProfile::Composite {
    ///     profiles: vec![
    ///         SpeedProfile::Sinusoidal { base: 5000, amplitude: 2000, period_secs: 60.0 },
    ///         SpeedProfile::RandomWalk { base: 5000, variance: 0.1 },
    ///     ],
    ///     combine_mode: CombineMode::Average,
    /// }
    /// ```
    Composite {
        /// 子模型列表
        profiles: Vec<SpeedProfile>,
        /// 组合方式
        combine_mode: CombineMode,
    },
}

/// 复合模式的组合方式
#[derive(Clone, Debug, Default)]
pub enum CombineMode {
    /// 取平均值
    #[default]
    Average,
    /// 取最大值
    Max,
    /// 取最小值
    Min,
    /// 累加（注意可能产生极高速率）
    Sum,
}

impl Default for SpeedProfile {
    fn default() -> Self {
        SpeedProfile::Constant(1000)
    }
}

impl SpeedProfile {
    /// 创建恒定速率模型
    pub fn constant(rate: usize) -> Self {
        SpeedProfile::Constant(rate)
    }

    /// 创建正弦波动模型
    pub fn sinusoidal(base: usize, amplitude: usize, period_secs: f64) -> Self {
        SpeedProfile::Sinusoidal {
            base,
            amplitude,
            period_secs,
        }
    }

    /// 创建阶梯变化模型
    pub fn stepped(steps: Vec<(f64, usize)>, loop_forever: bool) -> Self {
        SpeedProfile::Stepped {
            steps,
            loop_forever,
        }
    }

    /// 创建突发模式模型
    pub fn burst(
        base: usize,
        burst_rate: usize,
        burst_duration_ms: u64,
        burst_probability: f64,
    ) -> Self {
        SpeedProfile::Burst {
            base,
            burst_rate,
            burst_duration_ms,
            burst_probability: burst_probability.clamp(0.0, 1.0),
        }
    }

    /// 创建渐进模式模型
    pub fn ramp(start: usize, end: usize, duration_secs: f64) -> Self {
        SpeedProfile::Ramp {
            start,
            end,
            duration_secs,
        }
    }

    /// 创建随机波动模型
    pub fn random_walk(base: usize, variance: f64) -> Self {
        SpeedProfile::RandomWalk {
            base,
            variance: variance.clamp(0.0, 1.0),
        }
    }

    /// 创建复合模型
    pub fn composite(profiles: Vec<SpeedProfile>, combine_mode: CombineMode) -> Self {
        SpeedProfile::Composite {
            profiles,
            combine_mode,
        }
    }

    /// 获取基准速率（用于初始化和估算）
    pub fn base_rate(&self) -> usize {
        match self {
            SpeedProfile::Constant(rate) => *rate,
            SpeedProfile::Sinusoidal { base, .. } => *base,
            SpeedProfile::Stepped { steps, .. } => steps.first().map(|(_, r)| *r).unwrap_or(1000),
            SpeedProfile::Burst { base, .. } => *base,
            SpeedProfile::Ramp { start, .. } => *start,
            SpeedProfile::RandomWalk { base, .. } => *base,
            SpeedProfile::Composite { profiles, .. } => {
                let sum: usize = profiles.iter().map(|p| p.base_rate()).sum();
                sum / profiles.len().max(1)
            }
        }
    }

    /// 是否为恒定速率模型
    pub fn is_constant(&self) -> bool {
        matches!(self, SpeedProfile::Constant(_))
    }
}

/// 从配置类型转换为运行时类型
impl From<wp_conf::SpeedProfileConfig> for SpeedProfile {
    fn from(config: wp_conf::SpeedProfileConfig) -> Self {
        use wp_conf::SpeedProfileConfig as Cfg;
        match config {
            Cfg::Constant { rate } => SpeedProfile::Constant(rate),
            Cfg::Sinusoidal {
                base,
                amplitude,
                period_secs,
            } => SpeedProfile::Sinusoidal {
                base,
                amplitude,
                period_secs,
            },
            Cfg::Stepped {
                steps,
                loop_forever,
            } => SpeedProfile::Stepped {
                steps,
                loop_forever,
            },
            Cfg::Burst {
                base,
                burst_rate,
                burst_duration_ms,
                burst_probability,
            } => SpeedProfile::Burst {
                base,
                burst_rate,
                burst_duration_ms,
                burst_probability: burst_probability.clamp(0.0, 1.0),
            },
            Cfg::Ramp {
                start,
                end,
                duration_secs,
            } => SpeedProfile::Ramp {
                start,
                end,
                duration_secs,
            },
            Cfg::RandomWalk { base, variance } => SpeedProfile::RandomWalk {
                base,
                variance: variance.clamp(0.0, 1.0),
            },
            Cfg::Composite {
                profiles,
                combine_mode,
            } => SpeedProfile::Composite {
                profiles: profiles.into_iter().map(SpeedProfile::from).collect(),
                combine_mode: combine_mode.into(),
            },
        }
    }
}

/// 从配置类型转换为运行时类型
impl From<wp_conf::CombineModeConfig> for CombineMode {
    fn from(config: wp_conf::CombineModeConfig) -> Self {
        use wp_conf::CombineModeConfig as Cfg;
        match config {
            Cfg::Average => CombineMode::Average,
            Cfg::Max => CombineMode::Max,
            Cfg::Min => CombineMode::Min,
            Cfg::Sum => CombineMode::Sum,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_profile() {
        let profile = SpeedProfile::default();
        assert!(matches!(profile, SpeedProfile::Constant(1000)));
    }

    #[test]
    fn test_default_combine_mode() {
        let mode = CombineMode::default();
        assert!(matches!(mode, CombineMode::Average));
    }

    #[test]
    fn test_base_rate_constant() {
        assert_eq!(SpeedProfile::constant(5000).base_rate(), 5000);
        assert_eq!(SpeedProfile::constant(0).base_rate(), 0);
        assert_eq!(SpeedProfile::constant(1).base_rate(), 1);
    }

    #[test]
    fn test_base_rate_sinusoidal() {
        assert_eq!(SpeedProfile::sinusoidal(3000, 1000, 60.0).base_rate(), 3000);
        assert_eq!(SpeedProfile::sinusoidal(0, 1000, 60.0).base_rate(), 0);
    }

    #[test]
    fn test_base_rate_stepped() {
        let profile = SpeedProfile::stepped(vec![(10.0, 1000), (10.0, 5000)], true);
        assert_eq!(profile.base_rate(), 1000); // 返回第一个步骤的速率

        let empty = SpeedProfile::stepped(vec![], false);
        assert_eq!(empty.base_rate(), 1000); // 空步骤返回默认值
    }

    #[test]
    fn test_base_rate_burst() {
        let profile = SpeedProfile::burst(2000, 10000, 500, 0.1);
        assert_eq!(profile.base_rate(), 2000);
    }

    #[test]
    fn test_base_rate_ramp() {
        assert_eq!(SpeedProfile::ramp(100, 10000, 300.0).base_rate(), 100);
        assert_eq!(SpeedProfile::ramp(0, 10000, 300.0).base_rate(), 0);
    }

    #[test]
    fn test_base_rate_random_walk() {
        assert_eq!(SpeedProfile::random_walk(5000, 0.3).base_rate(), 5000);
    }

    #[test]
    fn test_base_rate_composite() {
        let profile = SpeedProfile::composite(
            vec![SpeedProfile::constant(1000), SpeedProfile::constant(3000)],
            CombineMode::Average,
        );
        assert_eq!(profile.base_rate(), 2000); // (1000 + 3000) / 2

        let empty = SpeedProfile::composite(vec![], CombineMode::Average);
        assert_eq!(empty.base_rate(), 0); // 空列表返回 0
    }

    #[test]
    fn test_builder_constant() {
        let p = SpeedProfile::constant(5000);
        assert!(matches!(p, SpeedProfile::Constant(5000)));
        assert!(p.is_constant());
    }

    #[test]
    fn test_builder_sinusoidal() {
        let p = SpeedProfile::sinusoidal(5000, 2000, 60.0);
        if let SpeedProfile::Sinusoidal {
            base,
            amplitude,
            period_secs,
        } = p
        {
            assert_eq!(base, 5000);
            assert_eq!(amplitude, 2000);
            assert!((period_secs - 60.0).abs() < 0.001);
        } else {
            panic!("Expected Sinusoidal variant");
        }
        assert!(!p.is_constant());
    }

    #[test]
    fn test_builder_stepped() {
        let steps = vec![(30.0, 1000), (30.0, 5000), (30.0, 2000)];
        let p = SpeedProfile::stepped(steps.clone(), true);
        if let SpeedProfile::Stepped {
            steps: s,
            loop_forever,
        } = p
        {
            assert_eq!(s.len(), 3);
            assert!(loop_forever);
        } else {
            panic!("Expected Stepped variant");
        }
    }

    #[test]
    fn test_builder_burst() {
        let p = SpeedProfile::burst(1000, 10000, 500, 0.1);
        if let SpeedProfile::Burst {
            base,
            burst_rate,
            burst_duration_ms,
            burst_probability,
        } = p
        {
            assert_eq!(base, 1000);
            assert_eq!(burst_rate, 10000);
            assert_eq!(burst_duration_ms, 500);
            assert!((burst_probability - 0.1).abs() < 0.001);
        } else {
            panic!("Expected Burst variant");
        }
    }

    #[test]
    fn test_builder_burst_clamp_probability() {
        // 超出范围应被 clamp
        let p = SpeedProfile::burst(1000, 10000, 500, 1.5);
        if let SpeedProfile::Burst {
            burst_probability, ..
        } = p
        {
            assert_eq!(burst_probability, 1.0);
        }

        let p = SpeedProfile::burst(1000, 10000, 500, -0.5);
        if let SpeedProfile::Burst {
            burst_probability, ..
        } = p
        {
            assert_eq!(burst_probability, 0.0);
        }
    }

    #[test]
    fn test_builder_ramp() {
        let p = SpeedProfile::ramp(100, 10000, 300.0);
        if let SpeedProfile::Ramp {
            start,
            end,
            duration_secs,
        } = p
        {
            assert_eq!(start, 100);
            assert_eq!(end, 10000);
            assert!((duration_secs - 300.0).abs() < 0.001);
        } else {
            panic!("Expected Ramp variant");
        }
    }

    #[test]
    fn test_builder_random_walk() {
        let p = SpeedProfile::random_walk(5000, 0.3);
        if let SpeedProfile::RandomWalk { base, variance } = p {
            assert_eq!(base, 5000);
            assert!((variance - 0.3).abs() < 0.001);
        } else {
            panic!("Expected RandomWalk variant");
        }
    }

    #[test]
    fn test_builder_random_walk_clamp_variance() {
        // 超出范围应被 clamp
        let p = SpeedProfile::random_walk(5000, 1.5);
        if let SpeedProfile::RandomWalk { variance, .. } = p {
            assert_eq!(variance, 1.0);
        }

        let p = SpeedProfile::random_walk(5000, -0.5);
        if let SpeedProfile::RandomWalk { variance, .. } = p {
            assert_eq!(variance, 0.0);
        }
    }

    #[test]
    fn test_builder_composite() {
        let profiles = vec![
            SpeedProfile::constant(1000),
            SpeedProfile::sinusoidal(2000, 500, 60.0),
        ];
        let p = SpeedProfile::composite(profiles, CombineMode::Max);
        if let SpeedProfile::Composite {
            profiles: ps,
            combine_mode,
        } = p
        {
            assert_eq!(ps.len(), 2);
            assert!(matches!(combine_mode, CombineMode::Max));
        } else {
            panic!("Expected Composite variant");
        }
    }

    #[test]
    fn test_is_constant() {
        assert!(SpeedProfile::constant(1000).is_constant());
        assert!(!SpeedProfile::sinusoidal(1000, 500, 60.0).is_constant());
        assert!(!SpeedProfile::stepped(vec![], false).is_constant());
        assert!(!SpeedProfile::burst(1000, 5000, 500, 0.1).is_constant());
        assert!(!SpeedProfile::ramp(100, 1000, 60.0).is_constant());
        assert!(!SpeedProfile::random_walk(1000, 0.3).is_constant());
        assert!(!SpeedProfile::composite(vec![], CombineMode::Average).is_constant());
    }

    #[test]
    fn test_clone() {
        let p1 = SpeedProfile::sinusoidal(5000, 2000, 60.0);
        let p2 = p1.clone();

        if let (
            SpeedProfile::Sinusoidal {
                base: b1,
                amplitude: a1,
                period_secs: ps1,
            },
            SpeedProfile::Sinusoidal {
                base: b2,
                amplitude: a2,
                period_secs: ps2,
            },
        ) = (&p1, &p2)
        {
            assert_eq!(b1, b2);
            assert_eq!(a1, a2);
            assert!((ps1 - ps2).abs() < 0.001);
        }
    }

    #[test]
    fn test_combine_mode_clone() {
        let modes = [
            CombineMode::Average,
            CombineMode::Max,
            CombineMode::Min,
            CombineMode::Sum,
        ];

        for mode in &modes {
            let cloned = mode.clone();
            assert!(std::mem::discriminant(mode) == std::mem::discriminant(&cloned));
        }
    }

    // ========== Tests for From<SpeedProfileConfig> ==========

    #[test]
    fn test_from_config_constant() {
        let config = wp_conf::SpeedProfileConfig::Constant { rate: 5000 };
        let profile: SpeedProfile = config.into();
        assert!(matches!(profile, SpeedProfile::Constant(5000)));
    }

    #[test]
    fn test_from_config_sinusoidal() {
        let config = wp_conf::SpeedProfileConfig::Sinusoidal {
            base: 5000,
            amplitude: 2000,
            period_secs: 60.0,
        };
        let profile: SpeedProfile = config.into();
        if let SpeedProfile::Sinusoidal {
            base,
            amplitude,
            period_secs,
        } = profile
        {
            assert_eq!(base, 5000);
            assert_eq!(amplitude, 2000);
            assert!((period_secs - 60.0).abs() < 0.001);
        } else {
            panic!("Expected Sinusoidal");
        }
    }

    #[test]
    fn test_from_config_stepped() {
        let config = wp_conf::SpeedProfileConfig::Stepped {
            steps: vec![(30.0, 1000), (30.0, 5000)],
            loop_forever: true,
        };
        let profile: SpeedProfile = config.into();
        if let SpeedProfile::Stepped {
            steps,
            loop_forever,
        } = profile
        {
            assert_eq!(steps.len(), 2);
            assert!(loop_forever);
        } else {
            panic!("Expected Stepped");
        }
    }

    #[test]
    fn test_from_config_burst() {
        let config = wp_conf::SpeedProfileConfig::Burst {
            base: 1000,
            burst_rate: 10000,
            burst_duration_ms: 500,
            burst_probability: 0.05,
        };
        let profile: SpeedProfile = config.into();
        if let SpeedProfile::Burst {
            base,
            burst_rate,
            burst_duration_ms,
            burst_probability,
        } = profile
        {
            assert_eq!(base, 1000);
            assert_eq!(burst_rate, 10000);
            assert_eq!(burst_duration_ms, 500);
            assert!((burst_probability - 0.05).abs() < 0.001);
        } else {
            panic!("Expected Burst");
        }
    }

    #[test]
    fn test_from_config_ramp() {
        let config = wp_conf::SpeedProfileConfig::Ramp {
            start: 100,
            end: 10000,
            duration_secs: 300.0,
        };
        let profile: SpeedProfile = config.into();
        if let SpeedProfile::Ramp {
            start,
            end,
            duration_secs,
        } = profile
        {
            assert_eq!(start, 100);
            assert_eq!(end, 10000);
            assert!((duration_secs - 300.0).abs() < 0.001);
        } else {
            panic!("Expected Ramp");
        }
    }

    #[test]
    fn test_from_config_random_walk() {
        let config = wp_conf::SpeedProfileConfig::RandomWalk {
            base: 5000,
            variance: 0.3,
        };
        let profile: SpeedProfile = config.into();
        if let SpeedProfile::RandomWalk { base, variance } = profile {
            assert_eq!(base, 5000);
            assert!((variance - 0.3).abs() < 0.001);
        } else {
            panic!("Expected RandomWalk");
        }
    }

    #[test]
    fn test_from_config_composite() {
        let config = wp_conf::SpeedProfileConfig::Composite {
            profiles: vec![
                wp_conf::SpeedProfileConfig::Constant { rate: 1000 },
                wp_conf::SpeedProfileConfig::Constant { rate: 3000 },
            ],
            combine_mode: wp_conf::CombineModeConfig::Max,
        };
        let profile: SpeedProfile = config.into();
        if let SpeedProfile::Composite {
            profiles,
            combine_mode,
        } = profile
        {
            assert_eq!(profiles.len(), 2);
            assert!(matches!(combine_mode, CombineMode::Max));
        } else {
            panic!("Expected Composite");
        }
    }

    #[test]
    fn test_from_config_combine_modes() {
        let modes = [
            (wp_conf::CombineModeConfig::Average, CombineMode::Average),
            (wp_conf::CombineModeConfig::Max, CombineMode::Max),
            (wp_conf::CombineModeConfig::Min, CombineMode::Min),
            (wp_conf::CombineModeConfig::Sum, CombineMode::Sum),
        ];

        for (cfg_mode, expected) in modes {
            let result: CombineMode = cfg_mode.into();
            assert!(std::mem::discriminant(&result) == std::mem::discriminant(&expected));
        }
    }
}
