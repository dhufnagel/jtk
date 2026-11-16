//! Entry point tying the pipeline together (port of `Formatter.java`).

use crate::ast::Converter;
use crate::comments::CommentsHelper;
use crate::doc::{Docs, Layout, State};
use crate::input::JavaInput;
use crate::ops::OpsBuilder;
use crate::output::{JavaOutput, Replacement};
use crate::visitor::Visitor;
use crate::{imports, modifiers, parse, string_wrapper};

pub const MAX_LINE_LENGTH: i32 = 120;

/// A formatting error, optionally with a 1-based line and column (in UTF-16 units, like javac).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatError {
    pub position: Option<(usize, usize)>,
    pub message: String,
}

impl FormatError {
    pub fn new(message: impl Into<String>) -> FormatError {
        FormatError { position: None, message: message.into() }
    }

    pub fn at(line: usize, column: usize, message: impl Into<String>) -> FormatError {
        FormatError { position: Some((line, column)), message: message.into() }
    }

    /// An error at a byte offset of `text`.
    pub fn at_offset(text: &str, offset: usize, message: impl Into<String>) -> FormatError {
        let offset = offset.min(text.len());
        let line_start = text[..offset].rfind(['\n', '\r']).map_or(0, |i| i + 1);
        let line = crate::newlines::count(&text[..line_start]) + 1;
        FormatError::at(line, crate::newlines::jlen(&text[line_start..offset]) + 1, message)
    }
}

/// `line:column: error: message`, like the original's diagnostics.
impl std::fmt::Display for FormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.position {
            Some((line, column)) => write!(f, "{line}:{column}: error: {}", self.message),
            None => write!(f, "error: {}", self.message),
        }
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
    format_file(text, &[(0, text.len())], options)
}

/// Like [`format`], but only formats the given byte ranges (extended to whole statements or declarations). As in
/// the original, imports are still fixed and strings reflowed in the whole file.
pub fn format_file(text: &str, ranges: &[(usize, usize)], options: &FormatOptions) -> Result<String, FormatError> {
    with_large_stack(|| format_on_current_thread(text, ranges, options))
}

/// Only removes unused imports and sorts imports (as enabled in `options`), without other formatting.
pub fn fix_imports(text: &str, options: &FormatOptions) -> Result<String, FormatError> {
    with_large_stack(|| {
        let mut text = text.to_string();
        if options.remove_unused_imports {
            text = imports::remove_unused_imports(&text);
        }
        if options.sort_imports {
            text = imports::reorder_imports(&text).map_err(FormatError::new)?;
        }
        Ok(text)
    })
}

/// The replacements that format the given byte ranges, like `--output-replacements` of the original: import fixing
/// is not applied, but if string reflow changes the text, a single replacement of the whole input is returned.
pub fn format_replacements_with_reflow(
    text: &str,
    ranges: &[(usize, usize)],
    options: &FormatOptions,
) -> Result<Vec<Replacement>, FormatError> {
    with_large_stack(|| {
        let replacements = format_replacements(text, ranges)?;
        if !options.reflow_long_strings {
            return Ok(replacements);
        }
        let formatted = apply_replacements(text, &replacements);
        if !string_wrapper::lines_need_wrapping(MAX_LINE_LENGTH as usize, &formatted, ranges) {
            return Ok(replacements);
        }
        let wrapped = string_wrapper::wrap(MAX_LINE_LENGTH as usize, &formatted, format_ranges)?;
        if wrapped == formatted {
            return Ok(replacements);
        }
        Ok(vec![Replacement { start: 0, end: text.len(), text: wrapped }])
    })
}

fn apply_replacements(text: &str, replacements: &[Replacement]) -> String {
    let mut result = text.to_string();
    let mut sorted: Vec<&Replacement> = replacements.iter().collect();
    sorted.sort_by_key(|r| std::cmp::Reverse(r.start));
    for r in sorted {
        result.replace_range(r.start..r.end, &r.text);
    }
    result
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

fn format_on_current_thread(
    text: &str,
    ranges: &[(usize, usize)],
    options: &FormatOptions,
) -> Result<String, FormatError> {
    let mut formatted = format_ranges(text, ranges)?;
    if options.remove_unused_imports {
        // Formatting does not change which names are used, so the input's (cached) syntax tree is analyzed.
        if let Some(used) = imports::used_import_names(text) {
            formatted = imports::remove_imports(&formatted, &used);
        }
    }
    if options.sort_imports {
        formatted = imports::reorder_imports(&formatted).map_err(FormatError::new)?;
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
    Ok(apply_replacements(text, &format_replacements(text, character_ranges)?))
}

/// The replacements that format the given byte ranges of the input.
pub fn format_replacements(text: &str, character_ranges: &[(usize, usize)]) -> Result<Vec<Replacement>, FormatError> {
    let mut input = JavaInput::new(text)?;
    let token_ranges = input.character_ranges_to_token_ranges(character_ranges)?;
    let reordered = modifiers::reorder_modifiers(&input, &token_ranges);
    if let Some(reordered) = &reordered {
        input = JavaInput::new(reordered)?;
    }
    let text = reordered.as_deref().unwrap_or(text);
    let tree = parse::parse(text).ok_or_else(|| FormatError::new("parse failed"))?;
    let unit = Converter { src: text }.compilation_unit(&tree.root_node())?;

    let mut visitor = Visitor::new(OpsBuilder::new(&input), text);
    visitor.scan(&unit);
    let mut builder = visitor.b;
    builder.sync(text.len());
    builder.drain();
    let ops = builder.build()?;
    let (docs, root) = Docs::build(&input, ops.ops);
    let comments = CommentsHelper::new(input.line_separator, MAX_LINE_LENGTH as usize);
    let layout = Layout { docs: &docs, comments: &comments, max_width: MAX_LINE_LENGTH };
    let state = layout.compute_breaks(root, &State::starting());
    let mut output = JavaOutput::new(&input, &ops.blank_lines, &ops.partial_format_ranges);
    docs.write(root, &state, &mut output);
    output.flush();
    // Token ranges refer to the (possibly modifier-reordered) input, as in the original.
    let token_ranges = input.character_ranges_to_token_ranges(character_ranges)?;
    Ok(output.format_replacements(&token_ranges))
}
