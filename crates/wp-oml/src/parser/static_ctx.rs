use std::cell::RefCell;
use std::collections::HashSet;

use winnow::ascii::multispace0;
use winnow::error::{ContextError, ErrMode};
use winnow::stream::Stream;
use wp_parser::Parser;
use wp_parser::WResult;
use wp_parser::atom::take_var_name;

use crate::language::PreciseEvaluator;

thread_local! {
    static STATIC_SYMBOLS: RefCell<Option<HashSet<String>>> = const { RefCell::new(None) };
}

pub fn install_symbols(symbols: Vec<String>) {
    STATIC_SYMBOLS.with(|ctx| {
        if symbols.is_empty() {
            *ctx.borrow_mut() = None;
        } else {
            *ctx.borrow_mut() = Some(symbols.into_iter().collect());
        }
    });
}

pub fn clear_symbols() {
    STATIC_SYMBOLS.with(|ctx| {
        *ctx.borrow_mut() = None;
    });
}

fn contains(name: &str) -> bool {
    STATIC_SYMBOLS.with(|ctx| {
        ctx.borrow()
            .as_ref()
            .map(|set| set.contains(name))
            .unwrap_or(false)
    })
}

pub fn parse_static_value(data: &mut &str) -> WResult<PreciseEvaluator> {
    let cp = data.checkpoint();
    multispace0.parse_next(data)?;
    match take_var_name.parse_next(data) {
        Ok(name) => {
            if contains(name) {
                Ok(PreciseEvaluator::StaticSymbol(name.to_string()))
            } else {
                data.reset(&cp);
                Err(ErrMode::Backtrack(ContextError::new()))
            }
        }
        Err(_) => {
            data.reset(&cp);
            Err(ErrMode::Backtrack(ContextError::new()))
        }
    }
}
