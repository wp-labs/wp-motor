// Use centralized alias for parser result, avoid direct dependency on winnow type name
use wp_model_core::model::DataField;
use wp_parser::WResult as ModalResult;

use crate::WplSep;

use super::WplEvalGroup;

pub trait LogicProc {
    fn process(
        &self,
        group: &WplEvalGroup,
        ups_sep: &WplSep,
        data: &mut &str,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()>;
}
