use std::path::{Path, PathBuf};

use orion_conf::EnvTomlLoad;
use orion_conf::TomlIO;
use orion_conf::error::{ConfIOReason, OrionConfResult};
use orion_error::{ErrorOwe, ErrorWith, ToStructError, UvsValidationFrom};
use orion_variate::EnvDict;
use serde_json::json;
use wp_conf::connectors::{ParamMap, param_value_from_toml};
use wp_conf::sinks::ConnectorRec;
use wp_conf::sinks::load_connectors_for;
use wp_conf::structure::SinkInstanceConf;
use wp_model_core::model::fmt_def::TextFmt;

use super::WarpConf;
use crate::orchestrator::config::WPGEN_TOML;
use crate::orchestrator::config::models::wpgen::{WpGenConfig, WpGenResolved};
use crate::types::AnyResult;
use wp_conf::engine::EngineConfig;

impl WarpConf {
    /// 加载已解析的 wpgen 配置，包含 connector 解析
    pub fn load_wpgen_config(
        &self,
        file_name: &str,
        dict: &EnvDict,
    ) -> OrionConfResult<WpGenResolved> {
        let conf = self.parse_wpgen_config(file_name, dict)?;
        let out_sink = self.resolve_out_sink(&conf, dict)?;
        Ok(WpGenResolved { conf, out_sink })
    }

    // 1) 解析 wpgen.toml 为 WpGenConfig 并做基本验证
    fn parse_wpgen_config(&self, file_name: &str, dict: &EnvDict) -> OrionConfResult<WpGenConfig> {
        let path = self.config_path_string(file_name);
        let conf = WpGenConfig::env_load_toml(&PathBuf::from(path.as_str()), dict)?;
        conf.validate()?;
        Ok(conf)
    }

    // 2) 根据是否指定 connect 选择默认文件输出或按 connectors 装配 out_sink
    fn resolve_out_sink(
        &self,
        conf: &WpGenConfig,
        dict: &EnvDict,
    ) -> OrionConfResult<SinkInstanceConf> {
        // 统一 name 缺省（仅用于展示）；connect 必须显式指定（不提供默认回退）
        let out_name = conf
            .output
            .name
            .clone()
            .unwrap_or_else(|| "gen_out".to_string());
        let conn_id = match conf.output.connect.clone() {
            Some(cnn) => cnn,
            None => {
                return ConfIOReason::from_validation(
                    "wpgen.output.connect must be set (no default fallback)",
                )
                .err_result();
            }
        };
        let (_start_root, conn) = self.load_connector_by_id(&conn_id, dict)?;
        let mut merged = Self::merge_params_with_whitelist(&conn, &conf.output.params, &conn_id)?;
        // 自动开启：当生成速率无限制（speed==0）且连接器类型为 tcp，且未显式设置 max_backoff/sendq_backoff/sendq_backpressure
        if conn.kind == "tcp" {
            let unlimited = conf.generator.speed == 0;
            let has_explicit = merged.contains_key("max_backoff");
            if unlimited {
                if !has_explicit {
                    merged.insert("max_backoff".into(), json!(true));
                }
            } else {
                // 限速场景强制关闭：即使用户显式设置也置为 false，保证 maybe_backoff 仅在无限速时启用
                if has_explicit {
                    merged.insert("max_backoff".into(), json!(false));
                }
            }
        }
        let fmt = Self::select_text_fmt(conn.kind.as_str(), &merged);
        let mut out = SinkInstanceConf::new_type(out_name, fmt, conn.kind.clone(), merged, None);
        out.connector_id = Some(conn_id);
        Ok(out)
    }

    // 2.1) 装载 connectors 并按 id 获取（带错误上下文）
    fn load_connector_by_id(
        &self,
        conn_id: &str,
        dict: &EnvDict,
    ) -> OrionConfResult<(String, ConnectorRec)> {
        let wp_conf = EngineConfig::load_or_init(self.work_root(), dict)
            .owe_res()
            .with("load_or_init")?
            .conf_absolutize(self.work_root());
        let configured_root = wp_conf.sinks_root().to_string();
        let configured_path = Path::new(&configured_root);
        let resolved_root = if configured_path.is_absolute() {
            configured_path.to_path_buf()
        } else {
            self.work_root().join(configured_path)
        };
        let start_root = resolved_root.to_string_lossy().to_string();
        let connectors = load_connectors_for(&start_root, dict)?;
        let conn = connectors.get(conn_id).cloned().ok_or_else(|| {
            let mut known: Vec<String> = connectors.keys().cloned().collect();
            known.sort();
            if known.len() > 8 {
                known.truncate(8);
            }
            ConfIOReason::from_validation(format!(
                "wpgen.output.connect='{}' 不存在：自 start='{}' 向上最多 32 层搜索 'connectors/sink.d' 未找到该 id；已知 id: [{}]",
                conn_id,
                start_root,
                known.join(", ")
            ))
        })?;
        Ok((start_root, conn))
    }

    // 2.2) 合并 output.params 到 connector.default_params，并校验 allow_override 白名单和误用嵌套表
    fn merge_params_with_whitelist(
        conn: &wp_conf::sinks::ConnectorRec,
        override_tbl: &toml::value::Table,
        conn_id: &str,
    ) -> OrionConfResult<ParamMap> {
        let mut merged = conn.default_params.clone();
        for (k, v) in override_tbl.iter() {
            if k == "params" || k == "params_override" {
                return ConfIOReason::from_validation(format!(
                    "invalid nested table '{}' in output.params; set keys [{}] directly",
                    k,
                    conn.allow_override.join(", ")
                ))
                .err_result();
            }
            if !conn.allow_override.iter().any(|x| x == k) {
                return ConfIOReason::from_validation(format!(
                    "override '{}' not allowed for connector '{}'; whitelist: [{}]",
                    k,
                    conn_id,
                    conn.allow_override.join(", ")
                ))
                .err_result();
            }
            merged.insert(k.clone(), param_value_from_toml(v));
        }
        Ok(merged)
    }

    // 2.3) 选择输出格式：文件类遵循 params.fmt，其它统一 Json
    fn select_text_fmt(kind: &str, merged: &ParamMap) -> TextFmt {
        if kind == "file" {
            let s = merged.get("fmt").and_then(|v| v.as_str()).unwrap_or("json");
            TextFmt::from(s)
        } else {
            TextFmt::Json
        }
    }
}
