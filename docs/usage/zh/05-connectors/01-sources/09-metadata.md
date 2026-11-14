# Source Meta 

## 概述

Warp Parse 系统在解析数据时，会自动向 DataRecord 追加一些机制数据字段，用于追踪数据的来源和处理路径。这些机制数据字段以 `wp_` 前缀标识，为系统提供了数据溯源和调试能力。

## 机制数据字段列表

### 1. wp_event_id

- **字段类型**: 字符串 (String)
- **描述**: 事件的唯一标识符
- **来源**: 从 SourceEvent.event_id 获取
-**用途**: 追踪单个事件在系统中的完整处理流程

取值是 64 位整数，布局固定为：

```text
 63            32 31        22 21                    0
+----------------+-----------+-----------------------+
|  秒 (32 bit)   | 实例(10)  |  每秒计数器 (22 bit)  |
+----------------+-----------+-----------------------+
```

- **秒**：Unix 时间戳（1970 起算），到 2106-02-07。高位在前，所以按**数值**比较时 `wp_event_id` 的大小就是时间序。
  > 若列建成 `VARCHAR`，字典序只在**位数相同**时成立。ID 今天 19 位，约 2044 年进到 20 位；跨过这个位数边界时字典序会错乱，排序请用 `CAST(... AS UNSIGNED)`，或给列加定长补零（如 `CHAR(20)`）。
- **实例号**：见下文 [实例号（WP_INS_ID）](#实例号wp_ins_id)；`instance_id - 100`。
- **每秒计数器**：每秒归零，上限 4,194,304（单实例 200 万 EPS 仍有 2 倍余量）。

**唯一性边界**：只要每个实例的 `WP_INS_ID` 不同，跨实例永不重复。但以下情况仍会重复，需在部署侧避免：

- 多个实例配置了相同的 `WP_INS_ID`（包括都没配置、都回退到默认 `100`）
- 下游列宽不够：`wp_event_id` 是字符串，若写成 32 位整数、`double`（超过 2^53 丢精度）或过短的 `VARCHAR`，看起来会像重复

> **关于 2038**：`wp_event_id` 在 wparse 内部始终是 `u64`，输出也是字符串字段，全程不涉及有符号数。只有当接收端的列建成有符号 `BIGINT` / `Int64` 时，2038-01-19 之后的值才会超出该列范围（值约等于 `secs × 2^32`，`secs` 到 `2^31` 时刚好越过 `i64::MAX`）。用字符串列或 `BIGINT UNSIGNED` 则没有这个问题，可一直用到 2106-02-07（`secs` 字段 32 位耗尽）。

### 2. wp_src_key

- **字段类型**: 字符串 (String)
- **描述**: 数据源的标识符
- **来源**: 从 SourceEvent.src_key 获取
- **用途**: 标识数据来源于哪个数据源（如 "syslog_1", "file_reader" 等）

### 3. wp_src_ip

- **字段类型**: IP 地址 (IP)
- **描述**: 数据源的客户端 IP 地址
- **来源**: 从 SourceEvent.ups_ip 获取
- **用途**: 记录发送数据的客户端 IP 地址，用于审计和定位

### 4. wp_event_md5

- **字段类型**: 字符串 (String, 32 位十六进制)
- **描述**: 事件原始 payload 的 MD5 指纹
- **来源**: `md5(payload)` 计算所得
- **用途**: 事件内容指纹，用于去重、比对、幂等校验
- **开关**: 由配置项 `gen_event_md5 = true` 控制（默认关闭）；且仅在 `gen_msg_id`（事件 meta 总开关，默认开启）开启时生效

### 5. 全局 tag（WP_GLOBAL_TAGS）

- **字段名**: 由实例自行定义，例如 `owner_id`；不能以 `wp_` 开头（保留给系统 meta，如 `wp_event_id`）
- **描述**: 每个 wparse 实例独立的全局字段，会盖到所有解析成功（含 `Partial`）的 record 上，包括 `copy_event_parse` 旁路 record
- **来源**: 实例自己的环境变量 `WP_GLOBAL_TAGS`，或实例 work root 下的 `.env`
- **格式**:
  - `WP_GLOBAL_TAGS="owner_id=team-a,env=prod"`
  - 值中包含 `,` 或 `=` 时使用 JSON：`WP_GLOBAL_TAGS='{"owner_id":"team-a","note":"a,b=c"}'`
- **覆盖语义**: 规则已产生同名字段时，全局 tag 会覆盖它（最终只保留一个字段）

`.env` 查找顺序：`<work_root>/.env` → `<work_root>/.warp_parse/.env`。已存在的进程环境变量优先，`.env` 不会覆盖它们。

## 实例号（WP_INS_ID）

每个 wparse 实例必须持有自己的实例号，它决定 `wp_event_id` 的实例段。多个实例共用同一个实例号会直接产出重复 ID。

- **环境变量**: `WP_INS_ID`
- **取值范围**: `100..=1123`（共 1024 个实例）
- **默认值**: `100`
- **读取顺序**: 进程环境（`docker -e` / systemd）优先；未设置时读取 work root 下的 `.env`，再退到 `<work_root>/.warp_parse/.env`，与 `WP_GLOBAL_TAGS` 完全一致
- **校验**: 非数字、越界、或首次发号后被改为其它值 → **启动失败**（不会静默回退）

```bash
# 方式一：环境变量
docker run -e WP_INS_ID=101 ...
# 方式二：work root/.env
WP_INS_ID=101
```

**未设置时会打 WARN**：单实例部署无所谓，多实例部署下所有未配置的实例都会拿到 `100`，ID 必然重复。

```text
WARN WP_INS_ID not set; using default instance id 100 (multiple instance will collide)
```

实例号写进历史数据后不可更改（否则同一实例前后两批数据的 ID 段会冲突），分配前请留足余量。

## 配置控制

上述机制字段由引擎配置（`wparse.toml`）控制：

| 配置项 | 默认 | 控制字段 |
|---|---|---|
| `gen_msg_id` | 开（解析期硬编码开启） | `wp_event_id` / `wp_src_key` / `wp_src_ip`（事件 meta 总开关） |
| `gen_event_md5` | 关 | `wp_event_md5`（嵌在 `gen_msg_id` 下，需 `gen_msg_id` 开启） |

字段一经盖戳，会出现在该事件产出的**所有** record 上——包括 `copy_event_parse` 产出的旁路 record。

## 关闭输出（wp_meta_disable）

若某个 sink 组不希望输出某个机制字段（如 `wp_event_md5`），可在 `[sink_group]` 配置 `wp_meta_disable`：

```toml
[sink_group]
name = "/sink/example"
wp_meta_disable = ["wp_event_md5"]
```

当前支持禁用的字段：`wp_oml_name`、`wp_event_md5`。该配置只在 sink 输出层过滤，不影响引擎盖戳。
