use crate::ast::WplPipe;
use crate::ast::group::{WplGroup, WplGroupType};
use crate::ast::{WplExpress, WplStatementType};
use crate::ast::{WplField, WplSep};
use crate::eval::builtins::{self, PipeLineResult, raw_to_utf8_string};
use crate::eval::runtime::field::FieldEvalUnit;
use crate::eval::runtime::field_pipe::PipeEnum;
use crate::eval::runtime::group::WplEvalGroup;
use std::borrow::Cow;
use wp_parse_api::{PipeHold, RawData, WparseError, WparseReason};

use crate::parser::error::{WplCodeError, WplCodeReason};
use crate::parser::wpl_rule::wpl_rule;
use anyhow::Result;
use orion_error::{ErrorWith, ToStructError, UvsDataFrom};
use orion_overload::new::New3;
use wp_log::debug_edata;
use wp_model_core::model::DataRecord;
use wp_parser::Parser;
use wp_parser::WResult as ModalResult;

// Internal DataResult for wp-lang usage
// Plugin developers should use wp_parse_api::DataResult instead
pub type DataResult = Result<(DataRecord, String), WparseError>;
pub const OPTIMIZE_TIMES: usize = 10000;

pub trait IntoRawData {
    fn into_raw(self) -> RawData;
}

impl IntoRawData for RawData {
    fn into_raw(self) -> RawData {
        self
    }
}

impl IntoRawData for String {
    fn into_raw(self) -> RawData {
        RawData::from_string(self)
    }
}

impl IntoRawData for &String {
    fn into_raw(self) -> RawData {
        RawData::from_string(self.as_str())
    }
}

impl IntoRawData for &str {
    fn into_raw(self) -> RawData {
        RawData::from_string(self)
    }
}

#[derive(Default, Clone)]
pub struct WplEvaluator {
    preorder: Vec<PipeHold>,
    group_units: Vec<WplEvalGroup>,
}
unsafe impl Send for WplEvaluator {}

impl WplEvaluator {
    pub fn preorder_proc(&self, data: RawData) -> Result<Vec<PipeLineResult>, WparseError> {
        let mut pipe_obj = Vec::new();
        let mut target = data;
        for proc_unit in &self.preorder {
            target = proc_unit.process(target)?;
            pipe_obj.push(PipeLineResult {
                name: proc_unit.name().to_string(),
                result: raw_to_utf8_string(&target),
            });
        }
        Ok(pipe_obj)
    }

    fn pipe_proc(&self, e_id: u64, data: RawData) -> Result<RawData, WparseError> {
        let mut target = data;
        for proc_unit in &self.preorder {
            target = proc_unit
                .process(target)
                .want("pipe convert")
                .with(e_id.to_string())
                .with(proc_unit.name())?;

            debug_edata!(
                e_id,
                "pipe  {}  out:{}",
                proc_unit.name(),
                raw_to_utf8_string(&target)
            );
        }
        Ok(target)
    }

    pub fn proc<D>(&self, e_id: u64, data: D, oth_suc_len: usize) -> DataResult
    where
        D: IntoRawData,
    {
        let mut working_raw: RawData = data.into_raw();
        if !self.preorder.is_empty() {
            working_raw = self.pipe_proc(e_id, working_raw)?;
        }

        let input_holder: Cow<'_, str> = match &working_raw {
            RawData::String(s) => Cow::Borrowed(s.as_str()),
            RawData::Bytes(b) => Cow::Owned(String::from_utf8_lossy(b).into_owned()),
            RawData::ArcBytes(b) => Cow::Owned(String::from_utf8_lossy(b).into_owned()),
        };
        let mut input: &str = input_holder.as_ref();

        let ori_len = input.len();
        match self.parse_groups(e_id, &mut input) {
            Ok(log) => Ok((log, input.to_string())),
            Err(e) => {
                let cur_pos = input.len();
                let pos = ori_len - cur_pos;
                if pos >= oth_suc_len {
                    Err(WparseReason::from_data(input.to_string(), Some(pos))
                        .to_err()
                        .with_detail(e.to_string()))
                } else {
                    Err(WparseError::from(WparseReason::NotMatch))
                }
            }
        }
    }
    pub fn from_code(code: &str) -> Result<Self, WplCodeError> {
        let mut cur_code = code;
        let rule = wpl_rule.parse_next(&mut cur_code).map_err(
            |err| {
                WplCodeReason::from_data(cur_code.to_string(), None)
                    .to_err()
                    .with_detail(err.to_string())
            }, //ParseCodeError::new(err.to_string())
        )?;
        let WplStatementType::Express(rule_define) = rule.statement;
        Self::from(&rule_define, None)
    }
    pub fn from(dy_lang: &WplExpress, inject: Option<&WplExpress>) -> Result<Self, WplCodeError> {
        let mut target_dpl = WplEvaluator {
            ..Default::default()
        };
        if let Some(inject) = inject {
            Self::assemble_ins(inject, &mut target_dpl)?;
        }
        Self::assemble_ins(dy_lang, &mut target_dpl)?;
        Ok(target_dpl)
    }

    fn assemble_ins(
        express: &WplExpress,
        target_dpl: &mut WplEvaluator,
    ) -> Result<(), WplCodeError> {
        builtins::ensure_builtin_pipe_units();
        for proc in &express.pipe_process {
            if let Some(pipe_unit) = builtins::registry::create_pipe_unit(proc) {
                target_dpl.preorder.push(pipe_unit);
            } else {
                return Err(WplCodeError::from(WplCodeReason::UnSupport(format!(
                    "Pipe processor '{}' not registered",
                    proc
                ))));
            }
        }
        for (i, group) in express.group.iter().enumerate() {
            let p_group = Self::assemble_group(i + 1, group)?;
            target_dpl.group_units.push(p_group);
        }
        Ok(())
    }
    fn assemble_group(index: usize, group: &WplGroup) -> Result<WplEvalGroup, WplCodeError> {
        let mut p_group =
            WplEvalGroup::new(index, group.meta.clone(), group.base_group_sep.clone());
        for (idx, conf) in group.fields.iter().enumerate() {
            let fpu = Self::assemble_fpu(idx + 1, conf, group.meta.clone())?;
            p_group.field_units.push(fpu)
        }
        Ok(p_group)
    }
    fn assemble_pipe(parent_idx: usize, pipe: &WplPipe) -> Result<PipeEnum, WplCodeError> {
        match pipe {
            WplPipe::Fun(fun) => Ok(PipeEnum::Fun(fun.clone())),
            WplPipe::Group(group) => {
                let mut p_group = WplEvalGroup::new(
                    parent_idx * 10,
                    group.meta.clone(),
                    group.base_group_sep.clone(),
                );
                for conf in &group.fields {
                    let fpu = Self::assemble_fpu(0, conf, group.meta.clone())?;
                    p_group.field_units.push(fpu)
                }
                Ok(PipeEnum::Group(p_group))
            }
        }
    }

    pub fn assemble_fpu(
        idx: usize,
        conf: &WplField,
        grp: WplGroupType,
    ) -> Result<FieldEvalUnit, WplCodeError> {
        let mut fpu = Self::build_fpu(idx, &grp, conf)?;
        if let Some(subs) = conf.sub_fields() {
            for (k, conf) in subs.conf_items().exact_iter() {
                let sub_fpu = Self::build_fpu(0, &grp, conf)?;
                fpu.add_sub_fpu(k.clone(), sub_fpu);
            }
            for (k, _, conf) in subs.conf_items().wild_iter() {
                let sub_fpu = Self::build_fpu(0, &grp, conf)?;
                fpu.add_sub_fpu(k.clone(), sub_fpu);
            }
        }
        Ok(fpu)
    }

    fn build_fpu(
        idx: usize,
        grp: &WplGroupType,
        conf: &WplField,
    ) -> Result<FieldEvalUnit, WplCodeError> {
        let mut fpu = FieldEvalUnit::create(idx, conf.clone(), grp.clone())?;
        for pipe_conf in conf.pipe.clone() {
            let pipe = Self::assemble_pipe(idx, &pipe_conf)?;
            fpu.add_pipe(pipe);
        }
        Ok(fpu)
    }
    //pub fn fields_proc(&self, data: &mut &str) -> WparseResult<DataRecord> {
    pub fn parse_groups(&self, e_id: u64, data: &mut &str) -> ModalResult<DataRecord> {
        let mut result = Vec::with_capacity(100);

        let sep = WplSep::default();
        for group_unit in self.group_units.iter() {
            match group_unit.proc(e_id, &sep, data, &mut result) {
                Ok(_) => {}
                Err(e) => {
                    return Err(e);
                }
            }
        }
        // Convert Vec<DataField> to Vec<FieldStorage>
        let storage_items: Vec<_> = result.into_iter()
            .map(|field| wp_model_core::model::FieldStorage::Owned(field))
            .collect();
        Ok(DataRecord::from(storage_items))
    }
}

pub struct StopWatch {
    continuous: bool,
    run_cnt: Option<usize>,
}

impl StopWatch {
    pub fn tag_used(&mut self) {
        if self.continuous
            && let Some(cnt) = &mut self.run_cnt
        {
            *cnt -= 1;
        }
    }
    pub fn is_stop(&self) -> bool {
        if !self.continuous {
            true
        } else {
            self.run_cnt == Some(0)
        }
    }
    pub fn allow_try(&self) -> bool {
        self.continuous && self.run_cnt.is_none()
    }
    pub fn new(continuous: bool, run_cnt: Option<usize>) -> Self {
        Self {
            continuous,
            run_cnt,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::fld_fmt::for_test::{fdc2, fdc2_1, fdc3, fdc4_1};
    use crate::ast::{WplField, WplFieldFmt};
    use crate::eval::builtins::raw_to_utf8_string;
    use crate::eval::runtime::vm_unit::WplEvaluator;
    use crate::eval::value::parse_def::Hold;
    use crate::types::AnyResult;
    use crate::{WparseResult, WplExpress, register_wpl_pipe};
    use orion_error::TestAssert;
    use orion_overload::new::New1;
    use smol_str::SmolStr;
    use wp_parse_api::{PipeProcessor, RawData};

    #[test]
    fn log_test_ty() -> AnyResult<()> {
        let mut data = r#"<158> May 15 14:19:16 skyeye SyslogClient[1]: 2023-05-15 14:19:16|10.180.8.8|alarm| {"_origin": 1}"#;

        let conf = WplExpress::new(vec![fdc3("auto", " ", true)?]);
        let ppl = WplEvaluator::from(&conf, None)?;

        let result = ppl.parse_groups(0, &mut data).assert();
        result.items.iter().for_each(|f| println!("{}", f));
        Ok(())
    }

    #[test]
    fn log_test_ips() -> AnyResult<()> {
        let conf = WplExpress::new(vec![fdc3("auto", " ", true)?]);
        let ppl = WplEvaluator::from(&conf, None)?;
        let mut data = r#"id=tos time="2023-05-15 09:11:53" fw=OS  pri=5 type=mgmt user=superman src=10.111.233.51 op="Modify pwd of manager" result=0 recorder=manager_so msg="null""#;
        let result = ppl.parse_groups(0, &mut data).assert();
        result.items.iter().for_each(|f| println!("{}", f));
        let mut data = r#"id=tos time="2023-05-15 09:11:53" fw=OS  pri=5 type=mgmt user=superman src=10.111.233.51 op="system admininfo modify name zhaolei new_password QXF5dW53ZWleMDIwNw== privilege config login_type local comment 安全管理员 add" result=0 recorder=config msg="nuid=tos time="2023-05-15 09:11:53" fw=OS  pri=5 type=mgmt user=superman src=10.111.233.51 op="webtr webadmin show" result=-1 recorder=config msg="error -8010 : 无效输入，分析" "#;
        let result = ppl.parse_groups(0, &mut data).assert();
        result.items.iter().for_each(|f| println!("{}", f));
        Ok(())
    }

    //59.x.x.x - - [06/Aug/2019:12:12:19 +0800] "GET /nginx-logo.png HTTP/1.1" 200 368 "http://119.x.x.x/" "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_14_5) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/75.0.3770.142 Safari/537.36" "-"
    #[test]
    fn log_test_nginx() -> AnyResult<()> {
        let conf = WplExpress::new(vec![fdc3("auto", " ", true)?]);
        let ppl = WplEvaluator::from(&conf, None)?;
        let mut data = r#"192.168.1.2 - - [06/Aug/2019:12:12:19 +0800] "GET /nginx-logo.png HTTP/1.1" 200 368 "http://119.122.1.4/" "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_14_5) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/75.0.3770.142 Safari/537.36" "-""#;

        let result = ppl.parse_groups(0, &mut data).assert();
        assert_eq!(data, "");
        result.items.iter().for_each(|f| println!("{}", f));
        Ok(())
    }

    #[test]
    fn test_huawei_default() -> AnyResult<()> {
        let mut data = r#"<190>May 15 2023 07:09:12 KM-KJY-DC-USG12004-B02 %%01POLICY/6/POLICYPERMIT(l):CID=0x814f041e;vsys=CSG_Security, protocol=6, source-ip=10.111.117.49, source-port=34616, destination-ip=10.111.48.230, destination-port=50051, time=2023/5/15 15:09:12, source-zone=untrust, destination-zone=trust, application-name=, line-name=HO202212080377705-1.%"#;

        let conf = WplExpress::new(vec![fdc3("auto", " ", true)?]);
        let ppl = WplEvaluator::from(&conf, None)?;
        let result = ppl.parse_groups(0, &mut data).assert();

        assert_eq!(data, "");
        result.items.iter().for_each(|f| println!("{}", f));
        Ok(())
    }

    #[test]
    fn test_huawei_detail() -> AnyResult<()> {
        //*auto chars: auto; *auto,
        let mut data = r#"<190>May 15 2023 07:09:12 KM-KJY-DC-USG12004-B02 %%01POLICY/6/POLICYPERMIT(l):CID=0x814f041e;vsys=CSG_Security, protocol=6"#;
        let fmt = WplFieldFmt {
            //separator: PrioSep::default(),
            scope_beg: Some("<".to_string()),
            scope_end: Some(">".to_string()),
            field_cnt: None,
            sub_fmt: None,
        };
        let conf = WplExpress::new(vec![
            fdc2_1("digit", fmt)?,
            fdc2("auto", " ")?,
            fdc2("chars", " ")?,
            fdc2("chars", ":")?,
            fdc2("kv", ";")?,
            fdc2("auto", ",")?,
            fdc2("auto", ",")?,
        ]);

        let ppl = WplEvaluator::from(&conf, None)?;
        let result = ppl.parse_groups(0, &mut data).assert();
        assert_eq!(data, "");
        result.items.iter().for_each(|f| println!("{}", f));
        Ok(())
    }

    #[test]
    fn test_huawei_simple() -> AnyResult<()> {
        //*auto chars: auto; *auto,
        let mut data = r#"<190>May 15 2023 07:09:12 KM-KJY-DC-USG12004-B02 %%01POLICY/6/POLICYPERMIT(l):CID=0x814f041e;vsys=CSG_Security, protocol=6"#;
        let conf = WplExpress::new(vec![
            fdc3("auto", " ", true)?,
            fdc2("chars", ":")?,
            fdc3("auto", ";", false)?,
            fdc3("auto", ",", true)?,
        ]);
        let ppl = WplEvaluator::from(&conf, None)?;
        let result = ppl.parse_groups(0, &mut data).assert();
        assert_eq!(data, "");
        result.items.iter().for_each(|f| println!("{:?}", f));
        Ok(())
    }

    #[test]
    fn test_huawei_simple2() -> AnyResult<()> {
        let mut data = r#"<190>May 15 2023 07:09:12 KM-KJY-DC-USG12004-B02 %%01POLICY/6/POLICYPERMIT(l):CID=0x814f041e;vsys=CSG_Security, protocol=6"#;
        let conf = WplExpress::new(vec![
            WplField::try_parse("symbol(<190>)[5]").assert(),
            fdc3("time", " ", false)?,
            WplField::try_parse("symbol(KM)[2]").assert(),
            fdc2("chars", ":")?,
            fdc3("auto", ";", false)?,
            fdc3("auto", ",", true)?,
        ]);
        let ppl = WplEvaluator::from(&conf, None)?;
        let result = ppl.parse_groups(0, &mut data).assert();
        assert_eq!(data, "");
        result.items.iter().for_each(|f| println!("{:?}", f));
        Ok(())
    }

    #[test]
    fn test_gen() -> AnyResult<()> {
        let mut data = r#"2345,2021-7-15 7:50:32,9OPP-MU-JME2-YGUW,chars_740,2022-1-18 19:30:30,jki=BkRzBo0f,138.11.13.43,tEu=GRcCwKkR,chars_493,Mrc=EskxskU3,sYp=jfKkn7th,UBa=eKhcfd9h,nXa=ZQSta6Je"#;
        let conf = WplExpress::new(vec![
            fdc3("digit", ",", false)?,
            fdc3("time", ",", false)?,
            fdc3("sn", ",", false)?,
            fdc3("chars", ",", false)?,
            fdc3("time", ",", false)?,
            fdc3("auto", ",", true)?,
        ]);
        let ppl = WplEvaluator::from(&conf, None)?;
        let result = ppl.parse_groups(0, &mut data).assert();
        assert_eq!(data, "");
        result.items.iter().for_each(|f| println!("{}", f));
        Ok(())
    }

    #[test]
    fn test_gen2() -> AnyResult<()> {
        let mut data = r#"7106,2020-6-10 2:54:9,U5BH-UC-UQVY-MMKU,chars_472,2020-9-22 13:4:6,Emm=LXJDV5DC,22.161.67.67,nsL=LvVRv5uf,chars_1534,DNw=0xCQKTaQ,UFh=dMPbabRG,q29=aMsZTj83,oUi=ywMsKT2G"#;
        let conf = WplExpress::new(vec![
            fdc3("digit", ",", false)?,
            fdc3("time", ",", false)?,
            fdc3("sn", ",", false)?,
            fdc3("chars", ",", false)?,
            fdc3("time", ",", false)?,
            fdc3("kv", ",", false)?,
            fdc3("ip", ",", false)?,
            fdc3("kv", ",", false)?,
            fdc3("chars", ",", false)?,
            fdc3("kv", ",", false)?,
            fdc3("kv", ",", false)?,
            fdc3("kv", ",", false)?,
            fdc3("kv", ",", false)?,
        ]);
        let ppl = WplEvaluator::from(&conf, None)?;
        let result = ppl.parse_groups(0, &mut data).assert();
        assert_eq!(data, "");
        result.items.iter().for_each(|f| println!("{}", f));

        let mut data = r#"1857,2021-4-10 0:46:8,R2IP-IF-06UT-7KUU,chars_1914,2021-4-15 2:19:43,u6s=TNSAlucV,228.211.38.109,k02=doYanSlf,chars_276,SIw=nu8atSqT,84e=e6qUb2k7,aVs=pk8M8rQU,5An=9upLU8aa"#;
        let result = ppl.parse_groups(0, &mut data).assert();
        assert_eq!(data, "");
        result.items.iter().for_each(|f| println!("{}", f));
        Ok(())
    }

    #[test]
    fn preorder_plg_pipe_unit_executes() -> AnyResult<()> {
        #[derive(Debug)]
        struct MockStage;

        impl PipeProcessor for MockStage {
            fn process(&self, data: RawData) -> WparseResult<RawData> {
                let mut value = raw_to_utf8_string(&data);
                value.push_str("-mock");
                Ok(RawData::from_string(value))
            }

            fn name(&self) -> &'static str {
                "mock_stage"
            }
        }

        register_wpl_pipe!("plg_pipe/MOCK-STAGE", || Hold::new(MockStage));

        let mut expr = WplExpress::new(vec![fdc3("auto", " ", true)?]);
        expr.pipe_process = vec![SmolStr::from("plg_pipe/MOCK-STAGE")];

        let evaluator = WplEvaluator::from(&expr, None)?;
        let results = evaluator.preorder_proc(RawData::from_string("data".to_string()))?;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].result, "data-mock");
        assert_eq!(results[0].name, "mock_stage");
        Ok(())
    }

    #[test]
    fn test_ignore() -> AnyResult<()> {
        let mut data = r#"2345,2021-7-15 7:50:32,9OPP-MU-JME2-YGUW,chars_740"#;
        let conf = WplExpress::new(vec![
            fdc3("_", ",", false)?,
            fdc3("_", ",", false)?,
            fdc3("_", ",", false)?,
            fdc3("_", ",", false)?,
        ]);
        let ppl = WplEvaluator::from(&conf, None)?;
        let result = ppl.parse_groups(0, &mut data).assert();
        assert_eq!(data, "");
        result.items.iter().for_each(|f| println!("{}", f));
        Ok(())
    }

    #[test]
    fn test_ignore_cnt() -> AnyResult<()> {
        let mut data = r#"2345,2021-7-15 7:50:32,9OPP-MU-JME2-YGUW,chars_740"#;
        let conf = WplExpress::new(vec![fdc4_1("_", ",", true, 4)?]);
        let ppl = WplEvaluator::from(&conf, None)?;
        let result = ppl.parse_groups(0, &mut data).assert();
        assert_eq!(data, "");
        assert_eq!(result.items.len(), 4);
        result.items.iter().for_each(|f| println!("{}", f));

        let mut data = r#"2345,2021-7-15 7:50:32,9OPP-MU-JME2-YGUW,chars_740"#;
        let conf = WplExpress::new(vec![fdc4_1("_", ",", true, 3)?]);
        let ppl = WplEvaluator::from(&conf, None)?;
        let result = ppl.parse_groups(0, &mut data).assert();
        assert_eq!(data, "chars_740");
        assert_eq!(result.items.len(), 3);
        Ok(())
    }

    #[test]
    fn test_pipe_unit_direct_lookup() -> AnyResult<()> {
        use crate::eval::builtins::raw_to_utf8_string;
        use crate::{create_preorder_pipe_unit, list_preorder_pipe_units};

        // Define MockStage for this test
        #[derive(Debug)]
        struct TestMockStage;

        impl PipeProcessor for TestMockStage {
            fn process(&self, data: RawData) -> WparseResult<RawData> {
                let mut value = raw_to_utf8_string(&data);
                value.push_str("-mock");
                Ok(RawData::from_string(value))
            }

            fn name(&self) -> &'static str {
                "test_mock_stage"
            }
        }

        // Test direct lookup after simplification
        // Register test processors with different naming schemes
        register_wpl_pipe!("direct-test", || Hold::new(TestMockStage));
        register_wpl_pipe!("plg_pipe/mock-prefix", || Hold::new(TestMockStage));

        // Test that direct naming works
        let processor = create_preorder_pipe_unit("direct-test");
        assert!(processor.is_some(), "Should find direct-test processor");

        // Test that prefixed registration also works (with full name)
        let processor = create_preorder_pipe_unit("plg_pipe/mock-prefix");
        assert!(
            processor.is_some(),
            "Should find plg_pipe/with-prefix processor"
        );

        // Test actual processing with different naming
        let test_data = RawData::from_string("hello".to_string());

        if let Some(processor) = create_preorder_pipe_unit("direct-test") {
            let result = processor.process(test_data.clone())?;
            assert_eq!(raw_to_utf8_string(&result), "hello-mock");
        }

        if let Some(processor) = create_preorder_pipe_unit("plg_pipe/mock-prefix") {
            let result = processor.process(test_data)?;
            assert_eq!(raw_to_utf8_string(&result), "hello-mock");
        }

        // List all processors to see what's registered
        let processors = list_preorder_pipe_units();
        println!("All registered processors: {:?}", processors);

        // Verify both naming approaches work (names are converted to uppercase)
        assert!(processors.contains(&"DIRECT-TEST".into()));
        assert!(processors.contains(&"PLG_PIPE/MOCK-PREFIX".into()));

        Ok(())
    }

    #[test]
    fn test_simplified_assemble_ins_logic() -> AnyResult<()> {
        use crate::eval::builtins::raw_to_utf8_string;
        use crate::{create_preorder_pipe_unit, list_preorder_pipe_units};

        // Define test processor
        #[derive(Debug)]
        struct SimplifiedTestStage;

        impl PipeProcessor for SimplifiedTestStage {
            fn process(&self, data: RawData) -> WparseResult<RawData> {
                let mut value = raw_to_utf8_string(&data);
                value.push_str("-simplified");
                Ok(RawData::from_string(value))
            }

            fn name(&self) -> &'static str {
                "simplified_test"
            }
        }

        // Register processors with both naming styles
        register_wpl_pipe!("simple-test", || Hold::new(SimplifiedTestStage));
        register_wpl_pipe!("plg_pipe/simple-prefix", || Hold::new(SimplifiedTestStage));

        // Test that both can be found directly
        let processor1 = create_preorder_pipe_unit("simple-test");
        assert!(processor1.is_some(), "Should find simple-test");

        let processor2 = create_preorder_pipe_unit("plg_pipe/simple-prefix");
        assert!(processor2.is_some(), "Should find plg_pipe/with-prefix");

        // Test that processors registered with plg_pipe/ prefix can be found without it
        // This would fail because registration is case-sensitive and stores full name
        let processor3 = create_preorder_pipe_unit("simple-prefix");
        assert!(
            processor3.is_none(),
            "Should NOT find with-prefix (was registered as plg_pipe/with-prefix)"
        );

        // Show all registered processors
        let processors = list_preorder_pipe_units();
        println!("All processors: {:?}", processors);

        Ok(())
    }
}
