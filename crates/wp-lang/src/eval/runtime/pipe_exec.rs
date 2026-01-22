use crate::ast::WplSep;
use crate::eval::runtime::field_pipe::{FieldIndex, FieldSelector, FieldSelectorSpec, PipeEnum};
use crate::eval::runtime::group::WplEvalGroup;
use once_cell::sync::OnceCell;
use winnow::combinator::fail;
use wp_model_core::model::{DataField, Value};
use wp_parser::symbol::ctx_desc;
use wp_parser::{Parser, WResult as ModalResult};

/// Heuristic thresholds to enable FieldIndex for Fun pipes.
/// Can be overridden via environment variables at process start:
/// - `WP_PIPE_FUN_THRESH` (default: 20)
/// - `WP_PIPE_FIELD_THRESH` (default: 1024)
struct Thresholds {
    fun: usize,
    fields: usize,
}

static PIPE_THRESHOLDS: OnceCell<Thresholds> = OnceCell::new();

#[inline]
fn thresholds() -> &'static Thresholds {
    PIPE_THRESHOLDS.get_or_init(|| Thresholds {
        fun: std::env::var("WP_PIPE_FUN_THRESH")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(20),
        fields: std::env::var("WP_PIPE_FIELD_THRESH")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1024),
    })
}

#[derive(Clone, Default)]
pub struct PipeExecutor {
    pipes: Vec<PipeEnum>,
}

impl PipeExecutor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_pipe(&mut self, pipe: PipeEnum) {
        self.pipes.push(pipe);
    }

    pub fn execute(&self, data: &mut Vec<DataField>) -> ModalResult<()> {
        let mut cursor = FieldCursor::new(data, &self.pipes);

        for pipe in &self.pipes {
            match pipe {
                PipeEnum::Fun(fun) => {
                    if let Some(selector) = fun.as_field_selector() {
                        cursor.apply_selector(selector, data)?;
                        continue;
                    }

                    if let Some(field_fun) = fun.as_field_pipe() {
                        let field = cursor.ensure_active_field(data, fun.auto_selector_spec())?;
                        field_fun.process(field)?;
                        continue;
                    }
                }
                PipeEnum::Group(group) => {
                    let Some(idx) =
                        cursor.ensure_active_index(data, Some(FieldSelectorSpec::Last))?
                    else {
                        continue;
                    };
                    let removed = data.remove(idx);
                    process_group_pipe(group, removed, data)?;
                    cursor.after_mutation(data, true);
                }
            }
        }
        Ok(())
    }
}

fn process_group_pipe(
    group: &WplEvalGroup,
    field: DataField,
    fields: &mut Vec<DataField>,
) -> ModalResult<()> {
    if let Value::Chars(res_data) = field.get_value() {
        let sep = WplSep::default();
        let mut data = res_data.as_str();
        group.proc(&sep, &mut data, fields)
    } else {
        fail.context(ctx_desc("not support parse pipe"))
            .parse_next(&mut "")
    }
}

struct FieldCursor {
    active_idx: Option<usize>,
    index: Option<FieldIndex>,
}

impl FieldCursor {
    fn new(fields: &[DataField], pipes: &[PipeEnum]) -> Self {
        let index = Self::build_index_if_needed(fields, pipes);
        Self {
            active_idx: None,
            index,
        }
    }

    fn apply_selector(
        &mut self,
        selector: &dyn FieldSelector,
        fields: &mut Vec<DataField>,
    ) -> ModalResult<()> {
        self.active_idx = selector.select(fields, self.index.as_ref())?;
        Ok(())
    }

    fn ensure_active_field<'a>(
        &mut self,
        fields: &'a mut [DataField],
        hint: Option<FieldSelectorSpec<'_>>,
    ) -> ModalResult<Option<&'a mut DataField>> {
        let idx = self.ensure_active_index(fields, hint)?;
        Ok(idx.and_then(move |i| fields.get_mut(i)))
    }

    fn ensure_active_index(
        &mut self,
        fields: &mut [DataField],
        hint: Option<FieldSelectorSpec<'_>>,
    ) -> ModalResult<Option<usize>> {
        if fields.is_empty() {
            self.active_idx = None;
            return Ok(None);
        }

        let mut hint = hint;
        if hint.is_none() && self.active_idx.is_none() {
            hint = Some(FieldSelectorSpec::Last);
        }

        if let Some(spec) = hint {
            self.active_idx = self.select_with_spec(fields, spec)?;
            return Ok(self.active_idx);
        }

        if let Some(idx) = self.active_idx
            && idx >= fields.len()
        {
            self.active_idx = None;
        }

        if self.active_idx.is_none() {
            self.active_idx = Some(fields.len() - 1);
        }

        Ok(self.active_idx)
    }

    fn select_with_spec(
        &mut self,
        fields: &mut [DataField],
        spec: FieldSelectorSpec<'_>,
    ) -> ModalResult<Option<usize>> {
        match spec {
            FieldSelectorSpec::Take(name) => self.select_by_name(fields, name),
            FieldSelectorSpec::Last => self.select_last(fields),
        }
    }

    fn select_by_name(&self, fields: &mut [DataField], name: &str) -> ModalResult<Option<usize>> {
        if let Some(idx) = self.index.as_ref().and_then(|map| map.get(name))
            && idx < fields.len()
        {
            return Ok(Some(idx));
        }
        if let Some(pos) = fields.iter().position(|f| f.get_name() == name) {
            Ok(Some(pos))
        } else {
            fail.context(ctx_desc("<pipe> | not exists"))
                .parse_next(&mut "")?;
            Ok(None)
        }
    }

    fn select_last(&self, fields: &mut [DataField]) -> ModalResult<Option<usize>> {
        if fields.is_empty() {
            fail.context(ctx_desc("<pipe> | not exists"))
                .parse_next(&mut "")?;
            Ok(None)
        } else {
            Ok(Some(fields.len() - 1))
        }
    }

    fn after_mutation(&mut self, fields: &[DataField], mutated: bool) {
        self.active_idx = None;
        if mutated && self.index.is_some() {
            self.index = Some(FieldIndex::build(fields));
        }
    }

    fn build_index_if_needed(fields: &[DataField], pipes: &[PipeEnum]) -> Option<FieldIndex> {
        let selector_cnt = pipes
            .iter()
            .filter(|pipe| match pipe {
                PipeEnum::Fun(fun) => fun.requires_index(),
                PipeEnum::Group(_) => false,
            })
            .count();

        let th = thresholds();
        if selector_cnt >= th.fun && fields.len() >= th.fields {
            Some(FieldIndex::build(fields))
        } else {
            None
        }
    }
}
