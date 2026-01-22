// Simple, dependency-light syslog header normalization

#[derive(Debug, Clone, Default)]
pub struct SyslogMeta {
    pub pri: Option<u8>,
    pub facility: Option<String>,
    pub severity: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Normalized {
    #[allow(dead_code)]
    pub header: Option<String>,
    pub message: String,
    pub meta: SyslogMeta,
}

// 零拷贝切片描述：在原始文本中的消息起止位置 + 元信息
#[derive(Debug, Clone, Default)]
pub struct NormalizedSlice {
    pub msg_start: usize,
    pub msg_end: usize,
    pub meta: SyslogMeta,
}

/// 仅计算消息在原始文本中的切片位置及元信息，避免分配
pub fn normalize_slice(line: &str) -> NormalizedSlice {
    if let Some(ns) = parse_rfc5424_slice(line) {
        return ns;
    }
    if let Some(ns) = parse_rfc3164_slice(line) {
        return ns;
    }
    NormalizedSlice {
        msg_start: 0,
        msg_end: line.len(),
        meta: Default::default(),
    }
}

pub fn normalize(line: &str) -> Normalized {
    if let Some(n) = parse_rfc5424(line) {
        return n;
    }
    if let Some(n) = parse_rfc3164(line) {
        return n;
    }
    Normalized {
        header: None,
        message: line.to_string(),
        meta: Default::default(),
    }
}

fn parse_rfc5424(input: &str) -> Option<Normalized> {
    // <PRI>VERSION SP TIMESTAMP SP HOSTNAME SP APP-NAME SP PROCID SP MSGID SP STRUCTURED-DATA [SP MSG]
    let bytes = input.as_bytes();
    if bytes.is_empty() || bytes[0] != b'<' {
        return None;
    }
    // find '>'
    let mut i = 1usize;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i >= bytes.len() || bytes[i] != b'>' {
        return None;
    }
    let pri_str = &input[1..i];
    // version: digits + space
    let mut j = i + 1;
    if j >= bytes.len() || !bytes[j].is_ascii_digit() {
        return None;
    }
    while j < bytes.len() && bytes[j].is_ascii_digit() {
        j += 1;
    }
    if j >= bytes.len() || bytes[j] != b' ' {
        return None;
    }
    j += 1; // skip space
    // Skip 5 space-separated tokens
    let mut tok = 0;
    while j < bytes.len() && tok < 5 {
        // consume token until space
        while j < bytes.len() && bytes[j] != b' ' {
            j += 1;
        }
        if j >= bytes.len() {
            return None;
        }
        // skip space
        j += 1;
        tok += 1;
    }
    if tok != 5 || j > bytes.len() {
        return None;
    }
    // Structured-data: '-' or '[' ... ']'
    if j < bytes.len() && bytes[j] == b'-' {
        j += 1;
        if j < bytes.len() && bytes[j] == b' ' {
            j += 1;
        }
        let msg = input[j..].to_string();
        let header = input[..j].trim_end().to_string();
        let meta = parse_pri_from_header(&format!("<{}>", pri_str));
        return Some(Normalized {
            header: Some(header),
            message: msg,
            meta,
        });
    }
    if j < bytes.len() && bytes[j] == b'[' {
        // find next ']'
        let rest = &input[j + 1..];
        if let Some(close) = rest.find(']') {
            j = j + 1 + close + 1; // after ']'
            if j < bytes.len() && bytes[j] == b' ' {
                j += 1;
            }
            let msg = input[j..].to_string();
            let header = input[..j].trim_end().to_string();
            let meta = parse_pri_from_header(&format!("<{}>", pri_str));
            return Some(Normalized {
                header: Some(header),
                message: msg,
                meta,
            });
        }
    }
    None
}

fn parse_rfc5424_slice(input: &str) -> Option<NormalizedSlice> {
    let bytes = input.as_bytes();
    if bytes.is_empty() || bytes[0] != b'<' {
        return None;
    }
    // PRI
    let mut i = 1usize;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i >= bytes.len() || bytes[i] != b'>' {
        return None;
    }
    let pri_str = &input[1..i];
    // VERSION SP
    let mut j = i + 1;
    if j >= bytes.len() || !bytes[j].is_ascii_digit() {
        return None;
    }
    while j < bytes.len() && bytes[j].is_ascii_digit() {
        j += 1;
    }
    if j >= bytes.len() || bytes[j] != b' ' {
        return None;
    }
    j += 1;
    // Skip 5 tokens (TIMESTAMP HOSTNAME APP PROCID MSGID)
    let mut tok = 0;
    while j < bytes.len() && tok < 5 {
        while j < bytes.len() && bytes[j] != b' ' {
            j += 1;
        }
        if j >= bytes.len() {
            return None;
        }
        j += 1;
        tok += 1;
    }
    if tok != 5 || j > bytes.len() {
        return None;
    }
    // Structured-data
    if j < bytes.len() && bytes[j] == b'-' {
        j += 1;
        if j < bytes.len() && bytes[j] == b' ' {
            j += 1;
        }
        let meta = parse_pri_from_header(&format!("<{}>", pri_str));
        return Some(NormalizedSlice {
            msg_start: j,
            msg_end: input.len(),
            meta,
        });
    }
    if j < bytes.len() && bytes[j] == b'[' {
        let rest = &input[j + 1..];
        if let Some(close) = rest.find(']') {
            j = j + 1 + close + 1; // after ']'
            if j < bytes.len() && bytes[j] == b' ' {
                j += 1;
            }
            let meta = parse_pri_from_header(&format!("<{}>", pri_str));
            return Some(NormalizedSlice {
                msg_start: j,
                msg_end: input.len(),
                meta,
            });
        }
    }
    None
}

fn parse_rfc3164(input: &str) -> Option<Normalized> {
    // <PRI>MMM SP DD SP HH:MM:SS SP HOST SP TAG ':' SP MSG
    let bytes = input.as_bytes();
    if bytes.is_empty() || bytes[0] != b'<' {
        return None;
    }
    let mut i = 1usize;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i >= bytes.len() || bytes[i] != b'>' {
        return None;
    }
    let pri_str = &input[1..i];
    i += 1; // after '>'
    // Month (3) + space
    if i + 4 > bytes.len() {
        return None;
    }
    i += 3;
    if bytes.get(i) != Some(&b' ') {
        return None;
    }
    i += 1;
    // Day digits + space
    if i >= bytes.len() || !bytes[i].is_ascii_digit() {
        return None;
    }
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i >= bytes.len() || bytes[i] != b' ' {
        return None;
    }
    i += 1;
    // Time HH:MM:SS + space
    if i + 9 > bytes.len() {
        return None;
    }
    i += 8;
    if bytes.get(i) != Some(&b' ') {
        return None;
    }
    i += 1;
    // Hostname token + space
    while i < bytes.len() && bytes[i] != b' ' {
        i += 1;
    }
    if i >= bytes.len() {
        return None;
    }
    i += 1; // skip space
    // Find ": " after tag
    if let Some(col) = input[i..].find(": ") {
        let msg_start = i + col + 2;
        let msg = input[msg_start..].to_string();
        let header = input[..msg_start].trim_end().to_string();
        let meta = parse_pri_from_header(&format!("<{}>", pri_str));
        return Some(Normalized {
            header: Some(header),
            message: msg,
            meta,
        });
    }
    None
}

fn parse_rfc3164_slice(input: &str) -> Option<NormalizedSlice> {
    let bytes = input.as_bytes();
    if bytes.is_empty() || bytes[0] != b'<' {
        return None;
    }
    let mut i = 1usize;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i >= bytes.len() || bytes[i] != b'>' {
        return None;
    }
    let pri_str = &input[1..i];
    i += 1; // after '>'
    if i + 4 > bytes.len() {
        return None;
    }
    i += 4; // Mon + space
    if i >= bytes.len() {
        return None;
    }
    if !bytes[i - 1].is_ascii() {
        return None;
    }
    if i >= bytes.len() || !bytes[i].is_ascii_digit() {
        return None;
    }
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i >= bytes.len() || bytes[i] != b' ' {
        return None;
    }
    i += 1;
    if i + 9 > bytes.len() {
        return None;
    }
    i += 8;
    if bytes.get(i) != Some(&b' ') {
        return None;
    }
    i += 1;
    while i < bytes.len() && bytes[i] != b' ' {
        i += 1;
    }
    if i >= bytes.len() {
        return None;
    }
    i += 1; // skip space
    if let Some(col) = input[i..].find(": ") {
        let msg_start = i + col + 2;
        let meta = parse_pri_from_header(&format!("<{}>", pri_str));
        return Some(NormalizedSlice {
            msg_start,
            msg_end: input.len(),
            meta,
        });
    }
    None
}

fn parse_pri_from_header(header: &str) -> SyslogMeta {
    // PRI appears as leading angle-bracket number, e.g. "<14>..."
    if let Some(end) = header.find('>')
        && header.starts_with('<')
        && let Ok(pri) = header[1..end].parse::<u16>()
    {
        let pri_u8 = (pri & 0xFF) as u8;
        let facility_code = (pri / 8) as u8;
        let severity_code = (pri % 8) as u8;
        return SyslogMeta {
            pri: Some(pri_u8),
            facility: Some(facility_name(facility_code).to_string()),
            severity: Some(severity_name(severity_code).to_string()),
        };
    }
    Default::default()
}

fn facility_name(code: u8) -> &'static str {
    match code {
        0 => "kern",
        1 => "user",
        2 => "mail",
        3 => "daemon",
        4 => "auth",
        5 => "syslog",
        6 => "lpr",
        7 => "news",
        8 => "uucp",
        9 => "clock",
        10 => "authpriv",
        11 => "ftp",
        12 => "ntp",
        13 => "audit",
        14 => "alert",
        15 => "cron",
        16 => "local0",
        17 => "local1",
        18 => "local2",
        19 => "local3",
        20 => "local4",
        21 => "local5",
        22 => "local6",
        23 => "local7",
        _ => "unknown",
    }
}

fn severity_name(code: u8) -> &'static str {
    match code {
        0 => "emerg",
        1 => "alert",
        2 => "crit",
        3 => "err",
        4 => "warn",
        5 => "notice",
        6 => "info",
        7 => "debug",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_rfc5424() {
        let input = "<14>1 2024-10-05T12:34:56Z host app 123 - - hello world";
        let n = normalize(input);
        assert_eq!(n.message, "hello world");
        assert_eq!(n.meta.pri, Some(14));
        assert_eq!(n.meta.facility.as_deref(), Some("user"));
        assert_eq!(n.meta.severity.as_deref(), Some("info"));
    }

    #[test]
    fn test_normalize_rfc3164() {
        let input = "<34>Oct 11 22:14:15 mymachine su: 'su root' failed";
        let n = normalize(input);
        assert!(n.message.contains("su root"));
        assert_eq!(n.meta.pri, Some(34));
        assert_eq!(n.meta.facility.as_deref(), Some("auth"));
        assert_eq!(n.meta.severity.as_deref(), Some("crit"));
    }

    #[test]
    fn test_normalize_plaintext() {
        let input = "just plaintext";
        let n = normalize(input);
        assert_eq!(n.message, input);
        assert!(n.header.is_none());
        assert!(n.meta.pri.is_none());
    }
}
