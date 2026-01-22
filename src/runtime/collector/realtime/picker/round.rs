#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DistStatus {
    // 本轮分发已就绪（可继续发送/拉取）
    Ready,
    // 本轮存在“待发送但暂时无法投递”的批（例如解析通道满），提示上层可做温和退让
    Pending,
    // 终止当前突发轮（例如解析侧或控制命令要求停止）
    Terminal,
}
impl DistStatus {
    pub fn is_pending(&self) -> bool {
        *self == DistStatus::Pending
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SrcStatus {
    // 源侧就绪（读取成功或无错误）
    Ready,
    // 源侧未命中（超时/暂时没有数据），提示上层避免忙等
    Miss,
    // 源侧终止（EOF/不可恢复错误）
    Terminal,
}
impl SrcStatus {
    pub fn is_miss(&self) -> bool {
        *self == SrcStatus::Miss
    }
}
/// 每轮突发过程的统计汇总
#[derive(Debug, Clone, getset::CopyGetters)]
#[get_copy = "pub"]
pub(crate) struct RoundStat {
    // 本轮累计成功发送的“批次数”（非事件数）
    send_cnt: usize,
    // 合并次数（用于测试追踪： merge 被调用的次数）
    merge_count: usize,
    // 当前轮内的子轮计数（用于限制一轮最大循环次数）
    round_idx: usize,
    // 分发侧状态（是否 pending / 需要终止）
    dist_status: DistStatus,
    // 源侧状态（是否 miss / 终止）
    src_status: SrcStatus,
}

impl RoundStat {
    pub fn new() -> Self {
        Self {
            send_cnt: 0,
            merge_count: 0,
            round_idx: 0,
            dist_status: DistStatus::Ready,
            src_status: SrcStatus::Ready,
        }
    }
    #[inline]
    pub fn terminal_by_round(&mut self, max: usize) -> bool {
        // 返回“是否达到本轮上限”，同时自增计数。
        // 为什么：限制单轮 burst 的长度，避免无界循环影响控制命令响应延迟。
        let is_end = self.round_idx >= max;
        self.round_idx += 1;
        is_end
    }

    #[inline]
    #[allow(clippy::wrong_self_convention)]
    pub fn to_dist_pending(&mut self) {
        self.dist_status = DistStatus::Pending
    }
    #[inline]
    #[allow(dead_code)]
    #[allow(clippy::wrong_self_convention)]
    pub fn to_dist_ready(&mut self) {
        self.dist_status = DistStatus::Ready
    }
    #[inline]
    #[allow(clippy::wrong_self_convention)]
    pub fn to_dist_terminal(&mut self) {
        self.dist_status = DistStatus::Terminal
    }
    #[inline]
    pub fn is_stop(&self) -> bool {
        self.src_status == SrcStatus::Terminal || self.dist_status == DistStatus::Terminal
    }

    #[inline]
    pub fn merge(mut self, other: Self) -> Self {
        self.add_proc(other.send_cnt);
        self.merge_count += other.merge_count + 1;
        self.dist_status = other.dist_status;
        self.src_status = other.src_status;
        self
    }
    #[inline]
    pub fn need_wait(&self, have_cnt: usize) -> bool {
        // 没有投递成功且分发侧 pending → 下游拥塞，应温和休眠；
        // 没有待处理且源侧 miss     → 上游暂无数据，避免忙等。
        (self.send_cnt == 0 && self.dist_status.is_pending())
            || (have_cnt == 0 && self.src_status.is_miss())
    }

    #[inline]
    pub fn add_proc(&mut self, delivered: usize) {
        self.send_cnt += delivered;
    }

    pub(crate) fn up_src_status(&mut self, status: SrcStatus) {
        self.src_status = status
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_accumulates_round_count() {
        let merged = RoundStat::new()
            .merge(RoundStat::new())
            .merge(RoundStat::new())
            .merge(RoundStat::new());
        assert_eq!(merged.merge_count(), 3);
    }

    #[test]
    fn terminal_by_round_returns_false_until_reaching_max() {
        let mut rs = RoundStat::new();
        assert!(!rs.terminal_by_round(1), "first round should not terminate");
        assert!(rs.terminal_by_round(1), "second round should terminate");
    }

    #[test]
    fn merge_accumulates_proc_and_wait_flags() {
        let mut first = RoundStat::new();
        first.add_proc(3);
        let mut second = RoundStat::new();
        second.add_proc(2);
        let merged = first.merge(second);
        assert_eq!(merged.send_cnt(), 5, "proc count should sum");
    }

    #[test]
    fn merge_propagates_source_status() {
        let mut second = RoundStat::new();
        second.up_src_status(SrcStatus::Miss);
        let merged = RoundStat::new().merge(second);
        assert!(
            merged.src_status().is_miss(),
            "src status should follow merged round"
        );
    }
}
