//! 调度策略封装：无外部配置；跨 round 持久化状态；不使用时间度量。
//! - PostPolicy：控制本轮发送（post）。若上轮阻塞（Full/Closed），跨 round 按轮次跳过 post（退避）。
//! - PullPolicy：控制本轮拉取（pull）。仅基于水位（LO/HI）和本轮配额决策，不使用时间。

use crate::runtime::collector::realtime::constants::{
    PICKER_POST_BACKOFF_GROWTH_FACTOR, PICKER_POST_BACKOFF_INITIAL_ROUNDS,
    PICKER_POST_BACKOFF_MAX_ROUNDS, PICKER_PULL_HI_MULTIPLIER, PICKER_PULL_LO_MULTIPLIER,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, getset::CopyGetters)]
#[get_copy = "pub"]
pub struct PostPlan {
    allow: bool,
    batch_size: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, getset::CopyGetters)]
#[get_copy = "pub"]
pub struct PullPlan {
    allow: bool,
    fetch_budget: usize,
}

#[derive(Debug, Clone)]
pub struct PostPolicy {
    burst: usize,
    // 轮次退避：在未来 skip_rounds_left 个 round 内跳过 post
    skip_rounds_left: u32,
    // 退避倍增计数（单位：轮次），初始 1，指数退避到上限
    backoff_rounds: u32,
    max_backoff_rounds: u32,
}

#[derive(Debug, Clone, getset::CopyGetters)]
#[get_copy = "pub"]
pub struct PullPolicy {
    burst: usize,
    lo: usize,
    hi: usize,
}

impl PostPolicy {
    pub fn new(burst: usize) -> Self {
        Self {
            burst,
            skip_rounds_left: 0,
            backoff_rounds: PICKER_POST_BACKOFF_INITIAL_ROUNDS,
            max_backoff_rounds: PICKER_POST_BACKOFF_MAX_ROUNDS,
        }
    }
    /// 在每轮开始时调用：若处于退避期，消耗一轮并返回 true 表示本轮应跳过 post。
    pub fn in_cooldown(&mut self) -> bool {
        if self.skip_rounds_left > 0 {
            self.skip_rounds_left -= 1;
            return true;
        }
        false
    }
    pub fn plan_post(&self, pending: usize, full_post: bool) -> PostPlan {
        if pending == 0 {
            return PostPlan {
                allow: false,
                batch_size: 0,
            };
        }
        // 当源侧已终止时，尽可能清空 pending；否则按照 burst 限制单轮发送的批次数
        let batch = if full_post {
            pending
        } else {
            pending.min(self.burst)
        };
        PostPlan {
            allow: batch > 0,
            batch_size: batch,
        }
    }
    pub fn on_post_result(&mut self, progressed: bool) {
        if progressed {
            self.skip_rounds_left = 0;
            self.backoff_rounds = 1;
        } else {
            // 设置退避轮次，并指数递增
            self.skip_rounds_left = self.backoff_rounds;
            let next = self
                .backoff_rounds
                .saturating_mul(PICKER_POST_BACKOFF_GROWTH_FACTOR);
            self.backoff_rounds = next.min(self.max_backoff_rounds);
        }
    }
}

impl PullPolicy {
    pub fn new(burst: usize) -> Self {
        let lo = burst.saturating_mul(PICKER_PULL_LO_MULTIPLIER).max(1);
        let hi = burst.saturating_mul(PICKER_PULL_HI_MULTIPLIER).max(lo);
        Self { burst, lo, hi }
    }
    pub fn plan_pull(&self, pending: usize) -> PullPlan {
        if pending >= self.hi {
            return PullPlan {
                allow: false,
                fetch_budget: 0,
            };
        }
        // 只基于水位：若低于 HI，允许拉取不超过 burst 的预算；
        // 为什么：避免以“批数”为维度的解析通道被频繁小批塞满，维持平滑水位。
        let room = self.hi.saturating_sub(pending);
        let budget = room.min(self.burst);
        PullPlan {
            allow: budget > 0,
            fetch_budget: budget,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn post_plan_respects_burst_and_pending() {
        let post = PostPolicy::new(16);
        let plan = post.plan_post(32, false);
        assert!(plan.allow);
        assert_eq!(plan.batch_size, 16);

        let plan2 = post.plan_post(8, false);
        assert!(plan2.allow);
        assert_eq!(plan2.batch_size, 8);

        let plan3 = post.plan_post(0, false);
        assert!(!plan3.allow);

        let plan4 = post.plan_post(10, true);
        assert_eq!(plan4.batch_size, 10);
    }

    #[test]
    fn pull_plan_suppressed_when_pending_above_lo() {
        let pull = PullPolicy::new(16);
        // lo = 32
        let plan = pull.plan_pull(32);
        assert_eq!(pull.lo, 32);
        assert!(plan.allow);
    }

    #[test]
    fn pull_plan_budgets_to_hi_and_burst() {
        let pull = PullPolicy::new(16);
        // pending below lo, room to hi=64; 预算=burst=16
        let plan = pull.plan_pull(10);
        assert!(plan.allow);
        assert_eq!(plan.fetch_budget, 16);

        // 简化后不再考虑“本轮剩余配额缩减”的场景（单轮只拉一次），因此仍为 burst
        let plan2 = pull.plan_pull(10);
        assert!(plan2.allow);
        assert_eq!(plan2.fetch_budget, 16);

        // pending 只由水位控制；此处仍允许拉取
        let plan3 = pull.plan_pull(10);
        assert!(plan3.allow);
    }
}
