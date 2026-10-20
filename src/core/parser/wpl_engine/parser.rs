//! 单个数据包解析逻辑

use super::types::ProcessResult;
use crate::core::parser::wpl_engine::pipeline::WplPipeline;
use crate::{core::parser::ParseOption, stat::MonSend};
use orion_conf::ToStructError;
use orion_error::UnifiedReason;
use std::sync::Arc;
use wp_connector_api::SourceEvent;
use wp_model_core::model::DataRecord;
use wp_model_core::model::data::Field;
use wpl::{WparseError, WparseReason, WparseResult};

/// 数据包解析器
#[derive(Clone, getset::Getters)]
#[get = "pub"]
pub struct MultiParser {
    pipelines: Vec<WplPipeline>,
}

impl MultiParser {
    /// 装配期固定分层：普通规则在前、`#[late]` 规则在后，层内保持声明序。
    pub fn new(mut pipelines: Vec<WplPipeline>) -> Self {
        // 装配期只做固定分层：`#[late]` 规则排在最后，层内保持声明序。
        // 否则首个优化窗口（OPTIMIZE_TIMES 条事件）之前，顺序仍由文件声明顺序决定。
        pipelines.sort_by_key(WplPipeline::initial_order_key);
        Self { pipelines }
    }

    /// 处理单个事件
    pub fn parse_event(&mut self, event: &SourceEvent, setting: &ParseOption) -> ProcessResult {
        let mut max_depth = 0;
        let mut best_idx: Option<usize> = None;
        let mut best_error = None;
        let rule_cnt = self.pipelines.len();

        // 尝试用每个规则处理事件
        for (idx, wpl_line) in self.pipelines.iter_mut().enumerate() {
            // #[no_match] rule 不参与自动匹配（仅作 copy_event_parse 等的调用目标）
            if !wpl_line.auto_match {
                continue;
            }
            let is_last = idx == rule_cnt - 1;

            // 调用 WPL 处理
            match wpl_line.proc(event, max_depth) {
                Ok((mut tdo_crate, un_parsed, side_records)) => {
                    stamp_event_meta(&mut tdo_crate, event, setting);
                    // 旁路 record 也盖事件 meta，并按 ProcessResult 需要 String 化 key
                    let side_records: Vec<(String, DataRecord)> = side_records
                        .into_iter()
                        .map(|(k, mut r)| {
                            stamp_event_meta(&mut r, event, setting);
                            (k.to_string(), r)
                        })
                        .collect();
                    wpl_line.hit_cnt += 1;

                    let wpl_key = wpl_line.wpl_key().to_string();

                    // 根据是否有残留数据返回不同的结果
                    if un_parsed.is_empty() {
                        let record = Arc::new(tdo_crate);
                        info_edata!(event.event_id, "wpl parse suc! wpl:{} ", wpl_key,);
                        return ProcessResult::Success {
                            wpl_key,
                            record,
                            side_records,
                        };
                    } else {
                        let parsed_len = event.payload.len() - un_parsed.len();
                        if un_parsed.len() * 5 > event.payload.len() {
                            info_edata!(event.event_id, "wpl parse not complete: {}", wpl_key,);
                            if parsed_len > max_depth {
                                max_depth = parsed_len;
                                best_idx = Some(idx);
                                best_error = Some(WparseReason::data_error().to_err());
                            }
                        } else {
                            let record = Arc::new(tdo_crate);
                            return ProcessResult::Partial {
                                wpl_key,
                                record,
                                residue: un_parsed.to_string(),
                                side_records,
                            };
                        }
                    }
                }
                Err(e) => {
                    // 记录解析深度最高的错误
                    if matches!(e.reason(), WparseReason::Uvs(UnifiedReason::DataError)) {
                        best_idx = Some(idx);
                        best_error = Some(e);
                        if max_depth == 0 {
                            // 当底层解析器未返回显式消费深度时，至少记录"已进入规则尝试"。
                            max_depth = 1;
                        }
                        debug_edata!(event.event_id, "wpl parse fail: {}", wpl_line.wpl_key());
                    } else if best_error.is_none() {
                        // 如果不是 DataError，作为备选记录第一个错误
                        best_idx = Some(idx);
                        best_error = Some(e);
                        break;
                    }

                    if is_last {
                        break;
                    }
                }
            }
        }

        // 所有规则都失败，返回深度最高的失败信息
        let best_wpl = best_idx.map_or(String::new(), |i| self.pipelines[i].wpl_key().to_string());
        let best_error = best_error
            .unwrap_or_else(|| WparseError::from(WparseReason::Uvs(UnifiedReason::system_error())));
        ProcessResult::Miss(super::types::ParseFailInfo::new(
            best_wpl, best_error, max_depth,
        ))
    }

    pub fn stop(&mut self) {
        self.pipelines.iter_mut().for_each(|i| i.stop());
    }

    /// 按命中次数重排自动匹配顺序。
    ///
    /// 排序键为 `(late, hit_cnt desc, index asc)`：普通规则永远在 `#[late]` 规则之前，
    /// 命中次数只在同一层内生效。排序后清空本窗口的命中计数。
    pub fn optimized(&mut self, _count: usize) {
        if self.pipelines.is_empty() {
            return;
        }

        self.pipelines.sort_by_key(WplPipeline::match_order_key);

        // 下一窗口重新记录命中情况
        for pipeline in &mut self.pipelines {
            pipeline.hit_cnt = 0;
        }
    }

    /// 更新规则命中计数并排序
    pub fn hit_count_sort(&mut self) {
        for x in &mut self.pipelines {
            x.access_cnt = 0;
        }
    }
    pub async fn send_stat(&mut self, mon_send: &MonSend) -> WparseResult<()> {
        for i in self.pipelines.iter_mut() {
            i.send_stat(mon_send).await?;
        }
        Ok(())
    }
}

/// 给 record 盖事件级 meta：wp_event_id / wp_src_key / wp_src_ip / wp_event_md5。
/// 由 gen_msg_id 总开关 + gen_event_md5 子开关控制；主 record 与旁路 record 都走此路径。
fn stamp_event_meta(record: &mut DataRecord, event: &SourceEvent, setting: &ParseOption) {
    if *setting.gen_msg_id() {
        record.set_id(event.event_id);
        record.append(Field::from_chars("wp_src_key", event.src_key.as_str()));
        if let Some(ups_ip) = event.ups_ip {
            record.append(Field::from_ip("wp_src_ip", ups_ip));
        }
        if *setting.gen_event_md5() {
            let digest = format!("{:x}", md5::compute(event.payload.as_bytes()));
            record.append(Field::from_chars("wp_event_md5", digest));
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::sinks::SinkGroupAgent;
    use wpl::WplEvaluator;

    fn dummy_pipeline(idx: usize, hit: usize) -> WplPipeline {
        let evaluator = WplEvaluator::from_code("rule dummy { ( _ ) }").expect("build wpl");
        let parser = evaluator;
        let mut pipeline = WplPipeline::new(
            idx,
            format!("rule-{}", idx),
            "pkg".to_string(),
            format!("rule-{}", idx),
            Vec::new(),
            parser,
            vec![SinkGroupAgent::null()],
            Vec::new(),
        );
        pipeline.hit_cnt = hit;
        pipeline
    }

    fn pipeline_with(idx: usize, code: &str, late: bool) -> WplPipeline {
        let mut pipeline = WplPipeline::new(
            idx,
            format!("rule-{}", idx),
            "pkg".to_string(),
            format!("rule-{}", idx),
            Vec::new(),
            WplEvaluator::from_code(code).expect("build wpl"),
            vec![SinkGroupAgent::null()],
            Vec::new(),
        );
        pipeline.late = late;
        pipeline
    }

    fn order(parser: &MultiParser) -> Vec<String> {
        parser
            .pipelines
            .iter()
            .map(|p| p.wpl_key().to_string())
            .collect()
    }

    fn event(payload: &str) -> SourceEvent {
        SourceEvent::new(
            1,
            "test-src",
            wp_model_core::raw::RawData::String(payload.to_string()),
            std::sync::Arc::new(wp_connector_api::Tags::new()),
        )
    }

    #[test]
    fn optimized_reorders_by_hit_count() {
        let pipelines = vec![
            dummy_pipeline(0, 1),
            dummy_pipeline(1, 5),
            dummy_pipeline(2, 3),
        ];

        let mut parser = MultiParser::new(pipelines);
        parser.optimized(0);

        assert_eq!(order(&parser), vec!["rule-1", "rule-2", "rule-0"]);
        assert!(parser.pipelines.iter().all(|p| p.hit_cnt == 0));
    }

    #[test]
    fn late_rule_sorted_last_at_construction() {
        let pipelines = vec![
            pipeline_with(0, "rule a { (chars) }", true),
            pipeline_with(1, "rule b { (chars) }", false),
            pipeline_with(2, "rule c { (chars) }", false),
        ];

        // 装配期就完成分层，不依赖首个优化窗口
        let parser = MultiParser::new(pipelines);
        assert_eq!(order(&parser), vec!["rule-1", "rule-2", "rule-0"]);
    }

    #[test]
    fn optimized_never_promotes_late_rule_above_normal() {
        let mut late = pipeline_with(0, "rule late { (chars) }", true);
        late.hit_cnt = 10_000;
        let pipelines = vec![
            late,
            pipeline_with(1, "rule a { (chars) }", false),
            pipeline_with(2, "rule b { (chars) }", false),
        ];

        let mut parser = MultiParser::new(pipelines);
        parser.optimized(0);

        // 命中次数再高也不能跨层提升
        assert_eq!(order(&parser), vec!["rule-1", "rule-2", "rule-0"]);
    }

    #[test]
    fn late_rules_sorted_by_hit_count_within_tier() {
        let mut a = pipeline_with(0, "rule a { (chars) }", true);
        a.hit_cnt = 1;
        let mut b = pipeline_with(1, "rule b { (chars) }", true);
        b.hit_cnt = 5;
        let pipelines = vec![a, b, pipeline_with(2, "rule c { (chars) }", false)];

        let mut parser = MultiParser::new(pipelines);
        parser.optimized(0);

        // 普通规则在前；late 层内仍按命中次数降序
        assert_eq!(order(&parser), vec!["rule-2", "rule-1", "rule-0"]);
    }

    #[test]
    fn late_rule_matches_when_normal_rules_miss() {
        let pipelines = vec![
            pipeline_with(0, "rule strict { (digit) }", false),
            pipeline_with(1, "rule catch_all { (chars\\0) }", true),
        ];
        let mut parser = MultiParser::new(pipelines);
        let setting = ParseOption::new(false, false, Vec::new());

        match parser.parse_event(&event("abc"), &setting) {
            ProcessResult::Success { wpl_key, .. } => assert_eq!(wpl_key, "rule-1"),
            other => panic!("expected deferred rule to match, got {:?}", other),
        }
    }

    #[test]
    fn no_match_takes_precedence_over_late() {
        // #[no_match] 优先：即使命中能力最强也不会参与自动匹配
        let mut catch_all = pipeline_with(1, "rule catch_all { (chars\\0) }", true);
        catch_all.auto_match = false;
        let pipelines = vec![
            pipeline_with(0, "rule strict { (digit) }", false),
            catch_all,
        ];
        let mut parser = MultiParser::new(pipelines);
        let setting = ParseOption::new(false, false, Vec::new());

        match parser.parse_event(&event("abc"), &setting) {
            ProcessResult::Miss(_) => {}
            other => panic!("no_match rule must be skipped, got {:?}", other),
        }
    }

    #[test]
    fn normal_rule_wins_over_late_when_both_match() {
        // late 规则声明在前，但装配期分层后必须让普通规则先尝试
        let pipelines = vec![
            pipeline_with(0, "rule catch_all { (chars\\0) }", true),
            pipeline_with(1, "rule normal { (chars\\0) }", false),
        ];
        let mut parser = MultiParser::new(pipelines);
        assert_eq!(order(&parser), vec!["rule-1", "rule-0"]);

        let setting = ParseOption::new(false, false, Vec::new());
        match parser.parse_event(&event("abc"), &setting) {
            ProcessResult::Success { wpl_key, .. } => assert_eq!(wpl_key, "rule-1"),
            other => panic!("normal rule should win, got {:?}", other),
        }
    }

    #[test]
    fn construction_ignores_hit_counts() {
        // 装配期只做分层 + 声明序，hit_cnt 不应参与（此时还没有命中数据）
        let mut a = pipeline_with(0, "rule a { (chars) }", false);
        a.hit_cnt = 1;
        let mut b = pipeline_with(1, "rule b { (chars) }", false);
        b.hit_cnt = 5;

        let parser = MultiParser::new(vec![a, b]);
        assert_eq!(order(&parser), vec!["rule-0", "rule-1"]);
    }

    #[test]
    fn late_rules_with_equal_hits_keep_declaration_order() {
        let pipelines = vec![
            pipeline_with(0, "rule a { (chars) }", true),
            pipeline_with(1, "rule b { (chars) }", true),
        ];
        let mut parser = MultiParser::new(pipelines);
        parser.optimized(0);

        assert_eq!(order(&parser), vec!["rule-0", "rule-1"]);
    }

    #[test]
    fn optimized_is_idempotent_for_late_tier() {
        let mut late = pipeline_with(0, "rule late { (chars) }", true);
        late.hit_cnt = 3;
        let pipelines = vec![
            late,
            pipeline_with(1, "rule a { (chars) }", false),
            pipeline_with(2, "rule b { (chars) }", false),
        ];
        let mut parser = MultiParser::new(pipelines);

        parser.optimized(0);
        let first = order(&parser);
        parser.optimized(0);
        let second = order(&parser);

        assert_eq!(first, second);
        assert_eq!(first, vec!["rule-1", "rule-2", "rule-0"]);
    }

    #[test]
    fn optimized_on_empty_parser_is_noop() {
        let mut parser = MultiParser::new(Vec::new());
        parser.optimized(0);
        assert!(order(&parser).is_empty());
    }
}

// 重新导出主要类型
