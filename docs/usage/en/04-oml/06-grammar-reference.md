# OML Grammar Reference

This document summarizes the current OML grammar based on the implementation in `wp-motor/crates/wp-oml/src/parser`. It is an implementation-oriented grammar summary, not an idealized language spec detached from the code.

> Key boundaries:
> - there is no formal privacy-section syntax in the current implementation
> - SQL starts with `select`
> - there is no `query ...` top-level syntax
> - `static` only supports pure constant expressions

---

## EBNF Conventions

- `|`: alternation
- `[ ... ]`: optional
- `{ ... }`: repeat zero or more times
- `(...)`: grouping
- literal keywords are written directly as strings

---

## Comments

OML supports `//` single-line comments and `#` single-line comments. They are stripped by `CommentParser::ignore_comment` before parsing (`wp-motor/crates/wp-oml/src/parser/code.rs`), so they may appear anywhere — between statements, after statements, or on the same line — without affecting the grammar below:

```oml
// top-level comment
# also supported
name : demo_alert
rule : /demo/log
---
// header-to-body separator is "---"
field1 = read(f1) ;   // trailing comment
field2 = read(f2) ;   # hash trailing comment
```

Notes:

- comments are removed during preprocessing and never reach the parser
- the grammar-level `comment` rule in `tree-sitter-oml` recognizes both `#` and `//` forms
- unlike WPL (which cannot use `#` because of `#[...]` annotations), OML has no annotation syntax, so `#` is safe as a comment marker

---

## Top-Level Structure

```ebnf
oml               = name_decl, { head_decl }, sep_line,
                    { static_block }, aggregate_item, { aggregate_item },
                    [ sep_line ] ;

name_decl         = "name", ":", obj_path ;
head_decl         = enable_decl | rule_decl ;
enable_decl       = "enable", ":", ("true" | "false") ;
rule_decl         = "rule", ":", rule_path, { rule_path } ;
rule_path         = path_with_wildcard ;
sep_line          = "---" ;

static_block      = "static", "{", static_item, { static_item }, "}" ;
static_item       = target, "=", static_expr, ";" ;

aggregate_item    = target_list, "=", eval_expr, ";" ;
target_list       = target, { ",", target } ;
target            = target_name, [ ":", data_type ] ;
target_name       = wild_key | "_" ;
```

## Top-Level Notes

- `name` is required and must come first
- `rule` and `enable` are both optional, and may appear in either order
- `rule` may contain multiple rule paths
- `---` is the required separator between the header and the body
- `static` must appear after `---` and before normal aggregate statements
- the body must contain at least one aggregate statement
- one extra trailing `---` is tolerated at the end, but it is not treated as the beginning of a privacy section

---

## Evaluation Expressions

```ebnf
eval_expr         = calc_expr
                  | match_expr
                  | lookup_expr
                  | object_expr
                  | pipe_expr
                  | collect_expr
                  | sql_expr
                  | fmt_expr
                  | access_direct_expr
                  | direct_expr
                  | value_expr
                  | builtin_fun
                  | static_ref ;
```

### `access_direct(...)`

Combines the intranet sides of src/dst IPs into an access direction, returning `L2L`/`L2W`/`W2L`/`W2W`.

```ebnf
access_direct_expr = "access_direct", "(", var_get, ",", var_get, ")", [ "|", pipe_item, { "|", pipe_item } ] ;
```

**Notes**
- Intranet-side checks use the intranet network knowledge (`[intranet_nets]` section of `knowdb.toml`)
- Missing/invalid src or dst output `Ignore`; chain `| on_fail('unknown')` for a fallback
- Supports IPv4 + IPv6; IPv4-mapped IPv6 is judged as IPv4

**Example**
```oml
direction = access_direct(read(sip), read(dip)) | on_fail('unknown') ;
# intranet→intranet: "L2L"; intranet→internet: "L2W"; internet→intranet: "W2L"; internet→internet: "W2W"
# missing src/dst: "unknown" (on_fail fallback)
```

### Direct Access

```ebnf
direct_expr       = read_expr | take_expr ;
read_expr         = "read", "(", [ read_args ], ")", [ default_body ] ;
take_expr         = "take", "(", [ read_args ], ")", [ default_body ] ;

read_args         = read_arg, { ",", read_arg } ;
read_arg          = option_arg | keys_arg | get_arg | json_path ;
option_arg        = "option", ":", "[", key_list, "]" ;
keys_arg          = ("in" | "keys"), ":", "[", key_list, "]" ;
get_arg           = "get", ":", simple_key ;
default_body      = "{", "_", ":", default_expr, [ ";" ], "}" ;
default_expr      = read_expr | take_expr | value_expr | builtin_fun | static_ref ;
```

**Notes**

- the current parser only recognizes `option`, `in`, `keys`, `get`, and JSON Path inside `read(...)` / `take(...)`
- `in:` and `keys:` are equivalent in the current implementation
- `@field` is syntactic sugar for `read(field)`, but it is not a standalone top-level expression; it only appears where `var_get` is accepted, such as `fmt`, `pipe`, `calc`, and `match`

**Examples**

```oml
id = read(user_id) ;
uid = read(option:[uid, user_id, id]) ;
metrics = collect read(keys:[cpu_*, mem_*]) ;
name = read(/user/info/name) ;
country = read(country) { _ : chars(CN) } ;
```

---

## Value Literals and Built-in Functions

```ebnf
value_expr        = data_type, "(", literal_body, ")" ;
builtin_fun       = "Now::time", "(", ")"
                  | "Now::date", "(", ")"
                  | "Now::hour", "(", ")" ;
static_ref        = ident ;
```

The `data_type` set (aligned with `tree-sitter-oml`):

```text
auto, ip, chars, digit, float, time, bool, obj, array,
time_iso, time_3339, time_2822, time_timestamp, time/clf,
url, domain, ip_net, kv, json, base64,
array/<subtype>, time/<subtype>
```

**Notes**

- the actual type set for value literals reuses the WPL datatype parser
- `obj` is OML-specific and does not appear in the WPL type table
- `array/<subtype>` and `time/<subtype>` accept a slash-suffixed subtype (e.g. `array/digit`, `time/iso`)
- the only directly supported built-in functions are `Now::time()`, `Now::date()`, and `Now::hour()`
- a bare identifier is treated as an expression only when it refers to a static symbol

---

## `fmt(...)`

```ebnf
fmt_expr          = "fmt", "(", string, ",", var_get, { ",", var_get }, ")" ;
var_get           = read_expr | take_expr | "@", ident ;
```

**Notes**

- `fmt` requires at least one argument after the format string
- the format string is currently parsed as a double-quoted string

**Example**

```oml
message = fmt("{host}:{code}", @host, read(status_code)) ;
```

---

## Pipe Expressions

```ebnf
pipe_expr         = [ "pipe" ], var_get, pipe_item, { pipe_item } ;
pipe_item         = "|", pipe_fun ;
pipe_fun          = pipe_fun_with_args | pipe_fun_simple ;

pipe_fun_with_args = "nth", "(", unsigned, ")"
                   | "get", "(", key_path, ")"
                   | "starts_with", "(", string, ")"
                   | "map_to", "(", map_value, ")"
                   | "base64_decode", "(", [ encode_type ], ")"
                   | "path", "(", [ "name" | "path" ], ")"
                   | "url", "(", [ "domain" | "host" | "uri" | "path" | "params" ], ")"
                   | "Time::to_ts_zone", "(", signed_int, ",", ("s" | "ss" | "ms" | "us"), ")" ;

pipe_fun_simple   = "base64_encode"
                  | "html_escape"
                  | "html_unescape"
                  | "str_escape"
                  | "json_escape"
                  | "json_unescape"
                  | "Time::to_ts"
                  | "Time::to_ts_ms"
                  | "Time::to_ts_us"
                  | "to_json"
                  | "to_str"
                  | "skip_empty"
                  | "ip4_to_int"
                  | "extract_main_word"
                  | "extract_subject_object" ;
```

**Notes**

- the `pipe` keyword may be omitted
- even without the keyword, the pipe source must still be `read(...)`, `take(...)`, or `@field`
- the current implementation does not support `sxf_get(...)` or `str_unescape`

**Examples**

```oml
host = read(url) | url(host) ;
ts = pipe read(time) | Time::to_ts_zone(0, ms) ;
flag = @message | starts_with('ERROR') | map_to(true) ;
```

---

## Objects and Arrays

```ebnf
object_expr       = "object", "{", object_item, { object_item }, "}" ;
object_item       = object_targets, "=", object_value, [ ";" ] ;
object_targets    = ident, { ",", ident }, [ ":", data_type ] ;
object_value      = read_expr | take_expr | value_expr | builtin_fun | static_ref ;

collect_expr      = "collect", var_get ;
```

**Notes**

- `object { ... }` does not recursively accept all top-level expressions
- object sub-values currently only support `read/take`, value literals, `Now::*`, and static symbols
- `collect` always takes a `var_get` source

**Examples**

```oml
info : obj = object {
    host : chars = read(hostname) ;
    city, province : chars = read() ;
} ;

ports : array = collect read(keys:[sport, dport]) ;
```

---

## `calc(...)`

```ebnf
calc_expr         = "calc", "(", add_expr, ")" ;
add_expr          = mul_expr, { ("+" | "-"), mul_expr } ;
mul_expr          = unary_expr, { ("*" | "/" | "%"), unary_expr } ;
unary_expr        = [ "-" ], primary_expr ;
primary_expr      = number | var_get | calc_fun | "(", add_expr, ")" ;
calc_fun          = ("abs" | "round" | "floor" | "ceil"), "(", add_expr, ")" ;
```

**Notes**

- field access inside `calc` supports `read(...)`, `take(...)`, and `@field`
- `%` only accepts integers

---

## `match ... { ... }`

```ebnf
match_expr        = "match", match_source, "{", match_case, { match_case }, "}" ;
match_source      = var_get | "(", var_get, { ",", var_get }, ")" ;
match_case        = match_cond, "=>", match_value, [ "," ], [ ";" ] ;
match_value       = read_expr | take_expr | value_expr | collect_expr | static_ref ;
```

The current implementation supports these condition forms:

- equality: `chars(ok)`
- negation: `!chars(error)`
- range: `in (digit(200), digit(299))`
- static-symbol equality / range
- condition functions:
  - `starts_with`
  - `ends_with`
  - `contains`
  - `regex_match`
  - `is_empty`
  - `iequals`
  - `iequals_any`
  - `gt`
  - `lt`
  - `eq`
  - `in_range`
- OR conditions: `cond1 | cond2`

**Important limitation**

- `calc(...)` is not accepted directly as a `match` branch result
- if you need a computed branch result, bind it to a field first and then `read(...)` it inside `match`

**Examples**

```oml
level = match read(status_code) {
    in (digit(200), digit(299)) => chars(success) ;
    in (digit(500), digit(599)) => chars(server_error) ;
    _ => chars(other) ;
} ;

priority = match (read(city), read(level)) {
    (chars(bj) | chars(sh), chars(high)) => chars(p1) ;
    _ => chars(p9) ;
} ;
```

---

## `lookup_nocase(...)`

```ebnf
lookup_expr       = "lookup_nocase", "(", ident, ",", lookup_arg, ",", lookup_arg, ")" ;
lookup_arg        = lookup_expr
                  | pipe_expr_noprefix
                  | direct_expr
                  | sql_literal
                  | value_expr
                  | builtin_fun
                  | static_ref ;
pipe_expr_noprefix = var_get, pipe_item, { pipe_item } ;
```

**Notes**

- the first argument is the static object symbol name
- the second and third arguments may be nested expressions, but only from the current parser's `lookup_arg` set

---

## SQL

```ebnf
sql_expr          = "select", sql_body, "where", sql_cond, ";" ;
```

**Current implementation details**

- SQL must start with `select`
- both `where` and the final `;` are required
- in strict mode, the body between `select` and `where` is validated as `<cols> from <table>`
- the `where` condition is parsed by the internal condition parser and may reference `read(...)`
- there is a compatibility rewrite for `fn(...) = literal`

**Example**

```oml
user_name, level =
    select name, level
    from users
    where id = read(user_id) ;
```

---

## `static { ... }`

```ebnf
static_expr       = static_value | static_object | static_calc ;
static_value      = value_expr ;
static_object     = object_expr ;
static_calc       = "calc", "(", static_calc_expr, ")" ;
```

**Restrictions**

- does not support `read(...)` or `take(...)`
- does not support `pipe` or `collect`
- does not support `fmt(...)`
- does not support `match`
- does not support `lookup_nocase(...)`
- does not support SQL
- does not support `Now::*`
- does not support referencing other static symbols

Even though `static_object` uses the `object { ... }` form, every inner object item must still satisfy these same static restrictions.

---

## Batch Targets

When a target name contains `*`, the current implementation enters batch mode:

```oml
* = take() ;
alert* = take() ;
*_log = read() ;
```

Batch mode only supports `read(...)` and `take(...)` as sources.

---

## Implementation-Difference Notes

If you see any of the following in older docs or examples, prefer the current implementation:

- privacy sections
- `query ...`
- `sxf_get(...)`
- runtime expressions inside `static`
- direct `calc(...)` results inside `match` branches
