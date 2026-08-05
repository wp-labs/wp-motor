# OML Functions Reference

This document lists only the built-in functions, built-in expressions, and pipe functions that are actually supported by the current parser implementation in `wp-motor/crates/wp-oml/src/parser`.

> Notes:
> 1. `match` condition functions are documented separately in [functions/match_functions.md](./functions/match_functions.md)
> 2. Older docs mentioned `sxf_get(...)`, but it is not supported by the current parser
> 3. Pipe expressions can be written either as `pipe read(x) | ...` or as `read(x) | ...`

---

## Quick Reference

### Built-in Expressions / Built-in Functions

| Name | Description | Example |
|------|-------------|---------|
| `calc(...)` | Explicit numeric expressions | `score = calc(read(cpu) * 0.7 + read(mem) * 0.3) ;` |
| `lookup_nocase(...)` | Case-insensitive lookup against a static object | `score = lookup_nocase(score_map, read(status), 40.0) ;` |
| `Now::time()` | Current time | `event_time = Now::time() ;` |
| `Now::date()` | Current date as `YYYYMMDD` | `today = Now::date() ;` |
| `Now::hour()` | Current hour as `YYYYMMDDHH` | `hour = Now::hour() ;` |
| `access_direct(src, dst)` | Access direction (`内到内`/`内到外`/`外到内`/`外到外`) | `dir = access_direct(read(sip), read(dip)) | on_fail('unknown') ;` |

### Pipe Functions

| Category | Name | Example |
|----------|------|---------|
| Access | `nth(index)` | `first = read(items) | nth(0) ;` |
| Access | `get(name)` | `code = read(obj) | get(code) ;` |
| Condition | `starts_with('prefix')` | `secure = read(url) | starts_with('https://') ;` |
| Mapping | `map_to(value)` | `flag = read(level) | starts_with('ERROR') | map_to(true) ;` |
| Encoding | `base64_encode` | `encoded = read(msg) | base64_encode ;` |
| Encoding | `base64_decode([EncodeType])` | `decoded = read(msg) | base64_decode(Utf8) ;` |
| Escaping | `html_escape` | `safe = read(html) | html_escape ;` |
| Escaping | `html_unescape` | `raw = read(html) | html_unescape ;` |
| Escaping | `str_escape` | `escaped = read(message) | str_escape ;` |
| Escaping | `json_escape` | `escaped = read(text) | json_escape ;` |
| Escaping | `json_unescape` | `raw = read(text) | json_unescape ;` |
| Time | `Time::to_ts` | `ts = read(time) | Time::to_ts ;` |
| Time | `Time::to_ts_ms` | `ts = read(time) | Time::to_ts_ms ;` |
| Time | `Time::to_ts_us` | `ts = read(time) | Time::to_ts_us ;` |
| Time | `Time::to_ts_zone(zone, unit)` | `ts = read(time) | Time::to_ts_zone(0, ms) ;` |
| Conversion | `to_json` | `payload = read(arr) | to_json ;` |
| Conversion | `to_str` | `ip_s = read(ip) | to_str ;` |
| Conversion | `ip4_to_int` | `ip_i = read(src_ip) | ip4_to_int ;` |
| Network | `intranet_ip` | `side = read(src_ip) | intranet_ip ;` |
| Control | `on_fail('value')` | `side = read(src_ip) | intranet_ip | on_fail('unknown') ;` |
| Extraction | `path([name|path])` | `file = read(path) | path(name) ;` |
| Extraction | `url([domain|host|uri|path|params])` | `host = read(url) | url(host) ;` |
| Control | `skip_empty` | `v = read(maybe_empty) | skip_empty ;` |
| Text | `extract_main_word` | `kw = read(msg) | extract_main_word ;` |
| Text | `extract_subject_object` | `info = read(msg) | extract_subject_object ;` |

---

## Built-in Expressions / Built-in Functions

### `calc(...)`

Executes explicit numeric calculations.

**Syntax**

```oml
calc(<expr>)
```

**Supported by the current implementation**

- operators: `+ - * / %`
- unary negation: `-x`
- parentheses
- math functions: `abs(...)`, `round(...)`, `floor(...)`, `ceil(...)`
- operands: numeric literals, `read(...)`, `take(...)`, `@field`

**Type rules**

- `+ - *`: result is `float` if either operand is floating-point, otherwise `digit`
- `/`: result is always `float`
- `%`: only accepts integer operands and returns `digit`

**Failure behavior**

The result becomes `ignore` in these cases:

- division by zero
- missing fields
- non-numeric input
- integer overflow
- `NaN` / `inf`
- `%` used with floating-point values

**Example**

```oml
risk_score : float = calc(read(cpu) * 0.7 + read(mem) * 0.3) ;
delta : digit = calc(read(cur) - read(prev)) ;
bucket : digit = calc(read(uid) % 16) ;
error_pct : digit = calc(round((read(err_cnt) * 100) / read(total_cnt))) ;
```

For more details, see [functions/calc.md](./functions/calc.md).

---

### `lookup_nocase(...)`

Performs case-insensitive lookup against an object defined in `static`.

**Syntax**

```oml
lookup_nocase(<dict_symbol>, <key_expr>, <default_expr>)
```

**Parameters**

- `dict_symbol`: the static object symbol name
- `key_expr`: the key to look up, commonly `read(status)`
- `default_expr`: the fallback value returned when no match is found

**Current implementation details**

- the first argument must be a static symbol name
- the second and third arguments may themselves contain nested `lookup_nocase`, direct `read/take`, noprefix pipes, SQL literals, value literals, `Now::*`, or static symbols
- the key is normalized with `trim + lowercase` before lookup

**Example**

```oml
static {
    score_map = object {
        error = float(90.0);
        warning = float(70.0);
        success = float(20.0);
    };
}

risk_score : float = lookup_nocase(score_map, read(status), 40.0) ;
```

For more details, see [functions/lookup_nocase.md](./functions/lookup_nocase.md).

---

### `Now::time()`

Returns the current time.

```oml
event_time : time = Now::time() ;
```

---

### `Now::date()`

Returns the current date in `YYYYMMDD` format.

```oml
today : digit = Now::date() ;
```

---

### `Now::hour()`

Returns the current hour in `YYYYMMDDHH` format.

```oml
current_hour : digit = Now::hour() ;
```

---

## Pipe Functions

### Basic Form

```oml
result = pipe read(field) | function1 | function2(param) ;
result = read(field) | function1 | function2(param) ;
```

The current parser only allows these pipe starting points:

- `read(...)`
- `take(...)`
- `@field`

A bare identifier cannot be used directly as a noprefix pipe source.

---

## Access Functions

### `nth(index)`

Reads the `index`-th element from an array.

```oml
first_item = read(items) | nth(0) ;
```

### `get(name)`

Reads a value from an object or nested structure. The argument is parsed with the implementation's `take_key`, so path-like values such as `a/b/c` are also accepted.

```oml
status = read(obj) | get(status) ;
nested = read(obj) | get(one/two) ;
```

### `path([name|path])`

Extracts a specific part from a file path.

```oml
file_name = read(file_path) | path(name) ;
dir_path = read(file_path) | path(path) ;
```

### `url([domain|host|uri|path|params])`

Extracts a specific part from a URL.

```oml
host = read(url) | url(host) ;
path_only = read(url) | url(path) ;
query = read(url) | url(params) ;
```

---

## Condition / Mapping Functions

### `starts_with('prefix')`

Keeps the original value only when the input string starts with the given prefix; otherwise the result becomes `ignore`.

```oml
https_only = read(url) | starts_with('https://') ;
```

### `map_to(value)`

Maps a non-`ignore` input to a fixed value.

Current supported argument types:

- `bool`
- integer
- floating-point number
- single-quoted string

```oml
is_error = read(level) | starts_with('ERROR') | map_to(true) ;
bucket = read(status) | starts_with('5') | map_to(500) ;
```

---

## Encoding / Escaping Functions

### `base64_encode`

```oml
encoded = read(message) | base64_encode ;
```

### `base64_decode([EncodeType])`

The argument may be omitted. If omitted, it defaults to `Utf8`.

```oml
decoded = read(payload) | base64_decode() ;
decoded_gbk = read(payload) | base64_decode(Gbk) ;
```

`EncodeType` follows the implementation enum. Common values include `Utf8`, `Gbk`, `Ascii`, and `Imap`.

### `html_escape`

```oml
safe_html = read(raw_html) | html_escape ;
```

### `html_unescape`

```oml
raw_html = read(encoded_html) | html_unescape ;
```

### `str_escape`

```oml
escaped = read(message) | str_escape ;
```

### `json_escape`

```oml
escaped = read(message) | json_escape ;
```

### `json_unescape`

```oml
raw = read(message) | json_unescape ;
```

---

## Time Conversion Functions

### `Time::to_ts`

Converts to a second-level timestamp.

```oml
ts = read(event_time) | Time::to_ts ;
```

### `Time::to_ts_ms`

Converts to a millisecond timestamp.

```oml
ts = read(event_time) | Time::to_ts_ms ;
```

### `Time::to_ts_us`

Converts to a microsecond timestamp.

```oml
ts = read(event_time) | Time::to_ts_us ;
```

### `Time::to_ts_zone(zone, unit)`

Converts to a timestamp using the specified timezone offset and unit.

Current supported units:

- `s`
- `ss`
- `ms`
- `us`

```oml
utc_ms = read(event_time) | Time::to_ts_zone(0, ms) ;
local_s = read(event_time) | Time::to_ts_zone(8, s) ;
```

---

## Conversion / Control Functions

### `to_json`

```oml
payload = read(arr) | to_json ;
```

### `to_str`

```oml
ip_text = read(src_ip) | to_str ;
```

### `ip4_to_int`

```oml
ip_num = read(src_ip) | ip4_to_int ;
```

### `skip_empty`

Returns `ignore` for an empty string; otherwise passes through the original value.

```oml
city = read(city) | skip_empty ;
```

---

## Text Extraction Functions

### `extract_main_word`

Extracts the main word from text.

```oml
keyword = read(message) | extract_main_word ;
```

### `extract_subject_object`

Extracts subject-object structure from text.

```oml
summary = read(message) | extract_subject_object ;
```

---

## Related Documents

- [06-grammar-reference.md](./06-grammar-reference.md)
- [functions/function_index.md](./functions/function_index.md)
- [functions/match_functions.md](./functions/match_functions.md)
