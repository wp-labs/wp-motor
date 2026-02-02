# WP-Motor 文档索引

本目录包含 WP-Motor 项目的技术文档和使用指南。

## 📚 文档结构

```
docs/
├── README.md                          # 本文件 - 文档索引
├── usage/                             # 用户使用指南
│   ├── wpl/                           # WPL 语言使用文档
│   │   ├── field_reference.md         # 字段引用使用指南
│   │   ├── separator.md               # 分隔符使用指南
│   │   └── chars_replace.md           # chars_replace 函数使用手册
│   └── oml/                           # OML 语言使用文档
│       ├── README.md                  # OML 使用总览
│       ├── pipe_functions.md          # 管道函数完整参考
│       ├── extract_main_word.md       # NLP 关键词提取
│       └── extract_subject_object.md  # 日志主客体分析
└── guide/                             # 开发者指南
    ├── wpl_field_func_development_guide.md    # WPL Field Function 开发指南
    └── oml_pipefun_development_guide.md       # OML PipeFun 开发指南
```

## 📖 使用指南 (usage/)

面向最终用户的功能使用文档。

### WPL 语言使用

- **[字段引用使用指南](usage/wpl/field_reference.md)** ⭐ 推荐
  - `@field_name` 和 `@'@special-field'` 语法
  - 单引号字段名支持（包含特殊字符）
  - 转义字符详解
  - 字段类型和别名
  - 实际应用场景（7+ 个场景）
  - 性能说明和最佳实践
  - FAQ 常见问题

- **[分隔符使用指南](usage/wpl/separator.md)**
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

### OML 语言使用

- **[OML 使用总览](usage/oml/README.md)** ⭐ 推荐
  - 快速开始和基本语法
  - 核心概念（访问器、管道函数、数据类型）
  - 常用示例和高级特性
  - 性能优化和调试技巧

- **[管道函数完整参考](usage/oml/pipe_functions.md)** ⭐ 重要
  - 19 个 PipeFun 详细文档
  - 编码转义（Base64、HTML、JSON）
  - 时间转换（时间戳、时区）
  - 数据提取（nth、get、to_json）
  - 网络解析（URL、路径、IP）
  - NLP 文本处理

- **[extract_main_word 使用指南](usage/oml/extract_main_word.md)**
  - NLP 关键词提取（中英文混合）
  - 词性标注和停用词过滤
  - 日志领域词优先识别
  - 链式处理和性能优化

- **[extract_subject_object 使用指南](usage/oml/extract_subject_object.md)**
  - 日志主客体结构分析
  - Subject-Action-Object-Status 识别
  - 词角色智能分类（英文词缀、中文词性）
  - Debug 模式和准确率测试

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

### OML 语言扩展

- **[OML PipeFun 开发指南](guide/oml_pipefun_development_guide.md)** ⭐ 重要
  - 完整的开发流程（6 步）
  - 3 种函数类型实现（无参数、单参数、双参数）
  - ValueProcessor trait 详解
  - DataField 类型系统
  - 最佳实践和调试技巧

**适用人群**: WP-Motor 核心开发者、OML/WPL 扩展开发者

## 🔍 快速导航

### 我想了解 WPL 字段引用的使用
→ [字段引用使用指南](usage/wpl/field_reference.md)

### 我想了解 WPL 分隔符的使用
→ [分隔符使用指南](usage/wpl/separator.md)

### 我想学习如何使用 chars_replace 函数
→ [chars_replace 使用指南](usage/wpl/chars_replace.md)

### 我想了解 OML 管道函数的使用
→ [OML 使用总览](usage/oml/README.md) | [管道函数参考](usage/oml/pipe_functions.md)

### 我想使用 NLP 功能分析日志
→ [extract_main_word](usage/oml/extract_main_word.md) | [extract_subject_object](usage/oml/extract_subject_object.md)

### 我想开发一个新的 WPL field function
→ [WPL Field Function 开发指南](guide/wpl_field_func_development_guide.md)

### 我想开发一个新的 OML pipe function
→ [OML PipeFun 开发指南](guide/oml_pipefun_development_guide.md)

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

- **2026-02-01**: 添加 OML 完整文档（使用总览 + 19 个 PipeFun 参考）
- **2026-02-01**: 添加 NLP 函数文档（extract_main_word + extract_subject_object）
- **2026-01-29**: 添加字段引用使用指南（@'@special-field' 单引号支持）
- **2026-01-29**: 添加分隔符使用指南（\s, \t, \S 支持）
- **2026-01-29**: 添加 chars_replace 完整文档（使用指南 + 开发指南）
- **2026-01-29**: 重组文档结构（usage/ 和 guide/ 分离）

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
**最后更新**: 2026-02-01
