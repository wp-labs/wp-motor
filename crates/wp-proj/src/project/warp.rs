use std::{
    env,
    path::{Path, PathBuf},
    sync::Arc,
};

use super::{Connectors, Oml, ProjectPaths, Sinks, Sources, Wpl, init::PrjScope};
use crate::{
    models::knowledge::Knowledge, sinks::clean_outputs, wparse::WParseManager, wpgen::WpGenManager,
};
use orion_variate::EnvDict;
use wp_conf::engine::EngineConfig;
use wp_error::run_error::RunResult;

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
///

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
    fn build(work_root: &Path, dict: &orion_variate::EnvDict) -> Self {
        let abs_root = normalize_work_root(work_root);
        let paths = ProjectPaths::from_root(&abs_root);
        let eng_conf = Arc::new(
            EngineConfig::load_or_init(&abs_root, dict)
                .expect("load engine config")
                .conf_absolutize(&abs_root),
        );
        let connectors = Connectors::new(paths.connectors.clone());
        let sinks_c = Sinks::new(&abs_root, eng_conf.clone());
        let sources_c = Sources::new(&abs_root, eng_conf.clone());
        let wpl = Wpl::new(&abs_root, eng_conf.clone());
        let oml = Oml::new(&abs_root, eng_conf.clone());
        let knowledge = Knowledge::new();
        let wparse_manager = WParseManager::new(&abs_root);
        let wpgen_manager = WpGenManager::new(&abs_root);

        Self {
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
        }
    }

    /// 静态初始化：创建并初始化完整项目
    pub fn init<P: AsRef<Path>>(
        work_root: P,
        mode: PrjScope,
        dict: &orion_variate::EnvDict,
    ) -> RunResult<Self> {
        let mut project = Self::build(work_root.as_ref(), dict);
        project.init_components(mode)?;
        Ok(project)
    }

    /// 静态加载：基于现有结构执行校验加载
    pub fn load<P: AsRef<Path>>(
        work_root: P,
        mode: PrjScope,
        dict: &orion_variate::EnvDict,
    ) -> RunResult<Self> {
        let mut project = Self::build(work_root.as_ref(), dict);
        project.load_components(mode)?;
        Ok(project)
    }

    #[cfg(test)]
    pub(crate) fn bare<P: AsRef<Path>>(work_root: P) -> Self {
        use wp_conf::test_support::ForTest;
        Self::build(work_root.as_ref(), &orion_variate::EnvDict::test_default())
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

        //  清理 sinks 输出数据
        if let Ok(sink_cleaned) = clean_outputs(self.work_root(), dict) {
            cleaned_any |= sink_cleaned;
        }

        //  清理 wpgen 生成数据（委托给 WPgenManager）
        if let Ok(wpgen_cleaned) = self.wpgen_manager.clean_outputs(dict) {
            cleaned_any |= wpgen_cleaned;
        }

        //  清理 wparse 相关临时数据（委托给 WParseManager）
        if let Ok(wparse_cleaned) = self.wparse_manager.clean_data(dict) {
            cleaned_any |= wparse_cleaned;
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
    if work_root.is_absolute() {
        work_root.to_path_buf()
    } else {
        let rel = work_root.to_path_buf();
        let base = env::current_dir().unwrap_or_else(|err| panic!("获取当前工作目录失败: {}", err));
        base.join(&rel)
    }
}
