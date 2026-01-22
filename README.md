# Warp Parse Engine

<div align="center">

[![CI](https://github.com/wp-labs/wp-engine/workflows/CI/badge.svg)](https://github.com/wp-labs/wp-engine/actions)
[![codecov](https://codecov.io/gh/wp-labs/wp-engine/graph/badge.svg?token=6SVCXBHB6B)](https://codecov.io/gh/wp-labs/wp-engine)
[![License](https://img.shields.io/badge/license-Elastic%20License%202.0-blue.svg)](LICENSE)

High-performance data parsing and processing engine built in Rust

</div>

## Overview

Warp Parse Engine (WP-Engine) is a high-performance, modular data parsing and processing engine designed for large-scale data stream processing. It provides the domain-specific language WPL (Warp Processing Language) for defining parsing rules and supports multiple data formats and protocols.

## Features

- **High Performance**: Built with Rust for optimal performance and memory safety
- **Domain-Specific Language**: WPL (Warp Processing Language) for flexible rule definitions
- **Multi-format Support**: JSON, CSV, Protobuf, Syslog, and custom formats
- **Real-time Processing**: Stream processing with sub-millisecond latency
- **Extensible Architecture**: Plugin system for custom processors and sinks
- **Enterprise Ready**: Built-in monitoring, metrics, and fault tolerance

## Architecture

```
wp-engine (root)
├── crates/                    # Core libraries
│   ├── orion_overload      # Common utilities and primitives
│   ├── orion_exp         # Expression evaluation
│   ├── wp-config         # Engine configuration management
│   ├── wp-data-utils    # Data structures and utilities
│   ├── wp-parser         # Low-level parsing primitives
│   ├── wp-lang           # WPL (Warp Processing Language)
│   ├── wp-oml            # Object Modeling Language
│   ├── wp-knowledge      # Knowledge database (KnowDB)
│   ├── wp-cli-core       # CLI shared infrastructure
│   ├── wp-cli-utils      # CLI utilities
│   ├── wp-proj           # Project management utilities
│   └── wp-stats          # Statistics collection
├── src/                      # Main application
│   ├── core/               # Core engine
│   ├── runtime/            # Runtime components
│   ├── sources/            # Data sources
│   ├── sinks/              # Data sinks
│   ├── facade/             Public API
│   └── orchestrator/       # Orchestration
└── tests/                    # Integration tests
```





## Feature Flags

- `default`: Community edition with core runtime
- `runtime-core`: Base runtime functionality
- `enterprise-backend`: Enterprise-only backend features
- `perf-ci`: Performance testing in CI
- `dev-tools`: Development utilities

## Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

## License

This project is licensed under the Elastic License 2.0 - see the [LICENSE](LICENSE) file for details.

## Support

- [Issues](https://github.com/wp-labs/wp-engine/issues)
- [Discussions](https://github.com/wp-labs/wp-engine/discussions)
- [Community Discord](https://discord.gg/wp-engine)

---

## Warp Parse Engine（Warp 解析引擎）

<div align="center">

[![Rust](https://img.shields.io/badge/rust-1.74+-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Elastic%20License%202.0-blue.svg)](LICENSE)
[![Build Status](https://img.shields.io/badge/build-passing-brightgreen.svg)](https://github.com/wp-labs/wp-engine)

用 Rust 构建的高性能数据解析和处理引擎

</div>

## 概述

Warp Parse Engine（WP-Engine）是一个高性能、模块化的数据解析和处理引擎，专为处理大规模数据流而设计，具有低延迟和高吞吐量的特点。它提供了领域特定语言（WPL）来定义解析规则，并支持多种数据格式和协议。

## 特性

- **高性能**：使用 Rust 构建，确保最佳性能和内存安全
- **领域特定语言**：WPL（Warp Processing Language）用于灵活的规则定义
- **多格式支持**：JSON、CSV、Protobuf、Syslog 和自定义格式
- **实时处理**：流处理，延迟低于毫秒级
- **可扩展架构**：插件系统支持自定义处理器和输出端
- **企业级就绪**：内置监控、指标和容错功能


## 架构

```
wp-engine (根目录)
├── crates/                    # 核心库
│   ├── orion_overload      # 通用工具和原语
│   ├── orion_exp         # 表达式求值
│   ├── wp-config         # 引擎配置管理
│   ├── wp-data-utils    # 数据结构和工具
│   ├── wp-parser         # 底层解析原语
│   ├── wp-lang           # WPL（Warp Processing Language）
│   ├── wp-oml            # 对象建模语言
│   ├── wp-knowledge      # 知识数据库 (KnowDB)
│   ├── wp-cli-core       # CLI 共享基础设施
│   ├── wp-cli-utils      # CLI 工具
│   ├── wp-proj           # 项目管理
│   └── wp-stats          # 统计收集
├── src/                      # 主应用
│   ├── core/               # 核心引擎
│   ├── runtime/            # 运行时组件
│   ├── sources/            # 数据源
│   ├── sinks/              # 数据汇
│   ├── facade/             # 公共 API
│   └── orchestrator/       # 编排
└── tests/                    # 集成测试
```

## 许可证

本项目采用 Elastic License 2.0 许可证 - 详情请参见 [LICENSE](LICENSE) 文件。

## 支持

- [问题反馈](https://github.com/wp-labs/wp-engine/issues)
- [讨论区](https://github.com/wp-labs/wp-engine/discussions)
- [社区 Discord](https://discord.gg/wp-engine)

---

**Warp Parse Dev Team**
