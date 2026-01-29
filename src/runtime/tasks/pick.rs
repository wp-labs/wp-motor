use crate::runtime::actor::TaskGroup;
use crate::runtime::actor::signal::ShutdownCmd;
use crate::runtime::collector::realtime::SourceWorker;
use crate::runtime::parser::workflow::ParseWorkerSender;
use crate::stat::MonSend;
use wp_conf::RunArgs;
use wp_connector_api::{SourceHandle, SourceMeta};
use wp_stat::StatRequires;
use wp_stat::StatStage;

pub struct PickerGroups {
    pub primary: TaskGroup,
    pub derived: Option<TaskGroup>,
}

/// 启动采集任务（pickers）
/// 使用 Frame 订阅通道启动采集任务（将 SourceFrame 分发到解析线程）
pub fn start_picker_tasks(
    run_args: &RunArgs,
    all_sources: Vec<SourceHandle>,
    mon_send: MonSend,
    parse_senders: Vec<ParseWorkerSender>,
    stat_reqs: &StatRequires,
) -> PickerGroups {
    let mut primary_group = TaskGroup::new("picker", ShutdownCmd::Immediate);
    let mut derived_group: Option<TaskGroup> = None;
    info_ctrl!("启动数据收集(Frame)： {}个数据源", all_sources.len());
    for source_h in all_sources {
        let is_derived = is_derived_source(&source_h.metadata);
        let worker = SourceWorker::new(
            run_args.speed_limit,
            run_args.line_max,
            mon_send.clone(),
            parse_senders.clone(),
        );
        let target_group = if is_derived {
            derived_group
                .get_or_insert_with(|| TaskGroup::new("picker-derived", ShutdownCmd::Immediate))
        } else {
            &mut primary_group
        };
        let cmd_sub = target_group.subscribe();
        let c_args = run_args.clone();
        let reqs = stat_reqs.get_requ_items(StatStage::Pick);
        info_ctrl!(
            "spawning picker for source '{}' (line_max={:?}, speed_limit={})",
            source_h.source.identifier(),
            c_args.line_max,
            c_args.speed_limit
        );
        target_group.append(tokio::spawn(async move {
            let max_line = c_args.line_max;
            let source_id = source_h.source.identifier();
            info_ctrl!("启动数据源 picker(Frame): {}", source_id);
            if let Err(e) = worker.run(source_h.source, cmd_sub, max_line, reqs).await {
                error_ctrl!("数据源 '{}' picker 错误: {}", source_id, e);
            } else {
                info_ctrl!("数据源 '{}' picker 正常结束", source_id);
            }
        }));
    }
    if primary_group.routin_is_finished() && derived_group.is_some() {
        info_ctrl!("仅检测到派生数据源，提升 derived picker 组为主采集组");
        let derived = derived_group.take();
        PickerGroups {
            primary: derived.expect("derived picker group must exist"),
            derived: None,
        }
    } else {
        PickerGroups {
            primary: primary_group,
            derived: derived_group,
        }
    }
}

fn is_derived_source(meta: &SourceMeta) -> bool {
    matches!(meta.tags.get("wp.role"), Some(val) if val == "derived")
}
