use std::net::{IpAddr, Ipv4Addr};

use winnow::{
    ModalResult as WResult, Parser,
    ascii::{Caseless, multispace0},
    combinator::{fail, peek, repeat},
    error::ContextError,
    token::any,
};

use crate::symbol::ctx_desc;
use crate::utils::peek_one;

#[derive(PartialEq)]
enum AddrKind {
    Ipv4,
    Ipv6,
}

fn head_ip<'a>(last: &mut Option<AddrKind>) -> impl Parser<&'a str, char, ContextError> + '_ {
    move |input: &mut &'a str| {
        let initial = (peek(any)).parse_next(input)?;
        match initial {
            '0'..='9' => any.parse_next(input),
            'A'..='F' | 'a'..='f' => {
                *last = Some(AddrKind::Ipv6);
                any.parse_next(input)
            }
            '.' => {
                if *last == Some(AddrKind::Ipv6) {
                    fail.parse_next(input)
                } else {
                    *last = Some(AddrKind::Ipv4);
                    any.parse_next(input)
                }
            }
            ':' => {
                if *last == Some(AddrKind::Ipv4) {
                    fail.parse_next(input)
                } else {
                    *last = Some(AddrKind::Ipv6);
                    any.parse_next(input)
                }
            }
            _ => fail.parse_next(input),
        }
    }
}

pub fn ip_v4(input: &mut &str) -> WResult<IpAddr> {
    let mut last_kind = None;
    // Build the candidate ip string, then parse using std::net::IpAddr::from_str.
    // Avoids relying on `try_map` error conversion semantics across winnow versions.
    let ip_str = match repeat(1.., head_ip(&mut last_kind))
        .fold(String::new, |mut acc, c| {
            acc.push(c);
            acc
        })
        .parse_next(input)
    {
        Ok(s) => s,
        Err(_e) => return fail.context(ctx_desc("<ipv4>")).parse_next(input),
    };
    match ip_str.parse::<IpAddr>() {
        Ok(ip) => Ok(ip),
        Err(_e) => fail.context(ctx_desc("<ipv4>")).parse_next(input),
    }
}
pub fn ip(input: &mut &str) -> WResult<IpAddr> {
    multispace0.parse_next(input)?;

    let str = peek_one.parse_next(input);
    if let Ok(s) = str {
        let addr = if s == "l" {
            Caseless("localhost")
                .map(|_| IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)))
                .context(ctx_desc("<localhost>"))
                .parse_next(input)?
        } else {
            ip_v4.context(ctx_desc("<ipv4>")).parse_next(input)?
        };
        Ok(addr)
    } else {
        fail.context(ctx_desc("ip error")).parse_next(input)
    }
}
