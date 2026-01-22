use educe::Educe;
use orion_conf::{
    ToStructError,
    error::{ConfIOReason, OrionConfResult},
};
use orion_error::UvsValidationFrom;
use orion_variate::EnvDict;
use std::path::Path;

#[derive(Educe, Deserialize, Serialize, PartialEq, Clone)]
#[educe(Debug, Default)]
pub struct FileSinkConf {
    #[educe(Default = "./out.dat")]
    pub path: String,
}

impl FileSinkConf {
    pub fn new<S: Into<String>>(path: S) -> Self {
        Self { path: path.into() }
    }

    pub fn new_path<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().display().to_string(),
        }
    }
    pub fn use_cli(&mut self, cli_path: Option<String>) {
        if let Some(cli_path) = cli_path {
            self.path = cli_path;
        }
    }
}

impl crate::structure::Validate for FileSinkConf {
    fn validate(&self) -> OrionConfResult<()> {
        if self.path.trim().is_empty() {
            return ConfIOReason::from_validation("out_file.path must not be empty").err_result();
        }
        let p = std::path::Path::new(&self.path);
        if let Some(parent) = p.parent()
            && !parent.as_os_str().is_empty()
            && !parent.exists()
        {
            std::fs::create_dir_all(parent).map_err(|e| {
                ConfIOReason::from_validation(format!(
                    "create parent dir failed: {:?}, err={}",
                    parent, e
                ))
                .to_err()
            })?;
        }
        Ok(())
    }
}

// ============================================================================
// ConfigLoader trait implementation for unified loading interface
// ============================================================================

impl crate::loader::traits::ConfigLoader for FileSinkConf {
    fn config_type_name() -> &'static str {
        "File Sink"
    }

    fn load_from_str(content: &str, _base: &Path, _dict: &EnvDict) -> OrionConfResult<Self> {
        // FileSinkConf 是一个简单的结构，直接从 TOML 解析即可
        let conf: FileSinkConf = toml::from_str(content)
            .map_err(|e| ConfIOReason::from_validation(format!("TOML 解析失败: {}", e)).to_err())?;

        Ok(conf)
    }

    fn validate(&self) -> OrionConfResult<()> {
        // 使用已有的 Validate trait 实现
        <Self as crate::structure::Validate>::validate(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::traits::ConfigLoader;
    use crate::test_support::ForTest;

    #[test]
    fn config_loader_file_sink_from_str() {
        let toml = r#"path = "/tmp/output.dat""#;
        let result = FileSinkConf::load_from_str(toml, Path::new("/"), &EnvDict::test_default());

        assert!(result.is_ok());
        let conf = result.unwrap();
        assert_eq!(conf.path, "/tmp/output.dat");
    }

    #[test]
    fn config_loader_file_sink_from_path() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, r#"path = "/var/log/test.log""#).unwrap();

        let result = FileSinkConf::load_from_path(file.path(), &EnvDict::test_default());

        assert!(result.is_ok());
        let conf = result.unwrap();
        assert_eq!(conf.path, "/var/log/test.log");
    }

    #[test]
    fn config_loader_file_sink_validation() {
        let invalid_toml = r#"path = """#;

        use std::io::Write;
        use tempfile::NamedTempFile;
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "{}", invalid_toml).unwrap();

        let result = FileSinkConf::load_from_path(file.path(), &EnvDict::test_default());

        assert!(result.is_err(), "空路径应该验证失败");
    }
}
