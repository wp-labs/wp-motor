use crate::traits::Component;
use crate::utils::error_conv::ResultExt;
use wp_error::run_error::RunResult;

// 重新导出 wp-cli-core 的类型，保持向后兼容
pub use wp_cli_core::knowdb::{CheckReport, CleanReport, TableCheck};

/// 知识库管理组件
///
/// 提供知识库的初始化、检查和清理功能。
/// 实现委托给 wp-cli-core::knowdb。
#[derive(Debug, Clone, Default)]
pub struct Knowledge;

impl Knowledge {
    /// 创建新的知识库管理实例
    pub fn new() -> Self {
        Self
    }

    /// 初始化知识库
    ///
    /// 在指定的工作根目录下创建知识库结构，包括：
    /// - models/knowledge/ 目录
    /// - knowdb.toml 配置文件
    /// - example/ 示例表目录及文件
    ///
    /// # 参数
    /// - `work_root`: 项目根目录
    ///
    /// # 示例
    /// ```no_run
    /// use wp_proj::models::Knowledge;
    ///
    /// let kb = Knowledge::new();
    /// kb.init("./my-project")?;
    /// # Ok::<(), wp_error::run_error::RunError>(())
    /// ```
    pub fn init(&self, work_root: &str) -> RunResult<()> {
        wp_cli_core::knowdb::init(work_root, false).to_run_err("知识库初始化失败")
    }

    /// 检查知识库状态
    ///
    /// 验证知识库配置文件和表文件的完整性。
    ///
    /// # 参数
    /// - `work_root`: 项目根目录
    /// - `dict`: 环境变量字典
    ///
    /// # 返回
    /// 返回检查报告，包含：
    /// - total: 总表数
    /// - ok: 通过检查的表数
    /// - fail: 未通过检查的表数
    /// - tables: 每个表的详细检查结果
    pub fn check(&self, work_root: &str, dict: &orion_variate::EnvDict) -> RunResult<CheckReport> {
        wp_cli_core::knowdb::check(work_root, dict).to_run_err("知识库检查失败")
    }

    /// 清理知识库数据
    ///
    /// 删除 models/knowledge/ 目录和 .run/authority.sqlite 缓存文件。
    ///
    /// # 参数
    /// - `work_root`: 项目根目录
    ///
    /// # 返回
    /// 返回清理报告，包含：
    /// - removed_models_dir: 是否删除了 models 目录
    /// - removed_authority_cache: 是否删除了权威缓存
    /// - not_found_models: models 目录是否不存在
    pub fn clean(&self, work_root: &str) -> RunResult<CleanReport> {
        wp_cli_core::knowdb::clean(work_root).to_run_err("知识库清理失败")
    }
}

impl Component for Knowledge {
    fn component_name(&self) -> &'static str {
        "Knowledge"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orion_variate::EnvDict;
    use tempfile::tempdir;
    use wp_conf::test_support::ForTest;

    #[test]
    fn knowledge_component_name() {
        let kb = Knowledge::new();
        assert_eq!(kb.component_name(), "Knowledge");
    }

    #[test]
    fn knowledge_init_creates_structure() {
        let temp = tempdir().unwrap();
        let kb = Knowledge::new();

        let result = kb.init(temp.path().to_str().unwrap());
        assert!(result.is_ok(), "初始化应该成功");

        // 验证目录和文件已创建
        let models_dir = temp.path().join("models/knowledge");
        assert!(models_dir.exists(), "models/knowledge 目录应该存在");
        assert!(
            models_dir.join("knowdb.toml").exists(),
            "knowdb.toml 应该存在"
        );
        assert!(models_dir.join("example").exists(), "example 目录应该存在");
    }

    #[test]
    fn knowledge_check_reports_status() {
        let temp = tempdir().unwrap();
        let kb = Knowledge::new();

        // 先初始化
        kb.init(temp.path().to_str().unwrap()).unwrap();

        // 再检查
        let report = kb
            .check(temp.path().to_str().unwrap(), &EnvDict::test_default())
            .unwrap();
        assert!(report.total > 0, "应该有至少一个表");
        assert_eq!(report.ok, report.total, "所有表应该通过检查");
    }

    #[test]
    fn knowledge_clean_removes_files() {
        let temp = tempdir().unwrap();
        let kb = Knowledge::new();

        // 先初始化
        kb.init(temp.path().to_str().unwrap()).unwrap();

        // 验证文件存在
        let models_dir = temp.path().join("models/knowledge");
        assert!(models_dir.exists());

        // 清理
        let report = kb.clean(temp.path().to_str().unwrap()).unwrap();
        assert!(report.removed_models_dir, "应该删除 models 目录");

        // 验证文件已删除
        assert!(!models_dir.exists(), "models 目录应该被删除");
    }

    #[test]
    fn knowledge_error_conversion_works() {
        let kb = Knowledge::new();

        // 使用无效路径触发错误
        let result = kb.check(
            "/nonexistent/path/that/does/not/exist",
            &EnvDict::test_default(),
        );
        assert!(result.is_err(), "应该返回 RunResult 错误");

        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("知识库检查失败"), "错误消息应该包含上下文");
    }
}
