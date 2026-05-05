//! New wpgen configuration structure (generalized)
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::structure::{ConfStdOperation, SinkInstanceConf, Validate};
use crate::utils::{backup_clean, save_conf};
use orion_conf::error::{ConfIOReason, OrionConfResult};
use orion_error::conversion::ToStructError;
use orion_variate::EnvDict;
use serde_derive::{Deserialize, Serialize};
use toml;

use super::speed_profile::SpeedProfileConfig;
// no external IO traits for resolved wpgen; handled in loader

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct WpGenConfig {
    #[serde(default = "default_version")]
    pub version: String,
    pub generator: GeneratorConfig,
    pub output: OutputConfig,
    pub logging: LoggingConfig,
    #[serde(default)]
    pub presets: HashMap<String, String>,
}

fn default_version() -> String {
    "1.0".to_string()
}

impl Default for WpGenConfig {
    fn default() -> Self {
        Self {
            version: "1.0".to_string(),
            generator: GeneratorConfig::default(),
            output: OutputConfig::default(),
            logging: LoggingConfig::default(),
            presets: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
#[serde(deny_unknown_fields)]
pub struct GeneratorConfig {
    pub count: Option<usize>,
    /// 恒定速率（向后兼容）
    /// 当 speed_profile 为 None 时使用此字段
    pub speed: usize,
    /// 动态速度模型（优先级高于 speed）
    pub speed_profile: Option<SpeedProfileConfig>,
    pub parallel: usize,
    pub rule_root: Option<String>,
    pub sample_pattern: Option<String>,
}

impl Default for GeneratorConfig {
    fn default() -> Self {
        Self {
            count: Some(1000),
            speed: 1000,
            speed_profile: None,
            parallel: 1,
            rule_root: None,
            sample_pattern: None,
        }
    }
}

impl GeneratorConfig {
    /// 获取基准速率
    ///
    /// 如果设置了 speed_profile 则从中获取基准速率，否则使用 speed 字段
    pub fn base_speed(&self) -> usize {
        self.speed_profile
            .as_ref()
            .map(|p| p.base_rate())
            .unwrap_or(self.speed)
    }

    /// 获取速度配置
    ///
    /// 如果设置了 speed_profile 则返回它，否则从 speed 创建恒定速率配置
    pub fn get_speed_profile(&self) -> SpeedProfileConfig {
        self.speed_profile
            .clone()
            .unwrap_or(SpeedProfileConfig::Constant { rate: self.speed })
    }

    /// 是否使用恒定速率
    pub fn is_constant_speed(&self) -> bool {
        self.speed_profile
            .as_ref()
            .map(|p| p.is_constant())
            .unwrap_or(true)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OutputConfig {
    // 统一走 connectors：connect + params；
    // 仍保留 file/kafka/syslog/stdout 以兼容迁移（旧式到新式），但不鼓励直接使用。
    pub connect: Option<String>,
    #[serde(default)]
    pub params: toml::value::Table,
    pub name: Option<String>,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            connect: Some("file_json_sink".to_string()),
            params: toml::value::Table::new(),
            name: None,
        }
    }
}

// Removed OutputType/DataFormat/ErrorHandling: 由 connectors 决定输出类型与格式

// 兼容类型移除：File/Kafka/Syslog/Stdout 等旧式输出定义已废弃

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default = "default_log_output")]
    pub output: String,
    pub file_path: Option<String>,
    pub format: Option<String>,
    pub rotation: Option<String>,
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_output() -> String {
    "file".to_string()
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            output: "file".to_string(),
            file_path: Some("./data/logs".to_string()),
            format: None,
            rotation: None,
        }
    }
}

// MonitoringConfig 已移除：旧版 [monitoring] 顶层段将触发未知字段错误（deny_unknown_fields）。

impl Validate for WpGenConfig {
    fn validate(&self) -> OrionConfResult<()> {
        // 1. version 非空
        if self.version.trim().is_empty() {
            return Err(ConfIOReason::validation_error()
                .to_err()
                .with_detail("wpgen.version must not be empty"));
        }

        // 2. generator.count > 0（如果 Some），=0 无意义
        if let Some(count) = self.generator.count
            && count == 0
        {
            return Err(ConfIOReason::validation_error()
                .to_err()
                .with_detail("wpgen.generator.count must be greater than 0"));
        }

        // 3. generator.speed 不校验：0 表示 unlimited

        // 4. generator.parallel > 0
        if self.generator.parallel == 0 {
            return Err(ConfIOReason::validation_error()
                .to_err()
                .with_detail("wpgen.generator.parallel must be greater than 0"));
        }

        // 5. speed_profile 参数合法性
        if let Some(ref profile) = self.generator.speed_profile {
            self.validate_speed_profile(profile)?;
        }

        // 6. output.connect 必须显式指定，运行期不会回退到默认输出。
        match self.output.connect.as_deref().map(str::trim) {
            Some(connect) if !connect.is_empty() => {}
            _ => {
                return Err(ConfIOReason::validation_error()
                    .to_err()
                    .with_detail("wpgen.output.connect is required"));
            }
        }

        // 7. logging.level 是已知级别
        self.validate_logging_level()?;

        // 8. logging.output 是已知类型
        self.validate_logging_output()?;

        Ok(())
    }
}

impl WpGenConfig {
    fn validate_speed_profile(&self, profile: &SpeedProfileConfig) -> OrionConfResult<()> {
        match profile {
            SpeedProfileConfig::Constant { rate } => {
                if *rate == 0 {
                    return Err(ConfIOReason::validation_error()
                        .to_err()
                        .with_detail("speed_profile.constant.rate must be greater than 0"));
                }
            }
            SpeedProfileConfig::Sinusoidal {
                base, period_secs, ..
            } => {
                if *base == 0 {
                    return Err(ConfIOReason::validation_error()
                        .to_err()
                        .with_detail("speed_profile.sinusoidal.base must be greater than 0"));
                }
                if *period_secs <= 0.0 {
                    return Err(ConfIOReason::validation_error().to_err().with_detail(
                        "speed_profile.sinusoidal.period_secs must be greater than 0",
                    ));
                }
            }
            SpeedProfileConfig::Stepped { steps, .. } => {
                if steps.is_empty() {
                    return Err(ConfIOReason::validation_error()
                        .to_err()
                        .with_detail("speed_profile.stepped.steps must not be empty"));
                }
                for (i, (duration_secs, rate)) in steps.iter().enumerate() {
                    if *duration_secs <= 0.0 {
                        return Err(ConfIOReason::validation_error().to_err().with_detail(
                            format!(
                                "speed_profile.stepped.steps[{}].duration must be greater than 0",
                                i
                            ),
                        ));
                    }
                    if *rate == 0 {
                        return Err(ConfIOReason::validation_error().to_err().with_detail(
                            format!(
                                "speed_profile.stepped.steps[{}].rate must be greater than 0",
                                i
                            ),
                        ));
                    }
                }
            }
            SpeedProfileConfig::Burst {
                base,
                burst_rate,
                burst_duration_ms,
                burst_probability,
            } => {
                if *base == 0 {
                    return Err(ConfIOReason::validation_error()
                        .to_err()
                        .with_detail("speed_profile.burst.base must be greater than 0"));
                }
                if *burst_rate == 0 {
                    return Err(ConfIOReason::validation_error()
                        .to_err()
                        .with_detail("speed_profile.burst.burst_rate must be greater than 0"));
                }
                if *burst_duration_ms == 0 {
                    return Err(ConfIOReason::validation_error().to_err().with_detail(
                        "speed_profile.burst.burst_duration_ms must be greater than 0",
                    ));
                }
                if !(0.0..=1.0).contains(burst_probability) {
                    return Err(ConfIOReason::validation_error().to_err().with_detail(
                        "speed_profile.burst.burst_probability must be between 0.0 and 1.0",
                    ));
                }
            }
            SpeedProfileConfig::Ramp {
                start,
                end,
                duration_secs,
            } => {
                if *start == 0 {
                    return Err(ConfIOReason::validation_error()
                        .to_err()
                        .with_detail("speed_profile.ramp.start must be greater than 0"));
                }
                if *end == 0 {
                    return Err(ConfIOReason::validation_error()
                        .to_err()
                        .with_detail("speed_profile.ramp.end must be greater than 0"));
                }
                if *duration_secs <= 0.0 {
                    return Err(ConfIOReason::validation_error()
                        .to_err()
                        .with_detail("speed_profile.ramp.duration_secs must be greater than 0"));
                }
            }
            SpeedProfileConfig::RandomWalk { base, variance } => {
                if *base == 0 {
                    return Err(ConfIOReason::validation_error()
                        .to_err()
                        .with_detail("speed_profile.random_walk.base must be greater than 0"));
                }
                if !(0.0..=1.0).contains(variance) {
                    return Err(ConfIOReason::validation_error().to_err().with_detail(
                        "speed_profile.random_walk.variance must be between 0.0 and 1.0",
                    ));
                }
            }
            SpeedProfileConfig::Composite { profiles, .. } => {
                if profiles.is_empty() {
                    return Err(ConfIOReason::validation_error()
                        .to_err()
                        .with_detail("speed_profile.composite.profiles must not be empty"));
                }
                for sub in profiles {
                    self.validate_speed_profile(sub)?;
                }
            }
        }
        Ok(())
    }

    fn validate_logging_level(&self) -> OrionConfResult<()> {
        let level = self.logging.level.to_lowercase();
        if Self::is_valid_level_string(&level) {
            Ok(())
        } else {
            Err(ConfIOReason::validation_error()
                .to_err()
                .with_detail(format!(
                    "wpgen.logging.level '{}' is invalid; expected comma-separated levels like: trace, debug, info, warn, error, or module=level pairs",
                    self.logging.level
                )))
        }
    }

    fn is_valid_level_string(level: &str) -> bool {
        const VALID_LEVELS: &[&str] = &["trace", "debug", "info", "warn", "error"];
        level.split(',').all(|seg| {
            let seg = seg.trim();
            if seg.is_empty() {
                return false;
            }
            let level_part = if let Some((_mod, lvl)) = seg.split_once('=') {
                lvl.trim()
            } else {
                seg
            };
            VALID_LEVELS.contains(&level_part)
        })
    }

    fn validate_logging_output(&self) -> OrionConfResult<()> {
        let output = self.logging.output.to_lowercase();
        match output.as_str() {
            "stdout" | "console" | "file" | "both" => Ok(()),
            _ => Err(ConfIOReason::validation_error()
                .to_err()
                .with_detail(format!(
                    "wpgen.logging.output '{}' is invalid; expected one of: stdout, console, file, both",
                    self.logging.output
                ))),
        }
    }
}

/// 运行期解析后的 wpgen 配置：保留原始新格式，同时给出已解析的输出目标
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WpGenResolved {
    pub conf: WpGenConfig,
    pub out_sink: SinkInstanceConf,
}

// WpGenResolved is assembled by loader; no direct disk IO here

impl LoggingConfig {
    /// 将新格式 logging 映射为运行期使用的 wp_log::conf::LogConf
    pub fn to_log_conf(&self) -> wp_log::conf::LogConf {
        use wp_log::conf::{FileLogConf, Output};
        let output = match self.output.as_str() {
            "stdout" | "console" => Output::Console,
            "both" => Output::Both,
            _ => Output::File,
        };
        let file = match &self.file_path {
            Some(p) => Some(FileLogConf { path: p.clone() }),
            None => Some(FileLogConf {
                path: "./logs".to_string(),
            }),
        };
        wp_log::conf::LogConf {
            level: self.level.clone(),
            levels: None,
            output,
            file,
        }
        /*
        #[allow(clippy::field_reassign_with_default)]

        let mut lc = LogConf::default();
        lc.level = self.level.clone();
        lc.levels = None; // 统一用合成后的 level 字符串解析
        lc.output = output;
        lc.file = file;
        lc
        */
    }
}

use orion_conf::EnvTomlLoad;
impl WpGenConfig {
    /// Load WpGenConfig from a path with generic path parameter support
    pub fn load_from_path<P: AsRef<Path>>(path: P, dict: &EnvDict) -> OrionConfResult<Self> {
        let conf = Self::env_load_toml(path.as_ref(), dict)?;
        Validate::validate(&conf)?;
        Ok(conf)
    }

    /// Initialize WpGenConfig to a path with generic path parameter support
    pub fn init_to_path<P: AsRef<Path>>(path: P) -> OrionConfResult<Self> {
        let mut conf = Self::default();
        conf.output.connect = Some("file_json_sink".to_string());
        conf.output
            .params
            .insert("base".into(), "data/in_dat".into());
        conf.output.params.insert("file".into(), "gen.dat".into());
        conf.logging.file_path = Some("./data/logs".to_string());
        save_conf(Some(conf.clone()), path, true)?;
        Ok(conf)
    }

    /// Safe clean WpGenConfig at path with generic path parameter support
    pub fn safe_clean_at_path<P: AsRef<Path>>(path: P) -> OrionConfResult<()> {
        backup_clean(path)
    }
}

impl ConfStdOperation for WpGenConfig {
    fn load(path: &str, dict: &EnvDict) -> OrionConfResult<Self>
    where
        Self: Sized,
    {
        WpGenConfig::load_from_path(PathBuf::from(path), dict)
    }

    fn init(path: &str) -> OrionConfResult<Self>
    where
        Self: Sized,
    {
        let mut conf = WpGenConfig::default();
        conf.output.connect = Some("file_json_sink".to_string());
        conf.output
            .params
            .insert("base".into(), "data/in_dat".into());
        conf.output.params.insert("file".into(), "gen.dat".into());
        conf.logging.file_path = Some("./data/logs".to_string());
        save_conf(Some(conf.clone()), path, true)?;
        Ok(conf)
    }

    fn safe_clean(path: &str) -> OrionConfResult<()> {
        backup_clean(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orion_variate::ValueType;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn tmp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let mut p = std::env::temp_dir();
        p.push(format!("{}_{}", prefix, nanos));
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn to_log_conf_uses_plain_level() {
        let lg = LoggingConfig {
            level: "warn".into(),
            output: "file".into(),
            file_path: Some("./data/logs".into()),
            format: None,
            rotation: None,
        };
        let lc = lg.to_log_conf();
        assert_eq!(lc.level, "warn");
        assert_eq!(lc.file.as_ref().unwrap().path, "./data/logs");
    }

    #[test]
    fn wpgen_config_load_with_env_variables() {
        let base = tmp_dir("wpgen_env");
        let conf_path = base.join("wpgen.toml");

        // 创建包含环境变量的 wpgen 配置
        let wpgen_toml = r#"
version = "1.0"

[generator]
count = 100
speed = 1000
parallel = 2
rule_root = "${RULE_ROOT}/models"

[output]
connect = "file_${ENV}"
name = "${OUTPUT_NAME}"

[output.params]
base = "${DATA_ROOT}/out"
file = "${OUTPUT_FILE}"

[logging]
level = "${LOG_LEVEL}"
output = "file"
file_path = "${LOG_PATH}"
"#;
        fs::write(&conf_path, wpgen_toml).unwrap();

        // 设置环境变量字典
        let mut dict = EnvDict::new();
        dict.insert("RULE_ROOT", ValueType::from("/opt/app"));
        dict.insert("ENV", ValueType::from("prod"));
        dict.insert("OUTPUT_NAME", ValueType::from("gen_output"));
        dict.insert("DATA_ROOT", ValueType::from("/data"));
        dict.insert("OUTPUT_FILE", ValueType::from("result.dat"));
        dict.insert("LOG_LEVEL", ValueType::from("info"));
        dict.insert("LOG_PATH", ValueType::from("/var/log/wpgen"));

        // 加载配置
        let config = WpGenConfig::load_from_path(&conf_path, &dict).expect("load wpgen config");

        // 验证变量替换
        assert_eq!(
            config.generator.rule_root,
            Some("/opt/app/models".to_string())
        );
        assert_eq!(config.output.connect, Some("file_prod".to_string()));
        assert_eq!(config.output.name, Some("gen_output".to_string()));
        assert_eq!(
            config.output.params.get("base").and_then(|v| v.as_str()),
            Some("/data/out")
        );
        assert_eq!(
            config.output.params.get("file").and_then(|v| v.as_str()),
            Some("result.dat")
        );
        assert_eq!(config.logging.level, "info");
        assert_eq!(config.logging.file_path, Some("/var/log/wpgen".to_string()));
    }

    #[test]
    fn wpgen_config_load_without_env_keeps_literal() {
        let base = tmp_dir("wpgen_literal");
        let conf_path = base.join("wpgen.toml");

        // 创建不含环境变量的配置
        let wpgen_toml = r#"
version = "1.0"

[generator]
count = 50
speed = 500

[output]
connect = "file_sink"

[logging]
level = "debug"
output = "stdout"
"#;
        fs::write(&conf_path, wpgen_toml).unwrap();

        let dict = EnvDict::new();
        let config = WpGenConfig::load_from_path(&conf_path, &dict).expect("load wpgen config");

        assert_eq!(config.generator.count, Some(50));
        assert_eq!(config.generator.speed, 500);
        assert_eq!(config.output.connect, Some("file_sink".to_string()));
        assert_eq!(config.logging.level, "debug");
    }

    #[test]
    fn wpgen_config_with_partial_env_substitution() {
        let base = tmp_dir("wpgen_partial");
        let conf_path = base.join("wpgen.toml");

        // 配置中混合使用变量和字面值
        let wpgen_toml = r#"
version = "1.0"

[generator]
count = 100
rule_root = "${APP_ROOT}/rules"

[output]
connect = "tcp_sink"

[logging]
level = "warn"
output = "file"
file_path = "${LOG_DIR}/app.log"
"#;
        fs::write(&conf_path, wpgen_toml).unwrap();

        let mut dict = EnvDict::new();
        dict.insert("APP_ROOT", ValueType::from("/home/app"));
        dict.insert("LOG_DIR", ValueType::from("/var/log"));

        let config = WpGenConfig::load_from_path(&conf_path, &dict).expect("load wpgen config");

        assert_eq!(
            config.generator.rule_root,
            Some("/home/app/rules".to_string())
        );
        assert_eq!(config.output.connect, Some("tcp_sink".to_string()));
        assert_eq!(
            config.logging.file_path,
            Some("/var/log/app.log".to_string())
        );
    }

    #[test]
    fn wpgen_config_with_speed_profile_constant() {
        let base = tmp_dir("wpgen_speed_const");
        let conf_path = base.join("wpgen.toml");

        let wpgen_toml = r#"
version = "1.0"

[generator]
count = 100

[generator.speed_profile]
type = "constant"
rate = 5000

[output]
connect = "file_sink"

[logging]
level = "info"
output = "file"
"#;
        fs::write(&conf_path, wpgen_toml).unwrap();

        let dict = EnvDict::new();
        let config = WpGenConfig::load_from_path(&conf_path, &dict).expect("load wpgen config");

        assert!(config.generator.speed_profile.is_some());
        assert_eq!(config.generator.base_speed(), 5000);
        let profile = config.generator.speed_profile.unwrap();
        assert!(matches!(
            profile,
            SpeedProfileConfig::Constant { rate: 5000 }
        ));
    }

    #[test]
    fn wpgen_config_with_speed_profile_sinusoidal() {
        let base = tmp_dir("wpgen_speed_sin");
        let conf_path = base.join("wpgen.toml");

        let wpgen_toml = r#"
version = "1.0"

[generator]
count = 100

[generator.speed_profile]
type = "sinusoidal"
base = 5000
amplitude = 2000
period_secs = 60.0

[output]
connect = "file_sink"

[logging]
level = "info"
output = "file"
"#;
        fs::write(&conf_path, wpgen_toml).unwrap();

        let dict = EnvDict::new();
        let config = WpGenConfig::load_from_path(&conf_path, &dict).expect("load wpgen config");

        assert!(config.generator.speed_profile.is_some());
        let profile = config.generator.speed_profile.unwrap();
        if let SpeedProfileConfig::Sinusoidal {
            base,
            amplitude,
            period_secs,
        } = profile
        {
            assert_eq!(base, 5000);
            assert_eq!(amplitude, 2000);
            assert!((period_secs - 60.0).abs() < 0.001);
        } else {
            panic!("Expected Sinusoidal profile");
        }
    }

    #[test]
    fn wpgen_config_with_speed_profile_ramp() {
        let base = tmp_dir("wpgen_speed_ramp");
        let conf_path = base.join("wpgen.toml");

        let wpgen_toml = r#"
version = "1.0"

[generator]
count = 100

[generator.speed_profile]
type = "ramp"
start = 100
end = 10000
duration_secs = 300.0

[output]
connect = "file_sink"

[logging]
level = "info"
output = "file"
"#;
        fs::write(&conf_path, wpgen_toml).unwrap();

        let dict = EnvDict::new();
        let config = WpGenConfig::load_from_path(&conf_path, &dict).expect("load wpgen config");

        assert!(config.generator.speed_profile.is_some());
        let profile = config.generator.speed_profile.unwrap();
        if let SpeedProfileConfig::Ramp {
            start,
            end,
            duration_secs,
        } = profile
        {
            assert_eq!(start, 100);
            assert_eq!(end, 10000);
            assert!((duration_secs - 300.0).abs() < 0.001);
        } else {
            panic!("Expected Ramp profile");
        }
    }

    #[test]
    fn wpgen_config_with_speed_profile_stepped() {
        let base = tmp_dir("wpgen_speed_step");
        let conf_path = base.join("wpgen.toml");

        let wpgen_toml = r#"
version = "1.0"

[generator]
count = 100

[generator.speed_profile]
type = "stepped"
steps = [[30.0, 1000], [30.0, 5000], [30.0, 2000]]
loop_forever = true

[output]
connect = "file_sink"

[logging]
level = "info"
output = "file"
"#;
        fs::write(&conf_path, wpgen_toml).unwrap();

        let dict = EnvDict::new();
        let config = WpGenConfig::load_from_path(&conf_path, &dict).expect("load wpgen config");

        assert!(config.generator.speed_profile.is_some());
        let profile = config.generator.speed_profile.unwrap();
        if let SpeedProfileConfig::Stepped {
            steps,
            loop_forever,
        } = profile
        {
            assert_eq!(steps.len(), 3);
            assert!(loop_forever);
        } else {
            panic!("Expected Stepped profile");
        }
    }

    #[test]
    fn wpgen_config_without_speed_profile_uses_speed() {
        let base = tmp_dir("wpgen_speed_default");
        let conf_path = base.join("wpgen.toml");

        let wpgen_toml = r#"
version = "1.0"

[generator]
count = 100
speed = 3000

[output]
connect = "file_sink"

[logging]
level = "info"
output = "file"
"#;
        fs::write(&conf_path, wpgen_toml).unwrap();

        let dict = EnvDict::new();
        let config = WpGenConfig::load_from_path(&conf_path, &dict).expect("load wpgen config");

        assert!(config.generator.speed_profile.is_none());
        assert_eq!(config.generator.speed, 3000);
        assert_eq!(config.generator.base_speed(), 3000);
        assert!(config.generator.is_constant_speed());

        // get_speed_profile should create constant from speed
        let profile = config.generator.get_speed_profile();
        assert!(matches!(
            profile,
            SpeedProfileConfig::Constant { rate: 3000 }
        ));
    }

    #[test]
    fn wpgen_loaders_reject_missing_output_connect_consistently() {
        let base = tmp_dir("wpgen_missing_connect");
        let conf_path = base.join("wpgen.toml");

        let wpgen_toml = r#"
version = "1.0"

[generator]
count = 100

[output]

[logging]
level = "info"
output = "file"
"#;
        fs::write(&conf_path, wpgen_toml).unwrap();

        let dict = EnvDict::new();
        let path = conf_path.to_string_lossy().to_string();
        let direct = WpGenConfig::load_from_path(&conf_path, &dict);
        let trait_load = <WpGenConfig as ConfStdOperation>::load(&path, &dict);

        assert!(
            direct
                .unwrap_err()
                .to_string()
                .contains("wpgen.output.connect is required")
        );
        assert!(
            trait_load
                .unwrap_err()
                .to_string()
                .contains("wpgen.output.connect is required")
        );
    }
}
