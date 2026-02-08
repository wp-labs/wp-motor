# WPL Functions Reference

> **Translation in Progress**
> 
> This document is currently being translated from Chinese to English.
> 
> Please refer to the Chinese version: `docs/10-user/03-wpl-new/05-functions-reference.md`

---

This document provides standardized reference for all WPL functions.

## WPL All Functions Quick Reference

### Preprocessing Pipeline Functions

| Function | Description | Example |
|----------|-------------|---------|
| `decode/base64` | Decode entire line as Base64 | `\|decode/base64\|` |
| `decode/hex` | Decode entire line as hexadecimal | `\|decode/hex\|` |
| `unquote/unescape` | Remove quotes and unescape | `\|unquote/unescape\|` |
| `plg_pipe/<name>` | Custom preprocessing extension | `\|plg_pipe/dayu\|` |

### Selector Functions

| Function | Description | Example |
|----------|-------------|---------|
| `take(name)` | Select specified field as active | `\|take(name)\|` |
| `last()` | Select last field as active | `\|last()\|` |

### Field Set Operations (f_ prefix)

| Function | Description | Example |
|----------|-------------|---------|
| `f_has(name)` | Check if field exists | `\|f_has(status)\|` |
| `f_chars_has(name, val)` | Check field equals string | `\|f_chars_has(status, success)\|` |
| `f_chars_in(name, [...])` | Check field in string list | `\|f_chars_in(method, [GET, POST])\|` |
| `f_digit_has(name, num)` | Check field equals number | `\|f_digit_has(code, 200)\|` |
| `f_ip_in(name, [...])` | Check IP in list | `\|f_ip_in(client_ip, [127.0.0.1])\|` |

---

**For the complete English documentation, please check back later or refer to the Chinese version.**
