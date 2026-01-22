use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, derive_getters::Getters, Default)]
pub struct SinkExpectOverride {
    /// 目标占比（0..=1）
    #[serde(default)]
    pub ratio: Option<f64>,
    /// 允许偏差（0..=1），与 ratio 同时出现时生效
    #[serde(default)]
    pub tol: Option<f64>,
    /// 最小占比（0..=1）
    #[serde(default)]
    pub min: Option<f64>,
    /// 最大占比（0..=1）
    #[serde(default)]
    pub max: Option<f64>,
}

impl SinkExpectOverride {
    pub fn validate(&self) -> crate::types::AnyResult<()> {
        use anyhow::bail;
        let in_min_max = |v: f64| v.is_finite() && (0.0..=1000.0).contains(&v);
        if let Some(r) = self.ratio
            && !in_min_max(r)
        {
            bail!("ratio must be in [0,1], got {}", r);
        }
        if let Some(t) = self.tol
            && !(t >= 0.0 && t.is_finite())
        {
            bail!("tol must be >= 0, got {}", t);
        }
        if let Some(mn) = self.min
            && !in_min_max(mn)
        {
            bail!("min must be in [0,1000], got {}", mn);
        }
        if let Some(mx) = self.max
            && !in_min_max(mx)
        {
            bail!("max must be in [0,1000], got {}", mx);
        }
        if let (Some(mn), Some(mx)) = (self.min, self.max)
            && mn > mx
        {
            bail!("min must be <= max ({} > {})", mn, mx);
        }
        // 互斥：ratio/tol 与 min/max 不建议同时出现
        let has_rt = self.ratio.is_some() || self.tol.is_some();
        let has_mm = self.min.is_some() || self.max.is_some();
        if has_rt && has_mm {
            bail!("expect: ratio/tol cannot be combined with min/max; choose one style");
        }
        Ok(())
    }
}
