//! Entry point tying the pipeline together (port of `Formatter.java`).

use crate::ast::Converter;
use crate::comments::CommentsHelper;
use crate::doc::{Docs, Layout, State};
use crate::input::JavaInput;
use crate::ops::OpsBuilder;
use crate::output::{JavaOutput, Replacement};
use crate::visitor::Visitor;
use crate::{imports, modifiers, string_wrapper};

pub const MAX_LINE_LENGTH: i32 = 120;

#[derive(Debug)]
pub struct FormatError(pub String);

impl std::fmt::Display for FormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for FormatError {}

/// What to do besides formatting. The defaults match the original command line tool.
#[derive(Clone, Copy, Debug)]
pub struct FormatOptions {
    pub remove_unused_imports: bool,
    pub sort_imports: bool,
    pub reflow_long_strings: bool,
}

impl Default for FormatOptions {
    fn default() -> Self {
        FormatOptions { remove_unused_imports: true, sort_imports: true, reflow_long_strings: true }
    }
}

/// Formats a Java compilation unit like the original command line tool: format, remove unused imports, sort
/// imports, and reflow long string literals (each step except formatting can be switched off).
pub fn format(text: &str, options: &FormatOptions) -> Result<String, FormatError> {
    with_large_stack(|| format_on_current_thread(text, options))
}

/// Deeply nested input (e.g. generated data files with thousands of concatenated strings) needs more stack than the
/// main thread's default, so formatting runs on a thread with a large (lazily committed) stack.
fn with_large_stack<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    const STACK_SIZE: usize = 1 << 28;
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(STACK_SIZE)
            .spawn_scoped(scope, f)
            .expect("failed to spawn formatter thread")
            .join()
            .unwrap_or_else(|e| std::panic::resume_unwind(e))
    })
}

fn format_on_current_thread(text: &str, options: &FormatOptions) -> Result<String, FormatError> {
    let mut formatted = format_without_reflow(text)?;
    if options.remove_unused_imports {
        formatted = imports::remove_unused_imports(&formatted);
    }
    if options.sort_imports {
        formatted = imports::reorder_imports(&formatted).map_err(FormatError)?;
    }
    if options.reflow_long_strings {
        formatted = string_wrapper::wrap(MAX_LINE_LENGTH as usize, &formatted, format_ranges)?;
    }
    Ok(formatted)
}

/// Formats a Java compilation unit and reflows long string literals, leaving imports alone.
pub fn format_source(text: &str) -> Result<String, FormatError> {
    format(text, &FormatOptions { remove_unused_imports: false, sort_imports: false, reflow_long_strings: true })
}

/// Formats a Java compilation unit, without reflowing long string literals.
pub fn format_without_reflow(text: &str) -> Result<String, FormatError> {
    format_ranges(text, &[(0, text.len())])
}

/// Formats only the given byte ranges of the input (extended to whole statements or declarations), like
/// `Formatter.formatSource(input, characterRanges)`.
pub fn format_ranges(text: &str, character_ranges: &[(usize, usize)]) -> Result<String, FormatError> {
    let replacements = format_replacements(text, character_ranges)?;
    let mut result = text.to_string();
    let mut sorted = replacements;
    sorted.sort_by_key(|r| std::cmp::Reverse(r.start));
    for r in sorted {
        result.replace_range(r.start..r.end, &r.text);
    }
    Ok(result)
}

/// The replacements that format the given byte ranges of the input.
pub fn format_replacements(text: &str, character_ranges: &[(usize, usize)]) -> Result<Vec<Replacement>, FormatError> {
    let mut input = JavaInput::new(text).map_err(|e| FormatError(e.0))?;
    let token_ranges = input.character_ranges_to_token_ranges(character_ranges).map_err(|e| FormatError(e.0))?;
    let reordered = modifiers::reorder_modifiers(&input, &token_ranges);
    if let Some(reordered) = &reordered {
        input = JavaInput::new(reordered).map_err(|e| FormatError(e.0))?;
    }
    let text = reordered.as_deref().unwrap_or(text);

    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_java::LANGUAGE.into()).map_err(|e| FormatError(e.to_string()))?;
    let tree = parser.parse(parser_text(text).as_ref(), None).ok_or_else(|| FormatError("parse failed".into()))?;
    let unit = Converter { src: text }.compilation_unit(&tree.root_node()).map_err(|e| FormatError(e.0))?;

    let mut visitor = Visitor::new(OpsBuilder::new(&input), text);
    visitor.scan(&unit);
    let mut builder = visitor.b;
    builder.sync(text.len());
    builder.drain();
    let ops = builder.build().map_err(FormatError)?;

    let (docs, root) = Docs::build(&input, ops.ops);
    let comments = CommentsHelper::new(input.line_separator, MAX_LINE_LENGTH as usize);
    let layout = Layout { docs: &docs, comments: &comments, max_width: MAX_LINE_LENGTH };
    let state = layout.compute_breaks(root, &State::starting());
    let mut output = JavaOutput::new(&input, &ops.blank_lines, &ops.partial_format_ranges);
    docs.write(root, &state, &mut output);
    output.flush();
    // Token ranges refer to the (possibly modifier-reordered) input, as in the original.
    let token_ranges = input.character_ranges_to_token_ranges(character_ranges).map_err(|e| FormatError(e.0))?;
    Ok(output.format_replacements(&token_ranges))
}

/// tree-sitter only ends line comments at `\n`; replace lone `\r` separators (same length, so positions are kept).
pub(crate) fn parser_text(text: &str) -> std::borrow::Cow<'_, str> {
    if !text.contains('\r') {
        return text.into();
    }
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    for (i, c) in text.char_indices() {
        if c == '\r' && bytes.get(i + 1) != Some(&b'\n') {
            out.push('\n');
        } else {
            out.push(c);
        }
    }
    out.into()
}
