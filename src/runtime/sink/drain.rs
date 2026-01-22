/// 通用的 sink drain 状态机：在 Stop 指令到达后只消费数据通道，直到全部关闭
#[derive(Debug, Clone)]
pub struct DrainState {
    draining: bool,
    open_channels: usize,
}

/// 通道关闭后的状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrainEvent {
    Pending,
    AllClosed,
    Drained,
}

impl DrainState {
    /// 创建一个新的 drain 状态，`open_channels` 为需要等待关闭的数据通道数量
    pub fn new(open_channels: usize) -> Self {
        Self {
            draining: false,
            open_channels,
        }
    }

    /// 当前是否处于 drain 模式
    pub fn is_draining(&self) -> bool {
        self.draining
    }

    /// 进入 drain 模式（幂等）
    pub fn start_draining(&mut self) {
        self.draining = true;
    }

    /// 通道关闭，返回当前状态
    pub fn channel_closed_is_drained(&mut self) -> DrainEvent {
        if self.open_channels > 0 {
            self.open_channels -= 1;
        }
        if self.open_channels == 0 {
            if self.draining {
                DrainEvent::Drained
            } else {
                DrainEvent::AllClosed
            }
        } else {
            DrainEvent::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_without_draining_returns_all_closed() {
        let mut state = DrainState::new(1);
        let evt = state.channel_closed_is_drained();
        assert!(matches!(evt, DrainEvent::AllClosed));
    }

    #[test]
    fn drain_state_reports_drained_after_close() {
        let mut state = DrainState::new(1);
        state.start_draining();
        let evt = state.channel_closed_is_drained();
        assert!(matches!(evt, DrainEvent::Drained));
    }

    #[tokio::test]
    async fn receiver_close_unblocks_drain_state() {
        let mut state = DrainState::new(1);
        state.start_draining();

        let (tx, mut rx) = tokio::sync::mpsc::channel(4);
        for _ in 0..4 {
            tx.send(()).await.unwrap();
        }
        let _other_tx = tx.clone();
        rx.close();

        while rx.recv().await.is_some() {}

        let evt = state.channel_closed_is_drained();
        assert!(matches!(evt, DrainEvent::Drained));
    }
}
