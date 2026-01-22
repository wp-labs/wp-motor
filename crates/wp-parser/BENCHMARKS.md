# Benchmark 测试指南

## 概述

wp-parser 提供了两套 benchmark 测试：

1. **quick_bench.rs** - 快速性能测试（推荐日常使用）
2. **parser_bench.rs** - 完整测试套件（详细性能分析）

## 快速开始

### 运行快速测试（~30秒）

```bash
cd crates/wp-parser
cargo bench --bench quick_bench
```

输出示例：
```
take_var_name_optimized    time: [26.7 ns]
take_json_path_optimized   time: [29.9 ns]
take_key_pair_optimized    time: [36.9 ns]
take_parentheses_nested    time: [88.7 ns]
scope_eval_nested          time: [7.2 ns]
get_scope_optimized        time: [51.6 ns]
```

### 运行完整测试（~5分钟）

```bash
cd crates/wp-parser
cargo bench --bench parser_bench
```

## 测试覆盖

### quick_bench - 关键优化点验证

| Benchmark | 测试内容 | 验证目标 |
|-----------|---------|---------|
| `take_var_name_optimized` | 变量名解析 | 零拷贝优化 |
| `take_json_path_optimized` | JSON路径解析 | 零拷贝优化 |
| `take_key_pair_optimized` | 键值对解析 | 减少堆分配 |
| `take_parentheses_nested` | 嵌套括号解析 | Bug修复验证 |
| `scope_eval_nested` | 作用域评估 | 核心算法性能 |
| `get_scope_optimized` | 作用域提取 | char解析器优化 |

### parser_bench - 完整功能测试

包含以下测试组：
- **take_var_name**: 4种复杂度的变量名
- **take_json_path**: 4种JSON路径模式
- **take_key_pair**: 3种键值对场景
- **take_parentheses_val**: 4种括号嵌套级别
- **scope_eval**: 5种不同分隔符的作用域
- **get_scope**: 4种作用域提取场景
- **peek_one**: 单字符预览
- **real_world_scenarios**: 3种实际应用场景

## 解读结果

### 时间单位说明
- **ns** (纳秒): 1/1,000,000,000 秒
- **µs** (微秒): 1/1,000,000 秒
- **ms** (毫秒): 1/1,000 秒

### 性能参考值

| 性能等级 | 时间范围 | 评价 |
|---------|---------|------|
| 🚀 极快 | < 10 ns | 优秀 |
| ⚡ 很快 | 10-50 ns | 良好 |
| ✅ 快速 | 50-100 ns | 可接受 |
| ⚠️ 一般 | 100-500 ns | 需关注 |
| 🐌 慢 | > 500 ns | 需优化 |

### 离群值 (Outliers)

Criterion 会报告离群值：
```
Found 11 outliers among 100 measurements (11.00%)
  1 (1.00%) high mild
  10 (10.00%) high severe
```

- **low severe/mild**: 比平均值快很多（可能是缓存命中）
- **high severe/mild**: 比平均值慢很多（可能是缓存未命中或GC）
- **< 5%**: 正常
- **5-15%**: 可接受
- **> 15%**: 需要调查性能不稳定原因

## 性能对比

### 查看历史趋势

Criterion 自动保存历史数据：

```bash
# 首次运行建立基准
cargo bench --bench quick_bench

# 修改代码后再次运行
cargo bench --bench quick_bench

# 输出会显示与上次的对比
# Example: time: [26.7 ns 26.9 ns 27.1 ns] change: [-5.2% -3.8% -2.4%]
#                                                   ↑ 表示比上次快了3.8%
```

### 与其他解析器对比

| 解析器 | take_var_name | 说明 |
|-------|--------------|------|
| wp-parser (优化后) | ~27 ns | 零拷贝 |
| wp-parser (优化前) | ~47 ns* | String分配 |
| nom (类似功能) | ~35 ns* | 参考值 |

*估算值，实际性能取决于具体实现

## 自定义 Benchmark

### 添加新测试

编辑 `benches/quick_bench.rs`:

```rust
c.bench_function("my_custom_test", |b| {
    b.iter(|| {
        let mut data = "test input";
        my_parser.parse_next(black_box(&mut data)).unwrap()
    });
});
```

### 调整采样数

```rust
criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(100)      // 增加采样数提高精度
        .measurement_time(Duration::from_secs(10));  // 增加测量时间
    targets = quick_benchmarks
}
```

## 持续集成

### GitHub Actions 示例

```yaml
- name: Run benchmarks
  run: |
    cd crates/wp-parser
    cargo bench --bench quick_bench -- --output-format bencher | tee output.txt

- name: Store benchmark result
  uses: benchmark-action/github-action-benchmark@v1
  with:
    tool: 'cargo'
    output-file-path: output.txt
```

## 性能优化 Checklist

在修改代码后，运行 benchmark 验证：

- [ ] 没有性能退化（< 5% 变慢）
- [ ] 预期的优化生效（> 10% 变快）
- [ ] 离群值保持正常范围（< 15%）
- [ ] 所有测试用例通过
- [ ] 更新 PERFORMANCE.md 文档

## 故障排查

### 性能波动大
- 关闭后台应用减少干扰
- 使用 `--sample-size 200` 增加采样
- 检查是否有 CPU 节流

### 编译失败
```bash
# 确保依赖最新
cargo update

# 清理重新编译
cargo clean
cargo bench
```

### 结果不符合预期
- 检查是否在 `--release` 模式
- 确认 `black_box()` 正确使用防止优化消除
- 查看 Criterion 生成的详细报告：`target/criterion/report/index.html`

## 参考资源

- [Criterion.rs 文档](https://bheisler.github.io/criterion.rs/book/)
- [Rust 性能优化指南](https://nnethercote.github.io/perf-book/)
- [PERFORMANCE.md](./PERFORMANCE.md) - 完整性能报告
