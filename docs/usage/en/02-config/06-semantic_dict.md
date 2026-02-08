# Semantic Dictionary Configuration

## Overview

The semantic dictionary system provides semantic support for log parsing, including:
- **Built-in System Dictionary**: Code-embedded, covering common vocabulary for log analysis
- **External Configuration Support**: Optional, supports adding or replacing the built-in dictionary

**Configuration File Location**: `models/knowledge/semantic_dict.toml` (knowledge configuration directory)

## Built-in System Dictionary

### Built-in Vocabulary Categories

All dictionaries are built into the code (`semantic_dict_loader.rs`), including:

1. **Core POS** (`core_pos`) - Hard-coded, not configurable
   - Used for the `extract_main_word` function
   - Includes: nouns (n, nr, ns...), verbs (v, vn...), English (eng), etc.

2. **Stop Words** (`stop_words`)
   - Chinese: 的, 了, 在, 是...
   - English: the, a, an, is...

3. **Log Domain Keywords** (`domain_words`) - **Supports any custom categories**
   - Common category examples:
     - Log levels: error, warn, info, debug...
     - System-related: exception, timeout, database...
     - Network-related: http, tcp, socket...
     - Security-related: attack, virus, malware...
   - Custom category examples:
     - Database: mysql, postgres, mongodb, redis...
     - Cloud-native: kubernetes, docker, pod, deployment...
     - Middleware: kafka, rabbitmq, elasticsearch...
     - Business domain: order, payment, product... (customized based on actual business)
   - **Note**: You can use any category name; the system will automatically merge all category vocabularies

4. **Status Words** (`status_words`)
   - English: failed, success, timeout...
   - Chinese: 失败, 成功, 超时...

5. **Action Verbs** (`action_verbs`)
   - English: connect, login, process...
   - Chinese: 连接, 登录, 处理...

6. **Entity Nouns** (`entity_nouns`)
   - English: connection, session, transaction...
   - Chinese: 连接, 会话, 事务...

## External Configuration (Optional)

### Configuration Method

Specify the external configuration file via environment variable:

```bash
export SEMANTIC_DICT_CONFIG=/path/to/custom_semantic_dict.toml
```

### Configuration Modes

#### 1. ADD Mode (Default)

**Add** external configuration vocabulary to the built-in system dictionary:

```toml
version = 1
mode = "add"  # Default value, can be omitted

[status_words]
english = ["aborted", "cancelled"]  # Add to built-in dictionary
chinese = ["中止", "取消"]
```

**Features:**
- Retains all built-in system vocabulary
- Adds custom vocabulary
- Suitable for extension scenarios

#### 2. REPLACE Mode

**Completely replace** the built-in system dictionary with external configuration vocabulary:

```toml
version = 1
mode = "replace"

[status_words]
english = ["running", "pending"]  # Only use these vocabularies
chinese = ["运行中", "等待中"]
```

**Features:**
- Ignores built-in system vocabulary
- Fully customizable
- Suitable for domain-specific customization (e.g., Kubernetes, finance)

### Configuration File Structure

```toml
# Version number (required)
version = 1

# Mode (optional, defaults to "add")
mode = "add"  # or "replace"

# Stop words (optional)
[stop_words]
chinese = ["word1", "word2"]
english = ["word1", "word2"]

# Log domain keywords (optional)
# Note: Supports any custom category names, not limited to the examples below
[domain_words]
# Common general categories
log_level = ["custom_level"]
system = ["cache", "queue"]
network = ["websocket", "grpc"]
security = ["firewall", "encryption"]

# Can add any custom categories
database = ["mysql", "postgres", "mongodb", "redis"]
cloud = ["kubernetes", "docker", "pod"]
middleware = ["kafka", "rabbitmq", "elasticsearch"]
business = ["order", "payment", "product"]
# ... any other category names

# Status words (optional)
[status_words]
english = ["aborted", "cancelled"]
chinese = ["中止", "取消"]

# Action verbs (optional)
[action_verbs]
english = ["deploy", "rollback"]
chinese = ["部署", "回滚"]

# Entity nouns (optional)
[entity_nouns]
english = ["migration", "notification"]
chinese = ["迁移任务", "通知"]
```

## Configuration Examples

### Example 1: Extend Built-in Dictionary (ADD Mode)

File: `models/knowledge/semantic_dict.toml`

```toml
version = 1
mode = "add"

# Add business-specific status words
[status_words]
english = ["processing", "queued", "archived"]
chinese = ["处理中", "队列中", "已归档"]

# Add business-specific action verbs
[action_verbs]
english = ["calculate", "aggregate", "transform"]
chinese = ["计算", "聚合", "转换"]
```

Usage:
```bash
export SEMANTIC_DICT_CONFIG=models/knowledge/semantic_dict.toml
./wp-engine
```

### Example 2: Kubernetes-specific Dictionary (REPLACE Mode)

File: `k8s_semantic_dict.toml`

```toml
version = 1
mode = "replace"

[stop_words]
chinese = ["的"]
english = ["the"]

# Use Kubernetes-specific custom categories
[domain_words]
# Kubernetes core resources
k8s_resources = ["pod", "deployment", "service", "namespace", "configmap", "daemonset", "statefulset"]

# Kubernetes network
k8s_network = ["ingress", "endpoint", "networkpolicy", "loadbalancer"]

# Kubernetes security
k8s_security = ["rbac", "serviceaccount", "secret", "rolebinding", "clusterrole"]

# Kubernetes storage
k8s_storage = ["persistentvolume", "pvc", "storageclass"]

# Container-related
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

### Example 3: Extend Only Specific Dictionaries

Configure only the parts that need extension, use built-in for others:

```toml
version = 1
# mode = "add" is the default, can be omitted

# Only extend status words, use built-in dictionary for others
[status_words]
english = ["custom_status_1", "custom_status_2"]
chinese = ["自定义状态1"]
```

## Best Practices

### 1. Choose the Right Mode

- **ADD Mode**: Use in most cases
  - Add business vocabulary based on general log analysis
  - Retain built-in common vocabulary

- **REPLACE Mode**: Use for specific domains
  - Kubernetes, cloud-native platforms
  - Industry-specific systems like finance, healthcare
  - Need precise control over vocabulary scope

### 2. Version Control

Include configuration file in version control:

```bash
git add models/knowledge/semantic_dict.toml
git commit -m "Add custom semantic dictionary for production"
```

### 3. Environment Separation

Use different configurations for different environments:

```bash
# Development environment
export SEMANTIC_DICT_CONFIG=models/knowledge/dev_semantic_dict.toml

# Production environment
export SEMANTIC_DICT_CONFIG=models/knowledge/prod_semantic_dict.toml
```

### 4. Configuration Validation

Test if configuration loads correctly:

```bash
# Set configuration
export SEMANTIC_DICT_CONFIG=/path/to/config.toml

# Run tests
cargo test -p wp-oml test_extract_main_word -- --nocapture
```

## Troubleshooting

### Configuration Load Failure

If there's a problem with the configuration file, the system will output a warning and use the built-in dictionary:

```
Warning: Failed to load external semantic dict config: <error message>.
```

**Common Causes:**
1. Configuration file does not exist
2. TOML format error
3. Version number mismatch

**Solutions:**
```bash
# Check if file exists
ls -l $SEMANTIC_DICT_CONFIG

# Verify TOML format
cat $SEMANTIC_DICT_CONFIG

# Check version number
grep "version" $SEMANTIC_DICT_CONFIG
```

### Vocabulary Not Taking Effect

**Troubleshooting Steps:**

1. Confirm environment variable is set:
   ```bash
   echo $SEMANTIC_DICT_CONFIG
   ```

2. Confirm correct mode:
   - ADD mode: New vocabulary should **add** to built-in dictionary
   - REPLACE mode: Only vocabulary in configuration file takes effect

3. Run tests to verify:
   ```bash
   cargo test -p wp-oml test_global_semantic_dict -- --nocapture
   ```

## Performance Considerations

### Load Timing

- Configuration is loaded once at application startup
- Uses `Lazy` for lazy initialization
- Loading triggered on first access to `SEMANTIC_DICT`

### Memory Usage

- All dictionaries use `HashSet` for storage
- Lookup time complexity: O(1)
- Typical memory usage: < 100KB

### Dictionary Size Recommendations

| Dictionary Type | Recommended Size | Notes |
|----------------|------------------|-------|
| Stop Words | 100-200 | Too many affects keyword extraction |
| Domain Words | 200-500 | Core business vocabulary |
| Status Words | 50-100 | Result-related vocabulary |
| Action Verbs | 100-200 | Behavior-related vocabulary |
| Entity Nouns | 50-100 | Special nouns |

## Related Documentation

- [extract_main_word Usage Guide](../04-oml/functions/extract_main_word.md)
- [extract_subject_object Usage Guide](../04-oml/functions/extract_subject_object.md)
- [Configuration Example](../../../config-examples/semantic_dict_example.toml)
- [Replace Mode Example](../../../config-examples/semantic_dict_replace_mode.toml)

## Technical Implementation

- **Loader**: `crates/wp-oml/src/core/evaluator/transform/pipe/semantic_dict_loader.rs`
- **Usage Code**: `crates/wp-oml/src/core/evaluator/transform/pipe/extract_word.rs`
- **Built-in Dictionary**: Defined in the `SemanticDict::builtin()` method

---

Last Updated: 2026-02-08
