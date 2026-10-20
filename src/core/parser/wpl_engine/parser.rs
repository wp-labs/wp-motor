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
use wpl::{ParseProgress, SideRecords, WparseError, WparseReason, WparseResult};

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
        let mut best_idx: Option<usize> = None;
        let mut best_progress = ParseProgress::default();
        let mut best_error = None;
        let rule_cnt = self.pipelines.len();

        // 尝试用每个规则处理事件
        for (idx, wpl_line) in self.pipelines.iter_mut().enumerate() {
            // #[no_match] rule 不参与自动匹配（仅作 copy_event_parse 等的调用目标）
            if !wpl_line.auto_match {
                continue;
            }
            let is_last = idx == rule_cnt - 1;

            // 调用 WPL 处理；`progress` 是本次尝试走到了输入的哪个位置。
            let (parsed, progress) = wpl_line.proc(event);

            // Miss 归因只接受「严格更靠后」的尝试：同进度保留先到的候选。
            // 规则按命中率排序、`#[late]` 排在最后，如果同进度也覆盖归因，
            // 一次全部失败的事件就会被归到最末尾那条兜底规则上。
            let is_candidate = best_idx.is_none() || progress.is_deeper_than(&best_progress);

            // 根据是否有残留数据返回不同的结果
            match parsed {
                Ok((mut tdo_crate, un_parsed, side_records)) => {
                    wpl_line.hit_cnt += 1;
                    let wpl_key = wpl_line.wpl_key().to_string();

                    if un_parsed.is_empty() {
                        let side_records =
                            stamp_record_and_sides(&mut tdo_crate, side_records, event, setting);
                        let record = Arc::new(tdo_crate);
                        info_edata!(event.event_id, "wpl parse suc! wpl:{} ", wpl_key,);
                        return ProcessResult::Success {
                            wpl_key,
                            record,
                            side_records,
                        };
                    } else if un_parsed.len() * 5 > event.payload.len() {
                        // 解析不足、整条丢弃：不盖戳，避免无谓的 meta/MD5 计算
                        info_edata!(event.event_id, "wpl parse not complete: {}", wpl_key,);
                        if is_candidate {
                            best_progress = progress;
                            best_idx = Some(idx);
                            best_error = Some(WparseReason::data_error().to_err());
                        }
                    } else {
                        let side_records =
                            stamp_record_and_sides(&mut tdo_crate, side_records, event, setting);
                        let record = Arc::new(tdo_crate);
                        return ProcessResult::Partial {
                            wpl_key,
                            record,
                            residue: un_parsed.to_string(),
                            side_records,
                        };
                    }
                }
                Err(e) => {
                    // 记录解析进度最靠后的错误
                    if matches!(e.reason(), WparseReason::Uvs(UnifiedReason::DataError)) {
                        if is_candidate {
                            best_progress = progress;
                            best_idx = Some(idx);
                            best_error = Some(e);
                        }
                        debug_edata!(event.event_id, "wpl parse fail: {}", wpl_line.wpl_key());
                    } else if best_error.is_none() {
                        // 如果不是 DataError，作为备选记录第一个错误
                        best_idx = Some(idx);
                        best_progress = progress;
                        best_error = Some(e);
                        break;
                    }

                    if is_last {
                        break;
                    }
                }
            }
        }

        // 所有规则都失败，返回进度最靠后的失败信息
        let best_wpl = best_idx.map_or(String::new(), |i| self.pipelines[i].wpl_key().to_string());
        let best_error = best_error
            .unwrap_or_else(|| WparseError::from(WparseReason::Uvs(UnifiedReason::system_error())));
        ProcessResult::Miss(super::types::ParseFailInfo::new(
            best_wpl,
            best_error,
            best_progress.consumed,
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

    pub async fn send_stat(&mut self, mon_send: &MonSend) -> WparseResult<()> {
        for i in self.pipelines.iter_mut() {
            i.send_stat(mon_send).await?;
        }
        Ok(())
    }
}

/// 对最终要输出的主 record 与旁路 record 统一盖戳。
///
/// 解析不足、会被丢弃的中间结果不调用此函数。
fn stamp_record_and_sides(
    record: &mut DataRecord,
    side_records: SideRecords,
    event: &SourceEvent,
    setting: &ParseOption,
) -> Vec<(String, DataRecord)> {
    stamp_event_meta(record, event, setting);
    side_records
        .into_iter()
        .map(|(key, mut side)| {
            stamp_event_meta(&mut side, event, setting);
            (key.to_string(), side)
        })
        .collect()
}

/// 给 record 盖事件级 meta：wp_event_id / wp_src_key / wp_src_ip / wp_event_md5，
/// 以及进程级全局 tag（`WP_GLOBAL_TAGS`）。
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
    // 全局 tag 独立于 gen_msg_id：只要实例配置了 WP_GLOBAL_TAGS 就盖戳。
    if let Some(global_tags) = setting.global_tags() {
        crate::core::parser::global_tags::apply_global_tags(record, global_tags);
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::core::parser::global_tags::GlobalTags;
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

    #[test]
    fn global_tags_stamped_on_success() {
        let pipelines = vec![pipeline_with(0, "rule a { (chars\\0) }", false)];
        let mut parser = MultiParser::new(pipelines);
        let tags = std::sync::Arc::new(GlobalTags::from_pairs(vec![(
            "owner_id".into(),
            "team-a".into(),
        )]));
        let setting = ParseOption::new(false, false, Vec::new()).with_global_tags(tags);

        match parser.parse_event(&event("abc"), &setting) {
            ProcessResult::Success { record, .. } => {
                assert_eq!(
                    record.field("owner_id").unwrap().get_value().as_str(),
                    Some("team-a")
                );
            }
            other => panic!("expected success, got {:?}", other),
        }
    }

    #[test]
    fn global_tags_override_rule_field() {
        let pipelines = vec![pipeline_with(0, "rule a { (chars:owner_id) }", false)];
        let mut parser = MultiParser::new(pipelines);
        let tags = std::sync::Arc::new(GlobalTags::from_pairs(vec![(
            "owner_id".into(),
            "team-a".into(),
        )]));
        let setting = ParseOption::new(false, false, Vec::new()).with_global_tags(tags);

        match parser.parse_event(&event("from-rule"), &setting) {
            ProcessResult::Success { record, .. } => {
                assert_eq!(
                    record.field("owner_id").unwrap().get_value().as_str(),
                    Some("team-a")
                );
                assert_eq!(
                    record
                        .items
                        .iter()
                        .filter(|f| f.get_name() == "owner_id")
                        .count(),
                    1
                );
            }
            other => panic!("expected success, got {:?}", other),
        }
    }

    #[test]
    fn global_tags_absent_when_tags_are_empty() {
        let pipelines = vec![pipeline_with(0, "rule a { (chars\\0) }", false)];
        let mut parser = MultiParser::new(pipelines);
        let setting = ParseOption::new(false, false, Vec::new())
            .with_global_tags(std::sync::Arc::new(GlobalTags::default()));

        match parser.parse_event(&event("abc"), &setting) {
            ProcessResult::Success { record, .. } => {
                assert!(record.field("owner_id").is_none());
            }
            other => panic!("expected success, got {:?}", other),
        }
    }

    #[test]
    fn global_tags_coexist_with_event_meta() {
        let pipelines = vec![pipeline_with(0, "rule a { (chars\\0) }", false)];
        let mut parser = MultiParser::new(pipelines);
        let tags = std::sync::Arc::new(GlobalTags::from_pairs(vec![(
            "owner_id".into(),
            "team-a".into(),
        )]));
        let setting = ParseOption::new(true, true, Vec::new()).with_global_tags(tags);

        match parser.parse_event(&event("abc"), &setting) {
            ProcessResult::Success { record, .. } => {
                assert!(record.field("wp_event_id").is_some());
                assert!(record.field("wp_src_key").is_some());
                assert!(record.field("wp_event_md5").is_some());
                assert_eq!(
                    record.field("owner_id").unwrap().get_value().as_str(),
                    Some("team-a")
                );
            }
            other => panic!("expected success, got {:?}", other),
        }
    }

    #[test]
    fn global_tags_stamped_on_partial() {
        // 解析 `{...}` 后只剩 `!`，residue 远小于 20%，走 Partial 分支
        let payload = "{aaaaaaaaaaaaaaaaaaaa}!";
        let pipelines = vec![pipeline_with(0, "rule partial { (chars:a<{,}>) }", false)];
        let mut parser = MultiParser::new(pipelines);
        let tags = std::sync::Arc::new(GlobalTags::from_pairs(vec![(
            "owner_id".into(),
            "team-a".into(),
        )]));
        let setting = ParseOption::new(false, false, Vec::new()).with_global_tags(tags);

        match parser.parse_event(&event(payload), &setting) {
            ProcessResult::Partial {
                record, residue, ..
            } => {
                assert_eq!(residue, "!");
                assert_eq!(
                    record.field("owner_id").unwrap().get_value().as_str(),
                    Some("team-a")
                );
            }
            other => panic!("expected partial, got {:?}", other),
        }
    }

    fn miss_info(
        parser: &mut MultiParser,
        payload: &str,
    ) -> crate::core::parser::wpl_engine::types::ParseFailInfo {
        let setting = ParseOption::new(false, false, Vec::new());
        match parser.parse_event(&event(payload), &setting) {
            ProcessResult::Miss(info) => info,
            other => panic!("expected miss, got {:?}", other),
        }
    }

    #[test]
    fn miss_keeps_first_rule_on_equal_depth() {
        // 两条规则都只解析到第 2 个字节就失败：`#[late]` 排在最后，不能靠“后到”抢走归因。
        let pipelines = vec![
            pipeline_with(0, "rule normal { (digit:a, digit:b) }", false),
            pipeline_with(1, "rule fallback { (digit:x, digit:y) }", true),
        ];
        let mut parser = MultiParser::new(pipelines);

        let info = miss_info(&mut parser, "12ab");
        assert_eq!(info.best_wpl, "rule-0");
        assert_eq!(info.depth, 2);
    }

    #[test]
    fn miss_prefers_rule_that_parsed_deepest() {
        // 兜底规则真的走得更远时，仍应成为归因目标：判定依据是深度，不是分层。
        let pipelines = vec![
            pipeline_with(0, "rule normal { (digit:a, digit:b) }", false),
            pipeline_with(1, "rule fallback { (digit:x, chars\\0, digit:y) }", true),
        ];
        let mut parser = MultiParser::new(pipelines);

        let info = miss_info(&mut parser, "12ab");
        assert_eq!(info.best_wpl, "rule-1");
        assert_eq!(info.depth, 4);
    }

    #[test]
    fn miss_does_not_regress_to_shallower_rule() {
        // 先深后浅：后面的浅层失败不能覆盖已记录的深度归因。
        let pipelines = vec![
            pipeline_with(0, "rule deep { (digit:a, chars\\0, digit:b) }", false),
            pipeline_with(1, "rule shallow { (digit:x, digit:y, digit:z) }", true),
        ];
        let mut parser = MultiParser::new(pipelines);

        let info = miss_info(&mut parser, "12ab");
        assert_eq!(info.best_wpl, "rule-0");
        assert_eq!(info.depth, 4);
    }

    #[test]
    fn miss_never_attributes_to_no_match_rule() {
        // `#[no_match]` 规则即使能走得更远，也不参与自动匹配与归因。
        let mut deep = pipeline_with(1, "rule deep { (digit:a, chars\\0, digit:b) }", false);
        deep.auto_match = false;
        let pipelines = vec![
            pipeline_with(0, "rule normal { (digit:a, digit:b) }", false),
            deep,
        ];
        let mut parser = MultiParser::new(pipelines);

        let info = miss_info(&mut parser, "12ab");
        assert_eq!(info.best_wpl, "rule-0");
        assert_eq!(info.depth, 2);
    }

    #[test]
    fn miss_reports_zero_depth_when_nothing_is_consumed() {
        // 所有规则都在第一个字段就失败：仍要给出具体归因，且深度如实为 0。
        let pipelines = vec![pipeline_with(0, "rule only { (digit:a) }", false)];
        let mut parser = MultiParser::new(pipelines);

        let info = miss_info(&mut parser, "abc");
        assert_eq!(info.best_wpl, "rule-0");
        assert_eq!(info.depth, 0);
    }

    #[test]
    fn miss_without_rules_falls_back_to_system_error() {
        let mut parser = MultiParser::new(Vec::new());

        let info = miss_info(&mut parser, "abc");
        assert!(info.best_wpl.is_empty());
        assert_eq!(info.depth, 0);
    }

    #[test]
    fn miss_ranks_by_ratio_across_different_pipe_lengths() {
        // base64("1234") = "MTIzNA=="
        //   无 pipe 规则: 输入 8 字节，消费 6 → 75%
        //   带 decode/base64 规则: 解出 "1234"，输入 4 字节，消费 4 → 100%
        //
        // 这条同时守住两件事：
        // 1. 按比例而非绝对字节排名（6 > 4 会把归因给错的规则）；
        // 2. 不能把前一条规则的绝对深度回灌成后一条的 `oth_suc_len`，
        //    否则 4 < 6 会让 piped 规则整条降级成 NotMatch、直接失去归因资格。
        let pipelines = vec![
            pipeline_with(0, "rule plain { (symbol(MTIzNA), digit:x) }", false),
            pipeline_with(
                1,
                "rule piped { |decode/base64| (digit:a, digit:b) }",
                false,
            ),
        ];
        let mut parser = MultiParser::new(pipelines);

        let info = miss_info(&mut parser, "MTIzNA==");
        assert_eq!(info.best_wpl, "rule-1");
        assert_eq!(info.depth, 4, "depth 是 pipe 后输入中的位置");
    }

    #[test]
    fn miss_ratio_baseline_plain_rule_consumes_six_of_eight() {
        // 上一条用例的前提：无 pipe 规则在 "MTIzNA==" 上确实只消费 6 字节。
        let pipelines = vec![pipeline_with(
            0,
            "rule plain { (symbol(MTIzNA), digit:x) }",
            false,
        )];
        let mut parser = MultiParser::new(pipelines);

        let info = miss_info(&mut parser, "MTIzNA==");
        assert_eq!(info.best_wpl, "rule-0");
        assert_eq!(info.depth, 6);
    }

    #[test]
    fn miss_does_not_count_rules_skipped_by_no_match() {
        // 只有 #[no_match] 规则时无人参与自动匹配，仍应回落 system_error。
        let mut only = pipeline_with(0, "rule only { (chars\\0) }", false);
        only.auto_match = false;
        let mut parser = MultiParser::new(vec![only]);

        let info = miss_info(&mut parser, "abc");
        assert!(info.best_wpl.is_empty());
        assert_eq!(info.depth, 0);
    }

    #[test]
    fn partial_result_is_returned_even_after_a_deeper_miss() {
        // 深度归因只用于 Miss；一旦有规则给出 Partial，它就应当胜出。
        let payload = "{aaaaaaaaaaaaaaaaaaaa}!";
        let pipelines = vec![
            pipeline_with(0, "rule deep_fail { (digit:a, chars\\0, digit:b) }", false),
            pipeline_with(1, "rule partial { (chars:a<{,}>) }", false),
        ];
        let mut parser = MultiParser::new(pipelines);
        let setting = ParseOption::new(false, false, Vec::new());

        match parser.parse_event(&event(payload), &setting) {
            ProcessResult::Partial {
                wpl_key, residue, ..
            } => {
                assert_eq!(wpl_key, "rule-1");
                assert_eq!(residue, "!");
            }
            other => panic!("expected partial, got {:?}", other),
        }
    }

    #[test]
    fn success_is_returned_even_after_a_deeper_miss() {
        let pipelines = vec![
            pipeline_with(0, "rule deep_fail { (digit:a, digit:b) }", false),
            pipeline_with(1, "rule wins { (chars\\0) }", false),
        ];
        let mut parser = MultiParser::new(pipelines);
        let setting = ParseOption::new(false, false, Vec::new());

        match parser.parse_event(&event("12ab"), &setting) {
            ProcessResult::Success { wpl_key, .. } => assert_eq!(wpl_key, "rule-1"),
            other => panic!("expected success, got {:?}", other),
        }
    }

    #[test]
    fn miss_does_not_touch_hit_counts() {
        // 失败不得计入命中，否则命中率排序会被失败次数污染。
        let pipelines = vec![
            pipeline_with(0, "rule a { (digit:x) }", false),
            pipeline_with(1, "rule b { (digit:y) }", false),
        ];
        let mut parser = MultiParser::new(pipelines);

        let _ = miss_info(&mut parser, "abc");
        assert!(parser.pipelines.iter().all(|p| p.hit_cnt == 0));
    }

    #[test]
    fn miss_tie_prefers_higher_hit_rate_after_optimized() {
        // 同进度时的归属跟随当前匹配顺序（= 命中率高的先到），而不是声明序。
        let mut a = pipeline_with(0, "rule a { (digit:x, digit:y) }", false);
        a.hit_cnt = 0;
        let mut b = pipeline_with(1, "rule b { (digit:x, digit:y) }", false);
        b.hit_cnt = 7;
        let mut parser = MultiParser::new(vec![a, b]);
        parser.optimized(0);
        assert_eq!(order(&parser), vec!["rule-1", "rule-0"]);

        let info = miss_info(&mut parser, "12ab");
        assert_eq!(info.best_wpl, "rule-1");
    }

    #[test]
    fn miss_on_empty_payload_reports_zero_depth() {
        let pipelines = vec![pipeline_with(0, "rule a { (digit:x) }", false)];
        let mut parser = MultiParser::new(pipelines);

        let info = miss_info(&mut parser, "");
        assert_eq!(info.best_wpl, "rule-0");
        assert_eq!(info.depth, 0);
    }
}

// 重新导出主要类型
