//! # wp-parser
//!
//! A parser combinator library for the Warp Flow DSL (Domain Specific Language).
//!
//! This crate provides parsing capabilities for various language constructs including:
//! - Conditional expressions with logical operators (AND, OR, NOT)
//! - Function calls with typed arguments
//! - Variable names, paths, and identifiers
//! - Scope matching with nested brackets and escaped delimiters
//! - SQL-style and Rust-style operator syntax
//! - Comment filtering (single-line `//` and multi-line `/* */`)
//!
//! ## Architecture
//!
//! The parser is built on top of the [`winnow`] parser combinator library and provides:
//!
//! - **Symbol parsers** (`symbol`, `sql_symbol`): Parse operators and punctuation
//! - **Atom parsers** (`atom`): Parse basic elements like variable names and key-value pairs
//! - **Scope evaluators** (`scope`): Match balanced delimiters with nesting and escape support
//! - **Conditional parsers** (`cond`): Parse complex boolean expressions
//! - **Function parsers** (`fun`): Parse function invocations with arguments
//! - **Utilities** (`utils`, `comment`): Helper functions and comment stripping
//!
//! ## Examples
//!
//! ### Parsing a variable name
//!
//! ```rust
//! use wp_parser::atom::take_var_name;
//! use winnow::Parser;
//!
//! let mut input = "my_var.field";
//! let var_name = take_var_name.parse_next(&mut input).unwrap();
//! assert_eq!(var_name, "my_var.field");
//! ```
//!
//! ### Parsing conditional expressions
//!
//! ```rust
//! ```
//!
//! ### Matching balanced scopes
//!
//! ```rust
//! use wp_parser::scope::ScopeEval;
//!
//! let input = "(nested (content) here)";
//! let length = ScopeEval::len(input, '(', ')');
//! assert_eq!(length, input.len());
//! ```
//!
//! ## Feature Flags
//!
//! This crate currently has no optional features.

pub use winnow::Parser;
// Centralized parse result alias. Switch here if we migrate away from ModalResult later.
pub type WResult<T> = winnow::ModalResult<T>;

pub mod atom;
pub mod comment;
pub mod cond;
pub mod fun;
pub mod net;
pub mod scope;
pub mod sql_symbol;
pub mod symbol;
pub mod utils;
