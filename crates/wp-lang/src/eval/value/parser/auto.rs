use super::prelude::*;
use crate::ast::WplSep;
use crate::generator::FmtField;
use wp_model_core::model::DataField;
use wp_model_core::model::FNameStr;

use crate::eval::runtime::field::FieldEvalUnit;
use crate::eval::value::parse_def::*;

pub struct CombinedParser {
    pub ps: Vec<ParserHold>,
}

impl CombinedParser {
    pub fn new() -> Self {
        Self { ps: Vec::new() }
    }
}
impl Default for CombinedParser {
    fn default() -> Self {
        Self::new()
    }
}

impl FieldParser for CombinedParser {
    fn parse<'a>(
        &self,
        fpu: &FieldEvalUnit,
        ups_sep: &WplSep,
        data: &mut &str,
        f_name: Option<FNameStr>,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()> {
        let mut last_e = None;

        // 优化: 在循环外计算默认名称，避免每次迭代都 clone
        let default_name = f_name.or_else(|| Some(fpu.conf().meta_name.clone()));

        let start = data.checkpoint();
        for p in &self.ps {
            // 每次迭代只需要 clone 一次（如果需要的话）
            match p.parse(fpu, ups_sep, data, default_name.clone(), out) {
                Ok(o) => {
                    return Ok(o);
                }
                Err(e) => {
                    data.reset(&start);
                    last_e = Some(e);
                }
            }
        }

        if let Some(e) = last_e {
            Err(e)
        } else {
            fail.context(ctx_desc("combine parse fail"))
                .parse_next(data)
        }
    }

    fn generate(
        &self,
        _gen: &mut GenChannel,
        _ups_sep: &WplSep,
        _f_conf: &WplField,
        _g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<FmtField> {
        unreachable!("Combine Parses not generate")
    }
}
