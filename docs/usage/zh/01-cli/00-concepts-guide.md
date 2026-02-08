# WarpParse 核心概念速查

本文档帮助零基础用户快速理解 WarpParse 的核心概念和术语。

## 什么是 ETL？

ETL 是 Extract（提取）、Transform（转换）、Load（加载）的缩写。简单来说：

- **Extract（提取）**：从日志文件、数据库、消息队列等地方读取原始数据
- **Transform（转换）**：把非结构化的文本转换成结构化的数据（如 JSON）
- **Load（加载）**：把处理好的数据存储到目标位置（文件、数据库、ES 等）

### 类比理解

就像一个智能快递分拣系统：
- **Extract** = 收件（从各个地方收集包裹）
- **Transform** = 分类（按地址、类型分类打标签）
- **Load** = 派送（送到对应的目的地）

---

## WarpParse 核心术语

| 术语 | 英文 | 通俗解释 | 类比 |
|------|------|---------|------|
| **输入源** | Source | 数据从哪里来 | 水源（文件、TCP、Kafka） |
| **输出源** | Sink | 数据到哪里去 | 水池（文件、数据库、ES） |
| **连接器** | Connector | 如何连接数据源/目标 | 水管接头（配置连接参数） |
| **WPL** | Warp Processing Language | 数据提取规则语言 | 筛子（定义如何从文本中提取字段） |
| **OML** | Object Modeling Language | 数据组装规则语言 | 模具（定义如何组装成目标格式） |
| **规则** | Rule | 一条解析规则 | 一个筛子的设计图纸 |
| **字段** | Field | 要提取的数据项 | 筛子上的一个孔 |
| **管道** | Pipe | 数据处理流程 | 水管（数据流动的路径） |

---

## 数据流转过程

```
原始日志 → [Source 读取] → [WPL 解析] → [OML 转换] → [Sink 输出] → 目标存储
   ↓              ↓              ↓              ↓              ↓
文件/TCP      connectors     提取字段       组装对象      文件/DB/ES
```

### 详细说明

1. **Source 读取**：从配置的数据源读取原始数据
   - 文件：读取日志文件
   - TCP：监听网络端口接收数据
   - Kafka：从消息队列消费数据

2. **WPL 解析**：使用 WPL 规则从原始文本中提取字段
   - 识别 IP 地址、时间、数字等类型
   - 解析 JSON、KV 等结构化数据
   - 验证和过滤字段

3. **OML 转换**：使用 OML 规则组装输出格式
   - 字段重命名和映射
   - 类型转换（字符串→数字→时间）
   - 条件判断和计算

4. **Sink 输出**：将处理后的数据写入目标
   - 文件：保存为 JSON/CSV 等格式
   - 数据库：写入 MySQL/PostgreSQL
   - 搜索引擎：写入 Elasticsearch

---

## 快速入门路径

阶段一：  
1、跑通 Getting Started，看到数据流转起来  
2、学习 WPL 基础类型（ip、digit、time、chars）  
3、学习 OML 数据组装  
4、在 editor 学习了解 demo 样例  

阶段二：  
1、在 editor 上开始完成 T1 题目  
2、在自己空间下创建 wpl和oml  
3、配置 Source 和 Sink  
4、使用 wpgen 生成样例数据，wparse 进行执行查看输出  

---

## 项目目录结构

```
工作目录/
├── conf/              # 配置文件
│   ├── wparse.toml   # 解析引擎配置
│   └── wpgen.toml    # 数据生成器配置
│
├── connectors/        # 连接器定义
│   ├── sink.d/       # 输出连接器（数据去哪里）
│   └── source.d/     # 输入连接器（数据从哪来）
│
├── data/              # 数据目录
│   ├── in_dat/       # 输入数据（原始日志）
│   │   └── gen.dat  # 生成的测试数据
│   ├── out_dat/      # 输出数据（解析结果）
│   │   ├── demo.json       # 所有成功解析的记录
│   │   ├── default.dat     # 命中wpl未命中oml异常数据
│   │   ├── error.dat       # 异常数据
│   │   ├── miss.dat        # 未命中wpl异常数据
│   │   ├── monitor.dat     # 监控数据
│   │   └── residue.dat     # 残留数据（部分解析成功）
│   ├── logs/         # 运行日志
│   └── rescue/       # 失败数据（用于排查问题）
│
├── models/            # 模型定义
│   ├── wpl/          # WPL 解析规则
│   │   └── wp-space/
│   │       ├── parse.wpl    # 解析规则文件
│   │       └── sample.dat   # 规则测试样本
│   ├── oml/          # OML 转换规则
│   │   └── wp-space/
│   │       └── adm.oml      # 对象映射规则
│   └── knowledge/    # 知识库（SQL 查询等）
│
└── topology/          # 拓扑配置
    ├── sources/      # 数据源配置
    └── sinks/        # 数据目标配置
```

### 目录说明

- **conf/**：全局配置，如日志级别、性能参数
- **connectors/**：定义如何连接数据源和目标
- **data/**：所有数据文件的存放位置
  - **in_dat/**：存放原始输入数据
    - `gen.dat`：wpgen 生成的测试数据
  - **out_dat/**：存放解析后的输出数据
    - `all.json`：所有成功解析的记录（JSON 格式）
    - `default.dat`：默认输出文件
    - `error.dat`：解析过程中出错的记录
    - `ignore.dat`：根据规则被忽略的记录
    - `miss.dat`：缺失必需字段的记录
    - `monitor.dat`：监控和统计数据
    - `residue.dat`：部分字段解析成功的残留数据
  - **logs/**：运行时日志，用于调试和排查问题
  - **rescue/**：解析失败的原始数据，用于重新处理
- **models/**：核心业务逻辑（WPL 规则和 OML 规则）
  - **wpl/**：WPL 解析规则，按命名空间组织
    - `wp-space/parse.wpl`：具体的解析规则文件
    - `wp-space/sample.dat`：规则对应的测试样本
  - **oml/**：OML 转换规则，按命名空间组织
    - `wp-space/adm.oml`：对象映射和转换规则
  - **knowledge/**：知识库，如 SQL 查询、IP 库等
- **topology/**：数据流拓扑配置

---

## 常用命令速查

| 命令 | 功能 | 示例 |
|------|------|------|
| `wproj init` | 初始化项目 | `wproj init --mode full` |
| `wproj check` | 检查配置 | `wproj check` |
| `wproj data stat` | 统计数据 | `wproj data stat` |
| `wpgen sample` | 生成测试数据 | `wpgen sample -n 3000` |
| `wparse batch` | 批处理模式 | `wparse batch --stat 3 -p` |
| `wparse daemon` | 守护进程模式 | `wparse daemon --stat-print` |
| `wprescue batch` | 恢复失败数据 | `wprescue batch` |

---

## 下一步

阅读完本文档后，建议按以下顺序学习：

1. **[快速入门](./01-getting_started.md)**：10 分钟跑通第一个示例
2. **[WPL 语言基础](../03-wpl/01-quickstart.md)**：学习数据提取规则
3. **[OML 语言基础](../04-oml/README.md)**：学习数据组装规则

---

## 相关文档

- [快速入门](./01-getting_started.md)
- [CLI 工具集](./README.md)
- [WPL 语言](../03-wpl/README.md)
- [OML 语言](../04-oml/README.md)
