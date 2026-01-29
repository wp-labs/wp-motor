# WP-Motor 文档索引

本目录包含 WP-Motor 项目的技术文档和使用指南。

## 📚 文档结构

```
docs/
├── README.md                          # 本文件 - 文档索引
├── usage/                             # 用户使用指南
│   └── wpl/                           # WPL 语言使用文档
│       ├── chars_replace.md           # chars_replace 函数使用手册
│       └── separator.md               # 分隔符使用指南
└── guide/                             # 开发者指南
    └── wpl_field_func_development_guide.md    # WPL Field Function 开发指南
```

## 📖 使用指南 (usage/)

面向最终用户的功能使用文档。

### WPL 语言使用

- **[分隔符使用指南](usage/wpl/separator.md)** ⭐ 推荐
  - 6 种内置分隔符（\s, \t, \S, \0 等）
  - 自定义字符和字符串分隔符
  - 实际应用场景（7+ 个场景）
  - 分隔符优先级和行为详解
  - 性能说明和最佳实践
  - FAQ 常见问题

- **[chars_replace 使用指南](usage/wpl/chars_replace.md)**
  - 基本语法和参数格式
  - 实际应用场景（7+ 个场景）
  - 使用限制和错误处理
  - 最佳实践和调试技巧
  - FAQ 常见问题

**适用人群**: 所有 WP-Motor 用户、日志工程师

## 🛠 开发指南 (guide/)

面向开发者的技术实现文档。

### WPL 语言扩展

- **[WPL Field Function 开发指南](guide/wpl_field_func_development_guide.md)** ⭐ 重要
  - 完整的开发流程（7 步）
  - 4 种函数类型的实现模式
  - 高级主题和性能优化
  - 常见错误解决方案
  - 开发检查清单

**适用人群**: WP-Motor 核心开发者、WPL 扩展开发者

## 🔍 快速导航

### 我想了解 WPL 分隔符的使用
→ [分隔符使用指南](usage/wpl/separator.md)

### 我想学习如何使用 chars_replace 函数
→ [chars_replace 使用指南](usage/wpl/chars_replace.md)

### 我想开发一个新的 WPL field function
→ [WPL Field Function 开发指南](guide/wpl_field_func_development_guide.md)

## 📝 文档规范

### 用户使用指南 (usage/)

**目标读者**: 最终用户
**内容风格**: 实用、易懂、示例丰富
**必须包含**:
- 快速开始
- 实际应用场景
- 使用限制
- 常见问题 FAQ

**命名规范**: `<function_name>.md` 或 `<feature_name>_usage.md`

### 开发者指南 (guide/)

**目标读者**: 开发者、贡献者
**内容风格**: 技术、详细、包含代码实现
**必须包含**:
- 实现原理
- 代码示例
- 测试用例
- 性能分析

**命名规范**: `<topic>_development_guide.md` 或 `<feature>_implementation.md`

## 🔄 文档更新流程

1. **新功能开发**
   - 在 `guide/` 创建开发指南
   - 在 `usage/` 创建使用手册
   - 更新本 README.md 索引

2. **功能更新**
   - 更新对应的文档
   - 在文档底部添加版本历史

3. **架构变更**
   - 在根目录创建设计决策文档
   - 添加到本索引

## 📅 最近更新

- **2026-01-29**: 添加分隔符使用指南（\s, \t, \S 支持）
- **2026-01-29**: 添加 chars_replace 完整文档（使用指南 + 开发指南）
- **2026-01-29**: 重组文档结构（usage/ 和 guide/ 分离）
- **2026-01-29**: 简化文档，保留核心内容

## 🤝 贡献指南

如果您想为文档做出贡献：

1. 遵循上述文档规范
2. 使用 Markdown 格式
3. 包含足够的代码示例
4. 更新本索引文件
5. 提交 Pull Request

## 📞 获取帮助

- **GitHub Issues**: https://github.com/wp-labs/wp-motor/issues
- **文档问题**: 在 issue 中标记 `documentation` 标签

---

**维护者**: WP-Motor Team
**最后更新**: 2026-01-29
