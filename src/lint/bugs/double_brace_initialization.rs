//! B025: an anonymous class whose body holds only instance initializers, `new ArrayList<>() {{ add(1); }}`
//! (Checkstyle AvoidDoubleBraceInitialization, PMD DoubleBraceInitialization): it creates a class per use and keeps
//! a reference to the enclosing instance.

use tree_sitter::Node;

use crate::lint::syntax::is_comment;
use crate::lint::{Diagnostic, Rule};

pub fn check(creation: &Node, _text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let Some(body) = creation.named_children(&mut creation.walk()).find(|c| c.kind() == "class_body") else {
        return;
    };
    let members: Vec<Node> = body.named_children(&mut body.walk()).filter(|c| !is_comment(c)).collect();
    if !members.is_empty() && members.iter().all(|m| m.kind() == "block") {
        diagnostics.push(Diagnostic::new(
            Rule::DoubleBraceInitialization,
            body.start_byte(),
            "Avoid double brace initialization",
        ));
    }
}
