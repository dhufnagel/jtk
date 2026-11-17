//! S001: control statement body without braces (Checkstyle NeedBraces, PMD ControlStatementBraces, Eclipse "Use
//! blocks"). `else if` is allowed.

use tree_sitter::Node;

use crate::lint::{Diagnostic, Edit, Rule, source};

pub fn check(statement: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    for field in ["consequence", "alternative", "body"] {
        let Some(body) = statement.child_by_field_name(field) else { continue };
        let is_else_if = field == "alternative" && body.kind() == "if_statement";
        if body.kind() == "block" || is_else_if {
            continue;
        }
        let keyword = if field == "alternative" { else_keyword(statement) } else { *statement };
        let message = format!("'{}' body without braces", keyword_name(statement, field));
        let braced = Edit::new(body.byte_range(), format!("{{ {} }}", source(&body, text)));
        diagnostics.push(Diagnostic::new(Rule::MissingBraces, keyword.start_byte(), message).with_fix(vec![braced]));
    }
}

fn else_keyword<'t>(statement: &Node<'t>) -> Node<'t> {
    statement.children(&mut statement.walk()).find(|c| c.kind() == "else").unwrap_or(*statement)
}

fn keyword_name(statement: &Node, field: &str) -> &'static str {
    match (statement.kind(), field) {
        ("if_statement", "alternative") => "else",
        ("if_statement", _) => "if",
        ("while_statement", _) => "while",
        ("do_statement", _) => "do",
        _ => "for",
    }
}
