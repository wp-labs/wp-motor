# Sink 配置


## 目录与文件组织
- sink_root：用例内通常为 `<case>/sink`
  - business.d/**/*.toml：业务组路由（场景输出，支持子目录）
  - infra.d/**/*.toml：基础组路由（default/miss/residue/intercept/error/monitor，支持子目录）
  - defaults.toml：默认组级期望 [defaults.expect]

- connectors/sink.d/*.toml：连接器定义（loader 自 sink_root 向上查找最近的该目录）


## 路由文件格式
- 顶层
  - version（可选）
  - sink_group
    - name：组名（字符串）
    - oml / rule：推荐扁平写法；均可为字符串或字符串数组；用于匹配模型或规则。
    - wp_meta_disable：可选，组级元信息 payload 输出关闭列表；当前支持 `["wp_oml_name", "wp_event_md5"]`。
    - expect：可选，组级期望（覆盖 defaults）
    - sinks：数组，每项为单个 sink 定义
- 单个 sink 字段
  - name：该 sink 的名称（组内唯一）；未提供则按 [index] 回退
  - connect：引用连接器 id（兼容读取 `use`/`connector`）
  - params：对连接器默认参数的白名单覆盖（keys 必须在连接器 allow_override 列表中）
  - expect：可选，单 sink 期望（仅 ratio/tol/min/max，互斥关系：ratio/tol 与 min/max 不可混用）
  - filter：可选，拦截条件文件路径；命中 true 时丢弃该 sink 并发送至 intercept

## 变量化示例

```toml
version = "2.0"

[sink_group]
name = "mysql_${ENV}"
oml = ["*"]

[[sink_group.sinks]]
name = "writer"
connect = "mysql_sink"
tags = ["env:${ENV}"]

[sink_group.sinks.params]
url = "${SEC_MYSQL_URL}"
table = "nginx_${ENV}"
```

说明：
- route 名、sink 名、tags 以及 `params` 中的字符串都可以按 `${VAR}` 变量化
- 对账号、密码、DSN、Token 这类字段，优先使用 `SEC_` 变量
- 变量来源与 `sec_key.toml` 约定见：[配置变量与安全字典（`${VAR}` / `sec_key.toml`）](08-variables_and_sec_key.md)

## 配置示例：
### 基础组
```toml
version = "2.0"
[sink_group]
name = "intercept"

[[sink_group.sinks]]
name = "intercept"
connect = "file_kv_sink"
params = { base = "./out", file = "intercept.dat" }
```

### 业务组（filter）
```toml
version = "2.0"

[sink_group]
name = "/sink/filter"
oml  = ["/oml/sh*"]

[[sink_group.sinks]]
name = "all"
connect = "file_kv_sink"
params = { base = "./out/sink", file = "all.dat" }

[[sink_group.sinks]]
name = "safe"
connect = "file_kv_sink"
filter = "./sink/business.d/filter.conf"   # 命中 -> 拦截，不写 safe
params = { base = "./out/sink", file = "safe.dat" }
```


## 说明
- 标识规则
  - 组名：sink_group.name（例如 /sink/example/simple）
  - sink 名：name（组内唯一；未显式提供时按索引回退为 [0]/[1]/…）
- 元信息输出控制（wp_meta_disable）
  - `wp_meta_disable` 只能配置在 `[sink_group]`，作用于该组内所有 sinks。
  - 当前支持的字段：`wp_oml_name`、`wp_event_md5`，用于关闭 JSON/CSV 等结构化文本 payload 中的 OML 名称字段输出。
  - 该配置不会关闭 Arrow framed 的 frame header tag；Arrow tag 仍由 OML `name` 传递给 sink 后写入 frame header。
  - 不要把 `wp_meta_disable` 放到 `[[sink_group.sinks]].params` 或 `wpgen.output.params`；这些位置会被校验拒绝，避免运行时控制字段泄漏给 connector params。
- 过滤语义（filter）
  - filter 是“拦截条件”：表达式求值为 true 时，该条数据不写入该 sink，而是转发到基础组 intercept（framework/intercept）
  - 每个 sink 可独立设置 filter；与 expect 相互独立




## 校验提示
- 分母决定：
  - basis = total_input：总输入
  - basis = group_input：该组各 sink 行数之和（或 stats 中该组输入）
  - basis = model：按模型粒度统计（目前以组内 sinks 行数之和替代）
- min_samples：当分母为 0 或小于该值时，组校验被忽略（打印提示，不 fail）
- 当 route 为非文件类写入 fmt 时，validate 会提示“fmt 由后端决定，已忽略”。

## 常见排错
- 连接器未找到：检查 connectors/sink.d 是否存在对应 id；`wproj sinks list` 可查看引用关系
- 覆盖参数不生效：检查 allow_override 白名单
- 遇到 `${VAR}` 未替换：先检查上层程序是否已注入变量字典，再检查变量名拼写和 `sec_key.toml` / 环境变量内容
- filter 未生效：
  - 路径解析相对当前工作目录（建议写相对 sink_root 的相对路径）
  - 日志中会打印“found path/not found filter …”
  - 表达式语法需通过 TCondParser；可先用简单表达式试验
