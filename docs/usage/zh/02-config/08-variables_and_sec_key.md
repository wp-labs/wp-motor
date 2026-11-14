# 安全变量与环境变量

本文面向配置使用者，说明两类变量的实际用法：
- 环境变量：用于环境名、目录、文件名、地址等非敏感值
- 安全变量：用于密码、Token、连接串等敏感值

在配置中，这两类变量都使用同一种引用方式：

```toml
${VAR_NAME}
```

如果变量不存在，也可以写缺省值：

```toml
${VAR_NAME:default_value}
```

## 先记结论

- 非安全变量：默认从当前 shell 环境读取
- 安全变量：默认从 `sec_key.toml` 读取
- 配置文件中的写法统一是 `${...}`
- 敏感值不要直接写在 `wpsrc.toml`、sink route、connector 配置里

## 环境变量怎么用

环境变量适合保存这些信息：
- 环境名，例如 `dev`、`test`、`prod`
- 普通目录路径
- 输出文件名
- 普通地址、端口、标签后缀

先在 shell 中设置：

```bash
export ENV=prod
export WORK_ROOT=/opt/wp
export OUTPUT_FILE=out.dat
export SRC_ADDR=127.0.0.1
```

然后在配置中引用：

```toml
[log_conf.file]
path = "${WORK_ROOT}/data/logs"

[[sources]]
key = "tcp_${ENV}"
connect = "tcp_src"
params = { addr = "${SRC_ADDR}", port = 19000 }

[output]
name = "gen_${ENV}"
params = { file = "${OUTPUT_FILE}" }
```

## 安全变量怎么用

安全变量适合保存这些信息：
- 数据库密码
- API Token
- 完整连接串
- Access Key / Secret Key

### 文件位置

如果使用 `warp-parse` 现成 CLI，安全变量文件默认从以下位置查找：

1. 当前工作目录下的 `.warp_parse/sec_key.toml`
2. 若不存在，则回退到 `$HOME/.warp_parse/sec_key.toml`

最常见的项目内写法是：

```text
./.warp_parse/sec_key.toml
```

### 文件内容

`sec_key.toml` 使用普通 TOML 键值对，例如：

```toml
sec_mysql_url = "mysql://writer:replace-me@127.0.0.1:3306/wparse"
sec_api_token = "replace-me"
sec_db_password = "replace-me"
```


因此上面的三个键，在配置里要这样引用：

```toml
${SEC_MYSQL_URL}
${SEC_API_TOKEN}
${SEC_DB_PASSWORD}
```

### 配置示例

```toml
[[sink_group.sinks]]
connect = "mysql_sink"

[sink_group.sinks.params]
url = "${SEC_MYSQL_URL}"
table = "nginx_prod"
```

## 环境变量与安全变量的关系

两者可以同时使用。

例如：

```toml
[[sink_group.sinks]]
connect = "mysql_sink"

[sink_group.sinks.params]
url = "${SEC_MYSQL_URL}"
table = "nginx_${ENV}"
```

这里：
- `${SEC_MYSQL_URL}` 来自 `sec_key.toml`
- `${ENV}` 来自 shell 环境变量

## 变量不存在时会怎样

`${VAR}`：
- 如果变量存在，就替换
- 如果变量不存在，就保持原样

`${VAR:default}`：
- 如果变量存在，就使用变量值
- 如果变量不存在，就使用 `default`

示例：

```toml
path = "${WORK_ROOT:/tmp/wp}/data/logs"
```

如果没有设置 `WORK_ROOT`，最终会使用 `/tmp/wp/data/logs`。

## 推荐做法

- 普通值放环境变量，例如 `ENV`、`WORK_ROOT`、`OUTPUT_FILE`
- 敏感值放 `sec_key.toml`
- 在配置里统一通过 `${...}` 引用
- 把 `.warp_parse/sec_key.toml` 加入 `.gitignore`
- 控制 `sec_key.toml` 文件权限，避免其他用户读取

## 不推荐做法

- 把密码、Token、连接串直接写进配置文件
- 在 `sec_key.toml` 里写 `SEC_MYSQL_URL = "..."`  
  这样会被处理成 `SEC_SEC_MYSQL_URL`
- 把敏感值放进 `tags`、`name`、`id` 这类容易出现在日志和诊断输出的位置

## 常见示例

### 示例 1：日志目录使用环境变量

```bash
export WORK_ROOT=/srv/wp
```

```toml
[log_conf.file]
path = "${WORK_ROOT}/data/logs"
```

### 示例 2：MySQL 连接串使用安全变量

`./.warp_parse/sec_key.toml`

```toml
mysql_url = "mysql://writer:replace-me@127.0.0.1:3306/wparse"
```

业务 route：

```toml
[[sink_group.sinks]]
connect = "mysql_sink"

[sink_group.sinks.params]
url = "${SEC_MYSQL_URL}"
table = "nginx_prod"
```

### 示例 3：环境变量与安全变量混用

```bash
export ENV=prod
```

`./.warp_parse/sec_key.toml`

```toml
mysql_url = "mysql://writer:replace-me@127.0.0.1:3306/wparse"
```

配置：

```toml
[sink_group.sinks.params]
url = "${SEC_MYSQL_URL}"
table = "nginx_${ENV}"
```

## 排错建议

- `${ENV}` 没替换：
  - 检查当前 shell 是否已 `export ENV=...`
  - 检查是不是在同一个终端里启动程序
- `${SEC_MYSQL_URL}` 没替换：
  - 检查 `./.warp_parse/sec_key.toml` 是否存在
  - 检查当前工作目录是否正确
  - 检查文件里是否写成了 `SEC_MYSQL_URL = "..."`，这属于错误写法
- 想确认 fallback：
  - 项目目录没有 `.warp_parse/sec_key.toml` 时，会继续尝试 `$HOME/.warp_parse/sec_key.toml`

## 实例级环境变量（WP_INS_ID / WP_GLOBAL_TAGS）

上面讲的 `${...}` 是**配置插值**。另有一类环境变量由引擎在启动时直接读取，不能写进 `wparse.toml`，也不能用 `${...}` 引用：

| 变量 | 作用 | 取值 |
| --- | --- | --- |
| `WP_INS_ID` | 实例号，决定 `wp_event_id` 的实例段 | `100..=1123`，默认 `100` |
| `WP_GLOBAL_TAGS` | 盖到每条记录上的全局字段 | `k=v,k2=v2` 或 JSON |

读取顺序与上面一致（先命中者生效，进程环境优先）：

1. 进程环境（`docker -e` / systemd `Environment=`）
2. `<work_root>/.env`
3. `<work_root>/.warp_parse/.env`

`WP_INS_ID` 取值非法（非数字或越界）会让 `wparse` **启动失败**；未设置时回退 `100` 并打 WARN —— 多实例部署下所有未配置的实例会共用 `100`，ID 必然重复，必须显式配置。

编号规划、K8s / Compose 示例与冲突边界见 `warp-parse` 仓库的 `docs/use/zh/operations/instance-id.md`。
