use super::super::prelude::*;
use crate::generator::FieldGenConf;
use crate::generator::GenChannel;
use crate::types::AnyResult;
use wp_model_core::model::DataField;
use wp_model_core::model::FNameStr;
use wp_parser::Parser;
use wp_parser::WResult as ModalResult;
use wp_parser::symbol::ctx_desc;

use crate::eval::runtime::field::FieldEvalUnit;
use crate::eval::value::parse_def::*;
use crate::parser::utils::take_to_end;

#[derive(Default)]
pub struct IgnoreP {}

impl PatternParser for IgnoreP {
    fn pattern_parse<'a>(
        &self,
        _e_id: u64,
        _fpu: &FieldEvalUnit,
        _ups_sep: &WplSep,
        data: &mut &str,
        name: FNameStr,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()> {
        //let _buffer = alt((quot_str, window_path, take_to_end))
        let _buffer = take_to_end.context(ctx_desc("<ignore>")).parse_next(data)?;
        out.push(DataField::from_ignore(name));
        Ok(())
    }

    fn patten_gen(
        &self,
        _gen: &mut GenChannel,
        _f_conf: &WplField,
        _g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<DataField> {
        unreachable!("ignore field should not generate")
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::WplField;
    use crate::eval::value::test_utils::ParserTUnit;
    use orion_error::TestAssert;

    use super::*;

    #[test]
    fn test_ignore_from_case() {
        let mut data = "  -[UMSyncService fetchPersona:forPid:completionHandler:]_block_invoke:";
        let conf = WplField::try_parse("_<-[,]>").assert();
        ParserTUnit::new(IgnoreP::default(), conf.clone())
            .verify_parse_suc(&mut data)
            .assert();
        assert_eq!(data, "_block_invoke:");
    }
}
