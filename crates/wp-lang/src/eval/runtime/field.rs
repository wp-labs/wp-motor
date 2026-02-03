use crate::ast::group::{GroupSeq, WplGroupType};
use crate::ast::{WplField, WplSep};
use crate::eval::runtime::field_pipe::PipeEnum;
use crate::eval::runtime::vm_unit::WplEvaluator;
use crate::eval::value::parse_def::{FieldParser, Hold, ParserHold};
use crate::eval::value::parser::ParserFactory;
use crate::eval::value::parser::base::CharsP;
use crate::generator::FieldGenConf;
use crate::generator::{FmtField, GenChannel};
use crate::parser::error::WplCodeResult;
use crate::types::AnyResult;
use derive_getters::Getters;
use wp_model_core::model::FNameStr;
// Use centralized parser result alias for consistency across crates
use wp_model_core::model::{DataField, DataType};
use wp_parser::WResult as ModalResult;

use super::pipe_exec::PipeExecutor;
use super::subunit::SubUnitManager;

#[derive(Clone, Getters)]
pub struct FieldEvalUnit {
    index: usize,
    conf: WplField,
    parser: ParserHold,
    pipe_exec: PipeExecutor,
    sub_units: SubUnitManager,
    next: Option<Box<FieldEvalUnit>>,
    pub group_enum: WplGroupType,
}

impl FieldEvalUnit {
    pub fn new(index: usize, conf: WplField, parser: ParserHold, group_enum: WplGroupType) -> Self {
        Self {
            index,
            conf,
            parser,
            pipe_exec: PipeExecutor::new(),
            sub_units: SubUnitManager::new(),
            next: None,
            group_enum,
        }
    }
    fn create_next(
        index: usize,
        meta: DataType,
        conf: WplField,
        group_enum: WplGroupType,
    ) -> WplCodeResult<Self> {
        let next = if let DataType::Array(next_name) = meta.clone() {
            let next_meta = DataType::from(next_name.as_str()).unwrap_or(DataType::Auto);
            Some(Box::new(Self::create_next(
                index,
                next_meta,
                conf.clone(),
                group_enum.clone(),
            )?))
            /*Some(Box::new(Self::create_next(
                index,
                next_meta,
                conf.clone(),
                group_enum.clone(),
            )?))
            */
        } else {
            None
        };
        let parser = ParserFactory::create(&meta)?;
        let mut cur_conf = conf.clone();
        cur_conf.meta_type = meta;
        let ins = Self {
            index,
            conf: cur_conf,
            parser,
            pipe_exec: PipeExecutor::new(),
            sub_units: SubUnitManager::new(),
            next,
            group_enum,
        };
        Ok(ins)
    }

    pub fn create(index: usize, conf: WplField, group_enum: WplGroupType) -> WplCodeResult<Self> {
        Self::create_next(index, conf.meta_type().clone(), conf, group_enum)
    }
    pub fn from_auto(conf: WplField) -> Self {
        WplEvaluator::assemble_fpu(0, &conf, WplGroupType::Seq(GroupSeq)).expect(" assemble fail")
    }
    pub fn add_pipe(&mut self, pipe: PipeEnum) {
        self.pipe_exec.add_pipe(pipe);
    }
    pub fn add_sub_fpu(&mut self, sub_key: String, fpu: FieldEvalUnit) {
        self.sub_units.add(sub_key, fpu);
    }
    pub fn get_sub_fpu(&self, sub_key: &str) -> Option<&FieldEvalUnit> {
        self.sub_units.get(sub_key)
    }
    pub fn conf_mut(&mut self) -> &mut WplField {
        &mut self.conf
    }
    pub fn default_fpu(&self, conf: WplField) -> FieldEvalUnit {
        let parser = CharsP::default();
        FieldEvalUnit::new(0, conf, Hold::new(parser), WplGroupType::Seq(GroupSeq))
    }
    pub fn for_test<T: FieldParser + Send + Sync + 'static>(parser: T, conf: WplField) -> Self {
        FieldEvalUnit::new(0, conf, Hold::new(parser), WplGroupType::Seq(GroupSeq))
    }
}

impl FieldEvalUnit {
    pub fn generate(
        &self,
        gnc: &mut GenChannel,
        sep: &WplSep,
        g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<FmtField> {
        self.parser.generate(gnc, sep, self.conf(), g_conf)
    }
    pub fn parse(
        &self,
        e_id: u64,
        upper_sep: &WplSep,
        data: &mut &str,
        run_key: Option<FNameStr>,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()> {
        let sep = self.conf.resolve_sep_ref(upper_sep);

        let data_rst = self
            .parser()
            .parse(e_id, self, sep.as_ref(), data, run_key.clone(), out);

        match data_rst {
            Ok(_) => self.pipe_exec.execute(e_id, out),
            Err(e) => {
                if self.conf.is_opt {
                    Ok(())
                } else {
                    Err(e)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use wildmatch::WildMatch;
    use wp_specs::WildArray;

    #[test]
    fn test_wild_array() {
        let value = WildArray(vec![WildMatch::new("/go")]);
        let value = serde_json::to_string(&value).unwrap();
        assert_eq!(value, r#"["/go"]"#.to_string());
    }
}
