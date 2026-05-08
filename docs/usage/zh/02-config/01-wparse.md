# Wparse配置

完整示例（推荐默认）
```toml
version = "1.0"
robust  = "normal"           # debug|normal|strict

[models]
wpl     = "./models/wpl"
oml     = "./models/oml"
knowledge = "./models/knowledge"

[topology]
sources = "./topology/sources"
sinks   = "./topology/sinks"

[performance]
rate_limit_rps = 10000        # 限速（records/second）
parse_workers  = 2            # 解析并发 worker 数
reload_timeout_ms = 10000     # reload 兜底超时（毫秒）；覆盖 graceful drain 与旧 processing 尾部清理
fetch_timeout_ms = 300 # realtime picker 每轮阻塞拉取的超时（毫秒）

[rescue]
path = "./data/rescue"        

[log_conf]
output = "File"               # Console|File|Both
level  = "warn,ctrl=info"

[log_conf.file]
path = "./data/logs"          # 文件输出目录；文件名自动取可执行名（wparse.log）

[stat]

[[stat.pick]]                 # 采集阶段统计
key    = "pick_stat"
target = "*"

[[stat.parse]]                # 解析阶段统计
key    = "parse_stat"
target = "*"

[[stat.sink]]                 # 下游阶段统计
key    = "sink_stat"
target = "*"
```

说明：
- `[models].knowledge` 是知识配置根目录，默认值为 `./models/knowledge`
- `semantic_dict.toml` 默认读取 `${models.knowledge}/semantic_dict.toml`
- `knowdb.toml` 默认读取 `${models.knowledge}/knowdb.toml`
<<<<<<< HEAD
- `fetch_timeout_ms` 默认 `300`；用于控制 realtime picker 单轮阻塞拉取的最长等待时间
- `reload_timeout_ms` 默认 `10000`；CLI `--reload-timeout-ms` 优先于配置文件
=======

## 变量化建议

`wparse.toml` 中的路径类字符串适合使用 `${VAR}` 变量化，例如：

```toml
[models]
knowledge = "${WORK_ROOT}/models/knowledge"

[rescue]
path = "${WORK_ROOT}/data/rescue"

[log_conf.file]
path = "${WORK_ROOT}/data/logs"
```

涉及外部变量文件、敏感值和 `sec_key.toml` 约定时，参考：[配置变量与安全字典（`${VAR}` / `sec_key.toml`）](08-variables_and_sec_key.md)。
>>>>>>> e12a12ddfff02e2df9314d213ed044b50a41be0e
