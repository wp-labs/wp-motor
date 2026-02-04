# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.10.0 latest ]

### Added
- **WPL Functions**: Add `starts_with` pipe function for efficient string prefix matching
  - Checks if a string field starts with a specified prefix
  - More performant than regex for simple prefix checks
  - Case-sensitive matching
  - Converts to ignore field when prefix doesn't match
- **OML Pipe Functions**: Add `starts_with` pipe function for OML query language
  - Supports same prefix matching functionality as WPL
  - Returns ignore field when prefix doesn't match
  - Usage: `pipe take(field) | starts_with('prefix')` or `take(field) | starts_with('prefix')`
- **OML Pipe Functions**: Add `map_to` pipe function for type-aware conditional value assignment
  - Replaces field value when field is not ignore
  - Supports multiple types with automatic type inference: string, integer, float, boolean
  - Preserves ignore fields unchanged
  - Usage examples:
    - `pipe take(field) | map_to('string')` - map to string
    - `pipe take(field) | map_to(123)` - map to integer
    - `pipe take(field) | map_to(3.14)` - map to float
    - `pipe take(field) | map_to(true)` - map to boolean
- **OML Match Expression**: Add function-based pattern matching support
  - Enables using functions like `starts_with` in match conditions
  - Syntax: `match read(field) { starts_with('prefix') => result, _ => default }`
  - More flexible than simple value comparison
  - Useful for log parsing, URL routing, and content classification
  - Supported functions:
    - **String matching**:
      - `starts_with(prefix)` - Check if string starts with prefix
      - `ends_with(suffix)` - Check if string ends with suffix
      - `contains(substring)` - Check if string contains substring
      - `regex_match(pattern)` - Match string against regex pattern
      - `is_empty()` - Check if string is empty (no arguments)
      - `iequals(value)` - Case-insensitive string comparison
    - **Numeric comparison**:
      - `gt(value)` - Check if numeric field > value
      - `lt(value)` - Check if numeric field < value
      - `eq(value)` - Check if numeric field equals value (with floating point tolerance)
      - `in_range(min, max)` - Check if numeric field is within range [min, max]
- **OML Parser**: Add quoted string support for `chars()` and other value constructors
  - Supports single quotes: `chars('hello world')`
  - Supports double quotes: `chars("hello world")`
  - Enables strings containing spaces and special characters
  - Escape sequence support: `\n`, `\r`, `\t`, `\\`, `\'`, `\"`
  - Backward compatible with unquoted syntax: `chars(hello)`
  - Works in all contexts: field assignments, match expressions, etc.
- **OML Transformer**: Add automatic temporary field filtering with performance optimization
  - Fields with names starting with `__` are automatically converted to ignore type after transformation
  - Parse-time detection: checks for temporary fields during OML parsing (one-time cost ~50-500ns)
  - Runtime optimization: skips filtering entirely when no temporary fields exist (~99% cost reduction)
  - Enables using intermediate/temporary fields in calculations without polluting final output
  - Example: `__temp = chars(value); result = pipe take(__temp) | base64_encode;`
  - The `__temp` field will be marked as ignore in the final output
  - Performance: ~1ns overhead for models without temp fields, ~500ns for models with temp fields

### Changed
- **OML Syntax**: `pipe` keyword is now optional in pipe expressions
  - Both `pipe take(field) | func` and `take(field) | func` are supported
  - Simplified syntax improves readability
  - Display output always includes `pipe` for consistency


## [1.13.3] - 2026-02-03

### Fixed
- **WPL Parser**: Fix compilation errors in pattern parser implementations by adding missing `event_id` parameter to all trait methods
- **Runtime**: Remove unused `debug_data` import in vm_unit module


## [1.13.2] - 2026-02-03

### Added
- **WPL Parser**: Add support for `\t` (tab) and `\S` (non-whitespace) separators in parsing expressions
- **WPL Parser**: Add support for quoted field names with special characters (e.g., `"field.name"`, `"field-name"`) #16
- **WPL Functions**: Add `chars_replace` function for character-level string replacement #13
- **WPL Functions**: Add `regex_match` function for regex pattern matching
- **WPL Functions**: Add `digit_range` function for numeric range validation
- **Documentation**: Add multi-language documentation structure for WPL guides

### Changed
- **Logging**: Optimize high-frequency log paths with `log_enabled!` guard to eliminate loop overhead when log level is filtered
- **Logging**: Add `event_id` to debug messages for better traceability
- **WPL Parser**: Add `event_id` parameter to `PatternParser` trait for improved event tracing across all parser implementations

### Fixed
- **Miss Sink**: Remove base64 encoding from raw data display to show actual content
- **Data Rescue**: Fix lost rescue data problem #19

### Removed
- **Syslog UDP Source**: Remove `SO_REUSEPORT` multi-instance support
  - Security risk: allows same-UID processes to intercept traffic
  - Cross-platform inconsistency: macOS/BSD doesn't provide kernel-level load balancing
  - See `docs/dar/udp_reuseport.md` for detailed design rationale


## [1.11.0] - 2026-01-28

### Added
- **Syslog UDP Source**: Added `udp_recv_buffer` configuration parameter to control UDP socket receive buffer size (default 8MB)
  - Helps prevent packet loss under high throughput conditions
  - Uses `socket2` crate for buffer configuration before socket binding
- **Syslog UDP Source**: Added batch receiving (up to 128 packets per `receive()` call) for better throughput
- **Syslog UDP Source**: Added `fast_strip` optimization (previously TCP-only)
  - Skip full syslog parsing when `header_mode = "skip"` and only stripping header
  - Fast path for RFC3164 (find `: `) and RFC5424 (skip fixed structure) formats
  - Reduces CPU overhead significantly at high EPS
- **Syslog UDP Source**: Added Linux `recvmmsg()` syscall support for batch receiving
  - Receive up to 64 datagrams in a single syscall on Linux
  - Reduces syscall overhead by ~60x compared to per-packet `recv_from()`
  - Automatically falls back to standard loop on non-Linux platforms
- **Syslog UDP Source**: Changed payload from `Bytes::copy_from_slice` to `Arc<[u8]>`
  - Zero-copy sharing downstream reduces memory allocation overhead
  - More consistent with TCP source's `ZcpMessage` pattern

### Changed
- **Syslog Architecture**: Major refactoring to eliminate duplicate parsing and unify UDP/TCP processing
  - Removed `SyslogDecoder` dependency from UDP source (now uses raw UDP socket)
  - UDP source passes raw bytes to `SourceEvent`, syslog processing happens in preprocessing hook
  - Unified preprocessing logic between UDP and TCP sources
  - `header_mode = "raw"` now correctly preserves full syslog message including header
  - Eliminated redundant `normalize_slice()` calls (was parsing twice: in decoder + preproc hook)
- **Syslog UDP Source**: Optimized preprocessing hook to be created once and reused via `Arc::clone()` instead of per-message allocation
- **Syslog header_mode**: Renamed configuration values for clarity with backward compatibility
  - `raw` (保留原样) - previously `keep`
  - `skip` (跳过头部) - previously `strip`
  - `tag` (提取标签) - previously `parse`
  - Legacy values (`keep`/`strip`/`parse`) remain supported as aliases
  - Default changed from `strip` to `skip`

### Removed
- **Syslog Protocol**: Removed `SyslogDecoder` and `SyslogFrame` from `protocol::syslog` module
  - No longer needed after UDP source refactoring
  - Syslog encoding (`SyslogEncoder`, `EmitMessage`) retained for sink usage
- **Benchmarks**: Replaced deprecated `criterion::black_box` with `std::hint::black_box` across all benchmark files
  - `crates/wp-stats/benches/wp_stats_bench.rs`
  - `crates/orion_exp/benches/or_we_bench.rs`
  - `crates/wp-oml/benches/oml_sql_bench*.rs`
  - `crates/wp-parser/benches/*.rs`
  - `crates/wp-lang/benches/nginx_10k.rs`
  - `crates/wp-knowledge/benches/read_bench.rs`
  - `src/sources/benches/normalize_bench.rs`
- **Documentation**: Updated Syslog source documentation with comprehensive configuration guide
  - Added UDP vs TCP protocol selection guide
  - Added performance tuning recommendations
  - Updated `wp-docs/10-user/02-config/02-sources.md`
  - Updated `wp-docs/10-user/05-connectors/01-sources/04-syslog_source.md`

### Fixed
- **Syslog RFC3164 Parser**: Implemented strict validation to prevent misidentification of non-standard formats
  - Added month name validation (Jan-Dec only)
  - Added strict timestamp format validation (HH:MM:SS with colons)
  - Added mandatory space validation after month, day, and time fields
  - Non-standard formats (e.g., ISO timestamps, invalid month names) now correctly fallback to passthrough
  - Examples that now correctly reject:
    - `<11>2025-07-07 09:42:43,132 sentinel - ...` (ISO format)
    - `<158>Jul23 17:18:36 skyeye ...` (missing space after month)
    - `<34>Xyz 11 22:14:15 host ...` (invalid month)
- **Clippy**: Fixed `bool_assert_comparison` warnings in syslog tests (`src/sources/syslog/mod.rs`)


## [1.10.4] - 2026-01-27

### Changed
- **Dependencies**: Updated `sysinfo` requirement from 0.37 to 0.38
- **License**: Changed license from Elastic License 2.0 to Apache 2.0
- **Support Links**: Updated support links to point to organization discussions

### Fixed
- **Monitoring**: Repaired monitoring statistics and examples for MetricCollectors


## [1.10.0] - 2026-01-22

### Added
- **KvArr Parser** (`crates/wp-lang/src/eval/value/parser/protocol/kvarr.rs`): New parser for key=value array format
  - Supports both `=` and `:` as key-value separators (e.g., `key=value` or `key:value`)
  - Flexible delimiter support: comma-separated, space-separated, or mixed
  - Automatic type inference for values (bool, integer, float, string)
  - Quoted and unquoted string values (e.g., `"value"` or `value`)
  - Duplicate key handling with automatic array indexing (e.g., `tag=alpha tag=beta` → `tag[0]`, `tag[1]`)
  - Subfield configuration support with type mapping and meta field ignoring (`_@name`)
  - Nested parser invocation through sub-parser context
  - WPL syntax: `kvarr(type@field1, type@field2, ...)`
- **Unicode-friendly string parsing**: Added `take_string` helper for general text arguments (e.g. 汉字) without changing the legacy `take_path` semantics (`crates/wp-parser/src/atom.rs`).
- **WPL Documentation Updates**:
  - Added `kvarr` to builtin types in grammar specification (`wp-docs/docs/10-user/03-wpl/04-wpl_grammar.md`)
  - New "KvArr 类型（键值对数组）" section in basics guide with syntax and examples (`wp-docs/docs/10-user/03-wpl/01-wpl_basics.md`)
  - New "2.1 KvArr 键值对数组解析" section in examples guide with 5 practical use cases (`wp-docs/docs/10-user/03-wpl/02-wpl_example.md`)

### Fixed
- **KvArr Parser**: Fixed meta fields being ignored in sub-parser context (`crates/wp-lang/src/eval/value/parser/protocol/kvarr.rs`)
- **Module Export**: Fixed missing `validate_groups` function export in `wp-cli-core::utils::validate` module (`crates/wp-cli-core/src/utils/validate/mod.rs`)
- **Single-quoted strings**: `single_quot_str_impl` now rejects raw `'` and accepts `\'` escapes, aligning behavior with double-quoted parser (`crates/wp-lang/src/parser/utils.rs`).
- **Chars* fun args**: `chars_has`/`chars_in` families switched to `take_string`, restoring `take_path` for identifiers while keeping Unicode support for free-form arguments (`crates/wp-lang/src/parser/wpl_fun.rs`).


## [1.9.0] - 2026-01-16

### Added
- `BlackHoleSink` now supports `sink_sleep_ms` parameter to control sleep delay per sink operation (0 = no sleep)
- `BlackHoleFactory` reads `sleep_ms` from `SinkSpec.params` to configure sleep behavior
- **Dynamic Speed Control Module** (`src/runtime/generator/speed/`): New module for variable data generation speed
  - `SpeedProfile` enum with multiple speed models:
    - `Constant` - Fixed rate generation
    - `Sinusoidal` - Sine wave oscillation (day/night cycles)
    - `Stepped` - Step-wise rate changes (business peak/off-peak)
    - `Burst` - Random burst spikes (traffic surges)
    - `Ramp` - Linear ramp up/down (load testing)
    - `RandomWalk` - Random fluctuations (natural jitter)
    - `Composite` - Combine multiple profiles (Average/Max/Min/Sum)
  - `DynamicSpeedController` - Calculates target rate based on elapsed time and profile
  - `DynamicRateLimiter` - Token bucket rate limiter with dynamic rate updates
- `GenGRA.speed_profile` field for configuring dynamic speed models in generators
- **wpgen.toml Configuration Support** (`crates/wp-config/src/generator/`):
  - `SpeedProfileConfig` - TOML-parseable configuration for speed profiles
  - `GeneratorConfig.speed_profile` - New optional field to configure dynamic speed in wpgen.toml
  - Helper methods: `base_speed()`, `get_speed_profile()`, `is_constant_speed()`
  - Backward compatible: Falls back to `speed` field when `speed_profile` is not set
- **Rescue Statistics Module** (`crates/wp-cli-core/src/rescue/`): New module for rescue data statistics
  - `RescueFileStat` - Single rescue file statistics (path, sink_name, size, line_count, modified_time)
  - `RescueStatSummary` - Aggregated statistics with per-sink breakdown
  - `SinkRescueStat` - Per-sink statistics (file_count, line_count, size_bytes)
  - `scan_rescue_stat()` - Scan rescue directory and generate statistics report
  - Multiple output formats: table, JSON, CSV
  - Supports nested directory scanning and `.dat` file filtering

### Changed
- **Rescue stat functionality migrated to wp-cli-core**: Rescue statistics is now a standalone CLI utility in `wp-cli-core::rescue` module, decoupled from wp-engine runtime

### Removed
- `WpRescueCLI` enum removed from wp-engine (rescue CLI should be defined in application layer)
- `RescueStatArgs` struct removed from wp-engine facade
- `run_rescue_stat()` function removed from wp-engine facade


## [1.8.2] - 2026-01-14

### Changed
- **Breaking**: Renamed `oml_parse` to `oml_parse_raw` for clarity (crates/wp-oml/src/parser/mod.rs)
- Removed deprecated pipe functions from OML language module

### Refactored
- **wp-oml**: Extracted nested functions from `oml_sql` to module level for improved readability (crates/wp-oml/src/parser/sql_prm.rs)
  - `is_sql_ident`, `sanitize_sql_body`, `rewrite_lhs_fn_eq_literal`, `to_sql_piece`, `fast_path_ip4_between_eq_one`
- **wp-oml**: Unified OML parser error contexts using shared helpers (`ctx_desc`, `ctx_literal`)
  - Affected files: keyword.rs, oml_aggregate.rs, oml_conf.rs, pipe_prm.rs, sql_prm.rs, utils.rs

### Fixed
- `wp_log::conf::LogConf` construction in wpgen configuration (crates/wp-config/src/generator/wpgen.rs)

## [1.8.1] - 2024-01-11

### Added
- **P0-3**: `ConfigLoader` trait to unify configuration loading interface (crates/wp-config/src/loader/traits.rs)
- **P0-4**: `ComponentBase` trait system to standardize component architecture across wp-proj
- **P0-5**: Unified API consistency with new `fs` utilities module in wp-proj
- **P0-2**: Error conversion helpers module (`error_conv`, `error_handler`) to simplify error handling
- **P0-1**: Centralized knowledge base operations in wp-cli-core to eliminate duplication
- Comprehensive documentation comments for ConfigLoader trait
- Path normalization for log directory display to remove redundant `./` components (crates/wp-proj/src/utils/log_handler.rs:48-76)
- Test case `normalize_path_removes_current_dir_components` to verify path normalization

### Changed
- **Breaking**: EnvDict parameter now required in all configuration loading functions
  - `validate_routes(work_root: &str, env_dict: &EnvDict)` (wp-cli-core/src/business/connectors/sinks.rs:18)
  - `collect_sink_statistics(sink_root: &Path, ctx: &Ctx, dict: &EnvDict)` (wp-cli-core/src/business/observability/sinks.rs:21)
  - `load_warp_engine_confs(work_root: &str, dict: &EnvDict)` (src/orchestrator/config/models/warp_helpers.rs:17)
  - And 13 more functions across wp-proj and wp-cli-core
- **Architecture**: Enforced top-level EnvDict initialization pattern
  - EnvDict must be created at application entry point (e.g., `load_sec_dict()` in warp-parse)
  - Crate-level functions only accept `dict: &EnvDict` parameter, never create instances
  - This follows dependency injection pattern for better testability and clarity
- Source and sink factories now return multiple connector definitions instead of single instance
- Improved table formatting in CLI output for better readability

### Fixed
- Default sink path resolution now works correctly
- Engine configuration path normalization to handle `.` and `..` components properly
- Empty stat fields are now skipped during serialization
- Project initialization bug resolved
- Documentation test closure parameter issues in error_conv module
- Log directory paths now display correctly without `././` in output messages (crates/wp-proj/src/utils/log_handler.rs:96,102)
- Clippy warning `field_reassign_with_default` in wpgen configuration (crates/wp-config/src/generator/wpgen.rs:125)

### Refactored
- **wp-proj Stage 1**: Extracted common patterns to reduce code duplication
- **wp-proj Stage 2**: Implemented Component trait system for models, I/O, and connectors
- **wp-proj Stage 3**: Documented standard error handling patterns
- **wp-proj Stage 4**: Merged `check` and `checker` modules to eliminate responsibility overlap
- Knowledge base operations delegated from wp-proj to wp-cli-core

### Removed
- `EnvDictExt` trait removed from wp-config as it violated architectural separation
  - App layer (warp-parse, wpgen) is responsible for EnvDict creation
  - Crate layer (wp-engine, wp-proj, wp-config) only receives and uses EnvDict
- Documentation files: `envdict-ext-usage.md`, `envdict-ext-quickref.md`

## [1.8.0] - 2024-01-05

### Added
- Environment variable templating support via `orion-variate` integration
- `EnvDict` type for managing environment variables during configuration loading
- Environment variable substitution in configuration files using `${VAR}` syntax
- Three-level variable resolution: dict → system env → default value
- Tests for environment variable substitution in config loading
- Path resolution for relative configuration paths

### Changed
- Updated `orion_conf` dependency to version 0.4
- Updated `wp-infras` dependencies to track main branch
- License changed from MIT to SLv2 (Server License v2)
- Work root resolution now uses `Option<String>` for better API clarity
- Configuration loading functions now accept `EnvDict` parameter
- Replaced direct `toml::from_str` calls with `EnvTomlLoad::env_parse_toml`

### Fixed
- Work root validation issue (#56) - invalid work-root paths now properly handled
- Partial parsing handling improved with residue tracking and error logging

### Removed
- `Cargo.lock` removed from version control
- Unnecessary `provided_root` parameter removed from path resolution functions

## Version Comparison Links
