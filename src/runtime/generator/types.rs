use super::speed::SpeedProfile;

#[derive(Clone, Debug)]
pub struct GenGRA {
    pub total_line: Option<usize>,
    /// 恒定速率（向后兼容）
    /// 当 speed_profile 为 None 时使用此字段
    pub gen_speed: usize,
    /// 动态速度模型（优先级高于 gen_speed）
    pub speed_profile: Option<SpeedProfile>,
    pub parallel: usize,
    pub stat_sec: usize,
    pub stat_print: bool,
    pub rescue: String,
}

impl Default for GenGRA {
    fn default() -> Self {
        Self {
            total_line: Some(1000),
            gen_speed: 1000,
            speed_profile: None,
            parallel: 1,
            stat_sec: 1,
            stat_print: false,
            rescue: "./rescue".to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SampleGRA {
    pub gen_conf: GenGRA,
}

#[derive(Clone, Debug, Default)]
pub struct RuleGRA {
    pub gen_conf: GenGRA,
}

impl GenGRA {
    /// 获取生成速率；若字段缺省返回默认值（与 Default 对齐）。
    /// 说明：用于在生成器直连路径上决定是否开启 backoff gate（gen_speed==0 视为无限速）。
    pub fn gen_conf_or_default_speed(&self) -> usize {
        self.gen_speed
    }

    /// 获取速度模型
    ///
    /// 如果设置了 speed_profile 则返回它，否则从 gen_speed 创建恒定速率模型
    pub fn get_speed_profile(&self) -> SpeedProfile {
        self.speed_profile
            .clone()
            .unwrap_or(SpeedProfile::Constant(self.gen_speed))
    }

    /// 获取基准速率（用于并行度计算等）
    pub fn base_speed(&self) -> usize {
        self.speed_profile
            .as_ref()
            .map(|p| p.base_rate())
            .unwrap_or(self.gen_speed)
    }

    /// 设置速度模型
    pub fn with_speed_profile(mut self, profile: SpeedProfile) -> Self {
        self.speed_profile = Some(profile);
        self
    }

    /// 使用恒定速率
    pub fn with_constant_speed(mut self, rate: usize) -> Self {
        self.gen_speed = rate;
        self.speed_profile = None;
        self
    }
}
