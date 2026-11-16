//! Port of `JavaCommentsHelper.java`: re-indents and wraps comments. Javadoc formatting is off in the Palantir style.

use crate::input::Tok;
use crate::newlines;

pub struct CommentsHelper {
    line_separator: &'static str,
    max_line_length: usize,
}

fn is_ws(c: char) -> bool {
    c.is_whitespace()
}

impl CommentsHelper {
    pub fn new(line_separator: &'static str, max_line_length: usize) -> CommentsHelper {
        CommentsHelper { line_separator, max_line_length }
    }

    pub fn rewrite(&self, tok: &Tok, _max_width: i32, column0: i32) -> String {
        if !tok.is_comment() {
            return tok.text.clone();
        }
        let column0 = column0.max(0) as usize;
        let lines: Vec<String> =
            newlines::lines(&tok.text).into_iter().map(|l| l.trim_end_matches(is_ws).to_string()).collect();
        if tok.is_slash_slash_comment() {
            self.indent_line_comments(lines, column0)
        } else if javadoc_shaped(&lines) {
            self.indent_javadoc(&lines, column0)
        } else {
            self.preserve_indentation(&lines, column0)
        }
    }

    fn preserve_indentation(&self, lines: &[String], column0: usize) -> String {
        let mut builder = String::new();
        let mut start_col: Option<usize> = None;
        for line in &lines[1..] {
            if let Some(idx) = line.chars().position(|c| !is_ws(c))
                && start_col.is_none_or(|s| idx < s)
            {
                start_col = Some(idx);
            }
        }
        builder.push_str(&lines[0]);
        let start_col = start_col.map(|s| s as isize).unwrap_or(-1);
        for line in &lines[1..] {
            builder.push_str(self.line_separator);
            builder.push_str(&" ".repeat(column0));
            let len = line.chars().count() as isize;
            if len >= start_col && start_col >= 0 {
                builder.extend(line.chars().skip(start_col as usize));
            } else {
                builder.push_str(line);
            }
        }
        builder
    }

    fn indent_line_comments(&self, lines: Vec<String>, column0: usize) -> String {
        let lines = self.wrap_line_comments(lines, column0);
        let mut builder = String::new();
        builder.push_str(lines[0].trim());
        let indent = " ".repeat(column0);
        for line in &lines[1..] {
            builder.push_str(self.line_separator);
            builder.push_str(&indent);
            builder.push_str(line.trim());
        }
        builder
    }

    fn wrap_line_comments(&self, lines: Vec<String>, column0: usize) -> Vec<String> {
        let mut result = Vec::new();
        for mut line in lines {
            if let Some(len) = missing_space_prefix(&line) {
                line = format!("{} {}", "/".repeat(len), &line[len..]);
            }
            if line.starts_with("// MOE:") {
                result.push(line);
                continue;
            }
            let prefix_len = line.chars().take_while(|&c| c == '/').count();
            let prefix = "/".repeat(prefix_len);
            loop {
                let chars: Vec<char> = line.chars().collect();
                if chars.len() + column0 <= self.max_line_length {
                    break;
                }
                let mut idx = self.max_line_length as isize - column0 as isize;
                while idx >= prefix_len as isize && !is_ws(chars[idx as usize]) {
                    idx -= 1;
                }
                if idx <= prefix_len as isize {
                    break;
                }
                let idx = idx as usize;
                result.push(chars[..idx].iter().collect());
                line = format!("{}{}", prefix, chars[idx..].iter().collect::<String>());
            }
            result.push(line);
        }
        result
    }

    fn indent_javadoc(&self, lines: &[String], column0: usize) -> String {
        let mut builder = String::new();
        builder.push_str(lines[0].trim());
        let indent = " ".repeat(column0 + 1);
        for line in &lines[1..] {
            builder.push_str(self.line_separator);
            builder.push_str(&indent);
            let line = line.trim();
            if !line.starts_with('*') {
                builder.push_str("* ");
            }
            builder.push_str(line);
        }
        builder
    }
}

/// Matches `^(//+)(?!noinspection|\$NON-NLS-\d+\$)[^\s/]`, returning the length of the slash group.
fn missing_space_prefix(line: &str) -> Option<usize> {
    let slashes = line.chars().take_while(|&c| c == '/').count();
    if slashes < 2 {
        return None;
    }
    let rest = &line[slashes..];
    if rest.starts_with("noinspection") {
        return None;
    }
    if let Some(r) = rest.strip_prefix("$NON-NLS-") {
        let digits = r.chars().take_while(|c| c.is_ascii_digit()).count();
        if digits > 0 && r[digits..].starts_with('$') {
            return None;
        }
    }
    let next = rest.chars().next()?;
    if is_ws(next) || next == '/' {
        return None;
    }
    Some(slashes)
}

fn javadoc_shaped(lines: &[String]) -> bool {
    let Some(first) = lines.first() else {
        return false;
    };
    let first = first.trim();
    if first.starts_with("/**") {
        return true;
    }
    if !first.starts_with("/*") {
        return false;
    }
    lines[1..].iter().all(|l| l.trim().starts_with('*'))
}
