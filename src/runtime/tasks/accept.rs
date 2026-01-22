use crate::runtime::actor::TaskGroup;
use crate::runtime::actor::command::spawn_ctrl_event_bridge;
use wp_connector_api::AcceptorHandle;

/// 将接受器任务（acceptors）添加到主组（pickers）中。
///
/// 这样做与旧版保持一致：接受器属于采集链路的一部分，在主流程（picker 组）完成后统一退出，
/// 既不会阻塞主组在 batch 模式下完成，也能在 daemon 模式下与采集线程同生共死，更利于优雅停机。
pub fn add_acceptor_tasks(group: &mut TaskGroup, all_acceptors: Vec<AcceptorHandle>) {
    info_ctrl!("启动接受器: {}个", all_acceptors.len());
    for handle in all_acceptors {
        let mut acceptor = handle.acceptor;
        let cmd_sub = group.subscribe();
        group.append(tokio::spawn(async move {
            info_ctrl!("启动接受器任务");
            let ctrl_rx = spawn_ctrl_event_bridge(cmd_sub.clone(), 1024);

            if let Err(e) = acceptor.accept_connection(ctrl_rx).await {
                error_ctrl!("接受器错误: {}", e);
            } else {
                info_ctrl!("接受器正常结束");
            }
        }));
    }
}
