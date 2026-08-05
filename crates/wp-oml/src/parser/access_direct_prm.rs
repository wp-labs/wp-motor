use crate::language::{AccessDirectOperation, PiPeOperation, PipeSource, PreciseEvaluator};
use crate::parser::oml_aggregate::oml_var_get;
use winnow::ascii::multispace0;
use winnow::combinator::{fail, repeat};
use winnow::error::{ContextError, ErrMode, StrContext};
use winnow::stream::Stream;
use wp_primitives::Parser;
use wp_primitives::WResult;
use wp_primitives::symbol::{ctx_desc, symbol_comma};
use wp_primitives::utils::get_scope;

/// 解析 `access_direct(read(src), read(dst))` → `PreciseEvaluator::AccessDirect`
pub fn oml_aga_access_direct(data: &mut &str) -> WResult<PreciseEvaluator> {
    let op = oml_access_direct.parse_next(data)?;
    Ok(PreciseEvaluator::AccessDirect(op))
}

/// 解析 `access_direct(read(src), read(dst)) | fun | ...` → `PreciseEvaluator::Pipe`
/// （access_direct 结果作为管道源，可接 on_fail 等管道函数）
pub fn oml_aga_access_direct_pipe(data: &mut &str) -> WResult<PreciseEvaluator> {
    let cp = data.checkpoint();
    let op = oml_access_direct.parse_next(data)?;
    match repeat(1.., crate::parser::pipe_prm::oml_pipe).parse_next(data) {
        Ok(items) => Ok(PreciseEvaluator::Pipe(PiPeOperation::new(
            PipeSource::AccessDirect(op),
            items,
        ))),
        Err(_) => {
            data.reset(&cp);
            fail.parse_next(data)
        }
    }
}

pub fn oml_access_direct(data: &mut &str) -> WResult<AccessDirectOperation> {
    multispace0.parse_next(data)?;
    "access_direct"
        .context(StrContext::Label("oml keyword"))
        .context(ctx_desc("need 'access_direct' keyword"))
        .parse_next(data)?;

    let scope = get_scope(data, '(', ')')?;
    let mut code_data: &str = scope;

    let src = oml_var_get.parse_next(&mut code_data)?;
    symbol_comma.parse_next(&mut code_data)?;
    let dst = oml_var_get.parse_next(&mut code_data)?;

    multispace0.parse_next(&mut code_data)?;
    if !code_data.is_empty() {
        return Err(ErrMode::Backtrack(ContextError::new()));
    }
    Ok(AccessDirectOperation::new(src, dst))
}

#[cfg(test)]
mod tests {
    use crate::language::{AccessDirectOperation, PreciseEvaluator};
    use crate::parser::access_direct_prm::{oml_aga_access_direct, oml_aga_access_direct_pipe};
    use crate::parser::utils::for_test::assert_oml_parse;
    use wp_primitives::Parser;
    use wp_primitives::WResult;

    #[test]
    fn test_parse_access_direct() -> WResult<()> {
        let mut code = r#" access_direct(read(sip), read(dip)) "#;
        assert_oml_parse(&mut code, oml_aga_access_direct);

        let mut code = r#" access_direct(take(sip), take(dip)) "#;
        assert_oml_parse(&mut code, oml_aga_access_direct);

        Ok(())
    }

    #[test]
    fn test_access_direct_round_trip() {
        let src = crate::language::DirectAccessor::Read(crate::language::FieldRead::new("sip".into()));
        let dst = crate::language::DirectAccessor::Read(crate::language::FieldRead::new("dip".into()));
        let op = AccessDirectOperation::new(src, dst);
        let disp = format!("{}", op);

        // Display 输出应能被重新解析（round-trip）
        let mut code = disp.as_str();
        let re = crate::parser::access_direct_prm::oml_access_direct.parse_next(&mut code);
        assert!(re.is_ok(), "round-trip parse failed: {}", disp);

        let _ = PreciseEvaluator::AccessDirect(op.clone());
    }

    #[test]
    fn test_pipe_prefix_access_direct() -> WResult<()> {
        // `pipe` 前缀形式也支持 access_direct 作为管道源
        let mut code = r#" pipe access_direct(read(sip), read(dip)) | on_fail('unknown') "#;
        assert_oml_parse(&mut code, crate::parser::pipe_prm::oml_aga_pipe);
        Ok(())
    }

    #[test]
    fn test_access_direct_pipe_round_trip() {
        // 无前缀解析 → Display（带 pipe 前缀）→ 重新解析（round-trip 一致）
        let mut code = r#" access_direct(read(sip), read(dip)) | on_fail('unknown') "#;
        let eval = oml_aga_access_direct_pipe
            .parse_next(&mut code)
            .expect("parse access_direct pipe");
        let disp = format!("{}", eval);

        let mut code2 = disp.as_str();
        let re = crate::parser::pipe_prm::oml_aga_pipe.parse_next(&mut code2);
        assert!(re.is_ok(), "round-trip parse failed: {}", disp);
    }
}
