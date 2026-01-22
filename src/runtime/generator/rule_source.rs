use crate::facade::generator::GenRuleUnit;
use crate::types::AnyResult;
use anyhow::anyhow;
use wpl::generator::FmtFieldVec;
use wpl::{WplCompiledRule, WplStatementType, wpl_compile_rule};

/// Precompiled rule generator: compile once via wpl::wpl_compile_rule; support fast batch generation.
pub struct RuleGenSource {
    rules: Vec<WplCompiledRule>,
}

impl RuleGenSource {
    pub fn from_units(units: Vec<GenRuleUnit>) -> AnyResult<Self> {
        let mut compiled: Vec<WplCompiledRule> = Vec::new();
        for u in units.into_iter() {
            if u.is_empty() {
                continue;
            }
            let fields_map = u.get_fields().clone();
            for wpl_rule in u.get_rules().iter() {
                match &wpl_rule.statement {
                    WplStatementType::Express(_) => {
                        let cr = wpl_compile_rule(wpl_rule, &fields_map)
                            .map_err(|e| anyhow!("compile_rule error: {}", e))?;
                        compiled.push(cr);
                    }
                }
            }
        }
        if compiled.is_empty() {
            return Err(anyhow!("no compiled rules (empty WPL or fields)"));
        }
        Ok(Self { rules: compiled })
    }

    #[inline]
    pub fn rule_len(&self) -> usize {
        self.rules.len()
    }

    /// Generate one record from rule index (wrap-around by length)
    pub fn gen_one(&self, idx: usize) -> AnyResult<FmtFieldVec> {
        let ridx = idx % self.rules.len();
        let comp = &self.rules[ridx];
        comp.gen_one()
    }

    /// Generate `count` records starting at `idx_begin` (wrap-around).
    pub fn gen_batch(&self, idx_begin: usize, count: usize) -> AnyResult<Vec<FmtFieldVec>> {
        let ridx = idx_begin % self.rules.len();
        let comp = &self.rules[ridx];
        comp.gen_batch(idx_begin, count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::generator::rules::GenRuleUnit;
    use std::collections::HashMap;
    use wp_model_core::model::FNameStr;
    use wp_parse_api::RawData;
    use wpl::generator::{FieldGenBuilder, FieldGenConf};
    use wpl::{WplCode, WplEvaluator, WplPackage};

    fn build_pkg(code: &str) -> WplPackage {
        let w =
            WplCode::build(std::path::PathBuf::from("inline.wpl"), code).expect("build wpl code");
        w.parse_pkg().expect("parse wpl package")
    }

    /// compile from package code + field confs, then generate `n` rows
    fn gen_rows(
        rule_pkg_code: &str,
        fields: HashMap<FNameStr, FieldGenConf>,
        n: usize,
    ) -> Vec<wpl::generator::FmtFieldVec> {
        let pkg = build_pkg(rule_pkg_code);
        let unit = GenRuleUnit::new(pkg, fields);
        let src = RuleGenSource::from_units(vec![unit]).expect("compile rule");
        assert!(src.rule_len() >= 1);
        src.gen_batch(0, n).expect("gen batch")
    }

    /// parse a single line with a rule (no package wrapper)
    fn parse_line(rule_code: &str, line: &str) -> wp_model_core::model::DataRecord {
        let eval = WplEvaluator::from_code(rule_code).expect("build evaluator");
        let raw = RawData::from_string(line.to_string());
        let (rec, _remain) = eval.proc(raw, 0).expect("parse generated line");
        rec
    }

    #[test]
    fn compile_and_generate_simple_digit_fields() {
        // Package + rule with two named digit fields (top-level: use ": var"; '@ref' is for subfields only)
        let rule_pkg = r#"package t { rule gen1 { ( digit : x, digit : y ) } }"#;
        let rule = r#"rule gen1 { ( digit : x, digit : y ) }"#;
        // Configure generators for named fields x/y
        let fields = FieldGenBuilder::new()
            .digit("x", 100, 200)
            .digit("y", 10, 19)
            .build();
        let rows = gen_rows(rule_pkg, fields, 8);
        assert_eq!(rows.len(), 8);

        for ffv in rows {
            assert_eq!(ffv.len(), 2, "expected 2 fields per record");
            let line = wpl::generator::RAWGenFmt(&ffv).to_string();
            let rec = parse_line(rule, line.as_str());
            use wp_model_core::model::types::value::Value;
            match rec.get_value("x") {
                Some(Value::Digit(v)) => assert!(*v >= 100 && *v < 200, "x out of range: {}", v),
                other => panic!("x not digit: {:?}", other),
            }
            match rec.get_value("y") {
                Some(Value::Digit(v)) => assert!(*v >= 10 && *v < 19, "y out of range: {}", v),
                other => panic!("y not digit: {:?}", other),
            }
        }
    }

    #[test]
    fn compile_and_generate_nginx_like() {
        // Nginx-like layout，用显式作用域/分隔保证“生成->渲染->再解析”稳定：
        // - 对含空格/复杂字符的字段加引号作用域 '"'；
        // - 去掉生成侧 '_'，改用具名 chars 字段；解析侧保持相同布局；
        // - time 使用通用 time（内部生成 RFC 样式时间），同样加引号作用域。
        let pkg_rule = r#"
package t {
  rule nginx { (
    ip:sip,
    chars:ign1",
    chars:ign2",
    time:recv_time",
    http/request:req",
    http/status:status,
    digit:bytes,
    chars:referer",
    http/agent:agent",
    chars:tail"
  ) }
}
"#;
        let rule = r#"rule nginx { ( ip:sip, chars:ign1", chars:ign2", time:recv_time", http/request:req", http/status:status, digit:bytes, chars:referer", http/agent:agent", chars:tail" ) }"#;

        // Provide generators only for bytes/referer; others use parser built-in generators
        let fields = FieldGenBuilder::new()
            .digit("bytes", 100, 4096)
            .chars("referer")
            .build();

        let rows = gen_rows(pkg_rule, fields, 6);
        for ffv in rows {
            let line = wpl::generator::RAWGenFmt(&ffv).to_string();
            // 回解析：使用与生成一致的作用域/分隔定义
            let rec = parse_line(rule, line.as_str());
            use wp_model_core::model::types::value::Value;
            assert!(
                matches!(rec.get_value("sip"), Some(Value::IpAddr(_))),
                "sip not ip"
            );
            assert!(
                matches!(rec.get_value("status"), Some(Value::Digit(_))),
                "status not digit"
            );
            assert!(
                matches!(rec.get_value("bytes"), Some(Value::Digit(_))),
                "bytes not digit"
            );
            assert!(
                matches!(rec.get_value("referer"), Some(Value::Chars(_))),
                "referer not chars"
            );
            assert!(
                matches!(rec.get_value("agent"), Some(Value::Chars(_))),
                "agent not chars"
            );
        }
    }
}
