# Warp-Parse 远程 Reload 管理面设计

## 背景

当前 `WPL / OML / KnowDB` 的运行时 reload 已经在引擎内部打通，统一入口是：

```rust
CommandType::LoadModel
```

但这只解决了“`wp-motor` 收到 reload 命令后如何执行”，还没有解决“外部如何安全地通知实例 reload”。

这里需要明确职责边界：

- `wp-motor` 是运行时引擎与 reload 执行层
- `warp-parse` 是宿主程序与管理面承载层

因此，HTTP 服务不应放在 `wp-motor` 里，而应放在 `warp-parse` 里。

## 设计结论

本方案选择：

1. **admin HTTP 由 `warp-parse` 实现**
2. **`wp-motor` 不实现 HTTP / TLS / 鉴权**
3. **`wp-motor` 只提供进程内 runtime command bus 与 reload 能力**
4. **`wproj engine reload` 调用的是 `warp-parse` 的 admin HTTP**

整体链路如下：

```text
remote client / wproj
    -> warp-parse admin HTTP
    -> warp-parse command gateway
    -> wp-motor runtime command bus
    -> CommandType::LoadModel
    -> runtime reload flow
    -> reload result
    -> HTTP response
```

## 为什么 HTTP 不放进 `wp-motor`

原因很直接：

1. `wp-motor` 应保持 runtime/core 属性，不引入 Web 管理面职责
2. TLS、Bearer Token、HTTP 路由、审计日志属于宿主程序层，不属于引擎核心
3. 如果以后还有别的宿主程序复用 `wp-motor`，不应该被 HTTP 管理面绑死
4. 这样可以让 `wp-motor` 更容易测试，避免 HTTP 与 runtime 生命周期耦合

因此，远程控制入口在 `warp-parse`，不是在 `wp-motor`。

## 为什么不采用 PID 文件作为正式远程控制接口

`PID` 文件仍然有价值，但只适合：

- 本机定位实例
- 本机 debug
- 辅助确认进程是否存在

它不适合作为正式远程控制协议，原因是：

1. `PID` 只能定位进程，不能表达结构化命令
2. 无法自然返回“执行中 / 成功 / 失败 / 超时强切”等结果
3. 不适合承载远程鉴权、审计和错误码

因此，本方案中：

- `PID` 文件继续保留
- 但正式远程控制走 `warp-parse` 的 admin HTTP

## 目标

1. 支持远程触发统一 reload
2. 明确 `warp-parse` 与 `wp-motor` 的职责边界
3. 支持结构化返回：成功、失败、进行中、超时强切
4. 支持首版可落地的远程安全方案：`HTTPS + Bearer Token`
5. 不额外引入 node agent

## 非目标

当前方案不追求：

1. `wp-motor` 内建 HTTP 管理服务
2. node agent
3. 首版支持 `mTLS`
4. 通用运维平台

## 职责边界

### `wp-motor` 负责什么

`wp-motor` 只负责运行时执行能力：

1. 接收 runtime command
2. 执行 `CommandType::LoadModel`
3. 保证 reload 单飞
4. 返回结构化执行结果
5. 提供运行状态快照

`wp-motor` 不负责：

1. HTTP server
2. TLS
3. Bearer Token 校验
4. HTTP 状态码
5. 远程访问审计格式

### `warp-parse` 负责什么

`warp-parse` 负责宿主层管理面：

1. 启动 admin HTTP server
2. 处理 HTTPS 与 Bearer Token
3. 将 HTTP 请求映射为 runtime command
4. 等待结果并返回 HTTP 响应
5. 记录管理面审计日志

## `wp-motor` 内部接口调整

当前内部通道是：

```rust
tokio::mpsc::channel::<CommandType>(...)
```

这对宿主层 HTTP 不够，因为 `warp-parse` 需要拿到 reload 结果并映射成 HTTP 响应。

建议 `wp-motor` 暴露带回包能力的进程内命令接口：

```rust
pub struct RuntimeCommandReq {
    pub request_id: String,
    pub command: CommandType,
    pub reply: tokio::sync::oneshot::Sender<RuntimeCommandResp>,
}

pub struct RuntimeCommandResp {
    pub request_id: String,
    pub accepted: bool,
    pub result: RuntimeCommandResult,
}

pub enum RuntimeCommandResult {
    ReloadDone,
    ReloadDoneWithForceReplace,
    ReloadRejectedBusy,
    ReloadFailed { reason: String },
}
```

还应补充一类运行状态查询接口，例如：

```rust
pub struct RuntimeStatusSnapshot {
    pub reloading: bool,
    pub last_reload_request_id: Option<String>,
    pub last_reload_result: Option<String>,
}
```

这样 `warp-parse` 才能实现：

- `POST /admin/v1/reloads/model`
- `GET /admin/v1/runtime/status`

## `warp-parse` admin HTTP 设计

### 路由

首版只建议提供两个接口：

```http
POST /admin/v1/reloads/model
GET  /admin/v1/runtime/status
```

第一版不要把接口做大，避免管理面过早膨胀。

### 触发 reload

```http
POST /admin/v1/reloads/model
```

请求头：

- `Authorization: Bearer <token>`
- `X-Request-Id: <uuid>` 可选

请求体：

```json
{
  "wait": true,
  "timeout_ms": 15000,
  "reason": "manual reload by operator"
}
```

字段语义：

- `wait=true`：HTTP 请求等待 reload 结果
- `timeout_ms`：仅表示 HTTP 等待时间，不改变引擎内部 drain 超时参数
- `reason`：审计用途

### 查询状态

```http
GET /admin/v1/runtime/status
```

返回内容建议至少包含：

- 实例标识
- 当前版本
- 当前是否正在 reload
- 最近一次 reload 结果
- 最近一次 reload 时间

## HTTP 响应语义

### reload 成功且未触发强切

```json
{
  "request_id": "6c6b8f8a-...",
  "accepted": true,
  "result": "reload_done",
  "force_replaced": false
}
```

HTTP：

```text
200 OK
```

### reload 成功，但发生超时强切

```json
{
  "request_id": "6c6b8f8a-...",
  "accepted": true,
  "result": "reload_done",
  "force_replaced": true,
  "warning": "graceful drain timed out, fallback to force replace"
}
```

HTTP：

```text
200 OK
```

### 正在 reload，拒绝重复请求

```json
{
  "request_id": "6c6b8f8a-...",
  "accepted": false,
  "result": "reload_in_progress"
}
```

HTTP：

```text
409 Conflict
```

### HTTP 等待超时

```json
{
  "request_id": "6c6b8f8a-...",
  "accepted": true,
  "result": "running"
}
```

HTTP：

```text
202 Accepted
```

### reload 失败

```json
{
  "request_id": "6c6b8f8a-...",
  "accepted": true,
  "result": "reload_failed",
  "error": "failed to build processing resource"
}
```

HTTP：

```text
500 Internal Server Error
```

## 并发控制

reload 必须由 `wp-motor` 侧保证单飞，不能只靠 `warp-parse` 的 HTTP handler 串行化。

原因是：

1. 以后不一定只有 HTTP 一种触发入口
2. 并发保护必须落在真正执行 reload 的一层

建议 `wp-motor` 内部显式维护：

```text
Idle
Reloading
```

规则如下：

1. `Idle` 才允许接收新的 reload
2. `Reloading` 状态下返回 `ReloadRejectedBusy`
3. 同一时刻只允许一个 reload 执行

## 安全设计

### 1. 安全责任在 `warp-parse`

安全边界必须放在 admin HTTP 宿主层，而不是 `wp-motor`。

也就是说：

- Bearer Token 校验在 `warp-parse`
- HTTPS 配置在 `warp-parse`
- 审计日志在 `warp-parse`
- `wp-motor` 不感知 token / header / TLS

### 2. 首版鉴权模式

当前只考虑一种正式鉴权模式：

```text
HTTPS + Bearer Token
```

原因：

1. 不依赖额外 CA / 证书体系
2. 落地快
3. 适合当前远程管理诉求

`mTLS` 当前不考虑。

### 3. 约束规则

admin API 应遵循以下约束：

1. 默认关闭
2. 管理面端口独立
3. 非 loopback 地址必须启用 HTTPS
4. 必须启用 Bearer Token
5. token 从本地受控文件读取
6. token 不允许写入日志
7. 管理请求必须记录审计日志

## 配置设计

HTTP 配置属于 `warp-parse`，不属于 `wp-motor`。

建议放在项目根目录的 `wparse.local.toml`（本地覆盖层，不参与 `project_remote` 同步）：

```toml
[admin_api]
enabled = true
bind = "127.0.0.1:19090"
request_timeout_ms = 15000
max_body_bytes = 4096

[admin_api.tls]
enabled = false
cert_file = ""
key_file = ""

[admin_api.auth]
token_file = "${HOME}/.warp_parse/admin_api.token"
```

约束规则：

1. `enabled=false` 时不启动 admin HTTP
2. 非 loopback 地址必须 `tls.enabled=true`
3. 鉴权固定为 Bearer Token，没有可配置的 auth mode
4. `token_file` 不存在或权限不安全时启动失败

## `wproj engine reload` 的角色

`wproj engine reload` 不维护独立 IPC 协议，而是调用 `warp-parse` 的 admin HTTP。

调用链路：

```text
wproj engine reload
    -> 读取目标实例地址和鉴权配置
    -> POST /admin/v1/reloads/model
    -> 输出结构化结果
```

因此：

- 正式远程接口是 `warp-parse admin HTTP`
- CLI 只是它的一个调用方

## 实现阶段建议

### Phase 1

先打通宿主层与引擎层边界：

1. `wp-motor` 提供带回包的 runtime command bus
2. `wp-motor` 提供 reload 单飞控制
3. `wp-motor` 提供状态快照
4. `warp-parse` 能调用 `LoadModel` 并拿到结果

### Phase 2

实现管理面：

1. `warp-parse` admin HTTP server
2. `POST /admin/v1/reloads/model`
3. `GET /admin/v1/runtime/status`
4. HTTPS + Bearer Token
5. 管理请求审计日志

### Phase 3

增强可运维性：

1. 增加 reload 历史缓存
2. 增加更细的状态查询
3. 增加限流与 IP 白名单

## 测试要求

### `wp-motor` 测试

1. 并发 reload 时只允许一个进入执行
2. reload 成功时正确返回 `ReloadDone`
3. 强切场景返回 `ReloadDoneWithForceReplace`
4. reload 失败时正确返回 `ReloadFailed`

### `warp-parse` 测试

1. Bearer Token 正确时允许访问
2. Bearer Token 错误时拒绝访问
3. 非 loopback 且未启用 HTTPS 时启动失败
4. HTTP handler 正确映射 `wp-motor` 返回结果

### 集成测试

1. 启动 `warp-parse`
2. 通过 admin HTTP 触发 reload
3. 验证 `wp-motor` reload 执行成功
4. 验证强切场景的返回语义

## 最终建议

当前阶段的推荐结论是：

1. **HTTP 放在 `warp-parse`**
2. **reload 执行能力放在 `wp-motor`**
3. **`wp-motor` 只暴露进程内 command bus 与状态接口**
4. **首版远程安全方案为 `HTTPS + Bearer Token`**
5. **`wproj engine reload` 只是 `warp-parse admin HTTP` 的 CLI 包装**

这样职责边界最清晰，也最符合后续实现与维护成本。
