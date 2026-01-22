# RFC 6587 TCP Syslog 分帧实现

本文档描述 TCP Syslog 数据源中 RFC 6587 分帧器的实现逻辑。

## 概述

RFC 6587 定义了两种 TCP 传输 Syslog 消息的分帧方法：

1. **Octet Counting (推荐)** - 使用长度前缀：`<length> <message>`
2. **Non-Transparent-Framing** - 使用换行符分隔：`<message>\n`

本实现支持两种方法的自动检测和混合使用。

## 核心实现

### 分帧逻辑 (tcp.rs:215-300)

```
process_buffer(line_buffer, data, client_ip, sender):
    1. 将新数据追加到缓冲区
    2. 循环处理缓冲区中的完整帧：
       a. 首先尝试 Octet Counting 方法
       b. 如果失败，回退到 Newline 分隔方法
       c. 发送完整消息到通道
    3. 返回或等待更多数据
```

### Octet Counting 检测算法

```rust
// 在前 10 个字节中查找空格
if let Some(space_pos) = line_buffer[..min(len, 10)].iter().position(|&b| b == b' ') {
    // 检查前缀是否全为数字
    if length_str.chars().all(|c| c.is_ascii_digit()) {
        if let Ok(msg_len) = length_str.parse::<usize>() {
            // 验证长度合理 (0 < len < 10MB)
            if msg_len > 0 && msg_len < 10_000_000 {
                // 检查是否有完整消息
                if line_buffer.len() >= space_pos + 1 + msg_len {
                    // 提取消息
                    extract_message(msg_len)
                }
            }
        }
    }
}
```

### Newline 分隔回退

```rust
// 如果 Octet Counting 失败，使用换行符分隔
if let Some(newline_pos) = line_buffer.iter().position(|&b| b == b'\n') {
    let line = extract_line(newline_pos);
    // trim 掉行尾空白（包括 \r）
    send_message(line.trim_end())
}
```

## 关键特性

### 1. 自动模式检测

分帧器自动检测每个消息使用的分帧方法：

- 如果缓冲区开始是 `<数字> `，尝试 Octet Counting
- 否则查找换行符进行分隔
- 支持在同一连接中混合使用两种方法

### 2. 安全保护

#### 长度验证
```rust
msg_len > 0 && msg_len < 10_000_000  // 0 < 长度 < 10MB
```

#### 缓冲区溢出保护
```rust
if line_buffer.len() > 10_000_000 {
    log::warn!("buffer overflow, clearing");
    line_buffer.clear();
    return Err(SourceError::SupplierError("buffer overflow"));
}
```

#### 长度前缀搜索限制
```rust
let space_search_limit = std::cmp::min(line_buffer.len(), 10);
// 只在前 10 字节搜索空格，防止扫描大量数据
```

### 3. 特殊字符处理

- **Octet Counting**: 保留消息中所有字符（包括 `\n`, `\r`, `\t`）
- **Newline 分隔**: 使用 `trim_end()` 移除行尾空白（包括 `\r\n`）

### 4. 增量处理

支持消息跨多个 TCP 读取操作：

```rust
// 第一次读取: "34 <165>1 2023-01"
process_buffer(&mut buf, data1, ...) // 等待更多数据

// 第二次读取: "-01T00:00:00Z msg"
process_buffer(&mut buf, data2, ...) // 提取完整消息
```

## 处理流程

### 主流程图

```mermaid
flowchart TD
    Start([TCP 数据到达]) --> Append[追加到 line_buffer]
    Append --> CheckEmpty{缓冲区<br/>为空?}
    CheckEmpty -->|是| Wait([等待更多数据])
    CheckEmpty -->|否| CheckSpace{前 10 字节<br/>有空格?}

    CheckSpace -->|是| CheckDigits{前缀<br/>全数字?}
    CheckSpace -->|否| FindNewline{查找<br/>换行符}

    CheckDigits -->|是| ValidateLen{长度合法?<br/>0 < len < 10MB}
    CheckDigits -->|否| FindNewline

    ValidateLen -->|是| CheckData{数据<br/>足够?}
    ValidateLen -->|否| FindNewline

    CheckData -->|是| ExtractOctet[提取 Octet Counting 消息]
    CheckData -->|否| Wait

    FindNewline -->|找到| ExtractLine[提取行]
    FindNewline -->|未找到| CheckOverflow{缓冲区<br/>> 10MB?}

    CheckOverflow -->|是| Error([清空缓冲区并报错])
    CheckOverflow -->|否| Wait

    ExtractOctet --> FilterEmpty{消息<br/>非空?}
    ExtractLine --> Trim[trim_end 移除行尾空白]
    Trim --> FilterEmpty

    FilterEmpty -->|是| Send[发送到通道]
    FilterEmpty -->|否| Loop

    Send --> Loop[继续循环处理]
    Loop --> CheckEmpty

    style Start fill:#e1f5ff
    style Wait fill:#fff4e1
    style Error fill:#ffe1e1
    style Send fill:#e1ffe1
```

### 架构概览

```mermaid
graph TB
    subgraph TCP Connection
        Client[Syslog 客户端]
        Stream[TcpStream]
    end

    subgraph Connection Handler
        ReadLoop[读取循环]
        Buffer[BytesMut 缓冲区]
        Framer[process_buffer 分帧器]
    end

    subgraph Framing Logic
        OctetDetect[Octet Counting 检测]
        NewlineDetect[Newline 分隔检测]
        Validator[长度和安全验证]
    end

    subgraph Message Pipeline
        Channel[mpsc Channel]
        Receiver[数据接收器]
        DataPacket[DataPacket 输出]
    end

    Client -->|发送 Syslog| Stream
    Stream -->|read| ReadLoop
    ReadLoop -->|追加数据| Buffer
    Buffer -->|调用| Framer

    Framer --> OctetDetect
    Framer --> NewlineDetect
    OctetDetect --> Validator

    Framer -->|提取的消息| Channel
    Channel --> Receiver
    Receiver -->|封装| DataPacket

    style Framer fill:#ffd700
    style Validator fill:#ff6b6b
    style Channel fill:#4ecdc4
```

### 分帧决策树

```mermaid
flowchart LR
    Input[输入数据] --> Parse{解析类型}

    Parse -->|34 msg| OctetValid[✓ Octet Counting]
    Parse -->|msg\n| Newline[✓ Newline 分隔]
    Parse -->|12a msg\n| InvalidPrefix[✗ 无效前缀]
    Parse -->|99999999 msg\n| TooLarge[✗ 长度超限]
    Parse -->|-10 msg\n| Negative[✗ 负数长度]
    Parse -->|100 short| Incomplete[⏳ 数据不足]

    InvalidPrefix -->|降级| Newline
    TooLarge -->|降级| Newline
    Negative -->|降级| Newline
    Incomplete -->|等待| Wait[等待更多数据]

    OctetValid --> Extract[提取消息]
    Newline --> Extract

    style OctetValid fill:#90EE90
    style Newline fill:#87CEEB
    style Extract fill:#FFD700
    style Wait fill:#FFA500
    style InvalidPrefix fill:#FFB6C1
    style TooLarge fill:#FFB6C1
    style Negative fill:#FFB6C1
```

### 消息处理时序图

```mermaid
sequenceDiagram
    participant C as Syslog Client
    participant T as TcpStream
    participant H as Connection Handler
    participant F as Framer (process_buffer)
    participant V as Validator
    participant Ch as Channel
    participant R as Receiver

    C->>T: 发送: "34 <165>1 2023-01-01T00:00:00Z msg"
    T->>H: read() 返回数据
    H->>F: process_buffer(data)

    Note over F: 追加到 line_buffer
    F->>F: 检查前 10 字节
    F->>F: 找到空格在位置 2
    F->>F: 提取 "34" 全为数字 ✓

    F->>V: 验证长度 34
    V-->>F: 合法 (0 < 34 < 10MB) ✓

    F->>F: 检查数据: len=37 >= 2+1+34 ✓
    F->>F: 提取消息

    Note over F: msg = "<165>1 2023-01-01T00:00:00Z msg"

    F->>F: 过滤空消息 ✓
    F->>Ch: send((ip, msg))
    Ch->>R: 接收消息
    R->>R: 封装 DataPacket

    Note over F: 继续循环检查缓冲区
    F-->>H: 返回 Ok(())
```

### 错误场景时序图

```mermaid
sequenceDiagram
    participant C as Syslog Client
    participant F as Framer
    participant Ch as Channel

    rect rgb(255, 240, 240)
        Note over C,Ch: 场景 1: 无效长度前缀
        C->>F: "12a invalid\n"
        F->>F: 检查 "12a" 不全为数字 ✗
        F->>F: 回退到换行分隔
        F->>F: 找到 \n 位置 12
        F->>F: 提取 "12a invalid"
        F->>Ch: send((ip, "12a invalid"))
    end

    rect rgb(255, 255, 240)
        Note over C,Ch: 场景 2: 数据不足
        C->>F: "100 short"
        F->>F: 长度 100 合法 ✓
        F->>F: 需要 104 字节，只有 9 字节 ✗
        F-->>C: 等待更多数据
        C->>F: " message..."
        F->>F: 现在有足够数据 ✓
        F->>Ch: send((ip, message))
    end

    rect rgb(255, 245, 245)
        Note over C,Ch: 场景 3: 缓冲区溢出
        C->>F: [11MB 无换行数据]
        F->>F: 缓冲区 > 10MB ✗
        F->>F: 清空缓冲区
        F-->>C: 返回 Err(buffer overflow)
    end
```


## 错误处理

### 无效输入自动降级

| 输入 | 处理方式 |
|------|---------|
| `12a message\n` | 前缀非纯数字 → 换行分隔 |
| `-10 message\n` | 负数长度 → 换行分隔 |
| `99999999 msg\n` | 长度超限 → 换行分隔 |
| `100 short\n` | 数据不足 → 等待更多数据 |
| 无换行的长数据 | 缓冲区溢出检查 → 清空并报错 |

### 空消息过滤

```rust
if !msg.is_empty() {
    sender.send((client_ip, msg)).await
}
```

- 空行被忽略
- 纯空白行被 `trim_end()` 后忽略

## RFC 6587 合规性

### Section 3.4.1 - Non-Transparent-Framing

✅ 支持换行符 (`\n`) 作为消息分隔符
✅ 正确处理 CRLF (`\r\n`) 行尾
✅ 消息不能包含换行符（通过换行分隔时）

### Section 3.4.2 - Octet Counting (推荐)

✅ 格式：`<length> <message>`
✅ 长度为 ASCII 数字
✅ 空格分隔长度和消息
✅ 消息可包含任何字符（包括换行符）
✅ 不需要消息尾部的特殊字符

### 扩展支持

✅ 同一连接中混合使用两种方法
✅ 自动检测和切换
✅ 增量数据处理
✅ 缓冲区溢出保护

## 性能优化

### 1. 零拷贝操作

```rust
use bytes::BytesMut;
line_buffer.split_to(n)  // 高效的缓冲区分割
```

### 2. 早期检测

- 只检查前 10 字节是否包含空格
- 避免全缓冲区扫描

### 3. 批量处理

- 单次 `process_buffer` 调用可处理多个消息
- 循环直到没有完整帧

## 测试覆盖

实现包含 23 个专门的分帧测试：

### 测试分类图

```mermaid
mindmap
  root((RFC 6587<br/>分帧测试))
    基础功能
      Octet Counting
      Newline 分隔
      包含换行的消息
      部分消息缓冲
      多消息批处理
    边界条件
      空行处理
      空白行处理
      CRLF 处理
      零长度消息
      大消息 1MB
    混合模式
      Octet + Newline
      Newline + Octet
      交替模式
    错误处理
      非数字前缀
      负数长度
      长度不匹配
      超大长度
      缓冲区溢出
      无空格前缀
      空格位置 10
    性能测试
      100 条小消息
      100 条换行消息
      增量大消息
    RFC 合规
      RFC 示例
      特殊字符保留
      Trailer 行为
```

### 测试覆盖矩阵

```mermaid
%%{init: {'theme':'base'}}%%
graph LR
    subgraph 功能覆盖 [23 个测试]
        Basic[基础功能<br/>5 tests ✓]
        Edge[边界条件<br/>5 tests ✓]
        Mixed[混合模式<br/>3 tests ✓]
        Error[错误处理<br/>7 tests ✓]
        Perf[性能测试<br/>3 tests ✓]
    end

    subgraph RFC 合规 [RFC 6587]
        Section1[Section 3.4.1<br/>Non-Transparent]
        Section2[Section 3.4.2<br/>Octet Counting]
    end

    Basic --> Section1
    Basic --> Section2
    Edge --> Section1
    Mixed --> Section1
    Mixed --> Section2
    Error --> Section2

    style Basic fill:#90EE90
    style Edge fill:#87CEEB
    style Mixed fill:#FFD700
    style Error fill:#FFB6C1
    style Perf fill:#DDA0DD
    style Section1 fill:#F0E68C
    style Section2 fill:#F0E68C
```

### 测试执行流程

```mermaid
stateDiagram-v2
    [*] --> 编译测试
    编译测试 --> 运行基础功能测试
    运行基础功能测试 --> 运行边界条件测试
    运行边界条件测试 --> 运行混合模式测试
    运行混合模式测试 --> 运行错误处理测试
    运行错误处理测试 --> 运行性能测试
    运行性能测试 --> 运行RFC合规测试
    运行RFC合规测试 --> 验证结果

    验证结果 --> [*]: 37 passed ✓

    note right of 验证结果
        23 个分帧测试
        + 14 个其他测试
        = 37 个总测试
        全部通过 ✓
    end note
```


测试文件: `src/tcp_tests.rs`

## 使用示例

### 发送 Octet Counting 消息

```bash
# 单条消息
echo -n "34 <165>1 2023-01-01T00:00:00Z msg" | nc localhost 514

# 多条消息
echo -n "5 msg1210 second msg" | nc localhost 514
```

### 发送 Newline 分隔消息

```bash
# 单条消息
echo "<165>1 2023-01-01T00:00:00Z msg" | nc localhost 514

# 多条消息
printf "msg1\nmsg2\nmsg3\n" | nc localhost 514
```

### 混合模式

```bash
# Octet counting + Newline
echo -n "5 msg1msg2" | nc localhost 514
echo "" | nc localhost 514
```

## 相关文件

- **实现**: `src/tcp.rs` (line 215-300)
- **测试**: `src/tcp_tests.rs`
- **配置**: `src/builder.rs`, `src/utils.rs`
- **API**: `src/lib.rs`

## 参考资料

- [RFC 6587 - Transmission of Syslog Messages over TCP](https://datatracker.ietf.org/doc/html/rfc6587)
- [RFC 5424 - The Syslog Protocol](https://datatracker.ietf.org/doc/html/rfc5424)
