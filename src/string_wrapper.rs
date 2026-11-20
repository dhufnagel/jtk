// Derived from palantir-java-format's StringWrapper.java
// Copyright 2015-2019 Google Inc.; Copyright 2019-2025 Palantir Technologies Inc.
// Modified by the jtk authors: ported to Rust. Licensed under the Apache License, Version 2.0.

//! Port of `StringWrapper.java`: wraps string literals that exceed the column limit, and re-indents text blocks.
//!
//! The original re-formats only the replaced ranges; formatting the whole (already formatted) file is equivalent.

use std::collections::HashMap;

use crate::newlines::{self, jlen};
use jtk_parser::ast::{K, LitKind, Tree, VarDecl};

const TEXT_BLOCK_DELIMITER: &str = "\"\"\"";

type Replacements = Vec<(usize, usize, String)>;

/// Reflows string literals in the given (formatted) source that extend past the given column limit.
///
/// `format` formats the given byte ranges of a source (see [`crate::formatter::format_ranges`]).
pub fn wrap<E>(
    column_limit: usize,
    input: &str,
    format: impl Fn(&str, &[(usize, usize)]) -> Result<String, E>,
) -> Result<String, E> {
    if !need_wrapping(column_limit, input) {
        return Ok(input.to_string());
    }
    let mut input = input.to_string();
    let mut replacements = reflow_replacements(column_limit, &input);
    if replacements.iter().all(|(start, end, text)| input[*start..*end] == *text) {
        // Nothing to change (often text blocks that are indented already). The passes below would only format the
        // ranges of the input, which is formatted already, and so return it unchanged.
        return Ok(input);
    }
    let first_pass = format(&input, &ranges(&replacements))?;
    if first_pass != input {
        // Formatting the replacement ranges changed something: recompute the replacements on the new input.
        input = first_pass;
        replacements = reflow_replacements(column_limit, &input);
    }
    let mut result = apply_replacements(&input, &replacements);
    // Format again, because broken strings might now fit on the first line in case of assignments.
    let second_pass = format(&result, &ranges_after_applied_replacements(&replacements))?;
    if second_pass != result {
        replacements = reflow_replacements(column_limit, &second_pass);
        result = apply_replacements(&second_pass, &replacements);
    }
    Ok(result)
}

fn sorted(replacements: &Replacements) -> Vec<&(usize, usize, String)> {
    let mut sorted: Vec<&(usize, usize, String)> = replacements.iter().collect();
    sorted.sort_by_key(|r| r.0);
    sorted
}

fn ranges(replacements: &Replacements) -> Vec<(usize, usize)> {
    sorted(replacements).iter().map(|r| (r.0, r.1)).collect()
}

/// The ranges the replacements occupy after they have been applied.
fn ranges_after_applied_replacements(replacements: &Replacements) -> Vec<(usize, usize)> {
    let mut result = Vec::new();
    let mut offset: isize = 0;
    for (start, end, text) in sorted(replacements) {
        let lower = (offset + *start as isize) as usize;
        let upper = lower + text.len();
        result.push((lower, upper));
        offset += text.len() as isize - (*end - *start) as isize;
    }
    result
}

/// Whether reflowing could change anything: there is a text block, or a string literal on an overlong line. (The
/// original only checks for overlong lines; when none contains a string, it computes no replacements and leaves the
/// input unchanged, so the stricter check here is equivalent and avoids parsing.)
fn need_wrapping(column_limit: usize, input: &str) -> bool {
    newlines::lines(input)
        .iter()
        .any(|line| line.contains(TEXT_BLOCK_DELIMITER) || (line.contains('"') && jlen(line) > column_limit))
}

/// Whether the lines touching `ranges` (byte ranges) exceed the column limit or belong to a text block (port of
/// `StringWrapper.linesNeedWrapping`, used for `--output-replacements`).
pub fn lines_need_wrapping(column_limit: usize, input: &str, ranges: &[(usize, usize)]) -> bool {
    let offsets = newlines::line_offsets(input);
    let mut inside_text_block = false;
    let mut lines_to_change = Vec::new();
    for (i, line) in newlines::lines(input).iter().enumerate() {
        let mut change = jlen(line) > column_limit;
        if line.contains(TEXT_BLOCK_DELIMITER) {
            inside_text_block = !inside_text_block;
            change = true;
        } else if inside_text_block {
            change = true;
        }
        if change {
            lines_to_change.push(i);
        }
    }
    // Lines as byte ranges, excluding the trailing line break (`Utils.lineRangesToCharRanges`).
    let line_end = |i: usize| offsets.get(i + 1).map_or(input.len() + 1, |&o| o) - 1;
    lines_to_change.iter().filter(|&&i| i < offsets.len()).any(|&i| {
        let (start, end) = (offsets[i], line_end(i));
        ranges.iter().any(|&(lo, hi)| lo.max(start) < hi.min(end))
    })
}

/// `TreeRangeMap.put`: a new replacement overrides any overlapping ones (in practice, identical ranges produced by
/// several long literals in the same concatenation).
fn put(replacements: &mut Replacements, start: usize, end: usize, value: String) {
    replacements.retain(|(s, e, _)| *e <= start || *s >= end);
    replacements.push((start, end, value));
}

fn apply_replacements(input: &str, replacements: &Replacements) -> String {
    let mut sorted: Vec<&(usize, usize, String)> = replacements.iter().collect();
    sorted.sort_by_key(|r| std::cmp::Reverse(r.0));
    let mut text = input.to_string();
    for (start, end, replacement) in sorted {
        text.replace_range(*start..*end, replacement);
    }
    text
}

struct LineMap {
    starts: Vec<usize>,
}

impl LineMap {
    /// 1-based line number.
    fn line(&self, pos: usize) -> usize {
        self.starts.partition_point(|&s| s <= pos)
    }
    fn line_start(&self, line: usize) -> usize {
        self.starts[line - 1]
    }
}

struct Reflower<'s> {
    input: &'s str,
    column_limit: usize,
    separator: &'static str,
    lines: LineMap,
}

/// A node enclosing a string literal: a tree, or a node of javac's tree that is not a tree here (a case, a catch
/// clause, an enum constant, a parameter), of which only the start matters.
#[derive(Clone, Copy)]
struct Enclosing<'t> {
    start: usize,
    tree: Option<&'t Tree>,
}

/// A string literal with the nodes enclosing it, outermost first.
struct Found<'t> {
    lit: &'t Tree,
    ancestors: Vec<Enclosing<'t>>,
}

impl<'t> Found<'t> {
    fn parent(&self) -> Option<&'t Tree> {
        self.ancestors.last()?.tree
    }

    /// The index in `ancestors` of the outermost of the `+` expressions directly enclosing the literal
    /// (`ancestors.len()`, standing for the literal itself, if its parent is not a `+` expression).
    fn concatenation(&self) -> usize {
        let mut i = self.ancestors.len();
        while i > 0 && self.ancestors[i - 1].tree.is_some_and(is_plus) {
            i -= 1;
        }
        i
    }

    /// The literal (`i == ancestors.len()`) or the tree enclosing it at `i`.
    fn tree(&self, i: usize) -> &'t Tree {
        self.ancestors.get(i).map_or(self.lit, |a| a.tree.expect("a tree"))
    }

    fn start(&self, i: usize) -> usize {
        self.ancestors.get(i).map_or(self.lit.start, |a| a.start)
    }
}

/// The state of [`Reflower::scan`].
struct Scan<'t> {
    long_strings: Vec<Found<'t>>,
    text_blocks: Vec<Found<'t>>,
    ancestors: Vec<Enclosing<'t>>,
}

impl<'t> Scan<'t> {
    fn found(&self, lit: &'t Tree) -> Found<'t> {
        Found { lit, ancestors: self.ancestors.clone() }
    }

    /// Runs `f` inside a node that is not a tree, starting at `start`.
    fn inside(&mut self, start: usize, f: impl FnOnce(&mut Self)) {
        self.ancestors.push(Enclosing { start, tree: None });
        f(self);
        self.ancestors.pop();
    }
}

fn is_string_literal(t: &Tree) -> bool {
    matches!(t.kind, K::Literal(LitKind::String))
}

fn is_plus(t: &Tree) -> bool {
    matches!(&t.kind, K::Binary { op, .. } if op == "+")
}

fn flatten_expression_tree(root: &Tree) -> Vec<&Tree> {
    let mut flat = Vec::new();
    let mut todo = vec![root];
    while let Some(first) = todo.pop() {
        match &first.kind {
            K::Binary { op, lhs, rhs } if op == "+" => {
                todo.push(rhs);
                todo.push(lhs);
            }
            _ => flat.push(first),
        }
    }
    flat
}

fn first_non_ws(s: &str) -> Option<usize> {
    s.char_indices().find(|(_, c)| !c.is_whitespace()).map(|(i, _)| i)
}

/// Java's `String.lines()`.
fn java_lines(s: &str) -> Vec<&str> {
    newlines::lines(s)
        .into_iter()
        .map(|l| l.strip_suffix("\r\n").or_else(|| l.strip_suffix('\n')).or_else(|| l.strip_suffix('\r')).unwrap_or(l))
        .collect()
}

/// Java's `String.stripIndent()` for a string without a trailing line terminator.
fn strip_indent(s: &str) -> String {
    let lines = java_lines(s);
    let lines: Vec<&str> = if lines.is_empty() { vec![""] } else { lines };
    let last = lines.len() - 1;
    let mut min = usize::MAX;
    for (i, l) in lines.iter().enumerate() {
        let blank = l.chars().all(char::is_whitespace);
        if !blank || i == last {
            let indent = l.chars().take_while(|c| c.is_whitespace()).count();
            min = min.min(indent);
        }
    }
    let mut out = Vec::new();
    for l in &lines {
        if l.chars().all(char::is_whitespace) {
            out.push(String::new());
        } else {
            let stripped: String = l.chars().skip(min).collect();
            out.push(stripped.trim_end_matches(char::is_whitespace).to_string());
        }
    }
    out.join("\n")
}

fn reflow_replacements(column_limit: usize, input: &str) -> Replacements {
    // Syntax errors are reported by the formatter.
    let Ok(unit) = jtk_parser::parser::parse_cached(input) else {
        return Vec::new();
    };
    let reflower = Reflower {
        input,
        column_limit,
        separator: newlines::guess_line_separator(input),
        lines: LineMap { starts: newlines::line_offsets(input) },
    };
    let mut scan = Scan { long_strings: Vec::new(), text_blocks: Vec::new(), ancestors: Vec::new() };
    reflower.scan(&unit, &mut scan);
    let mut replacements = Vec::new();
    reflower.indent_text_blocks(&mut replacements, &scan.text_blocks);
    reflower.wrap_long_strings(&mut replacements, &scan.long_strings);
    replacements
}

impl<'s> Reflower<'s> {
    fn column(&self, pos: usize) -> usize {
        let start = self.lines.line_start(self.lines.line(pos));
        jlen(&self.input[start..pos])
    }

    /// Collects the text blocks and the string literals on overlong lines, in source order.
    fn scan<'t>(&self, t: &'t Tree, s: &mut Scan<'t>) {
        if is_string_literal(t) {
            if self.input[t.start..].starts_with(TEXT_BLOCK_DELIMITER) {
                s.text_blocks.push(s.found(t));
                return;
            }
            // The receiver of a field access or method call.
            if let Some(Enclosing { tree: Some(p), .. }) = s.ancestors.last()
                && matches!(&p.kind, K::MemberSelect { expr, .. } if std::ptr::eq(&**expr, t))
            {
                return;
            }
            let mut line_end = t.end;
            while line_end < self.input.len() && newlines::has_newline_at(self.input, line_end).is_none() {
                line_end += 1;
            }
            if self.column(line_end) > self.column_limit {
                s.long_strings.push(s.found(t));
            }
            return;
        }
        s.ancestors.push(Enclosing { start: t.start, tree: Some(t) });
        let var = |v: &'t VarDecl, s: &mut Scan<'t>| {
            s.inside(v.start, |s| {
                v.mods.annotations.iter().chain(v.ty.as_deref()).chain(v.init.as_deref()).for_each(|c| self.scan(c, s));
            })
        };
        match &t.kind {
            K::Class(c) => {
                c.mods.annotations.iter().chain(&c.type_params).for_each(|c| self.scan(c, s));
                c.record_components.iter().for_each(|v| var(v, s));
                c.extends.as_deref().into_iter().chain(&c.implements).chain(&c.permits).for_each(|c| self.scan(c, s));
                for e in &c.enum_constants {
                    s.inside(e.mods.start, |s| {
                        e.mods
                            .annotations
                            .iter()
                            .chain(&e.args)
                            .chain(e.body.iter().flatten())
                            .for_each(|c| self.scan(c, s));
                    });
                }
                c.members.iter().for_each(|c| self.scan(c, s));
            }
            K::Method(m) => {
                m.mods
                    .annotations
                    .iter()
                    .chain(&m.type_params)
                    .chain(&m.return_type_annotations)
                    .chain(m.ret.as_deref())
                    .for_each(|c| self.scan(c, s));
                m.receiver.as_deref().into_iter().chain(&m.params).for_each(|v| var(v, s));
                m.throws
                    .iter()
                    .chain(m.body.as_deref())
                    .chain(m.default_value.as_deref())
                    .for_each(|c| self.scan(c, s));
            }
            K::Switch { selector, cases } | K::SwitchExpr { selector, cases } => {
                self.scan(selector, s);
                for c in cases {
                    s.inside(c.start, |s| {
                        c.labels
                            .iter()
                            .chain(c.guard.as_deref())
                            .chain(&c.stmts)
                            .chain(c.body.as_deref())
                            .for_each(|c| self.scan(c, s));
                    });
                }
            }
            K::Try { resources, block, catches, finally } => {
                resources.iter().chain(std::iter::once(&**block)).for_each(|c| self.scan(c, s));
                for c in catches {
                    s.inside(c.start, |s| {
                        var(&c.param, s);
                        self.scan(&c.block, s);
                    });
                }
                finally.iter().for_each(|c| self.scan(c, s));
            }
            _ => t.for_each_child(&mut |c| self.scan(c, s)),
        }
        s.ancestors.pop();
    }

    fn indent_text_blocks(&self, replacements: &mut Replacements, text_blocks: &[Found]) {
        let custom = self.compute_custom_text_blocks_indent(text_blocks);
        for (tb, custom) in text_blocks.iter().zip(custom) {
            let (start, end) = (tb.lit.start, tb.lit.end);
            let text = &self.input[start..end];
            let line_start = self.lines.line_start(self.lines.line(start));
            let start_column = first_non_ws(&self.input[line_start..end]).unwrap_or(0);
            let initial_lines = java_lines(text);
            let stripped = strip_indent(&initial_lines[1..].join(self.separator));
            let lines = java_lines(&stripped);
            let prefix = custom.unwrap_or_else(|| {
                let extra = if line_start + start_column + 4 > start { "" } else { "    " };
                format!("{extra}{}", " ".repeat(start_column))
            });
            let mut output = String::from(initial_lines[0]);
            for (i, line) in lines.iter().enumerate() {
                output.push_str(self.separator);
                if !line.is_empty() {
                    output.push_str(&prefix);
                }
                if i == lines.len() - 1 {
                    let without_delimiter =
                        line[..line.len() - TEXT_BLOCK_DELIMITER.len()].trim_end_matches(char::is_whitespace);
                    if !without_delimiter.is_empty() {
                        output.push_str(without_delimiter);
                        output.push('\\');
                        output.push_str(self.separator);
                        output.push_str(&prefix);
                    }
                    output.push_str(TEXT_BLOCK_DELIMITER);
                } else {
                    output.push_str(line);
                }
            }
            put(replacements, start, end, output);
        }
    }

    /// The indentation of each text block that is an argument of a method call or part of a concatenation, `None`
    /// for the others.
    fn compute_custom_text_blocks_indent(&self, text_blocks: &[Found]) -> Vec<Option<String>> {
        // Keyed by the address of the parent tree.
        let mut parent_to_indent: HashMap<*const Tree, String> = HashMap::new();
        let mut result = Vec::with_capacity(text_blocks.len());
        for tb in text_blocks {
            let indent = match tb.parent().map(|p| (p, &p.kind)) {
                Some((parent, K::MethodInvocation { select, args, .. }))
                    if args.iter().any(|a| std::ptr::eq(a, tb.lit)) =>
                {
                    let args: Vec<&Tree> = args.iter().collect();
                    Some(
                        parent_to_indent
                            .entry(parent)
                            .or_insert_with(|| self.compute_prefix_indentation(parent.start, select.end, &args, false))
                            .clone(),
                    )
                }
                Some((parent, _)) if is_plus(parent) => {
                    let root = tb.tree(tb.concatenation());
                    let flat = flatten_expression_tree(root);
                    Some(
                        parent_to_indent
                            .entry(root)
                            .or_insert_with(|| self.compute_prefix_indentation(root.start, root.end, &flat, true))
                            .clone(),
                    )
                }
                _ => None,
            };
            result.push(indent);
        }
        result
    }

    fn compute_prefix_indentation(
        &self,
        parent_start: usize,
        parent_end: usize,
        children: &[&Tree],
        use_start_line: bool,
    ) -> String {
        let start_parent_line = self.lines.line(parent_start);
        let end_parent_line = self.lines.line(parent_end);
        let parent_line = if use_start_line { start_parent_line } else { end_parent_line };
        let line_parent_start = self.lines.line_start(parent_line);
        let start_parent_column = first_non_ws(&self.input[line_parent_start..parent_end]).unwrap_or(0) as isize;
        let mut extra_indent: isize = 4;
        for e in children {
            let starting_pos = e.start;
            let start_line = self.lines.line(starting_pos);
            let line_start = self.lines.line_start(start_line);
            if start_line == parent_line {
                continue;
            }
            let start_column = first_non_ws(&self.input[line_start..e.end]).unwrap_or(0);
            if self.input[line_start + start_column..].starts_with(TEXT_BLOCK_DELIMITER)
                && starting_pos != line_start + start_column
            {
                continue;
            }
            extra_indent = extra_indent.max(start_column as isize - start_parent_column);
        }
        " ".repeat((extra_indent + start_parent_column).max(0) as usize)
    }

    fn wrap_long_strings(&self, replacements: &mut Replacements, long_strings: &[Found]) {
        for lit in long_strings {
            let enclosing = lit.concatenation();
            let (flat, first) = self.flatten(lit.lit, lit.tree(enclosing));
            // The outermost enclosing node that starts on the same line.
            let mut starting = enclosing;
            while starting > 0 && self.lines.line(lit.start(starting)) == self.lines.line(lit.start(starting - 1)) {
                starting -= 1;
            }
            let start_column = self.column(flat[0].start);
            let first_line_col = self.column(lit.start(starting));
            let end = flat[flat.len() - 1].end;
            let mut line_end = end;
            while line_end < self.input.len() && newlines::has_newline_at(self.input, line_end).is_none() {
                line_end += 1;
            }
            let trailing = line_end - end;
            let components = self.string_components(&flat);
            put(
                replacements,
                flat[0].start,
                end,
                reflow(self.separator, self.column_limit, trailing, components, first, start_column, first_line_col),
            );
        }
    }

    /// The run of string literals around `lit` in the concatenation `enclosing`, and whether it starts the
    /// concatenation.
    fn flatten<'t>(&self, lit: &'t Tree, enclosing: &'t Tree) -> (Vec<&'t Tree>, bool) {
        let flat = flatten_expression_tree(enclosing);
        let idx = flat.iter().position(|&n| std::ptr::eq(n, lit)).expect("the literal is part of the concatenation");
        let mut start_idx = idx;
        let mut end_idx = idx + 1;
        while start_idx > 0
            && is_string_literal(flat[start_idx - 1])
            && self.no_comments(flat[start_idx - 1], flat[start_idx])
        {
            start_idx -= 1;
        }
        while end_idx < flat.len()
            && is_string_literal(flat[end_idx])
            && self.no_comments(flat[end_idx - 1], flat[end_idx])
        {
            end_idx += 1;
        }
        (flat[start_idx..end_idx].to_vec(), start_idx == 0)
    }

    fn no_comments(&self, one: &Tree, two: &Tree) -> bool {
        self.input[one.end..two.start].chars().all(|c| c.is_whitespace() || c == '"' || c == '+')
    }

    fn string_components(&self, flat: &[&Tree]) -> Vec<String> {
        let mut result = Vec::new();
        let mut piece = String::new();
        for n in flat {
            let text = &self.input[n.start + 1..n.end - 1];
            let mut start = 0;
            let mut idx = 0;
            while idx < text.len() {
                let c = text[idx..].chars().next().unwrap();
                if c.is_whitespace() || has_escaped_whitespace_at(text, idx).is_some() {
                    // split here
                } else if has_escaped_newline_at(text, idx).is_some() {
                    while let Some(len) = has_escaped_newline_at(text, idx) {
                        idx += len;
                    }
                } else {
                    idx += c.len_utf8();
                    continue;
                }
                piece.push_str(&text[start..idx]);
                result.push(std::mem::take(&mut piece));
                start = idx;
                if idx < text.len() {
                    idx += text[idx..].chars().next().unwrap().len_utf8();
                }
            }
            if !piece.is_empty() {
                result.push(std::mem::take(&mut piece));
            }
            if start < text.len() {
                piece.push_str(&text[start..]);
            }
        }
        if !piece.is_empty() {
            result.push(piece);
        }
        result
    }
}

fn has_escaped_whitespace_at(input: &str, idx: usize) -> Option<usize> {
    input[idx..].starts_with("\\t").then_some(2)
}

fn has_escaped_newline_at(input: &str, idx: usize) -> Option<usize> {
    let mut offset = 0;
    if input[idx..].starts_with("\\r") {
        offset += 2;
    }
    if input[idx + offset..].starts_with("\\n") {
        offset += 2;
    }
    (offset > 0).then_some(offset)
}

fn reflow(
    separator: &str,
    column_limit: usize,
    trailing: usize,
    components: Vec<String>,
    first0: bool,
    text_start_column: usize,
    first_line_start_column: usize,
) -> String {
    let mut width = column_limit as isize - text_start_column as isize - 2;
    let mut input: std::collections::VecDeque<String> = components.into();
    let mut lines = Vec::new();
    let mut first = first0;
    while !input.is_empty() {
        let mut length: isize = 0;
        let mut line = Vec::new();
        if input.iter().map(|s| jlen(s) as isize).sum::<isize>() <= width {
            width -= trailing as isize;
        }
        while let Some(front) = input.front() {
            if !(length <= 4 || length + (jlen(front) as isize) < width) {
                break;
            }
            let text = input.pop_front().unwrap();
            length += jlen(&text) as isize;
            let ends_with_newline = text.ends_with("\\n") || text.ends_with("\\r");
            line.push(text);
            if ends_with_newline {
                break;
            }
        }
        if line.is_empty() {
            line.push(input.pop_front().unwrap());
        }
        lines.push(line.concat());
        if first {
            width -= 6;
            width += text_start_column as isize - first_line_start_column as isize;
            first = false;
        }
    }
    let indent = if first0 { first_line_start_column + 4 } else { text_start_column.saturating_sub(2) };
    let delimiter = format!("\"{separator}{}+ \"", " ".repeat(indent));
    format!("\"{}\"", lines.join(&delimiter))
}
