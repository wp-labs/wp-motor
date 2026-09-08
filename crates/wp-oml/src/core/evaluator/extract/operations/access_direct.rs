use super::operand_target;
use crate::core::AsyncFieldExtractor;
use crate::core::diagnostics::{self, OmlIssue, OmlIssueKind};
use crate::core::prelude::*;
use crate::language::{AccessDirectOperation, EvaluationTarget};
use async_trait::async_trait;
use std::net::IpAddr;
use wp_knowledge::intranet_nets::is_intranet;
use wp_model_core::model::{DataField, FieldStorage, Value};

/// IP 探测结果：区分「正常缺失（静默）」与「非空非法（诊断）」
#[derive(Clone, Copy)]
pub(crate) enum IpProbe {
    /// 合法 IP
    Ok(IpAddr),
    /// 正常缺失：字段不存在 / Null / Ignore / 空或纯空白字符串
    /// —— 属预期数据缺失，通常由 `on_fail` 兜底，不应告警
    Missing,
    /// 非空但非法（含类型不匹配）：真实输入错误，保留 WARN + ParseFail 诊断
    Invalid,
}

/// 从字段提取 IpAddr（`Value::IpAddr` 或合法 IP 字符串），
/// 并区分「正常缺失（静默 Ignore）」与「非空非法输入（诊断）」
pub(crate) fn probe_ip(field: Option<&DataField>) -> IpProbe {
    let Some(field) = field else {
        return IpProbe::Missing;
    };
    match field.get_value() {
        Value::IpAddr(ip) => IpProbe::Ok(*ip),
        Value::Chars(s) => {
            let s = s.trim();
            if s.is_empty() {
                IpProbe::Missing
            } else {
                match s.parse::<IpAddr>() {
                    Ok(ip) => IpProbe::Ok(ip),
                    Err(_) => IpProbe::Invalid,
                }
            }
        }
        Value::Null | Value::Ignore(_) => IpProbe::Missing,
        _ => IpProbe::Invalid,
    }
}

/// 根据 src/dst 的内外归属组合通信方向
fn access_direction_str(src: &IpAddr, dst: &IpAddr) -> &'static str {
    match (is_intranet(src), is_intranet(dst)) {
        (true, true) => "L2L",
        (true, false) => "L2W",
        (false, true) => "W2L",
        (false, false) => "W2W",
    }
}

impl AccessDirectOperation {
    pub(crate) fn extract_one(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        let src_probe = probe_ip(
            self.src()
                .extract_one(&operand_target(self.src(), target), src, dst)
                .as_ref(),
        );
        let dst_probe = probe_ip(
            self.dst()
                .extract_one(&operand_target(self.dst(), target), src, dst)
                .as_ref(),
        );

        match (src_probe, dst_probe) {
            (IpProbe::Ok(s), IpProbe::Ok(d)) => Some(DataField::from_chars(
                target.safe_name(),
                access_direction_str(&s, &d),
            )),
            _ => {
                // 仅「非空非法输入」才告警/诊断；正常缺失（字段缺失/Null/Ignore/空串）
                // 属预期场景，静默返回 Ignore 交由 on_fail 兜底，避免批量日志噪声
                let invalid =
                    matches!(src_probe, IpProbe::Invalid) || matches!(dst_probe, IpProbe::Invalid);
                if invalid {
                    let detail = "access_direct: src/dst ip invalid (non-empty non-ip input)";
                    warn_data!("{}", detail);
                    diagnostics::push(OmlIssue::new(OmlIssueKind::ParseFail, detail.to_string()));
                }
                Some(DataField::from_ignore(target.safe_name()))
            }
        }
    }

    pub(crate) fn extract_storage(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<FieldStorage> {
        self.extract_one(target, src, dst)
            .map(FieldStorage::from_owned)
    }

    pub(crate) fn extract_more(
        &self,
        _src: &mut DataRecordRef<'_>,
        _dst: &mut DataRecord,
        _cache: &mut FieldQueryCache,
    ) -> Vec<DataField> {
        Vec::new()
    }

    pub(crate) fn support_batch(&self) -> bool {
        false
    }
}

#[async_trait]
impl AsyncFieldExtractor for AccessDirectOperation {
    async fn extract_one_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<DataField> {
        self.extract_one(target, src, dst)
    }

    async fn extract_storage_async(
        &self,
        target: &EvaluationTarget,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
    ) -> Option<FieldStorage> {
        self.extract_storage(target, src, dst)
    }

    async fn extract_more_async(
        &self,
        src: &mut DataRecordRef<'_>,
        dst: &mut DataRecord,
        cache: &mut FieldQueryCache,
    ) -> Vec<DataField> {
        self.extract_more(src, dst, cache)
    }
}

#[cfg(test)]
mod tests {
    use crate::core::AsyncDataTransformer;
    use crate::parser::oml_parse_raw;
    use orion_error::dev::testing::TestAssert;
    use std::net::{IpAddr, Ipv4Addr};
    use wp_knowledge::cache::FieldQueryCache;
    use wp_model_core::model::{DataField, DataRecord, FieldStorage};

    const PRIVATE: [u8; 4] = [10, 0, 0, 1];
    const PUBLIC: [u8; 4] = [8, 8, 8, 8];

    async fn run_access_direct(sip: IpAddr, dip: IpAddr) -> String {
        let cache = &mut FieldQueryCache::default();
        let data = vec![
            FieldStorage::from_owned(DataField::from_ip("sip", sip)),
            FieldStorage::from_owned(DataField::from_ip("dip", dip)),
        ];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  access_direct(read(sip), read(dip)) ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        target
            .field("X")
            .expect("X field")
            .as_field()
            .get_value()
            .to_string()
    }

    fn v4(octets: [u8; 4]) -> IpAddr {
        IpAddr::V4(Ipv4Addr::from(octets))
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_access_direct_in_to_in() {
        assert_eq!(run_access_direct(v4(PRIVATE), v4(PRIVATE)).await, "L2L");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_access_direct_in_to_out() {
        assert_eq!(run_access_direct(v4(PRIVATE), v4(PUBLIC)).await, "L2W");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_access_direct_out_to_in() {
        assert_eq!(run_access_direct(v4(PUBLIC), v4(PRIVATE)).await, "W2L");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_access_direct_out_to_out() {
        assert_eq!(run_access_direct(v4(PUBLIC), v4(PUBLIC)).await, "W2W");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_access_direct_ipv6_combination() {
        // IPv6 ULA → 公网 = 内到外
        assert_eq!(
            run_access_direct(IpAddr::from_str("fc00::1").unwrap(), v4(PUBLIC)).await,
            "L2W"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_access_direct_missing_field_becomes_ignore() {
        let cache = &mut FieldQueryCache::default();
        // 只有 sip，没有 dip
        let data = vec![FieldStorage::from_owned(DataField::from_ip(
            "sip",
            v4(PRIVATE),
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  access_direct(read(sip), read(dip)) ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let out = target.field("X").expect("X field").as_field();
        assert!(
            matches!(out.get_value(), wp_model_core::model::Value::Ignore(_)),
            "缺失字段应输出 Ignore, got {:?}",
            out.get_value()
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_access_direct_pipe_on_fail() {
        let cache = &mut FieldQueryCache::default();
        // 只有 sip，没有 dip → access_direct 输出 Ignore → on_fail 替换为 "unknown"
        let data = vec![FieldStorage::from_owned(DataField::from_ip(
            "sip",
            v4(PRIVATE),
        ))];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  access_direct(read(sip), read(dip)) | on_fail('unknown') ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = DataField::from_chars("X".to_string(), "unknown");
        assert_eq!(target.field("X").map(|s| s.as_field()), Some(&expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_access_direct_pipe_on_fail_empty_string() {
        let cache = &mut FieldQueryCache::default();
        // 非法 IP → Ignore → on_fail('') 替换为空串
        let data = vec![
            FieldStorage::from_owned(DataField::from_chars("sip", "not-an-ip")),
            FieldStorage::from_owned(DataField::from_ip("dip", v4(PUBLIC))),
        ];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  access_direct(read(sip), read(dip)) | on_fail('') ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = DataField::from_chars("X".to_string(), "");
        assert_eq!(target.field("X").map(|s| s.as_field()), Some(&expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_access_direct_pipe_on_fail_keeps_success() {
        let cache = &mut FieldQueryCache::default();
        // 私网→公网 = "L2W"，on_fail 不干预成功结果
        let data = vec![
            FieldStorage::from_owned(DataField::from_ip("sip", v4(PRIVATE))),
            FieldStorage::from_owned(DataField::from_ip("dip", v4(PUBLIC))),
        ];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  access_direct(read(sip), read(dip)) | on_fail('unknown') ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = DataField::from_chars("X".to_string(), "L2W");
        assert_eq!(target.field("X").map(|s| s.as_field()), Some(&expect));
    }

    // ---- issue #360：正常缺失（空串/Null/Ignore/字段缺失）静默返回 Ignore，不告警不诊断 ----

    #[tokio::test(flavor = "current_thread")]
    async fn test_access_direct_empty_string_src_becomes_ignore() {
        // 空串/纯空白属于正常缺失：结果 Ignore（由 on_fail 兜底），不产生诊断
        let cache = &mut FieldQueryCache::default();
        let data = vec![
            FieldStorage::from_owned(DataField::from_chars("sip", "   ")),
            FieldStorage::from_owned(DataField::from_ip("dip", v4(PUBLIC))),
        ];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  access_direct(read(sip), read(dip)) ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let out = target.field("X").expect("X field").as_field();
        assert!(
            matches!(out.get_value(), wp_model_core::model::Value::Ignore(_)),
            "空串应输出 Ignore, got {:?}",
            out.get_value()
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_access_direct_empty_string_src_on_fail_unknown() {
        // 空串 → Ignore → on_fail('unknown') 返回 unknown，无副作用
        let cache = &mut FieldQueryCache::default();
        let data = vec![
            FieldStorage::from_owned(DataField::from_chars("sip", "")),
            FieldStorage::from_owned(DataField::from_ip("dip", v4(PUBLIC))),
        ];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  access_direct(read(sip), read(dip)) | on_fail('unknown') ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let expect = DataField::from_chars("X".to_string(), "unknown");
        assert_eq!(target.field("X").map(|s| s.as_field()), Some(&expect));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_access_direct_null_ip_becomes_ignore() {
        // IP 类型空值（conv 层对空串转换产物 Value::Null）同样静默 Ignore
        let cache = &mut FieldQueryCache::default();
        let data = vec![
            FieldStorage::from_owned(DataField::new(
                wp_model_core::model::DataType::IP,
                "sip".to_string(),
                wp_model_core::model::Value::Null,
            )),
            FieldStorage::from_owned(DataField::from_ip("dip", v4(PUBLIC))),
        ];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  access_direct(read(sip), read(dip)) ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let out = target.field("X").expect("X field").as_field();
        assert!(
            matches!(out.get_value(), wp_model_core::model::Value::Ignore(_)),
            "Null IP 应输出 Ignore, got {:?}",
            out.get_value()
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_access_direct_ignore_field_becomes_ignore() {
        // Ignore 字段属正常缺失，静默透传为 Ignore
        let cache = &mut FieldQueryCache::default();
        let data = vec![
            FieldStorage::from_owned(DataField::from_ignore("sip")),
            FieldStorage::from_owned(DataField::from_ip("dip", v4(PUBLIC))),
        ];
        let src = DataRecord::from(data);

        let mut conf = r#"
        name : test
        ---
        X  =  access_direct(read(sip), read(dip)) ;
         "#;
        let model = oml_parse_raw(&mut conf).await.assert();
        let target = model.transform_async(src, cache).await;
        let out = target.field("X").expect("X field").as_field();
        assert!(
            matches!(out.get_value(), wp_model_core::model::Value::Ignore(_)),
            "Ignore 字段应输出 Ignore, got {:?}",
            out.get_value()
        );
    }

    #[cfg(feature = "oml-diag")]
    mod diag_tests {
        use super::*;

        #[tokio::test(flavor = "current_thread")]
        async fn test_access_direct_missing_is_silent_no_issue() {
            // 正常缺失（缺 dip 字段）→ 无 ParseFail 诊断
            crate::core::diagnostics::reset();
            let cache = &mut FieldQueryCache::default();
            let data = vec![FieldStorage::from_owned(DataField::from_ip(
                "sip",
                v4(PRIVATE),
            ))];
            let src = DataRecord::from(data);

            let mut conf = r#"
            name : test
            ---
            X  =  access_direct(read(sip), read(dip)) | on_fail('unknown') ;
             "#;
            let model = oml_parse_raw(&mut conf).await.assert();
            let _ = model.transform_async(src, cache).await;
            let issues = crate::core::diagnostics::take();
            assert!(issues.is_empty(), "正常缺失不应产生诊断, got {:?}", issues);
        }

        #[tokio::test(flavor = "current_thread")]
        async fn test_access_direct_invalid_input_emits_issue() {
            // 非空非法字符串：真实输入错误，保留 ParseFail 诊断
            crate::core::diagnostics::reset();
            let cache = &mut FieldQueryCache::default();
            let data = vec![
                FieldStorage::from_owned(DataField::from_chars("sip", "not-an-ip")),
                FieldStorage::from_owned(DataField::from_ip("dip", v4(PUBLIC))),
            ];
            let src = DataRecord::from(data);

            let mut conf = r#"
            name : test
            ---
            X  =  access_direct(read(sip), read(dip)) | on_fail('unknown') ;
             "#;
            let model = oml_parse_raw(&mut conf).await.assert();
            let _ = model.transform_async(src, cache).await;
            let issues = crate::core::diagnostics::take();
            assert!(
                issues
                    .iter()
                    .any(|i| i.kind == crate::core::diagnostics::OmlIssueKind::ParseFail),
                "非法输入应产生 ParseFail 诊断, got {:?}",
                issues
            );
        }
    }

    use std::str::FromStr;
}
