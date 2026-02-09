use crate::WplSep;
use crate::ast::group::GroupNot;
use crate::eval::runtime::group::{LogicProc, WplEvalGroup};
use winnow::stream::Stream;
use wp_model_core::model::DataField;
use wp_parser::WResult as ModalResult;

impl LogicProc for GroupNot {
    fn process(
        &self,
        e_id: u64,
        group: &WplEvalGroup,
        ups_sep: &WplSep,
        data: &mut &str,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()> {
        not_proc(e_id, group, ups_sep, data, out)
    }
}

pub fn not_proc(
    e_id: u64,
    group: &WplEvalGroup,
    ups_sep: &WplSep,
    data: &mut &str,
    out: &mut Vec<DataField>,
) -> ModalResult<()> {
    // not() should have exactly one sub-field
    let fpu = match group.field_units.first() {
        Some(f) => f,
        None => {
            // No sub-field, return error
            return Err(winnow::error::ErrMode::Backtrack(
                winnow::error::ContextError::default(),
            ));
        }
    };

    let cur_sep = group.combo_sep(ups_sep);
    let ck_point = data.checkpoint();

    // Try to parse the sub-field
    let mut temp_out = Vec::new();
    match fpu.parse(
        e_id,
        &cur_sep,
        data,
        Some(fpu.conf().safe_name()),
        &mut temp_out,
    ) {
        Ok(_) => {
            // Sub-field matched - this is FAILURE for not()
            // Reset data position and return error
            data.reset(&ck_point);
            Err(winnow::error::ErrMode::Backtrack(
                winnow::error::ContextError::default(),
            ))
        }
        Err(_) => {
            // Sub-field failed to match - this is SUCCESS for not()
            // Important: DON'T reset data position!
            // The internal parser may have consumed some input before failing
            // (e.g., symbol(ERROR) might consume whitespace before failing)
            // We want to keep that consumption.
            // But if it's peek_symbol, it won't have consumed anything anyway.

            // Add ignore field to output
            if let Some(first_field) = group.field_units.first() {
                let name = first_field.conf().safe_name();
                out.push(DataField::from_ignore(name));
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::types::AnyResult;
    use crate::{WplEvaluator, wpl_express};
    use orion_error::TestAssert;
    use wp_model_core::model::DataField;
    use wp_parser::Parser;

    #[test]
    fn test_not_group_basic() -> AnyResult<()> {
        // not(symbol(ERROR)) should succeed when ERROR is NOT present
        let express = wpl_express.parse(r#"not(symbol(ERROR):test)"#).assert();
        let mut data = "INFO: hello world";
        let ppl = WplEvaluator::from(&express, None)?;

        let result = ppl.parse_groups(0, &mut data).assert();
        println!("{}", result);
        // Check that test field exists and is ignore type
        assert_eq!(result.get_field("test"), Some(&DataField::from_ignore("test")));

        Ok(())
    }

    #[test]
    fn test_not_group_failure() -> AnyResult<()> {
        // not(symbol(ERROR)) should fail when ERROR IS present
        let express = wpl_express.parse(r#"not(symbol(ERROR):test)"#).assert();
        let mut data = "ERROR: something wrong";
        let ppl = WplEvaluator::from(&express, None)?;

        let result = ppl.parse_groups(0, &mut data);
        assert!(result.is_err(), "not() should fail when symbol matches");

        Ok(())
    }

    #[test]
    fn test_not_with_peek_symbol() -> AnyResult<()> {
        // not(peek_symbol(ERROR)) should not consume input
        // Correct format: multiple parallel groups
        let express = wpl_express
            .parse(r#"not(peek_symbol(ERROR):test),(chars:msg)"#)
            .assert();
        let mut data = "INFO message";
        let ppl = WplEvaluator::from(&express, None)?;

        let result = ppl.parse_groups(0, &mut data).assert();
        println!("{}", result);
        // peek_symbol doesn't consume, so chars should get "INFO"
        assert_eq!(
            result.get_field("msg"),
            Some(&DataField::from_chars("msg", "INFO"))
        );

        Ok(())
    }
}
