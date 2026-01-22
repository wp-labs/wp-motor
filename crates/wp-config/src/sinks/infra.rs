#![allow(dead_code)]
use crate::structure::{FixedGroup, FlexGroup};
use anyhow::Result as AnyResult;
use orion_error::ToStructError;
use orion_variate::{EnvDict, EnvEvaluable};
use wp_error::config_error::ConfResult;

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub struct InfraSinkConf {
    #[serde(default = "FixedGroup::default_ins")]
    pub default: FixedGroup,
    #[serde(default = "FixedGroup::miss_ins")]
    pub miss: FixedGroup,
    #[serde(default = "FixedGroup::residue_ins")]
    pub residue: FixedGroup,
    #[serde(default = "FlexGroup::monitor_ins")]
    pub monitor: FlexGroup,
    #[serde(default = "FixedGroup::error_ins")]
    pub error: FixedGroup,
}

impl EnvEvaluable<InfraSinkConf> for InfraSinkConf {
    fn env_eval(mut self, dict: &orion_variate::EnvDict) -> Self {
        self.default = self.default.env_eval(dict);
        self.miss = self.miss.env_eval(dict);
        self.residue = self.residue.env_eval(dict);
        self.monitor = self.monitor.env_eval(dict);
        self.error = self.error.env_eval(dict);
        self
    }
}
impl InfraSinkConf {
    pub fn clean_local_file(&self) -> AnyResult<()> {
        for i in self.miss.sinks() {
            i.clean_sink_file()?;
        }
        for i in self.monitor.sinks() {
            i.clean_sink_file()?;
        }
        for i in self.default.sinks() {
            i.clean_sink_file()?;
        }
        for i in self.error.sinks() {
            i.clean_sink_file()?;
        }
        for i in self.residue.sinks() {
            i.clean_sink_file()?;
        }
        Ok(())
    }
    pub fn load(path: &str, dict: &EnvDict) -> ConfResult<Self> {
        // Prefer sink/infra.d under sink_root
        let infra_d = std::path::Path::new(path).join("infra.d");
        let business_d = std::path::Path::new(path).join("business.d");
        if infra_d.exists() || business_d.exists() {
            // 使用配置层统一输出：将 SinkRouteConf 映射为不同的 Infra 组
            let confs = crate::sinks::load_infra_route_confs(path, dict).map_err(|e| {
                wp_error::config_error::ConfError::from(wp_error::config_error::ConfReason::Syntax(
                    e.to_string(),
                ))
            })?;
            let mut conf = InfraSinkConf::default();
            for c in confs {
                let g = c.sink_group; // FlexiGroupConf
                match g.name().as_str() {
                    crate::sinks::GROUP_DEFAULT => {
                        conf.default = FixedGroup {
                            name: g.name().to_string(),
                            expect: g.expect.clone(),
                            sinks: g.sinks.clone(),
                            parallel: g.parallel_cnt(),
                        }
                    }
                    crate::sinks::GROUP_MISS => {
                        conf.miss = FixedGroup {
                            name: g.name().to_string(),
                            expect: g.expect.clone(),
                            sinks: g.sinks.clone(),
                            parallel: g.parallel_cnt(),
                        }
                    }
                    crate::sinks::GROUP_RESIDUE => {
                        conf.residue = FixedGroup {
                            name: g.name().to_string(),
                            expect: g.expect.clone(),
                            sinks: g.sinks.clone(),
                            parallel: g.parallel_cnt(),
                        }
                    }
                    crate::sinks::GROUP_ERROR => {
                        conf.error = FixedGroup {
                            name: g.name().to_string(),
                            expect: g.expect.clone(),
                            sinks: g.sinks.clone(),
                            parallel: g.parallel_cnt(),
                        }
                    }
                    crate::sinks::GROUP_MONITOR => {
                        conf.monitor = g;
                    }
                    _ => {
                        // 忽略业务组
                    }
                }
            }
            return Ok(conf);
        }
        // 不再支持 legacy framework.toml；缺失目录即返回错误（配置错误 NotFound）
        Err(wp_error::config_error::ConfReason::NotFound(format!(
            "infra routes not found under '{}': expected infra.d/ or business.d/",
            path
        ))
        .to_err())
    }
}
impl Default for InfraSinkConf {
    fn default() -> Self {
        Self {
            default: FixedGroup::default_ins(),
            miss: FixedGroup::miss_ins(),
            residue: FixedGroup::residue_ins(),
            monitor: FlexGroup::monitor_ins(),
            error: FixedGroup::error_ins(),
        }
    }
}
#[cfg(test)]
mod tests {
    use anyhow::Result as AnyResult;
    use std::fs::remove_file;
    use std::path::Path;

    use crate::utils::save_data;

    #[test]
    fn test_infra_conf() -> AnyResult<()> {
        let conf_path = "./temp/framework.toml";
        if Path::new(conf_path).exists() {
            remove_file(conf_path)?;
        }
        // Write a minimal, known-good framework template
        let s = r#"[defaults]
[defaults.expect]
basis = "group_input"
min_samples = 100
mode = "warn"

[default]
name = "default"

[[default.sinks]]
name = "default_sink"
fmt = "proto-text"
target = "file"
path = "./out/default.dat"

[miss]
name = "miss"

[[miss.sinks]]
name = "miss_sink"
fmt = "raw"
target = "file"
path = "./out/miss.dat"

[residue]
name = "residue"

[[residue.sinks]]
name = "residue_sink"
fmt = "raw"
target = "file"
path = "./out/residue.dat"

[monitor]
name = "monitor"

[[monitor.sinks]]
name = "monitor_sink"
fmt = "proto-text"
target = "file"
path = "./out/monitor.dat"

[error]
name = "error"

[[error.sinks]]
name = "err_sink"
fmt = "raw"
target = "file"
path = "./out/error.dat"
"#;
        save_data(Some(s.to_string()), conf_path, false)?;
        Ok(())
    }
}
