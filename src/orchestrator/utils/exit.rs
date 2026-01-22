#![allow(dead_code)]
use orion_error::ErrorCode;
use wp_error::run_error::RunReason;

/// Map RunReason to process exit code.
/// - Dist => 1
/// - Source => 2
/// - Uvs => underlying error_code()
pub fn code_for_run_reason(reason: &RunReason) -> i32 {
    match reason {
        RunReason::Dist(_) => 1,
        RunReason::Source(_) => 2,
        RunReason::Uvs(u) => u.error_code(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orion_error::UvsReason;
    use wp_error::run_error::{DistFocus, SourceFocus};

    #[test]
    fn test_mapping() {
        assert_eq!(code_for_run_reason(&RunReason::Dist(DistFocus::StgCtrl)), 1);
        assert_eq!(
            code_for_run_reason(&RunReason::Source(SourceFocus::NoData)),
            2
        );
        let uv = UvsReason::core_conf("bad conf");
        assert_eq!(code_for_run_reason(&RunReason::Uvs(uv)), 300);
    }
}
