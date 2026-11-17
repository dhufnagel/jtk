// Derived from the javadoc package of palantir-java-format (JavadocFormatter, JavadocLexer, JavadocWriter, ...)
// Copyright 2015-2019 Google Inc.; Copyright 2019-2025 Palantir Technologies Inc.
// Modified by the jtk authors: ported to Rust. Licensed under the Apache License, Version 2.0.

//! Javadoc formatting (port of the original's `javadoc` package: `JavadocFormatter`, `JavadocLexer`,
//! `JavadocWriter`, `Token`, `NestingCounter` and `CharStream`).
//!
//! The lexer turns a comment into a stream of tokens, the writer re-flows them to the line limit, and the result is
//! collapsed to a one-liner if it fits. Javadoc formatting is off by default, as in the original's Palantir style.

use std::sync::LazyLock;

use regex::Regex;

use crate::newlines::jlen;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Type {
    BeginJavadoc,
    EndJavadoc,
    FooterJavadocTagStart,
    ListOpenTag,
    ListCloseTag,
    ListItemOpenTag,
    ListItemCloseTag,
    HeaderOpenTag,
    HeaderCloseTag,
    ParagraphOpenTag,
    ParagraphCloseTag,
    BlockquoteOpenTag,
    BlockquoteCloseTag,
    PreOpenTag,
    PreCloseTag,
    CodeOpenTag,
    CodeCloseTag,
    TableOpenTag,
    TableCloseTag,
    MoeBeginStripComment,
    MoeEndStripComment,
    HtmlComment,
    BrTag,
    /// Whitespace outside `<pre>`, `<code>` and `<table>` (newlines, leading asterisks, spaces, tabs).
    Whitespace,
    /// A newline inside `<pre>`, `<code>` or `<table>`, where formatting is preserved.
    ForcedNewline,
    /// Permits but does not force a line break: it only keeps two literals from being joined.
    OptionalLineBreak,
    InlineTagOpen,
    InlineTagClose,
    Literal,
}

use Type::*;

#[derive(Clone, Debug)]
struct Token {
    ty: Type,
    value: String,
}

impl Token {
    fn new(ty: Type, value: impl Into<String>) -> Token {
        Token { ty, value: value.into() }
    }

    /// Length in UTF-16 code units, like Java's `String.length()`.
    fn length(&self) -> usize {
        jlen(&self.value)
    }
}

/// Mutable integer for tracking the level of nesting.
#[derive(Default)]
struct NestingCounter(i32);

impl NestingCounter {
    fn increment(&mut self) {
        self.0 += 1;
    }
    fn increment_if_positive(&mut self) {
        if self.0 > 0 {
            self.0 += 1;
        }
    }
    fn decrement_if_positive(&mut self) {
        if self.0 > 0 {
            self.0 -= 1;
        }
    }
    fn is_positive(&self) -> bool {
        self.0 > 0
    }
    fn reset(&mut self) {
        self.0 = 0;
    }
}

// Java's `\s` and `\w` are ASCII-only by default; Rust's are Unicode-aware, so they are spelled out.
const WS: &str = r"[ \t\n\x0B\x0C\r]";
const WORD: &str = r"[A-Za-z0-9_]";

fn re(pattern: &str) -> Regex {
    Regex::new(pattern).unwrap_or_else(|e| panic!("invalid regex {pattern}: {e}"))
}

fn open_tag(name: &str) -> Regex {
    re(&format!(r"(?i)^<(?:{name})\b[^>]*>"))
}

fn close_tag(name: &str) -> Regex {
    re(&format!(r"(?i)^</(?:{name})\b[^>]*>"))
}

struct Patterns {
    newline: Regex,
    footer_tag: Regex,
    moe_begin_strip_comment: Regex,
    moe_end_strip_comment: Regex,
    html_comment: Regex,
    pre_open: Regex,
    pre_close: Regex,
    code_open: Regex,
    code_close: Regex,
    table_open: Regex,
    table_close: Regex,
    list_open: Regex,
    list_close: Regex,
    list_item_open: Regex,
    list_item_close: Regex,
    header_open: Regex,
    header_close: Regex,
    paragraph_open: Regex,
    paragraph_close: Regex,
    blockquote_open: Regex,
    blockquote_close: Regex,
    br: Regex,
    inline_tag_open: Regex,
    literal: Regex,
    simple_tag: Regex,
    one_content_line: Regex,
    href: Regex,
    pre_code: Regex,
}

static PATTERNS: LazyLock<Patterns> = LazyLock::new(|| Patterns {
    newline: re(r"^[ \t]*\n[ \t]*[*]?[ \t]?"),
    // Only tags starting with a lowercase letter, to avoid matching annotations in code blocks; `@param <T>` is
    // matched specially in case the `<T>` is an HTML tag that is treated specially.
    footer_tag: re(&format!(r"^@(param{WS}+<{WORD}+>|[a-z]{WORD}*)")),
    moe_begin_strip_comment: re(&format!(r"^<!--{WS}*MOE:begin_intracomment_strip{WS}*-->")),
    moe_end_strip_comment: re(&format!(r"^<!--{WS}*MOE:end_intracomment_strip{WS}*-->")),
    html_comment: re(r"(?s)^<!--.*?-->"),
    pre_open: open_tag("pre"),
    pre_close: close_tag("pre"),
    code_open: open_tag("code"),
    code_close: close_tag("code"),
    table_open: open_tag("table"),
    table_close: close_tag("table"),
    list_open: open_tag("ul|ol|dl"),
    list_close: close_tag("ul|ol|dl"),
    list_item_open: open_tag("li|dt|dd"),
    list_item_close: close_tag("li|dt|dd"),
    header_open: open_tag("h[1-6]"),
    header_close: close_tag("h[1-6]"),
    paragraph_open: open_tag("p"),
    paragraph_close: close_tag("p"),
    blockquote_open: open_tag("blockquote"),
    blockquote_close: close_tag("blockquote"),
    br: open_tag("br"),
    inline_tag_open: re(&format!(r"^\{{@{WORD}*")),
    // Excludes `<` (so following HTML tags are not swallowed), braces, `@` and `*`.
    literal: re(r"(?s)^.[^ \t\n@<{}*]*"),
    simple_tag: re(&format!(r"(?i)^<{WORD}+{WS}*/?{WS}*>$")),
    // Java's `.` does not match any line terminator.
    one_content_line: re(r"^ */[*][*]\n *[*] ([^\n\r\x{85}\x{2028}\x{2029}]*)\n *[*]/$"),
    href: re(r"^href=[^>]*>$"),
    pre_code: re(r"^[ \t]*[{]@code$"),
});

/// Unbalanced `{@`, `<pre>`, `<code>` or `<table>`: the comment is left as it is.
struct LexError;

/// Port of `CharStream`: the lexer tries patterns, then reads what the last successful one matched.
struct CharStream {
    remaining: String,
    to_consume: usize,
}

impl CharStream {
    fn try_consume(&mut self, expected: &str) -> bool {
        if !self.remaining.starts_with(expected) {
            return false;
        }
        self.to_consume = expected.len();
        true
    }

    fn try_consume_regex(&mut self, pattern: &Regex) -> bool {
        match pattern.find(&self.remaining) {
            Some(m) => {
                debug_assert_eq!(m.start(), 0);
                self.to_consume = m.end();
                true
            }
            None => false,
        }
    }

    fn read_and_reset_recorded(&mut self) -> String {
        let rest = self.remaining.split_off(self.to_consume);
        let result = std::mem::replace(&mut self.remaining, rest);
        self.to_consume = 0;
        result
    }

    fn is_exhausted(&self) -> bool {
        self.remaining.is_empty()
    }
}

struct Lexer {
    input: CharStream,
    brace_depth: NestingCounter,
    pre_depth: NestingCounter,
    code_depth: NestingCounter,
    table_depth: NestingCounter,
    something_since_newline: bool,
}

fn lex(input: &str) -> Result<Vec<Token>, LexError> {
    // Strip `/**` and `*/` up front so that consuming a newline cannot swallow the `*/`.
    let inner = &input[3..input.len() - 2];
    let normalized = inner.replace("\r\n", "\n").replace('\r', "\n");
    let mut lexer = Lexer {
        input: CharStream { remaining: normalized, to_consume: 0 },
        brace_depth: NestingCounter::default(),
        pre_depth: NestingCounter::default(),
        code_depth: NestingCounter::default(),
        table_depth: NestingCounter::default(),
        something_since_newline: false,
    };
    lexer.generate_tokens()
}

impl Lexer {
    fn generate_tokens(&mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = vec![Token::new(BeginJavadoc, "/**")];
        while !self.input.is_exhausted() {
            let ty = self.consume_token()?;
            let value = self.input.read_and_reset_recorded();
            tokens.push(Token::new(ty, value));
        }
        self.check_matching_tags()?;
        tokens.push(Token::new(EndJavadoc, "*/"));
        let tokens = join_adjacent_literals_and_adjacent_whitespace(tokens);
        let tokens = infer_paragraph_tags(tokens);
        let tokens = optionalize_spaces_after_links(tokens);
        Ok(deindent_pre_code_blocks(tokens))
    }

    fn preserve_existing_formatting(&self) -> bool {
        self.pre_depth.is_positive() || self.table_depth.is_positive() || self.code_depth.is_positive()
    }

    fn check_matching_tags(&self) -> Result<(), LexError> {
        if self.brace_depth.is_positive()
            || self.pre_depth.is_positive()
            || self.table_depth.is_positive()
            || self.code_depth.is_positive()
        {
            return Err(LexError);
        }
        Ok(())
    }

    fn consume_token(&mut self) -> Result<Type, LexError> {
        let p = &*PATTERNS;
        let preserve = self.preserve_existing_formatting();
        let input = &mut self.input;
        if input.try_consume_regex(&p.newline) {
            self.something_since_newline = false;
            return Ok(if preserve { ForcedNewline } else { Whitespace });
        } else if input.try_consume(" ") || input.try_consume("\t") {
            return Ok(if preserve { Literal } else { Whitespace });
        }
        if !self.something_since_newline && input.try_consume_regex(&p.footer_tag) {
            self.check_matching_tags()?;
            self.something_since_newline = true;
            return Ok(FooterJavadocTagStart);
        }
        self.something_since_newline = true;
        let input = &mut self.input;
        if input.try_consume_regex(&p.inline_tag_open) {
            self.brace_depth.increment();
            return Ok(InlineTagOpen);
        } else if input.try_consume("{") {
            self.brace_depth.increment_if_positive();
            return Ok(Literal);
        } else if input.try_consume("}") {
            self.brace_depth.decrement_if_positive();
            return Ok(if self.brace_depth.is_positive() { Literal } else { InlineTagClose });
        }
        // Inside an inline tag, no HTML is interpreted.
        if self.brace_depth.is_positive() {
            let matched = input.try_consume_regex(&p.literal);
            debug_assert!(matched);
            return Ok(Literal);
        }
        if input.try_consume_regex(&p.pre_open) {
            self.pre_depth.increment();
            return Ok(if preserve { Literal } else { PreOpenTag });
        } else if input.try_consume_regex(&p.pre_close) {
            self.pre_depth.decrement_if_positive();
            return Ok(if self.preserve_existing_formatting() { Literal } else { PreCloseTag });
        }
        let input = &mut self.input;
        if input.try_consume_regex(&p.code_open) {
            self.code_depth.increment();
            return Ok(if preserve { Literal } else { CodeOpenTag });
        } else if input.try_consume_regex(&p.code_close) {
            self.code_depth.decrement_if_positive();
            return Ok(if self.preserve_existing_formatting() { Literal } else { CodeCloseTag });
        }
        let input = &mut self.input;
        if input.try_consume_regex(&p.table_open) {
            self.table_depth.increment();
            return Ok(if preserve { Literal } else { TableOpenTag });
        } else if input.try_consume_regex(&p.table_close) {
            self.table_depth.decrement_if_positive();
            return Ok(if self.preserve_existing_formatting() { Literal } else { TableCloseTag });
        }
        let input = &mut self.input;
        if preserve {
            let matched = input.try_consume_regex(&p.literal);
            debug_assert!(matched);
            return Ok(Literal);
        }
        let rules: [(&Regex, Type); 15] = [
            (&p.paragraph_open, ParagraphOpenTag),
            (&p.paragraph_close, ParagraphCloseTag),
            (&p.list_open, ListOpenTag),
            (&p.list_close, ListCloseTag),
            (&p.list_item_open, ListItemOpenTag),
            (&p.list_item_close, ListItemCloseTag),
            (&p.blockquote_open, BlockquoteOpenTag),
            (&p.blockquote_close, BlockquoteCloseTag),
            (&p.header_open, HeaderOpenTag),
            (&p.header_close, HeaderCloseTag),
            (&p.br, BrTag),
            (&p.moe_begin_strip_comment, MoeBeginStripComment),
            (&p.moe_end_strip_comment, MoeEndStripComment),
            (&p.html_comment, HtmlComment),
            (&p.literal, Literal),
        ];
        for (pattern, ty) in rules {
            if input.try_consume_regex(pattern) {
                return Ok(ty);
            }
        }
        // LITERAL_PATTERN matches any non-empty input.
        Err(LexError)
    }
}

/// Joins adjacent literal tokens (e.g. `<b>`, `foo`, `</b>`) and adjacent whitespace tokens. A literal starting with
/// `@` after whitespace is joined to the previous literal so that no line break turns it into a block tag.
fn join_adjacent_literals_and_adjacent_whitespace(input: Vec<Token>) -> Vec<Token> {
    let mut output = Vec::new();
    let mut accumulated = String::new();
    let mut last_tag_was_inline_tag_start = false;
    let mut i = 0;
    // The last token is always END_JAVADOC, so peeking never runs past the end.
    while i < input.len() {
        let next_type = input[i].ty;
        if next_type == InlineTagOpen {
            last_tag_was_inline_tag_start = true;
        }
        if matches!(next_type, Literal | InlineTagOpen | InlineTagClose) {
            accumulated.push_str(&input[i].value);
            i += 1;
            continue;
        }
        if accumulated.is_empty() {
            output.push(input[i].clone());
            i += 1;
            continue;
        }
        let mut seen_whitespace = String::new();
        while input[i].ty == Whitespace {
            seen_whitespace.push_str(&input[i].value);
            i += 1;
        }
        if input[i].ty == Literal && (input[i].value.starts_with('@') || last_tag_was_inline_tag_start) {
            last_tag_was_inline_tag_start = false;
            accumulated.push(' ');
            accumulated.push_str(&input[i].value);
            i += 1;
            continue;
        }
        output.push(Token::new(Literal, std::mem::take(&mut accumulated)));
        if !seen_whitespace.is_empty() {
            output.push(Token::new(Whitespace, seen_whitespace));
        }
        last_tag_was_inline_tag_start = false;
    }
    output
}

/// Inserts a `<p>` between literals separated by two or more line breaks.
fn infer_paragraph_tags(input: Vec<Token>) -> Vec<Token> {
    let mut output = Vec::new();
    let mut i = 0;
    while i < input.len() {
        if input[i].ty == Literal {
            output.push(input[i].clone());
            i += 1;
            if input[i].ty == Whitespace && input[i].value.matches('\n').count() > 1 {
                output.push(input[i].clone());
                i += 1;
                if input[i].ty == Literal {
                    output.push(Token::new(ParagraphOpenTag, "<p>"));
                }
            }
        } else {
            output.push(input[i].clone());
            i += 1;
        }
    }
    output
}

/// Turns whitespace after `href=...>` into an optional line break, so `<a href=foo>foo</a>` may be split.
fn optionalize_spaces_after_links(input: Vec<Token>) -> Vec<Token> {
    let mut output = Vec::new();
    let mut i = 0;
    while i < input.len() {
        if input[i].ty == Literal && PATTERNS.href.is_match(&input[i].value) {
            output.push(input[i].clone());
            i += 1;
            if input[i].ty == Whitespace {
                output.push(Token::new(OptionalLineBreak, input[i].value.clone()));
                i += 1;
            }
        } else {
            output.push(input[i].clone());
            i += 1;
        }
    }
    output
}

/// Removes common indentation inside `<pre>{@code` blocks, trims blank lines at their start and end, and moves the
/// closing `}` onto its own line.
fn deindent_pre_code_blocks(input: Vec<Token>) -> Vec<Token> {
    let mut output = Vec::new();
    let mut i = 0;
    while i < input.len() {
        if input[i].ty != PreOpenTag {
            output.push(input[i].clone());
            i += 1;
            continue;
        }
        output.push(input[i].clone());
        i += 1;
        let mut initial_newlines = Vec::new();
        while i < input.len() && input[i].ty == ForcedNewline {
            initial_newlines.push(input[i].clone());
            i += 1;
        }
        if input[i].ty != Literal || !PATTERNS.pre_code.is_match(&input[i].value) {
            output.extend(initial_newlines);
            output.push(input[i].clone());
            i += 1;
            continue;
        }
        output.push(Token::new(Literal, java_trim(&input[i].value)));
        i += 1;
        let mut saved = std::collections::VecDeque::new();
        while i < input.len() && input[i].ty != PreCloseTag {
            saved.push_back(input[i].clone());
            i += 1;
        }
        while saved.front().is_some_and(|t: &Token| t.ty == ForcedNewline) {
            saved.pop_front();
        }
        while saved.back().is_some_and(|t: &Token| t.ty == ForcedNewline) {
            saved.pop_back();
        }
        if saved.is_empty() {
            continue;
        }
        let mut trailing_brace = false;
        let last = saved.back().unwrap().clone();
        if last.ty == Literal && last.value.ends_with('}') {
            saved.pop_back();
            if last.length() > 1 {
                saved.push_back(Token::new(Literal, &last.value[..last.value.len() - 1]));
                saved.push_back(Token::new(ForcedNewline, ""));
            }
            trailing_brace = true;
        }
        let mut trim: Option<usize> = None;
        for t in &saved {
            if t.ty == Literal
                && let Some(idx) = t.value.find(|c| c != ' ')
                && trim.is_none_or(|current| idx < current)
            {
                trim = Some(idx);
            }
        }
        output.push(Token::new(ForcedNewline, "\n"));
        for t in saved {
            match trim {
                Some(trim) if t.ty == Literal && trim > 0 && t.length() > trim => {
                    // The trimmed prefix consists of spaces, so byte and UTF-16 offsets agree.
                    output.push(Token::new(Literal, &t.value[trim..]));
                }
                _ => output.push(t),
            }
        }
        if trailing_brace {
            output.push(Token::new(Literal, "}"));
        } else {
            output.push(Token::new(ForcedNewline, "\n"));
        }
    }
    output
}

/// Java's `String.trim()`: strips characters up to and including U+0020.
fn java_trim(s: &str) -> &str {
    s.trim_matches(|c: char| c <= ' ')
}

// ------------------------------------------------------------------------------------------------------------------
// Writer
// ------------------------------------------------------------------------------------------------------------------

/// Requested whitespace between two tokens, from lowest to highest priority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum RequestedWhitespace {
    None,
    Whitespace,
    Newline,
    BlankLine,
}

struct Writer {
    block_indent: usize,
    max_line_length: i32,
    output: String,
    continuing_list_item_of_innermost_list: bool,
    continuing_footer_tag: bool,
    continuing_list_item_count: NestingCounter,
    continuing_list_count: NestingCounter,
    post_write_modified_continuing_list_count: NestingCounter,
    remaining_on_line: i32,
    at_start_of_line: bool,
    requested_whitespace: RequestedWhitespace,
    requested_moe_begin_strip_comment: Option<Token>,
    indent_for_moe_end_strip_comment: usize,
    wrote_anything_significant: bool,
}

impl Writer {
    fn new(block_indent: usize, max_line_length: i32) -> Writer {
        Writer {
            block_indent,
            max_line_length,
            output: String::new(),
            continuing_list_item_of_innermost_list: false,
            continuing_footer_tag: false,
            continuing_list_item_count: NestingCounter::default(),
            continuing_list_count: NestingCounter::default(),
            post_write_modified_continuing_list_count: NestingCounter::default(),
            remaining_on_line: 0,
            at_start_of_line: false,
            requested_whitespace: RequestedWhitespace::None,
            requested_moe_begin_strip_comment: None,
            indent_for_moe_end_strip_comment: 0,
            wrote_anything_significant: false,
        }
    }

    fn request_whitespace(&mut self, w: RequestedWhitespace) {
        self.requested_whitespace = self.requested_whitespace.max(w);
    }

    fn request_blank_line(&mut self) {
        self.request_whitespace(RequestedWhitespace::BlankLine);
    }

    fn request_newline(&mut self) {
        self.request_whitespace(RequestedWhitespace::Newline);
    }

    fn write_begin_javadoc(&mut self) {
        self.output.push_str("/**");
        self.write_newline(true);
    }

    fn write_end_javadoc(&mut self) {
        self.output.push('\n');
        self.append_spaces(self.block_indent + 1);
        self.output.push_str("*/");
    }

    fn write_footer_javadoc_tag_start(&mut self, token: &Token) {
        // Close any unclosed lists.
        self.continuing_list_item_of_innermost_list = false;
        self.continuing_list_item_count.reset();
        self.continuing_list_count.reset();
        self.post_write_modified_continuing_list_count.reset();
        if !self.wrote_anything_significant {
            // Javadoc consisting solely of tags.
        } else if !self.continuing_footer_tag {
            // First footer tag after the body.
            self.request_blank_line();
        } else {
            self.continuing_footer_tag = false;
            self.request_newline();
        }
        self.write_token(token);
        self.continuing_footer_tag = true;
    }

    fn write_list_open(&mut self, token: &Token) {
        self.request_blank_line();
        self.write_token(token);
        self.continuing_list_item_of_innermost_list = false;
        self.continuing_list_count.increment();
        self.post_write_modified_continuing_list_count.increment();
        self.request_newline();
    }

    fn write_list_close(&mut self, token: &Token) {
        self.request_newline();
        self.continuing_list_item_count.decrement_if_positive();
        self.continuing_list_count.decrement_if_positive();
        self.write_token(token);
        self.post_write_modified_continuing_list_count.decrement_if_positive();
        self.request_blank_line();
    }

    fn write_list_item_open(&mut self, token: &Token) {
        self.request_newline();
        if self.continuing_list_item_of_innermost_list {
            self.continuing_list_item_of_innermost_list = false;
            self.continuing_list_item_count.decrement_if_positive();
        }
        self.write_token(token);
        self.continuing_list_item_of_innermost_list = true;
        self.continuing_list_item_count.increment();
    }

    fn write_paragraph_open(&mut self, token: &Token) {
        if !self.wrote_anything_significant {
            // An initial <p> is dropped.
            return;
        }
        self.request_blank_line();
        self.write_token(token);
    }

    /// For headers, `<pre>` and `<table>`: a blank line before the open tag and after the close tag.
    fn write_block_open(&mut self, token: &Token) {
        self.request_blank_line();
        self.write_token(token);
    }

    fn write_block_close(&mut self, token: &Token) {
        self.write_token(token);
        self.request_blank_line();
    }

    fn write_blockquote_open_or_close(&mut self, token: &Token) {
        self.request_blank_line();
        self.write_token(token);
        self.request_blank_line();
    }

    fn write_moe_end_strip_comment(&mut self, token: &Token) {
        self.write_newline(false);
        self.append_spaces(self.indent_for_moe_end_strip_comment);
        self.write_token(token);
        self.request_newline();
    }

    fn write_html_comment(&mut self, token: &Token) {
        self.request_newline();
        self.write_token(token);
        self.request_newline();
    }

    fn write_br(&mut self, token: &Token) {
        self.write_token(token);
        self.request_newline();
    }

    fn write_token(&mut self, token: &Token) {
        if self.requested_moe_begin_strip_comment.is_some() {
            self.request_newline();
        }
        if self.requested_whitespace == RequestedWhitespace::BlankLine
            && (self.post_write_modified_continuing_list_count.is_positive() || self.continuing_footer_tag)
        {
            // No blank lines inside lists or footer tags.
            self.requested_whitespace = RequestedWhitespace::Newline;
        }
        if self.requested_whitespace == RequestedWhitespace::BlankLine {
            self.write_blank_line();
            self.requested_whitespace = RequestedWhitespace::None;
        } else if self.requested_whitespace == RequestedWhitespace::Newline {
            self.write_newline(true);
            self.requested_whitespace = RequestedWhitespace::None;
        }
        let need_whitespace = self.requested_whitespace == RequestedWhitespace::Whitespace;
        // Wrap to respect the line limit (not at the start of a line, where a newline would not help).
        if !self.at_start_of_line && token.length() as i32 + i32::from(need_whitespace) > self.remaining_on_line {
            self.write_newline(true);
        }
        if !self.at_start_of_line && need_whitespace {
            self.output.push(' ');
            self.remaining_on_line -= 1;
        }
        if let Some(moe) = self.requested_moe_begin_strip_comment.take() {
            self.output.push_str(&moe.value);
            self.indent_for_moe_end_strip_comment = self.inner_indent();
            self.request_newline();
            self.write_token(token);
            return;
        }
        self.output.push_str(&token.value);
        if !matches!(token.ty, ListItemOpenTag | ParagraphOpenTag | HeaderOpenTag) {
            self.at_start_of_line = false;
        }
        self.remaining_on_line -= token.length() as i32;
        self.requested_whitespace = RequestedWhitespace::None;
        self.wrote_anything_significant = true;
    }

    fn write_blank_line(&mut self) {
        self.output.push('\n');
        self.append_spaces(self.block_indent + 1);
        self.output.push('*');
        self.write_newline(true);
    }

    fn write_newline(&mut self, auto_indent: bool) {
        self.output.push('\n');
        self.append_spaces(self.block_indent + 1);
        self.output.push('*');
        self.append_spaces(1);
        self.remaining_on_line = self.max_line_length - self.block_indent as i32 - 3;
        if auto_indent {
            let inner = self.inner_indent();
            self.append_spaces(inner);
            self.remaining_on_line -= inner as i32;
        }
        self.at_start_of_line = true;
    }

    fn inner_indent(&self) -> usize {
        let mut inner = self.continuing_list_item_count.0 * 4 + self.continuing_list_count.0 * 2;
        if self.continuing_footer_tag {
            inner += 4;
        }
        inner.max(0) as usize
    }

    fn append_spaces(&mut self, count: usize) {
        self.output.extend(std::iter::repeat_n(' ', count));
    }
}

/// Formats a Javadoc comment (starting with `/**` and ending with `*/`) that starts at column `block_indent`.
pub fn format_javadoc(input: &str, block_indent: usize, max_line_length: i32) -> String {
    if !(input.starts_with("/**") && input.ends_with("*/") && input.len() > 4) {
        return input.to_string();
    }
    let Ok(tokens) = lex(input) else {
        return input.to_string();
    };
    let result = render(&tokens, block_indent, max_line_length);
    make_single_line_if_possible(block_indent, max_line_length, result)
}

fn render(tokens: &[Token], block_indent: usize, max_line_length: i32) -> String {
    let mut w = Writer::new(block_indent, max_line_length);
    for token in tokens {
        match token.ty {
            BeginJavadoc => w.write_begin_javadoc(),
            EndJavadoc => {
                w.write_end_javadoc();
                return w.output;
            }
            FooterJavadocTagStart => w.write_footer_javadoc_tag_start(token),
            ListOpenTag => w.write_list_open(token),
            ListCloseTag => w.write_list_close(token),
            ListItemOpenTag => w.write_list_item_open(token),
            HeaderOpenTag | PreOpenTag | TableOpenTag => w.write_block_open(token),
            HeaderCloseTag | PreCloseTag | TableCloseTag => w.write_block_close(token),
            ParagraphOpenTag => w.write_paragraph_open(&standardize(token, "<p>")),
            BlockquoteOpenTag | BlockquoteCloseTag => w.write_blockquote_open_or_close(token),
            CodeOpenTag | CodeCloseTag | Literal | InlineTagOpen | InlineTagClose => w.write_token(token),
            MoeBeginStripComment => w.requested_moe_begin_strip_comment = Some(token.clone()),
            MoeEndStripComment => w.write_moe_end_strip_comment(token),
            HtmlComment => w.write_html_comment(token),
            BrTag => w.write_br(&standardize(token, "<br>")),
            Whitespace => w.request_whitespace(RequestedWhitespace::Whitespace),
            ForcedNewline => w.write_newline(false),
            ParagraphCloseTag | ListItemCloseTag | OptionalLineBreak => {}
        }
    }
    w.output
}

/// Replaces simple variants like `<P>` or `<br/>` with the standard spelling.
fn standardize(token: &Token, standard: &str) -> Token {
    if PATTERNS.simple_tag.is_match(&token.value) { Token::new(token.ty, standard) } else { token.clone() }
}

/// Collapses the comment to `/** text */` (or `/** */`) if its content is a single line that fits.
fn make_single_line_if_possible(block_indent: usize, max_line_length: i32, input: String) -> String {
    let one_liner_content_length = max_line_length - "/**  */".len() as i32 - block_indent as i32;
    if let Some(caps) = PATTERNS.one_content_line.captures(&input) {
        let content = &caps[1];
        if content.is_empty() {
            return "/** */".to_string();
        }
        if jlen(content) as i32 <= one_liner_content_length {
            return format!("/** {content} */");
        }
    }
    input
}
