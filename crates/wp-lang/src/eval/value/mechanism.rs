use crate::ast::{WplField, WplSep};
use crate::eval::runtime::field::FieldEvalUnit;
use crate::eval::value::field_parse::FieldParse;
use crate::eval::value::parse_def::{FieldParser, PatternParser};
use crate::generator::FieldGenConf;
use crate::generator::{FmtField, GenChannel};
use crate::types::AnyResult;
use crate::winnow::Parser;
use wp_model_core::model::FNameStr;
use wp_model_core::model::{DataField, DataType};

use winnow::ascii::multispace0;
use winnow::combinator::fail;
use winnow::stream::Stream;
use wp_parser::WResult as ModalResult;
use wp_parser::symbol::ctx_desc;
use wp_parser::utils::RestAble;

impl<T> FieldParser for T
where
    T: PatternParser,
{
    fn parse(
        &self,
        fpu: &FieldEvalUnit,
        ups_sep: &WplSep,
        data: &mut &str,
        f_name: Option<FNameStr>,
        out: &mut Vec<DataField>,
    ) -> ModalResult<()> {
        let mut name = Some(
            if *fpu.conf().meta_type() == DataType::Json
                || *fpu.conf().meta_type() == DataType::ExactJson
            {
                f_name.unwrap_or_default()
            } else {
                f_name.unwrap_or_else(|| fpu.conf().safe_name())
            },
        );
        multispace0.parse_next(data)?;
        if fpu.conf().have_scope() {
            let cp = data.checkpoint();
            let mut take = fpu.conf().scope_field(data)?;
            self.pattern_parse(
                fpu,
                ups_sep,
                &mut take,
                name.take().expect("name consumed once"),
                out,
            )
            .err_reset(data, &cp)?;
            multispace0.parse_next(&mut take)?;
            multispace0.parse_next(data)?;
            if !data.is_empty() && ups_sep.need_take_sep() {
                ups_sep.try_consume_sep(data)?;
            }
            if !take.is_empty() {
                return fail
                    .context(ctx_desc("patten parse left is not empty"))
                    .parse_next(data);
            }
            Ok(())
        } else {
            if fpu.conf().meta_type().parse_patten_first() {
                self.pattern_parse(
                    fpu,
                    ups_sep,
                    data,
                    name.take().expect("name consumed once"),
                    out,
                )?;
                multispace0.parse_next(data)?;
                //patten match will option proc sep symbol
                if !data.is_empty() && ups_sep.need_take_sep() {
                    ups_sep.try_consume_sep(data)?;
                }
                return Ok(());
            }
            let cp = data.checkpoint();
            let take = if let Some(cnt) = fpu.conf().field_cnt() {
                ups_sep.read_until_sep_repeat(cnt, data)?
            } else {
                ups_sep.read_until_sep(data)?
            };
            self.pattern_parse(
                fpu,
                ups_sep,
                &mut take.as_str(),
                name.take().expect("name consumed once"),
                out,
            )
            .err_reset(data, &cp)?;
            multispace0.parse_next(data)?;
            if !data.is_empty() && ups_sep.need_take_sep() {
                ups_sep.try_consume_sep(data)?;
            }
            Ok(())
        }
    }

    fn generate(
        &self,
        gnc: &mut GenChannel,
        ups_sep: &WplSep,
        f_conf: &WplField,
        g_conf: Option<&FieldGenConf>,
    ) -> AnyResult<FmtField> {
        let field = self.patten_gen(gnc, f_conf, g_conf)?;
        let sep = f_conf.resolve_sep(ups_sep);
        Ok(FmtField::new(
            f_conf.meta_type.clone(),
            field,
            f_conf.fmt_conf.clone(),
            sep,
        ))
    }
}
