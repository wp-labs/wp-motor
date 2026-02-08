# Wpgen配置

wpgen 是数据生成工具，用于按照规则或样本生成测试数据。

## 基础配置

配置文件路径：`conf/wpgen.toml`

```toml
version = "1.0"

[generator]
mode = "sample"          # 生成模式：rule | sample
count = 1000             # 生成总条数（可选）
duration_secs = 60       # 生成持续时间（秒，可选，与 count 二选一）
speed = 1000             # 恒定速率（行/秒），0 为无限速
parallel = 1             # 并行度
rule_root = "./rules"    # 规则目录（mode=rule 时使用）
sample_pattern = "*.txt" # 样本文件匹配模式（mode=sample 时使用）

[output]
# 引用 connectors/sink.d 中的连接器 id
connect = "file_kv_sink"
name = "gen_out"
# 覆写连接器参数（仅 allow_override 白名单内的键）
params = { base = "./src_dat", file = "gen.dat" }

[logging]
level = "warn"
output = "file"
file_path = "./data/logs/"
```

## 动态速度模型

除了使用 `speed` 字段指定恒定速率外，还可以使用 `speed_profile` 配置动态速度变化模型。
当 `speed_profile` 存在时，`speed` 字段将被忽略。

### 恒定速率 (constant)

固定速率生成数据。

```toml
[generator.speed_profile]
type = "constant"
rate = 5000              # 每秒生成行数
```

### 正弦波动 (sinusoidal)

速率按正弦曲线周期性波动，模拟周期性负载变化。

```toml
[generator.speed_profile]
type = "sinusoidal"
base = 5000              # 基准速率（行/秒）
amplitude = 2000         # 波动幅度（行/秒）
period_secs = 60.0       # 周期长度（秒）
```

速率范围：`[base - amplitude, base + amplitude]`，即上例中为 3000-7000 行/秒。

### 阶梯变化 (stepped)

速率按预定义的阶梯序列变化，适合模拟分阶段负载测试。

```toml
[generator.speed_profile]
type = "stepped"
# 格式：[[持续时间(秒), 速率], ...]
steps = [
    [30.0, 1000],        # 前 30 秒：1000 行/秒
    [30.0, 5000],        # 接下来 30 秒：5000 行/秒
    [30.0, 2000]         # 最后 30 秒：2000 行/秒
]
loop_forever = true      # 是否循环执行（默认 false）
```

### 突发模式 (burst)

在基准速率上随机触发高速突发，模拟突发流量场景。

```toml
[generator.speed_profile]
type = "burst"
base = 1000              # 基准速率（行/秒）
burst_rate = 10000       # 突发时速率（行/秒）
burst_duration_ms = 500  # 突发持续时间（毫秒）
burst_probability = 0.05 # 每秒触发突发的概率（0.0-1.0）
```

### 渐进模式 (ramp)

速率从起始值线性变化到目标值，适合压力递增测试。

```toml
[generator.speed_profile]
type = "ramp"
start = 100              # 起始速率（行/秒）
end = 10000              # 目标速率（行/秒）
duration_secs = 300.0    # 变化持续时间（秒）
```

达到目标速率后将保持该速率。支持正向（递增）和反向（递减）。

### 随机波动 (random_walk)

速率在基准值附近随机波动，模拟不规则负载。

```toml
[generator.speed_profile]
type = "random_walk"
base = 5000              # 基准速率（行/秒）
variance = 0.3           # 波动范围（0.0-1.0），0.3 表示 ±30%
```

速率范围：`[base * (1 - variance), base * (1 + variance)]`

### 复合模式 (composite)

组合多个速度模型，支持多种组合方式。

```toml
[generator.speed_profile]
type = "composite"
combine_mode = "average" # 组合方式：average | max | min | sum

# 子模型列表
[[generator.speed_profile.profiles]]
type = "sinusoidal"
base = 5000
amplitude = 2000
period_secs = 60.0

[[generator.speed_profile.profiles]]
type = "random_walk"
base = 5000
variance = 0.1
```

组合方式说明：
- `average`：取所有子模型速率的平均值（默认）
- `max`：取所有子模型速率的最大值
- `min`：取所有子模型速率的最小值
- `sum`：累加所有子模型速率

## 配置示例

### 示例 1：简单恒定速率

```toml
version = "1.0"

[generator]
mode = "sample"
count = 10000
speed = 5000
parallel = 2

[output]
connect = "file_json_sink"
params = { base = "./data", file = "output.dat" }

[logging]
level = "info"
output = "file"
file_path = "./logs"
```

### 示例 2：渐进压力测试

```toml
version = "1.0"

[generator]
mode = "rule"
duration_secs = 600      # 运行 10 分钟
parallel = 4
rule_root = "./rules"

[generator.speed_profile]
type = "ramp"
start = 100
end = 20000
duration_secs = 300.0    # 5 分钟内从 100 提升到 20000

[output]
connect = "kafka_sink"
params = { topic = "test-topic" }

[logging]
level = "warn"
output = "file"
file_path = "./logs"
```

### 示例 3：模拟真实业务负载

```toml
version = "1.0"

[generator]
mode = "sample"
duration_secs = 3600     # 运行 1 小时
parallel = 8

[generator.speed_profile]
type = "composite"
combine_mode = "average"

# 基础周期性波动（模拟日间/夜间流量差异）
[[generator.speed_profile.profiles]]
type = "sinusoidal"
base = 10000
amplitude = 5000
period_secs = 300.0

# 叠加随机噪声
[[generator.speed_profile.profiles]]
type = "random_walk"
base = 10000
variance = 0.15

[output]
connect = "tcp_sink"
params = { host = "127.0.0.1", port = 9000 }

[logging]
level = "info"
output = "both"
file_path = "./logs"
```

## 运行规则

- `wpgen` 会在加载 `conf/wpgen.toml` 时，若检测到 `[output].connect`：
  - 从 `ENGINE_CONF.sink_root` 向上查找最近的 `connectors/sink.d/` 目录
  - 读取目标连接器并与 `params` 合并（仅允许 `allow_override` 中的键）

- 当配置了 `parallel > 1` 时，速度模型会自动按并行度分配，确保总速率符合预期

- `count` 和 `duration_secs` 二选一：
  - 设置 `count` 时，生成指定条数后停止
  - 设置 `duration_secs` 时，运行指定秒数后停止
  - 两者都未设置时，将持续运行直到手动停止
