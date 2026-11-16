//! Parsing with tree-sitter, shared by all stages of the pipeline.
//!
//! Formatting, removing unused imports and reflowing strings each need a syntax tree, often of the same text (an
//! already formatted file, or one without unused imports). The last tree is cached per thread and reused when the
//! text is unchanged.

use std::cell::RefCell;

use tree_sitter::{Parser, Tree};

thread_local! {
    static PARSER: RefCell<Parser> = RefCell::new({
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_java::LANGUAGE.into()).expect("tree-sitter-java version mismatch");
        parser
    });
    static LAST: RefCell<Option<(String, Tree)>> = const { RefCell::new(None) };
}

/// Parses Java source, reusing the previous tree if `text` is unchanged.
pub fn parse(text: &str) -> Option<Tree> {
    if let Some(tree) = LAST.with_borrow(|last| last.as_ref().filter(|(t, _)| t == text).map(|(_, tree)| tree.clone()))
    {
        return Some(tree);
    }
    let tree = PARSER.with_borrow_mut(|parser| parser.parse(parser_text(text).as_ref(), None))?;
    LAST.set(Some((text.to_string(), tree.clone())));
    Some(tree)
}

/// tree-sitter only ends line comments at `\n`; replace lone `\r` separators (same length, so positions are kept).
pub fn parser_text(text: &str) -> std::borrow::Cow<'_, str> {
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
