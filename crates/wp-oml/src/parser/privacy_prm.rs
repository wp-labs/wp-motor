use crate::language::{
    Encoding, Hash, HashAlgo, Mask, MaskKind, NameStyle, OnError, PIPE_HASH, PIPE_MASK,
    PIPE_REMOVE, PIPE_TOKENIZE, PipeFun, Remove, TokenAlgo, Tokenize,
};
use ipnet::IpNet;
use winnow::ascii::{digit1, multispace0};
use winnow::combinator::{alt, peek};
use winnow::error::{ContextError, ErrMode, StrContext, StrContextValue};
use winnow::stream::Stream;
use winnow::token::{literal, take_until};
use winnow::{ModalResult as WResult, Parser};
use wp_primitives::atom::take_var_name;
use wp_primitives::net::ip;
use wp_primitives::symbol::{symbol_bracket_beg, symbol_bracket_end, symbol_colon, symbol_comma};

#[derive(Clone, Debug)]
enum ArgValue {
    Str(String),
    Num(u64),
    Bool(bool),
    Ident(String),
    Cidr(IpNet),
    Char(char),
}

fn cut_err<T>(label: &'static str, msg: &'static str) -> WResult<T> {
    let mut err = ContextError::new();
    err.push(StrContext::Label(label));
    err.push(StrContext::Expected(StrContextValue::Description(msg)));
    Err(ErrMode::Cut(err))
}

fn backtrack<T>() -> WResult<T> {
    Err(ErrMode::Backtrack(ContextError::new()))
}

fn expect_fun(data: &mut &str, expected: &'static str) -> WResult<()> {
    let cp = data.checkpoint();
    match take_var_name.parse_next(data) {
        Ok(name) if name == expected => Ok(()),
        _ => {
            data.reset(&cp);
            backtrack()
        }
    }
}

fn quoted_value(data: &mut &str) -> WResult<ArgValue> {
    multispace0.parse_next(data)?;
    let quote = alt((literal("\""), literal("'"))).parse_next(data)?;
    let body = take_until(0.., quote).parse_next(data)?;
    literal(quote).parse_next(data)?;
    Ok(ArgValue::Str(body.to_string()))
}

fn bool_value(data: &mut &str) -> WResult<ArgValue> {
    multispace0.parse_next(data)?;
    let cp = data.checkpoint();
    let name = take_var_name.parse_next(data)?;
    match name {
        "true" => Ok(ArgValue::Bool(true)),
        "false" => Ok(ArgValue::Bool(false)),
        _ => {
            data.reset(&cp);
            backtrack()
        }
    }
}

fn number_value(data: &mut &str) -> WResult<ArgValue> {
    multispace0.parse_next(data)?;
    let digits = digit1.parse_next(data)?;
    let value = digits
        .parse::<u64>()
        .map_err(|_| ErrMode::Backtrack(ContextError::new()))?;
    Ok(ArgValue::Num(value))
}

fn cidr_value(data: &mut &str) -> WResult<ArgValue> {
    multispace0.parse_next(data)?;
    let addr = ip.parse_next(data)?;
    literal("/").parse_next(data)?;
    let prefix = digit1.parse_next(data)?;
    let net = format!("{addr}/{prefix}")
        .parse::<IpNet>()
        .map_err(|_| ErrMode::Backtrack(ContextError::new()))?;
    Ok(ArgValue::Cidr(net))
}

fn star_value(data: &mut &str) -> WResult<ArgValue> {
    multispace0.parse_next(data)?;
    literal("*").parse_next(data)?;
    Ok(ArgValue::Char('*'))
}

fn ident_value(data: &mut &str) -> WResult<ArgValue> {
    multispace0.parse_next(data)?;
    let name = take_var_name.parse_next(data)?;
    Ok(ArgValue::Ident(name.to_string()))
}

fn arg_value(data: &mut &str) -> WResult<ArgValue> {
    alt((
        quoted_value,
        cidr_value,
        bool_value,
        number_value,
        star_value,
        ident_value,
    ))
    .parse_next(data)
}

fn named_arg(data: &mut &str) -> WResult<(String, ArgValue)> {
    multispace0.parse_next(data)?;
    let key = take_var_name.parse_next(data)?;
    symbol_colon.parse_next(data)?;
    let value = arg_value.parse_next(data)?;
    Ok((key.to_ascii_lowercase(), value))
}

fn apply_mask_arg(mask: &mut Mask, key: &str, value: ArgValue) -> Result<(), &'static str> {
    let allowed = match mask.kind {
        MaskKind::Ip => matches!(key, "pool" | "allow_unsalted" | "placeholder" | "on_error"),
        MaskKind::Mobile => matches!(key, "keep_last" | "char" | "placeholder" | "on_error"),
        MaskKind::IdCard => matches!(
            key,
            "keep_first" | "keep_last" | "char" | "placeholder" | "on_error"
        ),
        MaskKind::Mail => matches!(
            key,
            "keep_first" | "keep_domain" | "char" | "placeholder" | "on_error"
        ),
        MaskKind::Domain => matches!(key, "keep_levels" | "char" | "placeholder" | "on_error"),
        MaskKind::Name => matches!(key, "style" | "char" | "placeholder" | "on_error"),
        MaskKind::Address | MaskKind::Company => matches!(
            key,
            "keep_first" | "keep_last" | "char" | "placeholder" | "on_error"
        ),
        MaskKind::KeyMsg => matches!(key, "placeholder" | "on_error"),
        MaskKind::Text => matches!(
            key,
            "keep_first" | "keep_last" | "char" | "placeholder" | "on_error"
        ),
    };
    if !allowed {
        return Err("parameter is not valid for this mask kind");
    }
    match key {
        "keep_first" => mask.keep_first = Some(expect_num(value)?),
        "keep_last" => mask.keep_last = Some(expect_num(value)?),
        "keep_levels" => mask.keep_levels = Some(expect_num(value)?),
        "keep_domain" => mask.keep_domain = Some(expect_bool(value)?),
        "style" => mask.name_style = Some(expect_name_style(value)?),
        "char" => mask.char = Some(expect_char(value)?),
        "placeholder" => mask.placeholder = Some(expect_str(value)?),
        "on_error" => mask.on_error = expect_on_error(value)?,
        "pool" => mask.pool = Some(expect_cidr(value)?),
        "allow_unsalted" => mask.allow_unsalted = expect_bool(value)?,
        _ => return Err("unknown mask parameter"),
    }
    Ok(())
}

fn apply_hash_arg(hash: &mut Hash, key: &str, value: ArgValue) -> Result<(), &'static str> {
    match key {
        "algo" => hash.algo = expect_hash_algo(value)?,
        "encoding" => hash.encoding = expect_encoding(value)?,
        "truncate" => hash.truncate = Some(expect_num(value)?),
        _ => return Err("unknown hash parameter"),
    }
    Ok(())
}

fn apply_tokenize_arg(
    token: &mut Tokenize,
    key: &str,
    value: ArgValue,
) -> Result<(), &'static str> {
    match key {
        "algo" => token.algo = expect_token_algo(value)?,
        "prefix" => token.prefix = expect_str(value)?,
        _ => return Err("unknown tokenize parameter"),
    }
    Ok(())
}

fn expect_num(value: ArgValue) -> Result<usize, &'static str> {
    match value {
        ArgValue::Num(v) => usize::try_from(v).map_err(|_| "number out of range"),
        _ => Err("expected a non-negative integer"),
    }
}

fn expect_bool(value: ArgValue) -> Result<bool, &'static str> {
    match value {
        ArgValue::Bool(v) => Ok(v),
        _ => Err("expected true or false"),
    }
}

fn expect_str(value: ArgValue) -> Result<String, &'static str> {
    match value {
        ArgValue::Str(v) => Ok(v),
        ArgValue::Ident(v) => Ok(v),
        _ => Err("expected a string"),
    }
}

fn expect_char(value: ArgValue) -> Result<char, &'static str> {
    match value {
        ArgValue::Char(v) => Ok(v),
        ArgValue::Str(v) => {
            let mut chars = v.chars();
            match (chars.next(), chars.next()) {
                (Some(ch), None) => Ok(ch),
                _ => Err("char must contain exactly one character"),
            }
        }
        _ => Err("expected a single character"),
    }
}

fn expect_cidr(value: ArgValue) -> Result<IpNet, &'static str> {
    match value {
        ArgValue::Cidr(v) => Ok(v),
        _ => Err("expected an IP CIDR, e.g. pool:198.18.0.0/15"),
    }
}

fn expect_hash_algo(value: ArgValue) -> Result<HashAlgo, &'static str> {
    match value {
        ArgValue::Ident(v) => HashAlgo::from_name(&v).ok_or("supported: sha256, sha1, md5"),
        _ => Err("expected a hash algorithm"),
    }
}

fn expect_token_algo(value: ArgValue) -> Result<TokenAlgo, &'static str> {
    match value {
        ArgValue::Ident(v) => TokenAlgo::from_name(&v).ok_or("supported: aes_gcm"),
        _ => Err("expected a token algorithm"),
    }
}

fn expect_encoding(value: ArgValue) -> Result<Encoding, &'static str> {
    match value {
        ArgValue::Ident(v) => Encoding::from_name(&v).ok_or("supported: hex, base64url"),
        _ => Err("expected an encoding"),
    }
}

fn expect_name_style(value: ArgValue) -> Result<NameStyle, &'static str> {
    match value {
        ArgValue::Ident(v) => NameStyle::from_name(&v).ok_or("supported: head, tail, full"),
        _ => Err("expected a name style"),
    }
}

fn expect_on_error(value: ArgValue) -> Result<OnError, &'static str> {
    match value {
        ArgValue::Ident(v) => OnError::from_name(&v).ok_or("supported: placeholder, fail"),
        _ => Err("expected placeholder or fail"),
    }
}

fn parse_mask_kind(data: &mut &str) -> WResult<MaskKind> {
    multispace0.parse_next(data)?;
    let name = take_var_name.parse_next(data)?;
    MaskKind::from_name(name).ok_or_else(|| {
        let mut err = ContextError::new();
        err.push(StrContext::Label("mask kind"));
        err.push(StrContext::Expected(StrContextValue::Description(
            "ip | mobile | id_card | mail | domain | name | address | company | keymsg | text",
        )));
        ErrMode::Cut(err)
    })
}

/// `mask(kind, [params])`
pub fn oml_mask(data: &mut &str) -> WResult<PipeFun> {
    let cp = data.checkpoint();
    if expect_fun(data, PIPE_MASK).is_err() {
        data.reset(&cp);
        return backtrack();
    }
    symbol_bracket_beg.parse_next(data)?;
    let kind = parse_mask_kind(data)?;
    let mut mask = Mask::new(kind);
    loop {
        multispace0.parse_next(data)?;
        if peek(symbol_bracket_end).parse_next(data).is_ok() {
            break;
        }
        symbol_comma.parse_next(data)?;
        let (key, value) = named_arg(data)?;
        if let Err(msg) = apply_mask_arg(&mut mask, &key, value) {
            return cut_err("mask parameter", msg);
        }
    }
    symbol_bracket_end.parse_next(data)?;
    Ok(PipeFun::Mask(mask))
}

/// `hash([algo], [params])`
pub fn oml_hash(data: &mut &str) -> WResult<PipeFun> {
    let cp = data.checkpoint();
    if expect_fun(data, PIPE_HASH).is_err() {
        data.reset(&cp);
        return backtrack();
    }
    symbol_bracket_beg.parse_next(data)?;
    let mut hash = Hash::default();
    multispace0.parse_next(data)?;
    if peek(symbol_bracket_end).parse_next(data).is_err() {
        // The first argument may be a positional algorithm or a named parameter.
        if peek(named_arg).parse_next(data).is_ok() {
            let (key, value) = named_arg(data)?;
            if let Err(msg) = apply_hash_arg(&mut hash, &key, value) {
                return cut_err("hash parameter", msg);
            }
        } else {
            let name = take_var_name.parse_next(data)?;
            hash.algo = HashAlgo::from_name(name).ok_or_else(|| {
                let mut err = ContextError::new();
                err.push(StrContext::Label("hash algo"));
                err.push(StrContext::Expected(StrContextValue::Description(
                    "sha256 | sha1 | md5",
                )));
                ErrMode::Cut(err)
            })?;
        }
        loop {
            multispace0.parse_next(data)?;
            if peek(symbol_bracket_end).parse_next(data).is_ok() {
                break;
            }
            symbol_comma.parse_next(data)?;
            let (key, value) = named_arg(data)?;
            if let Err(msg) = apply_hash_arg(&mut hash, &key, value) {
                return cut_err("hash parameter", msg);
            }
        }
    }
    symbol_bracket_end.parse_next(data)?;
    Ok(PipeFun::Hash(hash))
}

/// `tokenize([params])`
pub fn oml_tokenize(data: &mut &str) -> WResult<PipeFun> {
    let cp = data.checkpoint();
    if expect_fun(data, PIPE_TOKENIZE).is_err() {
        data.reset(&cp);
        return backtrack();
    }
    symbol_bracket_beg.parse_next(data)?;
    let mut token = Tokenize::default();
    let mut first = true;
    loop {
        multispace0.parse_next(data)?;
        if peek(symbol_bracket_end).parse_next(data).is_ok() {
            break;
        }
        if !first {
            symbol_comma.parse_next(data)?;
        }
        let (key, value) = named_arg(data)?;
        if let Err(msg) = apply_tokenize_arg(&mut token, &key, value) {
            return cut_err("tokenize parameter", msg);
        }
        first = false;
    }
    symbol_bracket_end.parse_next(data)?;
    // Keys are fixed: the engine uses the system-level `pii_key`.
    Ok(PipeFun::Tokenize(token))
}

/// `remove`
pub fn oml_remove(data: &mut &str) -> WResult<PipeFun> {
    let cp = data.checkpoint();
    if expect_fun(data, PIPE_REMOVE).is_err() {
        data.reset(&cp);
        return backtrack();
    }
    Ok(PipeFun::Remove(Remove))
}
