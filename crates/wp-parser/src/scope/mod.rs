//! Scope evaluation utilities
//!
//! This module provides tools for matching and evaluating scoped content
//! with balanced delimiters.
//!
//! - [`ScopeEval`] - Basic scope matching with nested delimiters
//! - [`EscapedScopeEval`] - Scope matching with escape sequence support

mod basic;
mod escaped;

pub use basic::ScopeEval;
pub use escaped::EscapedScopeEval;
