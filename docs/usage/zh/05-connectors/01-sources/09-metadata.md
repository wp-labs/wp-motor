# Source Meta 

## 概述

Warp Parse 系统在解析数据时，会自动向 DataRecord 追加一些机制数据字段，用于追踪数据的来源和处理路径。这些机制数据字段以 `wp_` 前缀标识，为系统提供了数据溯源和调试能力。

## 机制数据字段列表

### 1. wp_event_id

- **字段类型**: 字符串 (String)
- **描述**: 事件的唯一标识符
- **来源**: 从 SourceEvent.event_id 获取
-**用途**: 追踪单个事件在系统中的完整处理流程

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
