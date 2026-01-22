///! 错误转换辅助模块
///!
///! 提供便捷的方法将 wp-cli-core 和 wp-config 的错误类型转换为 RunResult。
///!
///! # 设计原则
///!
///! - **wp-cli-core**: 使用 `anyhow::Result` (业务逻辑层)
///! - **wp-config**: 使用 `OrionConfResult` (配置层)
///! - **wp-proj**: 使用 `wp_error::RunResult` (应用层)
///! - 错误转换在边界层（wp-proj）进行
///!
///! # 使用示例
///!
///! ```no_run
///! use wp_proj::utils::error_conv::ResultExt;
///! use wp_error::run_error::RunResult;
///!
///! fn my_function() -> RunResult<()> {
///!     // 从 anyhow::Result 转换
///!     some_anyhow_function()
///!         .to_run_err("操作失败")?;
///!
///!     // 从 OrionConfResult 转换
///!     some_config_function()
///!         .to_run_err("配置加载失败")?;
///!
///!     Ok(())
///! }
///! # fn some_anyhow_function() -> anyhow::Result<()> { Ok(()) }
///! # fn some_config_function() -> Result<(), orion_error::StructError<orion_conf::error::ConfIOReason>> { Ok(()) }
///! ```
use orion_error::{ToStructError, UvsConfFrom};
use wp_error::run_error::{RunReason, RunResult};

/// Result 扩展 trait，提供统一的错误转换接口
///
/// 可以处理任何实现了 `std::fmt::Display` 的错误类型。
pub trait ResultExt<T, E> {
    /// 将 Result 转换为 RunResult，添加上下文信息
    ///
    /// # 示例
    /// ```no_run
    /// # use wp_proj::utils::error_conv::ResultExt;
    /// # use wp_error::run_error::RunResult;
    /// fn load_file(path: &str) -> RunResult<String> {
    ///     std::fs::read_to_string(path)
    ///         .to_run_err("读取文件失败")
    /// }
    /// ```
    fn to_run_err(self, context: &str) -> RunResult<T>;

    /// 将 Result 转换为 RunResult，使用闭包生成上下文（延迟求值）
    ///
    /// 当上下文信息的构建成本较高时使用，只有在错误发生时才会调用闭包。
    /// 闭包接收错误对象的引用作为参数。
    ///
    /// # 示例
    /// ```no_run
    /// # use wp_proj::utils::error_conv::ResultExt;
    /// # use wp_error::run_error::RunResult;
    /// fn process_item(id: i32) -> RunResult<()> {
    ///     expensive_operation(id)
    ///         .to_run_err_with(|e| format!("处理项目 {} 失败: {}", id, e))
    /// }
    /// # fn expensive_operation(id: i32) -> Result<(), std::io::Error> { Ok(()) }
    /// ```
    fn to_run_err_with<F>(self, f: F) -> RunResult<T>
    where
        F: FnOnce(&E) -> String;
}

/// 为所有 Result<T, E> 实现 ResultExt，其中 E 实现了 Display
impl<T, E: std::fmt::Display> ResultExt<T, E> for Result<T, E> {
    fn to_run_err(self, context: &str) -> RunResult<T> {
        self.map_err(|e| RunReason::from_conf(format!("{}: {}", context, e)).to_err())
    }

    fn to_run_err_with<F>(self, f: F) -> RunResult<T>
    where
        F: FnOnce(&E) -> String,
    {
        self.map_err(|e| {
            let msg = f(&e);
            RunReason::from_conf(msg).to_err()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn result_ext_converts_io_error() {
        let result: Result<(), io::Error> =
            Err(io::Error::new(io::ErrorKind::NotFound, "file not found"));

        let run_result: RunResult<()> = result.to_run_err("读取文件失败");

        assert!(run_result.is_err());
        let err_msg = run_result.unwrap_err().to_string();
        assert!(err_msg.contains("读取文件失败"));
        assert!(err_msg.contains("file not found"));
    }

    #[test]
    fn result_ext_preserves_success() {
        let result: Result<i32, io::Error> = Ok(42);

        let run_result: RunResult<i32> = result.to_run_err("不应该看到这个");

        assert_eq!(run_result.unwrap(), 42);
    }

    #[test]
    fn result_ext_with_lazy_context() {
        let mut counter = 0;

        let ok_result: Result<i32, io::Error> = Ok(100);
        let _ = ok_result.to_run_err_with(|_| {
            counter += 1; // 不应该执行
            "不应该看到".to_string()
        });
        assert_eq!(counter, 0, "成功时不应该调用闭包");

        let err_result: Result<i32, io::Error> = Err(io::Error::new(io::ErrorKind::Other, "error"));
        let run_result = err_result.to_run_err_with(|e| {
            counter += 1; // 应该执行
            format!("错误: {}", e)
        });

        assert_eq!(counter, 1, "失败时应该调用闭包一次");
        assert!(run_result.is_err());
    }

    #[test]
    fn result_ext_works_with_anyhow() {
        let anyhow_result: anyhow::Result<()> = Err(anyhow::anyhow!("something went wrong"));

        let run_result: RunResult<()> = anyhow_result.to_run_err("处理失败");

        assert!(run_result.is_err());
        let err_msg = run_result.unwrap_err().to_string();
        assert!(err_msg.contains("处理失败"));
        assert!(err_msg.contains("something went wrong"));
    }

    #[test]
    fn result_ext_works_with_string_error() {
        let string_result: Result<(), String> = Err("custom error".to_string());

        let run_result: RunResult<()> = string_result.to_run_err("自定义错误");

        assert!(run_result.is_err());
        let err_msg = run_result.unwrap_err().to_string();
        assert!(err_msg.contains("自定义错误"));
        assert!(err_msg.contains("custom error"));
    }
}
