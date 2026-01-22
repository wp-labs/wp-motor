//! 环境变量替换验证模块
//!
//! 提供通用的未替换变量检测功能，用于在配置加载后验证所有变量是否已被正确替换。

use orion_conf::error::{ConfIOReason, OrionConfResult};
use orion_error::{ToStructError, UvsValidationFrom};
use serde::Serialize;

/// 检测序列化后的配置中是否存在未替换的变量
///
/// 此函数将配置序列化为JSON，然后在JSON字符串中搜索 `${...}` 模式。
/// 如果发现未替换的变量，返回错误。
///
/// # 参数
/// - `config`: 任何实现了 `Serialize` 的配置对象
/// - `config_name`: 配置文件名或描述（用于错误消息）
///
/// # 返回
/// - `Ok(())`: 所有变量都已替换
/// - `Err`: 发现未替换的变量
///
/// # 示例
/// ```no_run
/// use wp_conf::loader::validate_vars::check_unresolved_variables;
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Config {
///     file: String,
/// }
///
/// let config = Config {
///     file: "${UNRESOLVED_VAR}".to_string(),
/// };
///
/// // 这将返回错误，因为变量未被替换
/// assert!(check_unresolved_variables(&config, "test.toml").is_err());
/// ```
pub fn check_unresolved_variables<T: Serialize>(
    config: &T,
    config_name: &str,
) -> OrionConfResult<()> {
    // 将配置序列化为JSON以便检查所有字段
    let json_str = serde_json::to_string(config).map_err(|e| {
        ConfIOReason::from_validation(format!("Failed to serialize config for validation: {}", e))
            .to_err()
    })?;

    // 查找未替换的变量
    if let Some(unresolved) = find_first_unresolved_var(&json_str) {
        return Err(ConfIOReason::from_validation(format!(
            "Unresolved variable '{}' found in {}. \
            Please define this variable in .warp_parse/sec_key.toml or environment. \
            Hint: For security-sensitive values, use SEC_ prefix (e.g., SEC_SINK_FILE_1).",
            unresolved, config_name
        ))
        .to_err());
    }

    Ok(())
}

/// 在字符串中查找第一个未替换的变量
///
/// 返回格式为 `${VAR_NAME}` 的字符串
fn find_first_unresolved_var(s: &str) -> Option<String> {
    if let Some(start) = s.find("${")
        && let Some(end) = s[start..].find('}')
    {
        let var_with_braces = &s[start..start + end + 1];
        return Some(var_with_braces.to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Serialize)]
    struct TestConfig {
        name: String,
        path: String,
    }

    #[test]
    fn detects_unresolved_variable() {
        let config = TestConfig {
            name: "test".to_string(),
            path: "${SEC_FILE_PATH}".to_string(),
        };

        let result = check_unresolved_variables(&config, "test.toml");
        assert!(result.is_err());

        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("${SEC_FILE_PATH}"));
        assert!(err_msg.contains("test.toml"));
    }

    #[test]
    fn passes_with_resolved_variables() {
        let config = TestConfig {
            name: "test".to_string(),
            path: "/path/to/file.dat".to_string(),
        };

        let result = check_unresolved_variables(&config, "test.toml");
        assert!(result.is_ok());
    }

    #[test]
    fn detects_variable_in_nested_field() {
        #[derive(Serialize)]
        struct NestedConfig {
            outer: TestConfig,
        }

        let config = NestedConfig {
            outer: TestConfig {
                name: "test".to_string(),
                path: "${NESTED_VAR}".to_string(),
            },
        };

        let result = check_unresolved_variables(&config, "nested.toml");
        assert!(result.is_err());
    }

    #[test]
    fn find_first_unresolved_var_works() {
        assert_eq!(
            find_first_unresolved_var("some ${VAR} text"),
            Some("${VAR}".to_string())
        );
        assert_eq!(
            find_first_unresolved_var("${VAR1} and ${VAR2}"),
            Some("${VAR1}".to_string())
        );
        assert_eq!(find_first_unresolved_var("no variables here"), None);
        assert_eq!(find_first_unresolved_var("${INCOMPLETE"), None);
    }
}
