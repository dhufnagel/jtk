//! A Rust port of palantir-java-format, limited to formatting Java source code.

pub mod ast;
pub mod comments;
pub mod doc;
pub mod formatter;
pub mod imports;
pub mod input;
pub mod modifiers;
pub mod newlines;
pub mod ops;
pub mod output;
pub mod string_wrapper;
pub mod visitor;

pub use formatter::{FormatError, FormatOptions, format, format_source, format_without_reflow};
