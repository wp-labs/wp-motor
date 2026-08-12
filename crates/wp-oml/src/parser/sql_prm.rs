use std::collections::HashMap;
use std::env;
use std::sync::atomic::{AtomicI8, Ordering};

#[cfg(test)]
use std::cell::Cell;

use winnow::combinator::fail;
use winnow::error::StrContext;
use winnow::token::take_until;

use wp_primitives::Parser;
use wp_primitives::WResult;
use wp_primitives::symbol::ctx_desc;

use crate::core::evaluator::query::sql::resolve_sql_route;
use crate::language::{
    ArgsTakeAble, CondAccessor, DirectAccessor, FieldTake, PreciseEvaluator, RecordOperation,
    SqlQuery,
};
use crate::parser::keyword::{kw_sql_select, kw_sql_where};

use super::cond::SCondParser;

#[cfg(test)]
thread_local! { static STRICT_TL: Cell<i8> = const { Cell::new(0) }; }

// 0: no override; 1: force strict on; -1: force strict off
static STRICT_OVERRIDE: AtomicI8 = AtomicI8::new(0);

fn is_sql_strict() -> bool {
    // test-thread override takes highest priority
    #[cfg(test)]
    {
        let v = STRICT_TL.with(|c| c.get());
        if v == 1 {
            return true;
        }
        if v == -1 {
            return false;
        }
    }
    // global override (rarely used)
    let ov = STRICT_OVERRIDE.load(Ordering::Relaxed);
    if ov == 1 {
        return true;
    } else if ov == -1 {
        return false;
    }
    env::var("OML_SQL_STRICT")
        .ok()
        .map(|v| v != "0")
        .unwrap_or(true)
}

#[cfg(test)]
pub fn set_sql_strict_for_test(val: Option<bool>) {
    // None: clear override; Some(true): on; Some(false): off
    let v = match val {
        Some(true) => 1,
        Some(false) => -1,
        None => 0,
    };
    // set thread-local to avoid cross-test races
    STRICT_TL.with(|c| c.set(v));
}

// ============================================================================
// Helper functions for SQL parsing (extracted from oml_sql for readability)
// ============================================================================

/// Check if a string is a valid SQL identifier (alphanumeric, underscore, dot).
fn is_sql_ident(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
}

// 只按最外层逗号切分，忽略括号和引号内部的逗号。
// 用于解析 select 列表和 IN (...) 参数列表，避免把函数参数切碎。
fn split_top_level_commas(input: &str) -> Option<Vec<String>> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;
    let mut in_single = false;
    let mut in_double = false;

    for ch in input.chars() {
        match ch {
            '\'' if !in_double => {
                in_single = !in_single;
                current.push(ch);
            }
            '"' if !in_single => {
                in_double = !in_double;
                current.push(ch);
            }
            '(' if !in_single && !in_double => {
                depth += 1;
                current.push(ch);
            }
            ')' if !in_single && !in_double => {
                depth -= 1;
                if depth < 0 {
                    return None;
                }
                current.push(ch);
            }
            ',' if !in_single && !in_double && depth == 0 => {
                let item = current.trim();
                if item.is_empty() {
                    return None;
                }
                items.push(item.to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }

    if depth != 0 || in_single || in_double {
        return None;
    }

    let tail = current.trim();
    if tail.is_empty() {
        return None;
    }
    items.push(tail.to_string());
    Some(items)
}

fn is_supported_select_item(item: &str) -> bool {
    // 第一版仅放开简单列项：列名、*、单层函数，以及 fn(distinct col)。
    // 这里仍保持严格约束，避免把任意 SQL 表达式直接透传到 select body。
    let item = item.trim();
    if item == "*" || is_sql_ident(item) {
        return true;
    }

    let open = match item.find('(') {
        Some(pos) => pos,
        None => return false,
    };
    if !item.ends_with(')') {
        return false;
    }

    let fn_name = item[..open].trim();
    if !is_sql_ident(fn_name) {
        return false;
    }

    let inner = item[open + 1..item.len() - 1].trim();
    if inner.is_empty() {
        return false;
    }
    if fn_name.eq_ignore_ascii_case("string_agg") {
        return is_supported_string_agg_inner(inner);
    }
    if let Some(rest) = inner.strip_prefix("distinct") {
        return is_sql_ident(rest.trim());
    }
    is_sql_ident(inner)
}

fn is_sql_string_literal(s: &str) -> bool {
    let st = s.trim();
    (st.starts_with('\'') && st.ends_with('\'') && st.len() >= 2)
        || (st.starts_with('"') && st.ends_with('"') && st.len() >= 2)
}

fn is_supported_string_agg_inner(inner: &str) -> bool {
    let Some(args) = split_top_level_commas(inner) else {
        return false;
    };
    if args.len() != 2 {
        return false;
    }
    let Some(first) = args[0].trim().strip_prefix("distinct") else {
        return false;
    };
    let expr = first.trim();
    let delimiter = args[1].trim();
    is_sql_ident(expr) && is_sql_string_literal(delimiter)
}

fn find_matching_paren(input: &str, open_pos: usize) -> Option<usize> {
    let bytes = input.as_bytes();
    let mut depth = 0i32;
    let mut in_single = false;
    let mut in_double = false;

    for (idx, ch) in input.char_indices().skip_while(|(idx, _)| *idx < open_pos) {
        match ch {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '(' if !in_single && !in_double => depth += 1,
            ')' if !in_single && !in_double => {
                depth -= 1;
                if depth == 0 {
                    return Some(idx);
                }
                if depth < 0 {
                    return None;
                }
            }
            _ => {}
        }
    }

    let _ = bytes;
    None
}

fn find_top_level_from(body: &str) -> Option<usize> {
    let bytes = body.as_bytes();
    let mut idx = 0usize;
    let mut depth = 0i32;
    let mut in_single = false;
    let mut in_double = false;

    while idx < bytes.len() {
        let ch = bytes[idx] as char;
        match ch {
            '\'' if !in_double => {
                in_single = !in_single;
                idx += 1;
            }
            '"' if !in_single => {
                in_double = !in_double;
                idx += 1;
            }
            '(' if !in_single && !in_double => {
                depth += 1;
                idx += 1;
            }
            ')' if !in_single && !in_double => {
                depth -= 1;
                if depth < 0 {
                    return None;
                }
                idx += 1;
            }
            _ => {
                if !in_single
                    && !in_double
                    && depth == 0
                    && idx + 4 <= bytes.len()
                    && bytes[idx..idx + 4].eq_ignore_ascii_case(b"from")
                    && idx
                        .checked_sub(1)
                        .is_some_and(|prev| bytes[prev].is_ascii_whitespace())
                    && bytes
                        .get(idx + 4)
                        .is_some_and(|next| next.is_ascii_whitespace())
                {
                    return Some(idx);
                }
                idx += 1;
            }
        }
    }

    None
}

fn sanitize_select_stmt(stmt: &str) -> Option<String> {
    let stmt = stmt.trim();
    let lower = stmt.to_ascii_lowercase();
    let select_kw = "select ";
    if !lower.starts_with(select_kw) {
        return None;
    }
    let body = &stmt[select_kw.len()..];
    let sanitized = sanitize_sql_body(body)?;
    Some(format!("select {}", sanitized))
}

fn sanitize_from_source(source: &str) -> Option<String> {
    let source = source.trim();
    if source.is_empty() {
        return None;
    }
    if is_sql_ident(source) {
        return Some(source.to_string());
    }
    if !source.starts_with('(') {
        return None;
    }

    let close_pos = find_matching_paren(source, 0)?;
    let inner = source[1..close_pos].trim();
    let remainder = source[close_pos + 1..].trim();
    let alias = if remainder.is_empty() {
        None
    } else if remainder.len() >= 3 && remainder[..3].eq_ignore_ascii_case("as ") {
        let alias = remainder[3..].trim();
        if !is_sql_ident(alias) {
            return None;
        }
        Some(alias)
    } else {
        if !is_sql_ident(remainder) {
            return None;
        }
        Some(remainder)
    };

    let sanitized_inner = sanitize_select_stmt(inner)?;
    match alias {
        Some(alias) => Some(format!("({}) {}", sanitized_inner, alias)),
        None => Some(format!("({})", sanitized_inner)),
    }
}

/// Sanitize SQL body to ensure safe identifiers.
/// Only allows `<cols> from <table>` where cols are identifiers, `*`, or a single
/// SQL function call like `group_concat(distinct asset_type)`.
fn sanitize_sql_body(body: &str) -> Option<String> {
    let body_trim = body.trim();
    let from_pos = find_top_level_from(body_trim)?;
    let cols_part = &body_trim[..from_pos];
    let table_name = sanitize_from_source(body_trim[from_pos + 4..].trim())?;
    let cols = split_top_level_commas(cols_part)?;
    if cols.is_empty() {
        return None;
    }
    for c in &cols {
        if !is_supported_select_item(c) {
            return None;
        }
    }
    Some(format!("{} from {}", cols.join(", "), table_name))
}

/// Rewrite `fn(...) = <literal>` to `<literal> = fn(...)` for compatibility.
/// This allows WHERE clauses like `ip4_between(x, a, b) = 1` to be rewritten
/// as `1 = ip4_between(x, a, b)`.
fn rewrite_lhs_fn_eq_literal(s: &str) -> Option<String> {
    let t = s.trim();
    let bytes = t.as_bytes();
    // quick check: starts with ident and '('
    let mut i = 0usize;
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    if i >= bytes.len() || !bytes[i].is_ascii_alphabetic() {
        return None;
    }
    while i < bytes.len()
        && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'.')
    {
        i += 1;
    }
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    if i >= bytes.len() || bytes[i] != b'(' {
        return None;
    }
    // find matching ')'
    let mut depth = 0i32;
    let mut j = i;
    while j < bytes.len() {
        let c = bytes[j];
        if c == b'(' {
            depth += 1;
        } else if c == b')' {
            depth -= 1;
            if depth == 0 {
                break;
            }
        }
        j += 1;
    }
    if j >= bytes.len() || depth != 0 {
        return None;
    }
    // remaining: ") ..."
    let mut k = j + 1;
    while k < bytes.len() && (bytes[k] == b' ' || bytes[k] == b'\t') {
        k += 1;
    }
    if k >= bytes.len() || bytes[k] != b'=' {
        return None;
    }
    k += 1; // skip '='
    while k < bytes.len() && (bytes[k] == b' ' || bytes[k] == b'\t') {
        k += 1;
    }
    let rhs = t[k..].trim();
    if rhs.is_empty() {
        return None;
    }
    let lhs = t[..=j].trim();
    Some(format!("{} = {}", rhs, lhs))
}

/// Convert a SQL piece, mapping `read(arg)` to `:arg` and collecting params.
fn to_sql_piece(s: &str, params: &mut HashMap<String, CondAccessor>) -> String {
    let st = s.trim();
    // SQL 条件里的 @ref 视为 read(ref)，统一编译成命名参数 :ref。
    if let Some(var) = st.strip_prefix('@')
        && is_sql_ident(var)
    {
        params.insert(var.to_string(), CondAccessor::from_read(var.to_string()));
        return format!(":{}", var);
    }
    if let Some(rest) = st.strip_prefix("read(")
        && let Some(rest2) = rest.strip_suffix(")")
    {
        let var = rest2.trim();
        params.insert(var.to_string(), CondAccessor::from_read(var.to_string()));
        return format!(":{}", var);
    }
    if st.starts_with("__") && is_sql_ident(st) {
        params.insert(st.to_string(), CondAccessor::from_read(st.to_string()));
        return format!(":{}", st);
    }
    if let Some(rest) = st.strip_prefix("take(")
        && let Some(rest2) = rest.strip_suffix(")")
    {
        let var = rest2.trim();
        if is_sql_ident(var) {
            params.insert(
                var.to_string(),
                CondAccessor::Tdc(RecordOperation::new(DirectAccessor::Take(FieldTake {
                    get: Some(var.to_string()),
                    option: Vec::new(),
                    collect: Vec::new(),
                    default_acq: None,
                }))),
            );
            return format!(":{}", var);
        }
    }
    st.to_string()
}

fn is_sql_literal_piece(s: &str) -> bool {
    let st = s.trim();
    if st.is_empty() {
        return false;
    }
    if (st.starts_with('\'') && st.ends_with('\'')) || (st.starts_with('"') && st.ends_with('"')) {
        return st.len() >= 2;
    }
    st.parse::<i64>().is_ok() || st.parse::<f64>().is_ok()
}

fn sql_string_literal(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

fn take_ident(input: &str) -> Option<(&str, &str)> {
    let end = input
        .char_indices()
        .take_while(|(_, ch)| ch.is_ascii_alphanumeric() || *ch == '_')
        .map(|(idx, ch)| idx + ch.len_utf8())
        .last()
        .unwrap_or(0);
    if end == 0 {
        None
    } else {
        Some((&input[..end], &input[end..]))
    }
}

fn render_like_interpolated_literal(
    rhs: &str,
    params: &mut HashMap<String, CondAccessor>,
) -> Option<String> {
    let rhs = rhs.trim();
    let quote = rhs.as_bytes().first().copied()?;
    if !matches!(quote, b'\'' | b'"') || rhs.as_bytes().last().copied()? != quote {
        return None;
    }
    let content = &rhs[1..rhs.len() - 1];
    let mut rest = content;
    let mut literal = String::new();
    let mut parts = Vec::new();
    let mut has_dynamic = false;

    while !rest.is_empty() {
        if let Some(after_at) = rest.strip_prefix('@')
            && let Some((name, tail)) = take_ident(after_at)
            && is_sql_ident(name)
        {
            if !literal.is_empty() {
                parts.push(sql_string_literal(&literal));
                literal.clear();
            }
            params.insert(name.to_string(), CondAccessor::from_read(name.to_string()));
            parts.push(format!(":{}", name));
            rest = tail;
            has_dynamic = true;
            continue;
        }

        let mut consumed_dynamic = false;
        for prefix in ["read(", "take("] {
            if let Some(after_prefix) = rest.strip_prefix(prefix)
                && let Some(close_pos) = after_prefix.find(')')
            {
                let name = after_prefix[..close_pos].trim();
                if is_sql_ident(name) {
                    if !literal.is_empty() {
                        parts.push(sql_string_literal(&literal));
                        literal.clear();
                    }
                    let token = format!("{}{})", prefix, name);
                    let sql_piece = to_sql_piece(&token, params);
                    parts.push(sql_piece);
                    rest = &after_prefix[close_pos + 1..];
                    has_dynamic = true;
                    consumed_dynamic = true;
                    break;
                }
            }
        }
        if consumed_dynamic {
            continue;
        }

        let ch = rest.chars().next()?;
        literal.push(ch);
        rest = &rest[ch.len_utf8()..];
    }

    if !has_dynamic {
        return None;
    }
    if !literal.is_empty() {
        parts.push(sql_string_literal(&literal));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" || "))
    }
}

// 在最外层扫描 SQL 关键字 `in`，允许后面直接跟 `(`，也允许写成 `in (`。
// 这里故意只识别顶层位置，避免误命中引号或括号内部的内容。
fn find_top_level_in_keyword(input: &str) -> Option<usize> {
    let bytes = input.as_bytes();
    let mut depth = 0i32;
    let mut in_single = false;
    let mut in_double = false;
    let mut idx = 0usize;

    while idx < bytes.len() {
        let ch = bytes[idx] as char;
        match ch {
            '\'' if !in_double => {
                in_single = !in_single;
                idx += 1;
                continue;
            }
            '"' if !in_single => {
                in_double = !in_double;
                idx += 1;
                continue;
            }
            '(' if !in_single && !in_double => {
                depth += 1;
                idx += 1;
                continue;
            }
            ')' if !in_single && !in_double => {
                depth -= 1;
                if depth < 0 {
                    return None;
                }
                idx += 1;
                continue;
            }
            _ => {}
        }

        if depth == 0 && !in_single && !in_double && idx + 1 < bytes.len() {
            let c1 = bytes[idx].to_ascii_lowercase();
            let c2 = bytes[idx + 1].to_ascii_lowercase();
            if c1 == b'i' && c2 == b'n' {
                let prev_ok = idx > 0
                    && ((bytes[idx - 1] as char).is_ascii_whitespace() || bytes[idx - 1] == b')');
                if !prev_ok {
                    idx += 1;
                    continue;
                }

                let mut next = idx + 2;
                while next < bytes.len() && (bytes[next] as char).is_ascii_whitespace() {
                    next += 1;
                }
                if next < bytes.len() && bytes[next] == b'(' {
                    return Some(idx);
                }
            }
        }

        idx += 1;
    }

    None
}

fn fast_path_in_list(s: &str) -> Option<(String, HashMap<String, CondAccessor>)> {
    // 针对 `field in (...)` 的轻量 fast path。
    // 当前只支持简单列表项（@ref/read/字面量），未命中时回退到通用条件解析。
    let t = s.trim();
    let in_pos = find_top_level_in_keyword(t)?;
    let lhs = t[..in_pos].trim();
    if !is_sql_ident(lhs) {
        return None;
    }
    let rhs = t[in_pos + 2..].trim();
    let inner = rhs.strip_prefix('(')?.strip_suffix(')')?.trim();
    let parts = split_top_level_commas(inner)?;
    if parts.is_empty() {
        return None;
    }

    let mut params = HashMap::new();
    let mut items = Vec::with_capacity(parts.len());
    for part in parts {
        let sql_piece = to_sql_piece(&part, &mut params);
        let trimmed = sql_piece.trim();
        if trimmed.starts_with(':') || is_sql_literal_piece(trimmed) {
            items.push(trimmed.to_string());
            continue;
        }
        return None;
    }

    Some((format!("{} IN ({})", lhs, items.join(", ")), params))
}

fn find_top_level_like_keyword(input: &str) -> Option<usize> {
    let bytes = input.as_bytes();
    let mut depth = 0i32;
    let mut in_single = false;
    let mut in_double = false;
    let mut idx = 0usize;

    while idx < bytes.len() {
        let ch = bytes[idx] as char;
        match ch {
            '\'' if !in_double => {
                in_single = !in_single;
                idx += 1;
                continue;
            }
            '"' if !in_single => {
                in_double = !in_double;
                idx += 1;
                continue;
            }
            '(' if !in_single && !in_double => {
                depth += 1;
                idx += 1;
                continue;
            }
            ')' if !in_single && !in_double => {
                depth -= 1;
                if depth < 0 {
                    return None;
                }
                idx += 1;
                continue;
            }
            _ => {}
        }

        if depth == 0 && !in_single && !in_double && idx + 4 <= bytes.len() {
            let is_like = bytes[idx..idx + 4].eq_ignore_ascii_case(b"like");
            let prev_ok = idx
                .checked_sub(1)
                .is_some_and(|prev| (bytes[prev] as char).is_ascii_whitespace());
            let next_ok = bytes
                .get(idx + 4)
                .is_some_and(|next| (*next as char).is_ascii_whitespace());
            if is_like && prev_ok && next_ok {
                return Some(idx);
            }
        }

        idx += 1;
    }

    None
}

fn fast_path_like(s: &str) -> Option<(String, HashMap<String, CondAccessor>)> {
    let t = s.trim();
    let like_pos = find_top_level_like_keyword(t)?;
    let lhs = t[..like_pos].trim();
    if !is_sql_ident(lhs) {
        return None;
    }
    let rhs = t[like_pos + 4..].trim();
    if rhs.is_empty() {
        return None;
    }

    let mut params = HashMap::new();
    if let Some(rhs_sql) = render_like_interpolated_literal(rhs, &mut params) {
        return Some((format!("{} LIKE {}", lhs, rhs_sql), params));
    }

    let rhs_sql = to_sql_piece(rhs, &mut params);
    let rhs_trim = rhs_sql.trim();
    if rhs_trim.starts_with(':') || is_sql_literal_piece(rhs_trim) {
        return Some((format!("{} LIKE {}", lhs, rhs_trim), params));
    }
    None
}

/// Fast path for `1 = ip4_between(read(x), a, b)` pattern.
/// Converts to range comparison without going through the generic cond parser.
fn fast_path_ip4_between_eq_one(s: &str) -> Option<(String, HashMap<String, CondAccessor>)> {
    let t = s.trim();
    let t = t.strip_prefix("1=").or_else(|| t.strip_prefix("1 ="))?;
    let t = t.trim_start();
    if !t.starts_with("ip4_between(") {
        return None;
    }
    let inside = t.strip_prefix("ip4_between(")?;
    let inside = inside.strip_suffix(")")?;
    let parts: Vec<&str> = inside.split(',').map(|x| x.trim()).collect();
    if parts.len() != 3 {
        return None;
    }
    let mut params: HashMap<String, CondAccessor> = HashMap::new();
    let p1 = to_sql_piece(parts[0], &mut params);
    let p2 = to_sql_piece(parts[1], &mut params);
    let p3 = to_sql_piece(parts[2], &mut params);
    // Prefer using range compare to avoid dependency on ip4_between UDF semantics
    let where_sql = format!("{} <= ip4_int({}) and {} >= ip4_int({})", p2, p1, p3, p1);
    Some((where_sql, params))
}

pub fn oml_sql(data: &mut &str) -> WResult<SqlQuery> {
    // Parse `select <body> where <cond>;`
    // We sanitize `<body>` to avoid unsafe identifiers: only [A-Za-z0-9_.] and '*' are allowed
    // and we split `cols from table`. If sanitize fails, we fall back to original body to keep
    // backward compatibility (recommended to provide whitelisted identifiers at source).
    kw_sql_select.parse_next(data)?;
    let sql_body = take_until(0.., "where")
        .context(ctx_desc("end to 'where'"))
        .parse_next(data)?;
    kw_sql_where.parse_next(data)?;
    let sql_cond_raw = take_until(0.., ";").parse_next(data)?;

    // Rewrite `fn(...) = <literal>` to `<literal> = fn(...)` for compatibility
    let sql_cond_buf: String =
        rewrite_lhs_fn_eq_literal(sql_cond_raw).unwrap_or_else(|| sql_cond_raw.to_string());

    // 优先处理 ip4_between(...)=1 这类 SQL 专用条件，避免落回通用条件解析。
    if let Some((w_sql, vars)) = fast_path_ip4_between_eq_one(&sql_cond_buf) {
        let sql = format!("select {} where {}", sql_body, w_sql);
        let route = resolve_sql_route(&sql);
        return Ok(SqlQuery::new_with_route(sql, vars, route));
    }

    // 优先处理 field in (...)，支持 @ref 形式的 OML 变量引用。
    if let Some((w_sql, vars)) = fast_path_in_list(&sql_cond_buf) {
        let sql = format!("select {} where {}", sql_body, w_sql);
        let route = resolve_sql_route(&sql);
        return Ok(SqlQuery::new_with_route(sql, vars, route));
    }

    // 处理 field LIKE <literal|@ref|read(...)|take(...)>，避免 LIKE 落回通用条件解析。
    if let Some((w_sql, vars)) = fast_path_like(&sql_cond_buf) {
        let sql = format!("select {} where {}", sql_body, w_sql);
        let route = resolve_sql_route(&sql);
        return Ok(SqlQuery::new_with_route(sql, vars, route));
    }

    // Generic path
    let mut sql_cond = sql_cond_buf.as_str();
    let cond = SCondParser::end_exp(&mut sql_cond, ";")?;
    let (w_sql, vars) = cond.args_take();

    // Strict mode: reject invalid body; compat mode: fallback to original
    let strict = is_sql_strict();
    let safe_body = match sanitize_sql_body(sql_body) {
        Some(b) => b,
        None if strict => {
            return fail
                .context(StrContext::Label("sql body"))
                .context(ctx_desc("expected `<cols from table>`"))
                .context(ctx_desc("cols in [A-Za-z0-9_.] or '*'"))
                .context(ctx_desc("table in [A-Za-z0-9_.]"))
                .parse_next(data);
        }
        None => sql_body.to_string(),
    };

    let sql = format!("select {} where {}", safe_body, w_sql);
    let route = resolve_sql_route(&sql);
    Ok(SqlQuery::new_with_route(sql, vars, route))
}

pub fn oml_aga_sql(data: &mut &str) -> WResult<PreciseEvaluator> {
    Ok(PreciseEvaluator::Sql(oml_sql.parse_next(data)?))
}
#[cfg(test)]
mod tests {
    use wp_knowledge::cache::FieldQueryCache;
    use wp_model_core::model::{DataField, DataRecord, FieldStorage};
    use wp_primitives::WResult as ModalResult;

    use crate::parser::utils::for_test::assert_oml_parse;
    use crate::parser::{sql_prm::oml_sql, utils::for_test::err_of_oml};
    use winnow::Parser;
    use wp_error::parse_error::OMLCodeReason;

    #[tokio::test(flavor = "current_thread")]
    async fn test_oml_sql() -> ModalResult<()> {
        super::set_sql_strict_for_test(Some(true));
        let mut code = r#" select a, b from table_1 where x = read (src);"#;
        assert_oml_parse(&mut code, oml_sql);

        let mut code = r#" select a, b from table_1 where x = take (src);"#;
        assert_oml_parse(&mut code, oml_sql);

        let mut code = r#" select a, b from table_1 where x = Now::time() ;"#;
        assert_oml_parse(&mut code, oml_sql);

        let mut code = r#" select a, b from table_1 where x = 1 ;"#;
        assert_oml_parse(&mut code, oml_sql);

        let mut code = r#" select a, b from table_1 where x = 'china' ;"#;
        assert_oml_parse(&mut code, oml_sql);

        let mut code = r#"select name,pinying from example where pinying = 'xiaolongnu' ;"#;
        assert_oml_parse(&mut code, oml_sql);
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_oml_sql_multi_provider_dotted_table() -> ModalResult<()> {
        super::set_sql_strict_for_test(Some(true));
        // 多 provider 场景：`<provider>.public.<table>` 三段表引用在 strict 模式下可解析
        let mut code = r#" select asset_name, asset_env, asset_owner from asset.public.asset_inventory where ip = read(sip) ;"#;
        let parsed = oml_sql.parse_next(&mut code)?;
        assert!(parsed.oml_sql().contains("asset.public.asset_inventory"));

        let mut code =
            r#" select country, city from geo.public.ip_geo_city where ip = read(sip) ;"#;
        let parsed = oml_sql.parse_next(&mut code)?;
        assert!(parsed.oml_sql().contains("geo.public.ip_geo_city"));
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_oml_sql2() -> ModalResult<()> {
        super::set_sql_strict_for_test(Some(true));
        let mut code = r#" select a, b from table_1 where x = Now::time() and y = read(src) ;"#;
        assert_oml_parse(&mut code, oml_sql);

        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_oml_sql_group_concat_in_ref() -> ModalResult<()> {
        super::set_sql_strict_for_test(Some(true));
        let mut code = r#" select group_concat(distinct asset_type) from asset_enrichment where ip in (@sip, @dip) ;"#;
        let parsed = oml_sql.parse_next(&mut code)?;
        assert_eq!(
            parsed
                .oml_sql()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
            "select group_concat(distinct asset_type) from asset_enrichment where ip IN (:sip, :dip)"
        );
        assert!(parsed.vars().contains_key("sip"));
        assert!(parsed.vars().contains_key("dip"));
        assert!(matches!(
            parsed.route(),
            crate::language::SqlKnowledgeRoute::Provider
                | crate::language::SqlKnowledgeRoute::Sqlite
                | crate::language::SqlKnowledgeRoute::Unknown
        ));
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_oml_sql_group_concat_in_ref_without_space_before_paren() -> ModalResult<()> {
        super::set_sql_strict_for_test(Some(true));
        let mut code = r#" select group_concat(distinct asset_type) from asset_enrichment where ip in(@sip, @dip) ;"#;
        let parsed = oml_sql.parse_next(&mut code)?;
        assert_eq!(
            parsed
                .oml_sql()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
            "select group_concat(distinct asset_type) from asset_enrichment where ip IN (:sip, :dip)"
        );
        assert!(parsed.vars().contains_key("sip"));
        assert!(parsed.vars().contains_key("dip"));
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_oml_sql_like_params() -> ModalResult<()> {
        super::set_sql_strict_for_test(Some(true));
        let cases = [
            (
                r#" select group_concat(distinct asset) from asset_tags where owner like '%raed(team)' ;"#,
                "select group_concat(distinct asset) from asset_tags where owner LIKE '%raed(team)'",
                None,
            ),
            (
                r#" select group_concat(distinct asset) from asset_tags where owner like @team ;"#,
                "select group_concat(distinct asset) from asset_tags where owner LIKE :team",
                Some("team"),
            ),
            (
                r#" select group_concat(distinct asset) from asset_tags where owner like read(team) ;"#,
                "select group_concat(distinct asset) from asset_tags where owner LIKE :team",
                Some("team"),
            ),
            (
                r#" select group_concat(distinct asset) from asset_tags where owner like take(team) ;"#,
                "select group_concat(distinct asset) from asset_tags where owner LIKE :team",
                Some("team"),
            ),
            (
                r#" select group_concat(distinct asset) from asset_tags where owner like '%read(team)' ;"#,
                "select group_concat(distinct asset) from asset_tags where owner LIKE '%' || :team",
                Some("team"),
            ),
            (
                r#" select group_concat(distinct asset) from asset_tags where owner like '%@team%' ;"#,
                "select group_concat(distinct asset) from asset_tags where owner LIKE '%' || :team || '%'",
                Some("team"),
            ),
            (
                r#" select group_concat(distinct asset) from asset_tags where owner like 'take(team)%' ;"#,
                "select group_concat(distinct asset) from asset_tags where owner LIKE :team || '%'",
                Some("team"),
            ),
        ];

        for (mut code, expected, param) in cases {
            let parsed = oml_sql.parse_next(&mut code)?;
            assert_eq!(
                parsed
                    .oml_sql()
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" "),
                expected
            );
            if let Some(param) = param {
                assert!(parsed.vars().contains_key(param));
            } else {
                assert!(parsed.vars().is_empty());
            }
        }
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_oml_sql_group_concat_in_take_refs() -> ModalResult<()> {
        super::set_sql_strict_for_test(Some(true));
        let mut code = r#" select group_concat(distinct asset_type) from asset_enrichment where ip in (take(src_ip), take(dst_ip)) ;"#;
        let parsed = oml_sql.parse_next(&mut code)?;
        assert_eq!(
            parsed
                .oml_sql()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
            "select group_concat(distinct asset_type) from asset_enrichment where ip IN (:src_ip, :dst_ip)"
        );
        assert!(parsed.vars().contains_key("src_ip"));
        assert!(parsed.vars().contains_key("dst_ip"));
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_oml_sql_bare_temp_var_eq_ref() -> ModalResult<()> {
        super::set_sql_strict_for_test(Some(true));
        let mut code = r#" select asset from asset_data where ip = __tsip ;"#;
        let parsed = oml_sql.parse_next(&mut code)?;
        assert_eq!(
            parsed
                .oml_sql()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
            "select asset from asset_data where ip = :ip"
        );
        assert!(parsed.vars().contains_key("ip"));
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_oml_sql_group_concat_in_bare_temp_refs() -> ModalResult<()> {
        super::set_sql_strict_for_test(Some(true));
        let mut code = r#" select group_concat(distinct asset_type) from asset_enrichment where ip in (__sip, __dip) ;"#;
        let parsed = oml_sql.parse_next(&mut code)?;
        assert_eq!(
            parsed
                .oml_sql()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
            "select group_concat(distinct asset_type) from asset_enrichment where ip IN (:__sip, :__dip)"
        );
        assert!(parsed.vars().contains_key("__sip"));
        assert!(parsed.vars().contains_key("__dip"));
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_oml_sql_string_agg_in_ref() -> ModalResult<()> {
        super::set_sql_strict_for_test(Some(true));
        let mut code = r#" select string_agg(distinct asset_type, ',') from v_asset_ip_details where ip in (@sip, @dip) ;"#;
        let parsed = oml_sql.parse_next(&mut code)?;
        assert_eq!(
            parsed
                .oml_sql()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
            "select string_agg(distinct asset_type, ',') from v_asset_ip_details where ip IN (:sip, :dip)"
        );
        assert!(parsed.vars().contains_key("sip"));
        assert!(parsed.vars().contains_key("dip"));
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_oml_sql_string_agg_in_take_refs() -> ModalResult<()> {
        super::set_sql_strict_for_test(Some(true));
        let mut code = r#" select string_agg(distinct asset_type, ',') from v_asset_ip_details where ip in (take(src_ip), take(dst_ip)) ;"#;
        let parsed = oml_sql.parse_next(&mut code)?;
        assert_eq!(
            parsed
                .oml_sql()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
            "select string_agg(distinct asset_type, ',') from v_asset_ip_details where ip IN (:src_ip, :dst_ip)"
        );
        assert!(parsed.vars().contains_key("src_ip"));
        assert!(parsed.vars().contains_key("dst_ip"));
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_oml_sql_string_agg_eq_bare_temp_ref() -> ModalResult<()> {
        super::set_sql_strict_for_test(Some(true));
        let mut code = r#" select string_agg(distinct asset_type, ',') from v_asset_ip_details where ip = __tsip ;"#;
        let parsed = oml_sql.parse_next(&mut code)?;
        assert_eq!(
            parsed
                .oml_sql()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
            "select string_agg(distinct asset_type, ',') from v_asset_ip_details where ip = :ip"
        );
        assert!(parsed.vars().contains_key("ip"));
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_oml_sql_subquery_body_ok() -> ModalResult<()> {
        super::set_sql_strict_for_test(Some(true));
        let mut code = r#" select asset from (select * from asset_data) where ip = @src_ip ;"#;
        let parsed = oml_sql.parse_next(&mut code)?;
        assert_eq!(
            parsed
                .oml_sql()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
            "select asset from (select * from asset_data) where ip = :ip"
        );
        assert!(parsed.vars().contains_key("ip"));
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_oml_sql_subquery_body_with_alias_ok() -> ModalResult<()> {
        super::set_sql_strict_for_test(Some(true));
        let mut code =
            r#" select asset from (select * from asset_data) as hits where ip = @src_ip ;"#;
        let parsed = oml_sql.parse_next(&mut code)?;
        assert_eq!(
            parsed
                .oml_sql()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
            "select asset from (select * from asset_data) hits where ip = :ip"
        );
        assert!(parsed.vars().contains_key("ip"));
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_oml_sql_strict_err() {
        super::set_sql_strict_for_test(Some(true));
        let code = r#" select a, b from table-1 where x = read(src) ;"#;
        let err = oml_sql.parse(code).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("sql body"));
        assert!(msg.contains("expected `"));
        super::set_sql_strict_for_test(None);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_oml_sql_compat_ok() -> ModalResult<()> {
        // 双保险：覆盖为兼容模式，且写入 env 变量
        super::set_sql_strict_for_test(Some(false));
        unsafe {
            std::env::set_var("OML_SQL_STRICT", "0");
        }
        let mut code = r#" select a, b from table-1 where x = read(src) ;"#;
        assert_oml_parse(&mut code, oml_sql);
        super::set_sql_strict_for_test(None);
        unsafe {
            std::env::remove_var("OML_SQL_STRICT");
        }
        Ok(())
    }

    use crate::core::AsyncDataTransformer;
    use crate::parser::oml_parse_raw;
    use orion_error::dev::testing::TestAssert;
    use wp_know::mem::memdb::MemDB;
    use wp_knowledge::facade as kdb;
    #[tokio::test(flavor = "current_thread")]
    async fn test_sql_udf_ip4_between_exec() -> ModalResult<()> {
        // 1) init in-memory provider and prepare table with an IPv4 range
        let db = MemDB::global();
        db.table_create(
            "CREATE TABLE IF NOT EXISTS zone (zone TEXT, ip_start_int INTEGER, ip_end_int INTEGER)",
        )
        .assert();
        // 10.0.0.0 - 10.255.255.255 => [167772160, 184549375]
        db.execute(
            "INSERT INTO zone (zone, ip_start_int, ip_end_int) VALUES ('A', 167772160, 184549375)",
        )
        .assert();
        let _ = kdb::init_mem_provider(db);

        // 2) build OML with UDF in WHERE
        let mut conf = r#"
name : test
---
zone : chars = select zone from zone where ip_start_int <= ip4_int(read(src_ip)) and ip_end_int >= ip4_int(read(src_ip)) ;
        "#;
        let model = oml_parse_raw(&mut conf).await.assert();

        // 3) transform with src_ip within range
        let src = DataRecord::from(vec![FieldStorage::from_owned(DataField::from_chars(
            "src_ip", "10.1.2.3",
        ))]);
        let cache = &mut FieldQueryCache::default();
        let out = model.transform_async(src, cache).await;
        use wp_model_core::model::Value;
        let zone = out.get2("zone").and_then(|f| match f.get_value() {
            Value::Chars(s) => Some(s.as_str()),
            _ => None,
        });
        assert_eq!(zone, Some("A"));
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_sql_udf_ip4_between_exec_async() -> ModalResult<()> {
        let db = MemDB::global();
        db.table_create(
            "CREATE TABLE IF NOT EXISTS zone (zone TEXT, ip_start_int INTEGER, ip_end_int INTEGER)",
        )
        .assert();
        db.execute("DELETE FROM zone").assert();
        db.execute(
            "INSERT INTO zone (zone, ip_start_int, ip_end_int) VALUES ('A', 167772160, 184549375)",
        )
        .assert();
        let _ = kdb::init_mem_provider(db);

        let mut conf = r#"
name : test
---
zone : chars = select zone from zone where ip_start_int <= ip4_int(read(src_ip)) and ip_end_int >= ip4_int(read(src_ip)) ;
        "#;
        let model = oml_parse_raw(&mut conf).await.assert();

        let src = DataRecord::from(vec![FieldStorage::from_owned(DataField::from_chars(
            "src_ip", "10.1.2.3",
        ))]);
        let cache = &mut FieldQueryCache::default();
        let out = model.transform_async(src, cache).await;
        use wp_model_core::model::Value;
        let zone = out.get2("zone").and_then(|f| match f.get_value() {
            Value::Chars(s) => Some(s.as_str()),
            _ => None,
        });
        assert_eq!(zone, Some("A"));
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_sql_aggregate_null_is_not_emitted() -> ModalResult<()> {
        let db = MemDB::global();
        db.table_create("CREATE TABLE IF NOT EXISTS asset_data (asset TEXT, ip TEXT)")
            .assert();
        db.execute("DELETE FROM asset_data").assert();
        db.execute("INSERT INTO asset_data (asset, ip) VALUES ('host-a', '10.0.0.1')")
            .assert();
        let _ = kdb::init_mem_provider(db);

        let mut conf = r#"
name : test
---
test_list : chars = select group_concat(distinct asset) from asset_data where ip in(@sip, @dip) ;
        "#;
        let model = oml_parse_raw(&mut conf).await.assert();

        let src = DataRecord::from(vec![
            FieldStorage::from_owned(DataField::from_chars("sip", "192.0.2.10")),
            FieldStorage::from_owned(DataField::from_chars("dip", "192.0.2.11")),
        ]);
        let cache = &mut FieldQueryCache::default();
        let out = model.transform_async(src, cache).await;

        assert!(out.get2("test_list").is_none(), "out={out:?}");
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_object_async_uses_nested_accessors() -> ModalResult<()> {
        let mut conf = r#"
name : test
---
static {
    tpl = object {
        zone = chars(A);
    };
}

payload = object {
    clone = tpl;
    source = read(src_ip);
};
        "#;
        let model = oml_parse_raw(&mut conf).await.assert();

        let src = DataRecord::from(vec![FieldStorage::from_owned(DataField::from_chars(
            "src_ip", "10.1.2.3",
        ))]);
        let cache = &mut FieldQueryCache::default();
        let out = model.transform_async(src, cache).await;
        use wp_model_core::model::Value;
        let payload = out.get2("payload").expect("payload field");
        let Value::Obj(obj) = payload.get_value() else {
            panic!("payload should be object");
        };
        let clone = obj.get("clone").expect("clone field");
        let source = obj.get("source").expect("source field");
        let Value::Obj(clone_obj) = clone.as_field().get_value() else {
            panic!("clone should be object");
        };
        let zone = clone_obj.get("zone").expect("zone field");
        assert_eq!(zone.as_field().get_value(), &Value::Chars("A".into()));
        assert_eq!(
            source.as_field().get_value(),
            &Value::Chars("10.1.2.3".into())
        );
        Ok(())
    }
    #[tokio::test(flavor = "current_thread")]
    async fn test_sql_oml_err() {
        let mut code = r#" selec a, b from table_1 where x = read (src);"#;
        let e = err_of_oml(&mut code, oml_sql);
        println!("err:{}, \nwhere:{}", e, code);
        assert!(
            matches!(e.reason(), OMLCodeReason::Syntax(s) if s.contains("need 'select' keyword"))
        );

        let mut code = r#" select a, b from table_1 whare x = read (src);"#;
        let e = err_of_oml(&mut code, oml_sql);
        println!("err:{}, \nwhere:{}", e, code);
        assert!(matches!(e.reason(), OMLCodeReason::Syntax(s) if s.contains("end to 'where'")));

        let mut code = r#" select a, b from table_1 where x = src;"#;
        let e = err_of_oml(&mut code, oml_sql);
        println!("err:{}, \nwhere:{}", e, code);
    }
}
