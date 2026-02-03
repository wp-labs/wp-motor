use crate::WplSep;
use crate::ast::group::GroupSomeOf;
use crate::eval::runtime::group::{LogicProc, WplEvalGroup};
use winnow::stream::Stream;
// Use workspace-level parser result alias
use wp_log::trace_edata;
use wp_model_core::model::DataField;
use wp_parser::WResult as ModalResult;

impl LogicProc for GroupSomeOf {
    fn process(
        &self,
        e_id: u64,
        group: &WplEvalGroup,
        ups_sep: &WplSep,
        data: &mut &str,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()> {
        let mut all_failed = false;
        let cur_sep = group.combo_sep(ups_sep);
        while !data.is_empty() && !all_failed {
            all_failed = true;
            for fpu in group.field_units.iter() {
                let ck_point = data.checkpoint();
                match fpu.parse(e_id, &cur_sep, data, None, out) {
                    Ok(_) => {
                        all_failed = false;
                        break;
                    }
                    Err(e) => {
                        data.reset(&ck_point);
                        trace_edata!(e_id, "fpt parse error :{},{}", fpu.conf(), e);
                        continue;
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::types::AnyResult;
    use crate::{WplEvaluator, wpl_express};
    use orion_error::TestAssert;
    use wp_parser::Parser;

    #[test]
    fn test_some_of_group1() -> AnyResult<()> {
        let express = wpl_express
            .parse(r#"some_of(ip:sip, time<[,]>,digit:id),(2*_,time<[,]>)"#)
            .assert();
        let mut data = r#"192.168.1.2 - - [06/Aug/2019:12:12:19 +0800] "#;
        let ppl = WplEvaluator::from(&express, None)?;

        let result = ppl.parse_groups(0, &mut data).assert();
        assert_eq!(data, "");
        println!("{}", result);

        let mut data = r#"2002 - - [06/Aug/2019:12:12:19 +0800] "#;
        let result = ppl.parse_groups(0, &mut data).assert();
        assert_eq!(data, "");
        println!("{}", result);

        let mut data = r#"192.168.1.2 2002 - - [06/Aug/2019:12:12:19 +0800] "#;
        let result = ppl.parse_groups(0, &mut data).assert();
        assert_eq!(data, "");
        println!("{}", result);

        let mut data = r#" 2004 192.168.1.2 2002 - - [06/Aug/2019:12:12:19 +0800] "#;
        let result = ppl.parse_groups(0, &mut data).assert();
        assert_eq!(data, "");
        println!("{}", result);

        Ok(())
    }

    #[test]
    fn test_some_of_group2() -> AnyResult<()> {
        let express = wpl_express
            .parse(
                r#"some_of (
        json( symbol(可用磁盘空间kB)@name,@value:disk_free),
        json( symbol(磁盘使用百分比)@name,@value:disk_used),
        json( symbol(空闲CPU百分比)@name,@value:cpu_free),
        json( symbol(空闲内存kB)@name,@value:memory_free),
        json( symbol(1分钟平均CPU负载)@name,@value:cpu_used_by_one_min),
        json( symbol(15分钟平均CPU负载)@name,@value:cpu_used_by_fifty_min),
        json( symbol(系统启动进程个数)@name,@value:process),
        json( symbol(磁盘1分钟平均负载)@name,@value:disk_used_by_one_min),
        json( symbol(磁盘15分钟平均负载)@name,@value:dist_used_by_fifty_min) )\,"#,
            )
            .assert();

        let mut data = r#"{"name": "空闲CPU百分比", "value": 96.8}, {"name": "空闲内存kB", "value": 102432896.0}, {"name": "1分钟平均CPU负载", "value": 2.52}, {"name": "15分钟平均CPU负载", "value": 4.9}, {"name": "系统启动进程个数", "value": 1340.0}, {"name": "可用磁盘空间kB", "value": 40565575858.0}, {"name": "磁盘使用百分比", "value": 8.63}, {"name": "磁盘1分钟平均负载", "value": 8.63}, {"name": "磁盘15分钟平均负载", "value": 8.64}"#;
        let ppl = WplEvaluator::from(&express, None)?;
        let result = ppl.parse_groups(0, &mut data).assert();
        assert_eq!(data, "");
        println!("{}", result);
        Ok(())
    }

    #[test]
    fn test_some_of_group3() -> AnyResult<()> {
        let express = wpl_express
            .parse(r#"some_of(kv(chars@b:bbbb<[,]>),kv(chars@c:ccc),kv(chars@a:aaaa))\|"#)
            .assert();

        let mut data = r#"b=[y]|c=z|a=x"#;
        let ppl = WplEvaluator::from(&express, None)?;
        let result = ppl.parse_groups(0, &mut data).assert();
        assert_eq!(data, "");
        println!("{}", result);

        let express = wpl_express
            .parse(r#"some_of(kv(chars@b:bbbb<[,]>),kv(chars@c:ccc),kv(chars@a:aaaa))\|"#)
            .assert();

        let mut data = r#"c=z|a=x"#;
        let ppl = WplEvaluator::from(&express, None)?;
        let result = ppl.parse_groups(0, &mut data).assert();
        assert_eq!(data, "");
        println!("{}", result);
        Ok(())
    }

    #[test]
    fn test_some_of_group4() -> AnyResult<()> {
        let express = wpl_express
            .parse(r#"some_of(kv(chars<[,]>),chars)\|"#)
            .assert();

        let mut data = r#"b=[y]|c= |a=[x]"#;
        let ppl = WplEvaluator::from(&express, None)?;
        let result = ppl.parse_groups(0, &mut data).assert();
        assert_eq!(data, "");
        println!("{}", result);
        Ok(())
    }
}
