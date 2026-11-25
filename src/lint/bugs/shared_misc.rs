//! Helpers shared by the rules B035 to B280 of batch 2: string literal values, type names resolved through the
//! file's imports, the receivers of `String` methods and a conservative check of Java regular expressions.

use super::shared_b::declared_type;
use crate::lint::tree::Context;
use jtk_parser::ast::{K, LitKind, Tree};

/// The value of a string literal that is not a text block and has no Unicode escapes (which apply before lexing).
pub(super) fn string_value(tree: &Tree, text: &str) -> Option<String> {
    let K::Literal(LitKind::String) = tree.kind else { return None };
    let code = &text[tree.start..tree.end];
    if code.starts_with("\"\"\"") || code.contains("\\u") {
        return None;
    }
    unescape(code.strip_prefix('"')?.strip_suffix('"')?)
}

/// The value of the content of a string or character literal.
fn unescape(content: &str) -> Option<String> {
    let mut value = String::new();
    let mut chars = content.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            value.push(c);
            continue;
        }
        let escaped = chars.next()?;
        value.push(match escaped {
            'b' => '\u{8}',
            't' => '\t',
            'n' => '\n',
            'f' => '\u{c}',
            'r' => '\r',
            's' => ' ',
            '"' | '\'' | '\\' => escaped,
            '0'..='7' => {
                let max_digits = if escaped <= '3' { 3 } else { 2 };
                let mut code = escaped.to_digit(8)?;
                for _ in 1..max_digits {
                    match chars.peek().and_then(|c| c.to_digit(8)) {
                        Some(digit) => {
                            code = code * 8 + digit;
                            chars.next();
                        }
                        None => break,
                    }
                }
                char::from_u32(code)?
            }
            _ => return None,
        });
    }
    Some(value)
}

/// Whether `tree` names the type `qualified` (such as `java.util.regex.Pattern`): fully qualified, or by its simple
/// name imported with a single-type import, an import on demand of its package or implicitly from `java.lang`. A
/// simple name is not resolved if the file declares a type of that name or imports another one with it (a type of the
/// same package in another file, which would shadow an import on demand, is not seen).
pub(super) fn names_type(tree: &Tree, qualified: &str, context: &Context) -> bool {
    let (package, simple) = qualified.rsplit_once('.').unwrap_or(("", qualified));
    match &tree.kind {
        K::Ident(name) if name == simple => {
            let unit = context.ancestors[0];
            let imports = match &unit.kind {
                K::CompilationUnit { imports, .. } => imports.as_slice(),
                _ => &[],
            };
            let mut imported = package == "java.lang";
            for import in imports {
                let K::Import { is_static: false, name } = &import.kind else { continue };
                let name: String = context.source(name).split_whitespace().collect();
                if name == qualified || name == format!("{package}.*") {
                    imported = true;
                } else if name.rsplit_once('.').is_some_and(|(_, last)| last == simple) {
                    return false;
                }
            }
            imported && !declares_type(unit, simple)
        }
        K::MemberSelect { .. } => context.source(tree).split_whitespace().collect::<String>() == qualified,
        _ => false,
    }
}

/// Whether a type named `simple` is declared anywhere in the tree.
fn declares_type(tree: &Tree, simple: &str) -> bool {
    let mut found = matches!(&tree.kind, K::Class(class) if class.name == simple)
        || matches!(&tree.kind, K::TypeParameter { name, .. } if name == simple);
    if !found {
        tree.for_each_child(&mut |child| found = found || declares_type(child, simple));
    }
    found
}

/// Whether the receiver of the checked method call is a `String`: `Some(true)` for a string literal or a variable
/// declared as `String`, `Some(false)` for one declared with another type, `this`, `super` and a capitalized name (a
/// class, for a static method), `None` if unknown.
pub(super) fn is_string(receiver: &Tree, call: &Tree, context: &Context) -> Option<bool> {
    match &receiver.kind {
        K::Literal(LitKind::String) => Some(true),
        K::Ident(name) if name == "this" || name == "super" || name.starts_with(char::is_uppercase) => Some(false),
        K::Ident(name) => {
            let mut path = context.ancestors.clone();
            path.push(call);
            let ty = declared_type(&path, name, context.text)?;
            Some(matches!(ty, "String" | "java.lang.String"))
        }
        K::Parens(inner) => is_string(inner, call, context),
        _ => None,
    }
}

/// The description of a syntax error of the Java regular expression (`java.util.regex.Pattern` without flags) that
/// certainly makes it invalid, if it finds one: an unescaped trailing backslash, an unknown escape of a letter, an
/// unclosed character class or group, an unmatched `)`, a quantifier without an operand (such as a leading `*` or
/// `a**`), `{` not starting a repetition `{n}`, `{n,}` or `{n,m}`, or a reversed repetition or character range.
/// Compared with `Pattern.compile` on 160,000 random patterns, it reported no valid one as invalid.
///
/// The check is conservative: what it does not model (Unicode properties, named groups, inline flags other than `i`,
/// `m`, `s`, `u`, `d` and `U`, ...) ends it without an error.
pub(super) fn regex_error(pattern: &str) -> Option<&'static str> {
    RegexChecker { chars: pattern.chars().collect(), at: 0 }.check().err().flatten()
}

/// What the checker found: `Some` a certain error, `None` something it does not model.
type Found = Option<&'static str>;

struct RegexChecker {
    chars: Vec<char>,
    at: usize,
}

/// What precedes a position in a sequence, for quantifiers.
#[derive(Clone, Copy, PartialEq)]
enum Before {
    /// The start of the pattern, a group or an alternative, or inline flags: a quantifier has no operand.
    Nothing,
    /// An atom that a quantifier may follow.
    Atom,
    /// A quantifier, possibly with its `?` or `+` modifier: a further quantifier has no operand.
    Quantifier,
    /// A quantifier without a modifier: `?` or `+` may follow.
    BareQuantifier,
}

impl RegexChecker {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }

    fn check(&mut self) -> Result<(), Found> {
        let mut depth = 0usize;
        let mut before = Before::Nothing;
        while let Some(c) = self.peek() {
            self.at += 1;
            before = match c {
                '\\' => {
                    self.escape(false)?;
                    Before::Atom
                }
                '[' => {
                    self.class()?;
                    Before::Atom
                }
                '(' => {
                    if self.peek() == Some('?') {
                        self.at += 1;
                        if self.group_kind()? {
                            depth += 1;
                        }
                    } else {
                        depth += 1;
                    }
                    Before::Nothing
                }
                ')' => {
                    depth = depth.checked_sub(1).ok_or(Some("unmatched closing ')'"))?;
                    Before::Atom
                }
                '|' => Before::Nothing,
                '*' | '+' | '?' => match before {
                    Before::BareQuantifier if c != '*' => Before::Quantifier,
                    Before::Atom => Before::BareQuantifier,
                    _ => return Err(Some("dangling quantifier")),
                },
                // Like `Pattern`, a repetition may follow anything.
                '{' => {
                    self.repetition()?;
                    Before::BareQuantifier
                }
                _ => Before::Atom,
            };
        }
        if depth > 0 { Err(Some("unclosed group")) } else { Ok(()) }
    }

    /// After `(?`: whether the construct opens a group (`false` for inline flags `(?i)`).
    fn group_kind(&mut self) -> Result<bool, Found> {
        match self.peek() {
            Some(':' | '=' | '!' | '>') => {
                self.at += 1;
                Ok(true)
            }
            Some('<') if matches!(self.chars.get(self.at + 1), Some('=' | '!')) => {
                self.at += 2;
                Ok(true)
            }
            _ => {
                // Inline flags, `(?i)` or `(?i:`; others (`x` changes the syntax, named groups) are not modelled.
                while let Some(c) = self.peek() {
                    self.at += 1;
                    match c {
                        'i' | 'm' | 's' | 'u' | 'd' | 'U' | '-' => {}
                        ')' => return Ok(false),
                        ':' => return Ok(true),
                        _ => return Err(None),
                    }
                }
                Err(None)
            }
        }
    }

    /// After `{` following an atom: `n}`, `n,}` or `n,m}` with `n <= m`.
    fn repetition(&mut self) -> Result<(), Found> {
        let min = self.number().ok_or(Some("illegal repetition"))?;
        let max = if self.peek() == Some(',') {
            self.at += 1;
            if self.peek() == Some('}') { None } else { Some(self.number().ok_or(Some("illegal repetition"))?) }
        } else {
            Some(min)
        };
        if self.peek() != Some('}') {
            return Err(Some("unclosed repetition"));
        }
        self.at += 1;
        match max {
            Some(max) if max < min => Err(Some("illegal repetition range")),
            _ => Ok(()),
        }
    }

    /// A decimal number of at most nine digits (larger ones are not modelled).
    fn number(&mut self) -> Option<u32> {
        let start = self.at;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.at += 1;
        }
        let digits: String = self.chars[start..self.at].iter().collect();
        (1..=9).contains(&digits.len()).then(|| digits.parse().ok()).flatten()
    }

    /// After `\`: the escaped character or sequence; `in_class` inside a character class. Returns the character
    /// it stands for if it is a single known one, such as a literal.
    fn escape(&mut self, in_class: bool) -> Result<Option<char>, Found> {
        let Some(c) = self.peek() else {
            return Err(Some(if in_class { "unclosed character class" } else { "unescaped trailing backslash" }));
        };
        self.at += 1;
        match c {
            // Constructs with arguments, quotes and octal escapes are not modelled, nor boundaries in classes.
            'p' | 'P' | 'x' | 'u' | 'c' | 'N' | 'k' | 'Q' | 'E' | '0' => Err(None),
            'b' | 'B' | 'R' | 'X' if in_class || self.peek() == Some('{') => Err(None),
            'b' | 'B' | 'R' | 'X' => Ok(None),
            't' => Ok(Some('\t')),
            'n' => Ok(Some('\n')),
            'r' => Ok(Some('\r')),
            'f' => Ok(Some('\u{c}')),
            'a' => Ok(Some('\u{7}')),
            'e' => Ok(Some('\u{1b}')),
            'd' | 'D' | 'h' | 'H' | 's' | 'S' | 'v' | 'V' | 'w' | 'W' | 'A' | 'G' | 'Z' | 'z' => Ok(None),
            '1'..='9' if in_class => Err(None),
            '1'..='9' => Ok(None),
            _ if c.is_ascii_alphanumeric() => Err(Some("illegal escape sequence")),
            _ => Ok(Some(c)),
        }
    }

    /// After `[`: the rest of the character class, with nested classes and ranges.
    fn class(&mut self) -> Result<(), Found> {
        if self.peek() == Some('^') {
            self.at += 1;
        }
        // A `]` right after the opening is a literal.
        let mut first = true;
        // The last single character, which may start a range.
        let mut previous: Option<char> = None;
        loop {
            let Some(c) = self.peek() else { return Err(Some("unclosed character class")) };
            self.at += 1;
            previous = match c {
                ']' if !first => return Ok(()),
                '[' => {
                    self.class()?;
                    None
                }
                '\\' => self.escape(true)?,
                '&' if self.peek() == Some('&') => {
                    self.at += 1;
                    None
                }
                '-' if previous.is_some() && !matches!(self.peek(), Some(']' | '[') | None) => {
                    let end = match self.peek() {
                        Some('\\') => {
                            self.at += 1;
                            self.escape(true)?
                        }
                        end => {
                            self.at += 1;
                            end
                        }
                    };
                    match (previous, end) {
                        (Some(start), Some(end)) if end < start => return Err(Some("illegal character range")),
                        (_, None) => return Err(None),
                        _ => {}
                    }
                    None
                }
                _ => Some(c),
            };
            first = false;
        }
    }
}
