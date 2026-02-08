# `static` 块：模型级常量与模板缓存

OML 允许在 `---` 分隔线之后声明 `static { ... }` 区块，将“只初始化一次、在运行时复用”的结构写在这里。解析完成后，`static` 中的表达式会被执行一次，结果进入模型常量池；随后的所有数据转换都直接引用缓存对象，无需再生成临时字段。

## 适用场景

- 事件模板、常量字典等纯字面量对象
- 需要在多个转换步骤中复用的结构（例如 `match` 结果）

## 语法示例

```oml
name : /oml/apache_error_e1
rule : apache/error/e1
---
static {
    e1_template = object {
        id = chars(E1);
        tpl = chars("jk2_init() Found child <*> in scoreboard slot <*>")
    };
    e2_template = object {
        id = chars(E2);
        tpl = chars("workerEnv.init() ok <*>")
    };
}

Content = read(Content);

target_template = match Content {
    starts_with("jk2_init() Found child") => e1_template;
    starts_with("workerEnv.init() ok") => e2_template;
    _ => e1_template;
};

EventId = target_template | get(id);
EventTemplate = target_template | get(tpl);
```

- `static { ... }` 中的赋值可使用任意合法表达式，但不得调用 `read()`/`take()` 等依赖输入数据的函数。
- 非 `static` 区块中直接写静态符号名即可引用缓存值，无需 `read()`。

## 执行模型

1. **解析阶段**：
   - 仅检查 `static` 语句和目标名称是否重复。
   - 生成 `EvalExp` AST 并登记符号名。
2. **构建阶段**：
   - `finalize_static_blocks` 统一执行所有静态表达式，构建常量池 `const_fields`。
   - 将 DSL 中的 `StaticSymbol` 占位符（如 match 结果、管道参数）重写为真实 `DataField`。
3. **运行阶段**：
   - 静态值来自常量池，不会再次执行 evaluator。

## 使用建议

- **匹配/管道**：`static` 变量可出现在 `match ... => symbol`、`read(symbol)`、管道起点等位置，解析器会自动识别。

## 限制

- `static` 语句仅支持单目标赋值，不可批量定义多个字段。
- 不允许在 `static` 中调用依赖输入记录的数据访问函数（`read()`/`take()` 等），否则编译期会报错。
- 静态符号仅在定义所在模型内可见，不会跨模型共享。
