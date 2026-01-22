# 表达式求值系统

## 概述

本模块提供了一个灵活的表达式求值系统，支持逻辑运算和比较运算，并允许自定义符号表示。

## 核心功能

1. **表达式类型**：
   - 逻辑表达式（AND、OR、NOT）
   - 比较表达式（==、!=、>、>=、<、<=、=*通配符匹配）

2. **主要特点**：
   - 类型安全的表达式构建
   - 可定制的符号提供器（支持Rust风格或SQL风格语法）
   - 针对数据上下文进行求值
   - 显示格式化功能

## 核心组件

   ### 表达式类型

   ```rust
   pub enum Expression<T, S> {
     Logic(LogicalExpress<T, S>),    // 逻辑运算
     Compare(Comparison<T, S>)       // 比较运算
   }
   ```

   ### 构建器

   `LogicalBuilder`提供了流畅的接口来构建表达式：

   ```rust
   // 创建AND表达式
   let expr = LogicalBuilder::and(left_expr, right_expr).build();

   // 创建OR表达式
   let expr = LogicalBuilder::or(left_expr, right_expr).build();

   // 创建NOT表达式
   let expr = LogicalBuilder::not(inner_expr).build();
   ```

   ### 符号提供器

   系统内置两种符号提供器：

   1. **RustSymbol** - 使用Rust风格运算符（`&&`、`||`、`!`、`==`等）
   2. **SQLSymbol** - 使用SQL风格关键字（`AND`、`OR`、`NOT`、`=`等）

   可以通过实现`LogicSymbolProvider`和`CmpSymbolProvider`特性来创建自定义提供器。

   ## 使用示例

   ### 创建表达式

   ```rust
   use orion_exp::*;

   // 创建比较表达式
   let age_check = Comparison::new(CmpOperator::Ge, "age", 18);
   let name_check = Comparison::new(CmpOperator::We, "name", "*admin*");

   // 使用逻辑运算符组合
   let expr = LogicalBuilder::and(
     Expression::Compare(age_check),
     Expression::Compare(name_check)
   ).build();
   ```

   ### 表达式求值

   ```rust
   let data = HashMap::from([
     ("age", 25),
     ("name", "admin_user")
   ]);

   assert!(expr.evaluate(&data));
   ```

   ### 表达式格式化

   ```rust
   println!("{}", expr);
   // 使用RustSymbol: "$age >= 18 && $name =* *admin*"
   // 使用SQLSymbol: "age >= 18 and name = *admin*"
   ```

   ## 支持的数据类型

   系统支持多种基础类型的求值：

   - 数值类型（`i64`、`u32`、`u64`、`u128`、`f64`）
   - 布尔值
   - 字符串（支持通配符匹配）
   - IP地址和网络
   - 日期时间值

   ## 实现说明

   1. 求值器避免了递归特性实现，防止无限编译
   2. 通配符匹配使用`*`作为通配字符
   3. 浮点数比较使用小量epsilon（0.0001）进行相等性检查

   ## 扩展系统

   ### 添加新类型支持

   1. 为类型实现`WildcardMatcher`
   2. 该类型将自动支持所有比较运算符

   ### 创建自定义语法

   1. 实现`LogicSymbolProvider`和`CmpSymbolProvider`
   2. 在创建表达式时使用你的实现

   ## 最佳实践

   1. 对于简单比较，直接使用`Comparison::new`
   2. 对于复杂逻辑，使用
