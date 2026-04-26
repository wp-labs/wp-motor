use std::{
    env,
    path::{Path, PathBuf},
    sync::Arc,
};

use super::{Connectors, Oml, ProjectPaths, Sinks, Sources, Wpl, init::PrjScope};
use crate::{
    models::knowledge::Knowledge, sinks::clean_outputs, wparse::WParseManager, wpgen::WpGenManager,
};
use orion_error::{ToStructError, UvsFrom, WrapStructError};
use orion_variate::{EnvDict, EnvEvaluable};
use wp_conf::engine::EngineConfig;
use wp_error::run_error::{RunError, RunReason, RunResult};

#[derive(Debug)]
struct DataCleanFailure {
    component: &'static str,
    error: RunError,
}

/// # WarpProject
///
/// 高层工程管理器，提供统一的项目管理接口。
///
/// ## 主要功能
///
/// 1. **项目初始化**: 创建完整的项目结构，包括配置、模板和模型
/// 2. **项目检查**: 验证项目配置和组件的完整性
/// 3. **组件管理**: 统一管理连接器、输入源、输出接收器等组件
/// 4. **模型管理**: 管理 WPL 解析规则和 OML 模型配置
pub struct WarpProject {
    // 项目路径管理器
    paths: ProjectPaths,
    eng_conf: Arc<EngineConfig>,
    // 环境变量字典
    pub(crate) dict: orion_variate::EnvDict,
    // 连接器管理
    connectors: Connectors,
    // 输出接收器管理
    sinks_c: Sinks,
    // 输入源管理
    sources_c: Sources,
    // WPL 解析规则管理
    wpl: Wpl,
    // OML 模型管理
    oml: Oml,
    // 知识库管理
    knowledge: Knowledge,
    // WParse 管理器
    wparse_manager: WParseManager,
    // WPgen 管理器
    wpgen_manager: WpGenManager,
}

impl WarpProject {
    fn build(work_root: &Path, dict: &orion_variate::EnvDict) -> RunResult<Self> {
        Self::build_with_engine_config(work_root, dict, true)
    }

    fn build_existing(work_root: &Path, dict: &orion_variate::EnvDict) -> RunResult<Self> {
        Self::build_with_engine_config(work_root, dict, false)
    }

    fn build_with_engine_config(
        work_root: &Path,
        dict: &orion_variate::EnvDict,
        init_missing_engine_config: bool,
    ) -> RunResult<Self> {
        let abs_root = normalize_work_root_result(work_root)?;
        let paths = ProjectPaths::from_root(&abs_root);
        if init_missing_engine_config {
            std::fs::create_dir_all(&abs_root).map_err(|err| {
                RunReason::from_conf()
                    .to_err()
                    .with_detail(format!("create work root '{}' failed", abs_root.display()))
                    .with_source(err)
            })?;
            std::fs::create_dir_all(&paths.conf_dir).map_err(|err| {
                RunReason::from_conf()
                    .to_err()
                    .with_detail(format!(
                        "create conf dir '{}' failed",
                        paths.conf_dir.display()
                    ))
                    .with_source(err)
            })?;
        }
        let eng_conf = if init_missing_engine_config {
            EngineConfig::load_or_init(&abs_root, dict)
                .map_err(|err| {
                    RunReason::from_conf()
                        .to_err()
                        .with_detail("load engine config failed")
                        .with_struct_source(err)
                })?
                .env_eval(dict)
                .conf_absolutize(&abs_root)
        } else {
            EngineConfig::load(&abs_root, dict)
                .map_err(|err| {
                    RunReason::from_conf()
                        .to_err()
                        .with_detail("load engine config failed")
                        .with_struct_source(err)
                })?
                .env_eval(dict)
                .conf_absolutize(&abs_root)
        };
        let eng_conf = Arc::new(eng_conf);
        let connectors = Connectors::new(paths.connectors.clone());
        let sinks_c = Sinks::new(&abs_root, eng_conf.clone());
        let sources_c = Sources::new(&abs_root, eng_conf.clone());
        let wpl = Wpl::new(&abs_root, eng_conf.clone());
        let oml = Oml::new(&abs_root, eng_conf.clone());
        let knowledge = Knowledge::new();
        let wparse_manager = WParseManager::new(&abs_root);
        let wpgen_manager = WpGenManager::new(&abs_root);

        Ok(Self {
            paths,
            eng_conf,
            dict: dict.clone(),
            connectors,
            sinks_c,
            sources_c,
            wpl,
            oml,
            knowledge,
            wparse_manager,
            wpgen_manager,
        })
    }

    /// 静态初始化：创建并初始化完整项目
    pub fn init<P: AsRef<Path>>(
        work_root: P,
        mode: PrjScope,
        dict: &orion_variate::EnvDict,
    ) -> RunResult<Self> {
        let mut project = Self::build(work_root.as_ref(), dict)?;
        project.init_components(mode)?;
        Ok(project)
    }

    /// 静态加载：基于现有结构执行校验加载
    pub fn load<P: AsRef<Path>>(
        work_root: P,
        mode: PrjScope,
        dict: &orion_variate::EnvDict,
    ) -> RunResult<Self> {
        let mut project = Self::build_existing(work_root.as_ref(), dict)?;
        project.load_components(mode)?;
        Ok(project)
    }

    #[cfg(test)]
    pub(crate) fn bare<P: AsRef<Path>>(work_root: P) -> Self {
        use wp_conf::test_support::ForTest;
        Self::build(work_root.as_ref(), &orion_variate::EnvDict::test_default())
            .expect("build bare project")
    }

    /// 获取工作根目录（向后兼容）
    pub fn work_root(&self) -> &str {
        self.paths.root.to_str().unwrap_or_default()
    }
    pub fn work_root_path(&self) -> &PathBuf {
        &self.paths.root
    }

    pub fn paths(&self) -> &ProjectPaths {
        &self.paths
    }

    pub fn connectors(&self) -> &Connectors {
        &self.connectors
    }

    pub fn sinks_c(&self) -> &Sinks {
        &self.sinks_c
    }

    pub fn sources_c(&self) -> &Sources {
        &self.sources_c
    }

    pub fn wpl(&self) -> &Wpl {
        &self.wpl
    }

    pub fn oml(&self) -> &Oml {
        &self.oml
    }

    pub fn knowledge(&self) -> &Knowledge {
        &self.knowledge
    }

    pub(crate) fn replace_engine_conf(&mut self, conf: EngineConfig) {
        let arc = Arc::new(conf);
        self.eng_conf = arc.clone();
        self.sinks_c.update_engine_conf(arc.clone());
        self.sources_c.update_engine_conf(arc.clone());
        self.wpl.update_engine_conf(arc.clone());
        self.oml.update_engine_conf(arc);
    }

    // ========== 配置管理方法 ==========

    /// 清理项目数据目录（委托给各个专门的模块处理）
    pub fn data_clean(&self, dict: &EnvDict) -> RunResult<()> {
        let mut cleaned_any = false;
        let mut failures: Vec<DataCleanFailure> = Vec::new();

        //  清理 sinks 输出数据
        match clean_outputs(self.work_root(), dict) {
            Ok(sink_cleaned) => cleaned_any |= sink_cleaned,
            Err(error) => failures.push(DataCleanFailure {
                component: "sinks",
                error,
            }),
        }

        //  清理 wpgen 生成数据（委托给 WPgenManager）
        match self.wpgen_manager.clean_outputs(dict) {
            Ok(wpgen_cleaned) => cleaned_any |= wpgen_cleaned,
            Err(error) => failures.push(DataCleanFailure {
                component: "wpgen",
                error,
            }),
        }

        //  清理 wparse 相关临时数据（委托给 WParseManager）
        match self.wparse_manager.clean_data(dict) {
            Ok(wparse_cleaned) => cleaned_any |= wparse_cleaned,
            Err(error) => failures.push(DataCleanFailure {
                component: "wparse",
                error,
            }),
        }

        if !failures.is_empty() {
            let detail = format!(
                "数据清理存在失败项: {}",
                failures
                    .iter()
                    .map(|failure| format!("{}: {}", failure.component, failure.error))
                    .collect::<Vec<_>>()
                    .join(" | ")
            );
            let first = failures.into_iter().next().expect("checked non-empty");
            return Err(first.error.wrap(RunReason::from_conf()).with_detail(detail));
        }

        if !cleaned_any {
            println!("No data files to clean");
        } else {
            println!("✓ Data cleanup completed");
        }

        Ok(())
    }
}

pub(crate) fn normalize_work_root(work_root: &Path) -> PathBuf {
    normalize_work_root_result(work_root).expect("normalize work root")
}

fn normalize_work_root_result(work_root: &Path) -> RunResult<PathBuf> {
    if work_root.is_absolute() {
        Ok(work_root.to_path_buf())
    } else {
        let rel = work_root.to_path_buf();
        let base = env::current_dir().map_err(|err| {
            RunReason::from_conf()
                .to_err()
                .with_detail("resolve current dir failed")
                .with_source(err)
        })?;
        Ok(base.join(&rel))
    }
}
