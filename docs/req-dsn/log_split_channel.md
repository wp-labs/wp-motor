# 日志分拆：Channel 派生源设计

## 需求
对于单条数据包，在拆分成多条日志（经 `send_to_src`/`split_to_src`）后，需要继续由解析链处理，并确保：
- 派生 Channel/TCP 源能够在上游耗尽后安全退出，避免 batch 模式挂住；
- 拆分后的日志仍按照 WPL/OML/Sink 路由正确落到业务 sink，不会因为路径写法不同而全部落入默认 sink。

## 场景实例
示例工程位于 `/Users/zuowenjian/devspace/wp-labs/wp-examples/core/split_pkgs`：
- 输入数据（`data/in_dat/gen.dat`）为 JSON 数据包，例如：
  ```
  {"date":1767006286.778936,"log":"180.57.30.149 - - [21/Jan/2025:01:40:02 +0800] \"GET /nginx-logo.png HTTP/1.1\" 500 ..."}
  ```
- 任务： 把log 拆出后再进行基于nginx WPL 进行解析
## 应用方案
- **WPL 配置**：`models/wpl/parse.wpl` 定义了 `pkg` 和 `raw` 两个规则——`pkg` 将 `log` 字段写入 `channel_mem`，`raw` 再从 Channel 解析：
  ```
  package /nginx {
      rule raw {
          (ip:sip,2*_,time/clf:recv_time<[,]>,http/request",http/status,digit,chars",http/agent",_")
      }
      rule pkg {
          (
              json | take(log) | json_unescape() | send_to_src()
          )
      }
  }
  ```
  使用者无需手动 drop Sender；真实源处理完成后，Channel 派生源会被框架 Stop/Close。
- **WPSRC 配置**：`topology/sources/wpsrc.toml` 中启用 `channel_src`：
  ```toml
  [[sources]]
  key = "file_1"
  connect = "file_src"

  [[sources]]
  key = "channel_mem"
  connect = "channel_src"
  ```
  即可为 `send_to_src` 提供目标 Channel。

## 主要设计
- **派生源识别**：Channel/TCP Factory 在 `SourceMeta` 中设置 `wp.role=derived` 标签。采集任务启动时根据标签将源拆成主组与派生组。
- **采集调度**：`start_picker_tasks` 引入 `PickerGroups`，`TaskManager` 在主组完成后向派生组广播 Stop，并调用 `close()`，确保 Channel/TCP 释放 Sender → EOF。
- **路由归一化**：`normalize_rule_path` / `normalize_match_input` 压缩多余 `/`，`extend_matches`、`update_sink_rule_index`、`alloc_parse_res` 等流程都使用归一化路径，避免 WPL/OML/Sink 命名不一致。
- **验证与回归**：相关单测（`channel::`、`send_to_src`、`extend::field_processor`、`wp-config normalize_match_input_collapse_slashes`）以及 `split_pkgs` 手动验证通过，确保业务 sink 能正确输出且 batch 正常退出。
