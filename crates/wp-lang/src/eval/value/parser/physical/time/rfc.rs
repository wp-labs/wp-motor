use super::common::parse_fixed;
use crate::eval::runtime::field::FieldEvalUnit;
use crate::eval::value::parse_def::PatternParser;
use crate::eval::value::parser::physical::time::gen_time;
use crate::generator::{FieldGenConf, GenChannel};
use crate::types::AnyResult;
use crate::winnow::Parser;
use chrono::format::Fixed;
use winnow::ascii::{alpha1, digit1, multispace0, multispace1};
use winnow::combinator::{alt, dispatch, fail, opt, peek, preceded};
use winnow::error::StrContext;
use winnow::stream::Stream as _;
use winnow::token::literal;
use wp_model_core::model::FNameStr;
use wp_model_core::model::{DataField, DateTimeValue};
use wp_parser::WResult;
use wp_parser::symbol::ctx_desc;

pub fn parse_rfc3339(data: &mut &str) -> WResult<DateTimeValue> {
    let items = &[chrono::format::Item::Fixed(Fixed::RFC3339)];
    // `map_err(|e| e.into())` is a no-op here; just use `?`
    let dt = parse_fixed(data, items)?;
    Ok(dt.naive_local())
}

pub fn parse_rfc2822(data: &mut &str) -> WResult<DateTimeValue> {
    let items = &[chrono::format::Item::Fixed(Fixed::RFC2822)];
    let dt = parse_fixed(data, items)?;
    Ok(dt.naive_local())
}

pub fn parse_time(data: &mut &str) -> WResult<DateTimeValue> {
    alt((
        parse_rfc3339.context(StrContext::Label("parse_rfc3339")),
        parse_rfc2822.context(StrContext::Label("parse_rfc2822")),
        parse_timep.context(StrContext::Label("parse_timep")),
    ))
    .parse_next(data)
}

fn parse_timep(data: &mut &str) -> WResult<DateTimeValue> {
    let date = preceded(
        multispace0,
        alt((parse_date_1, parse_date_2, parse_date_3, parse_date_4)),
    )
    .parse_next(data)?;
    let (h, min, s) = (multispace0, digit1, ":", digit1, ":", digit1)
        .map(|x| (x.1, x.3, x.5))
        .parse_next(data)?;
    let _ = opt(alt((parse_zone_1, parse_zone_2))).parse_next(data)?;
    let time_str = format!("{} {}:{}:{} +00:00", date, h, min, s);
    if let Ok(custom) = chrono::DateTime::parse_from_str(&time_str, "%Y-%b-%d %H:%M:%S %z") {
        Ok(custom.naive_local())
    } else {
        let cp = (*data).checkpoint();
        Err(winnow::error::ErrMode::Backtrack(
            wp_parser::utils::context_error(data, &cp, "time parse fail"),
        ))
    }
}

// ----- helpers moved from old time.rs -----

fn parse_zone_1<'a>(data: &mut &'a str) -> WResult<&'a str> {
    winnow::combinator::delimited(".", digit1, "Z").parse_next(data)
}

fn parse_zone_2<'a>(data: &mut &'a str) -> WResult<&'a str> {
    let flag = alt((literal("+"), literal("-")));
    (multispace0, flag, digit1).map(|x| x.2).parse_next(data)
}

//2023-05-31 00:22:10.894600Z
//06/Aug/2019
fn parse_date_2(data: &mut &str) -> WResult<String> {
    let (_, d, _, m, _, y) =
        (multispace0, digit1, "/", month_patten, "/", digit1).parse_next(data)?;
    let _ = opt(literal(":")).parse_next(data)?;
    Ok(format!("{}-{}-{}", y, m, d))
}

fn parse_date_1(data: &mut &str) -> WResult<String> {
    let sep = alt((literal("-"), literal("/")));
    let sep2 = alt((literal("-"), literal("/")));
    let (_, y, _, m_digit, _, d) = (
        multispace0,
        digit1,
        sep,
        digit1.try_map(str::parse::<u32>),
        sep2,
        digit1,
    )
        .parse_next(data)?;

    let m = match m_digit {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "Jan",
    };
    Ok(format!("{}-{}-{}", y, m, d))
}

//May 17 08:28:12
fn parse_date_4(data: &mut &str) -> WResult<String> {
    let (_, m, _, d, _) =
        (multispace0, month_patten, multispace1, digit1, multispace1).parse_next(data)?;
    let now_year = chrono::Local::now().format("%Y").to_string();
    Ok(format!("{}-{}-{}", now_year, m, d))
}

//May 15 2023
fn parse_date_3(data: &mut &str) -> WResult<String> {
    let (m, _, d, _, y) = (
        month_patten,
        multispace1,
        digit1,
        multispace1,
        digit1.try_map(str::parse::<u32>).verify(|x| *x > 1970),
    )
        .parse_next(data)?;
    Ok(format!("{}-{}-{}", y, m, d))
}

//Sat Jun  3 20:17:21 2023
fn month_patten<'a>(input: &mut &'a str) -> WResult<&'a str> {
    dispatch!( peek(alpha1);
        "Jan" => alpha1,
        "Feb" => alpha1,
        "Mar" => alpha1,
        "Apr" => alpha1,
        "May" => alpha1,
        "Jun" => alpha1,
        "Jul" => alpha1,
        "Aug" => alpha1,
        "Sep" => alpha1,
        "Oct" => alpha1,
        "Nov" => alpha1,
        "Dec" => alpha1,
        _ => fail,
    )
    .parse_next(input)
}

#[derive(Default)]
pub struct TimeP {}
#[derive(Default)]
pub struct TimeISOP {}
#[derive(Default)]
pub struct TimeRFC3339 {}
#[derive(Default)]
pub struct TimeRFC2822 {}

impl PatternParser for TimeP {
    fn pattern_parse(
        &self,
        _e_id: u64,
        _fpu: &FieldEvalUnit,
        _ups_sep: &crate::ast::WplSep,
        data: &mut &str,
        name: FNameStr,
        out: &mut Vec<DataField>,
    ) -> WResult<()> {
        let time = alt((
            parse_rfc3339.context(StrContext::Label("<rfc3339>")),
            parse_rfc2822.context(StrContext::Label("<rfc2822>")),
            parse_timep.context(StrContext::Label("parse_timep")),
        ))
        .context(ctx_desc("<time>"))
        .parse_next(data)?;
        out.push(DataField::from_time(name, time));
        Ok(())
    }

    fn patten_gen(
        &self,
        gnc: &mut GenChannel,
        f_conf: &crate::ast::WplField,
        g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<DataField> {
        gen_time(gnc, f_conf, g_conf)
    }
}

impl PatternParser for TimeISOP {
    fn pattern_parse(
        &self,
        _e_id: u64,
        _fpu: &FieldEvalUnit,
        _: &crate::ast::WplSep,
        data: &mut &str,
        name: FNameStr,
        out: &mut Vec<DataField>,
    ) -> WResult<()> {
        let time = parse_rfc3339.parse_next(data)?;
        out.push(DataField::from_time(name, time));
        Ok(())
    }
    fn patten_gen(
        &self,
        gnc: &mut GenChannel,
        f_conf: &crate::ast::WplField,
        g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<DataField> {
        gen_time(gnc, f_conf, g_conf)
    }
}
impl PatternParser for TimeRFC3339 {
    fn pattern_parse(
        &self,
        e_id: u64,
        fpu: &FieldEvalUnit,
        s: &crate::ast::WplSep,
        d: &mut &str,
        n: FNameStr,
        o: &mut Vec<DataField>,
    ) -> WResult<()> {
        TimeISOP {}.pattern_parse(e_id, fpu, s, d, n, o)
    }
    fn patten_gen(
        &self,
        g: &mut GenChannel,
        f: &crate::ast::WplField,
        c: Option<&FieldGenConf>,
    ) -> AnyResult<DataField> {
        gen_time(g, f, c)
    }
}
impl PatternParser for TimeRFC2822 {
    fn pattern_parse(
        &self,
        _e_id: u64,
        _: &FieldEvalUnit,
        _: &crate::ast::WplSep,
        data: &mut &str,
        name: FNameStr,
        out: &mut Vec<DataField>,
    ) -> WResult<()> {
        let time = parse_rfc2822.parse_next(data)?;
        out.push(DataField::from_time(name, time));
        Ok(())
    }
    fn patten_gen(
        &self,
        gnc: &mut GenChannel,
        f_conf: &crate::ast::WplField,
        g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<DataField> {
        gen_time(gnc, f_conf, g_conf)
    }
}
