//! 速度变化模型配置
//!
//! 支持从 TOML 配置文件解析各种速度变化模式

use serde_derive::{Deserialize, Serialize};

/// 速度变化模型配置
///
/// 用于在配置文件中定义数据生成的速度变化模式
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SpeedProfileConfig {
    /// 恒定速率
    ///
    /// ```toml
    /// [generator.speed_profile]
    /// type = "constant"
    /// rate = 5000
    /// ```
    Constant {
        /// 每秒生成行数
        rate: usize,
    },

    /// 正弦波动模式
    ///
    /// ```toml
    /// [generator.speed_profile]
    /// type = "sinusoidal"
    /// base = 5000
    /// amplitude = 2000
    /// period_secs = 60.0
    /// ```
    Sinusoidal {
        /// 基准速率 (行/秒)
        base: usize,
        /// 波动幅度 (行/秒)
        amplitude: usize,
        /// 周期长度 (秒)
        period_secs: f64,
    },

    /// 阶梯变化模式
    ///
    /// ```toml
    /// [generator.speed_profile]
    /// type = "stepped"
    /// steps = [[30.0, 1000], [30.0, 5000], [30.0, 2000]]
    /// loop_forever = true
    /// ```
    Stepped {
        /// 阶梯配置：[(持续时间秒, 目标速率), ...]
        steps: Vec<(f64, usize)>,
        /// 是否循环执行
        #[serde(default)]
        loop_forever: bool,
    },

    /// 突发模式
    ///
    /// ```toml
    /// [generator.speed_profile]
    /// type = "burst"
    /// base = 1000
    /// burst_rate = 10000
    /// burst_duration_ms = 500
    /// burst_probability = 0.05
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
    /// ```toml
    /// [generator.speed_profile]
    /// type = "ramp"
    /// start = 100
    /// end = 10000
    /// duration_secs = 300.0
    /// ```
    Ramp {
        /// 起始速率 (行/秒)
        start: usize,
        /// 目标速率 (行/秒)
        end: usize,
        /// 变化持续时间 (秒)
        duration_secs: f64,
    },

    /// 随机波动模式
    ///
    /// ```toml
    /// [generator.speed_profile]
    /// type = "random_walk"
    /// base = 5000
    /// variance = 0.3
    /// ```
    RandomWalk {
        /// 基准速率 (行/秒)
        base: usize,
        /// 波动范围 (0.0-1.0)，如 0.3 表示 ±30%
        variance: f64,
    },

    /// 复合模式
    ///
    /// ```toml
    /// [generator.speed_profile]
    /// type = "composite"
    /// combine_mode = "average"
    ///
    /// [[generator.speed_profile.profiles]]
    /// type = "sinusoidal"
    /// base = 5000
    /// amplitude = 2000
    /// period_secs = 60.0
    ///
    /// [[generator.speed_profile.profiles]]
    /// type = "random_walk"
    /// base = 5000
    /// variance = 0.1
    /// ```
    Composite {
        /// 子模型列表
        profiles: Vec<SpeedProfileConfig>,
        /// 组合方式
        #[serde(default)]
        combine_mode: CombineModeConfig,
    },
}

/// 复合模式的组合方式
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CombineModeConfig {
    /// 取平均值
    #[default]
    Average,
    /// 取最大值
    Max,
    /// 取最小值
    Min,
    /// 累加
    Sum,
}

impl Default for SpeedProfileConfig {
    fn default() -> Self {
        SpeedProfileConfig::Constant { rate: 1000 }
    }
}

impl SpeedProfileConfig {
    /// 获取基准速率（用于估算）
    pub fn base_rate(&self) -> usize {
        match self {
            SpeedProfileConfig::Constant { rate } => *rate,
            SpeedProfileConfig::Sinusoidal { base, .. } => *base,
            SpeedProfileConfig::Stepped { steps, .. } => {
                steps.first().map(|(_, r)| *r).unwrap_or(1000)
            }
            SpeedProfileConfig::Burst { base, .. } => *base,
            SpeedProfileConfig::Ramp { start, .. } => *start,
            SpeedProfileConfig::RandomWalk { base, .. } => *base,
            SpeedProfileConfig::Composite { profiles, .. } => {
                let sum: usize = profiles.iter().map(|p| p.base_rate()).sum();
                sum / profiles.len().max(1)
            }
        }
    }

    /// 是否为恒定速率
    pub fn is_constant(&self) -> bool {
        matches!(self, SpeedProfileConfig::Constant { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_config() {
        let toml_str = r#"
            type = "constant"
            rate = 5000
        "#;
        let config: SpeedProfileConfig = toml::from_str(toml_str).unwrap();
        assert!(matches!(
            config,
            SpeedProfileConfig::Constant { rate: 5000 }
        ));
        assert_eq!(config.base_rate(), 5000);
    }

    #[test]
    fn test_sinusoidal_config() {
        let toml_str = r#"
            type = "sinusoidal"
            base = 5000
            amplitude = 2000
            period_secs = 60.0
        "#;
        let config: SpeedProfileConfig = toml::from_str(toml_str).unwrap();
        if let SpeedProfileConfig::Sinusoidal {
            base,
            amplitude,
            period_secs,
        } = config
        {
            assert_eq!(base, 5000);
            assert_eq!(amplitude, 2000);
            assert!((period_secs - 60.0).abs() < 0.001);
        } else {
            panic!("Expected Sinusoidal");
        }
    }

    #[test]
    fn test_stepped_config() {
        let toml_str = r#"
            type = "stepped"
            steps = [[30.0, 1000], [30.0, 5000], [30.0, 2000]]
            loop_forever = true
        "#;
        let config: SpeedProfileConfig = toml::from_str(toml_str).unwrap();
        if let SpeedProfileConfig::Stepped {
            steps,
            loop_forever,
        } = config
        {
            assert_eq!(steps.len(), 3);
            assert!(loop_forever);
        } else {
            panic!("Expected Stepped");
        }
    }

    #[test]
    fn test_burst_config() {
        let toml_str = r#"
            type = "burst"
            base = 1000
            burst_rate = 10000
            burst_duration_ms = 500
            burst_probability = 0.05
        "#;
        let config: SpeedProfileConfig = toml::from_str(toml_str).unwrap();
        if let SpeedProfileConfig::Burst {
            base,
            burst_rate,
            burst_duration_ms,
            burst_probability,
        } = config
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
    fn test_ramp_config() {
        let toml_str = r#"
            type = "ramp"
            start = 100
            end = 10000
            duration_secs = 300.0
        "#;
        let config: SpeedProfileConfig = toml::from_str(toml_str).unwrap();
        if let SpeedProfileConfig::Ramp {
            start,
            end,
            duration_secs,
        } = config
        {
            assert_eq!(start, 100);
            assert_eq!(end, 10000);
            assert!((duration_secs - 300.0).abs() < 0.001);
        } else {
            panic!("Expected Ramp");
        }
    }

    #[test]
    fn test_random_walk_config() {
        let toml_str = r#"
            type = "random_walk"
            base = 5000
            variance = 0.3
        "#;
        let config: SpeedProfileConfig = toml::from_str(toml_str).unwrap();
        if let SpeedProfileConfig::RandomWalk { base, variance } = config {
            assert_eq!(base, 5000);
            assert!((variance - 0.3).abs() < 0.001);
        } else {
            panic!("Expected RandomWalk");
        }
    }

    #[test]
    fn test_composite_config() {
        let toml_str = r#"
            type = "composite"
            combine_mode = "max"

            [[profiles]]
            type = "constant"
            rate = 1000

            [[profiles]]
            type = "constant"
            rate = 3000
        "#;
        let config: SpeedProfileConfig = toml::from_str(toml_str).unwrap();
        if let SpeedProfileConfig::Composite {
            profiles,
            combine_mode,
        } = config
        {
            assert_eq!(profiles.len(), 2);
            assert!(matches!(combine_mode, CombineModeConfig::Max));
        } else {
            panic!("Expected Composite");
        }
    }

    #[test]
    fn test_default_combine_mode() {
        let toml_str = r#"
            type = "composite"

            [[profiles]]
            type = "constant"
            rate = 1000
        "#;
        let config: SpeedProfileConfig = toml::from_str(toml_str).unwrap();
        if let SpeedProfileConfig::Composite { combine_mode, .. } = config {
            assert!(matches!(combine_mode, CombineModeConfig::Average));
        }
    }

    #[test]
    fn test_stepped_default_loop() {
        let toml_str = r#"
            type = "stepped"
            steps = [[10.0, 1000]]
        "#;
        let config: SpeedProfileConfig = toml::from_str(toml_str).unwrap();
        if let SpeedProfileConfig::Stepped { loop_forever, .. } = config {
            assert!(!loop_forever); // default is false
        }
    }

    #[test]
    fn test_serialize_roundtrip() {
        let config = SpeedProfileConfig::Sinusoidal {
            base: 5000,
            amplitude: 2000,
            period_secs: 60.0,
        };
        let toml_str = toml::to_string(&config).unwrap();
        let parsed: SpeedProfileConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(config, parsed);
    }
}
