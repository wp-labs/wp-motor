# 语义词典配置说明

## 概述

语义词典系统为日志解析提供语义支持，包括：
- **系统内置词典**：代码内置，涵盖日志分析的常用词汇
- **外部配置支持**：可选，支持添加或替换内置词典

**配置文件位置**：`models/knowledge/semantic_dict.toml`（知识配置目录）

## 系统内置词典

### 内置词汇类别

所有词典都内置在代码中（`semantic_dict_loader.rs`），包括：

1. **核心词性** (`core_pos`) - 硬编码，不可配置
   - 用于 `extract_main_word` 函数
   - 包含：名词类 (n, nr, ns...)、动词类 (v, vn...)、英文 (eng) 等

2. **停用词** (`stop_words`)
   - 中文：的、了、在、是...
   - 英文：the, a, an, is...

3. **日志领域关键词** (`domain_words`) - **支持任意自定义分类**
   - 通用分类示例：
     - 日志级别：error, warn, info, debug...
     - 系统相关：exception, timeout, database...
     - 网络相关：http, tcp, socket...
     - 安全相关：attack, virus, malware...
   - 自定义分类示例：
     - 数据库：mysql, postgres, mongodb, redis...
     - 云原生：kubernetes, docker, pod, deployment...
     - 中间件：kafka, rabbitmq, elasticsearch...
     - 业务领域：order, payment, product...（根据实际业务定制）
   - **注意**：可以使用任意分类名，系统会自动合并所有分类的词汇

4. **状态词** (`status_words`)
   - 英文：failed, success, timeout...
   - 中文：失败、成功、超时...

5. **动作词** (`action_verbs`)
   - 英文：connect, login, process...
   - 中文：连接、登录、处理...

6. **实体名词** (`entity_nouns`)
   - 英文：connection, session, transaction...
   - 中文：连接、会话、事务...

## 外部配置（可选）

### 配置方式

通过环境变量指定外部配置文件：

```bash
export SEMANTIC_DICT_CONFIG=/path/to/custom_semantic_dict.toml
```

### 配置模式

#### 1. ADD 模式（默认）

将外部配置的词汇**添加**到系统内置词典：

```toml
version = 1
mode = "add"  # 默认值，可省略

[status_words]
english = ["aborted", "cancelled"]  # 添加到内置词典
chinese = ["中止", "取消"]
```

**特点：**
- 保留所有系统内置词汇
- 添加自定义词汇
- 适合扩展场景

#### 2. REPLACE 模式

用外部配置的词汇**完全替换**系统内置词典：

```toml
version = 1
mode = "replace"

[status_words]
english = ["running", "pending"]  # 仅使用这些词汇
chinese = ["运行中", "等待中"]
```

**特点：**
- 忽略系统内置词汇
- 完全自定义
- 适合特定领域定制（如 Kubernetes、金融等）

### 配置文件结构

```toml
# 版本号（必须）
version = 1

# 模式（可选，默认 "add"）
mode = "add"  # 或 "replace"

# 停用词（可选）
[stop_words]
chinese = ["词1", "词2"]
english = ["word1", "word2"]

# 日志领域关键词（可选）
# 注意：支持任意自定义分类名，不局限于下面的例子
[domain_words]
# 常用的通用分类
log_level = ["custom_level"]
system = ["cache", "queue"]
network = ["websocket", "grpc"]
security = ["firewall", "encryption"]

# 可以添加任意自定义分类
database = ["mysql", "postgres", "mongodb", "redis"]
cloud = ["kubernetes", "docker", "pod"]
middleware = ["kafka", "rabbitmq", "elasticsearch"]
business = ["order", "payment", "product"]
# ... 其他任意分类名

# 状态词（可选）
[status_words]
english = ["aborted", "cancelled"]
chinese = ["中止", "取消"]

# 动作词（可选）
[action_verbs]
english = ["deploy", "rollback"]
chinese = ["部署", "回滚"]

# 实体名词（可选）
[entity_nouns]
english = ["migration", "notification"]
chinese = ["迁移任务", "通知"]
```

## 配置示例

### 示例 1：扩展内置词典（ADD 模式）

文件：`models/knowledge/semantic_dict.toml`

```toml
version = 1
mode = "add"

# 添加业务特定的状态词
[status_words]
english = ["processing", "queued", "archived"]
chinese = ["处理中", "队列中", "已归档"]

# 添加业务特定的动作词
[action_verbs]
english = ["calculate", "aggregate", "transform"]
chinese = ["计算", "聚合", "转换"]
```

使用：
```bash
export SEMANTIC_DICT_CONFIG=models/knowledge/semantic_dict.toml
./wp-engine
```

### 示例 2：Kubernetes 专用词典（REPLACE 模式）

文件：`k8s_semantic_dict.toml`

```toml
version = 1
mode = "replace"

[stop_words]
chinese = ["的"]
english = ["the"]

# 使用 Kubernetes 专用的自定义分类
[domain_words]
# Kubernetes 核心资源
k8s_resources = ["pod", "deployment", "service", "namespace", "configmap", "daemonset", "statefulset"]

# Kubernetes 网络
k8s_network = ["ingress", "endpoint", "networkpolicy", "loadbalancer"]

# Kubernetes 安全
k8s_security = ["rbac", "serviceaccount", "secret", "rolebinding", "clusterrole"]

# Kubernetes 存储
k8s_storage = ["persistentvolume", "pvc", "storageclass"]

# 容器相关
container = ["docker", "containerd", "image", "registry"]

[status_words]
english = ["running", "pending", "failed", "succeeded", "crashloopbackoff", "imagepullbackoff", "terminating"]
chinese = []

[action_verbs]
english = ["create", "delete", "update", "scale", "rollout", "apply", "patch", "exec"]
chinese = []

[entity_nouns]
english = ["pod", "node", "cluster", "container", "volume", "controller"]
chinese = []
```

### 示例 3：仅扩展部分词典

只配置需要扩展的部分，其他使用内置：

```toml
version = 1
# mode = "add" 是默认值，可以省略

# 只扩展状态词，其他都使用内置词典
[status_words]
english = ["custom_status_1", "custom_status_2"]
chinese = ["自定义状态1"]
```

## 最佳实践

### 1. 选择合适的模式

- **ADD 模式**：大多数情况下使用
  - 在通用日志分析基础上添加业务词汇
  - 保留系统内置的常用词汇

- **REPLACE 模式**：特定领域使用
  - Kubernetes、云原生平台
  - 金融、医疗等行业专用系统
  - 需要精确控制词汇范围

### 2. 版本控制

将配置文件纳入版本控制：

```bash
git add models/knowledge/semantic_dict.toml
git commit -m "Add custom semantic dictionary for production"
```

### 3. 环境分离

为不同环境使用不同配置：

```bash
# 开发环境
export SEMANTIC_DICT_CONFIG=models/knowledge/dev_semantic_dict.toml

# 生产环境
export SEMANTIC_DICT_CONFIG=models/knowledge/prod_semantic_dict.toml
```

### 4. 配置验证

测试配置是否正确加载：

```bash
# 设置配置
export SEMANTIC_DICT_CONFIG=/path/to/config.toml

# 运行测试
cargo test -p wp-oml test_extract_main_word -- --nocapture
```

## 故障排查

### 配置加载失败

如果配置文件有问题，系统会输出警告并使用内置词典：

```
Warning: Failed to load external semantic dict config: <error message>.
```

**常见原因：**
1. 配置文件不存在
2. TOML 格式错误
3. 版本号不匹配

**解决方法：**
```bash
# 检查文件是否存在
ls -l $SEMANTIC_DICT_CONFIG

# 验证 TOML 格式
cat $SEMANTIC_DICT_CONFIG

# 检查版本号
grep "version" $SEMANTIC_DICT_CONFIG
```

### 词汇未生效

**检查步骤：**

1. 确认环境变量已设置：
   ```bash
   echo $SEMANTIC_DICT_CONFIG
   ```

2. 确认模式正确：
   - ADD 模式：新词汇应该**添加**到内置词典
   - REPLACE 模式：只有配置文件中的词汇生效

3. 运行测试验证：
   ```bash
   cargo test -p wp-oml test_global_semantic_dict -- --nocapture
   ```

## 性能考虑

### 加载时机

- 配置在应用程序启动时加载一次
- 使用 `Lazy` 实现延迟初始化
- 首次访问 `SEMANTIC_DICT` 时触发加载

### 内存占用

- 所有词典使用 `HashSet` 存储
- 查找时间复杂度：O(1)
- 典型内存占用：< 100KB

### 词典大小建议

| 词典类型 | 建议大小 | 说明 |
|---------|---------|------|
| 停用词 | 100-200 | 过多会影响关键词提取 |
| 领域词 | 200-500 | 业务核心词汇 |
| 状态词 | 50-100 | 结果相关词汇 |
| 动作词 | 100-200 | 行为相关词汇 |
| 实体名词 | 50-100 | 特殊名词 |

## 相关文档

- [extract_main_word 使用指南](../04-oml/functions/extract_main_word.md)
- [extract_subject_object 使用指南](../04-oml/functions/extract_subject_object.md)
- [配置示例](../../../config-examples/semantic_dict_example.toml)
- [替换模式示例](../../../config-examples/semantic_dict_replace_mode.toml)

## 技术实现

- **加载器**：`crates/wp-oml/src/core/evaluator/transform/pipe/semantic_dict_loader.rs`
- **使用代码**：`crates/wp-oml/src/core/evaluator/transform/pipe/extract_word.rs`
- **内置词典**：在 `SemanticDict::builtin()` 方法中定义

---

更新时间：2026-02-08
