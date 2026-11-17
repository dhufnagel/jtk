//! jtk, a Java toolkit. Its formatter is a Rust port of palantir-java-format, with byte-identical output.

pub mod ast;
pub mod cli;
pub mod comments;
pub mod config;
pub mod doc;
pub mod formatter;
pub mod imports;
pub mod input;
pub mod javadoc;
pub mod lint;
pub mod lsp;
pub mod modifiers;
pub mod newlines;
pub mod ops;
pub mod output;
pub mod parse;
pub mod parser;
pub mod string_wrapper;
pub mod visitor;

pub use formatter::{
    FormatError, FormatOptions, fix_imports, format, format_file, format_replacements_with_reflow, format_source,
    format_without_reflow,
};
