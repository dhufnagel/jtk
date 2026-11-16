//! Port of `StringWrapper.java`: wraps string literals that exceed the column limit, and re-indents text blocks.
//!
//! The original re-formats only the replaced ranges; formatting the whole (already formatted) file is equivalent.

use std::collections::HashMap;

use tree_sitter::Node;

use crate::newlines::{self, jlen};

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
    if replacements.is_empty() {
        // Formatting an empty set of ranges changes nothing, so the passes below would be no-ops.
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

fn need_wrapping(column_limit: usize, input: &str) -> bool {
    newlines::lines(input).iter().any(|line| jlen(line) > column_limit || line.contains(TEXT_BLOCK_DELIMITER))
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

fn is_comment(n: &Node) -> bool {
    matches!(n.kind(), "line_comment" | "block_comment")
}

fn is_plus(n: &Node, input: &str) -> bool {
    n.kind() == "binary_expression"
        && n.child_by_field_name("operator").is_some_and(|op| &input[op.byte_range()] == "+")
}

/// The parent, skipping nodes that javac does not have (e.g. argument lists).
fn javac_parent<'t>(n: &Node<'t>) -> Option<Node<'t>> {
    let p = n.parent()?;
    if p.kind() == "argument_list" { p.parent() } else { Some(p) }
}

fn flatten_expression_tree<'t>(root: Node<'t>, input: &str) -> Vec<Node<'t>> {
    let mut flat = Vec::new();
    let mut todo = vec![root];
    while let Some(first) = todo.pop() {
        if is_plus(&first, input) {
            todo.push(first.child_by_field_name("right").unwrap());
            todo.push(first.child_by_field_name("left").unwrap());
        } else {
            flat.push(first);
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
    let Some(tree) = crate::parse::parse(input) else {
        return Vec::new();
    };
    let reflower = Reflower {
        input,
        column_limit,
        separator: newlines::guess_line_separator(input),
        lines: LineMap { starts: newlines::line_offsets(input) },
    };
    let mut long_strings = Vec::new();
    let mut text_blocks = Vec::new();
    reflower.scan(tree.root_node(), &mut long_strings, &mut text_blocks);
    let mut replacements = Vec::new();
    reflower.indent_text_blocks(&mut replacements, &text_blocks);
    reflower.wrap_long_strings(&mut replacements, &long_strings);
    replacements
}

impl<'s> Reflower<'s> {
    fn column(&self, pos: usize) -> usize {
        let start = self.lines.line_start(self.lines.line(pos));
        jlen(&self.input[start..pos])
    }

    fn scan<'t>(&self, n: Node<'t>, long_strings: &mut Vec<Node<'t>>, text_blocks: &mut Vec<Node<'t>>) {
        if n.kind() == "string_literal" {
            let pos = n.start_byte();
            if self.input[pos..].starts_with(TEXT_BLOCK_DELIMITER) {
                text_blocks.push(n);
                return;
            }
            if let Some(p) = n.parent()
                && matches!(p.kind(), "field_access" | "method_invocation")
                && p.child_by_field_name("object").is_some_and(|o| o.id() == n.id())
            {
                return;
            }
            let mut line_end = n.end_byte();
            while line_end < self.input.len() && newlines::has_newline_at(self.input, line_end).is_none() {
                line_end += 1;
            }
            if self.column(line_end) <= self.column_limit {
                return;
            }
            long_strings.push(n);
            return;
        }
        let mut cursor = n.walk();
        for c in n.children(&mut cursor) {
            self.scan(c, long_strings, text_blocks);
        }
    }

    fn indent_text_blocks(&self, replacements: &mut Replacements, text_blocks: &[Node]) {
        let custom = self.compute_custom_text_blocks_indent(text_blocks);
        for tb in text_blocks {
            let start = tb.start_byte();
            let end = tb.end_byte();
            let text = &self.input[start..end];
            let line_start = self.lines.line_start(self.lines.line(start));
            let start_column = first_non_ws(&self.input[line_start..end]).unwrap_or(0);
            let initial_lines = java_lines(text);
            let stripped = strip_indent(&initial_lines[1..].join(self.separator));
            let lines = java_lines(&stripped);
            let prefix = custom.get(&tb.id()).cloned().unwrap_or_else(|| {
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

    fn compute_custom_text_blocks_indent(&self, text_blocks: &[Node]) -> HashMap<usize, String> {
        let mut parent_to_indent: HashMap<usize, String> = HashMap::new();
        let mut result = HashMap::new();
        for tb in text_blocks {
            let Some(parent) = javac_parent(tb) else { continue };
            let is_arg = tb.parent().is_some_and(|p| p.kind() == "argument_list");
            if is_arg && matches!(parent.kind(), "method_invocation" | "explicit_constructor_invocation") {
                let args_node = tb.parent().unwrap();
                let mut cursor = args_node.walk();
                let args: Vec<Node> = args_node.named_children(&mut cursor).filter(|c| !is_comment(c)).collect();
                let select_end = parent
                    .child_by_field_name(if parent.kind() == "method_invocation" { "name" } else { "constructor" })
                    .map(|n| n.end_byte())
                    .unwrap_or(parent.end_byte());
                let indent = parent_to_indent
                    .entry(parent.id())
                    .or_insert_with(|| self.compute_prefix_indentation(parent.start_byte(), select_end, &args, false))
                    .clone();
                result.insert(tb.id(), indent);
            } else if is_plus(&parent, self.input) {
                let mut root = parent;
                while let Some(p) = root.parent()
                    && is_plus(&p, self.input)
                {
                    root = p;
                }
                let flat = flatten_expression_tree(root, self.input);
                let indent = parent_to_indent
                    .entry(root.id())
                    .or_insert_with(|| self.compute_prefix_indentation(root.start_byte(), root.end_byte(), &flat, true))
                    .clone();
                result.insert(tb.id(), indent);
            }
        }
        result
    }

    fn compute_prefix_indentation(
        &self,
        parent_start: usize,
        parent_end: usize,
        children: &[Node],
        use_start_line: bool,
    ) -> String {
        let start_parent_line = self.lines.line(parent_start);
        let end_parent_line = self.lines.line(parent_end);
        let parent_line = if use_start_line { start_parent_line } else { end_parent_line };
        let line_parent_start = self.lines.line_start(parent_line);
        let start_parent_column = first_non_ws(&self.input[line_parent_start..parent_end]).unwrap_or(0) as isize;
        let mut extra_indent: isize = 4;
        for e in children {
            let starting_pos = e.start_byte();
            let start_line = self.lines.line(starting_pos);
            let line_start = self.lines.line_start(start_line);
            if start_line == parent_line {
                continue;
            }
            let start_column = first_non_ws(&self.input[line_start..e.end_byte()]).unwrap_or(0);
            if self.input[line_start + start_column..].starts_with(TEXT_BLOCK_DELIMITER)
                && starting_pos != line_start + start_column
            {
                continue;
            }
            extra_indent = extra_indent.max(start_column as isize - start_parent_column);
        }
        " ".repeat((extra_indent + start_parent_column).max(0) as usize)
    }

    fn wrap_long_strings(&self, replacements: &mut Replacements, long_strings: &[Node]) {
        for lit in long_strings {
            let mut enclosing = *lit;
            while let Some(p) = enclosing.parent()
                && is_plus(&p, self.input)
            {
                enclosing = p;
            }
            let (flat, first) = self.flatten(lit, enclosing);
            let mut starting = enclosing;
            while let Some(p) = javac_parent(&starting)
                && self.lines.line(starting.start_byte()) == self.lines.line(p.start_byte())
            {
                starting = p;
            }
            let start_column = self.column(flat[0].start_byte());
            let first_line_col = self.column(starting.start_byte());
            let end = flat[flat.len() - 1].end_byte();
            let mut line_end = end;
            while line_end < self.input.len() && newlines::has_newline_at(self.input, line_end).is_none() {
                line_end += 1;
            }
            let trailing = line_end - end;
            let components = self.string_components(&flat);
            put(
                replacements,
                flat[0].start_byte(),
                end,
                reflow(self.separator, self.column_limit, trailing, components, first, start_column, first_line_col),
            );
        }
    }

    fn flatten<'t>(&self, lit: &Node<'t>, enclosing: Node<'t>) -> (Vec<Node<'t>>, bool) {
        let flat = flatten_expression_tree(enclosing, self.input);
        let idx = flat.iter().position(|n| n.id() == lit.id()).unwrap();
        let mut start_idx = idx;
        let mut end_idx = idx + 1;
        while start_idx > 0
            && flat[start_idx - 1].kind() == "string_literal"
            && self.no_comments(&flat[start_idx - 1], &flat[start_idx])
        {
            start_idx -= 1;
        }
        while end_idx < flat.len()
            && flat[end_idx].kind() == "string_literal"
            && self.no_comments(&flat[end_idx - 1], &flat[end_idx])
        {
            end_idx += 1;
        }
        (flat[start_idx..end_idx].to_vec(), start_idx == 0)
    }

    fn no_comments(&self, one: &Node, two: &Node) -> bool {
        self.input[one.end_byte()..two.start_byte()].chars().all(|c| c.is_whitespace() || c == '"' || c == '+')
    }

    fn string_components(&self, flat: &[Node]) -> Vec<String> {
        let mut result = Vec::new();
        let mut piece = String::new();
        for n in flat {
            let text = &self.input[n.start_byte() + 1..n.end_byte() - 1];
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
