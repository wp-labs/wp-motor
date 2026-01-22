//! SQLite 扩展：在 rusqlite 连接上注册内置 UDF，供 KnowDB 导入/查询使用。
//!
//! 说明：
//! - 每个新建的 Connection 都需要注册一次（权威库写连接、只读线程克隆连接分别注册）。
//! - 本模块仅包含轻量、与 IP/CIDR 相关的函数；字符串类函数请优先使用 SQLite 内置的 lower/upper/trim。

use rusqlite::Result as SqlResult;
use rusqlite::functions::{Context, FunctionFlags};

/// 注册内置 UDF 到给定连接。
/// 注意：需在每个新建的 Connection 上调用一次（writer/reader 各自注册）。
/// 在给定连接上注册内置函数集合。
pub fn register_builtin(conn: &rusqlite::Connection) -> SqlResult<()> {
    // ip4_int(text) -> integer（u32 转 i64）
    conn.create_scalar_function(
        "ip4_int",
        1,
        FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx: &Context| {
            let s: String = ctx.get(0)?;
            Ok(parse_ipv4_to_u32(&s).map(|v| v as i64).unwrap_or(0))
        },
    )?;

    // cidr4_min(text) -> integer（CIDR 起始地址）
    conn.create_scalar_function(
        "cidr4_min",
        1,
        FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx: &Context| {
            let s: String = ctx.get(0)?;
            Ok(cidr4_min(&s).map(|v| v as i64).unwrap_or(0))
        },
    )?;

    // cidr4_max(text) -> integer（CIDR 结束地址，含）
    conn.create_scalar_function(
        "cidr4_max",
        1,
        FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx: &Context| {
            let s: String = ctx.get(0)?;
            Ok(cidr4_max(&s).map(|v| v as i64).unwrap_or(0))
        },
    )?;

    // ip4_between(ip, start, end) -> integer (1/0)
    conn.create_scalar_function(
        "ip4_between",
        3,
        FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx: &Context| {
            let ip: String = ctx.get(0)?;
            // 参数 1/2 既可能是整数列（*_int），也可能是字符串；两种都尝试
            let s = ctx
                .get::<i64>(1)
                .ok()
                .map(|i| i as u32)
                .or_else(|| {
                    let s: String = ctx.get(1).ok()?;
                    parse_ipv4_to_u32(&s)
                })
                .unwrap_or(u32::MAX);
            let e = ctx
                .get::<i64>(2)
                .ok()
                .map(|i| i as u32)
                .or_else(|| {
                    let s: String = ctx.get(2).ok()?;
                    parse_ipv4_to_u32(&s)
                })
                .unwrap_or(0);
            let v = parse_ipv4_to_u32(&ip).unwrap_or(u32::MAX);
            Ok(((v >= s) && (v <= e)) as i64)
        },
    )?;

    // cidr4_contains(ip, cidr) -> integer (1/0)
    conn.create_scalar_function(
        "cidr4_contains",
        2,
        FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx: &Context| {
            let ip_s: String = ctx.get(0)?;
            let cidr_s: String = ctx.get(1)?;
            let ip = match parse_ipv4_to_u32(&ip_s) {
                Some(v) => v,
                None => return Ok(0),
            };
            let (net_ip, mask) = match parse_cidr4(&cidr_s) {
                Some(v) => v,
                None => return Ok(0),
            };
            let net = u32::from(net_ip) & mask;
            Ok(((ip & mask) == net) as i64)
        },
    )?;

    // ip4_text(integer) -> text
    conn.create_scalar_function(
        "ip4_text",
        1,
        FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx: &Context| {
            // 同时支持整型或可解析为整型的字符串
            let val = ctx.get::<i64>(0).ok();
            let v = if let Some(i) = val {
                i as u32
            } else {
                let s: String = ctx.get(0)?;
                match s.trim().parse::<u64>() {
                    Ok(n) => n as u32,
                    Err(_) => 0,
                }
            };
            Ok(ipv4_from_u32(v))
        },
    )?;

    // trim_quotes(text) -> text：去除两端成对引号（支持 ' 或 "），容忍前后空白
    conn.create_scalar_function(
        "trim_quotes",
        1,
        FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx: &Context| {
            let s: String = ctx.get(0)?;
            Ok(trim_quotes(&s))
        },
    )?;
    Ok(())
}

/// 解析点分 IPv4 为 u32；容忍前后空白与引号。
fn parse_ipv4_to_u32(s: &str) -> Option<u32> {
    // 允许带空白/引号
    let t = s.trim().trim_matches('"');
    let ip: std::net::Ipv4Addr = t.parse().ok()?;
    Some(u32::from(ip))
}

fn cidr4_min(s: &str) -> Option<u32> {
    let (ip, mask) = parse_cidr4(s)?;
    Some(u32::from(ip) & mask)
}

fn cidr4_max(s: &str) -> Option<u32> {
    let (ip, mask) = parse_cidr4(s)?;
    Some((u32::from(ip) & mask) | !mask)
}

fn parse_cidr4(s: &str) -> Option<(std::net::Ipv4Addr, u32)> {
    let t = s.trim().trim_matches('"');
    let mut it = t.split('/');
    let ip_s = it.next()?;
    let pfx_s = it.next()?;
    if it.next().is_some() {
        return None;
    }
    let ip: std::net::Ipv4Addr = ip_s.parse().ok()?;
    let pfx: u32 = pfx_s.parse().ok()?;
    if pfx > 32 {
        return None;
    }
    let mask = if pfx == 0 { 0 } else { u32::MAX << (32 - pfx) };
    Some((ip, mask))
}

/// 将整数 IPv4 转为点分字符串。
fn ipv4_from_u32(v: u32) -> String {
    let ip = std::net::Ipv4Addr::from(v);
    ip.to_string()
}

/// 去除两端成对引号（' 或 "),先 trim 再判断；未成对则返回 trim 后原串
fn trim_quotes(s: &str) -> String {
    let t = s.trim();
    if t.len() >= 2 {
        let b = t.as_bytes();
        let mut hidx = 0usize; // 参与配对判断的“头部”索引
        // 开头允许 `\"` 或 `\'`：跳过反斜杠，仅用于配对判断
        if b.len() >= 2 && b[0] == b'\\' && (b[1] == b'"' || b[1] == b'\'') {
            hidx = 1;
        }

        if b.len() >= 2 {
            let tidx = b.len() - 1; // 尾部引号所在下标（若确认为引号）
            let head = b[hidx];
            let tail = b[tidx];
            if (head == b'"' && tail == b'"') || (head == b'\'' && tail == b'\'') {
                // 生成去除头尾引号（以及尾部可能存在的反斜杠）的子串边界
                let start = hidx + 1;
                let mut end_excl = tidx; // 先排除尾部引号
                // 如果尾部是转义形式（... \\" 或 ... \\\'），也排除反斜杠
                if tidx >= 1 && b[tidx - 1] == b'\\' {
                    end_excl = tidx - 1;
                }
                if start <= end_excl {
                    return t[start..end_excl].to_string();
                }
                return String::new();
            }
        }
    }
    t.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_ip4_scalar_funcs() {
        let conn = Connection::open_in_memory().unwrap();
        register_builtin(&conn).unwrap();

        let v: i64 = conn
            .query_row("SELECT ip4_int('1.2.3.4')", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v as u32, 0x01020304);

        let s: String = conn
            .query_row("SELECT ip4_text(16909060)", [], |r| r.get(0))
            .unwrap();
        assert_eq!(s, "1.2.3.4");

        let ok: i64 = conn
            .query_row(
                "SELECT ip4_between('10.0.0.5','10.0.0.1','10.0.0.10')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(ok, 1);

        let cmin: i64 = conn
            .query_row("SELECT cidr4_min('10.0.0.0/8')", [], |r| r.get(0))
            .unwrap();
        let cmax: i64 = conn
            .query_row("SELECT cidr4_max('10.0.0.0/8')", [], |r| r.get(0))
            .unwrap();
        // 10.0.0.0/8 => 10.0.0.0 .. 10.255.255.255
        let exp_min = parse_ipv4_to_u32("10.0.0.0").unwrap() as i64;
        let exp_max = parse_ipv4_to_u32("10.255.255.255").unwrap() as i64;
        assert_eq!(cmin, exp_min);
        assert_eq!(cmax, exp_max);

        let contains: i64 = conn
            .query_row(
                "SELECT cidr4_contains('10.1.2.3','10.0.0.0/8') AS ok",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(contains, 1);

        // trim_quotes
        let z: String = conn
            .query_row("SELECT trim_quotes('  \"work_zone\"  ')", [], |r| r.get(0))
            .unwrap();
        assert_eq!(z, "work_zone");
        let z2: String = conn
            .query_row("SELECT trim_quotes('no_quotes')", [], |r| r.get(0))
            .unwrap();
        assert_eq!(z2, "no_quotes");

        // 支持反斜杠转义的成对引号
        // 注意：在 Rust 源码里书写 SQL 字符串时需要多重转义，这里改为绑定参数方式更直观
        let z3: String = conn
            .query_row("SELECT trim_quotes(?1)", ["\\\"work_zone\\\""], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(z3, "work_zone");
    }
}
