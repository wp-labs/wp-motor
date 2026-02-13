# WPL Grammar Reference (EBNF)

This document provides the formal grammar definition of WPL, suitable for:
- Compiler/parser developers
- Precise understanding of syntax rules
- Tool integration development

**For regular users, please refer to:**
- Quick Start: [01-quickstart.md](./01-quickstart.md)
- Core Concepts: [02-core-concepts.md](./02-core-concepts.md)
- Practical Guide: [03-practical-guide.md](./03-practical-guide.md)
- Language Reference: [04-language-reference.md](./04-language-reference.md)

---

## 📑 Document Navigation

| Section | Description |
|---------|-------------|
| [Complete EBNF Definition](#complete-ebnf-definition) | Formal grammar definition |
| [Semantics](#semantics) | Semantic interpretation of grammar rules |
| [Implementation Reference](#implementation-reference) | Source code locations |

---

## Complete EBNF Definition

The authoritative implementation is in the `crates/wp-lang` parser; this document is kept in sync with the source code.

```ebnf
; WPL Grammar (EBNF)
; Based on the parser implementation (winnow) in crates/wp-lang
; Note: This file provides grammar productions and necessary lexical conventions. Unless explicitly noted, optional whitespace `ws` is allowed between tokens.

wpl_document     = { package_decl } ;

package_decl     = [ annotation ] "package" ws? ident ws? "{" ws? rule_decl+ ws? "}" ;

rule_decl        = [ annotation ] "rule" ws? rule_name ws? "{" ws? statement ws? "}" ;

statement        = plg_pipe_block | express ;

plg_pipe_block   = ["@"]? "plg_pipe" ws? "(" ws? "id" ws? ":" ws? key ws? ")" ws? "{" ws? express ws? "}" ;

express          = [ preproc ] group { ws? "," ws? group } ;

preproc          = "|" ws? preproc_step { ws? "|" ws? preproc_step } ws? "|" ;   ; At least one step, ending with '|'
preproc_step     = builtin_preproc | plg_pipe_step ;
builtin_preproc  = ns "/" name ;
plg_pipe_step    = "plg_pipe" ws? "/" ws? key ;                   ; Look up custom extensions through registry
ns               = "decode" | "unquote" | "strip" ;              ; Namespace whitelist
name             = ("base64" | "hex") | "unescape" | "bom" ;     ; Step name whitelist
; Supported preprocessors:
;   decode/base64 - Base64 decoding
;   decode/hex - Hexadecimal decoding
;   unquote/unescape - URL unescape decoding
;   strip/bom - Remove BOM (Byte Order Mark)

group            = [ group_meta ] ws? "(" ws? field_list_opt ws? ")" [ ws? group_len ] [ ws? group_sep ] ;
group_meta       = "alt" | "opt" | "some_of" | "seq" | "not" ;
group_len        = "[" number "]" ;
group_sep        = sep ;

; List: allows empty, allows trailing comma
field_list_opt   = [ field { ws? "," ws? field } [ ws? "," ] ] ;

field            = [ repeat ] data_type [ symbol_content ]
                   [ subfields ]
                   [ ":" ws? var_name ]
                   [ length ]
                   [ format ]
                   [ sep ]
                   { pipe } ;                              ; Allows multiple pipes

repeat           = [ number ] "*" ;                        ; "*ip" or "3*ip"
length           = "[" number "]" ;                       ; Only top-level fields support (subfields do not)

; Subfield list for composite fields (e.g., kvarr/json)
subfields        = "(" ws? subfields_opt ws? ")" ;
subfields_opt    = [ subfield { ws? "," ws? subfield } [ ws? "," ] ] ;
subfield         = [ opt_datatype | data_type ]
                   [ symbol_content ]
                   [ "@" ref_path ]
                   [ ":" ws? var_name ]
                   [ format ]
                   [ sep ]
                   { pipe } ;

opt_datatype     = "opt" "(" ws? data_type ws? ")" ;     ; Declare this subfield as optional

; Field data types (corresponds to external crate wp-model-core::DataType)
data_type        = builtin_type | ns_type | array_type ;

builtin_type     = "auto" | "bool" | "chars" | "symbol" | "peek_symbol"
                   | "digit" | "float" | "_" | "sn"
                   | "time" | "time/clf" | "time_iso" | "time_3339" | "time_2822" | "time_timestamp"
                   | "ip" | "ip_net" | "domain" | "email" | "port"
                   | "hex" | "base64"
                   | "kv" | "kvarr" | "json" | "exact_json"
                   | "url"
                   | "proto_text" | "obj"
                   | "id_card" | "mobile_phone" ;

ns_type          = path_ident ;                               ; e.g., http/request, http/status, etc.
; Note: Implementation should validate whitelist prefixes (like "http/") to avoid arbitrary path expansion.
; Supported namespace types:
;   http/request, http/status, http/agent (or http/user_agent), http/method
;   time/clf (or time/apache, time/httpd, time/nginx)
;   time/rfc3339 (alias time_3339), time/rfc2822 (alias time_2822)
;   time/timestamp (or time/epoch, alias time_timestamp)
;   proto/text (alias proto_text)

array_type       = "array" [ "/" subtype ] ;                 ; e.g., "array" or "array/ip" or "array/chars"
subtype          = key ;                                      ; Can be any supported type name

; Only allowed when data_type is symbol/peek_symbol
symbol_content   = "(" symbol_chars ")" ;

; Field display/extraction format
format           = scope_fmt | quote_fmt ;
scope_fmt        = "<" any_chars "," any_chars ">" ;   ; Scope delimiters, e.g., <[,]>
quote_fmt        = '"' ;                                ; Equivalent to '"' at both ends


; Separator (two forms)
sep              = shortcut_sep | pattern_sep ;

; Shortcut separator: backslash-escaped character sequence
shortcut_sep     = sep_char , { sep_char } ;             ; e.g., "\\," => ","; "\\!\\|" => "!|"
sep_char         = '\\' , any_char ;

; Pattern separator: brace-wrapped pattern expression
pattern_sep      = "{" pattern_content "}" ;
pattern_content  = glob_segments [ preserve ] ;
glob_segments    = glob_segment { glob_segment } ;
glob_segment     = literal_char | wildcard | escape_seq ;

; Wildcards
wildcard         = "*" | "?" ;                          ; "*" - zero or more arbitrary characters (non-greedy); "?" - exactly one arbitrary character
; Constraint: at most one "*" allowed in a pattern

; Escape sequences
escape_seq       = "\\" ( special_char | macro_char ) ;
special_char     = "\\" | "*" | "?" | "{" | "}" | "(" | ")" ;  ; Literal escape
macro_char       = "s" | "S" | "h" | "H" | "0" | "n" | "t" | "r" ;  ; Macro characters
; Supported macros:
;   \s - one or more consecutive whitespace characters [ \t\r\n]+
;   \S - one or more consecutive non-whitespace characters [^ \t\r\n]+
;   \h - one or more consecutive horizontal whitespace [ \t]+
;   \H - one or more consecutive non-horizontal-whitespace [^ \t]+
;   \0 - null byte
;   \n - newline
;   \t - tab
;   \r - carriage return

; Preserve marker
preserve         = "(" glob_segments ")" ;              ; Match but don't consume, only at pattern end
; Constraints:
;   1. preserve can only appear at the end of pattern_content
;   2. preserve cannot contain "*" wildcard
;   3. preserve cannot be nested

; Literal character (any character except special characters)
literal_char     = any_char_except_special ;            ; Any character except '\', '*', '?', '{', '}', '(', ')'

; Field-level pipe: function call or nested group
pipe             = "|" ws? ( fun_call | group ) ;

; Built-in functions (wpl_fun.rs):
; - Selector functions: take, last
; - f_ prefix indicates field collection operations (requires field name)
; - No prefix indicates active field operations
; - Transform functions: json_unescape, base64_decode, chars_replace
; - Wrapper functions: not
fun_call         = selector_fun | target_fun | active_fun | transform_fun | wrapper_fun ;

; Selector functions
selector_fun     = take_fun | last_fun ;
take_fun         = "take" "(" ws? key ws? ")" ;
last_fun         = "last" "(" ws? ")" ;

; Field collection operation functions (f_ prefix)
target_fun       = f_has | f_chars_has | f_chars_not_has | f_chars_in
                 | f_digit_has | f_digit_in | f_ip_in ;
f_has            = "f_has" "(" ws? key ws? ")" ;
f_chars_has      = "f_chars_has" "(" ws? key ws? "," ws? path ws? ")" ;
f_chars_not_has  = "f_chars_not_has" "(" ws? key ws? "," ws? path ws? ")" ;
f_chars_in       = "f_chars_in" "(" ws? key ws? "," ws? path_array ws? ")" ;
f_digit_has      = "f_digit_has" "(" ws? key ws? "," ws? number ws? ")" ;
f_digit_in       = "f_digit_in" "(" ws? key ws? "," ws? number_array ws? ")" ;
f_ip_in          = "f_ip_in" "(" ws? key ws? "," ws? ip_array ws? ")" ;

; Active field operation functions (no prefix)
active_fun       = has_fun | chars_has | chars_not_has | chars_in | starts_with | regex_match
                 | digit_has | digit_in | digit_range | ip_in ;
has_fun          = "has" "(" ws? ")" ;
chars_has        = "chars_has" "(" ws? path ws? ")" ;
chars_not_has    = "chars_not_has" "(" ws? path ws? ")" ;
chars_in         = "chars_in" "(" ws? path_array ws? ")" ;
starts_with      = "starts_with" "(" ws? quoted_string ws? ")" ;
regex_match      = "regex_match" "(" ws? quoted_string ws? ")" ;
digit_has        = "digit_has" "(" ws? number ws? ")" ;
digit_in         = "digit_in" "(" ws? number_array ws? ")" ;
digit_range      = "digit_range" "(" ws? number ws? "," ws? number ws? ")" ;
ip_in            = "ip_in" "(" ws? ip_array ws? ")" ;

; Transform functions
transform_fun    = json_unescape | base64_decode | chars_replace ;
json_unescape    = "json_unescape" "(" ws? ")" ;
base64_decode    = "base64_decode" "(" ws? ")" ;
chars_replace    = "chars_replace" "(" ws? path ws? "," ws? path ws? ")" ;

; Wrapper functions
wrapper_fun      = not_fun ;
not_fun          = "not" "(" ws? fun_call ws? ")" ;

path_array       = "[" ws? path { ws? "," ws? path } ws? "]" ;
number_array     = "[" ws? number { ws? "," ws? number } ws? "]" ;
ip_array         = "[" ws? ip_addr { ws? "," ws? ip_addr } ws? "]" ;

annotation       = "#[" ws? ann_item { ws? "," ws? ann_item } ws? "]" ;
ann_item         = tag_anno | copy_raw_anno ;
tag_anno         = "tag" "(" ws? tag_kv { ws? "," ws? tag_kv } ws? ")" ;
tag_kv           = ident ":" ( quoted_string | raw_string ) ;      ; Key is identifier; value is string
copy_raw_anno    = "copy_raw" "(" ws? "name" ws? ":" ws? ( quoted_string | raw_string ) ws? ")" ;

; Lexical and auxiliary tokens --------------------------------------------------------
field_name       = var_name ;
rule_name        = exact_path ;
key              = key_char { key_char } ;              ; [A-Za-z0-9_./-]+
var_name         = var_char { var_char } ;              ; [A-Za-z0-9_.-]+
ref_path         = ref_char { ref_char } ;              ; [A-Za-z0-9_./\-.[\]*]+
; Identifier and path identifier (recommended syntax)
ident            = ( letter | '_' ) { letter | digit | '_' | '.' | '-' } ;
path_ident       = ident { "/" ident } ;

exact_path       = exact_path_char { exact_path_char } ; ; Does not contain '[' ']' '*'
exact_path_char  = letter | digit | '_' | '.' | '/' | '-' ;
path             = key | ref_path ;

number           = digit { digit } ;
digit            = '0' | '1' | '2' | '3' | '4' | '5' | '6' | '7' | '8' | '9' ;

key_char         = letter | digit | '_' | '.' | '/' | '-' ;
var_char         = letter | digit | '_' | '.' | '-' ;
ref_char         = key_char | '[' | ']' | '*' ;

letter           = 'A'..'Z' | 'a'..'z' ;

quoted_string    = '"' { escaped | char_no_quote_backslash } '"' ;
raw_string       = 'r' '#' '"' { any_char } '"' '#' ;          ; r#"..."#, no escape processing inside (can contain '"')
char_no_quote    = ? any char except '"' ? ;
escaped          = '\\' ( '"' | '\\' | 'n' | 't' | 'r' | 'x' hex hex ) ;
char_no_quote_backslash = ? any char except '"' and '\\' ? ;
hex              = '0'..'9' | 'a'..'f' | 'A'..'F' ;

free_string      = { fchar } ;                          ; Until ',' or ')' (not included)
fchar            = ? any char except ',' and ')' ? ;

symbol_chars     = { schar } ;                          ; Allows all chars except ')' and '\\', or use '\)' to escape
schar            = char_no_close_paren_backslash | '\\' ')' ;
char_no_close_paren_backslash = ? any char except ')' and '\\' ? ;
any_chars        = { any_char } ;
any_char         = ? any character ? ;

ip_addr          = quoted_string | ipv4 | ipv6 ;        ; Supports IPv4/IPv6 bare literals or quoted
ipv4             = digit1 "." digit1 "." digit1 "." digit1 ;
digit1           = digit { digit } ;
ipv6             = ? valid IPv6 literal (RFC 4291), including compressed forms like "::1" ? ;

ws               = { ' ' | '\t' | '\n' | '\r' } ;

; Reserved keywords (cannot be used as identifiers; conflict validation performed by implementation)
ReservedKeyword  = "package" | "rule" | "alt" | "opt" | "some_of" | "seq" | "not" | "order"
                 | "tag" | "copy_raw" | "include" | "macro" ;


```

---

## Semantics

### Preprocessing Pipeline
- The `preproc` pipeline (e.g., `|decode/base64|unquote/unescape|`) appears at the beginning of `express`, independent of field-level `pipe`
- Applies to the entire line input, executed before field parsing
- Execution order: left to right
- Must end with `|`
- Supported preprocessors:
  - `decode/base64` - Base64 decoding
  - `decode/hex` - Hexadecimal decoding
  - `unquote/unescape` - URL unescape decoding
  - `strip/bom` - Remove BOM (Byte Order Mark)

### Group Metadata (Group Meta)
- `group_meta` specifies the behavior pattern of a group
- `alt` - Alternative matching, matches any one field in the group
- `opt` - Optional group, the entire group may not exist
- `some_of` - Partial matching, matches some fields in the group
- `seq` - Sequence matching, matches all fields in the group in order
- `not` - Negation matching, matches content other than the fields in the group

### Group Length and Separator
- `group` can be followed by `[n]` and separator `sep`
- Length applies to all fields within the group
- `sep` is only stored on the group, specific combination strategy see implementation


### Symbol Types
- `symbol/peek_symbol` can carry `symbol_content`, e.g., `symbol(boy)`
- `peek_symbol` is equivalent to `symbol`, only changing the "peek" semantics (lookahead without consuming)

### Subfields
- In `subfields`, when `"@ref"` is not explicitly specified, the key defaults to `"*"` (wildcard key)
- Subfields support `opt(type)` to mark as optional

### Separators
Separators support two forms:

#### Shortcut Separator
- Single or multiple characters escaped with backslash
- For example, `\\,` represents comma `,`, `\\!\\|` represents string `"!|"`
- Priority: field-level > group-level > upstream

#### Pattern Separator
- Pattern expression wrapped in braces `{...}`
- Supports wildcards, whitespace macros, and preserve markers
- Suitable for complex separation scenarios

**Wildcards:**
- `*` - Zero or more arbitrary characters (non-greedy, shortest match)
- `?` - Exactly one arbitrary character
- Constraint: at most one `*` allowed in a pattern

**Whitespace Macros:**
- `\s` - One or more consecutive whitespace characters `[ \t\r\n]+`
- `\S` - One or more consecutive non-whitespace characters `[^ \t\r\n]+`
- `\h` - One or more consecutive horizontal whitespace `[ \t]+`
- `\H` - One or more consecutive non-horizontal-whitespace `[^ \t]+`

**Preserve Marker:**
- `(...)` - Match but don't consume, leave matched content for next stage
- Can only appear at pattern end
- Cannot contain `*` wildcard inside

**Examples:**
- `chars{*=}` - Match to first `=` sign
- `chars{\s=}` - Skip whitespace then match `=`
- `chars{*(key=)}` - Match to `key=` and preserve `key=` for next field
- `chars{field?:\s}` - Match `field` + any char + `:` + whitespace

**Constraints:**
- At most one `*` in a pattern
- `()` only at pattern end
- `*` not allowed inside `()`
- `()` cannot be nested
- Cannot be used with upstream separator

### Annotations
- `annotation` can be used for `package` and `rule`
- If both exist, they are merged on the `rule` side (`rule` takes priority)

### Pipe Functions
- Field-level pipe functions include: selectors, condition checks, transforms, wrappers
- `not()` wrapper can invert the success/failure result of any pipe function
- `starts_with()` checks if a string field starts with a specified prefix
- `regex_match()` supports regular expression pattern matching
- `digit_range()` supports numeric range checking (closed interval)
- `chars_replace()` performs string replacement operations

---

## Related Documentation

- Quick Start: [01-quickstart.md](./01-quickstart.md)
- Core Concepts: [02-core-concepts.md](./02-core-concepts.md)
- Practical Guide: [03-practical-guide.md](./03-practical-guide.md)
- Language Reference: [04-language-reference.md](./04-language-reference.md)
- Functions Reference: [05-functions-reference.md](./05-functions-reference.md)

---

## Implementation Reference

- Grammar implementation: `crates/wp-lang/src/parser/`
- Pipe functions: `crates/wp-lang/src/parser/wpl_fun.rs`
- Data types: External crate `wp-model-core`
