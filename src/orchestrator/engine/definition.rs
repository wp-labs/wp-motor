use derive_getters::Getters;
use std::path::PathBuf;
use wpl::{WplCode, WplCodeResult};

#[derive(Clone, Getters)]
pub struct WplCodePKG {
    code_vec: Vec<WplCode>,
    ///// 源 key 列表（用于构建 WPL 仓库的 source 索引；不再解析 source.wpl）
    //source_keys: Vec<String>,
}

impl WplCodePKG {
    pub fn from_code(code: &str) -> WplCodeResult<Self> {
        Ok(Self {
            code_vec: vec![WplCode::try_from((PathBuf::from("in_code"), code))?],
            //source_keys: Vec::new(),
        })
    }

    pub fn from_model(wpl: Vec<WplCode>) -> Self {
        Self {
            code_vec: wpl,
            //source_keys: Vec::new(),
        }
    }

    /// 附带源 key 的构造函数：取代旧的 `new(wpl, source_wpl)`
    pub fn from_codes(wpl: Vec<WplCode>) -> Self {
        Self { code_vec: wpl }
    }
}
