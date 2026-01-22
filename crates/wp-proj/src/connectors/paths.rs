use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct ProjectPaths {
    pub root: PathBuf,
    pub conf_dir: PathBuf,
    pub connectors: ConnectorsPaths,
}

#[derive(Clone, Debug)]
#[allow(dead_code)] // Fields are used in tests but not detected
pub struct ConnectorsPaths {
    pub base: PathBuf,
    pub source_dir: PathBuf,
    pub sink_dir: PathBuf,
}

impl ProjectPaths {
    pub fn from_root<P: AsRef<std::path::Path>>(root: P) -> Self {
        let root_path = root.as_ref().to_path_buf();

        // 项目结构约定：conf 和 connectors 目录是固定的项目结构
        // 这些路径不是用户可配置的，而是项目架构的标准约定
        let conf_dir = root_path.join("conf");
        let connectors_base = root_path.join("connectors");

        Self {
            root: root_path,
            conf_dir,
            connectors: ConnectorsPaths {
                base: connectors_base.clone(),
                source_dir: connectors_base.join("source.d"),
                sink_dir: connectors_base.join("sink.d"),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_paths_populates_standard_layout() {
        let paths = ProjectPaths::from_root("/work");
        assert_eq!(paths.conf_dir, PathBuf::from("/work/conf"));
        assert_eq!(
            paths.connectors.source_dir,
            PathBuf::from("/work/connectors/source.d")
        );
        assert_eq!(
            paths.connectors.sink_dir,
            PathBuf::from("/work/connectors/sink.d")
        );
    }
}
