# WPL 规则语言

WPL (Warp Processing Language) 是 Warp Parse 解析子系统（warp-parse）使用的规则语言，用于描述字段抽取、协议解析与简单判定逻辑。

---

## 📚 文档导航

### 按学习路径

```
🆕 新手入门
   ↓
01-quickstart.md ────→ 5分钟上手，复制即用
   ↓
07-complete-types-examplemd ──→ 🌟 完整功能演示（强烈推荐）
   ↓
02-core-concepts.md ──→ 理解设计理念和核心概念
   ↓
03-practical-guide.md → 按任务查找解决方案
   ↓
04-language-reference.md → 查阅类型和语法
   ↓
05-functions-reference.md → 查阅函数
```

### 按用户角色

| 我是... | 推荐阅读 |
|---------|---------|
| **WPL 新手** | [01-quickstart.md](./01-quickstart.md) → [02-core-concepts.md](./02-core-concepts.md) |
| **日常使用者** | [03-practical-guide.md](./03-practical-guide.md) - 按任务查找 |
| **开发者/集成** | [04-language-reference.md](./04-language-reference.md) + [05-functions-reference.md](./05-functions-reference.md) |
| **编译器开发** | [06-grammar-reference.md](./06-grammar-reference.md) - EBNF 语法 |

### 按任务查找

| 我想... | 查看文档 |
|---------|---------|
| 🚀 快速上手 | [01-quickstart.md](./01-quickstart.md) |
| 🎯 **查看完整类型示例** | **[07-complete-types-example.md](./07-complete-types-example.md)** |
| 💡 理解概念 | [02-core-concepts.md](./02-core-concepts.md) |
| 📝 解析 Nginx 日志 | [03-practical-guide.md § 1](./03-practical-guide.md#1-解析-web-服务器日志) |
| 📊 解析 JSON 数据 | [03-practical-guide.md § 2](./03-practical-guide.md#2-解析-json-数据) |
| 🔑 解析 KV 键值对 | [03-practical-guide.md § 3](./03-practical-guide.md#3-解析-kv-键值对) |
| 🔐 处理 Base64 编码 | [03-practical-guide.md § 4](./03-practical-guide.md#4-处理编码数据) |
| ✅ 验证字段 | [03-practical-guide.md § 5](./03-practical-guide.md#5-字段验证与过滤) |
| 🔍 查某个类型 | [04-language-reference.md § 类型系统](./04-language-reference.md#📋-类型系统) |
| ⚙️ 查某个函数 | [05-functions-reference.md](./05-functions-reference.md) |
| 📖 查语法规则 | [06-grammar-reference.md](./06-grammar-reference.md) |

---

## 📖 文档列表

| 文档 | 内容 | 适合人群 |
|------|------|---------|
| [01-quickstart.md](./01-quickstart.md) | 5 分钟快速入门 + 3 个最常用场景 + 练习 | 所有人 |
| **[07-complete-types-example.md](./07-complete-types-example.md)** | **完整类型系统示例 - 23 种类型速查** | **所有人** |
| [02-core-concepts.md](./02-core-concepts.md) | 设计理念 + 类型系统 + 匹配语义 + 管道系统 | 想深入理解的用户 |
| [03-practical-guide.md](./03-practical-guide.md) | 按任务组织的实战示例 + 常见问题 | 日常使用者 |
| [04-language-reference.md](./04-language-reference.md) | 完整类型列表 + 语法元素 + 速查表 | 开发者 |
| [05-functions-reference.md](./05-functions-reference.md) | 所有函数的标准化参考 | 开发者 |
| [06-grammar-reference.md](./06-grammar-reference.md) | EBNF 形式化语法定义 | 编译器开发者 |

---

## ⚡ 快速示例

### Nginx 访问日志

```wpl
package nginx {
  rule access_log {
    (
      ip:client_ip,
      2*_,
      time/clf<[,]>:time,
      http/request":request,
      digit:status,
      digit:bytes
    )
  }
}
```

### JSON API 响应

```wpl
package api {
  rule response {
    (json(
      chars@user,
      digit@code,
      chars@message
    ))
  }
}
```

### 华为防火墙日志（Base64）

```wpl
package firewall {
  rule huawei_log {
    |decode/base64|
    (
      digit:id,
      time:timestamp,
      sn:serial,
      chars:type\:,
      opt(kvarr),
      kvarr
    )
  }
}
```

更多示例请查看：[01-quickstart.md](./01-quickstart.md) 和 [03-practical-guide.md](./03-practical-guide.md)

---

## 🎯 完整类型系统示例

**想快速了解 WPL 支持的所有数据类型？**

👉 **[查看完整类型示例](./07-complete-types-example.md)** - 一个示例展示 23 种主要数据类型

该文档包含：
- ✅ **完整可运行**的输入数据 + WPL 规则 + 输出结果
- ✅ **23 种类型**：基础、时间、网络、结构化、协议、编码
- ✅ **每种类型详解**：语法、示例、使用场景
- ✅ **常见组合模式**：复制即用的类型组合

**适合：**
- 🆕 新手快速了解 WPL 能力
- 📚 开发者作为类型速查手册
- 🔍 遇到陌生数据格式时快速查找对应类型

---

## 🎯 核心特性

- **声明式**：描述"是什么"，而非"怎么做"
- **类型安全**：自动验证和转换（IP、时间、JSON 等）
- **组合性**：小规则组合成复杂规则
- **强大的管道**：预处理（Base64/Hex 解码）+ 字段级验证
- **灵活的匹配**：顺序、择一、可选、重复
- **子字段提取**：JSON/KV 嵌套字段

---

## 💬 快速帮助

### 常见问题

**Q: 从哪里开始学习？**
A: 从 [01-quickstart.md](./01-quickstart.md) 开始，5 分钟即可上手。

**Q: 如何解析我的日志格式？**
A: 查看 [03-practical-guide.md](./03-practical-guide.md)，找到相似的场景并调整。

**Q: 某个类型/函数怎么用？**
A: 查看 [04-language-reference.md](./04-language-reference.md) 或 [05-functions-reference.md](./05-functions-reference.md)。

**Q: 解析失败怎么调试？**
A: 参考 [01-quickstart.md § 调试技巧](./01-quickstart.md#快速调试技巧) 或 [03-practical-guide.md § 常见问题](./03-practical-guide.md#7-常见问题)。

---

**开始学习：** [01-quickstart.md](./01-quickstart.md) - 5分钟快速入门