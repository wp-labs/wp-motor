# KnowDB Configuration

This guide describes the directory-based configuration and loading specification for the Knowledge Database (KnowDB).

Applicable Scope
- Initialize authoritative database (CSV -> SQLite), used for loading by wparse/wproj and other tools at startup

Core Principles
- External SQL: DDL/DML for each table is placed in .sql files in the corresponding directory
- Security: Runtime only allows access to table names declared in configuration; SQL only supports {table} placeholder
- Ready to use by default: Most fields can be omitted; built-in defaults and auto-detection satisfy common scenarios

Directory Layout (Recommended)
```
models/knowledge/
  knowdb.toml                     # This configuration
  example/
    create.sql
    insert.sql
    data.csv                     # Single data file (table directory root)
  address/
    create.sql
    insert.sql
    data.csv
```

Top-level Configuration (models/knowledge/knowdb.toml)
```toml
version = 2

[[tables]]
name = "example"
# dir defaults to name when omitted; this example uses directory models/knowledge/example
# data_file defaults to data.csv in table directory when omitted
columns.by_header = ["name", "pinying"]

# To add more tables, append [[tables]] sections
```

SQL File Specifications
- create.sql: Table creation statement, must exist; can use placeholder `{table}`; allows multiple statements (e.g., `CREATE INDEX`)
- insert.sql: Insert statement, must exist; parameter positions use `?1..?N`; allows `{table}`
- clean.sql: Optional; if not present, `DELETE FROM {table}` is executed by default before loading

Column Mapping (columns)
- Recommended `by_header=[..]`, maps CSV header names to columns in `insert.sql`
- If `has_header=false`, must provide `by_index=[..]`
- Optional enhancement (implementation layer): If columns not configured and `insert.sql` explicitly lists columns, can parse insert column names as `by_header`

Loading Strategy (defaults can be omitted)
- Defaults: `transaction=true`, `batch_size=2000`, `on_error="fail"`
- on_error:
  - fail: Fails and rolls back on bad row (missing columns/parse failure)
  - skip: Skips bad rows and counts warnings

Auto-detection (when data_file is not configured)
- Uses `{base_dir}/{tables.dir}/data.csv`
- Reports error if not exists

Security Constraints
- Runtime (facade/query_cipher/SQL evaluation) only allows table names declared in `[[tables]].name`
- SQL templates only allow `{table}` placeholder; other dynamic concatenation is prohibited

Minimal Runnable Example
1) Directory
```
models/knowledge/knowdb.toml
models/knowledge/example/{create.sql, insert.sql, data.csv}
```
2) create.sql
```sql
CREATE TABLE IF NOT EXISTS {table} (
  id      INTEGER PRIMARY KEY,
  name    TEXT NOT NULL,
  pinying TEXT NOT NULL
);
```
3) insert.sql
```sql
INSERT INTO {table} (name, pinying) VALUES (?1, ?2);
```
4) data.csv
```
name,pinying
令狐冲,linghuchong
任盈盈,renyingying
```
5) knowdb.toml (minimal)
```toml
version = 2
base_dir = "./models/knowledge"
[[tables]]
name = "example"
dir  = "example"
columns.by_header = ["name", "pinying"]
[tables.expected_rows]
min = 1
max = 100
```

Common Errors and Troubleshooting
- Missing create.sql / insert.sql: Fails at startup and points to missing file
- `has_header=false` but `by_index` not provided: Loading error
- `expected_rows.min` not satisfied: Insufficient data, loading fails
- Data source not found: Neither `data_file` configured nor default path `data.csv` exists
- Runtime SQL accesses undeclared table: Security validation fails

Relationship with Applications
- wparse/wproj etc. load knowdb at startup: Create authoritative database and set up Query Provider
- `query_cipher(table)` (loading single-column word list) formerly used by privacy module is disabled by default in current version; implement desensitization on business side if needed

Built-in SQL Functions (UDF)
- Runtime registration:
  - Both import phase (authoritative database write connection) and query phase (thread-cloned read-only connection) automatically register.
  - Can be used directly in `INSERT/SELECT/WHERE` (not involved in DDL).
- Signatures and Semantics:
  - `ip4_int(text) -> integer`: Dotted IPv4 to 32-bit integer; tolerates whitespace/quotes; returns `0` on invalid input.
  - `ip4_between(ip_text, start_text, end_text) -> integer`: Whether in closed interval `[start,end]` (1/0).
  - `cidr4_min(text) -> integer`: CIDR start address (inclusive), e.g., `10.0.0.0/8`.
  - `cidr4_max(text) -> integer`: CIDR end address (inclusive).
  - `cidr4_contains(ip_text, cidr_text) -> integer`: Whether IP falls within CIDR range (1/0).
  - `ip4_text(integer|string) -> text`: 32-bit integer to dotted IPv4 (useful for debugging/display).
  - `trim_quotes(text) -> text`: Removes paired quotes (' or ") at both ends, tolerates surrounding whitespace; returns original (whitespace trimmed) if not paired.
- Import Example (insert.sql):
  ```sql
  INSERT INTO {table} (ip_start_int, ip_end_int, zone)
  VALUES (ip4_int(?1), ip4_int(?2), trim_quotes(?3));
  ```
- Query Example (regular SQL):
  ```sql
  -- Range match (recommended integer comparison, avoid directly comparing function returns in WHERE)
  SELECT zone FROM zone
  WHERE ip_start_int <= ip4_int(:ip)
    AND ip_end_int   >= ip4_int(:ip)
  LIMIT 1;

  -- CIDR match
  SELECT zone FROM zone
  WHERE cidr4_contains(:ip, :cidr) = 1;

  -- Debug output
  SELECT ip4_text(ip_start_int) AS ip_start, ip4_text(ip_end_int) AS ip_end, zone
  FROM zone
  LIMIT 5;
  ```
- OML SQL Exact Evaluation:
  - OML's `select ... from ... where ...;` syntax has identifier whitelist restrictions on column segments, not recommended to write functions directly in column segments.
  - Recommended to produce numeric IP upstream (e.g., `src_ip_int`) and use integer comparison in OML where:
    ```sql
    from_zone: chars = sql(
      select zone from zone
      where ip_start_int <= read(src_ip_int)
        and ip_end_int   >= read(src_ip_int);
    )
    ```
- Notes:
  - Currently invalid IPv4/CIDR input returns `0` (or match failure) for import resilience; customize if strict behavior needed.
  - SQLite natively provides `lower/upper/trim` and other string functions, can be combined with above UDFs.
