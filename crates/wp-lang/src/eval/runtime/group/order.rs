use derive_getters::Getters;
use orion_overload::new::New3;
use winnow::combinator::{fail, trace};
use wp_model_core::model::DataField;

use wp_parser::Parser;
use wp_parser::WResult as ModalResult;

use crate::ast::WplSep;
use crate::ast::group::{GroupSeq, WplGroupType};
use crate::eval::runtime::field::FieldEvalUnit;
use crate::eval::runtime::vm_unit::StopWatch;

use crate::eval::desc::group_idx_desc;
use wp_parser::symbol::ctx_desc;

use super::LogicProc;

#[derive(Default, Clone, Getters)]
pub struct WplEvalGroup {
    pub rule: WplGroupType,
    pub field_units: Vec<FieldEvalUnit>,
    sep: Option<WplSep>,
    index: usize,
}
impl New3<usize, WplGroupType, Option<WplSep>> for WplEvalGroup {
    fn new(index: usize, meta: WplGroupType, sep: Option<WplSep>) -> Self {
        Self {
            rule: meta,
            field_units: Vec::new(),
            sep,
            index,
        }
    }
}
impl WplEvalGroup {
    pub fn combo_sep(&self, ups: &WplSep) -> WplSep {
        if let Some(cur) = &self.sep {
            let mut combo = cur.clone();
            combo.override_with(ups);
            combo
        } else {
            ups.clone()
        }
    }
}

// WplEvalGroup 仅在当前执行上下文内使用；去除不安全的 Send 实现。

impl LogicProc for GroupSeq {
    fn process(
        &self,
        group: &WplEvalGroup,
        ups_sep: &WplSep,
        data: &mut &str,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()> {
        let mut field_parsed = 0;

        let cur_sep = group.combo_sep(ups_sep);
        for fpu in group.field_units.iter() {
            let mut stop_watch = StopWatch::new(fpu.conf().continuous, fpu.conf().continuous_cnt);

            loop {
                stop_watch.tag_used();
                match fpu.parse(&cur_sep, data, None, out) {
                    Ok(_) => {
                        field_parsed += 1;
                    }
                    Err(e) => {
                        if stop_watch.allow_try() {
                            break;
                        }
                        return Err(e);
                    }
                }
                if stop_watch.is_stop() || data.is_empty() {
                    break;
                }
            }
        }
        if field_parsed < group.field_units.len() {
            warn!(
                "parsed field:{} , need field:{}",
                field_parsed,
                group.field_units.len()
            );
            return fail
                .context(ctx_desc("parse less more data !"))
                .parse_next(data);
        }
        Ok(())
    }
}

//pub const OPTIMIZE_TIMES: usize = 10000;

impl WplEvalGroup {
    pub fn proc(&self, sep: &WplSep, data: &mut &str, out: &mut Vec<DataField>) -> ModalResult<()> {
        match &self.rule {
            WplGroupType::Opt(x) => trace("<opt><group>", move |data: &mut &str| {
                x.process(self, sep, data, out)
            })
            .context(ctx_desc("<opt>"))
            .context(ctx_desc(group_idx_desc(self.index)))
            .parse_next(data),
            WplGroupType::Seq(x) => trace("<group>", move |data: &mut &str| {
                x.process(self, sep, data, out)
            })
            .context(ctx_desc(group_idx_desc(self.index)))
            .parse_next(data),
            WplGroupType::Alt(x) => trace("<alt><group>", move |data: &mut &str| {
                x.process(self, sep, data, out)
            })
            .context(ctx_desc("<alt>"))
            .context(ctx_desc(group_idx_desc(self.index)))
            .parse_next(data),
            WplGroupType::SomeOf(x) => trace("<someof><group>", move |data: &mut &str| {
                x.process(self, sep, data, out)
            })
            .context(ctx_desc("<someof>"))
            .context(ctx_desc(group_idx_desc(self.index)))
            .parse_next(data),
        }
    }
}

#[cfg(test)]
mod tests {
    use orion_error::TestAssert;
    use wp_parser::Parser;

    use crate::ast::WplSep;
    use crate::eval::runtime::vm_unit::WplEvaluator;
    use crate::eval::value::parser::ParserFactory;
    use crate::generator::{FmtFieldVec, GenChannel};
    use crate::parser::parse_code::wpl_express;
    use crate::types::AnyResult;
    use wp_model_core::model::DataType;
    use wp_model_core::model::Value;

    #[test]
    fn test_pipeline() -> AnyResult<()> {
        let express = wpl_express
            .parse(r#"(chars<[,]> | (ip,_,time ) )"#)
            .assert();
        let mut data = r#"[192.168.1.2 _ 06/Aug/2019:12:12:19 +0800]"#;
        let ppl = WplEvaluator::from(&express, None)?;
        let result = ppl.parse_groups(&mut data).assert();
        assert_eq!(data, "");
        assert!(result.field("ip").is_some());
        assert!(result.field("time").is_some());
        println!("{}", result);
        Ok(())
    }
    #[test]
    fn test_pipeline2() -> AnyResult<()> {
        let express = wpl_express
            .parse(r#"(chars<[,]> | (ip, time)\, )"#)
            .assert();
        let mut data = r#"[192.168.1.2 , 06/Aug/2019:12:12:19 +0800]"#;
        let ppl = WplEvaluator::from(&express, None)?;
        let result = ppl.parse_groups(&mut data).assert();
        assert_eq!(data, "");
        assert!(result.field("ip").is_some());
        assert!(result.field("time").is_some());
        println!("{}", result);
        Ok(())
    }
    #[test]
    fn test_pipeline3() -> AnyResult<()> {
        let express = wpl_express
            .parse(r#"(kv(chars<[,]> | (ip,_,time ) ))"#)
            .assert();
        println!("{:?}", express);
        let mut data = r#"data : [192.168.1.2 _ 06/Aug/2019:12:12:19 +0800]"#;
        let ppl = WplEvaluator::from(&express, None)?;
        let result = ppl.parse_groups(&mut data).assert();
        println!("{}", result);
        assert_eq!(data, "");
        assert!(result.field("ip").is_some());
        assert!(result.field("time").is_some());
        Ok(())
    }

    #[test]
    fn test_pipeline4() -> AnyResult<()> {
        let express = wpl_express
            .parse(r#"(json(chars@data<[,]> | (ip,_,time ) ))"#)
            .assert();
        let mut data = r#"{ "data" : "[192.168.1.2 _ 06/Aug/2019:12:12:19 +0800]" } "#;
        let ppl = WplEvaluator::from(&express, None)?;
        let result = ppl.parse_groups(&mut data).assert();
        println!("{}", result);
        assert_eq!(data, "");
        assert!(result.field("ip").is_some());
        assert!(result.field("time").is_some());
        Ok(())
    }
    #[test]
    fn test_gen() -> AnyResult<()> {
        let rule = wpl_express.parse(r#"(ip,time,kv)"#).assert();
        let mut fieldset = FmtFieldVec::new();
        let sep = WplSep::default();
        for group in &rule.group {
            for f_conf in &group.fields {
                let mut ch = GenChannel::new();
                let meta = DataType::from(f_conf.meta_name.as_str())?;
                let parser = ParserFactory::create(&meta)?;
                let field = parser.generate(&mut ch, &sep, f_conf, None)?;
                fieldset.push(field);
            }
        }
        for fmf in fieldset {
            println!(
                "{} :{}:{}{}",
                fmf.meta,
                fmf.data_field,
                fmf.field_fmt,
                fmf.sep.sep_str()
            )
        }
        Ok(())
    }

    #[test]
    fn test_group_sep_and_field_sep_precedence() -> AnyResult<()> {
        // 组分隔符（mid）作用于组内全部字段
        let express = wpl_express.parse(r#"(chars:a, chars:b)\|"#).assert();
        let mut data = r#"foo|bar"#;
        let ppl = WplEvaluator::from(&express, None)?;
        let result = ppl.parse_groups(&mut data).assert();
        assert_eq!(data, "");
        let a = result.field("a").and_then(|f| match f.get_value() {
            Value::Chars(s) => Some(s.clone()),
            _ => None,
        });
        let b = result.field("b").and_then(|f| match f.get_value() {
            Value::Chars(s) => Some(s.clone()),
            _ => None,
        });
        assert_eq!(a, Some("foo".into()));
        assert_eq!(b, Some("bar".into()));

        // 字段分隔符（high）优先级高于组分隔符（mid）
        let express = wpl_express.parse(r#"(chars:a, chars:b\|)\,"#).assert();
        let mut data = r#"x,y|z"#;
        let ppl = WplEvaluator::from(&express, None)?;
        let result = ppl.parse_groups(&mut data).assert();
        // a 用组分隔符 ','，b 用字段分隔符 '|'
        let a = result.field("a").and_then(|f| match f.get_value() {
            Value::Chars(s) => Some(s.clone()),
            _ => None,
        });
        let b = result.field("b").and_then(|f| match f.get_value() {
            Value::Chars(s) => Some(s.clone()),
            _ => None,
        });
        assert_eq!(a, Some("x".into()));
        assert_eq!(b, Some("y".into()));
        Ok(())
    }
}
