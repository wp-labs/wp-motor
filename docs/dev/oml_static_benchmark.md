# OML 静态块 vs 临时变量性能基准

为验证 `static { ... }` 块带来的收益，`crates/wp-oml/benches/oml_static_block.rs` 提供了一组 Criterion 基准，比较“静态模板”与“临时字段 + object”两种实现方式的吞吐差异。

## 基准内容

| 方案 | 描述 |
|------|------|
| `bench_static` | 在 `static` 块内定义模板对象，`match` 分支直接引用常量符号，`EventId/EventTemplate` 直接从常量对象读取 |
| `bench_temp` | 沿用旧模式：`__E1` 临时字段在每条记录上执行 `object { ... }`，`__target` 保存 match 结果，再通过 `read(__target)` 获取字段 |

两者的输入完全相同（单条 `Content: "foo message"`），基准只关注 evaluator 本身的计算成本。

## 运行方式

```bash
# 在仓库根目录执行
cargo bench -p wp-oml oml_static_block
```

输出示例（不同机器数值会有差异）：

```
oml_static_vs_temp/static_block
                        time:   [45.1 ns 46.0 ns 47.2 ns]
oml_static_vs_temp/temp_field
                        time:   [78.3 ns 80.5 ns 82.9 ns]
```

可以看到 `static_block` 方案省去了每条记录重新构造 `object { ... }` 的成本（约 30ns+），在大量模板或频繁匹配场景中收益更明显。

## 基准位置

- 代码：`crates/wp-oml/benches/oml_static_block.rs`
- `Cargo.toml` 已登记 `[[bench]] name = "oml_static_block"`
- 基准依赖 `criterion` 与 `wp-oml` 本身，无额外依赖

## 注意事项

- 该基准仅衡量 evaluator 内部开销，真实模型还会受 `match` 条件、FieldQueryCache 命中率等因素影响。
- 如需复现旧写法性能，确保保留 `__E*` 和 `__target` 的原始操作；任何额外简化（例如省掉 `read(__target)`）都会改变对比结果。

如需扩展基准覆盖更多模板大小或匹配复杂度，可在同一文件中追加新的 `String` 输入或不同的 `match` 场景。EOF
