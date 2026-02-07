## OML `static` 块与常量模板设计评审

### 背景

- `loghub-benchmark` 的 Apache OML 模型（`benchmark/models/oml/apache/apache_e1.oml`）包含多个以 `__E*` 命名的临时字段，这些字段在模型生命周期内保持不变。
- 当前实现会在每条记录的转换过程中重新构造这些对象，再由临时字段清理逻辑过滤掉，带来不必要的 CPU 消耗。
- 目标：提供一套 DSL 语义，让此类“初始化一次、运行时复用”的数据结构在解析阶段就完成构建，并在运行期直接复用，彻底摆脱临时变量机制。

### 决策概述

- 为 OML 引入 `static { ... }` 代码块，块内语句在模型加载后立即执行且仅执行一次。
- `static` 作用域内声明的符号默认不可变，构造完成后写入 `ObjModel` 的常量池，在后续数据转换过程中通过名称直接引用。
- 运行期 DSL 不再需要 `__temp` 风格的字段：直接引用 `static` 中的符号，无需 `read()`。

### 语法提案

1. **静态块**
   ```oml
   ---
   static {
       symbol = expression;
       another = object { ... };
   }
   ```
   - 允许出现在 `---` 之后，普通 per-record 赋值语句之前。
   - 块内禁止调用依赖输入记录或非纯函数（如 `read(...)`, `now()`, `rand()` 等）。
   - 允许引用同块更早定义的静态符号，解析阶段需校验无循环依赖。

2. **运行期引用**
   - `static` 中声明的符号在后续 DSL 中可直接使用，例如 `target = match ... => symbol;`。
   - 禁止再次赋值或覆盖静态符号，违者在解析阶段报错。

3. **执行语义**
   - 解析完成后，构建 `ObjModel` 时遍历 `static` 块表达式并立即求值，结果存入 `const_fields: HashMap<String, Arc<DataField>>`。
   - 若求值失败（语法或运行错误），构建流程中止并返回错误，确保模型加载过程即可发现问题。
   - 数据转换阶段，若访问的字段存在于 `const_fields`，直接克隆缓存值，跳过 evaluator 与临时字段过滤逻辑。

### 新版 DSL 示例（来自 `apache_e1.oml`）

```oml
name : /oml/apache_error_e1
rule : apache/error/e1_jk2_found_child
---
static {
    e1_template = object {
        id = "E1";
        tpl = "jk2_init() Found child <*> in scoreboard slot <*>"
    };
    e2_template = object {
        id = "E2";
        tpl = "workerEnv.init() ok <*>"
    };
    e3_template = object {
        id = "E3";
        tpl = "mod_jk child workerEnv in error state <*>"
    };
    e4_template = object {
        id = "E4";
        tpl = "[client <*>] Directory index forbidden by rule: <*>"
    };
    e5_template = object {
        id = "E5";
        tpl = "jk2_init() Can't find child <*> in scoreboard"
    };
    e6_template = object {
        id = "E6";
        tpl = "mod_jk child init <*> <*>"
    };
}

Time = read(Time);
Level = read(Level);
Content = read(Content);

target_template = match read(Content) {
    starts_with("jk2_init() Found child") => e1_template;
    starts_with("workerEnv.init() ok") => e2_template;
    starts_with("mod_jk child workerEnv in error state") => e3_template;
    contains("Directory index forbidden by rule:") => e4_template;
    starts_with("jk2_init() Can't find child") => e5_template;
    starts_with("mod_jk child init") => e6_template;
};

EventId = target_template | get(id);
EventTemplate = target_template | get(tpl);
```

- `static` 中的模板在解析阶段构建一次，运行期 `target_template` 直接引用对应对象。
- `read()` 继续保留原语义：仅用于输入记录字段；静态引用无需 `read()`。

### 实现要点

1. **解析器**
   - 扩展语法以识别 `static` 块，生成独立的 AST 列表 `StaticItem`。
   - 在解析阶段检查静态块中的表达式是否引用非法函数或未定义符号。

2. **模型构建**
   - `ObjModel` 新增 `static_items` 与 `const_fields`：
     ```rust
     pub struct ObjModel {
         ...
         static_items: Vec<EvalExp>,
         const_fields: HashMap<String, Arc<DataField>>,
     }
     ```
   - 构建流程：先执行 `static_items`，填充 `const_fields`，再保留原有 `items`。

3. **运行期执行**
   - `transform_ref` 中，若赋值目标在 `const_fields`，直接 clone 缓存值并跳过 evaluator。
   - 临时字段过滤逻辑保持，但在新 DSL 下变为兜底路径。

4. **错误处理 & 热更新**
   - 如果静态初始化出现错误，阻止模型加载，避免运行期部分可用。
   - 模型热更新时刷新 `const_fields`，确保新版本静态值生效。

### 进一步可选优化

- **命名空间**：允许写成 `static templates { ... }`，帮助区分不同类别的静态资源。
- **多块/复用**：支持多个 `static` 块或 `include static "templates.oml"`，方便共享模板库。
- **语法糖**：提供 `template E1 "..."` 之类的宏，在编译期展开为标准 `object`。
- **工具支持**：在 `wparse lint` 中增加对静态符号的补全、未使用检查与非法引用提示。

### 结论

引入 `static` 块可以在 DSL 层面彻底区分“初始化一次的常量资源”与“按记录转换的逻辑”，既消除重复构造与临时字段开销，又让语义更直观。建议优先落地 `static` 块 + 常量池执行路径，后续再视需求引入命名空间、模板宏等增强特性。
