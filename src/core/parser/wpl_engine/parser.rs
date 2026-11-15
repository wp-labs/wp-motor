//! 单个数据包解析逻辑

use super::types::ProcessResult;
use crate::core::parser::upstream_meta::UpstreamKeep;
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

/// 事件级 meta 字段名。
const META_EVENT_ID: &str = "wp_event_id";
const META_SRC_KEY: &str = "wp_src_key";
const META_SRC_IP: &str = "wp_src_ip";
const META_EVENT_MD5: &str = "wp_event_md5";

/// 上游（接力场景）已经盖过的 wp_* meta 字段。
#[derive(Default)]
struct UpstreamMeta {
    event_id: bool,
    src_key: bool,
    src_ip: bool,
    event_md5: bool,
}

impl UpstreamMeta {
    fn is_complete(&self) -> bool {
        self.event_id && self.src_key && self.src_ip && self.event_md5
    }
}

/// 给 record 盖事件级 meta：wp_event_id / wp_src_key / wp_src_ip / wp_event_md5，
/// 以及进程级全局 tag（`WP_GLOBAL_TAGS`）。
/// 由 gen_msg_id 总开关 + gen_event_md5 子开关控制；主 record 与旁路 record 都走此路径。
///
/// **上游已有的 wp_* meta 不会被覆盖**：接力链路（A 解析完 → B 再解析 A 的输出）里，
/// 规则把上游 ID 当作字段提取出来时，沿用上游值而不是重新覆盖，否则跨阶段关联会断掉。
fn stamp_event_meta(record: &mut DataRecord, event: &SourceEvent, setting: &ParseOption) {
    if *setting.gen_msg_id() {
        let upstream = scan_upstream_meta(record);
        let keep = UpstreamKeep::load();

        apply_event_id(record, upstream.event_id, keep.event_id, event.event_id);

        upsert_meta(record, upstream.src_key, keep.src, META_SRC_KEY, || {
            Field::from_chars(META_SRC_KEY, event.src_key.as_str())
        });
        if let Some(ups_ip) = event.ups_ip {
            upsert_meta(record, upstream.src_ip, keep.src, META_SRC_IP, || {
                Field::from_ip(META_SRC_IP, ups_ip)
            });
        }
        if *setting.gen_event_md5() {
            upsert_meta(
                record,
                upstream.event_md5,
                keep.event_md5,
                META_EVENT_MD5,
                || {
                    let digest = format!("{:x}", md5::compute(event.payload.as_bytes()));
                    Field::from_chars(META_EVENT_MD5, digest)
                },
            );
        }
    }
    // 全局 tag 独立于 gen_msg_id：只要实例配置了 WP_GLOBAL_TAGS 就盖戳。
    if let Some(global_tags) = setting.global_tags() {
        crate::core::parser::global_tags::apply_global_tags(record, global_tags);
    }
}

/// 按「是否保留上游」决定一个 meta 字段怎么落：
///
/// - 上游没有 → 追加本实例的值
/// - 上游有 + 保留 → 不动
/// - 上游有 + 不保留 → 先移除同名旧值，再追加本实例的值（保证只有一个）
fn upsert_meta(
    record: &mut DataRecord,
    upstream_present: bool,
    keep_upstream: bool,
    name: &str,
    make: impl FnOnce() -> wp_model_core::model::DataField,
) {
    if upstream_present && keep_upstream {
        return;
    }
    if upstream_present {
        while record.remove_field(name) {}
    }
    record.append(make());
}

/// 单次遍历找出上游已带的 meta 字段，避免对每条记录做四次线性查找。
fn scan_upstream_meta(record: &DataRecord) -> UpstreamMeta {
    let mut found = UpstreamMeta::default();
    for item in record.items.iter() {
        let name = item.get_name();
        // 绝大多数业务字段在这里就退出（长度检查 + 3 字节比较），
        // 避免对每个字段做 4 次完整字符串比较。
        if !name.starts_with("wp_") {
            continue;
        }
        match name {
            META_EVENT_ID => found.event_id = true,
            META_SRC_KEY => found.src_key = true,
            META_SRC_IP => found.src_ip = true,
            META_EVENT_MD5 => found.event_md5 = true,
            _ => continue,
        }
        if found.is_complete() {
            break;
        }
    }
    found
}

/// 决定 `record.id`：上游已带合法 `wp_event_id` 且允许保留时沿用，否则发新号。
///
/// 上游字段存在但取值不可用时，保留原值不动（尊重上游内容），
/// 内部句柄仍然发新号，保证路由/日志侧的唯一性。
fn apply_event_id(
    record: &mut DataRecord,
    has_upstream: bool,
    keep_upstream: bool,
    generated: u64,
) {
    if has_upstream && keep_upstream {
        match upstream_event_id(record) {
            Some(id) => {
                // 字段已存在，`set_id` 不会重复追加；这里只需同步内部句柄。
                record.id = id;
                note_adopted_upstream_id(id);
            }
            None => {
                record.id = generated;
                note_unusable_upstream_id();
            }
        }
        return;
    }

    // 上游没有，或要求以本实例为准：清掉旧值再发新号。
    if has_upstream {
        while record.remove_field(META_EVENT_ID) {}
    }
    record.set_id(generated);
}

/// 读上游 `wp_event_id` 的数值。
///
/// 同时支持按 `chars` 与按整数（`digit` / `u64`）建模的上游字段：
/// 后者 `Value::as_str()` 返回 `None`，此时用 `Display` 取十进制文本。
fn upstream_event_id(record: &DataRecord) -> Option<u64> {
    let value = record.field(META_EVENT_ID)?.get_value();
    match value.as_str() {
        Some(raw) => parse_canonical_event_id(raw),
        None => parse_canonical_event_id(&value.to_string()),
    }
}

/// 只接受**规范十进制**：全为 ASCII 数字、无前导零、非 0。
///
/// 这样内部句柄与字段文本严格一致（而非仅数值相等），
/// `" 42 "` / `"+42"` / `"0042"` 这类写法一律不采纳。
fn parse_canonical_event_id(raw: &str) -> Option<u64> {
    if raw.is_empty() || !raw.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    if raw.len() > 1 && raw.starts_with('0') {
        return None;
    }
    raw.parse::<u64>().ok().filter(|id| *id != 0)
}

static ADOPT_NOTICE: std::sync::Once = std::sync::Once::new();
static UNUSABLE_NOTICE: std::sync::Once = std::sync::Once::new();

fn note_adopted_upstream_id(id: u64) {
    ADOPT_NOTICE.call_once(|| {
        wp_log::info_ctrl!(
            "relay: adopting upstream wp_event_id (first value {id}); downstream keeps the upstream id"
        );
    });
}

fn note_unusable_upstream_id() {
    UNUSABLE_NOTICE.call_once(|| {
        wp_log::warn_ctrl!(
            "relay: upstream wp_event_id is not a positive integer; keeping the field as-is and \
             issuing a new internal id (check the rule that extracts {META_EVENT_ID})"
        );
    });
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

    /// 盖戳后的 `wp_event_id` 必须是 `秒(32)|实例(10)|计数器(22)` 布局。
    ///
    /// 不能用 `event()` 假事件（它的 `event_id` 是硬编码的 1），
    /// 这里用真实发号器构造事件。
    #[test]
    fn stamped_event_id_follows_the_layout() {
        let pipelines = vec![pipeline_with(0, "rule a { (chars\\0) }", false)];
        let mut parser = MultiParser::new(pipelines);
        let setting = ParseOption::new(true, false, Vec::new());

        let event_id = crate::sources::event_id::next_event_id();
        let source = SourceEvent::new(
            event_id,
            "test-src",
            wp_model_core::raw::RawData::String("abc".to_string()),
            std::sync::Arc::new(wp_connector_api::Tags::new()),
        );

        match parser.parse_event(&source, &setting) {
            ProcessResult::Success { record, .. } => {
                let raw = record
                    .field("wp_event_id")
                    .unwrap()
                    .get_value()
                    .as_str()
                    .expect("wp_event_id 是字符串字段")
                    .to_string();
                assert_eq!(raw, event_id.to_string());

                let (secs, lane, _) = wp_model_core::event_id::decompose_event_id(event_id);
                assert_eq!(
                    wp_model_core::event_id::INS_ID_BASE + lane,
                    wp_model_core::event_id::instance_id(),
                    "wp_event_id 的实例号段与当前实例号不一致"
                );
                assert!(secs > 1_700_000_000, "秒段应取自当前时间，实际 {secs}");
            }
            other => panic!("expected success, got {:?}", other),
        }
    }

    // ---------------- 接力（上游已带 wp_* meta） ----------------

    /// 规则把上游的 `wp_event_id` 当普通字段提取出来：沿用上游值，不覆盖、不重复追加。
    #[test]
    fn upstream_event_id_is_preserved_and_adopted() {
        let pipelines = vec![pipeline_with(
            0,
            "rule relay { (chars:wp_event_id) }",
            false,
        )];
        let mut parser = MultiParser::new(pipelines);
        let setting = ParseOption::new(true, false, Vec::new());

        match parser.parse_event(&event("12345"), &setting) {
            ProcessResult::Success { record, .. } => {
                assert_eq!(
                    record.field("wp_event_id").unwrap().get_value().as_str(),
                    Some("12345"),
                    "上游的 wp_event_id 必须原样保留"
                );
                assert_eq!(record.id, 12345, "内部句柄必须与字段保持一致");
                assert_eq!(
                    record
                        .items
                        .iter()
                        .filter(|f| f.get_name() == "wp_event_id")
                        .count(),
                    1,
                    "不得重复追加 wp_event_id"
                );
            }
            other => panic!("expected success, got {:?}", other),
        }
    }

    /// 上游的 `wp_src_key` 不被本实例的 source key 覆盖，也不重复追加。
    #[test]
    fn upstream_src_key_is_not_overwritten() {
        let pipelines = vec![pipeline_with(0, "rule relay { (chars:wp_src_key) }", false)];
        let mut parser = MultiParser::new(pipelines);
        let setting = ParseOption::new(true, true, Vec::new());

        match parser.parse_event(&event("upstream-src"), &setting) {
            ProcessResult::Success { record, .. } => {
                assert_eq!(
                    record.field("wp_src_key").unwrap().get_value().as_str(),
                    Some("upstream-src"),
                    "event() 的 src_key 是 test-src，不得覆盖上游值"
                );
                assert_eq!(
                    record
                        .items
                        .iter()
                        .filter(|f| f.get_name() == "wp_src_key")
                        .count(),
                    1
                );
            }
            other => panic!("expected success, got {:?}", other),
        }
    }

    /// 上游的 `wp_event_md5` 不被重算值覆盖（即使 gen_event_md5 已开启）。
    #[test]
    fn upstream_event_md5_is_not_recomputed() {
        let pipelines = vec![pipeline_with(
            0,
            "rule relay { (chars:wp_event_md5) }",
            false,
        )];
        let mut parser = MultiParser::new(pipelines);
        let setting = ParseOption::new(true, true, Vec::new());

        match parser.parse_event(&event("deadbeefdeadbeefdeadbeefdeadbeef"), &setting) {
            ProcessResult::Success { record, .. } => {
                assert_eq!(
                    record.field("wp_event_md5").unwrap().get_value().as_str(),
                    Some("deadbeefdeadbeefdeadbeefdeadbeef"),
                    "不得用本实例重算的 md5 覆盖上游值"
                );
                assert_eq!(
                    record
                        .items
                        .iter()
                        .filter(|f| f.get_name() == "wp_event_md5")
                        .count(),
                    1
                );
            }
            other => panic!("expected success, got {:?}", other),
        }
    }

    /// 上游值不可采纳时：字段原样保留，内部句柄仍发新号。
    #[test]
    fn unusable_upstream_event_id_keeps_field_and_issues_new_id() {
        // 非数字、0、负数、前导零、带正号：均不算规范十进制。
        // （纯空白会被 WPL 的 `chars` 自行 trim，不属于本层行为）
        for payload in ["not-a-number", "0", "-5", "0042", "+42"] {
            let pipelines = vec![pipeline_with(
                0,
                "rule relay { (chars:wp_event_id) }",
                false,
            )];
            let mut parser = MultiParser::new(pipelines);
            let setting = ParseOption::new(true, false, Vec::new());

            match parser.parse_event(&event(payload), &setting) {
                ProcessResult::Success { record, .. } => {
                    assert_eq!(
                        record.field("wp_event_id").unwrap().get_value().as_str(),
                        Some(payload),
                        "上游值必须保留（payload={payload:?}）"
                    );
                    assert_eq!(record.id, 1, "内部句柄仍取事件号（payload={payload:?}）");
                    assert_eq!(
                        record
                            .items
                            .iter()
                            .filter(|f| f.get_name() == "wp_event_id")
                            .count(),
                        1,
                        "不得重复追加（payload={payload:?}）"
                    );
                }
                other => panic!("expected success, got {:?}", other),
            }
        }
    }

    /// 上游按整数建模（`digit:wp_event_id`）时同样能采纳。
    #[test]
    fn numeric_upstream_event_id_is_adopted() {
        let pipelines = vec![pipeline_with(
            0,
            "rule relay { (digit:wp_event_id) }",
            false,
        )];
        let mut parser = MultiParser::new(pipelines);
        let setting = ParseOption::new(true, false, Vec::new());

        match parser.parse_event(&event("12345"), &setting) {
            ProcessResult::Success { record, .. } => {
                assert_eq!(record.id, 12345, "整数型字段也要能取到值");
                assert_eq!(
                    record
                        .items
                        .iter()
                        .filter(|f| f.get_name() == "wp_event_id")
                        .count(),
                    1
                );
            }
            other => panic!("expected success, got {:?}", other),
        }
    }

    /// 上游的 `wp_src_ip` 不被本连接的客户端 IP 覆盖，也不重复追加。
    #[test]
    fn upstream_src_ip_is_not_overwritten() {
        let pipelines = vec![pipeline_with(0, "rule relay { (chars:wp_src_ip) }", false)];
        let mut parser = MultiParser::new(pipelines);
        let setting = ParseOption::new(true, false, Vec::new());

        let mut source = event("10.9.9.9");
        source.ups_ip = Some(std::net::IpAddr::V4(std::net::Ipv4Addr::new(172, 16, 0, 1)));

        match parser.parse_event(&source, &setting) {
            ProcessResult::Success { record, .. } => {
                assert_eq!(
                    record
                        .items
                        .iter()
                        .filter(|f| f.get_name() == "wp_src_ip")
                        .count(),
                    1,
                    "不得重复追加 wp_src_ip"
                );
                assert_eq!(
                    record.field("wp_src_ip").unwrap().get_value().as_str(),
                    Some("10.9.9.9"),
                    "上游值优先于本连接的对端 IP"
                );
            }
            other => panic!("expected success, got {:?}", other),
        }
    }

    // ---------------- meta 落盘策略（upsert_meta / apply_event_id） ----------------

    fn rec_with_meta(name: &'static str, value: &str) -> DataRecord {
        DataRecord::from(vec![Field::from_chars(name, value)])
    }

    #[test]
    fn upsert_meta_appends_when_absent() {
        let mut record = rec_with_meta("msg", "hello");
        upsert_meta(&mut record, false, false, META_SRC_KEY, || {
            Field::from_chars(META_SRC_KEY, "local")
        });
        assert_eq!(
            record.field(META_SRC_KEY).unwrap().get_value().as_str(),
            Some("local")
        );
    }

    #[test]
    fn upsert_meta_keeps_upstream_when_asked() {
        let mut record = rec_with_meta(META_SRC_KEY, "upstream");
        upsert_meta(&mut record, true, true, META_SRC_KEY, || {
            Field::from_chars(META_SRC_KEY, "local")
        });
        assert_eq!(
            record.field(META_SRC_KEY).unwrap().get_value().as_str(),
            Some("upstream")
        );
        assert_eq!(
            record
                .items
                .iter()
                .filter(|f| f.get_name() == META_SRC_KEY)
                .count(),
            1
        );
    }

    #[test]
    fn upsert_meta_replaces_upstream_when_overriding() {
        // 故意放两个同名旧值，验证全部被清掉
        let mut record = DataRecord::from(vec![
            Field::from_chars(META_SRC_KEY, "old-a"),
            Field::from_chars(META_SRC_KEY, "old-b"),
        ]);
        upsert_meta(&mut record, true, false, META_SRC_KEY, || {
            Field::from_chars(META_SRC_KEY, "local")
        });
        assert_eq!(
            record
                .items
                .iter()
                .filter(|f| f.get_name() == META_SRC_KEY)
                .count(),
            1,
            "覆盖后只允许保留一个"
        );
        assert_eq!(
            record.field(META_SRC_KEY).unwrap().get_value().as_str(),
            Some("local")
        );
    }

    #[test]
    fn apply_event_id_overrides_upstream_when_keep_disabled() {
        let mut record = rec_with_meta(META_EVENT_ID, "12345");
        apply_event_id(&mut record, true, false, 777);

        assert_eq!(record.id, 777, "不保留时必须用本实例的新号");
        assert_eq!(
            record.field(META_EVENT_ID).unwrap().get_value().as_str(),
            Some("777"),
            "字段也要被覆盖为新号"
        );
        assert_eq!(
            record
                .items
                .iter()
                .filter(|f| f.get_name() == META_EVENT_ID)
                .count(),
            1
        );
    }

    #[test]
    fn apply_event_id_keeps_upstream_when_enabled() {
        let mut record = rec_with_meta(META_EVENT_ID, "12345");
        apply_event_id(&mut record, true, true, 777);

        assert_eq!(record.id, 12345);
        assert_eq!(
            record.field(META_EVENT_ID).unwrap().get_value().as_str(),
            Some("12345")
        );
    }

    /// 上游没有 meta 时，行为与之前一致：全部字段由本实例盖戳。
    #[test]
    fn absent_upstream_meta_is_still_stamped() {
        let pipelines = vec![pipeline_with(0, "rule a { (chars\\0) }", false)];
        let mut parser = MultiParser::new(pipelines);
        let setting = ParseOption::new(true, true, Vec::new());

        match parser.parse_event(&event("abc"), &setting) {
            ProcessResult::Success { record, .. } => {
                assert_eq!(record.id, 1);
                assert!(record.field("wp_event_id").is_some());
                assert!(record.field("wp_src_key").is_some());
                assert!(record.field("wp_event_md5").is_some());
                assert!(
                    record.field("wp_src_ip").is_none(),
                    "event() 不携带 ups_ip，不应凭空盖戳"
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
    // ---------------- 性能探针（默认 ignore） ----------------

    /// `cargo test --release --lib perf_probe_meta_stamp -- --ignored --nocapture`
    #[test]
    #[ignore = "perf probe"]
    fn perf_probe_meta_stamp() {
        use std::hint::black_box;
        use std::time::Instant;

        const N: usize = 1_000_000;
        const FIELDS: usize = 28;

        fn plain_record() -> DataRecord {
            let mut fields = Vec::with_capacity(FIELDS);
            for i in 0..FIELDS {
                fields.push(Field::from_chars(format!("f{i}"), "value"));
            }
            DataRecord::from(fields)
        }

        fn relay_record() -> DataRecord {
            let mut fields = Vec::with_capacity(FIELDS + 3);
            fields.push(Field::from_chars(META_EVENT_ID, "7708000000000000001"));
            fields.push(Field::from_chars(META_SRC_KEY, "file_1"));
            fields.push(Field::from_chars(
                META_EVENT_MD5,
                "ef4e89f986d38fb1df8b224fc04f2fc6",
            ));
            for i in 0..FIELDS {
                fields.push(Field::from_chars(format!("f{i}"), "value"));
            }
            DataRecord::from(fields)
        }

        let time = |label: &str, mut f: Box<dyn FnMut()>| {
            let start = Instant::now();
            for _ in 0..N {
                f();
            }
            let ns = start.elapsed().as_nanos() as f64 / N as f64;
            println!("{label:<46} {ns:>8.1} ns/record");
            ns
        };

        let plain = plain_record();
        let relay = relay_record();

        let payload = vec![b'x'; 300];
        let _md5_cost = time(
            "md5(payload 300B)（接力保留时省掉）",
            Box::new(|| {
                black_box(format!("{:x}", md5::compute(black_box(&payload))));
            }),
        );

        println!("记录字段数 = {FIELDS}（不含 meta），迭代 {N} 次");

        let set_id = time(
            "set_id（改动前就有的一次扫描）",
            Box::new(|| {
                let mut r = black_box(DataRecord::clone(&plain));
                r.set_id(black_box(42));
                black_box(&r);
            }),
        );

        let scan = time(
            "scan_upstream_meta（本次新增）",
            Box::new(|| {
                black_box(scan_upstream_meta(black_box(&plain)));
            }),
        );

        let scan_relay = time(
            "scan_upstream_meta（有 wp_ 字段，提前 break）",
            Box::new(|| {
                black_box(scan_upstream_meta(black_box(&relay)));
            }),
        );

        let setting = ParseOption::new(true, true, Vec::new());
        let ev = event("payload-xyz");

        let common = time(
            "stamp_event_meta 常见路径（无上游 meta）",
            Box::new(|| {
                let mut r = black_box(DataRecord::clone(&plain));
                stamp_event_meta(&mut r, &ev, &setting);
                black_box(&r);
            }),
        );

        let keep = time(
            "stamp_event_meta 接力保留（有上游 meta）",
            Box::new(|| {
                let mut r = black_box(DataRecord::clone(&relay));
                stamp_event_meta(&mut r, &ev, &setting);
                black_box(&r);
            }),
        );

        let override_case = time(
            "stamp_event_meta 接力覆盖（移除 3 个 + 重新发号）",
            Box::new(|| {
                let mut r = black_box(DataRecord::clone(&relay));
                while r.remove_field(META_EVENT_ID) {}
                while r.remove_field(META_SRC_KEY) {}
                while r.remove_field(META_EVENT_MD5) {}
                r.set_id(black_box(42));
                black_box(&r);
            }),
        );

        println!();
        println!(
            "新增扫描占 stamp_event_meta 常见路径的比例: {:.1}%",
            scan / common * 100.0
        );
        println!("新增扫描相对 set_id 单次扫描: {:.2}x", scan / set_id);
        println!(
            "有上游 meta 时保留 vs 常见路径: {:.2}x（{keep:.1} vs {common:.1} ns）",
            keep / common
        );
        println!("有上游 meta 时提前 break 的扫描: {scan_relay:.1} ns");
        println!(
            "接力覆盖 vs 接力保留: {:.2}x（{override_case:.1} vs {keep:.1} ns）",
            override_case / keep
        );
    }
}

// 重新导出主要类型
