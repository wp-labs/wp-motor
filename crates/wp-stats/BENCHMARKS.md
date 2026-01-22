# wp-stats 基准测试（Criterion）

本 crate 提供了基于 Criterion 的性能基准，用于评估统计采集与合并流程：

- record_task：不同规模下 `record_task` 的吞吐
- collect_top_n：在 top-N（含倍数保留）下的 `collect_stat` 性能
- merge_reports：两份 Report 的合并性能（包含键重叠场景）

## 运行

- 编译但不执行：
  - `cargo bench -p wp-stats --no-run`
- 执行指定基准：
  - `cargo bench -p wp-stats --bench wp_stats_bench`
- 指定采样/时长（示例）：
  - `cargo bench -p wp-stats --bench wp_stats_bench -- --sample-size 30 --measurement-time 5`

### 快速与稳定模式

- 快速模式（开发机上快速验证）：
  - `cargo bench -p wp-stats --bench wp_stats_bench -- --sample-size 10 --warm-up-time 1 --measurement-time 1`
- 稳定模式（减少抖动，生成可靠对比基线）：
  - `cargo bench -p wp-stats --bench wp_stats_bench -- --sample-size 50 --warm-up-time 3 --measurement-time 5`
- 仅跑某一组：
  - `cargo bench -p wp-stats --bench wp_stats_bench -- record_task`
  - `cargo bench -p wp-stats --bench wp_stats_bench -- lru_capacity`
  - `cargo bench -p wp-stats --bench wp_stats_bench -- lru_hit_rate`

### 基线管理（回归/提升对比）

- 保存当前结果为基线：
  - `cargo bench -p wp-stats --bench wp_stats_bench -- --save-baseline stat-baseline`
- 以后对比该基线：
  - `cargo bench -p wp-stats --bench wp_stats_bench -- --baseline stat-baseline`
- 可调噪声阈值（单位百分比），用于抑制微小变动：
  - `cargo bench -p wp-stats --bench wp_stats_bench -- --noise-threshold 1.0`

Criterion 的历史数据与报告位于：`target/criterion/`，HTML 总览：`target/criterion/report/index.html`。

## 基准内容

1) record_task
- All/Item 目标模式
- 元素规模：100 / 1_000 / 10_000
- 每个 key 记 1 次，并在末尾 collect 以模拟完整路径

2) collect_top_n
- 评估 `collect_stat` 在 top-N 截断（含倍数保留）时的排序与截断开销
- 组合：max=10,n=100 与 max=100,n=10_000

3) merge_reports
- 两个 Collector 产出的 Report 合并（含键重叠、不同计数）
- 组合：max=50,n=1_000 与 max=200,n=5_000

> 注：为避免把字符串分配开销过多地计入，基准均预先构造 keys 列表，循环中仅借用 `&str`。

## 图表与报告解读

- 报告位置：`target/criterion/`，HTML 总览 `target/criterion/report/index.html`。
- time：每次基准的耗时统计区间（越低越好）；thrpt：吞吐（元素/秒，越高越好）。
- change：与基线的相对变化；`p < 0.05` 表示统计学显著；`within noise threshold` 表示变动很小。
- outliers：离群点，常见于短小基准或系统负载波动；建议在“稳定模式”下生成基线再对比。
- gnuplot 未安装时会使用 plotters 后端生成图表，功能不受影响。
