use super::common::fast_apache_dt;
use crate::eval::runtime::field::FieldEvalUnit;
use crate::eval::value::parse_def::PatternParser;
use crate::generator::{FieldGenConf, GenChannel};
use crate::types::AnyResult;
use winnow::stream::Stream as _;
use wp_model_core::model::DataField;
use wp_model_core::model::FNameStr;

#[derive(Default)]
pub struct TimeCLF {}

impl PatternParser for TimeCLF {
    fn pattern_parse(
        &self,
        _e_id: u64,
        _: &FieldEvalUnit,
        _: &crate::ast::WplSep,
        data: &mut &str,
        name: FNameStr,
        out: &mut Vec<DataField>,
    ) -> wp_parser::WResult<()> {
        // Avoid explicit deref; pass through and let auto-deref handle coercions
        if let Some((consumed, ndt)) = fast_apache_dt(data) {
            *data = &data[consumed..];
            out.push(DataField::from_time(name, ndt));
            return Ok(());
        }
        let cp = (*data).checkpoint();
        Err(winnow::error::ErrMode::Backtrack(
            wp_parser::utils::context_error(data, &cp, "<time/clf> parse failed"),
        ))
    }
    fn patten_gen(
        &self,
        gnc: &mut GenChannel,
        f_conf: &crate::ast::WplField,
        g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<DataField> {
        super::gen_time(gnc, f_conf, g_conf)
    }
}

#[cfg(test)]
mod tests {
    use super::fast_apache_dt;
    use chrono::NaiveDate;

    #[test]
    fn test_fast_apache_plain() {
        let s = "06/Aug/2019:12:12:19 +0800 rest";
        let (consumed, ndt) = fast_apache_dt(s).expect("parse");
        assert_eq!(ndt.date(), NaiveDate::from_ymd_opt(2019, 8, 6).unwrap());
        assert!(s[consumed..].starts_with(" rest"));
    }

    #[test]
    fn test_fast_apache_bracketed() {
        let s = "[06/Aug/2019:12:12:19 +0800] tail";
        let (consumed, ndt) = fast_apache_dt(s).expect("parse");
        assert_eq!(ndt.date(), NaiveDate::from_ymd_opt(2019, 8, 6).unwrap());
        assert!(s[consumed..].starts_with(" tail"));
    }

    #[test]
    fn test_fast_apache_invalid() {
        assert!(fast_apache_dt("06-08-2019 12:12:19").is_none());
    }
}
