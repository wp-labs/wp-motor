# OML Functions Reference

> **Translation in Progress**
> 
> This document is currently being translated from Chinese to English.
> 
> Please refer to the Chinese version: `docs/10-user/04-oml-new/04-functions-reference.md`

---

This document provides complete reference for all built-in functions and pipeline functions, using a standardized format for easy lookup.

## OML All Functions Quick Reference

### Built-in Functions

| Function | Description | Example |
|----------|-------------|---------|
| `Now::time()` | Get current time | `event_time = Now::time() ;` |
| `Now::date()` | Get current date (YYYYMMDD) | `today = Now::date() ;` |
| `Now::hour()` | Get current hour (YYYYMMDDHH) | `current_hour = Now::hour() ;` |

### Pipeline Functions

| Category | Function | Description | Example |
|----------|----------|-------------|---------|
| **Encoding** | `base64_encode` | Base64 encode | `read(data) \| base64_encode` |
| | `base64_decode` | Base64 decode (supports Utf8/Gbk) | `read(data) \| base64_decode(Utf8)` |
| **Escaping** | `html_escape` | HTML escape | `read(text) \| html_escape` |
| | `json_escape` | JSON escape | `read(text) \| json_escape` |
| **Time** | `Time::to_ts` | Convert to timestamp (seconds, UTC+8) | `read(time) \| Time::to_ts` |
| | `Time::to_ts_zone` | Convert to specified timezone timestamp | `read(time) \| Time::to_ts_zone(0, ms)` |
| **Data Access** | `nth(index)` | Get array element | `read(arr) \| nth(0)` |
| | `get(key)` | Get object field | `read(obj) \| get(name)` |
| | `url(part)` | Extract URL parts | `read(url) \| url(domain)` |
| **Conversion** | `to_str` | Convert to string | `read(ip) \| to_str` |
| | `to_json` | Convert to JSON | `read(arr) \| to_json` |
| | `ip4_to_int` | IPv4 to integer | `read(ip) \| ip4_to_int` |
| **Control** | `skip_empty` | Skip empty values | `read(field) \| skip_empty` |

---

**For the complete English documentation, please check back later or refer to the Chinese version.**
