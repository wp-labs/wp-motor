use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime};
use winnow::Parser as _;
use winnow::combinator::fail;
use wp_parser::symbol::ctx_desc;

// Shared helper to parse a fixed chrono::format::Item sequence and advance input
pub fn parse_fixed<'a>(
    data: &mut &'a str,
    fixed: &'a [chrono::format::Item<'a>],
) -> wp_parser::WResult<DateTime<FixedOffset>> {
    use chrono::format::{Parsed, parse_and_remainder};
    let mut parsed: Parsed = Parsed::new();
    let remain = match parse_and_remainder(&mut parsed, data, fixed.iter()) {
        Ok(v) => v,
        Err(_) => fail
            .context(ctx_desc("chrono fixed parse failed"))
            .parse_next(data)?,
    };
    let dt = match parsed.to_datetime() {
        Ok(v) => v,
        Err(_) => fail
            .context(ctx_desc("chrono fixed parse failed"))
            .parse_next(data)?,
    };
    *data = remain;
    Ok(dt)
}

// Strict fast-path CLF time: dd/Mon/yyyy:HH:MM:SS [+/-ZZZZ], optionally wrapped by [ ... ]
pub fn fast_apache_dt(s: &str) -> Option<(usize, NaiveDateTime)> {
    let b = s.as_bytes();
    let mut off = 0usize;
    let had_bracket = if b.first() == Some(&b'[') {
        off = 1;
        true
    } else {
        false
    };
    let bb = &b[off..];
    if bb.len() < 20 {
        return None;
    }
    // day (2 digits)
    let d1 = *bb.first()?;
    let d2 = *bb.get(1)?;
    if *bb.get(2)? != b'/' {
        return None;
    }
    if !d1.is_ascii_digit() || !d2.is_ascii_digit() {
        return None;
    }
    let day = (d1 - b'0') as u32 * 10 + (d2 - b'0') as u32;
    // month (ASCII, case-sensitive)
    let mm = [*bb.get(3)?, *bb.get(4)?, *bb.get(5)?];
    let month = match mm {
        [b'J', b'a', b'n'] => 1,
        [b'F', b'e', b'b'] => 2,
        [b'M', b'a', b'r'] => 3,
        [b'A', b'p', b'r'] => 4,
        [b'M', b'a', b'y'] => 5,
        [b'J', b'u', b'n'] => 6,
        [b'J', b'u', b'l'] => 7,
        [b'A', b'u', b'g'] => 8,
        [b'S', b'e', b'p'] => 9,
        [b'O', b'c', b't'] => 10,
        [b'N', b'o', b'v'] => 11,
        [b'D', b'e', b'c'] => 12,
        _ => return None,
    } as u32;
    if *bb.get(6)? != b'/' {
        return None;
    }
    // year (4 digits)
    let y0 = *bb.get(7)?;
    let y1 = *bb.get(8)?;
    let y2 = *bb.get(9)?;
    let y3 = *bb.get(10)?;
    if !(y0.is_ascii_digit() && y1.is_ascii_digit() && y2.is_ascii_digit() && y3.is_ascii_digit()) {
        return None;
    }
    let y = ((y0 - b'0') as i32) * 1000
        + ((y1 - b'0') as i32) * 100
        + ((y2 - b'0') as i32) * 10
        + ((y3 - b'0') as i32);
    if *bb.get(11)? != b':' {
        return None;
    }
    // HH:MM:SS
    let dd = |i: usize| -> Option<u32> {
        let a = *bb.get(i)?;
        let c = *bb.get(i + 1)?;
        if !a.is_ascii_digit() || !c.is_ascii_digit() {
            return None;
        }
        Some(((a - b'0') as u32) * 10 + (c - b'0') as u32)
    };
    let h = dd(12)?;
    if *bb.get(14)? != b':' {
        return None;
    }
    let m = dd(15)?;
    if *bb.get(17)? != b':' {
        return None;
    }
    let s2 = dd(18)?;
    let date = NaiveDate::from_ymd_opt(y, month, day)?;
    let time = NaiveTime::from_hms_opt(h, m, s2)?;
    let ndt = NaiveDateTime::new(date, time);
    // zone
    let mut i = 20usize;
    if bb.get(i) == Some(&b' ') {
        i += 1;
        while i < bb.len()
            && (bb[i].is_ascii_digit() || bb[i] == b'+' || bb[i] == b'-' || bb[i] == b':')
        {
            i += 1;
        }
    }
    let mut consumed = off + i;
    if had_bracket && b.get(consumed) == Some(&b']') {
        consumed += 1;
    }
    Some((consumed, ndt))
}
