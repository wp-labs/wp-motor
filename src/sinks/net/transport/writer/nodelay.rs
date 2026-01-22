use super::NetWriter;
use super::Transport;

impl NetWriter {
    /// 基于 avg/cap 的分类并带回差/时间防抖动态切换 TCP_NODELAY。
    pub(super) fn maybe_toggle_nodelay(&mut self, cap: usize, avg: usize) {
        use std::time::{Duration, Instant};
        // 无 TCP 连接则跳过
        let Transport::Tcp(ref stream) = self.transport else {
            return;
        };
        if cap == 0 {
            return;
        }
        // Hysteresis: off->on 使用 cap/56；on->off 使用 cap/72
        let on_up = cap / 56;
        let off_down = cap / 72;
        let cur = self.nodelay_on.unwrap_or(false); // 默认 off（允许 Nagle）
        let desired_on = if cur {
            // 当前 on，只有 avg 足够小才关闭
            avg >= off_down
        } else {
            // 当前 off，只有 avg 足够大才开启
            avg > on_up
        };
        // 时间防抖：两次切换至少间隔 10ms
        const NODELAY_DEBOUNCE_MS: u64 = 10;
        let now = Instant::now();
        if desired_on != cur {
            if let Some(last) = self.nodelay_last_change
                && now.saturating_duration_since(last) < Duration::from_millis(NODELAY_DEBOUNCE_MS)
            {
                return;
            }
            let _ = stream.set_nodelay(desired_on);
            self.nodelay_on = Some(desired_on);
            self.nodelay_last_change = Some(now);
        }
    }
}
