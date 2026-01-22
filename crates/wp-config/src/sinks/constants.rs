/// 监控组名
pub const GROUP_MONITOR: &str = "monitor";
/// 默认组名
pub const GROUP_DEFAULT: &str = "default";
/// 缺失组名
pub const GROUP_MISS: &str = "miss";
/// 残余组名
pub const GROUP_RESIDUE: &str = "residue";
/// 拦截组名
/// 错误组名
pub const GROUP_ERROR: &str = "error";

/// 基础设施组名集合（含 monitor/default/miss/residue/error）
pub const INFRA_GROUPS: &[&str] = &[
    GROUP_MONITOR,
    GROUP_DEFAULT,
    GROUP_MISS,
    GROUP_RESIDUE,
    GROUP_ERROR,
];

/// 是否为基础设施组名
pub fn is_infra_group_name(name: &str) -> bool {
    INFRA_GROUPS.contains(&name)
}
