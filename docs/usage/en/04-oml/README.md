# OML Object Model Language (New Version)

> **Note**: This is the new comprehensive OML documentation. English translation is in progress.
> 
> For now, please refer to the Chinese version at `docs/10-user/04-oml-new/`

OML (Object Model Language) is a declarative transformation language that converts parsed data from WPL into the desired output format.

## Documentation Structure

- **🌟 [Complete Feature Example](./07-complete-example.md)** - Comprehensive example showcasing all OML features
- **[Quick Start](./01-quickstart.md)** - Get started with OML in 5 minutes
- **[Core Concepts](./02-core-concepts.md)** - Understand OML's design philosophy
- **[Practical Guide](./03-practical-guide.md)** - Task-oriented solutions
- **[Functions Reference](./04-functions-reference.md)** - All function documentation
- **[Integration Guide](./05-integration.md)** - Integrate OML into data pipelines
- **[Grammar Reference](./06-grammar-reference.md)** - Formal grammar definition

## Key Features

- **Declarative**: Describe "what you want" rather than "how to implement"
- **Type System**: 8 data types with automatic inference and conversion
- **Powerful Functions**: Built-in functions, pipeline functions, pattern matching
- **Flexible Operations**: Field extraction, data aggregation, conditional processing
- **SQL Integration**: Enrich data with database queries

## Quick Example

**WPL Parsed Data:**
```
client_ip: 192.168.1.100
status: 200
timestamp: 2024-01-15 14:30:00
```

**OML Configuration:**
```oml
name : web_log_processor
rule : /nginx/access_log
---
# Extract fields
ip : ip = read(client_ip) ;
code : digit = read(status) ;

# Transform timestamp
ts_ms = read(timestamp) | Time::to_ts_zone(0, ms) ;

# Conditional processing
level = match read(code) {
    in (digit(200), digit(299)) => chars(success) ;
    in (digit(400), digit(499)) => chars(error) ;
    _ => chars(other) ;
} ;

# Create structured output
log : obj = object {
    client : ip = read(ip) ;
    status : digit = read(code) ;
    timestamp : digit = read(ts_ms) ;
    level : chars = read(level) ;
} ;
```

**Output:**
```json
{
    "log": {
        "client": "192.168.1.100",
        "status": 200,
        "timestamp": 1705318200000,
        "level": "success"
    }
}
```

## Translation Status

| Document | Status |
|----------|--------|
| README.md | ✅ Completed |
| 01-quickstart.md | 🔄 In Progress |
| 02-core-concepts.md | 🔄 In Progress |
| 03-practical-guide.md | 🔄 In Progress |
| 04-functions-reference.md | 🔄 In Progress |
| 05-integration.md | 🔄 In Progress |
| 06-grammar-reference.md | 🔄 In Progress |
| 07-complete-example.md | 🔄 In Progress |

## How OML Works with WPL

```
1. WPL parses raw data
   ↓
2. Generates structured data + rule identifier (e.g., /nginx/access_log)
   ↓
3. System finds matching OML configuration (via rule field)
   ↓
4. Executes OML transformation
   ↓
5. Outputs to Sink
```

## Related Documentation

- [Legacy OML Documentation](../04-oml/README.md) - Previous version
- [WPL Rule Language](../03-wpl-new/README.md) - Data parsing language
- [Sinks Configuration](../05-connectors/02-sinks/README.md) - Output configuration

---

**For the latest Chinese documentation, please visit:** `docs/10-user/04-oml-new/`
